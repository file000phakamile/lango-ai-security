//! Native chat (Phase 2) integration tests. Same `#[sqlx::test]` pattern as
//! every other file in this directory — a fresh, throwaway, fully migrated
//! Postgres per test, handlers called directly as plain async functions.
//!
//! The forwarded/streaming path is tested against a REAL local HTTP mock
//! server (`wiremock`), not the real OpenAI API — no live OpenAI key was
//! available in this environment (see Questions.md's honest verification
//! statement). `Config::openai_api_base_url` exists specifically to make
//! this possible: it's overridden here to point at the mock server, so this
//! test exercises the real SSE-parsing, real streaming-to-client, and real
//! background response-scan/DB-write code paths end to end — only the
//! actual OpenAI network call is substituted.
//!
//! Run with `cargo test --test chat` (requires `DATABASE_URL`).

use axum::extract::{Path, State};
use axum::Json;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use lango_backend::{
    auth::AuthUser,
    config::Config,
    crypto,
    models::{ChatRequest, Claims},
    routes,
    state::AppState,
};

fn test_config(openai_api_base_url: &str) -> Config {
    Config {
        database_url: String::new(),
        jwt_signing_secret: "test-only-secret".to_string(),
        port: 0,
        cors_origin: "http://localhost".to_string(),
        api_key_encryption_key: "a".repeat(64),
        openai_api_base_url: openai_api_base_url.to_string(),
    }
}

fn claims_for(user_id: Uuid, session_id: Uuid, department: &str, role: &str, organisation_id: Uuid) -> Claims {
    Claims {
        sub: user_id,
        session_id,
        email: "user@chat-test.test".to_string(),
        department: department.to_string(),
        role: role.to_string(),
        organisation_id,
        exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
    }
}

struct SeededUser {
    user_id: Uuid,
    session_id: Uuid,
}

async fn insert_org(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (name) VALUES ($1) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("insert organisation")
}

async fn insert_user(pool: &PgPool, org_id: Uuid, email: &str, role: &str) -> SeededUser {
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, department, role, organisation_id, consent_accepted_at, consent_policy_version) \
         VALUES ($1, 'unused-hash', 'General', $2, $3, now(), 'v1') RETURNING id",
    )
    .bind(email)
    .bind(role)
    .bind(org_id)
    .fetch_one(pool)
    .await
    .expect("insert user");

    let session_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (user_id, expires_at) VALUES ($1, now() + interval '1 day') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .expect("insert session");

    SeededUser { user_id, session_id }
}

async fn insert_api_key(pool: &PgPool, org_id: Uuid, plaintext: &str, created_by: Uuid, encryption_key: &str) {
    let encrypted = crypto::encrypt_secret(plaintext, encryption_key).unwrap();
    let last_four = crypto::last_four(plaintext);
    sqlx::query(
        "INSERT INTO organization_api_keys (organisation_id, provider, encrypted_key, last_four, created_by) \
         VALUES ($1, 'openai', $2, $3, $4)",
    )
    .bind(org_id)
    .bind(encrypted)
    .bind(last_four)
    .bind(created_by)
    .execute(pool)
    .await
    .expect("insert organization_api_keys row");
}

async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
    let body = response.into_body();
    axum::body::to_bytes(body, 1_000_000)
        .await
        .expect("read response body")
        .to_vec()
}

/// Splits the wire-protocol body (`ChatStreamMeta` JSON, `\0`, streamed
/// text) into its two parts.
fn split_meta_and_content(bytes: &[u8]) -> (serde_json::Value, String) {
    let nul_pos = bytes.iter().position(|b| *b == 0).expect("body must contain a NUL-separated meta prefix");
    let meta: serde_json::Value = serde_json::from_slice(&bytes[..nul_pos]).expect("meta prefix must be valid JSON");
    let content = String::from_utf8(bytes[nul_pos + 1..].to_vec()).expect("streamed content must be valid UTF-8");
    (meta, content)
}

/// A single OpenAI-shaped SSE stream body — real documented wire format.
fn sse_body(chunks: &[&str]) -> String {
    let mut body = String::new();
    for chunk in chunks {
        body.push_str(&format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\n",
            serde_json::to_string(chunk).unwrap()
        ));
    }
    body.push_str("data: [DONE]\n\n");
    body
}

