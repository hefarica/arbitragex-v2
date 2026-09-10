// CB-04 (2026-09-07)
/**
 * control-board-drift — unit tests (WO CB-04, sovereign drift guard).
 *
 * Harness mirrors control-board.test.ts (CB-02): vitest + test doubles for
 * pool/redis — no real PG/Redis is touched. The fake pool implements the
 * EXACT `audit_log` contract CB-02-API pinned (migration 011 columns:
 * actor/action/target_kind/target_id/before_state/after_state — INSERT +
 * DISTINCT ON (target_id) SELECTs, and the compensatory
 * `control_board.toggle_failed` cancellation of the approvals query) and
 * keeps the inserted drift rows in an in-memory ledger so a second scan
 * sees its own history (dedup is tested against real ledger semantics, not
 * a stubbed map).
 *
 * Charter gates pinned here:
 *   (1)  THE REVERT FLOW — an external flip of a class-A key (operator
 *        approved ON, world shows OFF) is DETECTED, the exact diff lands in
 *        audit_log, and Redis is REVERTED to the operator-approved value.
 *        Demonstrated with fixtures only — never against production.
 *   (2)  foreign/deleted key with approval on record ⇒ drift + revert
 *        (observed is NEVER interpreted — null, with the reason saying so)
 *   (3)  sovereignty baseline: no board approval on record ⇒ NO drift
 *        claim, NO revert (nothing approved to defend — honest, not blind)
 *   (4)  class B drift ⇒ alert + audit WITHOUT revert (impossible without
 *        restart — honest, never simulated); non-boolean env ⇒ not comparable
 *   (5)  class C ⇒ not observable, never simulated, no audit
 *   (6)  dedup (R9): one audit row per NEW occurrence (fingerprint includes
 *        the revert outcome — a failed attempt then a successful retry each
 *        get their truthful row; a persisting drift never floods the ledger)
 *   (7)  census absent/corrupt, ledger unavailable ⇒ honest NON-computable
 *        statuses — never a fabricated "consistent" (RULE 00)
 *   (8)  guard interval + scanIfDue TTL throttle + stop()
 *   (9)  route wiring: /api/v1/control-board/drift (admin-gated, full diff)
 *        and the `drift` banner field inside the board snapshot (CB-03 wire)
 */
import { describe, it, expect, vi, afterEach } from "vitest";
import express, { type Express, type RequestHandler } from "express";
import request from "supertest";
import type pg from "pg";
import type { Redis } from "ioredis";
import {
  runControlBoardDriftScan,
  createControlBoardDriftGuard,
  toControlBoardDriftBanner,
  parseStrictBoolean,
  envKeyOfControlKey,
  CONTROL_BOARD_DRIFT_AUDIT_ACTION,
  CONTROL_BOARD_DRIFT_GUARD_ACTOR,
  CONTROL_BOARD_DRIFT_ENDPOINT_PATH,
  type ControlBoardDriftReport,
} from "./control-board-drift.js";
import {
  mountControlBoard,
  CONTROL_BOARD_CENSUS_REDIS_KEY,
  CONTROL_BOARD_TOGGLE_AUDIT_ACTION,
} from "../routes/control-board.js";

const ADMIN_TOKEN = "test-admin-token-32-bytes-of-entropy-aaaa";
const CLASS_A_KEY = "arbx:runtime:route_scanner";
const CLASS_B_ENV_VAR = "ARBX_CB04_TEST_FLAG"; // fixture-only env name (never a real knob)

/** Test-only requireAdminToken — mirrors @arbx/shared contract (CB-02 test). */
function requireAdminToken(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-admin-token");
    if (!got || got !== expected) {
      res.status(401).json({ error: "unauthorized", source: "admin_token" });
      return;
    }
    next();
  };
}

// ─── Census fixture (CB-01 conventions: A = bare Redis key, B = env:VAR) ─────

