FROM rust:1.91-slim-bookworm AS builder
# PERF-STACK WO-8 (2026-09-20): EPYC-Rome soporta x86-64-v3 (avx2+bmi2+fma
# verificado en VPS). Release SOLO se buildea en VPS (RULE 01).
ENV RUSTFLAGS="-C target-cpu=x86-64-v3"
WORKDIR /build
RUN m1="http://deb.debian.org"; m2="http://ftp.de.debian.org"; s="/etc/apt/sources.list.d/debian.sources"; u=0; for i in 1 2 3 4 5 6 7 8; do if apt-get update; then u=1; break; fi; if [ $((i % 2)) = 1 ]; then sed -i "s|$m1|$m2|g" "$s" 2>/dev/null || true; else sed -i "s|$m2|$m1|g" "$s" 2>/dev/null || true; fi; sleep 10; done; [ "$u" = 1 ] || { echo "APT-GPG-02: apt update fallo 8 intentos con rotacion de espejo"; exit 1; } && apt-get install -y --no-install-recommends ca-certificates && sed -i "s|http://deb.debian.org|https://deb.debian.org|g" "$s" && apt-get update && apt-get install -y --no-install-recommends \
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
RUN cargo build --release -p sim-ctl

FROM debian:bookworm-slim AS runtime
# wget added for compose healthcheck (M2 Capa1 Fase 2 — 2026-05-15).
RUN m1="http://deb.debian.org"; m2="http://ftp.de.debian.org"; s="/etc/apt/sources.list.d/debian.sources"; u=0; for i in 1 2 3 4 5 6 7 8; do if apt-get update; then u=1; break; fi; if [ $((i % 2)) = 1 ]; then sed -i "s|$m1|$m2|g" "$s" 2>/dev/null || true; else sed -i "s|$m2|$m1|g" "$s" 2>/dev/null || true; fi; sleep 10; done; [ "$u" = 1 ] || { echo "APT-GPG-02: apt update fallo 8 intentos con rotacion de espejo"; exit 1; } && apt-get install -y --no-install-recommends ca-certificates && sed -i "s|http://deb.debian.org|https://deb.debian.org|g" "$s" && apt-get update && apt-get install -y --no-install-recommends ca-certificates tini wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
WORKDIR /app
COPY --from=builder /build/target/release/sim-ctl /usr/local/bin/sim-ctl
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
EXPOSE 3003
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/sim-ctl"]
