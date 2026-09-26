# syntax=docker/dockerfile:1

# ---- build stage ----
# Pinned to bookworm to match the runtime stage's glibc (see user-service note).
FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Toolchain for native deps: openssl, and cmake/gcc + libcurl for librdkafka.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential cmake pkg-config libssl-dev zlib1g-dev libsasl2-dev libcurl4-openssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .

# Cache mounts keep the cargo registry and target dir across builds, so an
# interrupted build resumes cheaply (librdkafka is a long compile).
RUN --mount=type=cache,target=/app/target,id=messenger-service-target-bookworm \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release -p messenger-service \
    && cp target/release/messenger-service /usr/local/bin/messenger-service

# ---- runtime stage ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 libsasl2-2 zlib1g libcurl4 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --no-create-home --uid 10001 appuser

COPY --from=builder /usr/local/bin/messenger-service /usr/local/bin/messenger-service

USER appuser
EXPOSE 3031
ENTRYPOINT ["/usr/local/bin/messenger-service"]