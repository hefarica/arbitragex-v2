FROM node:20-bookworm-slim AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY package.json ./
COPY shared-ts ./shared-ts
COPY backend/api-server ./backend/api-server
RUN npm install --workspaces --include-workspace-root --silent
RUN npm run -w @arbx/shared build \
 && npm run -w @arbx/api-server build

FROM node:20-bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends tini ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
COPY --from=builder /build/node_modules /app/node_modules
COPY --from=builder /build/shared-ts/dist /app/shared-ts/dist
COPY --from=builder /build/shared-ts/package.json /app/shared-ts/package.json
COPY --from=builder /build/backend/api-server/dist /app/backend/api-server/dist
COPY --from=builder /build/backend/api-server/package.json /app/backend/api-server/package.json
COPY configs /app/configs
USER arbx
ENV ARBX_CONFIG_PATH=/app/configs/app.toml
EXPOSE 8080
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["node","/app/backend/api-server/dist/index.js"]
