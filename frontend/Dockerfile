FROM node:20-bookworm-slim AS builder
WORKDIR /build
# arbx-no-hardcode-doctrine: edge URL must be supplied at build time by the
# caller (compose passes it via build.args). next.config.js asserts it is set
# when NODE_ENV=production. No default here on purpose.
ARG NEXT_PUBLIC_EDGE_URL
ARG NEXT_PUBLIC_WS_URL
# Optional: WalletConnect/Reown project id. Default empty → wagmiConfig degrades
# fail-honest ('walletconnect_project_id_missing'); injected/MetaMask unaffected.
# The value comes from the caller's env (VPS .env / CI secret) — never hardcoded.
ARG NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID=
ENV NEXT_PUBLIC_EDGE_URL=$NEXT_PUBLIC_EDGE_URL
ENV NEXT_PUBLIC_WS_URL=$NEXT_PUBLIC_WS_URL
ENV NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID=$NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Root lockfile + every workspace package.json must be present before `npm ci`.
# The root package.json lists all workspaces; if any path is missing, npm exits 1
# (previously hidden by `--silent`, and masked on Docker Build by GHA layer cache).
COPY package.json package-lock.json ./
COPY shared-ts/package.json ./shared-ts/
COPY backend/selector-api/package.json ./backend/selector-api/
COPY backend/api-server/package.json ./backend/api-server/
COPY edge/worker/package.json ./edge/worker/
COPY edge/dev-local/package.json ./edge/dev-local/
COPY frontend/package.json ./frontend/

# Deterministic install of the frontend workspace graph. No --silent: CI must
# surface ERESOLVE/ENOENT/OOM instead of a blank exit 1.
RUN npm ci --workspace=@arbx/frontend --include-workspace-root --no-audit --no-fund

# App sources after install so dependency layer stays cacheable.
COPY frontend ./frontend
RUN npm run -w @arbx/frontend build

FROM node:20-bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends tini ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
ENV NODE_ENV=production
COPY --from=builder /build/node_modules /app/node_modules
COPY --from=builder /build/frontend/.next /app/frontend/.next
COPY --from=builder /build/frontend/package.json /app/frontend/package.json
COPY --from=builder /build/frontend/next.config.js /app/frontend/next.config.js
COPY --from=builder /build/frontend/public /app/frontend/public
USER arbx
WORKDIR /app/frontend
EXPOSE 5173
# node_modules is hoisted to /app/node_modules by npm workspaces.
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["node","/app/node_modules/next/dist/bin/next","start","-p","5173"]
