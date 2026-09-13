// CB-04 (2026-09-07)
/**
 * control-board-drift — SOVEREIGN drift guard (WO CB-04 + operator order
 * "ni Dios mueva si no es mi deseo", GOAL-WORKORDERS.md:47).
 *
 * Semantics: the control board (CB-02) is the OPERATOR'S APPROVAL REGISTRY.
 * This guard periodically compares, per module:
 *
 *   APPROVED — the last APPROVED board write for the module:
 *              the latest `control_board.toggle` row in `audit_log`
 *              (SINGULAR — migration 011, CB-02-DISENO §15-R1) that is NOT
 *              cancelled by a compensatory `control_board.toggle_failed`
 *              row of the same attempt. CB-02 INSERTs the toggle row BEFORE
 *              the Redis SET; if the SET fails it INSERTs a compensatory row
 *              carrying the same payload `at` (applied:false + redis_error)
 *              — the ledger is append-only (011:21 REVOKE UPDATE, DELETE),
 *              so a failed attempt is recognised by its compensatory
 *              sibling, never by mutating the original row. A failed
 *              attempt therefore falls back to the PREVIOUS landed
 *              approval (the operator's last effective wish). Only rows
 *              whose after_state carries an explicit boolean `on` count as
 *              an approval (RULE 00 — a garbage/foreign payload is no
 *              approval; the module stays unarmed). Honest limitation: if
 *              CB-02's compensatory INSERT itself failed (logged loudly as
 *              control_board.audit_compensate_failed), the attempt stands
 *              uncancelled — the guard enforces what the ledger records.
 *              For class B the approval is the census-carried `declared_on`
 *              (the state the operator deployed — the env IS the operator's
 *              declaration at boot).
 *   OBSERVED — the real state of the switch NOW:
 *              class A: the LIVE value of the module's Redis control_key
 *              (same strict "true"/"false" dialect the board writes,
 *              parseDeclaredValue — foreign values are never interpreted);
 *              class B: the boot env of THIS api-server container
 *              (`process.env`, `env:` control_key convention from the CB-01
 *              census), compared only when the value is an UNAMBIGUOUS
 *              boolean ("true"/"false", trim+case-insensitive — the only
 *              spelling every real Rust consumer dialect agrees on; "on",
 *              "yes", "1", "0" are consumer-specific dialects and are
 *              reported verbatim as observed_raw WITHOUT a drift claim);
 *              class C: NOT observable from this service — reported as such,
 *              never simulated (§34.3 candado, never touched).
 *
 * On mismatch (RULE 00 — fail-honest, nothing fabricated):
 *   (a) INSERT audit_log (`control_board.drift`, migration 011 columns) with
 *       the EXACT diff —
 *       deduplicated by fingerprint so a persisting drift does not flood the
 *       ledger every scan (R9/LOGFLOOD lesson: one row per NEW occurrence);
 *   (b) the report feeds the visible alert: `drift` banner field inside the
 *       GET /api/v1/control-board snapshot (ControlBoardDriftSchema,
 *       ControlBoardLed.tsx:87 — CB-03 renders it) + the full diff payload
 *       at GET /api/v1/control-board/drift (the banner's diff_href);
 *   (c) class A: REVERT — write the Redis control_key back to the operator's
 *       approved value (the external change is UNDONE; this is NOT an
 *       auto-flip: it restores the last approved state, the opposite of
 *       flipping). Class B/C: alert + diff WITHOUT revert — remediation of a
 *       boot-env module is impossible without restart (CB-05 proposal flow),
 *       which is stated honestly, never simulated.
 *
 * Sovereignty baseline (documented honestly): the guard defends APPROVALS it
 * has on record. A class-A key the operator has never toggled through the
 * board carries `no_approval_record` — observed state is surfaced, but there
 * is nothing approved to defend or revert to. The first operator toggle
 * (CB-02 PUT) arms the guard for that module.
 *
 * Audit-vs-revert ordering note (deliberate deviation from CB-02's
 * audit-first-write-never, documented for the mesa): CB-02 blocks the Redis
 * WRITE when the audit INSERT fails because a toggle is a NEW operator
 * action. Here the approval being enforced is ALREADY in the ledger (the
 * toggle row that defined approved_on) — the drift row is the EVENT record
 * of the attempt. The class-A revert therefore runs FIRST and the audit row
 * is written SECOND carrying the TRUE revert outcome (R8: the ledger never
 * records a speculative result, and a revert that landed is never left
 * unrecorded — the fingerprint includes revert_applied, so a failed attempt
 * followed by a successful retry gets its own row). If the drift INSERT
 * still fails, the report carries `audit_persisted:false` + a loud
 * logger.warn for forensics (the reverted Redis state itself is observable).
 * Never silent.
 *
 * Class-B observation caveat (honest limitation, never hidden): observed env
 * is the API-SERVER container's boot env. The fleet boots from the same
 * compose .env, so a `.env` edit + recreate (the goal's "env drift, deploy"
 * case) is detected; a per-service compose override that only touches one
 * worker is NOT visible here and would need per-service observation (future
 * WO, not invented now).
 *
 * Mode-safety (§34): this guard only RESTORES operator-approved runtime
 * state. It never touches the executor/wallet/signing/broadcast paths; the
 * §34.3 terminus is class C here — not observed, not reverted, not togglable
 * (default-deny / MainnetRefused stay untouchable by construction).
 */
