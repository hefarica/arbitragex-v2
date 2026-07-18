# Paper-Mode State Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a per-chain, auditable, consistent, degradable, and fail-safe paper-mode state authority across the entire ArbitrageX v2 stack, replacing fragmented boolean sources with a structured `PaperModeState` resolver.

**Architecture:** A single pure `resolvePaperModeState()` resolver reads Redis per-chain keys (with MGET), falls back through confidence tiers (explicit → observed → inferred → default_safe), and never conflates passive archiver evidence with explicit operator authority. All readiness endpoints, frontend components, and tests consume this one canonical source.

**Tech Stack:** TypeScript (Node 20), ioredis, Express, Vitest, Playwright, Next.js 14, Zod

## Global Constraints

- Redis: use `MGET` only; `KEYS` is prohibited in production
- Chain-id is mandatory everywhere; no default (especially not `1`)
- Auto-seed on startup is prohibited; reconciler is gated by `ARBX_PAPER_AUTO_RECONCILE=on`
- Global Redis key `arbx:papermode` is legacy-degraded (confidence `explicit_legacy`, grade YELLOW)
- Inferred confidence never produces GREEN; only EXPLICIT does
- All writes require: admin token, CSRF, chain validation, mainnet policy, audit log
- No live/capital paths are modified; all changes are additive/read-only on existing code

---

## Task 1: Types and Pure Resolver

**Files:**
- Create: `backend/api-server/src/readiness/paper-mode-state.ts`
- Test: `backend/api-server/src/readiness/paper-mode-state.test.ts`

**Interfaces:**
- Produces: `PaperModeConfidence`, `ChainPaperMode`, `PaperModeState`, `resolvePaperModeState()`

- [ ] **Step 1: Write the failing test**

```typescript
// paper-mode-state.test.ts
import { describe, it, expect } from "vitest";
import { resolvePaperModeState } from "./paper-mode-state.js";

function makeRedis(stubs: Record<string, string | null>) {
  return {
    mget: async (keys: string[]) => keys.map((k) => stubs[k] ?? null),
  } as any;
}

describe("resolvePaperModeState", () => {
  it("returns EXPLICIT when all per-chain keys are ON", async () => {
    const state = await resolvePaperModeState({
      redis: makeRedis({ "arbx:papermode:1": '{"enabled":true}' }),
      env: {},
      enabledChainIds: [1],
    });
    expect(state.enabled).toBe(true);
    expect(state.confidence).toBe("explicit");
    expect(state.chains[0].enabled).toBe(true);
  });

  it("returns INFERRED when Redis empty but archiver ON", async () => {
    const state = await resolvePaperModeState({
      redis: makeRedis({}),
      env: { ARBX_PAPER_ARCHIVER_MODE: "on" },
      enabledChainIds: [1],
    });
    expect(state.enabled).toBe(true);
    expect(state.confidence).toBe("inferred");
    expect(state.degraded).toBe(false);
  });

  it("returns CONFLICT when per-chain OFF but archiver ON", async () => {
    const state = await resolvePaperModeState({
      redis: makeRedis({ "arbx:papermode:1": '{"enabled":false}' }),
      env: { ARBX_PAPER_ARCHIVER_MODE: "on" },
      enabledChainIds: [1],
    });
    expect(state.conflict).toBe(true);
    expect(state.enabled).toBe(false);
  });

  it("returns explicit_legacy when only global key exists", async () => {
    const state = await resolvePaperModeState({
      redis: makeRedis({ "arbx:papermode": '{"enabled":true}' }),
      env: {},
      enabledChainIds: [1, 42161],
    });
    expect(state.confidence).toBe("explicit_legacy");
    expect(state.degraded).toBe(true);
  });

  it("aggregated confidence is minimum of chains", async () => {
    const state = await resolvePaperModeState({
      redis: makeRedis({
        "arbx:papermode:1": '{"enabled":true}',
        "arbx:papermode:42161": null,
      }),
      env: { ARBX_PAPER_ARCHIVER_MODE: "on" },
      enabledChainIds: [1, 42161],
    });
    expect(state.confidence).toBe("inferred");
  });

  it("returns default_safe when no data at all", async () => {
    const state = await resolvePaperModeState({
      redis: makeRedis({}),
      env: {},
      enabledChainIds: [1],
    });
    expect(state.enabled).toBe(true);
    expect(state.confidence).toBe("default_safe");
    expect(state.degraded).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd backend/api-server && npx vitest run src/readiness/paper-mode-state.test.ts`