async fn wait_until_response_flagged_is_set(pool: &PgPool, audit_log_id: Uuid) -> Option<bool> {
    for _ in 0..40 {
        let flagged: Option<bool> = sqlx::query_scalar("SELECT response_flagged FROM audit_log WHERE id = $1")
            .bind(audit_log_id)
            .fetch_one(pool)
            .await
            .expect("query audit_log");
        if flagged.is_some() {
            return flagged;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    None
}

#[sqlx::test]
async fn blocked_prompt_never_calls_openai_and_creates_no_chat_rows(pool: PgPool) {
    let org = insert_org(&pool, "Chat Test Org Blocked").await;
    let user = insert_user(&pool, org, "blocked@chat-test.test", "staff").await;
    let state = AppState { db: pool.clone(), config: test_config("http://127.0.0.1:0/unused") };
    let claims = claims_for(user.user_id, user.session_id, "General", "staff", org);

    // Same low-confidence generic token used in detection::scan's own
    // generic_low_confidence_token_fails_closed test — no specific prefix,
    // fails closed.
    let payload = ChatRequest {
        conversation_id: None,
        message: "token: aZ9xK2mQ7pL4vN8tR3wY6bC1dF5gH0jS2u".to_string(),
    };

    let response = routes::chat::chat(State(state), AuthUser(claims), Json(payload))
        .await
        .expect("blocked chat call should still succeed as a normal response");

    let bytes = body_bytes(response).await;
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).expect("blocked response must be JSON");
    assert_eq!(parsed["blocked"], true);
    assert_eq!(parsed["decision"], "blocked_low_confidence");
    assert!(parsed["conversation_id"].is_null());

    let conversation_count: i64 = sqlx::query_scalar("SELECT count(*) FROM chat_conversations WHERE organisation_id = $1")
        .bind(org)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(conversation_count, 0, "a blocked prompt must never create a conversation");

    let audit_row: (String, String) =
        sqlx::query_as("SELECT decision, ai_model_used FROM audit_log WHERE organisation_id = $1")
            .bind(org)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audit_row.0, "blocked_low_confidence");
    assert!(audit_row.1.contains("blocked pre-gateway"), "ai_model_used must show no provider was called: {}", audit_row.1);
}

#[sqlx::test]
async fn forwarding_without_an_organisation_api_key_is_a_clear_error(pool: PgPool) {
    let org = insert_org(&pool, "Chat Test Org No Key").await;
    let user = insert_user(&pool, org, "nokey@chat-test.test", "staff").await;
    let state = AppState { db: pool.clone(), config: test_config("http://127.0.0.1:0/unused") };
    let claims = claims_for(user.user_id, user.session_id, "General", "staff", org);

    let payload = ChatRequest { conversation_id: None, message: "What is the capital of Zimbabwe?".to_string() };
    let result = routes::chat::chat(State(state), AuthUser(claims), Json(payload)).await;

    let err = result.expect_err("forwarding with no configured key must fail");
    let message = err.to_string();
    assert!(message.contains("OpenAI API key"), "error should explain the real problem: {message}");
}

#[sqlx::test]
async fn a_clean_prompt_is_forwarded_streamed_and_the_response_is_scanned_and_stored(pool: PgPool) {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse_body(&["Harare", " is the capital."])))
        .mount(&mock_server)
        .await;

    let org = insert_org(&pool, "Chat Test Org Clean").await;
    let user = insert_user(&pool, org, "clean@chat-test.test", "staff").await;
    insert_api_key(&pool, org, "sk-test-key-clean-path-000000000000", user.user_id, &"a".repeat(64)).await;

    let base_url = format!("{}/v1/chat/completions", mock_server.uri());
    let state = AppState { db: pool.clone(), config: test_config(&base_url) };
    let claims = claims_for(user.user_id, user.session_id, "General", "staff", org);

    let payload = ChatRequest { conversation_id: None, message: "What is the capital of Zimbabwe?".to_string() };
    let response = routes::chat::chat(State(state.clone()), AuthUser(claims), Json(payload))
        .await
        .expect("forwarded chat call should succeed");

    let bytes = body_bytes(response).await;
    let (meta, content) = split_meta_and_content(&bytes);
    assert_eq!(meta["decision"], "cleared_no_entities");
    assert_eq!(content, "Harare is the capital.");

    let audit_log_id = Uuid::parse_str(meta["audit_log_id"].as_str().unwrap()).unwrap();
    let flagged = wait_until_response_flagged_is_set(&pool, audit_log_id).await;
    assert_eq!(flagged, Some(false), "a clean response must end up not flagged");

    let response_scan_result: String =
        sqlx::query_scalar("SELECT response_scan_result FROM audit_log WHERE id = $1")
            .bind(audit_log_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        response_scan_result.to_lowercase().contains("no sensitive"),
        "response_scan_result should reflect the real scan_response outcome, got: {response_scan_result}"
    );

    let messages: Vec<(String, String)> = sqlx::query_as(
        "SELECT role, redacted_content FROM chat_messages mc \
         JOIN chat_conversations cc ON cc.id = mc.conversation_id \
         WHERE cc.organisation_id = $1 ORDER BY mc.created_at",
    )
    .bind(org)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0], ("user".to_string(), "What is the capital of Zimbabwe?".to_string()));
    assert_eq!(messages[1], ("assistant".to_string(), "Harare is the capital.".to_string()));
}