import { randomUUID } from "node:crypto";
import type pg from "pg";
import type { Redis } from "ioredis";
import {
  loadCensus,
  parseDeclaredValue,
  CONTROL_BOARD_TOGGLE_AUDIT_ACTION,
  CONTROL_BOARD_TOGGLE_FAILED_AUDIT_ACTION,
} from "../routes/control-board.js";
// NOTE on the route↔service import cycle: it is SAFE by construction — every
// cross-module reference here is CALL-time (inside functions), never
// module-eval time, so function hoisting resolves the bindings in either
// evaluation order. The service reuses the route's census loader/strict
// parser so the snapshot and the drift scan can never disagree (single
// source), rather than duplicating that logic.

/** CB-04 (2026-09-07) — audit row action for drift-guard events (CB-02 owns
 *  the `audit_log` table, migration 011; CB-05 §6.1 row contract:
 *  actor/action/target_kind/target_id/before_state/after_state). */
export const CONTROL_BOARD_DRIFT_AUDIT_ACTION = "control_board.drift";

/** CB-04 (2026-09-07) — system actor for guard events. Distinct from the
 *  operator actors of `control_board.toggle` rows so the ledger separates
 *  operator approvals from guard enforcement events. */
export const CONTROL_BOARD_DRIFT_GUARD_ACTOR = "control-board-drift-guard";

/** CB-04 (2026-09-07) — full-diff endpoint consumed by the CB-03 banner
 *  (ControlBoardDrift.diff_href). Admin-gated exactly like the board itself. */
export const CONTROL_BOARD_DRIFT_ENDPOINT_PATH = "/api/v1/control-board/drift";

// ─── Report types (wire: GET /api/v1/control-board/drift + banner field) ─────

export type ControlBoardDriftStatus =
  | "consistent"
  | "drift_detected"
  | "census_absent"
  | "census_unreadable"
  | "audit_unavailable";

export type ControlBoardDriftReason =
  | "aligned" // approved == observed (drift:false rows only)
  | "approved_vs_observed_mismatch" // A: approved known, observed known, differ
  | "observed_absent_with_approval" // A: key deleted, approved known
  | "observed_foreign_value" // A: key holds a non-board dialect, approved known
  | "env_differs_from_declared" // B: unambiguous boolean env differs from census declared_on
  | "no_approval_record" // A: no applied board toggle — nothing approved to defend
  | "not_comparable_non_boolean_env" // B: env value not an unambiguous boolean
  | "not_observable_class_c" // C: §34.3 candado — not observable here, never simulated
  | "census_invalid_control_key"; // census defect: class A sin control_key / class B sin convención env:

export interface ControlBoardDriftModuleEntry {
  id: string;
  name: string;
  module_class: "A" | "B" | "C";
  control_key: string | null;
  /** last operator-approved state (audit ledger for A, census declared_on for B; null = none) */
  approved_on: boolean | null;
  /** strictly-parsed observed state; null = absent/foreign/not comparable — never guessed */
  observed_on: boolean | null;
  /** verbatim observed value (Redis string / env string) — never interpreted */
  observed_raw: string | null;
  observed_source: "redis" | "env" | null;
  drift: boolean;
  reason: ControlBoardDriftReason;
  /** class A only: a revert was attempted in THIS scan */
  reverted: boolean;
  /** null = no revert applicable (class B/C); true/false = revert SET outcome */
  revert_applied: boolean | null;
  /** drift audit row landed for this scan (false only on INSERT failure) */
  audit_persisted: boolean | null;
}

export interface ControlBoardDriftSummary {
  modules_scanned: number;
  class_a_scanned: number;
  class_b_scanned: number;
  class_c_skipped: number;
  drifts: number;
  reverted: number;
  revert_failed: number;
  not_comparable: number;
  audit_writes_failed: number;
}

export interface ControlBoardDriftReport {
  status: ControlBoardDriftStatus;
  checked_at: string;
  scan_id: string;
  modules: ControlBoardDriftModuleEntry[];
  summary: ControlBoardDriftSummary;
}

/** CB-03 wire contract — ControlBoardDriftSchema (ControlBoardLed.tsx:87):
 *  { detected, checked_at, summary?, diff_href? }. */
export interface ControlBoardDriftBanner {
  detected: boolean;
  checked_at: string;
  summary: string | null;
  diff_href: string | null;
}

// ─── Strict parsers (RULE 00 — the only values every consumer agrees on) ─────

/**
 * CB-04 (2026-09-07) — strict BOOLEAN parse for env observation.
 * Accepts exactly "true"/"false" after trim+case-insensitive (the spelling
 * both real Rust dialects agree on: `eq_ignore_ascii_case("true")`,
 * main.rs:252-258, and env_bool's "1"|"true"|"yes"|"on",
 * scoring_pipeline.rs:283-289 — "on"/"yes"/"1"/"0" DISAGREE between those
 * dialects, so they are NOT claimed as booleans; they surface verbatim as
 * observed_raw with reason not_comparable_non_boolean_env).
 */
export function parseStrictBoolean(raw: string | null | undefined): boolean | null {
  if (raw === null || raw === undefined) return null;
  const v = raw.trim().toLowerCase();
  if (v === "true") return true;
  if (v === "false") return false;
  return null;
}

/** CB-04 (2026-09-07) — class-B control_key convention is `env:VAR`
 *  (CB-01-MODULES.json); strip the prefix to read the env var. */
export function envKeyOfControlKey(controlKey: string | null | undefined): string | null {
  if (!controlKey) return null;
  if (!controlKey.startsWith("env:")) return null;
  const name = controlKey.slice("env:".length);
  return name.length > 0 ? name : null;
}

// ─── Scan ────────────────────────────────────────────────────────────────────

export interface DriftScanDeps {
  pool: pg.Pool | null;
  redis: Redis | null;
  logger: {
    warn: (obj: object, msg?: string) => void;
    info: (obj: object, msg?: string) => void;
  };
  /** injectable for tests; default process.env (the boot env of this container) */
  env?: Record<string, string | undefined>;
}

/**
 * Latest APPROVED toggle per module (CB-02-API-APPLY §5.1 MUST-FIX — same
 * table/columns as loadLastToggles, control-board.ts:433-480): the ledger is
 * `audit_log` (SINGULAR, migration 011: actor/action/target_kind/target_id/
 * before_state/after_state, target_kind='control_board_module', payload in
 * after_state). CB-02 INSERTs the toggle row BEFORE the Redis SET and, when
 * the SET fails, a COMPENSATORY `control_board.toggle_failed` row carrying
 * the SAME payload `at` (append-only — 011:21 REVOKEs UPDATE). An approval
 * is therefore the LATEST toggle row without a compensatory sibling: a
 * change that never landed is not an approval, and a failed attempt falls
 * back to the previous landed approval (the operator's last effective wish).
 * The compensatory sibling pairs on after_state->>'at' (both rows are
 * written from the same `at` const, control-board.ts PUT; plain SQL `=`
 * never pairs two at-less rows — a foreign at-less INSERT cannot cancel an
 * approval). Honest limitation (documented in the module header): if CB-02's
 * compensatory INSERT itself failed, the attempt stands uncancelled.
 */
