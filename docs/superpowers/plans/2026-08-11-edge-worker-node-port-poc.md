# edge/worker Node Port — POC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove `edge/worker/src/index.ts` (Hono, Cloudflare-shaped) runs in Node 20 on the VPS with a Redis-backed KV shim, so that `/api/recon/timeseries` (and the other ~60 worker-only routes) respond — the root fix for B-02.

**Architecture:** Add two modules beside the canonical `index.ts` (left byte-identical): `kv-redis.ts` implements the `KVNamespace` slice the worker uses over `ioredis`, and `node-server.ts` is the Node entry that wires `process.env` + two `RedisKV` instances into the worker's `Env` and serves via `@hono/node-server`. A `tsconfig.build.json` emits `dist/` without touching the wrangler `tsconfig.json`. Validate on the VPS at port 8788 (prod edge 8787 untouched).

**Tech Stack:** Node 20, Hono 4, `@hono/node-server`, `ioredis` 5, TypeScript 5.

## Global Constraints

- **Canonical source frozen:** `edge/worker/src/index.ts` must remain byte-identical. Do not edit it. A future Cloudflare deploy depends on it unchanged.
- **PROD edge untouched:** The POC binds to port **8788**. Never start anything on 8787. Never edit `compose.prod.yml` in this plan.
- **No backend services on local Windows (RULE 01):** All runtime execution is on the VPS via SSH (`arbx`). Local work is file editing + typecheck only.
- **No new Redis instance:** Reuse the existing `arbitragex-v2-redis-1`. Prefix all shim keys to avoid collisions: `edge:cache:` for ARBX_CACHE, `edge:rl:` for RATE_LIMIT.
- **§36 branch discipline:** Work on a fresh branch `poc/edge-worker-node` off `origin/main`. Verify `git branch --show-current` before every commit.
- **ioredis pattern:** Mirror dev-local's client options — `retryStrategy: (t) => Math.min(t*50, 2000)`, `maxRetriesPerRequest: 3` (edge/dev-local/src/index.ts:993-996).
- **Worker `Env` type (exact field names, edge/worker/src/index.ts:19-32):** `ARBX_ENV`, `API_SERVER_URL`, `ALLOWED_ORIGINS`, `ARBX_EDGE_TOKEN`, `JWT_SECRET`, `ARBX_CACHE: KVNamespace`, `RATE_LIMIT: KVNamespace`, `ARBX_TELEMETRY?: D1Database`, `SYBIL_ASN_DENYLIST?: string`.
- **KV methods the worker actually calls:** `env.ARBX_CACHE.get(key)` (string), `env.ARBX_CACHE.put(key, val, { expirationTtl })`, `env.RATE_LIMIT.get(key)`, `env.RATE_LIMIT.get(key, "json")`, `env.RATE_LIMIT.put(key, val, { expirationTtl })`, `env.RATE_LIMIT.delete(key)`.

---

### Task 1: `kv-redis.ts` — Redis-backed KVNamespace shim

**Files:**
- Create: `edge/worker/src/kv-redis.ts`
- Test: `edge/worker/src/kv-redis.test.ts`

**Interfaces:**
- Consumes: `ioredis` (default export `Redis`), Node `process.env`.
- Produces: `class RedisKV` implementing the KV slice above. Constructor signature: `new RedisKV(redis: Redis, prefix: string)`. Methods: `get(key): Promise<string | null>`, `get<T>(key, type: "json"): Promise<T | null>`, `put(key, value, opts?): Promise<void>`, `delete(key): Promise<void>`.

- [ ] **Step 1: Write the failing test**

Create `edge/worker/src/kv-redis.test.ts`:

```typescript
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { Redis } from "ioredis";
import { RedisKV } from "./kv-redis.js";

const REDIS_URL = process.env["REDIS_URL"] ?? "redis://localhost:6379";

describe("RedisKV", () => {
  let redis: Redis;
  let kv: RedisKV;

  beforeAll(() => {
    redis = new Redis(REDIS_URL, { maxRetriesPerRequest: 3 });
    kv = new RedisKV(redis, "test:kv:");
  });

  afterAll(async () => {
    await redis.del(await redis.keys("test:kv:*").then((ks) => ks.length ? redis.del(...ks) : 0));
    await redis.quit();
  });

  it("puts and gets a string value", async () => {
    await kv.put("greeting", "hello");
    expect(await kv.get("greeting")).toBe("hello");
  });

  it("returns null for a missing key", async () => {
    expect(await kv.get("does-not-exist")).toBeNull();
  });

  it("gets a JSON value with type:'json'", async () => {
    await kv.put("obj", JSON.stringify({ fails: 3, blockedUntil: 0 }));
    const v = await kv.get<{ fails: number; blockedUntil: number }>("obj", "json");
    expect(v).toEqual({ fails: 3, blockedUntil: 0 });
  });

  it("returns null on type:'json' when value is not parseable", async () => {
    await kv.put("bad", "{not json");
    expect(await kv.get("bad", "json")).toBeNull();
  });

  it("applies expirationTtl via SET EX", async () => {
    await kv.put("ephemeral", "gone", { expirationTtl: 1 });
    expect(await kv.get("ephemeral")).toBe("gone");
    await new Promise((r) => setTimeout(r, 1100));
    expect(await kv.get("ephemeral")).toBeNull();
  });

  it("deletes a key", async () => {
    await kv.put("todelete", "x");
    await kv.delete("todelete");
    expect(await kv.get("todelete")).toBeNull();
  });

  it("namespaces keys with the prefix", async () => {
    await kv.put("namespaced", "v");
    const raw = await redis.get("test:kv:namespaced");
    expect(raw).toBe("v");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run (on VPS, from repo root, inside the edge/worker workspace):
```bash
cd /opt/arbitragex-v2 && npx vitest run --root edge/worker src/kv-redis.test.ts
```
Expected: FAIL with `Failed to resolve import "./kv-redis.js"` or `RedisKV is not exported`.

- [ ] **Step 3: Write minimal implementation**

Create `edge/worker/src/kv-redis.ts`:

```typescript
import type { Redis } from "ioredis";

/**
 * Redis-backed implementation of the KVNamespace slice that edge/worker uses.
 * Mirrors Cloudflare KV semantics for the four operations the worker calls:
 * get(string), get(json), put(with optional expirationTtl), delete.
 *
 * All keys are prefixed so the worker's keyspace never collides with the
 * keys used by api-server / dev-local in the shared Redis instance.
 */
export class RedisKV {
  constructor(
    private readonly redis: Redis,
    private readonly prefix: string,
  ) {}

  private k(key: string): string {
    return `${this.prefix}${key}`;
  }

  async get(key: string): Promise<string | null>;
  async get<T>(key: string, type: "json"): Promise<T | null>;
  async get<T>(key: string, type?: "json"): Promise<string | T | null> {
    const raw = await this.redis.get(this.k(key));
    if (raw === null) return null;
    if (type === "json") {
      try {
        return JSON.parse(raw) as T;
      } catch {
        return null;
      }
    }
    return raw;
  }

  async put(
    key: string,
    value: string,
    opts?: { expirationTtl?: number },
  ): Promise<void> {
    const k = this.k(key);
    if (opts?.expirationTtl && opts.expirationTtl > 0) {
      await this.redis.set(k, value, "EX", opts.expirationTtl);
    } else {
      await this.redis.set(k, value);
    }
  }

