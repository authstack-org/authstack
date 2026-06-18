# ── Stage 1: admin CSS ────────────────────────────────────────────────────────
FROM node:22-slim AS assets

WORKDIR /app

COPY package.json ./
COPY static/css/admin.input.css ./static/css/

RUN npm install && npm run build:css

# ── Stage 2: build ────────────────────────────────────────────────────────────
FROM rust:1-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates
COPY static ./static
COPY --from=assets /app/static/css/admin.css ./static/css/admin.css

RUN cargo build --release

# ── Stage 3: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/authstack /app/authstack
COPY --from=builder /app/static /app/static
COPY docker-entrypoint.sh /app/docker-entrypoint.sh
RUN chmod +x /app/docker-entrypoint.sh

EXPOSE 8080

ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["serve"]