async function loadApprovedToggles(
  pool: pg.Pool,
  moduleIds: string[],
  logger: DriftScanDeps["logger"],
): Promise<Map<string, boolean> | null> {
  const out = new Map<string, boolean>();
  if (moduleIds.length === 0) return out;
  try {
    const q = await pool.query(
      `SELECT DISTINCT ON (target_id) target_id, after_state, created_at
         FROM audit_log a
        WHERE target_kind = 'control_board_module'
          AND action = $1
          AND target_id = ANY($2::text[])
          AND NOT EXISTS (
                SELECT 1
                  FROM audit_log f
                 WHERE f.target_kind = 'control_board_module'
                   AND f.action = $3
                   AND f.target_id = a.target_id
                   AND (f.after_state->>'at') = (a.after_state->>'at')
              )
        ORDER BY target_id, created_at DESC`,
      [CONTROL_BOARD_TOGGLE_AUDIT_ACTION, moduleIds, CONTROL_BOARD_TOGGLE_FAILED_AUDIT_ACTION],
    );
    for (const row of q.rows as Array<{ target_id: string; after_state: unknown }>) {
      let afterState: unknown = row.after_state;
      if (typeof afterState === "string") {
        try {
          afterState = JSON.parse(afterState);
        } catch {
          afterState = null;
        }
      }
      const p = (afterState ?? {}) as { on?: unknown };
      // RULE 00: only an EXPLICIT boolean after_state.on is an approval — a
      // garbage/foreign payload is no approval (module stays unarmed).
      if (typeof p.on === "boolean") out.set(row.target_id, p.on);
    }
    return out;
  } catch (e) {
    logger.warn({ event: "control_board_drift.approved_read_failed", err: (e as Error).message });
    return null; // null = unreadable ⇒ scan makes NO approval claims (honest abort)
  }
}

/** Latest drift-row fingerprint per module (dedup: one audit row per NEW
 *  occurrence — a persisting drift must not flood the ledger every tick).
 *  Same `audit_log` contract as the approvals query (migration 011; the
 *  fingerprint rides in after_state). */
async function loadLastDriftFingerprints(
  pool: pg.Pool,
  moduleIds: string[],
  logger: DriftScanDeps["logger"],
): Promise<Map<string, string> | null> {
  const out = new Map<string, string>();
  if (moduleIds.length === 0) return out;
  try {
    const q = await pool.query(
      `SELECT DISTINCT ON (target_id) target_id, after_state, created_at
         FROM audit_log
        WHERE target_kind = 'control_board_module'
          AND action = $1
          AND target_id = ANY($2::text[])
        ORDER BY target_id, created_at DESC`,
      [CONTROL_BOARD_DRIFT_AUDIT_ACTION, moduleIds],
    );
    for (const row of q.rows as Array<{ target_id: string; after_state: unknown }>) {
      let afterState: unknown = row.after_state;
      if (typeof afterState === "string") {
        try {
          afterState = JSON.parse(afterState);
        } catch {
          afterState = null;
        }
      }
      const p = (afterState ?? {}) as { fingerprint?: unknown };
      if (typeof p.fingerprint === "string") out.set(row.target_id, p.fingerprint);
    }
    return out;
  } catch (e) {
    logger.warn({ event: "control_board_drift.dedup_read_failed", err: (e as Error).message });
    return null; // unreadable dedup ⇒ we still report; INSERTs proceed (worst case: an extra row)
  }
}

function driftFingerprint(
  moduleId: string,
  reason: ControlBoardDriftReason,
  approvedOn: boolean | null,
  observedRaw: string | null,
  revertApplied: boolean | null,
): string {
  return JSON.stringify([moduleId, reason, approvedOn, observedRaw, revertApplied]);
}

function emptySummary(): ControlBoardDriftSummary {
  return {
    modules_scanned: 0,
    class_a_scanned: 0,
    class_b_scanned: 0,
    class_c_skipped: 0,
    drifts: 0,
    reverted: 0,
    revert_failed: 0,
    not_comparable: 0,
    audit_writes_failed: 0,
  };
}

