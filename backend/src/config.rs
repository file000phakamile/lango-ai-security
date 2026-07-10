use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_signing_secret: String,
    pub port: u16,
    pub cors_origin: String,
}

impl Config {
    pub fn from_env() -> Self {
        // Loads backend/.env if present; falls back to real process env
        // (e.g. in a hosted deployment). Never panics on a missing .env file.
        let _ = dotenvy::dotenv();

        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set (see backend/.env.example)");
        let jwt_signing_secret = env::var("JWT_SIGNING_SECRET")
            .expect("JWT_SIGNING_SECRET must be set (see backend/.env.example)");
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        let cors_origin =
            env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());

        Self {
            database_url,
            jwt_signing_secret,
            port,
            cors_origin,
        }
    }
}
