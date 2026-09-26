# syntax=docker/dockerfile:1

# ---- build stage ----
# Pinned to bookworm to match the runtime stage's glibc (2.36); unpinned
# `rust:1-slim` is trixie-based and produces binaries that fail on bookworm.
FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Toolchain for native deps: cmake/gcc for librdkafka, pkg-config/libssl for sqlx/openssl.
# librdkafka's cmake build enables curl support, so libcurl dev headers are required.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential cmake pkg-config libssl-dev zlib1g-dev libsasl2-dev libcurl4-openssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the whole workspace; .dockerignore trims build artifacts.
COPY . .

# Cache mounts keep the cargo registry and target dir across builds, so an
# interrupted build resumes cheaply.
RUN --mount=type=cache,target=/app/target,id=user-service-target-bookworm \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release -p user-service \
    && cp target/release/user-service /usr/local/bin/user-service

# ---- runtime stage ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 libsasl2-2 zlib1g libcurl4 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --no-create-home --uid 10001 appuser

COPY --from=builder /usr/local/bin/user-service /usr/local/bin/user-service

USER appuser
EXPOSE 3030
ENTRYPOINT ["/usr/local/bin/user-service"]