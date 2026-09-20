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
# SEC-3 (RULE 03 class): next.config.js headers() runs during `next build`,
# so the HSTS gate (ARBX_TLS_ENABLED === "true") must be present in the BUILD
# stage env — a compose `environment:` entry never reaches the bake. Compose
# passes it via build.args (prod: ${ARBX_TLS_ENABLED:-true}, dev: :-false).
# Default true: the public dapp is HTTPS; HTTP-only builds override to false.
ARG ARBX_TLS_ENABLED=true
ENV NEXT_PUBLIC_EDGE_URL=$NEXT_PUBLIC_EDGE_URL
ENV NEXT_PUBLIC_WS_URL=$NEXT_PUBLIC_WS_URL
ENV NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID=$NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID
ENV ARBX_TLS_ENABLED=$ARBX_TLS_ENABLED
# Prevent Corepack from mis-reading packageManager and trying yarn.
ENV COREPACK_ENABLE_STRICT=0
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

# Full lockfile install. No --silent: CI must surface install errors.
RUN npm ci --no-audit --no-fund

# App sources after install so dependency layer stays cacheable.
COPY frontend ./frontend
# PERF-STACK WO-9 (2026-09-20): output: "standalone" (next.config.js) makes the
# build trace the minimal runtime into frontend/.next/standalone — no more
# full-node_modules layout + `next start` in the runtime stage.
RUN npm run -w @arbx/frontend build

FROM node:20-bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends tini ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
ENV NODE_ENV=production
# Standalone tree: the build ran from the workspace root (/build), so Next
# nests the app at ./frontend/ inside .next/standalone (monorepo layout).
# server.js + traced node_modules come from the standalone copy; static assets
# and public/ are NOT traced and must be copied alongside (Next docs pattern).
COPY --from=builder --chown=arbx:arbx /build/frontend/.next/standalone ./
COPY --from=builder --chown=arbx:arbx /build/frontend/.next/static ./frontend/.next/static
COPY --from=builder --chown=arbx:arbx /build/frontend/public ./frontend/public
USER arbx
WORKDIR /app/frontend
EXPOSE 5173
# server.js reads HOSTNAME/PORT from env (defaults 0.0.0.0/3000).
ENV HOSTNAME=0.0.0.0 PORT=5173
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["node", "server.js"]