/**
 * CB-04 (2026-09-07) — ONE drift scan (periodic driver + on-demand share it).
 * Pure with respect to inputs except its DECLARED side effects: drift audit
 * INSERTs (deduped) and class-A REVERT SETs. Fail-honest at every step:
 * census absent/unreadable or ledger unreadable ⇒ status says so and NO
 * module claims are made — never a fabricated "consistent".
 */
export async function runControlBoardDriftScan(deps: DriftScanDeps): Promise<ControlBoardDriftReport> {
  const { pool, redis, logger } = deps;
  const env = deps.env ?? process.env;
  const scanId = randomUUID();
  const checkedAt = new Date().toISOString();
  const summary = emptySummary();

  if (!redis) {
    return {
      status: "census_unreadable",
      checked_at: checkedAt,
      scan_id: scanId,
      modules: [],
      summary,
    };
  }
  const census = await loadCensus(redis, logger);
  if (!census.ok) {
    // absent ⇒ no baseline ⇒ nothing comparable (same honesty as CB-02 GET).
    // unreadable/corrupt ⇒ same, loudly.
    return {
      status: census.absent ? "census_absent" : "census_unreadable",
      checked_at: checkedAt,
      scan_id: scanId,
      modules: [],
      summary,
    };
  }
  const modules = census.modules;
  if (!pool) {
    // Without the audit ledger there is no approved state to compare —
    // the scan makes NO claims (never "consistent" by ignorance).
    return {
      status: "audit_unavailable",
      checked_at: checkedAt,
      scan_id: scanId,
      modules: [],
      summary,
    };
  }

  const moduleIds = modules.map((m) => m.id);
  const approved = await loadApprovedToggles(pool, moduleIds, logger);
  if (approved === null) {
    return {
      status: "audit_unavailable",
      checked_at: checkedAt,
      scan_id: scanId,
      modules: [],
      summary,
    };
  }
  const lastFingerprints = await loadLastDriftFingerprints(pool, moduleIds, logger);

  // Observe class A in parallel (C-S-E asincronía paralela; per-key read
  // failure ⇒ observed null for that module only — never a scan abort).
  const classAObserved = await Promise.all(
    modules.map(async (m) => {
      if (m.module_class !== "A" || !m.control_key) return null;
      try {
        return await redis.get(m.control_key);
      } catch (e) {
        logger.warn({
          event: "control_board_drift.observed_read_failed",
          module: m.id,
          control_key: m.control_key,
          err: (e as Error).message,
        });
        return null;
      }
    }),
  );

  const entries: ControlBoardDriftModuleEntry[] = [];
  for (let i = 0; i < modules.length; i++) {
    const m = modules[i]!;
    summary.modules_scanned += 1;

    // ── class C: §34.3 candado — not observable from this service ──
    if (m.module_class === "C") {
      summary.class_c_skipped += 1;
      entries.push({
        id: m.id,
        name: m.name,
        module_class: "C",
        control_key: m.control_key ?? null,
        approved_on: null,
        observed_on: null,
        observed_raw: null,
        observed_source: null,
        drift: false,
        reason: "not_observable_class_c",
        reverted: false,
        revert_applied: null,
        audit_persisted: null,
      });
      continue;
    }

    // ── class B: boot env observation (alert-only, never revert) ──
    if (m.module_class === "B") {
      summary.class_b_scanned += 1;
      const envName = envKeyOfControlKey(m.control_key);
      if (!envName) {
        // census defect: class B without the env: convention
        entries.push({
          id: m.id,
          name: m.name,
          module_class: "B",
          control_key: m.control_key ?? null,
          approved_on: m.declared_on ?? null,
          observed_on: null,
          observed_raw: null,
          observed_source: null,
          drift: false,
          reason: "census_invalid_control_key",
          reverted: false,
          revert_applied: null,
          audit_persisted: null,
        });
        continue;
      }
      const raw = env[envName] ?? null;
      const observed = parseStrictBoolean(raw);
      const approvedOn = m.declared_on ?? null;
      if (observed === null || approvedOn === null) {
        summary.not_comparable += 1;
        entries.push({
          id: m.id,
          name: m.name,
          module_class: "B",
          control_key: m.control_key ?? null,
          approved_on: approvedOn,
          observed_on: null,
          observed_raw: raw,
          observed_source: "env",
          drift: false,
          reason: "not_comparable_non_boolean_env",
          reverted: false,
          revert_applied: null,
          audit_persisted: null,
        });
        continue;
      }
      if (observed === approvedOn) {
        entries.push({
          id: m.id,
          name: m.name,
          module_class: "B",
          control_key: m.control_key ?? null,
          approved_on: approvedOn,
          observed_on: observed,
          observed_raw: raw,
          observed_source: "env",
          drift: false,
          reason: "aligned",
          reverted: false,
          revert_applied: null,
          audit_persisted: null,
        });
        continue;
      }
      // Class-B drift: alert + diff, NO revert (impossible without restart —
      // honest, never simulated; remediation is the CB-05 proposal flow).
      summary.drifts += 1;
      const entry: ControlBoardDriftModuleEntry = {
        id: m.id,
        name: m.name,
        module_class: "B",
        control_key: m.control_key ?? null,
        approved_on: approvedOn,
        observed_on: observed,
        observed_raw: raw,
        observed_source: "env",
        drift: true,
        reason: "env_differs_from_declared",
        reverted: false,
        revert_applied: null,
        audit_persisted: null,
      };
      await maybeAuditDrift(pool, entry, scanId, checkedAt, lastFingerprints, logger, summary);
      entries.push(entry);
      continue;
    }

    // ── class A: Redis observation + sovereign REVERT ──
    summary.class_a_scanned += 1;
    if (!m.control_key) {
      // census defect: class A sin control_key (CB-02 PUT also 503s this)
      entries.push({
        id: m.id,
        name: m.name,
        module_class: "A",
        control_key: null,
        approved_on: null,
        observed_on: null,
        observed_raw: null,
        observed_source: null,
        drift: false,
        reason: "census_invalid_control_key",
        reverted: false,
        revert_applied: null,
        audit_persisted: null,
      });
      continue;
    }
    const raw = classAObserved[i] ?? null; // noUncheckedIndexedAccess guard
    const observed = parseDeclaredValue(raw); // strict board dialect
    const approvedOn = approved.get(m.id) ?? null;
    if (approvedOn === null) {
      // Sovereignty baseline: no board approval on record — observed is
      // surfaced, but nothing is defended or reverted (first toggle arms it).
      entries.push({
        id: m.id,
        name: m.name,
        module_class: "A",
        control_key: m.control_key,
        approved_on: null,
        observed_on: observed,
        observed_raw: raw,
        observed_source: "redis",
        drift: false,
        reason: "no_approval_record",
        reverted: false,
        revert_applied: null,
        audit_persisted: null,
      });
      continue;
    }
    if (observed === approvedOn) {
      entries.push({
        id: m.id,
        name: m.name,
        module_class: "A",
        control_key: m.control_key,
        approved_on: approvedOn,
        observed_on: observed,
        observed_raw: raw,
        observed_source: "redis",
        drift: false,
        reason: "aligned",
        reverted: false,
        revert_applied: null,
        audit_persisted: null,
      });
      continue;
    }

    // Class-A DRIFT: the external world moved what the operator approved.
    summary.drifts += 1;
    const reason: ControlBoardDriftReason =
      observed === null
        ? raw === null
          ? "observed_absent_with_approval"
          : "observed_foreign_value"
        : "approved_vs_observed_mismatch";
    const entry: ControlBoardDriftModuleEntry = {
      id: m.id,
      name: m.name,
      module_class: "A",
      control_key: m.control_key,
      approved_on: approvedOn,
      observed_on: observed,
      observed_raw: raw,
      observed_source: "redis",
      drift: true,
      reason,
      reverted: true, // a revert is attempted for every class-A drift
      revert_applied: null,
      audit_persisted: null,
    };
    // (c) REVERT FIRST — restore the operator's approved value (the board
    // dialect). The audit row is written AFTER with the TRUE outcome (see
    // the ordering note in the module header: R8 — the ledger never records
    // a speculative revert result).
    try {
      await redis.set(m.control_key, approvedOn ? "true" : "false");
      entry.revert_applied = true;
      summary.reverted += 1;
      logger.info({
        event: "control_board_drift.reverted",
        module: m.id,
        control_key: m.control_key,
        approved_on: approvedOn,
        observed_raw: raw,
        reason,
      });
    } catch (e) {
      entry.revert_applied = false;
      summary.revert_failed += 1;
      logger.warn({
        event: "control_board_drift.revert_failed",
        module: m.id,
        control_key: m.control_key,
        approved_on: approvedOn,
        observed_raw: raw,
        err: (e as Error).message,
      });
    }
    // (a) exact-diff audit row (deduped), now carrying revert_applied.
    await maybeAuditDrift(pool, entry, scanId, checkedAt, lastFingerprints, logger, summary);
    entries.push(entry);
  }

  return {
    status: summary.drifts > 0 ? "drift_detected" : "consistent",
    checked_at: checkedAt,
    scan_id: scanId,
    modules: entries,
    summary,
  };
}

