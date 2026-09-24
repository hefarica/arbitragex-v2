FROM node:20-bookworm-slim AS builder
WORKDIR /build
COPY package.json ./
COPY shared-ts ./shared-ts
COPY edge/dev-local ./edge/dev-local
RUN npm install --workspaces --include-workspace-root --silent
RUN npm run -w @arbx/shared build \
 && npm run -w @arbx/edge-dev-local build

FROM node:20-bookworm-slim AS runtime
WORKDIR /app
RUN m1="http://deb.debian.org"; m2="http://ftp.de.debian.org"; s="/etc/apt/sources.list.d/debian.sources"; u=0; for i in 1 2 3 4 5 6 7 8; do if apt-get update; then u=1; break; fi; if [ $((i % 2)) = 1 ]; then sed -i "s|$m1|$m2|g" "$s" 2>/dev/null || true; else sed -i "s|$m2|$m1|g" "$s" 2>/dev/null || true; fi; sleep 10; done; [ "$u" = 1 ] || { echo "APT-GPG-02: apt update fallo 8 intentos con rotacion de espejo"; exit 1; } && apt-get install -y --no-install-recommends ca-certificates && sed -i "s|http://deb.debian.org|https://deb.debian.org|g" "$s" && apt-get update && apt-get install -y --no-install-recommends tini ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
COPY --from=builder /build/node_modules /app/node_modules
COPY --from=builder /build/shared-ts/dist /app/shared-ts/dist
COPY --from=builder /build/shared-ts/package.json /app/shared-ts/package.json
COPY --from=builder /build/edge/dev-local/dist /app/edge/dev-local/dist
COPY --from=builder /build/edge/dev-local/package.json /app/edge/dev-local/package.json
COPY configs /app/configs
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
EXPOSE 8787
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["node","/app/edge/dev-local/dist/index.js"]
