# Design: edge/worker Node Port — POC (B-02 root fix)

> **Status:** Approved design, pre-implementation.
> **Date:** 2026-08-11.
> **Anomaly:** B-02 (5 endpoints 404 in prod).
> **Skill flow:** brainstorming → (this spec) → writing-plans → implementation.

---

## 1. Problem (root cause, verified)

Production runs `edge/dev-local/` (DEV-ONLY Express shim) instead of `edge/worker/`
(canonical Hono Worker). Evidence (all gathered 2026-08-11):

- `compose.prod.yml` → `edge` service builds `dockerfile: edge/dev-local/Dockerfile`.
- Deployed edge container `dist/index.js` is Express-style and proxies ONLY
  `/api/recon/summary`; `grep timeseries` = 0. (The Hono `worker/src/index.ts:548`
  line that should proxy `/api/recon/timeseries` never reaches prod.)
- `edge/README.md` explicitly: "`dev-local/` — DEV-ONLY Express shim … **Do not
  deploy to production.** If worker and dev-local diverge, **worker is canonical**."
- api-server is fresh (image built 2026-08-11 16:21, has `recon/timeseries` at L755,
  returns 200 on `127.0.0.1:8080/api/v1/recon/timeseries`). The 404 is purely the
  edge layer.

Consequence: ~60 routes that exist only in `worker/` (recon/timeseries, operator,
wallets, drift, cartridges, killswitch, circuit-breakers, readiness extras, …) are
absent from prod. B-02's five 404s are the visible tip.

**Earlier false diagnosis (corrected):** initial hypothesis was "edge image cache
stale (Aug 9)." A `--no-cache` rebuild was performed; it compiled `dev-local/`
faithfully and changed nothing. The drift is **source-level, not cache-level**.

## 2. Goal of this POC

Prove that `edge/worker/src/index.ts` (Hono, Cloudflare-shaped) can run in a plain
Node 20 container against the existing VPS services, with no change to its canonical
source. On success, a follow-up plan wires it into `compose.prod.yml` (replacing
`dev-local`).

**Non-goals (this POC):** replacing the prod edge (port 8787), touching
`compose.prod.yml`, porting the WebSocket Carnot route, restoring Cloudflare
ASN/threat protection, or implementing the D1 telemetry sink.

## 3. Approach (chosen: A — separate entry + adapter)

```
edge/worker/
  src/
    index.ts          # EXISTING, CANONICAL, UNCHANGED (export default app)
    kv-redis.ts       # NEW (~60 lines): KVNamespace impl over ioredis
    node-server.ts    # NEW (~20 lines): Node entry — env wiring + serve()
  tsconfig.build.json # NEW: extends tsconfig.json with noEmit:false, outDir:dist
  Dockerfile.node     # NEW (~30 lines): build (tsc emit) + Node 20 runtime
  package.json        # MODIFIED: add @hono/node-server, ioredis
```

Nothing else is touched. `index.ts` stays byte-identical so a future Cloudflare
deploy still works.

### 3.1 `kv-redis.ts` — Redis-backed KVNamespace shim

Implements the slice of `KVNamespace` the worker uses:
- `get(key: string): Promise<string | null>`
- `get<T>(key: string, type: "json"): Promise<T | null>` (JSON.parse, null on fail)
- `put(key: string, value: string, opts?: { expirationTtl?: number }): Promise<void>`
  → Redis `SET key value EX ttl` (or `SET` without EX when ttl absent)
- `delete(key: string): Promise<void>`

All keys are namespaced with a constructor-supplied prefix (`edge:cache:` for
ARBX_CACHE, `edge:rl:` for RATE_LIMIT) to avoid collisions with existing Redis keys
used by api-server / dev-local. `ioredis` is already a project dependency (dev-local
uses it); reuse the same version.

### 3.2 `node-server.ts` — Node entry

1. Read `process.env` (`API_SERVER_URL`, `ARBX_EDGE_TOKEN`, `ALLOWED_ORIGINS`,
   `ARBX_ENV`, optional `JWT_SECRET`, `SYBIL_ASN_DENYLIST`, `REDIS_URL`, `PORT`).
2. Construct a single `ioredis` client from `REDIS_URL`.
3. Build the worker `Env` object: string fields from `process.env`; `ARBX_CACHE` =
   `new RedisKV(redis, "edge:cache:")`; `RATE_LIMIT` = `new RedisKV(redis,
   "edge:rl:")`; `ARBX_TELEMETRY` left `undefined` (worker guards on its presence).
4. Import `app` from `./index.js` and serve with `@hono/node-server`:
   `serve({ fetch: (req) => app.fetch(req, env), port: PORT ?? 8788 })`.
5. **`executionCtx` caveat:** Hono sets `c.executionCtx` only under the CF runtime;
   under `@hono/node-server` it is `undefined`. The worker references it once
   (`c.executionCtx.waitUntil`, L292), exclusively inside `if (c.env.ARBX_TELEMETRY)`.
   Because the POC leaves `ARBX_TELEMETRY` undefined, that branch is unreachable and
   the missing `executionCtx` cannot crash. **If** a future step enables telemetry,
   the fix is a Hono app-level middleware that injects a stub `executionCtx` onto the
   context before routes run — out of scope for this POC.

### 3.3 `tsconfig.build.json`

Extends `tsconfig.json` but overrides: `"noEmit": false`, `"outDir": "dist"`,
`"module": "NodeNext"`, `"moduleResolution": "NodeNext"`, drops
`"types": ["@cloudflare/workers-types"]` (replace with node types). The canonical
`tsconfig.json` (wrangler check) is untouched.

### 3.4 `Dockerfile.node`

