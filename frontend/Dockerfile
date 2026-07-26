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
# Build, then force-hoist the next package to /build/node_modules/next so the
# runtime CMD path is stable regardless of whether npm nested it under
# frontend/node_modules (workspace install) or the root hoist.
RUN npm run -w @arbx/frontend build \
 && NEXT_PKG="$(find /build -type d -path '*/node_modules/next' ! -path '*/next/node_modules/*' | head -n1)" \
 && echo "resolved next package: ${NEXT_PKG}" \
 && test -n "${NEXT_PKG}" \
 && test -f "${NEXT_PKG}/dist/bin/next" \
 && mkdir -p /build/node_modules \
 && if [ "${NEXT_PKG}" != "/build/node_modules/next" ]; then \
      rm -rf /build/node_modules/next; \
      cp -a "${NEXT_PKG}" /build/node_modules/next; \
    fi \
 && test -f /build/node_modules/next/dist/bin/next

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
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["node","/app/node_modules/next/dist/bin/next","start","-p","5173"]