#[sqlx::test]
async fn a_response_leaking_a_national_id_is_flagged_retroactively_not_blocked(pool: PgPool) {
    let mock_server = MockServer::start().await;
    // A real national_id-shaped string in the ASSISTANT's reply — scan_response
    // never blocks or redacts it (see detection::scan::scan_response's own
    // doc comment), only flags it after the fact.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse_body(&["Sure, the ID on file is 63-123456A23."])))
        .mount(&mock_server)
        .await;

    let org = insert_org(&pool, "Chat Test Org Leak").await;
    let user = insert_user(&pool, org, "leak@chat-test.test", "staff").await;
    insert_api_key(&pool, org, "sk-test-key-leak-path-0000000000000", user.user_id, &"a".repeat(64)).await;

    let base_url = format!("{}/v1/chat/completions", mock_server.uri());
    let state = AppState { db: pool.clone(), config: test_config(&base_url) };
    let claims = claims_for(user.user_id, user.session_id, "General", "staff", org);

    let payload = ChatRequest { conversation_id: None, message: "Do we have that customer's ID on file?".to_string() };
    let response = routes::chat::chat(State(state.clone()), AuthUser(claims), Json(payload))
        .await
        .expect("forwarded chat call should succeed");

    let bytes = body_bytes(response).await;
    let (meta, content) = split_meta_and_content(&bytes);
    // The response streamed to the user is untouched — this codebase never
    // redacts a response, only flags it (see detection/scan.rs).
    assert_eq!(content, "Sure, the ID on file is 63-123456A23.");

    let audit_log_id = Uuid::parse_str(meta["audit_log_id"].as_str().unwrap()).unwrap();
    let flagged = wait_until_response_flagged_is_set(&pool, audit_log_id).await;
    assert_eq!(flagged, Some(true), "a response leaking a national id must end up flagged");

    let assistant_row: (String, Option<bool>) = sqlx::query_as(
        "SELECT redacted_content, response_flagged FROM chat_messages WHERE role = 'assistant' \
         AND audit_log_id = $1",
    )
    .bind(audit_log_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // "redacted_content" for an assistant row is the response verbatim, not
    // actually redacted — see migration 0017's own comment on this.
    assert_eq!(assistant_row.0, "Sure, the ID on file is 63-123456A23.");
    assert_eq!(assistant_row.1, Some(true));
}

#[sqlx::test]
async fn conversation_history_sent_to_the_provider_is_redacted_content_only(pool: PgPool) {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse_body(&["Noted."])))
        .mount(&mock_server)
        .await;

    let org = insert_org(&pool, "Chat Test Org History").await;
    let user = insert_user(&pool, org, "history@chat-test.test", "staff").await;
    insert_api_key(&pool, org, "sk-test-key-history-00000000000000", user.user_id, &"a".repeat(64)).await;

    let base_url = format!("{}/v1/chat/completions", mock_server.uri());
    let state = AppState { db: pool.clone(), config: test_config(&base_url) };

    // Turn 1: a message containing a national id — redacted and forwarded.
    let claims_1 = claims_for(user.user_id, user.session_id, "General", "staff", org);
    let payload_1 = ChatRequest {
        conversation_id: None,
        message: "Please check account for ID 63-123456A23 today.".to_string(),
    };
    let response_1 = routes::chat::chat(State(state.clone()), AuthUser(claims_1), Json(payload_1))
        .await
        .expect("turn 1 should succeed");
    let bytes_1 = body_bytes(response_1).await;
    let (meta_1, _) = split_meta_and_content(&bytes_1);
    assert_eq!(meta_1["decision"], "redacted_and_forwarded");
    let conversation_id = Uuid::parse_str(meta_1["conversation_id"].as_str().unwrap()).unwrap();

    // Give the background task a moment to finish writing turn 1's rows.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Turn 2: continue the same conversation.
    let claims_2 = claims_for(user.user_id, user.session_id, "General", "staff", org);
    let payload_2 = ChatRequest { conversation_id: Some(conversation_id), message: "Thanks.".to_string() };
    let response_2 = routes::chat::chat(State(state), AuthUser(claims_2), Json(payload_2))
        .await
        .expect("turn 2 should succeed");
    let _ = body_bytes(response_2).await;

    // The second call to the provider must have received turn 1's history
    // with the national id already redacted — never the raw digits.
    let requests = mock_server.received_requests().await.expect("mock server tracks requests");
    assert_eq!(requests.len(), 2, "the provider must have been called exactly twice");
    let second_call_body: serde_json::Value = requests[1].body_json().unwrap();
    let full_request_text = second_call_body.to_string();
    assert!(!full_request_text.contains("63-123456A23"), "the raw national id must never reach the provider");
    assert!(full_request_text.contains("REDACTED"), "the redacted placeholder must be what the provider sees");
}

