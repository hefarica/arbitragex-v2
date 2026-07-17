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
});
