# Multi-stage Rust build for Zitadel identity platform.
# Stage 1: Build web assets
FROM node:24-slim AS web
WORKDIR /app
COPY package*.json ./
COPY web/package.json web/
COPY packages/ packages/
RUN npm ci --prefer-offline
COPY web/ web/
RUN npm run build -w web

# Stage 2: Build Rust binary
FROM rust:1.94-bookworm AS builder
WORKDIR /app
# Cache dependencies by building a dummy first.
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
# Copy migration SQL files (referenced by include_str!).
COPY migrations/ migrations/
# Copy web assets for rust-embed.
COPY --from=web /app/web/dist/ web/dist/
RUN cargo build --locked --release --bin zitadel

# Stage 3: Minimal runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/zitadel /usr/local/bin/zitadel
COPY fixtures/prod-seed.yaml /etc/zitadel/seed.yaml
EXPOSE 8080
ENTRYPOINT ["zitadel"]
CMD ["start"]
