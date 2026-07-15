use axum::{
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use axum::http::{HeaderValue, Method};
use tower_governor::GovernorLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use lango_backend::{
    config::Config, observability::error_log_middleware, rate_limit::rate_limit_config, routes, state::AppState,
};

#[tokio::main]
async fn main() {
    // Real observability ("response scanning + observability + hardening"
    // task, Part 2): `LOG_FORMAT=json` switches to machine-parseable JSON
    // log lines (what a hosted deployment's log aggregator — Render's own
    // log viewer, or anything downstream of it — can actually index and
    // query), while the default stays the existing human-readable format
    // for local `cargo run` development. Both paths use the exact same
    // `tracing` events; only the output encoding changes, so nothing about
    // application code needs to know or care which is active.
    let json_logs = std::env::var("LOG_FORMAT").map(|v| v == "json").unwrap_or(false);
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "lango_backend=info,tower_http=info".into());
    if json_logs {
        tracing_subscriber::fmt().json().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    let config = Config::from_env();

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to Postgres — is it running? see docker-compose.yml");

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("failed to run database migrations");

    let port = config.port;
    let state = AppState { db, config: config.clone() };

    // v0.1 assumes a single known frontend origin (the Next.js dev server, or
    // whatever CORS_ORIGIN is set to) rather than a wildcard — a small, real
    // security improvement over `Any` that costs nothing in a local setup.
    let cors_origin: HeaderValue = config
        .cors_origin
        .parse()
        .expect("CORS_ORIGIN must be a valid origin, e.g. http://localhost:3000");
    let cors = CorsLayer::new()
        .allow_origin(cors_origin)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([axum::http::header::AUTHORIZATION, axum::http::header::CONTENT_TYPE])
        // Compliance export (product-depth task, Part 2) responds with a
        // file download and a Content-Disposition header carrying its
        // filename — browsers hide response headers from JS by default
        // unless the server explicitly opts them into CORS exposure, so
        // without this the frontend's download helper could still get the
        // file bytes but never the real filename.
        .expose_headers([axum::http::header::CONTENT_DISPOSITION]);

    // Basic security hardening (product-depth task, Part 3): a single
    // global, per-IP rate limit covering every route in this router,
    // applied as the outermost layer so it rejects abusive traffic before
    // any other work happens — before CORS, before the observability
    // middleware, before any handler or database query. Real gap this
    // closes: before this task, there was NO rate limiting anywhere in
    // this backend (docs/ARCHITECTURE.md and docs/SECURITY_PRIVACY.md both
    // stated this honestly as a target-only item) — /api/auth/login and
    // /api/scan in particular had no protection against a brute-force or
    // abuse loop.
    //
    // 10 req/s sustained with a burst of 30: generous enough for normal
    // dashboard usage (the dashboard fires several parallel GETs on load —
    // see lib/lango/api-client.ts's Promise.all), tight enough to meaningfully
    // slow down a credential-stuffing loop against /api/auth/login or a
    // scan-spam loop against /api/scan. One single global limit, not a
    // per-route policy, deliberately — simplicity was chosen over tuning
    // each endpoint individually for this pass; see Questions.md for the
    // full reasoning and what a more mature setup would look like.
    //
    // SmartIpKeyExtractor reads the real client IP from X-Forwarded-For /
    // X-Real-IP / Forwarded headers (falling back to the raw peer address),
    // which is the correct choice ONLY because this backend is deployed
    // behind Render's own reverse proxy (a trusted provider that sets these
    // headers correctly) — see docs/SECURITY_PRIVACY.md. Using this
    // extractor behind an UNTRUSTED proxy (or with none at all, exposed
    // directly to the internet) would let a client trivially bypass the
    // limit by spoofing the header; that is not this deployment's actual
    // topology, but it's worth stating the assumption explicitly rather
    // than leaving it implicit.
    let governor_conf = rate_limit_config();

    let app = Router::new()
        .route("/api/auth/login", post(routes::auth::login))
        .route("/api/organisations/signup", post(routes::organisations::signup))
        .route("/api/consent/accept", post(routes::consent::accept_consent))
        .route("/api/scan", post(routes::scan::scan))
        // Response scanning (product-depth task, Part 1) — the second half
        // of the pipeline; see routes/response_scan.rs.
        .route("/api/scan/response", post(routes::response_scan::scan_response_handler))
        .route("/api/audit-log", get(routes::audit_log::get_audit_log))
        .route("/api/fairness", get(routes::fairness::get_fairness))
        .route("/api/drift", get(routes::drift::get_drift))
        .route(
            "/api/security-events",
            get(routes::security_events::get_security_events),
        )
        .route(
            "/api/command-center/summary",
            get(routes::command_center::get_summary),
        )
        // Health module (see docs/HEALTH_MODULE.md) — deliberately NOT
        // "/api/health" or "/health/summary", to avoid any confusion with
        // the unauthenticated infra healthcheck route below, which is a
        // completely unrelated concept that predates this module.
        .route(
            "/api/health-data-guard/summary",
            get(routes::health::get_health_summary),
        )
        // Policy builder (product-depth task, Part 1) — compliance_admin
        // only, enforced inside each handler, not just by routing.
        .route(
            "/api/policy/settings",
            get(routes::policy::get_settings).put(routes::policy::update_threshold),
        )
        .route(
            "/api/policy/custom-patterns",
            post(routes::policy::create_custom_pattern),
        )
        .route(
            "/api/policy/custom-patterns/:id",
            delete(routes::policy::delete_custom_pattern),
        )
        // Compliance export (product-depth task, Part 2) — compliance_admin
        // only, enforced inside the handler.
        .route(
            "/api/compliance-export",
            get(routes::compliance_export::export),
        )
        // Active learning loop (product-depth task, Part 3) — a human
        // confirm/overturn judgment on a flagged low-confidence row
        // (compliance_admin or department_reviewer, department-scoped for
        // the latter), and the compliance_admin-only export of everything
        // recorded so far.
        .route(
            "/api/audit-log/:id/review-decision",
            post(routes::review_decisions::record_review_decision),
        )
        .route(
            "/api/labelled-dataset",
            get(routes::labelled_dataset::export),
        )
        // Real observability (product-depth task, Part 2) —
        // compliance_admin only; see routes/backend_errors.rs's own
        // comment for the stated v1 scope limitation (not organisation-
        // scoped).
        .route(
            "/api/backend-errors",
            get(routes::backend_errors::get_backend_errors),
        )
        // No auth required — this is what render.yaml's healthCheckPath
        // (and any external uptime check) hits.
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .with_state(state.clone())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        // Sees the final response exactly as the client will receive it
        // (after CORS headers, after routing), which is what its
        // status-code check needs. See src/observability.rs.
        .layer(axum::middleware::from_fn_with_state(state, error_log_middleware))
        // Truly outermost — rejects a rate-limited request before it
        // reaches anything else in this stack at all.
        .layer(GovernorLayer { config: governor_conf });

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("failed to bind port");

    tracing::info!("lango-backend listening on http://0.0.0.0:{}", port);

    // `into_make_service_with_connect_info::<SocketAddr>()`, not the plain
    // `into_make_service()` every other route in this codebase's history
    // used — the governor rate limiter's IP-based key extraction needs the
    // real peer address available as connection info, even when (as here)
    // the primary extraction path reads a forwarded-for header instead.
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await
        .expect("server error");
}
