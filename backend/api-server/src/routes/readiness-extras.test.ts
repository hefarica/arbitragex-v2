/**
 * readiness-extras tests — pure-function assertions (no Express, no PG).
 *
 * Coverage:
 *   - probeEnv redacts values and reports presence/length only.
 *   - envBlockers detects missing critical vars.
 *   - envBlockers tolerates paper-mode and blocks non-paper modes.
 *   - doctrinalBlockers emits the 5 remaining phase items (A.4 resolved
 *     2026-08-20 via gate_c_validation fork-validation evidence).
 *   - summarize counts severities + unions blocked phases.
 *   - overallStatus follows the precedence (critical > partial > ready).
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { __forTesting } from "./readiness-extras.js";

const { probeEnv, envBlockers, doctrinalBlockers, summarize, overallStatus } = __forTesting;

// Snapshot + restore process.env around each test to keep them isolated.
const SAVED_ENV = { ...process.env };
beforeEach(() => { process.env = { ...SAVED_ENV }; });
afterEach(() => { process.env = { ...SAVED_ENV }; });

describe("probeEnv (redaction contract)", () => {
  it("returns env_present=false when variable is unset", () => {
    delete process.env["ARBX_PROBE_TEST"];
    const ev = probeEnv("ARBX_PROBE_TEST");
    expect(ev.env_present).toBe(false);
    expect(ev.redacted_value).toBe(null);
    expect(ev.value_length).toBe(null);
    expect(ev.source).toBe("env");
  });

  it("returns env_present=true with literal 'present' (never the raw value)", () => {
    process.env["ARBX_PROBE_TEST"] = "secret-rpc-url-with-key-1234567890";
    const ev = probeEnv("ARBX_PROBE_TEST");
    expect(ev.env_present).toBe(true);
    expect(ev.redacted_value).toBe("present");
    expect(ev.value_length).toBe("secret-rpc-url-with-key-1234567890".length);
    // CRITICAL: the raw value MUST NOT appear anywhere on the wire.
    expect(JSON.stringify(ev)).not.toContain("secret-rpc-url");
  });

  it("treats empty string as not present (length-zero is missing)", () => {
    process.env["ARBX_PROBE_TEST"] = "";
    const ev = probeEnv("ARBX_PROBE_TEST");
    expect(ev.env_present).toBe(false);
    expect(ev.redacted_value).toBe(null);
  });
});

describe("envBlockers", () => {
  it("emits rpc_http_1_missing when RPC_HTTP_1 absent", () => {
    delete process.env["RPC_HTTP_1"];
    const b = envBlockers();
    const rpc = b.find((x) => x.id === "rpc_http_1_missing");
    expect(rpc).toBeDefined();
    expect(rpc!.severity).toBe("critical");
    expect(rpc!.blocks).toContain("A.4");
    expect(rpc!.blocks).toContain("LIVE");
    expect(rpc!.operator_required).toBe(true);
  });

  it("emits executor_1_missing when EXECUTOR_1 absent", () => {
    delete process.env["EXECUTOR_1"];
    const b = envBlockers();
    const exec = b.find((x) => x.id === "executor_1_missing");
    expect(exec).toBeDefined();
    expect(exec!.severity).toBe("critical");
  });

  it("emits sim_orchestrator_gas_price_missing when var absent", () => {
    delete process.env["SIM_ORCHESTRATOR_GAS_PRICE_WEI"];
    const b = envBlockers();
    expect(b.find((x) => x.id === "sim_orchestrator_gas_price_missing")).toBeDefined();
  });

  it("does NOT emit arbx_trade_mode_not_paper when paper", () => {
    process.env["ARBX_TRADE_MODE"] = "paper";
    const b = envBlockers();
    expect(b.find((x) => x.id === "arbx_trade_mode_not_paper")).toBeUndefined();
  });

  it("DOES emit arbx_trade_mode_not_paper when value is 'live'", () => {
    process.env["ARBX_TRADE_MODE"] = "live";
    const b = envBlockers();
    const m = b.find((x) => x.id === "arbx_trade_mode_not_paper");
    expect(m).toBeDefined();
    expect(m!.severity).toBe("critical");
    expect(m!.blocks).toContain("LIVE");
  });

  it("does NOT emit arbx_trade_mode_not_paper when var is unset (paper is default)", () => {
    delete process.env["ARBX_TRADE_MODE"];
    const b = envBlockers();
    expect(b.find((x) => x.id === "arbx_trade_mode_not_paper")).toBeUndefined();
  });

  it("emits database_url_missing when DATABASE_URL absent", () => {
    delete process.env["DATABASE_URL"];
    const b = envBlockers();
    expect(b.find((x) => x.id === "database_url_missing")).toBeDefined();
  });

  it("regression: never embeds raw env values in blocker payload", () => {
    process.env["RPC_HTTP_1"] = "https://eth-mainnet.g.alchemy.com/v2/SECRET-API-KEY-1234";
    process.env["EXECUTOR_1"] = "0xDEADBEEF1234567890abcdefDEADBEEF12345678";
    process.env["DATABASE_URL"] = "postgres://user:supersecret@host:5432/db";
    const b = envBlockers();
    const serialized = JSON.stringify(b);
    expect(serialized).not.toContain("SECRET-API-KEY");
    expect(serialized).not.toContain("supersecret");
    expect(serialized).not.toContain("DEADBEEF1234567890");
  });
});

describe("doctrinalBlockers", () => {
  // A.4 (a4_fork_real_not_executed) was resolved 2026-08-20 — removed from
  // the blocker list after the canonical fork-validation pass (evidence:
  // gate_c_validation row a4_fork_validation_20260820T013304Z.log,
  // a4_state=A4_PASSED). See the readiness-extras.ts doctrinalBlockers
  // comment for the full evidence trail.
  it("emits exactly 5 doctrinal phase blockers (A.5..A.9) — A.4 resolved 2026-08-20", () => {
    const b = doctrinalBlockers();
    expect(b.length).toBe(5);
    const ids = b.map((x) => x.id).sort();
    expect(ids).toEqual([
      "a5_paper_shadow_not_executed",
      "a6_circuit_breakers_partial",
      "a7_private_relay_no_submit_pending",
      "a8_confidence_scoring_not_wired",
      "a9_go_no_go_formal_pending",
    ]);
  });

  it("A.4 blocker is gone (resolved via gate_c_validation evidence)", () => {
    const b = doctrinalBlockers();
    expect(b.find((x) => x.id === "a4_fork_real_not_executed")).toBeUndefined();
  });

  it("A.5 blocks A.5 + LIVE (NOT A.4)", () => {
    const b = doctrinalBlockers();
    const a5 = b.find((x) => x.id === "a5_paper_shadow_not_executed");
    expect(a5!.blocks).toContain("A.5");
    expect(a5!.blocks).toContain("LIVE");
    expect(a5!.blocks).not.toContain("A.4");
  });

  it("A.9 is critical severity", () => {
    const b = doctrinalBlockers();
    const a9 = b.find((x) => x.id === "a9_go_no_go_formal_pending");
    expect(a9!.severity).toBe("critical");
  });
});

describe("summarize", () => {
  it("counts severities and unions blocked phases", () => {
    const b = [
      { severity: "critical", blocks: ["A.4", "LIVE"] },
      { severity: "critical", blocks: ["LIVE"] },
      { severity: "high", blocks: ["A.5"] },
      { severity: "low", blocks: [] },
    ] as Parameters<typeof summarize>[0];
    const s = summarize(b);
    expect(s.critical).toBe(2);
    expect(s.high).toBe(1);
    expect(s.medium).toBe(0);
    expect(s.low).toBe(1);
    expect(s.blocked_phases.sort()).toEqual(["A.4", "A.5", "LIVE"]);
  });

  it("empty list yields zeros and empty phases", () => {
    const s = summarize([]);
    expect(s.critical).toBe(0);
    expect(s.high).toBe(0);
    expect(s.blocked_phases).toEqual([]);
  });
});

describe("overallStatus", () => {
  it("returns 'blocked' when any critical present", () => {
    expect(overallStatus({ critical: 1, high: 0, medium: 0, low: 0, blocked_phases: [] })).toBe("blocked");
  });
  it("returns 'partial' when no critical but high/medium/low present", () => {
    expect(overallStatus({ critical: 0, high: 1, medium: 0, low: 0, blocked_phases: [] })).toBe("partial");
    expect(overallStatus({ critical: 0, high: 0, medium: 2, low: 0, blocked_phases: [] })).toBe("partial");
    expect(overallStatus({ critical: 0, high: 0, medium: 0, low: 3, blocked_phases: [] })).toBe("partial");
  });
  it("returns 'ready' only when ALL counts are zero", () => {
    expect(overallStatus({ critical: 0, high: 0, medium: 0, low: 0, blocked_phases: [] })).toBe("ready");
  });
});
