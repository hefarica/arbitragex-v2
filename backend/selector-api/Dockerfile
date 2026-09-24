# ArbitrageX v2 — selector-api (TS). Build context must be the REPO ROOT (not backend/) so workspaces resolve.
FROM node:20-bookworm-slim AS builder
WORKDIR /build
RUN m1="http://deb.debian.org"; m2="http://ftp.de.debian.org"; s="/etc/apt/sources.list.d/debian.sources"; u=0; for i in 1 2 3 4 5 6 7 8; do if apt-get update; then u=1; break; fi; if [ $((i % 2)) = 1 ]; then sed -i "s|$m1|$m2|g" "$s" 2>/dev/null || true; else sed -i "s|$m2|$m1|g" "$s" 2>/dev/null || true; fi; sleep 10; done; [ "$u" = 1 ] || { echo "APT-GPG-02: apt update fallo 8 intentos con rotacion de espejo"; exit 1; } && apt-get install -y --no-install-recommends ca-certificates && sed -i "s|http://deb.debian.org|https://deb.debian.org|g" "$s" && apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY package.json ./
COPY shared-ts ./shared-ts
COPY backend/selector-api ./backend/selector-api
RUN npm install --workspaces --include-workspace-root --silent
RUN npm run -w @arbx/shared build \
 && npm run -w @arbx/selector-api build
# PERF-STACK WO-3 (2026-09-20): runtime sin dev-deps (node_modules ~60-70% menor).
RUN npm prune --workspaces --include-workspace-root --omit=dev

FROM node:20-bookworm-slim AS runtime
WORKDIR /app
RUN m1="http://deb.debian.org"; m2="http://ftp.de.debian.org"; s="/etc/apt/sources.list.d/debian.sources"; u=0; for i in 1 2 3 4 5 6 7 8; do if apt-get update; then u=1; break; fi; if [ $((i % 2)) = 1 ]; then sed -i "s|$m1|$m2|g" "$s" 2>/dev/null || true; else sed -i "s|$m2|$m1|g" "$s" 2>/dev/null || true; fi; sleep 10; done; [ "$u" = 1 ] || { echo "APT-GPG-02: apt update fallo 8 intentos con rotacion de espejo"; exit 1; } && apt-get install -y --no-install-recommends ca-certificates && sed -i "s|http://deb.debian.org|https://deb.debian.org|g" "$s" && apt-get update && apt-get install -y --no-install-recommends tini ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
COPY --from=builder /build/node_modules /app/node_modules
COPY --from=builder /build/shared-ts/dist /app/shared-ts/dist
COPY --from=builder /build/shared-ts/package.json /app/shared-ts/package.json
COPY --from=builder /build/backend/selector-api/dist /app/backend/selector-api/dist
COPY --from=builder /build/backend/selector-api/package.json /app/backend/selector-api/package.json
COPY configs /app/configs
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
# PERF-STACK WO-3 (2026-09-20): semántica runtime prod (mensajes/overhead, no datos).
ENV NODE_ENV=production
EXPOSE 3002
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["node","/app/backend/selector-api/dist/index.js"]