  async delete(key: string): Promise<void> {
    await this.redis.del(this.k(key));
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cd /opt/arbitragex-v2 && npx vitest run --root edge/worker src/kv-redis.test.ts
```
Expected: PASS (all 7 tests). If Redis connection fails, confirm `REDIS_URL` is set and `arbitragex-v2-redis-1` is reachable from the shell.

- [ ] **Step 5: Commit**

```bash
git checkout -b poc/edge-worker-node origin/main
git add edge/worker/src/kv-redis.ts edge/worker/src/kv-redis.test.ts
git commit -m "feat(edge-worker): RedisKV shim — KVNamespace over ioredis for Node port POC"
```

---

### Task 2: `tsconfig.build.json` + dependency wiring

**Files:**
- Create: `edge/worker/tsconfig.build.json`
- Modify: `edge/worker/package.json` (dependencies block only)

**Interfaces:**
- Consumes: existing `edge/worker/tsconfig.json` (extends it).
- Produces: a build config that emits `edge/worker/dist/**/*.js` with Node module resolution; `@hono/node-server` and `ioredis` resolvable from `edge/worker`.

- [ ] **Step 1: Create the build tsconfig**

Create `edge/worker/tsconfig.build.json`:

```json
{
  "extends": "./tsconfig.json",
  "compilerOptions": {
    "noEmit": false,
    "outDir": "dist",
    "rootDir": "src",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "types": ["node"]
  },
  "include": ["src/**/*.ts"],
  "exclude": ["src/**/*.test.ts"]
}
```

- [ ] **Step 2: Add the build + start scripts and dependencies**

Modify `edge/worker/package.json`. Replace the `scripts` block and the `dependencies` block with:

```json
  "scripts": {
    "build": "tsc -p tsconfig.build.json",
    "build:check": "tsc --noEmit -p tsconfig.json",
    "start:node": "node dist/node-server.js",
    "dev": "wrangler dev",
    "deploy": "wrangler deploy",
    "test": "vitest run --passWithNoTests",
    "typecheck": "tsc --noEmit -p tsconfig.json"
  },
  "dependencies": {
    "@hono/node-server": "^1.13.7",
    "hono": "^4.12.18",
    "ioredis": "^5.4.6"
  },
```

Leave `devDependencies` unchanged (keep `@cloudflare/workers-types`, `typescript`, `vitest`, `wrangler`).

- [ ] **Step 3: Install on the VPS**

```bash
ssh arbx 'cd /opt/arbitragex-v2 && npm install --workspaces --include-workspace-root'
```
Expected: installs `@hono/node-server` + `ioredis` into the workspace; no lockfile conflict.

- [ ] **Step 4: Verify the build config compiles (will fail — node-server.ts not yet created)**

```bash
ssh arbx 'cd /opt/arbitragex-v2 && npx tsc -p edge/worker/tsconfig.build.json --noEmit'
```
Expected: FAIL — but the only errors should be about `node-server.ts` missing or `shared-ts` resolution, NOT about `tsconfig.build.json` itself being malformed. Confirm no "Cannot find tsconfig" error. This step just validates the config is well-formed before Task 3 supplies the entry.

- [ ] **Step 5: Commit**

```bash
git add edge/worker/tsconfig.build.json edge/worker/package.json
git commit -m "build(edge-worker): tsconfig.build.json (Node emit) + @hono/node-server + ioredis deps"
```

---

### Task 3: `node-server.ts` — Node entry wiring the worker `Env`

**Files:**
- Create: `edge/worker/src/node-server.ts`

**Interfaces:**
- Consumes: `app` (default export) from `./index.js`; `RedisKV` from `./kv-redis.js`; worker `Env` type (field names listed in Global Constraints).
- Produces: a runnable Node entry. `EDGE_PORT` env var selects the listen port (default 8788 for the POC).

- [ ] **Step 1: Write the Node entry**

Create `edge/worker/src/node-server.ts`:

```typescript
/**
 * Node entry for the canonical edge/worker Hono app — POC for B-02.
 *
 * The worker source (./index.ts) is byte-identical to the Cloudflare deploy;
 * this file only adapts the runtime: it builds the worker `Env` from
 * process.env + two RedisKV shim instances and serves via @hono/node-server.
 *
 * Bindings ARBX_TELEMETRY (D1) is intentionally left undefined — the worker
 * guards it with `if (c.env.ARBX_TELEMETRY)`, so the one executionCtx.waitUntil
 * reference is unreachable and the missing CF executionCtx cannot crash.
 */
import { serve } from "@hono/node-server";
import { Redis } from "ioredis";
import app from "./index.js";
import { RedisKV } from "./kv-redis.js";

const REDIS_URL = process.env["REDIS_URL"] ?? "redis://localhost:6379";
const PORT = Number(process.env["EDGE_PORT"] ?? process.env["PORT"] ?? 8788);

const redis = new Redis(REDIS_URL, {
  retryStrategy: (times: number) => Math.min(times * 50, 2000),
  maxRetriesPerRequest: 3,
});

const env = {
  ARBX_ENV: process.env["ARBX_ENV"] ?? "production",
  API_SERVER_URL: process.env["API_SERVER_URL"] ?? "http://127.0.0.1:8080",
  ALLOWED_ORIGINS: process.env["ALLOWED_ORIGINS"] ?? "",
  ARBX_EDGE_TOKEN: process.env["ARBX_EDGE_TOKEN"] ?? "",
  JWT_SECRET: process.env["JWT_SECRET"] ?? "",
  SYBIL_ASN_DENYLIST: process.env["SYBIL_ASN_DENYLIST"],
  ARBX_CACHE: new RedisKV(redis, "edge:cache:"),
  RATE_LIMIT: new RedisKV(redis, "edge:rl:"),
};

serve(
  { fetch: (req) => app.fetch(req, env), port: PORT },
  (info) => {
    console.log(JSON.stringify({
      event: "edge-worker.node.listen",
      port: info.port,
      api_server_url: env.API_SERVER_URL,
    }));
  },
);
```

- [ ] **Step 2: Build the worker to dist/**

```bash
ssh arbx 'cd /opt/arbitragex-v2 && npx tsc -p edge/worker/tsconfig.build.json'
```
Expected: emits `edge/worker/dist/index.js`, `dist/node-server.js`, `dist/kv-redis.js`. No TS errors. If `@cloudflare/workers-types`-only globals (`KVNamespace`, `D1Database`) cause type errors under NodeNext, confirm they are only *referenced* (in `Env`) — add a local minimal type alias at the top of `kv-redis.ts` if the compiler rejects the structural type; do NOT edit `index.ts`.

- [ ] **Step 3: Typecheck-only the canonical config (regression guard)**

```bash
ssh arbx 'cd /opt/arbitragex-v2 && npx tsc --noEmit -p edge/worker/tsconfig.json'
```
Expected: PASS unchanged — proves the canonical Cloudflare build is untouched.

- [ ] **Step 4: Commit**

```bash
git add edge/worker/src/node-server.ts
git commit -m "feat(edge-worker): node-server entry — Env wiring + @hono/node-server (POC)"
```

---

### Task 4: Run the POC on VPS :8788 and verify the 4 success criteria

**Files:**
- None (runtime validation only).

**Interfaces:**
- Consumes: the built `dist/node-server.js` from Task 3; live api-server on `127.0.0.1:8080`; live Redis.

- [ ] **Step 1: Pull the POC branch onto the VPS**

```bash
git push origin poc/edge-worker-node
ssh arbx 'cd /opt/arbitragex-v2 && git fetch origin && git checkout poc/edge-worker-node && npm install --workspaces --include-workspace-root && npx tsc -p edge/worker/tsconfig.build.json'
```
Expected: clean checkout + build on the VPS.

- [ ] **Step 2: Start the worker on :8788 (background, prod edge untouched)**

Read `ARBX_EDGE_TOKEN` and `REDIS_URL` from the VPS `.env`, then start detached:

```bash
ssh arbx 'cd /opt/arbitragex-v2 && \
  EDGE_TOKEN=$(grep -E "^ARBX_EDGE_TOKEN=" .env | cut -d= -f2-) && \
  REDIS_URL=$(grep -E "^REDIS_URL=" .env | cut -d= -f2- || echo redis://redis:6379) && \
  EDGE_PORT=8788 \
  API_SERVER_URL=http://127.0.0.1:8080 \
  ARBX_EDGE_TOKEN=$EDGE_TOKEN \
  REDIS_URL=$REDIS_URL \
  ALLOWED_ORIGINS=* \
  ARBX_ENV=production \
  nohup node edge/worker/dist/node-server.js > /tmp/edge-worker-poc.log 2>&1 & \
  echo "started pid $!"'
```
Expected: prints a PID, no immediate error. Prod edge on 8787 is untouched.

- [ ] **Step 3: Wait for listen + check startup log for CF-runtime errors**

```bash
ssh arbx 'for i in $(seq 1 15); do grep -q edge-worker.node.listen /tmp/edge-worker-poc.log && break; sleep 1; done; cat /tmp/edge-worker-poc.log'
```
Expected log line: `{"event":"edge-worker.node.listen","port":8788,...}`. **Success criterion #4:** log contains NONE of `cf is not defined`, `executionCtx`, `WebSocketPair is not defined`, `KVNamespace ... not defined`.

- [ ] **Step 4: Criterion #1 — `/health`**

```bash
ssh arbx 'curl -s http://127.0.0.1:8788/health'
```
Expected: HTTP 200, body contains `"edge-worker"`.

- [ ] **Step 5: Criterion #2 — `/api/recon/timeseries` (B-02 keystone)**

```bash
ssh arbx 'curl -s -w "\nHTTP %{http_code}\n" http://127.0.0.1:8788/api/recon/timeseries'
```
Expected: HTTP 200, JSON body has a `points` array.

- [ ] **Step 6: Criterion #3 — `/api/recon/summary` (no-regression)**

```bash
ssh arbx 'curl -s -w "\nHTTP %{http_code}\n" http://127.0.0.1:8788/api/recon/summary'
```
Expected: HTTP 200, JSON body has `window_hours` + `totals`.

- [ ] **Step 7: Stop the POC process**

```bash
ssh arbx 'pkill -f "edge/worker/dist/node-server.js" && echo stopped'
```
Expected: `stopped`. Prod edge still serving on 8787 (verify with `curl -s edge-arbx.ape-tv.net/health`).

- [ ] **Step 8: Record the verdict**

Append a verdict block to the spec doc (`docs/superpowers/specs/2026-08-11-edge-worker-node-port-poc-design.md`) under a new `## 8. POC verdict` section: PASS/FAIL per criterion, the raw curl outputs, and the `/tmp/edge-worker-poc.log` tail. Commit:

```bash
git add docs/superpowers/specs/2026-08-11-edge-worker-node-port-poc-design.md
git commit -m "docs(edge-worker-poc): record POC verdict (4-criteria result)"
```

---

## Self-Review (run before handoff)

**Spec coverage:** §3.1 kv-redis.ts → Task 1. §3.2 node-server.ts → Task 3. §3.3 tsconfig.build.json → Task 2. §3.4 Dockerfile.node → **deferred** (spec §4 says Dockerfile is written but NOT built during POC; this plan defers it entirely to the follow-up compose-wiring plan, which matches the spec's "Non-goals" for the POC). §4 execution → Task 4. §7 verdict gate → Task 4 Step 8. No gaps.

**Placeholder scan:** Every code step contains complete code. No TBD/TODO. The one conditional ("add a local minimal type alias at the top of kv-redis.ts if the compiler rejects the structural type") names the exact fallback and where it goes; it is not a placeholder, it is a typed runtime escape hatch for a known TS-strict risk.

**Type consistency:** `RedisKV` constructor `(redis, prefix)` — identical in Task 1 (def), Task 3 (use). Worker `Env` field names — identical across Global Constraints, Task 3 wiring, and the canonical source (index.ts:19-32). `kv.get(key)` vs `kv.get(key, "json")` overload — defined in Task 1, exercised by the worker (RATE_LIMIT lockout uses `"json"`). `app.fetch(req, env)` — the Hono Node-server calling convention, used in Task 3.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-11-edge-worker-node-port-poc.md`. Two execution options:

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks.
2. **Inline Execution** — execute tasks in this session with checkpoints.

Which approach?
