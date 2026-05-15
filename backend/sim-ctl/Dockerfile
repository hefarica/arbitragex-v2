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
COPY token-enricher ./token-enricher
RUN cargo build --release -p sim-ctl

FROM debian:bookworm-slim AS runtime
# wget added for compose healthcheck (M2 Capa1 Fase 2 — 2026-05-15).
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
WORKDIR /app
COPY --from=builder /build/target/release/sim-ctl /usr/local/bin/sim-ctl
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
EXPOSE 3003
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/sim-ctl"]
