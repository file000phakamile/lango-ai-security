# syntax=docker/dockerfile:1
#
# Multi-stage build for the Lango backend (Rust/Axum). Build context is
# `backend/` (see render.yaml's dockerContext) — all COPY paths below are
# relative to this directory, not the repo root.
#
# Migrations are NOT copied into the runtime image: sqlx::migrate!("./migrations")
# in src/main.rs is a compile-time macro that embeds the migration SQL directly
# into the binary, so nothing on disk is needed for them to run at startup.

# ---------------------------------------------------------------------------
# Build stage
# ---------------------------------------------------------------------------
FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Build dependencies first, in their own layer, so editing application code
# doesn't force every dependency to recompile on the next build.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin \
    && echo "fn main() {}" > src/main.rs \
    && echo "fn main() {}" > src/bin/seed.rs \
    && touch src/lib.rs \
    && cargo build --release --bin lango-backend \
    && rm -rf src

# Now bring in the real source and rebuild — only this layer re-runs when
# application code changes; the dependency artifacts above stay cached.
COPY . .
RUN touch src/main.rs src/lib.rs src/bin/seed.rs \
    && cargo build --release --bin lango-backend

# ---------------------------------------------------------------------------
# Runtime stage — slim, no Rust toolchain, no source
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# ca-certificates: required for TLS to the managed Postgres instance
# (sqlx is built with the "runtime-tokio-rustls" feature — no OpenSSL
# system package needed, just trusted root certs).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/lango-backend ./lango-backend

# Render injects PORT at runtime (see backend/src/config.rs, which reads it
# with a fallback) and performs its own container health checks against
# healthCheckPath in render.yaml — EXPOSE here is documentation, not
# functional in Render's environment.
EXPOSE 8080

CMD ["./lango-backend"]