const CENSUS = {
  modules: [
    {
      id: "route_scanner_multihop",
      name: "route_scanner multihop (RU-3)",
      module_class: "A",
      control_key: CLASS_A_KEY,
      verified_on: true,
      verified_source: "log:route_scanner.mode",
    },
    {
      id: "scoring_hard_gate",
      name: "Scoring hard gate",
      module_class: "B",
      control_key: `env:${CLASS_B_ENV_VAR}`,
      declared_on: false,
      verified_on: false,
    },
    {
      id: "terminus_paper_live",
      name: "Terminus paper/live §34.3",
      module_class: "C",
      control_key: "operator-gate:§34.3",
    },
  ],
};

// ─── Harness (fake ledger keeps inserted drift rows → dedup sees history) ────

interface LedgerRow {
  target_id: string;
  payload: Record<string, unknown>;
}

interface HarnessOpts {
  census?: string | null; // raw census JSON | null (absent) | undefined (default fixture)
  classARaw?: string | null; // live value at CLASS_A_KEY; undefined = not set
  env?: Record<string, string | undefined>;
  /** chronological toggle attempts; `failed:true` simulates a toggle whose
   *  Redis SET failed — the fake's SELECT cancels it exactly like the real
   *  NOT EXISTS on the compensatory control_board.toggle_failed sibling;
   *  `afterState` overrides the row payload verbatim (garbage-payload tests) */
  approvedToggles?: Array<{
    target: string;
    on: boolean;
    failed?: boolean;
    afterState?: Record<string, unknown>;
  }>;
  ledgerRows?: LedgerRow[]; // pre-existing drift rows (fingerprints for dedup)
  poolNull?: boolean;
  failApprovedSelect?: boolean;
  failInsert?: boolean;
  failSet?: boolean;
  failRedisGet?: boolean;
}

interface Harness {
  poolQuery: ReturnType<typeof vi.fn>;
  redisGet: ReturnType<typeof vi.fn>;
  redisSet: ReturnType<typeof vi.fn>;
  store: Map<string, string>;
  ledgerRows: LedgerRow[];
  scan: () => Promise<ControlBoardDriftReport>;
}

function buildHarness(opts: HarnessOpts = {}): Harness {
  const store = new Map<string, string>();
  if (opts.census !== null) {
    store.set(CONTROL_BOARD_CENSUS_REDIS_KEY, opts.census ?? JSON.stringify(CENSUS));
  }
  if (opts.classARaw !== undefined && opts.classARaw !== null) {
    store.set(CLASS_A_KEY, opts.classARaw);
  }

  const redisGet = vi.fn(async (k: string) => {
    if (opts.failRedisGet) throw new Error("redis down");
    return store.get(k) ?? null;
  });
  const redisSet = vi.fn(async (k: string, v: string) => {
    if (opts.failSet) throw new Error("redis write refused");
    store.set(k, v);
    return "OK";
  });
  const redis = { get: redisGet, set: redisSet } as unknown as Redis;

  const ledgerRows: LedgerRow[] = opts.ledgerRows ? [...opts.ledgerRows] : [];

  const poolQuery = vi.fn(async (text: string, values: unknown[]) => {
    const t = String(text);
    if (t.startsWith("INSERT INTO audit_log")) {
      if (opts.failInsert) throw new Error("pg insert refused");
      // 011: (actor, action, target_kind, target_id, before_state, after_state)
      const payload = JSON.parse(String(values[4])) as Record<string, unknown>;
      ledgerRows.push({ target_id: String(values[2]), payload });
      return { rows: [{ id: ledgerRows.length }], rowCount: 1 };
    }
    if (t.startsWith("SELECT DISTINCT ON")) {
      const action = values[0];
      if (action === CONTROL_BOARD_TOGGLE_AUDIT_ACTION) {
        if (opts.failApprovedSelect) throw new Error("pg approved-select refused");
        // Simulates the REAL query (audit_log + NOT EXISTS compensatory
        // sibling): a `failed` attempt carries a control_board.toggle_failed
        // row with the same payload.at and is CANCELLED — only LANDED
        // approvals survive; DISTINCT ON keeps the LATEST per target_id.
        const attempts = (opts.approvedToggles ?? []).filter((a) => !a.failed);
        const latest = new Map<string, (typeof attempts)[number]>();
        for (const a of attempts) latest.set(a.target, a);
        const rows = [...latest.values()].map((a) => ({
          target_id: a.target,
          actor: "admin",
          after_state: a.afterState ?? { on: a.on, applied: true, reason: "fixture approval", at: "2026-09-07T12:00:00.000Z" },
          created_at: new Date("2026-09-07T12:00:00Z"),
        }));
        return { rows, rowCount: rows.length };
      }
      if (action === CONTROL_BOARD_DRIFT_AUDIT_ACTION) {
        // DISTINCT ON (target_id) … ORDER BY created_at DESC → last row per target_id
        const seen = new Map<string, LedgerRow>();
        for (const r of ledgerRows) seen.set(r.target_id, r);
        return {
          rows: [...seen.entries()].map(([target_id, r]) => ({
            target_id,
            actor: CONTROL_BOARD_DRIFT_GUARD_ACTOR,
            after_state: r.payload,
            created_at: new Date("2026-09-07T12:00:00Z"),
          })),
          rowCount: seen.size,
        };
      }
      throw new Error(`unexpected SELECT action in test: ${String(action)}`);
    }
    throw new Error(`unexpected sql in test: ${t.slice(0, 60)}`);
  });
  const pool = opts.poolNull ? null : ({ query: poolQuery } as unknown as pg.Pool);

  const logger = { warn: vi.fn(), info: vi.fn() };
  const scan = () =>
    runControlBoardDriftScan({ pool, redis, logger, env: opts.env ?? {} });

  return { poolQuery, redisGet, redisSet, store, ledgerRows, scan };
}

