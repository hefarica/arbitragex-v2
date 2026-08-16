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
RUN npm run -w @arbx/frontend build \
 && NEXT_BIN="$(find /build/frontend/node_modules /build/node_modules -path '*/next/dist/bin/next' -type f 2>/dev/null | head -n1)" \
 && echo "resolved next binary: ${NEXT_BIN}" \
 && test -n "${NEXT_BIN}" \
 # Guarantee a stable layout for the runtime stage:
 #   /build/frontend/node_modules/next  → full package + peer graph siblings
 # so Node can resolve '../server/require-hook' and '@swc/helpers/*'.
 && if [ ! -e /build/frontend/node_modules/next ]; then \
      mkdir -p /build/frontend/node_modules; \
      NEXT_PKG="$(dirname "$(dirname "$(dirname "${NEXT_BIN}")")")"; \
      cp -a "${NEXT_PKG}" /build/frontend/node_modules/next; \
    fi \
 && test -f /build/frontend/node_modules/next/dist/bin/next \
 && printf '%s\n' '#!/bin/sh' 'exec node /app/frontend/node_modules/next/dist/bin/next "$@"' > /build/frontend-next-entrypoint.sh \
 && chmod 755 /build/frontend-next-entrypoint.sh

FROM node:20-bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends tini ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r arbx && useradd -r -g arbx arbx
ENV NODE_ENV=production
# Root hoist (shared deps) + workspace-local node_modules (next + @swc/helpers…).
COPY --from=builder /build/node_modules /app/node_modules
COPY --from=builder /build/frontend/node_modules /app/frontend/node_modules
COPY --from=builder /build/frontend/.next /app/frontend/.next
COPY --from=builder /build/frontend/package.json /app/frontend/package.json
COPY --from=builder /build/frontend/next.config.js /app/frontend/next.config.js
COPY --from=builder /build/frontend/public /app/frontend/public
COPY --from=builder /build/frontend-next-entrypoint.sh /app/frontend-next-entrypoint.sh
USER arbx
WORKDIR /app/frontend
EXPOSE 5173
# NODE_PATH lets nested next see root-hoisted packages when needed.
ENV NODE_PATH=/app/node_modules
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/app/frontend-next-entrypoint.sh","start","-p","5173"]