/**
 * (a) INSERT the exact-diff audit row — ONLY when the fingerprint differs
 * from the latest recorded drift for the module (R9 dedup: one row per NEW
 * occurrence; a persisting drift stays visible in the report/banner without
 * flooding the ledger). Best-effort ordering documented in the module header.
 */
async function maybeAuditDrift(
  pool: pg.Pool,
  entry: ControlBoardDriftModuleEntry,
  scanId: string,
  checkedAt: string,
  lastFingerprints: Map<string, string> | null,
  logger: DriftScanDeps["logger"],
  summary: ControlBoardDriftSummary,
): Promise<void> {
  const fingerprint = driftFingerprint(
    entry.id,
    entry.reason,
    entry.approved_on,
    entry.observed_raw,
    entry.revert_applied,
  );
  if (lastFingerprints !== null && lastFingerprints.get(entry.id) === fingerprint) {
    entry.audit_persisted = null; // dedup: occurrence already recorded
    return;
  }
  const payload = {
    wo: "CB-04",
    module: entry.id,
    control_key: entry.control_key,
    module_class: entry.module_class,
    approved_on: entry.approved_on,
    observed_on: entry.observed_on,
    observed_raw: entry.observed_raw,
    reason: entry.reason,
    reverted: entry.reverted,
    revert_applied: entry.revert_applied, // true/false for class A (revert ran first); null for class B (no revert possible)
    at: checkedAt,
    scan_id: scanId,
    fingerprint,
  };
  try {
    // CB-04 (2026-09-07) — migration 011 columns, the SAME contract CB-02's
    // writes use (control-board.ts PUT): before_state = the DRIFTED state
    // observed (what the world looked like before the revert), after_state =
    // the full exact-diff payload carrying the TRUE revert outcome.
    await pool.query(
      `INSERT INTO audit_log (actor, action, target_kind, target_id, before_state, after_state)
       VALUES ($1, $2, 'control_board_module', $3, $4::jsonb, $5::jsonb)`,
      [
        CONTROL_BOARD_DRIFT_GUARD_ACTOR,
        CONTROL_BOARD_DRIFT_AUDIT_ACTION,
        entry.id,
        JSON.stringify({ on: entry.observed_on }),
        JSON.stringify(payload),
      ],
    );
    entry.audit_persisted = true;
  } catch (e) {
    entry.audit_persisted = false;
    summary.audit_writes_failed += 1;
    logger.warn({
      event: "control_board_drift.audit_insert_failed",
      module: entry.id,
      fingerprint,
      err: (e as Error).message,
    });
  }
}