// ─── Unit: strict parsers ────────────────────────────────────────────────────

describe("CB-04 parseStrictBoolean / envKeyOfControlKey", () => {
  it("accepts only the unambiguous booleans every real Rust dialect agrees on", () => {
    expect(parseStrictBoolean("true")).toBe(true);
    expect(parseStrictBoolean("TRUE")).toBe(true);
    expect(parseStrictBoolean(" true ")).toBe(true);
    expect(parseStrictBoolean("false")).toBe(false);
    // "on"/"yes"/"1"/"0" DISAGREE between eq_ignore_ascii_case("true") and
    // env_bool("1"|"true"|"yes"|"on") — never claimed as booleans (RULE 00).
    expect(parseStrictBoolean("on")).toBeNull();
    expect(parseStrictBoolean("yes")).toBeNull();
    expect(parseStrictBoolean("1")).toBeNull();
    expect(parseStrictBoolean("0")).toBeNull();
    expect(parseStrictBoolean("v2")).toBeNull();
    expect(parseStrictBoolean(null)).toBeNull();
  });

  it("strips the env: convention of the CB-01 census (and rejects the rest)", () => {
    expect(envKeyOfControlKey("env:ARBX_ORCHESTRATOR_MODE")).toBe("ARBX_ORCHESTRATOR_MODE");
    expect(envKeyOfControlKey("env:")).toBeNull();
    expect(envKeyOfControlKey("arbx:runtime:route_scanner")).toBeNull();
    expect(envKeyOfControlKey(null)).toBeNull();
  });
});

// ─── The charter scan: approved vs observed, audit + revert ──────────────────

