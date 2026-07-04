FROM rust:1.91-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
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
COPY sed-core ./sed-core
# mcp-sim-engine: workspace member (9a45a96); manifest required by the resolver
# even though it is never built here, else "failed to load manifest" aborts.
COPY mcp-sim-engine ./mcp-sim-engine
RUN cargo build --release --locked -p relays-client

FROM debian:bookworm-slim AS runtime
# wget added for compose healthcheck (M2 Capa1 Fase 2 — 2026-05-15).
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
WORKDIR /app
COPY --from=builder /build/target/release/relays-client /usr/local/bin/relays-client
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
EXPOSE 3005
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/relays-client"]
