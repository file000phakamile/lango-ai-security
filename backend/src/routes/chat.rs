//! Native in-app chat (Phase 2). Reuses `scan_prompt`/`scan_response`
//! exactly as `routes::scan`/`routes::response_scan` already do — no
//! detection logic lives in this file. What's genuinely new here: calling a
//! real AI provider (`providers::openai::OpenAiProvider`) with the redacted
//! prompt, streaming its reply back to the browser as it arrives, and
//! running the response scan in the background once the reply is fully
//! assembled (mirroring the browser extension's own fail-open,
//! flag-after-the-fact response-scanning behavior — see
//! `extension/content/response-scanner.js` — but without that module's DOM-
//! mutation debounce heuristic, since this backend knows exactly when its
//! own stream ends).
//!
//! **Wire protocol for a successful (non-blocked) response**: the HTTP
//! response body is `text/plain`, starting with one JSON object
//! (`models::ChatStreamMeta`, serialized), a single `\0` byte, and then the
//! assistant's reply streamed as plain UTF-8 text chunks. A custom response
//! header was considered instead and rejected: `HeaderValue` requires
//! visible ASCII, and `user_message` is built dynamically from detected
//! entity names — not ASCII-guaranteed. No SES/EventSource framing is used
//! either, matching this codebase's existing "plain `fetch` + manual
//! `Authorization` header" convention (native `EventSource` cannot set
//! custom headers, so it was never a fit here — see Questions.md).
//!
//! A blocked prompt (`decision == "blocked_low_confidence"`) is the one
//! case that returns a normal, non-streamed JSON body instead
//! (`models::ChatBlockedResponse`) — no provider call, no conversation/
//! message rows written, same "a block prevents sending, nothing else
//! happens" behavior as the extension.
use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    response::{IntoResponse, Response},
    Json,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    crypto,
    detection::scan::{hash_prompt, response_scan_result_for, scan_prompt_with_config, scan_response, ScanConfig},
    error::{AppError, AppResult},
    models::{
        ChatBlockedResponse, ChatConversationRow, ChatConversationsResponse, ChatMessageRow,
        ChatMessagesResponse, ChatRequest, ChatStreamMeta,
    },
    providers::{openai::OpenAiProvider, ChatProvider, ChatTurn},
    routes::response_scan::update_audit_log_response_scan,
    routes::scan::{check_consent, insert_audit_log_row, load_scan_config},
    state::AppState,
};

/// `audit_log.ai_model_used` for a chat turn that never reached OpenAI —
/// mirrors `detection::scan::NO_PROVIDER_MODEL_LABEL`'s honesty but is
/// worded for a codebase where a live provider genuinely IS connected for
/// forwarded turns, just not for this specific blocked one.
const BLOCKED_AI_MODEL_LABEL: &str = "not applicable - blocked pre-gateway, no provider called";

/// `audit_log.response_scan_result` for a forwarded chat turn, immediately
/// after the prompt-scan INSERT and before the async response scan
/// completes — distinct from `response_scan_result_for`'s existing strings
/// (which describe the extension's provider-less `/api/scan` path, still
/// accurate for that path but not this one).
const PENDING_RESPONSE_SCAN_RESULT: &str = "pending - awaiting async response scan";