describe("CB-04 runControlBoardDriftScan — class A (sovereign revert)", () => {
  it("(1) THE CHARTER REVERT FLOW: external flip detected → exact-diff audit row → Redis REVERTED to the operator-approved value", async () => {
    const h = buildHarness({
      // operator approved ON through the board (CB-02 toggle, applied:true)
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      // …but the world shows OFF — someone flipped the key outside the board
      classARaw: "false",
    });
    const report = await h.scan();

    expect(report.status).toBe("drift_detected");
    const entry = report.modules.find((m) => m.id === "route_scanner_multihop")!;
    expect(entry.drift).toBe(true);
    expect(entry.approved_on).toBe(true);
    expect(entry.observed_on).toBe(false);
    expect(entry.observed_raw).toBe("false");
    expect(entry.reason).toBe("approved_vs_observed_mismatch");
    expect(entry.reverted).toBe(true);
    expect(entry.revert_applied).toBe(true);

    // THE REVERT: the exact class-A key rewritten to the APPROVED value,
    // in the board's strict dialect ("true"/"false" — control-board.ts PUT).
    expect(h.redisSet).toHaveBeenCalledWith(CLASS_A_KEY, "true");
    expect(h.store.get(CLASS_A_KEY)).toBe("true");

    // The exact-diff audit row: guard actor, drift action, revert outcome TRUE.
    expect(h.ledgerRows).toHaveLength(1);
    const row = h.ledgerRows[0]!;
    expect(row.target_id).toBe("route_scanner_multihop");
    const insertArgs = h.poolQuery.mock.calls.find(([t]) =>
      String(t).startsWith("INSERT INTO audit_log"),
    )!;
    // Migration 011 contract (CB-02-API-APPLY §5.1): audit_log + target_kind
    // 'control_board_module' + before/after_state JSONB — 5 bound values.
    expect(String(insertArgs[0])).toContain("target_kind");
    expect(insertArgs[1]).toEqual([
      CONTROL_BOARD_DRIFT_GUARD_ACTOR,
      CONTROL_BOARD_DRIFT_AUDIT_ACTION,
      "route_scanner_multihop",
      expect.any(String), // before_state {on: observed drifted state}
      expect.any(String), // after_state = the full exact diff payload
    ]);
    const beforeState = JSON.parse(String(insertArgs[1][3])) as { on: unknown };
    expect(beforeState.on).toBe(false); // the drifted state observed, honestly recorded
    expect(row.payload["approved_on"]).toBe(true);
    expect(row.payload["observed_on"]).toBe(false);
    expect(row.payload["observed_raw"]).toBe("false");
    expect(row.payload["revert_applied"]).toBe(true);
    expect(row.payload["wo"]).toBe("CB-04");

    // Ledger honesty: the revert ran BEFORE the audit row (so the row records
    // the true outcome, not a speculation).
    expect(h.redisSet.mock.invocationCallOrder[0]).toBeLessThan(
      h.poolQuery.mock.invocationCallOrder[2]!, // [0] approved SELECT, [1] dedup SELECT, [2] INSERT
    );
  });

  it("(2a) FOREIGN value at the control key (non-board dialect) with approval on record → observed null + reason observed_foreign_value + revert", async () => {
    const h = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "enabled=yes", // foreign writer — never interpreted
    });
    const report = await h.scan();
    const entry = report.modules.find((m) => m.id === "route_scanner_multihop")!;
    expect(entry.observed_on).toBeNull();
    expect(entry.observed_raw).toBe("enabled=yes");
    expect(entry.reason).toBe("observed_foreign_value");
    expect(entry.drift).toBe(true);
    expect(h.redisSet).toHaveBeenCalledWith(CLASS_A_KEY, "true");
  });

  it("(2b) control key DELETED with approval on record → observed_absent_with_approval + revert restores it", async () => {
    const h = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: null, // never set ⇒ key absent
    });
    const report = await h.scan();
    const entry = report.modules.find((m) => m.id === "route_scanner_multihop")!;
    expect(entry.observed_raw).toBeNull();
    expect(entry.reason).toBe("observed_absent_with_approval");
    expect(entry.drift).toBe(true);
    expect(h.redisSet).toHaveBeenCalledWith(CLASS_A_KEY, "true");
  });

  it("(4) consistent (approved == observed) → aligned, NO audit row, NO redis write", async () => {
    const h = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "true",
    });
    const report = await h.scan();
    expect(report.status).toBe("consistent");
    const entry = report.modules.find((m) => m.id === "route_scanner_multihop")!;
    expect(entry.drift).toBe(false);
    expect(entry.reason).toBe("aligned");
    expect(h.redisSet).not.toHaveBeenCalled();
    expect(h.ledgerRows).toHaveLength(0);
    expect(report.summary.drifts).toBe(0);
  });

  it("(3) sovereignty baseline: NO board approval on record → drift NOT claimed, NO revert (first operator toggle arms the guard)", async () => {
    const h = buildHarness({
      approvedToggles: [], // the operator never toggled this module
      classARaw: "false", // …and the world shows something — surfaced, not defended
    });
    const report = await h.scan();
    expect(report.status).toBe("consistent"); // no claims made ≠ green: see no_approval_record below
    const entry = report.modules.find((m) => m.id === "route_scanner_multihop")!;
    expect(entry.drift).toBe(false);
    expect(entry.reason).toBe("no_approval_record");
    expect(entry.observed_on).toBe(false); // observed surfaced honestly
    expect(h.redisSet).not.toHaveBeenCalled(); // nothing approved to revert to
    expect(h.ledgerRows).toHaveLength(0);
  });

  it("(6) revert FAILS then RETRIES: one row per truthful outcome (fingerprint includes revert result); persisting drift never floods the ledger (R9)", async () => {
    // Attempt 1: revert fails (redis refuses the write).
    const h1 = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "false",
      failSet: true,
    });
    const r1 = await h1.scan();
    const e1 = r1.modules.find((m) => m.id === "route_scanner_multihop")!;
    expect(e1.revert_applied).toBe(false);
    expect(r1.summary.revert_failed).toBe(1);
    expect(h1.ledgerRows).toHaveLength(1);
    expect(h1.ledgerRows[0]!.payload["revert_applied"]).toBe(false);

    // Attempt 2: same failure, same fingerprint ⇒ dedup — NO new ledger row.
    const h2 = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "false",
      failSet: true,
      ledgerRows: h1.ledgerRows, // the ledger carries attempt 1's row
    });
    await h2.scan();
    expect(h2.ledgerRows).toHaveLength(1); // dedup held — no flood

    // Attempt 3: revert LANDS ⇒ different fingerprint ⇒ its own truthful row.
    const h3 = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "false",
      ledgerRows: h1.ledgerRows,
    });
    const r3 = await h3.scan();
    expect(h3.redisSet).toHaveBeenCalledWith(CLASS_A_KEY, "true");
    expect(h3.ledgerRows).toHaveLength(2);
    expect(h3.ledgerRows[1]!.payload["revert_applied"]).toBe(true);
    expect(r3.summary.reverted).toBe(1);
  });

  it("(10) CB-02-API §5.1/R1: a toggle whose SET failed (compensatory control_board.toggle_failed sibling) is NOT an approval — the LAST LANDED approval is defended", async () => {
    // (a) aligned: Redis still holds the landed approval; the failed OFF
    //     attempt never landed and must NOT read as "approved OFF".
    const ha = buildHarness({
      approvedToggles: [
        { target: "route_scanner_multihop", on: true }, // landed (older)
        { target: "route_scanner_multihop", on: false, failed: true }, // SET failed ⇒ compensatory row
      ],
      classARaw: "true",
    });
    const ra = await ha.scan();
    const ea = ra.modules.find((m) => m.id === "route_scanner_multihop")!;
    expect(ea.approved_on).toBe(true); // the LANDED approval, not the failed attempt
    expect(ea.drift).toBe(false);
    expect(ea.reason).toBe("aligned");
    expect(ha.redisSet).not.toHaveBeenCalled();
    expect(ha.ledgerRows).toHaveLength(0);

    // (b) drift AFTER the failed attempt: an external actor flips the key —
    //     the guard defends the last LANDED approval (ON), never the failed OFF.
    const hb = buildHarness({
      approvedToggles: [
        { target: "route_scanner_multihop", on: true },
        { target: "route_scanner_multihop", on: false, failed: true },
      ],
      classARaw: "false",
    });
    const rb = await hb.scan();
    const eb = rb.modules.find((m) => m.id === "route_scanner_multihop")!;
    expect(eb.approved_on).toBe(true);
    expect(eb.reason).toBe("approved_vs_observed_mismatch");
    expect(hb.redisSet).toHaveBeenCalledWith(CLASS_A_KEY, "true"); // sovereign revert to LANDED approval
  });

  it("(11) garbage after_state (no explicit boolean on) is NO approval — RULE 00: the module stays unarmed, never 'approved false'", async () => {
    const h = buildHarness({
      approvedToggles: [
        {
          target: "route_scanner_multihop",
          on: true, // ignored — afterState overrides the payload verbatim
          afterState: { module: "route_scanner_multihop", note: "foreign row without a boolean on" },
        },
      ],
      classARaw: "false",
    });
    const report = await h.scan();
    const entry = report.modules.find((m) => m.id === "route_scanner_multihop")!;
    expect(entry.approved_on).toBeNull();
    expect(entry.reason).toBe("no_approval_record"); // honest: not armed, not misread
    expect(entry.drift).toBe(false);
    expect(h.redisSet).not.toHaveBeenCalled(); // nothing approved to defend
    expect(h.ledgerRows).toHaveLength(0);
  });
});