Multi-stage, mirrors `edge/dev-local/Dockerfile` shape:
- builder: `node:20-bookworm-slim`, copy `package.json` + `shared-ts` + `edge/worker`,
  `npm install --workspaces`, build `@arbx/shared` then `tsc -p
  edge/worker/tsconfig.build.json`.
- runtime: `node:20-bookworm-slim`, copy `node_modules`, `shared-ts/dist`,
  `edge/worker/dist`, `configs`; `CMD ["node", "edge/worker/dist/node-server.js"]`.

**This Dockerfile is written but NOT built during the POC** — the POC runs the
worker directly via `node` after a manual `tsc` on the VPS, to iterate fast. The
Dockerfile is validated in the follow-up plan.

## 4. Execution (POC on VPS, isolated port 8788)

Per RULE 01 (no backend services on local Windows), the POC runs on the VPS only.

1. Commit the 3 new files + tsconfig.build + package.json on branch
   `poc/edge-worker-node`.
2. Push to VPS; on the VPS run `npm install` in `edge/worker` (for
   `@hono/node-server` + `ioredis`) and `npx tsc -p edge/worker/tsconfig.build.json`.
3. Start: `EDGE_PORT=8788 API_SERVER_URL=http://127.0.0.1:8080 ARBX_EDGE_TOKEN=<from
   .env> ALLOWED_ORIGINS=* REDIS_URL=<from .env> node edge/worker/dist/node-server.js`
   — bound to 8788, **never** 8787. Prod edge untouched.
4. Success criteria (4 binary curl checks):
   1. `curl -s :8788/health` → HTTP 200, body contains `"edge-worker"`.
   2. `curl -s :8788/api/recon/timeseries` → HTTP 200, JSON has a `points` array
      (the B-02 keystone route).
   3. `curl -s :8788/api/recon/summary` → HTTP 200 (no-regression).
   4. Startup logs contain none of: `cf is not defined`,
      `executionCtx`, `WebSocketPair is not defined`, `KVNamespace` type error.
5. On pass → kill the process, proceed to the follow-up compose-wiring plan. On fail
   → capture logs, diagnose the failing shim path, report back.

## 5. Risks & mitigations

| Risk | Evidence | Mitigation |
|---|---|---|
| `cf-connecting-ip` header absent | L226 already `?? "anon"` | Graceful; no action. |
| `c.req.raw.cf` undefined (ASN/threat) | L233-270 null-safe | Skipped silently; acceptable for POC. |
| `c.executionCtx.waitUntil` (L292) | Only inside `if (ARBX_TELEMETRY)` | Leave binding undefined; add waitUntil shim if needed. |
| `WebSocketPair` (L605, Carnot) | CF-specific | Only triggers on `upgrade: websocket`; POC curls don't send it. |
| `fetch(..., { cf: {} })` (L341 etc.) | Node fetch ignores unknown option | Harmless; no action. |
| Redis key collision | dev-local/api-server use Redis | Prefix all shim keys (`edge:cache:`, `edge:rl:`). |
| `@hono/node-server` env binding shape | `app.fetch(req, env)` is the Hono API | Verified against Hono docs pattern. |

## 6. Out of scope (follow-up plan, post-POC)

- Replace `edge` service in `compose.prod.yml` to build `Dockerfile.node`.
- Decide fate of `edge/dev-local/` (keep as dev-only or deprecate).
- `D1` telemetry (map to Postgres or drop).
- WebSocket Carnot via `ws` lib.
- L4 frontend validation (Playwright) that `/recon` renders the timeseries chart.

## 7. Verdict gate

POC passes iff all 4 criteria in §4 pass. A pass is **not** a prod deploy — it
unlocks the follow-up compose-wiring plan, which itself goes through CI (14 required
checks) + auto-deploy + L4 before B-02 is CLOSED.

## 8. POC verdict — PASS (2026-08-11)

All 4 success criteria met. Branch `poc/edge-worker-node` (HEAD `67f4992a`),
worker ran as a one-off container on `arbitragex-v2_arbx-net` at `:8788`,
proxying to `api-server:8080` with the Redis KV shim.

| # | Criterion | Result |
|---|---|---|
| 1 | `/health` → 200 + `edge-worker` | ✅ `{"ok":true,"service":"edge-worker","env":"production"}` |
| 2 | `/api/recon/timeseries` → 200 + `points` | ✅ 24 hourly buckets, honest zeros (no fabrication) |
| 3 | `/api/recon/summary` → 200 (no-regression) | ✅ totals + revert_rate + top_strategies |
| 4 | Startup log clean of CF-runtime errors | ✅ only `edge-worker.node.listen` |

**KV shim proven (not bypassed):** second call to `/api/recon/summary` returned
`x-arbx-cache: HIT` after the first `MISS`; `redis-cli KEYS "edge:cache:*"`
shows the key `edge:cache:arbx:cache:recon`. Both `ARBX_CACHE` and `RATE_LIMIT`
namespaces exercised through `RedisKV`.

**Build architecture (resolved during execution):** the build config uses
`types: ["@cloudflare/workers-types", "node"]` — `index.ts` was authored against
the CF types, so keeping them in the build (alongside node) yields a clean
compile. An earlier hand-rolled `cloudflare-types.d.ts` approach was abandoned
after it fought the Node lib types (undici augmentation poisoned
`Response`/`RequestInit`). `index.ts` remains byte-identical to the CF deploy.

**Production impact: zero.** POC container ran on `:8788`; prod edge (`:8787`)
untouched throughout — verified post-teardown: `/health` 200, `/api/recon/timeseries`
still 404 (deliberately not fixed until the compose-wiring follow-up).

**Unlocked:** follow-up plan to wire `Dockerfile.node` into `compose.prod.yml`
(replacing `dev-local`), then CI → auto-deploy → L4 (Playwright `/recon` chart).

