use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_signing_secret: String,
    pub port: u16,
    pub cors_origin: String,
    /// 64-hex-character (32-byte) AES-256-GCM key used to encrypt/decrypt
    /// organisation API keys at rest (native chat feature, Phase 1). See
    /// crypto.rs. Loaded the same way as jwt_signing_secret — required, no
    /// silent fallback, since a missing key here would otherwise fail much
    /// later and less clearly (at the first attempt to save or use an
    /// organisation's OpenAI key).
    pub api_key_encryption_key: String,
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
        let api_key_encryption_key = env::var("API_KEY_ENCRYPTION_KEY")
            .expect("API_KEY_ENCRYPTION_KEY must be set (see backend/.env.example) — 64 hex chars, e.g. `openssl rand -hex 32`");

        Self {
            database_url,
            jwt_signing_secret,
            port,
            cors_origin,
            api_key_encryption_key,
        }
    }
}