describe("CB-04 runControlBoardDriftScan — class B (alert-only, never revert)", () => {
  it("(4) env drift vs census declaration → drift + audit WITHOUT any redis write (impossible without restart — honest)", async () => {
    const h = buildHarness({
      // census declares the operator deployed this OFF…
      env: { [CLASS_B_ENV_VAR]: "true" }, // …but the booted env says ON (env drift / deploy)
      approvedToggles: [], // class A silent
    });
    const report = await h.scan();
    const entry = report.modules.find((m) => m.id === "scoring_hard_gate")!;
    expect(entry.drift).toBe(true);
    expect(entry.reason).toBe("env_differs_from_declared");
    expect(entry.approved_on).toBe(false); // census declared_on
    expect(entry.observed_on).toBe(true); // process env (injected fixture)
    expect(entry.observed_raw).toBe("true");
    expect(entry.observed_source).toBe("env");
    // NEVER a revert for class B — remediation is restart-gated (CB-05 flow).
    expect(h.redisSet).not.toHaveBeenCalled();
    expect(h.ledgerRows).toHaveLength(1);
    expect(h.ledgerRows[0]!.payload["revert_applied"]).toBeNull();
  });

  it("(8) non-boolean env value (mode string) → not_comparable, NO drift claim (RULE 00: no invented truthiness)", async () => {
    const h = buildHarness({
      env: { [CLASS_B_ENV_VAR]: "v2" }, // a mode — not an unambiguous boolean
    });
    const report = await h.scan();
    const entry = report.modules.find((m) => m.id === "scoring_hard_gate")!;
    expect(entry.drift).toBe(false);
    expect(entry.reason).toBe("not_comparable_non_boolean_env");
    expect(entry.observed_on).toBeNull();
    expect(entry.observed_raw).toBe("v2"); // surfaced verbatim, never interpreted
    expect(h.ledgerRows).toHaveLength(0);
    expect(report.summary.not_comparable).toBe(1);
  });

  it("(9) class C (§34.3) → not observable, never simulated, no audit, no write", async () => {
    const h = buildHarness({});
    const report = await h.scan();
    const entry = report.modules.find((m) => m.id === "terminus_paper_live")!;
    expect(entry.drift).toBe(false);
    expect(entry.reason).toBe("not_observable_class_c");
    expect(entry.observed_on).toBeNull();
    expect(entry.observed_source).toBeNull();
    expect(h.ledgerRows).toHaveLength(0);
    expect(h.redisSet).not.toHaveBeenCalled();
    expect(report.summary.class_c_skipped).toBe(1);
  });
});

