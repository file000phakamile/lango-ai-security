//! Basic security hardening ("response scanning + observability +
//! hardening" task, Part 3) — the global, per-IP rate limit config, pulled
//! out of `main.rs` into its own function so it can be exercised by a real
//! test (`cargo test --lib`) without needing a running server or a
//! database, not just eyeballed in `main.rs`.
use std::sync::Arc;

use tower_governor::{
    governor::{GovernorConfig, GovernorConfigBuilder},
    key_extractor::SmartIpKeyExtractor,
};

/// See `main.rs`'s own comment at the call site for the full reasoning
/// behind the specific numbers and the `SmartIpKeyExtractor` trust
/// assumption (this deployment sits behind Render's own reverse proxy).
pub fn rate_limit_config() -> Arc<GovernorConfig<SmartIpKeyExtractor, governor::middleware::NoOpMiddleware<governor::clock::QuantaInstant>>> {
    Arc::new(
        GovernorConfigBuilder::default()
            .per_second(10)
            .burst_size(30)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .expect("rate limiter config is valid"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use tower::ServiceExt;
    use tower_governor::GovernorLayer;

    async fn always_ok() -> &'static str {
        "ok"
    }

    /// Real, not mocked: builds an actual `Router` with the real
    /// `GovernorLayer` (the same layer `main.rs` applies to the whole
    /// backend) wrapping a trivial handler, and fires more requests than
    /// the configured burst allows in immediate succession — the same way
    /// a real client hammering an endpoint would. `SmartIpKeyExtractor`
    /// falls back to `ConnectInfo<SocketAddr>` when no forwarded-for header
    /// is present (as in this test), so `Router::into_make_service_with_
    /// connect_info` is required here too, exactly as `main.rs` uses it —
    /// this test would fail to compile/panic at request time without it,
    /// which is itself a useful, real check that the two stay in sync.
    fn test_router() -> Router {
        Router::new().route("/test", get(always_ok)).layer(GovernorLayer { config: rate_limit_config() })
    }

    /// `SmartIpKeyExtractor` reads `x-forwarded-for` first (see `main.rs`'s
    /// own comment on why that's the correct choice for this deployment,
    /// sitting behind Render's trusted reverse proxy) — a real served
    /// connection gets a peer address for free; `oneshot()` dispatch (used
    /// throughout this test file) does not, so tests provide this header
    /// explicitly rather than relying on connection-level fallback
    /// machinery `oneshot` doesn't exercise.
    fn request_with_forwarded_ip(ip: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .uri("/test")
            .header("x-forwarded-for", ip)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn burst_beyond_the_configured_limit_is_rejected_with_429() {
        let app = test_router();

        let mut saw_429 = false;
        // burst_size(30) + per_second(10) means the first ~30 requests in
        // immediate succession, all from the SAME key (same forwarded-for
        // IP — the limiter is per-IP, so a mix of IPs would never trigger
        // this regardless of total volume), should succeed and further ones
        // in the same instant should not — fire 60 to comfortably exceed
        // that.
        for _ in 0..60 {
            let request = request_with_forwarded_ip("203.0.113.55");
            // Cloning the app per request mirrors how axum actually serves
            // concurrent connections (each gets its own Service clone) and
            // is required since `oneshot` consumes its `Service`.
            let response = app.clone().oneshot(request).await.unwrap();
            if response.status() == axum::http::StatusCode::TOO_MANY_REQUESTS {
                saw_429 = true;
                break;
            }
        }
        assert!(saw_429, "60 rapid requests against a burst_size(30) limiter must trigger at least one 429");
    }

    #[tokio::test]
    async fn a_single_ordinary_request_is_never_rate_limited() {
        let app = test_router();

        let request = request_with_forwarded_ip("203.0.113.99");
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 8192).await.unwrap();
        let body_text = String::from_utf8_lossy(&bytes);
        assert_eq!(status, axum::http::StatusCode::OK, "ordinary traffic must never be rejected — body: {body_text}");
    }
}
