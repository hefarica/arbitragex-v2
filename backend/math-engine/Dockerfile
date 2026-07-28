# Multi-stage build. Pinned Rust version. Non-root runtime user.
# Build context must be the `backend/` directory so workspace members resolve.
FROM rust:1.91-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
# Workspace members required by cargo's resolver (manifest readability).
COPY shared-rs ./shared-rs
COPY searcher-rs ./searcher-rs
COPY sim-ctl ./sim-ctl
COPY relays-client ./relays-client
COPY recon ./recon
COPY math-engine ./math-engine
COPY prioritization-spine ./prioritization-spine
COPY simulator-v2 ./simulator-v2
COPY sim-core ./sim-core
COPY token-enricher ./token-enricher
COPY mcp-sim-engine ./mcp-sim-engine
COPY sed-core ./sed-core
# Build ONLY the math-engine binary (feature `api` exposes the axum router).
# --locked (not --frozen) so the registry index may refresh inside the build.
RUN cargo build --release --locked -p math-engine --features api --bin math-engine

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tini wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
WORKDIR /app
COPY --from=builder /build/target/release/math-engine /usr/local/bin/math-engine
USER arbx
ENV MATH_ENGINE_PORT=3006
ENV RUST_LOG=info
EXPOSE 3006
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/math-engine"]