describe("CB-04 runControlBoardDriftScan — fail-honest statuses (RULE 00)", () => {
  it("(7a) census absent → census_absent, ZERO pool calls, ZERO writes (same honesty as CB-02's empty snapshot)", async () => {
    const h = buildHarness({ census: null });
    const report = await h.scan();
    expect(report.status).toBe("census_absent");
    expect(report.modules).toEqual([]);
    expect(h.poolQuery).not.toHaveBeenCalled();
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(7b) census corrupt → census_unreadable (loud, never partial)", async () => {
    const h = buildHarness({ census: "{not json" });
    const report = await h.scan();
    expect(report.status).toBe("census_unreadable");
    expect(report.modules).toEqual([]);
  });

  it("(7c) pool null → audit_unavailable: without the ledger there is NO approved state — no claims, never a fabricated consistent", async () => {
    const h = buildHarness({ poolNull: true });
    const report = await h.scan();
    expect(report.status).toBe("audit_unavailable");
    expect(report.modules).toEqual([]);
  });

  it("(7d) approved-ledger SELECT fails → audit_unavailable, no writes", async () => {
    const h = buildHarness({ failApprovedSelect: true });
    const report = await h.scan();
    expect(report.status).toBe("audit_unavailable");
    expect(h.redisSet).not.toHaveBeenCalled();
  });

  it("(7e) census defect: class A sin control_key → census_invalid_control_key, no write (mirrors CB-02 PUT 503)", async () => {
    const census = JSON.stringify({
      modules: [
        { id: "broken_class_a", name: "A sin control_key", module_class: "A" },
      ],
    });
    const h = buildHarness({ census });
    const report = await h.scan();
    expect(report.modules[0]!.reason).toBe("census_invalid_control_key");
    expect(report.modules[0]!.drift).toBe(false);
    expect(h.redisSet).not.toHaveBeenCalled();
  });
});

