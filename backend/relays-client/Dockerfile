FROM rust:1.91-slim-bookworm AS builder
# PERF-STACK WO-8 (2026-09-20): EPYC-Rome soporta x86-64-v3 (avx2+bmi2+fma
# verificado en VPS). Release SOLO se buildea en VPS (RULE 01).
ENV RUSTFLAGS="-C target-cpu=x86-64-v3"
WORKDIR /build
RUN sed -i "s|http://deb.debian.org|https://deb.debian.org|g" /etc/apt/sources.list.d/debian.sources /etc/apt/sources.list 2>/dev/null; for i in 1 2 3 4 5; do apt-get update && break || sleep 8; done && apt-get install -y --no-install-recommends \
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
COPY semiotic-bridge ./semiotic-bridge
# mcp-sim-engine: workspace member (9a45a96); manifest required by the resolver
# even though it is never built here, else "failed to load manifest" aborts.
COPY mcp-sim-engine ./mcp-sim-engine
RUN cargo build --release --locked -p relays-client

FROM debian:bookworm-slim AS runtime
# wget added for compose healthcheck (M2 Capa1 Fase 2 — 2026-05-15).
RUN sed -i "s|http://deb.debian.org|https://deb.debian.org|g" /etc/apt/sources.list.d/debian.sources /etc/apt/sources.list 2>/dev/null; for i in 1 2 3 4 5; do apt-get update && break || sleep 8; done && apt-get install -y --no-install-recommends ca-certificates tini wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
WORKDIR /app
COPY --from=builder /build/target/release/relays-client /usr/local/bin/relays-client
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
EXPOSE 3005
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/relays-client"]
