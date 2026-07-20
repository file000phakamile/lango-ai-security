//! The only implemented `ChatProvider` (Phase 2). Calls OpenAI's real
//! `POST /v1/chat/completions` endpoint with `stream: true` and parses its
//! Server-Sent-Events response into plain text deltas.
//!
//! **Verification status, stated plainly**: this has been tested against a
//! mocked OpenAI response (see the tests below, which drive the SSE-parsing
//! logic against a fixture matching OpenAI's real documented wire format)
//! but has NOT been exercised against the real, live OpenAI API in this
//! session — no live OpenAI API key was available in this environment. See
//! Questions.md for the full verification honesty statement this task
//! required.
use axum::async_trait;
use futures_util::StreamExt;
use tokio::sync::mpsc;

use super::{ChatProvider, ChatTurn};
use crate::error::{AppError, AppResult};

/// A small, inexpensive, widely-available chat model — reasonable default
/// for a v1 that has no per-organisation model selection UI yet (Phase 3
/// only exposes key provisioning/rotation, not model choice).
pub const OPENAI_MODEL: &str = "gpt-4o-mini";

/// The exact string written to `audit_log.ai_model_used` for a chat turn
/// that actually reached OpenAI — shared so `routes::chat` and this file
/// can't drift on what "the model that handled this" means.
pub fn ai_model_used_label() -> String {
    format!("openai:{OPENAI_MODEL}")
}

pub struct OpenAiProvider {
    client: reqwest::Client,
    /// Defaults to OpenAI's real endpoint (`Config::openai_api_base_url`) —
    /// overridable so tests can point this at a local mock HTTP server
    /// instead of making a real network call (see tests/chat.rs).
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(base_url: &str) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.to_string() }
    }
}

/// Parses one SSE "frame" (everything between two `\n\n` delimiters) and
/// pushes any text delta it contains onto `deltas`. Returns `true` if this
/// frame was OpenAI's terminal `data: [DONE]` marker. Split out from
/// `stream_chat` so it can be unit-tested directly against fixture data
/// without a real HTTP call.
fn parse_sse_frame(frame: &str, deltas: &mut Vec<String>) -> bool {
    for line in frame.lines() {
        let Some(data) = line.strip_prefix("data: ") else { continue };
        if data == "[DONE]" {
            return true;
        }
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) else { continue };
        if let Some(delta) = parsed["choices"][0]["delta"]["content"].as_str() {
            deltas.push(delta.to_string());
        }
    }
    false
}

#[async_trait]
impl ChatProvider for OpenAiProvider {
    async fn stream_chat(
        &self,
        api_key: &str,
        history: &[ChatTurn],
    ) -> AppResult<mpsc::Receiver<AppResult<String>>> {
        let messages: Vec<serde_json::Value> = history
            .iter()
            .map(|t| serde_json::json!({"role": t.role, "content": t.content}))
            .collect();

        let response = self
            .client
            .post(&self.base_url)
            .bearer_auth(api_key)
            .json(&serde_json::json!({
                "model": OPENAI_MODEL,
                "messages": messages,
                "stream": true,
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("OpenAI request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            // Never include `api_key` or `messages` (redacted prompt/history
            // content) in this log line — status + response body only.
            tracing::error!(status = %status, body = %body, "OpenAI chat completion request failed");
            return Err(AppError::Internal(format!(
                "The OpenAI API returned an error (status {status}). Please try again shortly."
            )));
        }

        let (tx, rx) = mpsc::channel::<AppResult<String>>(32);
        tokio::spawn(async move {
            let mut byte_stream = response.bytes_stream();
            let mut buffer = String::new();
            while let Some(chunk) = byte_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx
                            .send(Err(AppError::Internal(format!("OpenAI stream error: {e}"))))
                            .await;
                        return;
                    }
                };
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(pos) = buffer.find("\n\n") {
                    let frame: String = buffer.drain(..pos + 2).collect();
                    let mut deltas = Vec::new();
                    let done = parse_sse_frame(&frame, &mut deltas);
                    for delta in deltas {
                        if tx.send(Ok(delta)).await.is_err() {
                            return; // receiver dropped — client disconnected
                        }
                    }
                    if done {
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_realistic_openai_sse_stream_into_ordered_text_deltas() {
        // Fixture shape matches OpenAI's real, documented streaming format
        // for POST /v1/chat/completions with stream: true.
        let frames = [
            r#"data: {"id":"1","choices":[{"delta":{"role":"assistant"},"index":0}]}"#,
            r#"data: {"id":"1","choices":[{"delta":{"content":"Hello"},"index":0}]}"#,
            r#"data: {"id":"1","choices":[{"delta":{"content":", world"},"index":0}]}"#,
            r#"data: {"id":"1","choices":[{"delta":{},"index":0,"finish_reason":"stop"}]}"#,
            "data: [DONE]",
        ];

        let mut all_deltas = Vec::new();
        let mut saw_done = false;
        for frame in frames {
            let mut deltas = Vec::new();
            let done = parse_sse_frame(frame, &mut deltas);
            all_deltas.extend(deltas);
            if done {
                saw_done = true;
            }
        }

        assert_eq!(all_deltas, vec!["Hello".to_string(), ", world".to_string()]);
        assert!(saw_done);
    }

    #[test]
    fn a_malformed_frame_is_skipped_rather_than_panicking() {
        let mut deltas = Vec::new();
        let done = parse_sse_frame("data: not valid json at all", &mut deltas);
        assert!(deltas.is_empty());
        assert!(!done);
    }

    #[test]
    fn a_frame_with_no_data_line_produces_nothing() {
        let mut deltas = Vec::new();
        let done = parse_sse_frame(": this is a comment/keepalive line\n", &mut deltas);
        assert!(deltas.is_empty());
        assert!(!done);
    }

    #[test]
    fn ai_model_used_label_names_the_real_provider_and_model() {
        assert_eq!(ai_model_used_label(), "openai:gpt-4o-mini");
    }
}
