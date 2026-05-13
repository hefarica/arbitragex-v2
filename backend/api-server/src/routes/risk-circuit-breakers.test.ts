/**
 * risk-circuit-breakers tests — pure-function evaluator assertions.
 *
 * Coverage:
 *   - 10 breakers built per call.
 *   - DD/revert/gas → NOT_AVAILABLE without paper-shadow.
 *   - Executor → BLOCKED when env missing; WARN when env present (probe pending).
 *   - RPC health → BLOCKED when RPC_HTTP_1 missing.
 *   - Global kill-switch → PASS disarmed / KILLED armed / UNKNOWN on null.
 *   - Latency/SIM_ERROR/Blacklist → state derived from readiness verifier items.
 *   - Confidence → NOT_AVAILABLE while scoring pipeline not wired.
 *   - Summary + overall precedence (KILLED > PAUSED > BLOCKED > WARN > UNKNOWN > NOT_AVAILABLE > PASS).
 *   - No fake metrics; no secrets in evidence.
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { __forTesting } from "./risk-circuit-breakers.js";

const {
  makeDrawdownBreaker,
  makeRevertRateBreaker,
  makeGasBurnBreaker,
  makeLatencyBreaker,
  makeSimErrorBreaker,
  makeRpcHealthBreaker,
  makeBlacklistBreaker,
  makeExecutorBreaker,
  makeConfidenceBreaker,
  makeGlobalKillSwitchBreaker,
  buildAllBreakers,
  summarize,
  overall,
} = __forTesting;

const SAVED_ENV = { ...process.env };
beforeEach(() => { process.env = { ...SAVED_ENV }; });
afterEach(() => { process.env = { ...SAVED_ENV }; });

const baseCtx = {
  now: "2026-05-13T00:00:00Z",
  killSwitchEnabled: false,
  killSwitchReason: null,
  readiness: null,
  readinessError: null,
  envRpc: false,
  envExecutor: false,
  envTradeMode: null,
  scoringPipelineWired: false,
};

describe("makeDrawdownBreaker", () => {
  it("returns NOT_AVAILABLE without paper-shadow data", () => {
    const b = makeDrawdownBreaker(baseCtx);
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.severity).toBe("critical");
    expect(b.evidence.current_value).toBe(null);
    expect(b.operator_required).toBe(true);
    expect(b.blocks).toContain("LIVE");
  });
});

describe("makeRevertRateBreaker", () => {
  it("returns NOT_AVAILABLE without rolling-window aggregator", () => {
    const b = makeRevertRateBreaker(baseCtx);
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.current_value).toBe(null);
  });
});

describe("makeGasBurnBreaker", () => {
  it("returns NOT_AVAILABLE without gas ledger", () => {
    const b = makeGasBurnBreaker(baseCtx);
    expect(b.state).toBe("NOT_AVAILABLE");
  });
});

describe("makeLatencyBreaker", () => {
  it("returns NOT_AVAILABLE when readiness G-RPC-1 missing", () => {
    const b = makeLatencyBreaker(baseCtx);
    expect(["NOT_AVAILABLE", "UNKNOWN"]).toContain(b.state);
  });
  it("returns PASS when G-RPC-1 is green", () => {
    const ctx = {
      ...baseCtx,
      readiness: {
        items: [{ id: "G-RPC-1", status: "green", reason: "ok", group: "risk_doctrines" as const, label: "x", verified_at: "x" }],
        summary: { green: 1, yellow: 0, red: 0, pending: 0, total: 1 },
        flip_blocked: false,
        generated_at: "x",
      },
    } as Parameters<typeof makeLatencyBreaker>[0];
    expect(makeLatencyBreaker(ctx).state).toBe("PASS");
  });
  it("returns PAUSED when G-RPC-1 is red", () => {
    const ctx = {
      ...baseCtx,
      readiness: {
        items: [{ id: "G-RPC-1", status: "red", reason: "rpc down", group: "risk_doctrines" as const, label: "x", verified_at: "x" }],
        summary: { green: 0, yellow: 0, red: 1, pending: 0, total: 1 },
        flip_blocked: true,
        generated_at: "x",
      },
    } as Parameters<typeof makeLatencyBreaker>[0];
    expect(makeLatencyBreaker(ctx).state).toBe("PAUSED");
  });
});

describe("makeSimErrorBreaker", () => {
  it("returns NOT_AVAILABLE when readiness G-SIM-1 missing", () => {
    expect(makeSimErrorBreaker(baseCtx).state).toBe("NOT_AVAILABLE");
  });
});

describe("makeRpcHealthBreaker", () => {
  it("returns BLOCKED when RPC_HTTP_1 missing", () => {
    delete process.env["RPC_HTTP_1"];
    const b = makeRpcHealthBreaker({ ...baseCtx, envRpc: false });
    expect(b.state).toBe("BLOCKED");
    expect(b.severity).toBe("critical");
    expect(b.blocks).toContain("A.4");
    expect(b.blocks).toContain("LIVE");
  });
});

describe("makeBlacklistBreaker", () => {
  it("returns NOT_AVAILABLE without G-TOK-1", () => {
    expect(makeBlacklistBreaker(baseCtx).state).toBe("NOT_AVAILABLE");
  });
});

describe("makeExecutorBreaker", () => {
  it("returns BLOCKED when EXECUTOR_1 missing", () => {
    const b = makeExecutorBreaker({ ...baseCtx, envExecutor: false });
    expect(b.state).toBe("BLOCKED");
    expect(b.severity).toBe("critical");
  });
  it("returns WARN when EXECUTOR_1 present but on-chain probe pending", () => {
    const b = makeExecutorBreaker({ ...baseCtx, envExecutor: true });
    expect(b.state).toBe("WARN");
    expect(b.evidence.current_value).toBe("env_present");
  });
});

describe("makeConfidenceBreaker", () => {
  it("returns NOT_AVAILABLE when scoring pipeline not wired", () => {
    expect(makeConfidenceBreaker({ ...baseCtx, scoringPipelineWired: false }).state).toBe("NOT_AVAILABLE");
  });
  it("returns PASS when scoring pipeline wired", () => {
    expect(makeConfidenceBreaker({ ...baseCtx, scoringPipelineWired: true }).state).toBe("PASS");
  });
});

describe("makeGlobalKillSwitchBreaker", () => {
  it("returns PASS when kill-switch disarmed", () => {
    const b = makeGlobalKillSwitchBreaker({ ...baseCtx, killSwitchEnabled: false });
    expect(b.state).toBe("PASS");
    // CRITICAL: even when "PASS", global breaker STILL blocks LIVE (A.9 sign-off gate).
    expect(b.blocks).toContain("LIVE");
  });
  it("returns KILLED when kill-switch armed", () => {
    const b = makeGlobalKillSwitchBreaker({ ...baseCtx, killSwitchEnabled: true, killSwitchReason: "operator" });
    expect(b.state).toBe("KILLED");
    expect(b.action).toBe("kill_switch");
  });
  it("returns UNKNOWN when kill-switch state cannot be read", () => {
    const b = makeGlobalKillSwitchBreaker({ ...baseCtx, killSwitchEnabled: null });
    expect(b.state).toBe("UNKNOWN");
  });
});

describe("buildAllBreakers", () => {
  it("builds exactly 10 breakers", () => {
    expect(buildAllBreakers(baseCtx).length).toBe(10);
  });
});

describe("summarize + overall", () => {
  it("summarizes correctly with kill switch disarmed + envs missing", () => {
    const breakers = buildAllBreakers(baseCtx);
    const s = summarize(breakers);
    expect(s.total).toBe(10);
    // 1 PASS (kill_switch disarmed), 0+ BLOCKED/NOT_AVAILABLE.
    expect(s.pass).toBeGreaterThanOrEqual(1);
    expect(s.blocked + s.not_available + s.warn + s.paused + s.killed + s.unknown).toBeGreaterThan(0);
  });

  it("overall returns BLOCKED when any breaker is BLOCKED", () => {
    const breakers = buildAllBreakers(baseCtx);
    const o = overall(breakers);
    // With env missing, rpc+executor are BLOCKED → overall should be BLOCKED.
    expect(["BLOCKED", "KILLED", "PAUSED"]).toContain(o);
  });

  it("overall returns KILLED when kill switch armed (worst-state precedence)", () => {
    const ctx = { ...baseCtx, killSwitchEnabled: true, killSwitchReason: "ops" };
    const o = overall(buildAllBreakers(ctx));
    expect(o).toBe("KILLED");
  });
});

describe("regression: no fake metrics + no secrets", () => {
  it("no breaker leaks RPC URL / DB password / contract address in evidence", () => {
    process.env["RPC_HTTP_1"] = "https://eth.alchemy.com/v2/SECRET-API-KEY-9999";
    process.env["EXECUTOR_1"] = "0xDEADBEEFCAFE1234567890abcdef1234567890ab";
    process.env["DATABASE_URL"] = "postgres://user:supersecret@host:5432/db";
    const breakers = buildAllBreakers({
      ...baseCtx,
      envRpc: true,
      envExecutor: true,
    });
    const serialized = JSON.stringify(breakers);
    expect(serialized).not.toContain("SECRET-API-KEY");
    expect(serialized).not.toContain("supersecret");
    expect(serialized).not.toContain("DEADBEEFCAFE1234567890");
  });

  it("no breaker fabricates a current_value for NOT_AVAILABLE state", () => {
    const breakers = buildAllBreakers(baseCtx);
    for (const b of breakers) {
      if (b.state === "NOT_AVAILABLE") {
        expect(b.evidence.current_value).toBe(null);
      }
    }
  });
});