pub async fn chat(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<ChatRequest>,
) -> AppResult<Response> {
    if payload.message.trim().is_empty() {
        return Err(AppError::BadRequest("message must not be empty.".to_string()));
    }
    if payload.message.len() > 20_000 {
        return Err(AppError::BadRequest(
            "message exceeds the maximum accepted length (20,000 chars).".to_string(),
        ));
    }

    let (_, scan_config) = tokio::try_join!(
        check_consent(&state.db, claims.sub),
        load_scan_config(&state.db, claims.organisation_id)
    )?;

    // Ownership check for an existing conversation — a real WHERE clause on
    // BOTH organisation_id and user_id, same discipline as every other
    // multi-tenant query in this codebase (see
    // tests/chat_multi_tenant_isolation.rs). A brand-new conversation is
    // NOT created here — creation is deferred until we know the prompt will
    // actually be forwarded, so a blocked prompt never leaves an empty
    // conversation behind.
    let existing_conversation_id = match payload.conversation_id {
        Some(id) => {
            let owned: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM chat_conversations WHERE id = $1 AND organisation_id = $2 AND user_id = $3",
            )
            .bind(id)
            .bind(claims.organisation_id)
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?;
            Some(owned.ok_or_else(|| AppError::NotFound("Conversation not found in your account.".to_string()))?)
        }
        None => None,
    };

    // Step 1 of the required flow: run the existing scan_prompt logic,
    // exactly as built, no changes to detection logic itself.
    let outcome = scan_prompt_with_config(&payload.message, &scan_config);
    let original_prompt_hash = hash_prompt(&payload.message);
    let entities_json = serde_json::to_value(&outcome.entities_detected)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Step 2: if the decision blocks, return the block immediately — do not
    // call OpenAI at all.
    if outcome.decision == "blocked_low_confidence" {
        insert_audit_log_row(
            &state.db,
            claims.session_id,
            claims.sub,
            &claims.department,
            &None,
            &entities_json,
            outcome.risk_score,
            outcome.decision,
            &outcome.reason_string,
            BLOCKED_AI_MODEL_LABEL,
            response_scan_result_for(outcome.decision),
            &original_prompt_hash,
            &None,
            outcome.sensitivity_class,
            &None,
            claims.organisation_id,
        )
        .await?;

        tracing::info!(
            organisation_id = %claims.organisation_id,
            decision = outcome.decision,
            risk_score = outcome.risk_score,
            "chat prompt scanned and blocked"
        );

        return Ok(Json(ChatBlockedResponse {
            blocked: true,
            conversation_id: existing_conversation_id,
            decision: outcome.decision.to_string(),
            reason_string: outcome.reason_string,
            user_message: outcome.user_message,
            entities_detected: outcome.entities_detected,
        })
        .into_response());
    }

    // Steps 3-5: redacts-and-forwards (either tier) or clears — all three
    // reach OpenAI. Requires the organisation to have provisioned a key
    // (Phase 3) — checked AFTER scanning (so a misconfigured organisation
    // still gets a real, recorded scan decision, not a silent no-op) but
    // BEFORE any conversation/message row is written, so a missing key
    // never leaves a half-written turn behind.
    let stored_key: Option<(String,)> = sqlx::query_as(
        "SELECT encrypted_key FROM organization_api_keys WHERE organisation_id = $1 AND provider = 'openai'",
    )
    .bind(claims.organisation_id)
    .fetch_optional(&state.db)
    .await?;
    let (encrypted_key,) = stored_key.ok_or_else(|| {
        AppError::BadRequest(
            "Your organisation has not configured an OpenAI API key yet. Ask a compliance admin to \
             add one in Policy Builder."
                .to_string(),
        )
    })?;
    let api_key = crypto::decrypt_secret(&encrypted_key, &state.config.api_key_encryption_key)?;

    let conversation_id = match existing_conversation_id {
        Some(id) => id,
        None => {
            sqlx::query_scalar(
                "INSERT INTO chat_conversations (organisation_id, user_id) VALUES ($1, $2) RETURNING id",
            )
            .bind(claims.organisation_id)
            .bind(claims.sub)
            .fetch_one(&state.db)
            .await?
        }
    };

    let redacted_prompt_for_storage =
        if outcome.decision == "redacted_and_forwarded" || outcome.decision == "redacted_low_confidence_review" {
            Some(outcome.redacted_prompt.clone())
        } else {
            None
        };

    let audit_log_id = insert_audit_log_row(
        &state.db,
        claims.session_id,
        claims.sub,
        &claims.department,
        &None,
        &entities_json,
        outcome.risk_score,
        outcome.decision,
        &outcome.reason_string,
        &crate::providers::openai::ai_model_used_label(),
        PENDING_RESPONSE_SCAN_RESULT,
        &original_prompt_hash,
        &redacted_prompt_for_storage,
        outcome.sensitivity_class,
        &None,
        claims.organisation_id,
    )
    .await?;

    // chat_messages.redacted_content is ALWAYS outcome.redacted_prompt for a
    // user turn, regardless of decision — the user's raw message is never
    // stored (matching audit_log's principle exactly); for
    // cleared_no_entities, redacted_prompt is identical to the original
    // text, which is safe by construction (scanning found nothing sensitive
    // in it) — see migration 0017's own comment on this.
    sqlx::query(
        "INSERT INTO chat_messages (conversation_id, role, redacted_content, risk_score, decision, audit_log_id) \
         VALUES ($1, 'user', $2, $3, $4, $5)",
    )
    .bind(conversation_id)
    .bind(&outcome.redacted_prompt)
    .bind(outcome.risk_score)
    .bind(outcome.decision)
    .bind(audit_log_id)
    .execute(&state.db)
    .await?;

    tracing::info!(
        organisation_id = %claims.organisation_id,
        audit_log_id = %audit_log_id,
        decision = outcome.decision,
        risk_score = outcome.risk_score,
        "chat prompt scanned and forwarded"
    );

    // Conversation context sent to OpenAI is built entirely from stored,
    // already-redacted content (including this turn's own row, just
    // inserted above) — OpenAI never sees a raw prompt any more than the
    // database does.
    let history_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT role, redacted_content FROM chat_messages WHERE conversation_id = $1 ORDER BY created_at ASC",
    )
    .bind(conversation_id)
    .fetch_all(&state.db)
    .await?;
    let history: Vec<ChatTurn> = history_rows
        .into_iter()
        .map(|(role, content)| ChatTurn { role, content })
        .collect();

    let provider = OpenAiProvider::new(&state.config.openai_api_base_url);
    let provider_rx = provider.stream_chat(&api_key, &history).await?;

    let meta = ChatStreamMeta {
        conversation_id,
        audit_log_id,
        decision: outcome.decision.to_string(),
        user_message: outcome.user_message,
    };
    let mut prefix = serde_json::to_vec(&meta).map_err(|e| AppError::Internal(e.to_string()))?;
    prefix.push(0u8);

    let (client_tx, client_rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(32);
    // Best-effort: if this very first send fails the client disconnected
    // before anything streamed — the background task below still runs to
    // completion so the audit trail and stored message are accurate either
    // way.
    let _ = client_tx.send(Ok(Bytes::from(prefix))).await;

    tokio::spawn(stream_and_scan(
        state.clone(),
        provider_rx,
        client_tx,
        scan_config,
        audit_log_id,
        conversation_id,
    ));

    let body = Body::from_stream(ReceiverStream::new(client_rx));
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Forwards each chunk from the provider to the client as it arrives (true
/// streaming — the user never waits for the full reply), then, once the
/// provider stream ends, drops `client_tx` immediately so the client's
/// `fetch` stream completes right away, and ONLY THEN runs the response
/// scan and the DB writes it produces. This is the "in parallel with
/// streaming, run scan_response... cannot un-send what already streamed"
/// requirement translated into a concrete ordering: nothing about the
/// client's perceived latency depends on how long the response scan takes.
///
/// If a flagged response comes back, this cannot un-send what already
/// streamed — exactly like the extension's own response scanner — so
/// instead it attaches a retroactive warning: `audit_log.response_flagged`
/// and `chat_messages.response_flagged` are updated after the fact, and the
/// frontend picks this up on its next poll of the conversation (see
/// Questions.md — no websocket precedent exists anywhere in this codebase).
async fn stream_and_scan(
    state: AppState,
    mut provider_rx: mpsc::Receiver<AppResult<String>>,
    client_tx: mpsc::Sender<Result<Bytes, std::io::Error>>,
    scan_config: ScanConfig,
    audit_log_id: Uuid,
    conversation_id: Uuid,
) {
    let mut full_text = String::new();
    let mut client_connected = true;
    loop {
        match provider_rx.recv().await {
            Some(Ok(delta)) => {
                full_text.push_str(&delta);
                if client_connected && client_tx.send(Ok(Bytes::from(delta.into_bytes()))).await.is_err() {
                    // Client disconnected — keep draining the provider so
                    // the stored record still reflects the real, complete
                    // reply, just stop trying to forward bytes nobody reads.
                    client_connected = false;
                }
            }
            Some(Err(e)) => {
                tracing::error!(error = %e, "chat provider stream error");
                break;
            }
            None => break,
        }
    }
    // Ends the client's HTTP response now — the response scan below never
    // adds latency to what the user perceives as "the reply finished".
    drop(client_tx);

    if full_text.trim().is_empty() {
        return;
    }

    // Insert the assistant row now, response_flagged still NULL — see
    // migration 0017's own comment on this column's lifecycle.
    let assistant_message_id: Option<Uuid> = match sqlx::query_scalar(
        "INSERT INTO chat_messages (conversation_id, role, redacted_content, audit_log_id) \
         VALUES ($1, 'assistant', $2, $3) RETURNING id",
    )
    .bind(conversation_id)
    .bind(&full_text)
    .bind(audit_log_id)
    .fetch_one(&state.db)
    .await
    {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::error!(error = %e, "failed to insert assistant chat_messages row");
            None
        }
    };

    // Step 4 of the required flow: run the existing scan_response logic on
    // the assembled response once it stabilises — here, "stabilised" simply
    // means the provider's stream has ended, which this backend knows
    // exactly (unlike the browser extension's DOM-mutation debounce
    // heuristic, which has to guess).
    let outcome = scan_response(&full_text, &scan_config);
    let entities_json = serde_json::to_value(&outcome.entities_detected).unwrap_or(serde_json::Value::Array(vec![]));
    let response_text_hash = hash_prompt(&full_text);

    if let Err(e) = update_audit_log_response_scan(
        &state.db,
        audit_log_id,
        &entities_json,
        outcome.risk_score,
        outcome.flagged,
        &response_text_hash,
        &outcome.user_message,
    )
    .await
    {
        tracing::error!(error = %e, "failed to update audit_log with chat response scan result");
    }

    if let Some(message_id) = assistant_message_id {
        if let Err(e) = sqlx::query("UPDATE chat_messages SET risk_score = $1, response_flagged = $2 WHERE id = $3")
            .bind(outcome.risk_score)
            .bind(outcome.flagged)
            .bind(message_id)
            .execute(&state.db)
            .await
        {
            tracing::error!(error = %e, "failed to update chat_messages.response_flagged");
        }
    }

    tracing::info!(
        audit_log_id = %audit_log_id,
        flagged = outcome.flagged,
        risk_score = outcome.risk_score,
        "chat response scanned"
    );
}

pub async fn list_conversations(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<ChatConversationsResponse>> {
    let rows: Vec<ChatConversationRow> = sqlx::query_as(
        "SELECT id, title, created_at FROM chat_conversations WHERE organisation_id = $1 AND user_id = $2 \
         ORDER BY created_at DESC",
    )
    .bind(claims.organisation_id)
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(ChatConversationsResponse {
        conversations: rows.into_iter().map(Into::into).collect(),
    }))
}

pub async fn list_messages(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(conversation_id): Path<Uuid>,
) -> AppResult<Json<ChatMessagesResponse>> {
    let owned: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM chat_conversations WHERE id = $1 AND organisation_id = $2 AND user_id = $3",
    )
    .bind(conversation_id)
    .bind(claims.organisation_id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?;
    owned.ok_or_else(|| AppError::NotFound("Conversation not found in your account.".to_string()))?;

    let rows: Vec<ChatMessageRow> = sqlx::query_as(
        "SELECT id, role, redacted_content, risk_score, decision, response_flagged, created_at \
         FROM chat_messages WHERE conversation_id = $1 ORDER BY created_at ASC",
    )
    .bind(conversation_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(ChatMessagesResponse {
        messages: rows.into_iter().map(Into::into).collect(),
    }))
}

