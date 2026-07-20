//! Provider-adapter interface for the native chat feature (Phase 2). Only
//! `OpenAiProvider` (see `openai.rs`) is implemented and tested — this trait
//! exists so a second provider can be added later without restructuring
//! `routes::chat`, not because a second provider is being built now.
//!
//! Deliberately provider-agnostic: a decrypted API key and a redacted
//! conversation history go in, a stream of text deltas comes out. No
//! OpenAI-specific type (model names, OpenAI's own request/response JSON
//! shape) appears in this trait — those live entirely inside `openai.rs`.
pub mod openai;

use axum::async_trait;
use tokio::sync::mpsc;

use crate::error::AppResult;

/// One turn of conversation history sent to a provider. `role` is
/// `"user"` or `"assistant"` — a de facto cross-provider convention, not an
/// OpenAI-specific concept.
#[derive(Clone, Debug)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

/// `stream_chat` returns a `Receiver` (rather than a boxed `Stream`) so this
/// trait stays object-safe and dyn-compatible without pulling in an
/// associated-type/GAT dance — a `Receiver<AppResult<String>>` is already a
/// concrete, `Send + 'static` type regardless of which provider produced it.
/// The axum handler wraps it in `tokio_stream::wrappers::ReceiverStream`
/// itself (see routes/chat.rs).
#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn stream_chat(
        &self,
        api_key: &str,
        history: &[ChatTurn],
    ) -> AppResult<mpsc::Receiver<AppResult<String>>>;
}