// ─── Banner (CB-03 wire: ControlBoardDriftSchema) ────────────────────────────

/** CB-04 (2026-09-07) — map a report to the CB-03 banner field. RULE 00:
 *  "no drift detected" is only claimed when a comparison actually ran —
 *  census/ledger-unavailable states carry detected:false with a summary that
 *  says NON-COMPUTABLE (never a fabricated green). */
export function toControlBoardDriftBanner(
  report: ControlBoardDriftReport,
): ControlBoardDriftBanner {
  const { summary, status } = report;
  let text: string;
  switch (status) {
    case "drift_detected": {
      const unresolved = summary.drifts - summary.reverted;
      text =
        `${summary.drifts} módulo(s) divergen del último estado aprobado — ` +
        `${summary.reverted} revertido(s) al valor aprobado por el operador` +
        (unresolved > 0
          ? `, ${unresolved} sin revertir (clase B/C: requiere restart — CB-05)`
          : "");
      break;
    }
    case "consistent": {
      const comparable = summary.modules_scanned - summary.not_comparable;
      text =
        `sin drift: ${comparable} módulos comparables contra el registro de aprobación` +
        (summary.not_comparable > 0 ? ` (${summary.not_comparable} no comparables — ver diff)` : "");
      break;
    }
    case "census_absent":
      text = "censo CB-01 no publicado (arbx:config:control_board) — drift NO computable";
      break;
    case "census_unreadable":
      text = "censo ilegible (redis caído o JSON corrupto) — drift NO computable";
      break;
    case "audit_unavailable":
      text = "audit_log no disponible — estado aprobado NO computable";
      break;
  }
  return {
    detected: status === "drift_detected" && summary.drifts > 0,
    checked_at: report.checked_at,
    summary: text,
    diff_href: CONTROL_BOARD_DRIFT_ENDPOINT_PATH,
  };
}