Expected: FAIL — `resolvePaperModeState` not defined

- [ ] **Step 3: Implement resolver**

```typescript
// paper-mode-state.ts
import type { Redis } from "ioredis";

export type PaperModeConfidence =
  | "explicit"
  | "explicit_legacy"
  | "observed"
  | "inferred"
  | "default_safe";

export interface ChainPaperMode {
  chain_id: number;
  enabled: boolean;
  source: string;
  confidence: PaperModeConfidence;
  conflict: boolean;
  updated_at: string | null;
}

export interface PaperModeState {
  enabled: boolean;
  chain_id: number | null;
  source: string;
  confidence: PaperModeConfidence;
  degraded: boolean;
  conflict: boolean;
  updated_at: string | null;
  reasons: string[];
  chains: ChainPaperMode[];
}

function parseEnabled(raw: string | null): boolean | null {
  if (raw === null) return null;
  try {
    return (JSON.parse(raw) as { enabled?: unknown }).enabled === true;
  } catch {
    return raw === "1" || raw === "true";
  }
}

function extractUpdatedAt(raw: string | null): string | null {
  if (!raw) return null;
  try {
    return (JSON.parse(raw) as { updated_at?: string }).updated_at ?? null;
  } catch {
    return null;
  }
}

function confidenceRank(c: PaperModeConfidence): number {
  const map: Record<PaperModeConfidence, number> = {
    explicit: 4,
    explicit_legacy: 3,
    observed: 2,
    inferred: 1,
    default_safe: 0,
  };
  return map[c];
}

function minConfidence(a: PaperModeConfidence, b: PaperModeConfidence): PaperModeConfidence {
  return confidenceRank(a) < confidenceRank(b) ? a : b;
}

export async function resolvePaperModeState(opts: {
  redis: Pick<Redis, "mget"> | null;
  env: NodeJS.ProcessEnv;
  enabledChainIds: number[];
  chainId?: number | null;
  logger?: { warn?: (obj: object, msg?: string) => void };
}): Promise<PaperModeState> {
  const { redis, env, enabledChainIds, chainId = null, logger } = opts;
  const reasons: string[] = [];
  const chains: ChainPaperMode[] = [];
  let globalEnabled: boolean | null = null;
  let globalRaw: string | null = null;

  if (redis && enabledChainIds.length > 0) {
    try {
      const perChainKeys = enabledChainIds.map((id) => `arbx:papermode:${id}`);
      const values = await redis.mget(["arbx:papermode", ...perChainKeys]);
      globalRaw = values[0] ?? null;
      globalEnabled = parseEnabled(globalRaw);

      for (let i = 0; i < enabledChainIds.length; i++) {
        const cid = enabledChainIds[i];
        const raw = values[i + 1] ?? null;
        const explicit = parseEnabled(raw);
        const updatedAt = extractUpdatedAt(raw);

        if (explicit !== null) {
          chains.push({
            chain_id: cid,
            enabled: explicit,
            source: `redis:arbx:papermode:${cid}`,
            confidence: "explicit",
            conflict: false,
            updated_at: updatedAt,
          });
        } else if (globalEnabled !== null) {
          chains.push({
            chain_id: cid,
            enabled: globalEnabled,
            source: "redis:arbx:papermode (legacy global)",
            confidence: "explicit_legacy",
            conflict: false,
            updated_at: extractUpdatedAt(globalRaw),
          });
        } else {
          chains.push({
            chain_id: cid,
            enabled: true,
            source: "pending_explicit_state",
            confidence: "inferred",
            conflict: false,
            updated_at: null,
          });
          reasons.push(`missing_explicit_state:${cid}`);
        }
      }
    } catch (e) {
      logger?.warn?.(
        { event: "paper_mode_state.redis_err", err: (e as Error).message },
        "Redis MGET failed",
      );
      reasons.push("redis_degraded");
    }
  }

  if (chains.length === 0) {
    const passiveArchiverOn =
      (env["ARBX_PAPER_ARCHIVER_MODE"] ?? "").toLowerCase() === "on" ||
      (env["ARBX_OPPS_BRIDGE_MODE"] ?? "").toLowerCase() === "on";

    if (passiveArchiverOn) {
      reasons.push("inferred_from_archiver_env");
      for (const cid of enabledChainIds.length > 0 ? enabledChainIds : [null as any]) {
        chains.push({
          chain_id: cid ?? 0,
          enabled: true,
          source: "env:ARBX_PAPER_ARCHIVER_MODE=on",
          confidence: "inferred",
          conflict: false,
          updated_at: null,
        });
      }
    } else {
      const tradeMode = env["ARBX_TRADE_MODE"];
      const fallbackEnabled = tradeMode === undefined || tradeMode === "paper";
      reasons.push(`fallback_from_env:ARBX_TRADE_MODE=${tradeMode ?? "unset"}`);
      for (const cid of enabledChainIds.length > 0 ? enabledChainIds : [null as any]) {
        chains.push({
          chain_id: cid ?? 0,
          enabled: fallbackEnabled,
          source: "env:ARBX_TRADE_MODE",
          confidence: "inferred",
          conflict: false,
          updated_at: null,
        });
      }
    }
  }

  const hasConflict =
    chains.some((c) => c.conflict) ||
    (chains.some((c) => c.enabled) && chains.some((c) => !c.enabled));

  if (hasConflict) {
    reasons.push("chain_conflict_detected");
  }

  const allEnabled = chains.every((c) => c.enabled);
  const aggregatedConfidence = chains.reduce(
    (min, c) => minConfidence(min, c.confidence),
    "explicit" as PaperModeConfidence,
  );
  const degraded = chains.some((c) => c.confidence === "explicit_legacy" || c.confidence === "inferred" || c.confidence === "default_safe");

  return {
    enabled: allEnabled,
    chain_id: chainId,
    source: chainId !== null
      ? chains.find((c) => c.chain_id === chainId)?.source ?? "aggregated"
      : "aggregated",
    confidence: hasConflict ? "inferred" : aggregatedConfidence,
    degraded: degraded || reasons.includes("redis_degraded"),
    conflict: hasConflict,
    updated_at: chains
      .map((c) => c.updated_at)
      .filter(Boolean)
      .sort()
      .pop() ?? null,
    reasons,
    chains,
  };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd backend/api-server && npx vitest run src/readiness/paper-mode-state.test.ts`
