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
 *   - composeNextAction (WO H-1, 2026-09-07): next_action is derived from the
 *     LIVE blockers (severity-ordered, A.9 sign-off terminal) and never
 *     claims "All known blockers cleared" while blockers exist.
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { __forTesting } from "./readiness-extras.js";

const { probeEnv, envBlockers, doctrinalBlockers, readinessItemsToBlockers, summarize, overallStatus, composeNextAction } =
  __forTesting;

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
  //
  // A.8 (a8_confidence_scoring_not_wired) was resolved 2026-08-29 — removed
  // after PR #470 wired score_and_publish to ConfidenceScore on every paper
  // opportunity (prod: XLEN arbx:scoring:scored=87, last-hour rows rejected|87,
  // scored_opportunities_total=90). A.7 flipped pending→partial the same date
  // (module shipped in relays-client, runtime call-site pending).
  // A.5 (a5_paper_shadow_not_executed) was resolved 2026-08-29 — removed
  // after the A5-STALL closure (kill-switch fail-closed default after Redis
  // key loss + anvil fork 403s; fixed via canonical admin re-arm, alchemy
  // fork URL, Redis AOF, halt logs, G-PIPE-1 gate; prod: selector consuming,
  // validated flowing, G-PAP-1 green explicit). See the readiness-extras.ts
  // comment above the array for the full evidence trail including the honest
  // S4 follow-up (0 of 639,955 sims ever passed — probe token-funding gap).
  //
  // A.6 (a6_circuit_breakers_partial) was resolved 2026-09-07 — removed after
  // PR #542 (arbx_risk_cb_* emission + circuit_breakers alert group) + PR #548
  // (job-scope L4 fix). L4 prod: 10 series live, HardDown NOT firing with
  // breakers honestly at NOT_AVAILABLE(5), loop freshness ~49s, group loaded
  // in Prometheus, behaviour pinned by monitoring/tests/risk_cb_test.yml.
  // A.7 (a7_private_relay_no_submit_partial) was resolved 2026-09-07 — removed
  // after PR #543 wired relay_no_submit_sim into submit_engine step 4.5
  // (pre-egress; paper=LogOnly, non-paper 0/3=drop fail-closed; 77/77 tests;
  // M1 barrier untouched).
  it("emits exactly 1 doctrinal phase blocker (A.9) — A.4 resolved 2026-08-20, A.8+A.5 2026-08-29, A.6+A.7 2026-09-07", () => {
    const b = doctrinalBlockers();
    expect(b.length).toBe(1);
    const ids = b.map((x) => x.id).sort();
    expect(ids).toEqual(["a9_go_no_go_formal_pending"]);
  });

  it("A.6 blocker is gone (resolved 2026-09-07 via A6-CBPROM-01 + L4 job-scope fix)", () => {
    const b = doctrinalBlockers();
    expect(b.find((x) => x.id === "a6_circuit_breakers_partial")).toBeUndefined();
  });

  it("A.7 blocker is gone (resolved 2026-09-07 via A7-RELAYSIM-CALLSITE-01)", () => {
    const b = doctrinalBlockers();
    expect(b.find((x) => x.id === "a7_private_relay_no_submit_partial")).toBeUndefined();
  });

  it("A.4 blocker is gone (resolved via gate_c_validation evidence)", () => {
    const b = doctrinalBlockers();
    expect(b.find((x) => x.id === "a4_fork_real_not_executed")).toBeUndefined();
  });

  it("A.5 blocker is gone (resolved 2026-08-29 via A5-STALL closure)", () => {
    const b = doctrinalBlockers();
    expect(b.find((x) => x.id === "a5_paper_shadow_not_executed")).toBeUndefined();
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

describe("composeNextAction (WO H-1, 2026-09-07 — live blockers drive next_action)", () => {
  // composeNextAction only reads id/severity/required_action — a narrow
  // factory keeps these tests about the composition contract, not the full
  // Blocker wire shape.
  type NextActionBlocker = Parameters<typeof composeNextAction>[0][number];
  const mk = (
    id: string,
    severity: "critical" | "high" | "medium" | "low",
    required_action = `Resolve readiness item ${id}.`,
  ): NextActionBlocker => ({ id, severity, required_action }) as NextActionBlocker;

  it("regression H-1: G-PIPE-1 (critical) + G-DISK-1 (high) + A.9 pending — no false 'All known blockers cleared'", () => {
    // Reproduces the live /live-readiness state the Browse Auditor saw
    // 2026-09-09: critical G-PIPE-1, high G-DISK-1, A.9 pending — while the
    // old next_action claimed "All known blockers cleared at this layer".
    const blockers = [
      mk("readiness_g_disk_1", "high", "Resolve readiness item G-DISK-1 (Host disk usage below critical threshold)."),
      mk("readiness_g_pipe_1", "critical", "Resolve readiness item G-PIPE-1 (Paper pipeline stream flow (detected→validated→simulated))."),
      mk("a9_go_no_go_formal_pending", "critical", "Generate the formal GO/NO-GO ledger."),
    ];
    const s = composeNextAction(blockers);
    expect(s).not.toContain("All known blockers cleared");
    // severity ordering: critical G-PIPE-1 lands before high G-DISK-1
    expect(s.indexOf("readiness_g_pipe_1")).toBeGreaterThan(-1);
    expect(s.indexOf("readiness_g_pipe_1")).toBeLessThan(s.indexOf("readiness_g_disk_1"));
    // each surfaced blocker carries its severity + required action
    expect(s).toContain("readiness_g_pipe_1 [critical]");
    expect(s).toContain("Resolve readiness item G-PIPE-1");
    // the terminal step is the A.9 sign-off
    expect(s.endsWith("Await A.9 formal GO/NO-GO sign-off.")).toBe(true);
  });

  it("consumes the exact id mapping produced by readinessItemsToBlockers (guards id drift)", () => {
    const items = [
      {
        id: "G-PIPE-1",
        group: "operations",
        label: "Paper pipeline stream flow (detected→validated→simulated)",
        status: "red",
        reason: "selector consumer stalled: 500 entries behind on arbx:opps:detected (deliverable, ≥500)",
        verified_at: "2026-09-07T00:00:00.000Z",
      },
    ] as Parameters<typeof readinessItemsToBlockers>[0];
    const blockers = readinessItemsToBlockers(items);
    const s = composeNextAction(blockers);
    expect(s).toContain("readiness_g_pipe_1");
    expect(s).toContain("Resolve readiness item G-PIPE-1");
  });

  it("keeps the specific A.4 runbook FIRST when the A.4 blocker is present", () => {
    const blockers = [
      mk("readiness_g_pipe_1", "critical"),
      mk("a4_fork_real_not_executed", "critical"),
    ];
    const s = composeNextAction(blockers);
    expect(s.startsWith("Provide RPC_HTTP_1 + EXECUTOR_1")).toBe(true);
    expect(s).toContain("readiness_g_pipe_1");
  });

  it("A.9-only state names the pending sign-off instead of a false all-clear", () => {
    const s = composeNextAction([mk("a9_go_no_go_formal_pending", "critical")]);
    expect(s).toBe("Await A.9 formal GO/NO-GO sign-off.");
    expect(s).not.toContain("All known blockers cleared");
  });

  it("caps derived blocker parts at 3 — lower severities drop first", () => {
    const blockers = [
      mk("b_low_1", "low"),
      mk("b_high_1", "high"),
      mk("b_crit_1", "critical"),
      mk("b_med_1", "medium"),
      mk("b_low_2", "low"),
    ];
    const s = composeNextAction(blockers);
    expect(s).toContain("b_crit_1");
    expect(s).toContain("b_high_1");
    expect(s).toContain("b_med_1");
    expect(s).not.toContain("b_low_1");
    expect(s).not.toContain("b_low_2");
  });

  it("empty list keeps the honest default (all-clear stated ONLY when nothing blocks)", () => {
    expect(composeNextAction([])).toBe(
      "All known blockers cleared at this layer; await A.9 formal GO/NO-GO sign-off.",
    );
  });

  it("does not mutate the input blockers array (stable sort on a copy)", () => {
    const blockers = [
      mk("b_low_1", "low"),
      mk("b_crit_1", "critical"),
    ];
    const idsBefore = blockers.map((b) => b.id).join(",");
    composeNextAction(blockers);
    expect(blockers.map((b) => b.id).join(",")).toBe(idsBefore);
  });
});
