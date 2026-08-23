import { describe, it, expect } from "vitest";
import {
  resolvePaperModeState,
  type ResolvePaperModeStateDeps,
  type RedisLike,
  type PaperModeState,
} from "./paper-mode-state.js";

function makeRedis(values: (string | null)[]): RedisLike {
  return {
    mget: async () => values,
  };
}

// PAPKEY-01 regression: the resolver must request the SAME per-chain key the
// canonical writer (POST /admin/config/paper-mode, B0.2+) sets. The old mock
// answered positionally and never saw the key names — which is exactly how the
// divergent `arbx:papermode:chain:<id>` read slipped through and made G-PAP-1
// un-satisfiable (confidence could never reach "explicit").
function recordingRedis(values: (string | null)[]) {
  const requestedKeys: string[][] = [];
  const redis: RedisLike = {
    mget: async (...keys: string[]) => {
      requestedKeys.push(keys);
      return values;
    },
  };
  return { redis, requestedKeys };
}

function chain(state: PaperModeState, chainId: number) {
  return state.chains.find((c) => c.chain_id === chainId);
}

describe("resolvePaperModeState()", () => {
  it("EXPLICIT when per-chain ON", async () => {
    const deps: ResolvePaperModeStateDeps = {
      redis: makeRedis([
        null,
        JSON.stringify({ enabled: true, updated_at: "2026-07-17T00:00:00Z" }),
      ]),
      enabledChainIds: [1],
      chainId: 1,
      env: {},
    };

    const state = await resolvePaperModeState(deps);

    expect(state.enabled).toBe(true);
    expect(state.confidence).toBe("explicit");
    expect(state.source).toBe("redis");
    expect(state.degraded).toBe(false);
    expect(state.conflict).toBe(false);
    expect(chain(state, 1)?.confidence).toBe("explicit");
    expect(chain(state, 1)?.enabled).toBe(true);
  });

  it("INFERRED when Redis empty + archiver ON", async () => {
    const deps: ResolvePaperModeStateDeps = {
      redis: makeRedis([null]),
      enabledChainIds: [1],
      chainId: 1,
      env: { ARBX_PAPER_ARCHIVER_MODE: "on" },
    };

    const state = await resolvePaperModeState(deps);

    expect(state.enabled).toBe(true);
    expect(state.confidence).toBe("inferred");
    expect(state.source).toBe("env");
    expect(chain(state, 1)?.confidence).toBe("inferred");
    expect(chain(state, 1)?.enabled).toBe(true);
  });

  it("CONFLICT when per-chain OFF + archiver ON", async () => {
    const deps: ResolvePaperModeStateDeps = {
      redis: makeRedis([
        null,
        JSON.stringify({ enabled: false, updated_at: "2026-07-17T00:00:00Z" }),
      ]),
      enabledChainIds: [1],
      chainId: 1,
      env: { ARBX_PAPER_ARCHIVER_MODE: "on" },
    };

    const state = await resolvePaperModeState(deps);

    expect(state.enabled).toBe(false);
    expect(state.conflict).toBe(true);
    expect(chain(state, 1)?.enabled).toBe(false);
    expect(chain(state, 1)?.conflict).toBe(true);
    expect(state.reasons.some((r) => /one or more chains/.test(r))).toBe(true);
  });

  it("explicit_legacy when only global exists", async () => {
    const deps: ResolvePaperModeStateDeps = {
      redis: makeRedis([
        JSON.stringify({ enabled: true, updated_at: "2026-07-17T00:00:00Z" }),
      ]),
      enabledChainIds: [1],
      chainId: 1,
      env: {},
    };

    const state = await resolvePaperModeState(deps);

    expect(state.enabled).toBe(true);
    expect(state.confidence).toBe("explicit_legacy");
    expect(state.degraded).toBe(true);
    expect(chain(state, 1)?.confidence).toBe("explicit_legacy");
    expect(state.reasons.some((r) => /legacy global key/.test(r))).toBe(true);
  });

  it("aggregated confidence = minimum", async () => {
    const deps: ResolvePaperModeStateDeps = {
      redis: makeRedis([
        null,
        JSON.stringify({ enabled: true, updated_at: "2026-07-17T00:00:00Z" }),
        null,
      ]),
      enabledChainIds: [1, 2],
      chainId: null,
      env: { ARBX_PAPER_ARCHIVER_MODE: "on" },
    };

    const state = await resolvePaperModeState(deps);

    expect(state.enabled).toBe(true);
    expect(state.confidence).toBe("inferred");
    expect(chain(state, 1)?.confidence).toBe("explicit");
    expect(chain(state, 2)?.confidence).toBe("inferred");
  });

  it("default_safe when no data", async () => {
    const deps: ResolvePaperModeStateDeps = {
      redis: makeRedis([null]),
      enabledChainIds: [1],
      chainId: 1,
      env: {},
    };

    const state = await resolvePaperModeState(deps);

    expect(state.enabled).toBe(true);
    expect(state.confidence).toBe("default_safe");
    expect(state.source).toBe("default");
    expect(chain(state, 1)?.confidence).toBe("default_safe");
  });

  it("PAPKEY-01: requests the canonical writer key arbx:papermode:<chain_id> (not the orphan chain: variant)", async () => {
    const { redis, requestedKeys } = recordingRedis([
      null,
      JSON.stringify({ enabled: true, updated_at: "2026-08-23T00:00:00Z" }),
    ]);
    const deps: ResolvePaperModeStateDeps = {
      redis,
      enabledChainIds: [1],
      chainId: 1,
      env: {},
    };

    const state = await resolvePaperModeState(deps);

    // One MGET: [legacy global, per-chain canonical keys...] — exactly the
    // keys POST /admin/config/paper-mode + paper-mode-reconcile write.
    expect(requestedKeys).toEqual([["arbx:papermode", "arbx:papermode:1"]]);
    expect(requestedKeys[0]!.join(",")).not.toContain("arbx:papermode:chain:");
    // And the value the writer actually sets is now observed as explicit.
    expect(state.confidence).toBe("explicit");
    expect(chain(state, 1)?.confidence).toBe("explicit");
  });

  it("PAPKEY-01: multi-chain MGET uses one canonical key per chain", async () => {
    const { redis, requestedKeys } = recordingRedis([
      null,
      JSON.stringify({ enabled: true }),
      JSON.stringify({ enabled: false }),
    ]);
    await resolvePaperModeState({
      redis,
      enabledChainIds: [1, 42161],
      chainId: null,
      env: {},
    });

    expect(requestedKeys).toEqual([
      ["arbx:papermode", "arbx:papermode:1", "arbx:papermode:42161"],
    ]);
  });
});
