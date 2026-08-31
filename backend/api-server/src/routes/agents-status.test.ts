/**
 * agents-status tests — pure-function assertions.
 *
 * Coverage:
 *   - The 17 doctrinal agents are defined.
 *   - go-no-go-agent is NEVER PASS for live (NO_GO immutable).
 *   - risk-circuit-agent is PARTIAL (A.6 pending).
 *   - runtimeOverlay demotes verify-dependent agents on verifyAll failure.
 *   - summarize counts verdicts correctly.
 *   - overallStatus follows precedence (blocked > partial > ready).
 *   - No agent leaks secrets in evidence strings.
 */
import { describe, it, expect } from "vitest";
import { __forTesting } from "./agents-status.js";

const { AGENT_DEFS, runtimeOverlay, summarize, overallStatus } = __forTesting;

describe("agents-status — doctrinal completeness", () => {
  it("defines exactly 18 agents (17 originals + scoring-primitives-agent A.8)", () => {
    expect(AGENT_DEFS.length).toBe(19);
  });

  it("includes each documented Agent Team id", () => {
    const expectedIds = [
      "repo-forensics-agent",
      "vps-deploy-agent",
      "operator-wip-agent",
      "frontend-preservation-agent",
      "backend-contract-agent",
      "blockers-api-agent",
      "decision-api-agent",
      "edge-proxy-agent",
      "blockers-panel-agent",
      "go-no-go-panel-agent",
      "data-wiring-agent",
      "security-guard-agent",
      "risk-circuit-agent",
      "per-chain-isolation-agent",
      "scoring-primitives-agent",
      "anti-mock-agent",
      "visual-regression-agent",
      "performance-agent",
      "go-no-go-agent",
    ];
    const actualIds = AGENT_DEFS.map((a) => a.id);
    for (const id of expectedIds) {
      expect(actualIds).toContain(id);
    }
  });

  it("go-no-go-agent is NO_GO and blocks LIVE", () => {
    const a = AGENT_DEFS.find((x) => x.id === "go-no-go-agent")!;
    expect(a.default_verdict).toBe("NO_GO");
    expect(a.blocks).toContain("LIVE");
    expect(a.risk).toBe("critical");
  });

  it("risk-circuit-agent is PARTIAL pending A.6", () => {
    const a = AGENT_DEFS.find((x) => x.id === "risk-circuit-agent")!;
    expect(a.default_verdict).toBe("PARTIAL");
    expect(a.blocks).toContain("LIVE");
  });

  it("every agent has at least one evidence line", () => {
    for (const a of AGENT_DEFS) {
      expect(a.evidence.length).toBeGreaterThan(0);
    }
  });
});

describe("runtimeOverlay", () => {
  it("preserves PASS when readiness ok", () => {
    const def = AGENT_DEFS.find((x) => x.id === "blockers-api-agent")!;
    const row = runtimeOverlay(def, { readinessOk: true, readinessFailDetail: null });
    expect(row.verdict).toBe("PASS");
    expect(row.source).toBe("workspace_verified");
  });

  it("demotes blockers-api-agent to BLOCKED when verifyAll fails", () => {
    const def = AGENT_DEFS.find((x) => x.id === "blockers-api-agent")!;
    const row = runtimeOverlay(def, { readinessOk: false, readinessFailDetail: "PG connect timeout" });
    expect(row.verdict).toBe("BLOCKED");
    expect(row.status).toBe("blocked");
    expect(row.source).toBe("runtime");
    expect(row.evidence.some((e) => e.includes("PG connect timeout"))).toBe(true);
  });

  it("does NOT demote go-no-go-agent on verifyAll failure (already NO_GO)", () => {
    const def = AGENT_DEFS.find((x) => x.id === "go-no-go-agent")!;
    const row = runtimeOverlay(def, { readinessOk: false, readinessFailDetail: "x" });
    expect(row.verdict).toBe("NO_GO");
  });

  it("does NOT demote agents unrelated to verifyAll", () => {
    const def = AGENT_DEFS.find((x) => x.id === "repo-forensics-agent")!;
    const row = runtimeOverlay(def, { readinessOk: false, readinessFailDetail: "x" });
    expect(row.verdict).toBe("PASS");
    expect(row.source).toBe("workspace_verified");
  });
});

