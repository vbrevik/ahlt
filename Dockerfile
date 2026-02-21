# ── Stage 1: Build ────────────────────────────────────────────────────
FROM rust:1.88-bookworm AS builder

WORKDIR /app

# Cache dependencies by building a dummy project first
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Copy source and build for real
COPY src/ src/
COPY templates/ templates/
COPY static/ static/
COPY migrations/ migrations/
COPY data/seed/ data/seed/
RUN touch src/main.rs && cargo build --release

# ── Stage 2: Runtime ─────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/ahlt /app/ahlt
COPY templates/ templates/
COPY static/ static/
COPY migrations/ migrations/
COPY data/seed/ data/seed/

# Default environment — DATABASE_URL must be provided at runtime
ENV APP_ENV=dev \
    HOST=0.0.0.0 \
    PORT=8080 \
    COOKIE_SECURE=false \
    RUST_LOG=info

EXPOSE 8080

CMD ["./ahlt"]