Expected: PASS (6/6)

- [ ] **Step 5: Commit**

```bash
git add backend/api-server/src/readiness/paper-mode-state.ts backend/api-server/src/readiness/paper-mode-state.test.ts
git commit -m "feat(readiness): add per-chain PaperModeState resolver with MGET"
```

---

## Remaining Tasks Summary

| Task | Files | What |
|---|---|---|
| T2 | `paper-mode-readiness.ts`, `paper-mode-readiness.test.ts` | Combine `PaperModeState` + accumulation → grade |
| T3 | `g-pap-1.ts` | Wire resolver + grader |
| T4 | `readiness-extras.ts`, `readiness-steps.ts`, `index.ts` | Wire real Redis |
| T5 | `paper-mode-state.ts` (route) | `GET /api/paper-mode/state?chain_id=` |
| T6 | `paper-mode-reconcile.ts` | Lua-atomic reconciler with dry-run |
| T7 | `api-client.ts`, `usePaperModeState.ts` | Frontend API + hook |
| T8 | `paper-mode-toggle.tsx`, `SystemGuardBanner.tsx`, `site-header.tsx` | Frontend components |
| T9 | `paper-mode-alignment.spec.ts`, `paper-mode-end-to-end.spec.ts` | Playwright E2E |
| T10 | — | Verification, typecheck, tests, deploy |

---

## Rollback

```bash
git revert HEAD~9..HEAD
git push
docker compose --env-file .env -f docker/compose.prod.yml up -d --build api-server
```