// ─── Periodic guard ──────────────────────────────────────────────────────────

export interface ControlBoardDriftGuard {
  /** On-demand scan, TTL-throttled (banner/diff endpoint callers). Returns
   *  the freshest report (never triggers a concurrent duplicate scan). */
  scanIfDue(): Promise<ControlBoardDriftReport | null>;
  getLastReport(): ControlBoardDriftReport | null;
  /** CB-03 banner field — null until the first scan has run. */
  getBanner(): ControlBoardDriftBanner | null;
  stop(): void;
}

export interface ControlBoardDriftGuardOptions {
  /** periodic scan cadence (default 60s — gentle; scan is read-mostly) */
  intervalMs?: number | undefined;
  /** minimum spacing between real scans (default 5s — endpoint stampede guard) */
  minScanIntervalMs?: number | undefined;
}

/**
 * CB-04 (2026-09-07) — the periodic drift guard. One instance per process
 * (created by mountControlBoard). The interval is unref'd (never holds the
 * event loop — tests/process exit cleanly) and every tick is fully caught
 * (a scan failure degrades the banner honestly, never throws unhandled).
 */
export function createControlBoardDriftGuard(
  deps: DriftScanDeps,
  opts?: ControlBoardDriftGuardOptions,
): ControlBoardDriftGuard {
  const intervalMs = opts?.intervalMs ?? 60_000;
  const minScanIntervalMs = opts?.minScanIntervalMs ?? 5_000;
  let lastReport: ControlBoardDriftReport | null = null;
  let lastScanAt = 0;
  let inFlight: Promise<void> | null = null;

  const runScan = async (): Promise<void> => {
    lastScanAt = Date.now();
    try {
      lastReport = await runControlBoardDriftScan(deps);
      if (lastReport.status === "drift_detected") {
        deps.logger.warn({
          event: "control_board_drift.scan",
          status: lastReport.status,
          drifts: lastReport.summary.drifts,
          reverted: lastReport.summary.reverted,
        });
      }
    } catch (e) {
      // Never expected (scan catches internally) — belt and braces so a
      // guard bug can NEVER take the api-server down.
      deps.logger.warn({ event: "control_board_drift.scan_crashed", err: (e as Error).message });
    }
  };

  /** Single-flight wrapper: the interval tick and on-demand scanIfDue share
   *  one mutex so a tick + an endpoint poll can never run two concurrent
   *  scans (which could race the ledger dedup into a duplicate audit row). */
  const tick = async (): Promise<void> => {
    if (inFlight) {
      await inFlight;
      return;
    }
    inFlight = runScan();
    try {
      await inFlight;
    } finally {
      inFlight = null;
    }
  };

  const timer: ReturnType<typeof setInterval> = setInterval(() => {
    void tick();
  }, intervalMs);
  if (typeof timer === "object" && timer !== null && typeof timer.unref === "function") {
    timer.unref();
  }

  return {
    async scanIfDue(): Promise<ControlBoardDriftReport | null> {
      if (Date.now() - lastScanAt < minScanIntervalMs) return lastReport;
      await tick();
      return lastReport;
    },
    getLastReport(): ControlBoardDriftReport | null {
      return lastReport;
    },
    getBanner(): ControlBoardDriftBanner | null {
      return lastReport === null ? null : toControlBoardDriftBanner(lastReport);
    },
    stop(): void {
      clearInterval(timer);
    },
  };
}