#[sqlx::test]
async fn a_conversation_cannot_be_listed_or_read_by_another_organisations_user(pool: PgPool) {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse_body(&["Hi there."])))
        .mount(&mock_server)
        .await;

    let org_a = insert_org(&pool, "Chat Isolation Handler Org A").await;
    let org_b = insert_org(&pool, "Chat Isolation Handler Org B").await;
    let user_a = insert_user(&pool, org_a, "a@chat-handler-test.test", "staff").await;
    let user_b = insert_user(&pool, org_b, "b@chat-handler-test.test", "staff").await;
    insert_api_key(&pool, org_a, "sk-test-key-org-a-000000000000000000", user_a.user_id, &"a".repeat(64)).await;

    let base_url = format!("{}/v1/chat/completions", mock_server.uri());
    let state = AppState { db: pool.clone(), config: test_config(&base_url) };

    let claims_a = claims_for(user_a.user_id, user_a.session_id, "General", "staff", org_a);
    let payload = ChatRequest { conversation_id: None, message: "Hello there.".to_string() };
    let response = routes::chat::chat(State(state.clone()), AuthUser(claims_a), Json(payload))
        .await
        .expect("org A's chat call should succeed");
    let bytes = body_bytes(response).await;
    let (meta, _) = split_meta_and_content(&bytes);
    let conversation_id = Uuid::parse_str(meta["conversation_id"].as_str().unwrap()).unwrap();

    // Org A can list its own conversation.
    let claims_a_again = claims_for(user_a.user_id, user_a.session_id, "General", "staff", org_a);
    let own_list = routes::chat::list_conversations(State(state.clone()), AuthUser(claims_a_again))
        .await
        .expect("org A must be able to list its own conversations");
    assert_eq!(own_list.0.conversations.len(), 1);

    // Org B's user cannot list org A's conversation (their own list is empty)...
    let claims_b = claims_for(user_b.user_id, user_b.session_id, "General", "staff", org_b);
    let other_list = routes::chat::list_conversations(State(state.clone()), AuthUser(claims_b))
        .await
        .expect("org B's own list call should succeed");
    assert_eq!(other_list.0.conversations.len(), 0);

    // ...and cannot fetch org A's conversation's messages directly by id, even
    // knowing its exact UUID.
    let claims_b_again = claims_for(user_b.user_id, user_b.session_id, "General", "staff", org_b);
    let result = routes::chat::list_messages(State(state), AuthUser(claims_b_again), Path(conversation_id)).await;
    assert!(result.is_err(), "org B must never be able to read org A's chat messages");
}
