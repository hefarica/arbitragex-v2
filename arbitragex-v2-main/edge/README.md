# Edge

This directory holds the ArbitrageX v2 edge layer. Two flavors, **one source of truth
for behavior** (they must expose the same public interface):

## `worker/` — canonical production edge (Cloudflare Workers)

Deployed via `wrangler deploy`. Provides:
- JWT auth + optional Cloudflare Access in front.
- Rate-limit (per-isolate in S1, KV-backed in S7).
- Read cache in KV (2 s TTL for `/status` and `/api/opportunities/live`).
- Sanitization and CORS.
- Injects `x-arbx-edge-token` when calling `api-server`.
- **Never** calls hot-path services directly.

### Local dev

```bash
cd edge/worker
npm install
npm run dev      # wrangler dev on :8787
```

Requires `wrangler.toml` KV/D1 IDs to be filled. Secrets via `wrangler secret put`.

## `dev-local/` — DEV-ONLY Express shim

A small Node/Express service that exposes the same routes as the Worker so developers
without Cloudflare accounts can run the full docker-compose stack. **Do not deploy to
production.** It is marked explicitly in its `package.json` description.

### Local

```bash
npm run -w @arbx/edge-dev-local dev
# or inside docker-compose:
docker compose -f docker/docker-compose.prod-like.yml up edge
```

## Contracts

Both variants MUST:
- expose `GET /health` and `GET /metrics`
- propagate `x-arbx-trace-id`
- inject `x-arbx-edge-token` on every upstream call
- never surface internal endpoint URLs to clients
- proxy read-only paths only (`/status`, `/api/opportunities/live`, `/api/risk/alerts`)

## Security posture

- Only `edge/*` is exposed to the Internet. All other services listen only on the internal docker network.
- Any admin action (kill-switch, config mutation) goes through `api-server`, not the edge.
- If worker and dev-local diverge in behavior, **worker is canonical**.
