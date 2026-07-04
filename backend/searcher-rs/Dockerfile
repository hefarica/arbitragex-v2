# ArbitrageX v2 — searcher-rs
# Multi-stage build. Pinned Rust version. Non-root runtime user.
# Build context must be the `backend/` directory so the shared-rs crate is visible.

FROM rust:1.91-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml ./Cargo.toml
# Cargo.lock is REQUIRED when the build below uses --locked --frozen (offline
# mode). Without it, cargo errors with "no matching package named X found" on
# every transitive dep. Regression from commit 3277135 which added --frozen
# without copying the lockfile.
COPY Cargo.lock ./Cargo.lock
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
COPY sim-core ./sim-core
# token-enricher: workspace member added later. Not a build target for
# searcher-rs but cargo's workspace resolver requires every declared member
# to have a readable manifest, otherwise `cargo build -p searcher-rs` errors
# with "failed to load manifest for workspace member /build/token-enricher".
COPY token-enricher ./token-enricher
COPY sed-core ./sed-core
# mcp-sim-engine: workspace member (9a45a96). Never a build target here, but the
# workspace resolver requires its manifest to be present, else `cargo build -p
# searcher-rs` fails "failed to load manifest for workspace member
# /build/mcp-sim-engine". Same rationale as token-enricher/simulator-v2 above.
COPY mcp-sim-engine ./mcp-sim-engine
# --locked enforces reproducibility (Cargo.lock must match Cargo.toml).
# --frozen would also enable --offline, which requires a pre-populated
# ~/.cargo/registry index — not available in this fresh build context.
# token-enricher Dockerfile dropped --frozen for the same reason in 8d9b5f3.
RUN cargo build --release --locked -p searcher-rs

FROM debian:bookworm-slim AS runtime
# wget added for compose healthcheck (M2 Capa1 Fase 2 — 2026-05-15).
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tini wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
WORKDIR /app
COPY --from=builder /build/target/release/searcher-rs /usr/local/bin/searcher-rs
# FASE OMEGA fix: ship the master cartridges so the filesystem auto-loader
# (cartridge_loader::load_cartridges_from_dir, CWD-relative "cartridges") finds
# them on a cold boot. Without this the dir is absent and boot-load returns 0
# (cartridge_loader.dir_not_found); hot-reload via Redis still works regardless.
COPY --from=builder /build/searcher-rs/cartridges /app/cartridges
# Hardening (Phase 1 route-discovery): ship the strategy applicability config so
# the engine loads the declared matrix instead of only the embedded safe
# defaults. If absent/invalid the engine still falls back to safe defaults
# (route_discovery.config_default). Read-only at runtime.
COPY --from=builder /build/searcher-rs/config /app/config
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
# Absolute path so the route-discovery worker is independent of CWD. The worker
# honors ARBX_ROUTE_APPLICABILITY_CONFIG over its relative default.
ENV ARBX_ROUTE_APPLICABILITY_CONFIG=/app/config/strategies/route_applicability.yaml
EXPOSE 9001
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/searcher-rs"]
