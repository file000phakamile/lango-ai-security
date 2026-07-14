use axum::{
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use axum::http::{HeaderValue, Method};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use lango_backend::{config::Config, routes, state::AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lango_backend=info,tower_http=info".into()),
        )
        .init();

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

    let app = Router::new()
        .route("/api/auth/login", post(routes::auth::login))
        .route("/api/organisations/signup", post(routes::organisations::signup))
        .route("/api/consent/accept", post(routes::consent::accept_consent))
        .route("/api/scan", post(routes::scan::scan))
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
        // No auth required — this is what render.yaml's healthCheckPath
        // (and any external uptime check) hits.
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("failed to bind port");

    tracing::info!("lango-backend listening on http://0.0.0.0:{}", port);

    axum::serve(listener, app)
        .await
        .expect("server error");
}