// ─── Banner mapping (CB-03 wire: ControlBoardDriftSchema) ────────────────────

describe("CB-04 toControlBoardDriftBanner", () => {
  it("drift_detected → detected:true with human summary + the diff endpoint href", async () => {
    const h = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "false",
    });
    const report = await h.scan();
    const banner = toControlBoardDriftBanner(report);
    expect(banner.detected).toBe(true);
    expect(banner.checked_at).toBe(report.checked_at);
    expect(banner.summary).toContain("revertido");
    expect(banner.diff_href).toBe(CONTROL_BOARD_DRIFT_ENDPOINT_PATH);
  });

  it("consistent → detected:false with the comparison count", async () => {
    const h = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "true",
    });
    const banner = toControlBoardDriftBanner(await h.scan());
    expect(banner.detected).toBe(false);
    expect(banner.summary).toContain("sin drift");
  });

  it("NON-computable statuses are NEVER green claims — the summary says NO COMPUTABLE", async () => {
    const h = buildHarness({ census: null });
    const banner = toControlBoardDriftBanner(await h.scan());
    expect(banner.detected).toBe(false);
    expect(banner.summary).toContain("NO computable");
  });
});

// ─── Guard: interval + TTL throttle + stop ───────────────────────────────────

describe("CB-04 createControlBoardDriftGuard", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("(8a) interval tick fires the scan (periodic driver — unref'd, fully caught)", async () => {
    vi.useFakeTimers();
    const h = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "false",
    });
    const guard = createControlBoardDriftGuard(
      { pool: h.poolQuery ? ({ query: h.poolQuery } as unknown as pg.Pool) : null, redis: { get: h.redisGet, set: h.redisSet } as unknown as Redis, logger: { warn: vi.fn(), info: vi.fn() }, env: {} },
      { intervalMs: 1_000 },
    );
    expect(guard.getLastReport()).toBeNull(); // interval-only start: no IO at mount
    await vi.advanceTimersByTimeAsync(1_000);
    expect(guard.getLastReport()).not.toBeNull();
    expect(guard.getLastReport()!.status).toBe("drift_detected");
    expect(h.redisSet).toHaveBeenCalledWith(CLASS_A_KEY, "true"); // revert ran on tick
    guard.stop();
  });

  it("(8b) scanIfDue TTL: a second call inside the window returns the cached report WITHOUT rescanning", async () => {
    vi.useFakeTimers();
    const h = buildHarness({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "true",
    });
    const guard = createControlBoardDriftGuard(
      { pool: { query: h.poolQuery } as unknown as pg.Pool, redis: { get: h.redisGet, set: h.redisSet } as unknown as Redis, logger: { warn: vi.fn(), info: vi.fn() }, env: {} },
      { intervalMs: 3_600_000, minScanIntervalMs: 10_000 },
    );
    const first = await guard.scanIfDue();
    expect(first).not.toBeNull();
    const selectsAfterFirst = h.poolQuery.mock.calls.filter(([t]) =>
      String(t).startsWith("SELECT DISTINCT ON"),
    ).length;
    const second = await guard.scanIfDue(); // within 10s TTL (fake clock frozen)
    expect(second).toBe(first);
    const selectsAfterSecond = h.poolQuery.mock.calls.filter(([t]) =>
      String(t).startsWith("SELECT DISTINCT ON"),
    ).length;
    expect(selectsAfterSecond).toBe(selectsAfterFirst); // no rescan — no stampede
    guard.stop();
  });

  it("(8c) stop() kills the interval (no scan after stop)", async () => {
    vi.useFakeTimers();
    const h = buildHarness({});
    const guard = createControlBoardDriftGuard(
      { pool: { query: h.poolQuery } as unknown as pg.Pool, redis: { get: h.redisGet, set: h.redisSet } as unknown as Redis, logger: { warn: vi.fn(), info: vi.fn() }, env: {} },
      { intervalMs: 500 },
    );
    guard.stop();
    await vi.advanceTimersByTimeAsync(2_000);
    expect(guard.getLastReport()).toBeNull();
  });
});

