FROM node:20-bookworm-slim AS builder
WORKDIR /build
# arbx-no-hardcode-doctrine: edge URL must be supplied at build time by the
# caller (compose passes it via build.args). next.config.js asserts it is set
# when NODE_ENV=production. No default here on purpose.
ARG NEXT_PUBLIC_EDGE_URL
ARG NEXT_PUBLIC_WS_URL
ENV NEXT_PUBLIC_EDGE_URL=$NEXT_PUBLIC_EDGE_URL
ENV NEXT_PUBLIC_WS_URL=$NEXT_PUBLIC_WS_URL
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY package.json ./
COPY frontend ./frontend
RUN npm install --workspaces --include-workspace-root --silent
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
