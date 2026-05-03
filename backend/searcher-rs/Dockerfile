# ArbitrageX v2 — searcher-rs
# Multi-stage build. Pinned Rust version. Non-root runtime user.
# Build context must be the `backend/` directory so the shared-rs crate is visible.

FROM rust:1.90-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml ./Cargo.toml
COPY shared-rs ./shared-rs
COPY searcher-rs ./searcher-rs
# sim-ctl/relays-client/recon also live in the workspace; add stub members or copy them too.
COPY sim-ctl ./sim-ctl
COPY relays-client ./relays-client
COPY recon ./recon
COPY math-engine ./math-engine
COPY prioritization-spine ./prioritization-spine
# simulator-v2: workspace member added in Sprint 4. Optional dep of searcher-rs
# (feature `v2-simulator`); metadata resolution requires the path to exist.
COPY simulator-v2 ./simulator-v2
RUN cargo build --release -p searcher-rs

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
WORKDIR /app
COPY --from=builder /build/target/release/searcher-rs /usr/local/bin/searcher-rs
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
EXPOSE 9001
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/searcher-rs"]
