FROM rust:1.85-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml ./Cargo.toml
COPY shared-rs ./shared-rs
COPY searcher-rs ./searcher-rs
COPY sim-ctl ./sim-ctl
COPY relays-client ./relays-client
COPY recon ./recon
RUN cargo build --release -p relays-client

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
WORKDIR /app
COPY --from=builder /build/target/release/relays-client /usr/local/bin/relays-client
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
EXPOSE 3005
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/relays-client"]