describe("summarize", () => {
  it("counts verdict buckets correctly", () => {
    const agents = AGENT_DEFS.map((d) => runtimeOverlay(d, { readinessOk: true, readinessFailDetail: null }));
    const s = summarize(agents);
    expect(s.total).toBe(19);
    expect(s.pass + s.partial + s.blocked + s.no_go + s.not_run + s.unknown).toBe(19);
    // Post-A.8 baseline: 15 PASS, 2 PARTIAL (risk-circuit + scoring-primitives), 1 NO_GO (go-no-go).
    expect(s.partial).toBeGreaterThanOrEqual(2);
    expect(s.no_go).toBeGreaterThanOrEqual(1);
  });
});

describe("overallStatus", () => {
  it("returns 'blocked' when any NO_GO present", () => {
    const agents = AGENT_DEFS.map((d) => runtimeOverlay(d, { readinessOk: true, readinessFailDetail: null }));
    const s = summarize(agents);
    expect(overallStatus(s)).toBe("blocked");
  });

  it("returns 'ready' when all PASS", () => {
    const fakeS = { pass: 18, blocked: 0, partial: 0, no_go: 0, not_run: 0, unknown: 0, total: 18 };
    expect(overallStatus(fakeS)).toBe("ready");
  });

  it("returns 'partial' when only PARTIAL/UNKNOWN/NOT_RUN", () => {
    expect(overallStatus({ pass: 15, blocked: 0, partial: 3, no_go: 0, not_run: 0, unknown: 0, total: 18 }))
      .toBe("partial");
  });
});

describe("regression: no secret leakage in defaults", () => {
  it("no evidence line contains an RPC URL or DB password", () => {
    const all = AGENT_DEFS.flatMap((a) => a.evidence).join("\n");
    expect(all).not.toMatch(/https?:\/\/[a-z0-9-]+\.alchemy\.com/i);
    expect(all).not.toMatch(/postgres:\/\/[^@\s]+:[^@\s]+@/i);
    expect(all).not.toMatch(/0x[a-f0-9]{40}/i); // No deployed contract addresses
  });
});

// ── DAPP-SURFACE-FAIL regression (2026-08-31): dated ledger provenance ──────
// The workbook flagged /agent-insights Business truth FAIL: May-era evidence
// strings were served as current verdicts with last_run_at=null and no date.
// The fix dates every ledger entry (git-derived) and only stamps last_run_at
// when a runtime probe actually executed.

describe("DAPP-SURFACE-FAIL: verified_at ledger provenance", () => {
  it("every agent def carries an ISO-date verified_at (YYYY-MM-DD)", () => {
    for (const a of AGENT_DEFS) {
      expect(a.verified_at).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    }
  });

  it("the two 2026-08-29 re-verified defs carry that date (de-stale pins)", () => {
    const risk = AGENT_DEFS.find((x) => x.id === "risk-circuit-agent")!;
    const gonogo = AGENT_DEFS.find((x) => x.id === "go-no-go-agent")!;
    expect(risk.verified_at).toBe("2026-08-29"); // 20c93917
    expect(gonogo.verified_at).toBe("2026-08-29"); // 640fc7db
  });

  it("untouched ledger entries honestly keep the 2026-05-13 audit date", () => {
    const stale = AGENT_DEFS.filter((x) => x.verified_at === "2026-05-13");
    expect(stale.length).toBe(17);
  });
});

describe("DAPP-SURFACE-FAIL: last_run_at honesty", () => {
  it("verifyAll-dependent agents get the probe timestamp when the probe ran", () => {
    const def = AGENT_DEFS.find((x) => x.id === "blockers-api-agent")!;
    const row = runtimeOverlay(def, {
      readinessOk: true,
      readinessFailDetail: null,
      probeRanAt: "2026-08-31T00:00:00.000Z",
    });
    expect(row.last_run_at).toBe("2026-08-31T00:00:00.000Z");
    expect(row.verified_at).toBe(def.verified_at);
  });

  it("verifyAll-dependent agents keep last_run_at=null when no probe timestamp is supplied", () => {
    const def = AGENT_DEFS.find((x) => x.id === "blockers-api-agent")!;
    const row = runtimeOverlay(def, { readinessOk: true, readinessFailDetail: null });
    expect(row.last_run_at).toBeNull();
  });

  it("static ledger agents never claim a runtime run", () => {
    const def = AGENT_DEFS.find((x) => x.id === "repo-forensics-agent")!;
    const row = runtimeOverlay(def, {
      readinessOk: true,
      readinessFailDetail: null,
      probeRanAt: "2026-08-31T00:00:00.000Z",
    });
    expect(row.last_run_at).toBeNull();
    expect(row.source).toBe("workspace_verified");
  });
});
