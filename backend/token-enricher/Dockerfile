# ArbitrageX v2 — token-enricher
# Multi-stage build. Pinned Rust version. Non-root runtime user.
# Build context must be the `backend/` directory so the workspace is visible.
#
# Defect #3 fix: COPY entire backend/ so all 9 workspace members are present.
# Cargo workspace metadata resolution requires every member's Cargo.toml to be
# readable during `cargo build -p token-enricher`, even for members that are
# not direct dependencies.
#
# Defect #11: using rust:1.82-bookworm (pinned, publicly available, covers
# MSRV 1.75 declared in backend/Cargo.toml).

FROM rust:1.82-bookworm AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the entire backend workspace so all members are available.
COPY . ./

RUN cargo build --release -p token-enricher

# ---------------------------------------------------------------------------
# Runtime image
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx -u 65532 arbx

WORKDIR /app

COPY --from=builder /build/target/release/token-enricher /usr/local/bin/token-enricher

# Security: run as unprivileged user (uid 65532, matches distroless nonroot convention).
USER arbx

# Default port for Prometheus scraping.
EXPOSE 9004

# tini as PID 1: handles SIGTERM/SIGCHLD correctly so our Rust process
# receives SIGTERM cleanly and runs its graceful-shutdown path.
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/token-enricher"]