// ─── Route wiring: diff endpoint + banner field in the snapshot ──────────────

const AUTH = ["x-arbx-admin-token", ADMIN_TOKEN] as const;

function buildApp(opts: HarnessOpts = {}): {
  app: Express;
  harness: Harness;
  mounted: ReturnType<typeof mountControlBoard>;
} {
  const h = buildHarness(opts);
  const app = express();
  app.use(express.json());
  const mounted = mountControlBoard(app, {
    pool: opts.poolNull ? null : ({ query: h.poolQuery } as unknown as pg.Pool),
    redis: { get: h.redisGet, set: h.redisSet } as unknown as Redis,
    requireAdminToken,
    adminToken: ADMIN_TOKEN,
    logger: { warn: vi.fn(), info: vi.fn() },
    driftGuardIntervalMs: 3_600_000, // interval never fires inside a test
  });
  return { app, harness: h, mounted };
}

describe("CB-04 route wiring (mountControlBoard additive extension)", () => {
  it("(9a) GET /api/v1/control-board/drift without admin token → 401", async () => {
    const { app } = buildApp({});
    const res = await request(app).get(CONTROL_BOARD_DRIFT_ENDPOINT_PATH);
    expect(res.status).toBe(401);
  });

  it("(9b) GET /api/v1/control-board/drift → 200 full diff report (on-demand scanIfDue)", async () => {
    const { app, harness } = buildApp({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "false", // drift present
    });
    const res = await request(app).get(CONTROL_BOARD_DRIFT_ENDPOINT_PATH).set(...AUTH);
    expect(res.status).toBe(200);
    expect(res.body.status).toBe("drift_detected");
    expect(res.body.checked_at).toEqual(expect.any(String));
    expect(res.body.scan_id).toEqual(expect.any(String));
    const entry = (res.body.modules as Array<Record<string, unknown>>).find(
      (m) => m["id"] === "route_scanner_multihop",
    )!;
    expect(entry["drift"]).toBe(true);
    expect(entry["approved_on"]).toBe(true);
    expect(entry["observed_on"]).toBe(false);
    expect(res.body.summary.drifts).toBe(1);
    expect(res.body.summary.reverted).toBe(1);
    // The scan REVERTED through the endpoint-triggered path too.
    expect(harness.redisSet).toHaveBeenCalledWith(CLASS_A_KEY, "true");
  });

  it("(9c) board snapshot carries the CB-03 banner field: null before the first scan, populated after the diff endpoint ran", async () => {
    const { app } = buildApp({
      approvedToggles: [{ target: "route_scanner_multihop", on: true }],
      classARaw: "false",
    });

    // Before any scan: honest absence (CB-03 renders "sin dato", never green).
    const before = await request(app).get("/api/v1/control-board").set(...AUTH);
    expect(before.status).toBe(200);
    expect(before.body.drift).toBeNull();

    // Trigger the first scan via the diff endpoint…
    await request(app).get(CONTROL_BOARD_DRIFT_ENDPOINT_PATH).set(...AUTH);

    // …and the banner rides the snapshot (exact CB-03 wire keys).
    const after = await request(app).get("/api/v1/control-board").set(...AUTH);
    expect(after.status).toBe(200);
    const drift = after.body.drift as Record<string, unknown>;
    expect(drift["detected"]).toBe(true);
    expect(drift["checked_at"]).toEqual(expect.any(String));
    expect(drift["summary"]).toEqual(expect.any(String));
    expect(drift["diff_href"]).toBe(CONTROL_BOARD_DRIFT_ENDPOINT_PATH);
  });

  it("(9d) mountControlBoard hands back a drift-guard shutdown handle", async () => {
    const { mounted } = buildApp({});
    expect(typeof mounted.stopDriftGuard).toBe("function");
    expect(() => mounted.stopDriftGuard()).not.toThrow();
  });
});
