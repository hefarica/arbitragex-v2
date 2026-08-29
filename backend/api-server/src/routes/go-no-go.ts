/**
 * go-no-go — A.9 formal GO/NO-GO ledger machinery (ARBX-RDY-06).
 *
 *   GET  /api/v1/go-no-go/ledger       (public read; GENERATES + persists a
 *                                       canonical ledger document)
 *   GET  /api/v1/go-no-go/status       (public read of RECORDED state)
 *   POST /admin/go-no-go/sign-off      (admin token + x-arbx-actor; records a
 *                                       human operator decision)
 *
 * WHAT THIS IS: the machinery that captures, verifies, and persists the A.9
 * two-operator sign-off. The SIGN-OFF ITSELF is human-by-design — nothing
 * here decides GO/NO-GO; operators do, and this route only records it.
 *
 * WHAT THIS IS NOT: it NEVER flips anything live. live_exec_policy
 * (relays-client) stays default-deny (CLAUDE.md §34.3). `go_live_eligible`
 * is a READ of recorded state — a necessary condition list, never a switch.
 *
 * Ledger lifecycle:
 *   1. GET /ledger composes the CURRENT facts (deps.buildLedgerFacts —
 *      injected at mount time from index.ts via buildDefaultLedgerFacts so
 *      this module stays free of index.ts-level wiring), hashes the
 *      canonical JSON (sha256 over sorted-keys, whitespace-free JSON) and
 *      persists the generation to audit_log (action go_no_go.ledger_generated,
 *      target_id = ledger_hash). Re-generating identical facts is deduplicated
 *      (no second audit row) so polling can never flood the audit trail.
 *   2. Operator A signs POST /admin/go-no-go/sign-off with the ledger_hash
 *      they reviewed. A hash that no longer matches the current generation
 *      is a STALE sign-off → 400 (the facts changed; re-review required).
 *   3. Operator B signs the same ledger_hash. Two DISTINCT actors are
 *      enforced by UNIQUE (ledger_hash, actor) (migration 110): a second
 *      sign-off from the same actor → 409.
 *
 * Honesty contract (RULE 00 / R8): every fact carries its source; an
 * unavailable source is recorded as unavailable (null / available:false with
 * reason) and FAILS CLOSED for eligibility — it is never invented.
 */

import { createHash } from "node:crypto";
import type { Application, Request, RequestHandler, Response } from "express";
import type pg from "pg";
import { z } from "zod";
import { verifyAll } from "../readiness/verifiers/index.js";
import { resolvePaperModeState } from "../readiness/paper-mode-state.js";

// ---------------------------------------------------------------------------
// Contract constants.
// ---------------------------------------------------------------------------

/** Ledger document schema version — bump on any breaking facts-shape change. */
export const LEDGER_SCHEMA_VERSION = 1;

export const LEDGER_GENERATED_ACTION = "go_no_go.ledger_generated";
export const SIGN_OFF_ACTION = "go_no_go.sign_off";
export const GO_NO_GO_TARGET_KIND = "go_no_go";

/** Fail-closed bar for the paper-safety leg of go_live_eligible. */
const ZERO_CAPITAL_USD = 0;

// ---------------------------------------------------------------------------
// Types — wire contract. Frontend consumers mirror these; treat any breaking
// change as a coordinated bump of LEDGER_SCHEMA_VERSION.
// ---------------------------------------------------------------------------

export type GoNoGoState =
  | "no_ledger"
  | "awaiting_first"
  | "awaiting_second"
  | "signed_go"
  | "signed_no_go"
  | "conflicted";

export type SignOffDecision = "GO" | "NO_GO";

/**
 * The canonical facts document. The two decision-critical sections are
 * typed; every section must carry a human-readable `source` so the ledger
 * is self-describing about provenance (R8).
 *
 *   - blockers.unresolved_count  — null = source unavailable (fail-closed).
 *   - paper_safety.paper_mode_active — null = source unavailable (fail-closed).
 *   - paper_safety.capital_exposure_usd — 0 is the STRUCTURAL shadow
 *     invariant (same precedent as /api/capital-gates hardcoded-safe
 *     posture), not an observation.
 */
export interface LedgerFacts {
  schema_version: number;
  blockers: {
    unresolved_count: number | null;
    critical_count: number | null;
    source: string;
    [key: string]: unknown;
  };
  paper_safety: {
    paper_mode_active: boolean | null;
    capital_exposure_usd: number;
    source: string;
    [key: string]: unknown;
  };
  [key: string]: unknown;
}

/** Injected at mount time (index.ts → buildDefaultLedgerFacts). */
export type BuildLedgerFacts = () => Promise<LedgerFacts>;

export interface GoNoGoDeps {
  pool: pg.Pool | null;
  logger: { warn: (obj: object, msg?: string) => void };
  requireAdminToken: (expected: string) => RequestHandler;
  adminToken: string;
  buildLedgerFacts: BuildLedgerFacts;
}

interface SignOffRow {
  actor: string;
  decision: string;
  signed_at: Date | string;
}

/** audit_log row shape for a ledger generation (after_state is JSONB). */
interface LedgerAuditRow {
  id: string;
  created_at: Date | string;
  after_state: {
    ledger_hash?: unknown;
    summary?: { unresolved_blockers?: unknown; paper_safe?: unknown } | null;
    [key: string]: unknown;
  } | null;
}

// ---------------------------------------------------------------------------
// Canonicalization + hashing (exported for tests).
// ---------------------------------------------------------------------------

/** Recursively sort object keys; arrays keep order. JSON-safe values only. */
function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value !== null && typeof value === "object") {
    const src = value as Record<string, unknown>;
    const out: Record<string, unknown> = {};
    for (const k of Object.keys(src).sort()) {
      out[k] = canonicalize(src[k]);
    }
    return out;
  }
  return value;
}

/** Sorted-keys, whitespace-free JSON — the stable serialization for hashing. */
export function canonicalJsonStringify(value: unknown): string {
  return JSON.stringify(canonicalize(value));
}

/**
 * ledger_hash = sha256 over canonicalJsonStringify({schema_version, facts}).
 * `generated_at` is deliberately NOT part of the hash input — identical facts
 * must yield the identical hash, or dedupe/staleness would be meaningless.
 */
export function computeLedgerHash(facts: unknown, schemaVersion: number): string {
  return createHash("sha256")
    .update(canonicalJsonStringify({ schema_version: schemaVersion, facts }), "utf8")
    .digest("hex");
}

/** Unresolved blockers as a finite number, else null (fail-closed). */
function unresolvedBlockersOf(facts: LedgerFacts): number | null {
  const n = facts.blockers?.unresolved_count;
  return typeof n === "number" && Number.isFinite(n) && n >= 0 ? n : null;
}

/** Paper safety = paper mode explicitly ACTIVE and capital exposure 0. */
function paperSafeOf(facts: LedgerFacts): boolean {
  return (
    facts.paper_safety?.paper_mode_active === true &&
    Number(facts.paper_safety?.capital_exposure_usd) === ZERO_CAPITAL_USD
  );
}

/**
 * State derivation from the sign-off rows recorded for ONE ledger_hash:
 * 1→awaiting_second; ≥2 all-GO→signed_go; ≥2 all-NO_GO→signed_no_go;
 * mixed→conflicted; none→awaiting_first. Exported for tests.
 */
export function deriveGoNoGoState(rows: { decision: string }[]): GoNoGoState {
  if (rows.length === 0) return "awaiting_first";
  const hasGo = rows.some((r) => r.decision === "GO");
  const hasNoGo = rows.some((r) => r.decision === "NO_GO");
  if (hasGo && hasNoGo) return "conflicted";
  if (rows.length >= 2) return hasGo ? "signed_go" : "signed_no_go";
  return "awaiting_second";
}

function isoOrNull(v: Date | string | null | undefined): string | null {
  if (v == null) return null;
  return v instanceof Date ? v.toISOString() : new Date(v).toISOString();
}

function isPgUniqueViolation(e: unknown): boolean {
  const err = e as { code?: string; message?: string; constraint?: string };
  return (
    err?.code === "23505" ||
    (typeof err?.message === "string" && err.message.includes("go_no_go_signoffs_ledger_hash_actor_key"))
  );
}

// ---------------------------------------------------------------------------
// Persistence helpers — mirror the index.ts writeAudit INSERT (same SQL, so
// rows land in the same PII-hardened shape: anonymized IP CIDR + hashed UA).
// ---------------------------------------------------------------------------

async function insertAuditRow(
  pool: pg.Pool,
  row: {
    actor: string;
    action: string;
    targetId: string | null;
    before: unknown;
    after: unknown;
    ip: string | null;
    traceId: string | null;
    userAgent: string | null;
  },
): Promise<void> {
  await pool.query(
    `INSERT INTO audit_log (actor, action, target_kind, target_id, before_state, after_state, ip_address, trace_id, user_agent)
     VALUES ($1,$2,$3,$4,$5::jsonb,$6::jsonb,arbx_anonymize_ip($7)::cidr,$8,arbx_hash_user_agent($9))`,
    [
      row.actor,
      row.action,
      GO_NO_GO_TARGET_KIND,
      row.targetId,
      JSON.stringify(row.before ?? null),
      JSON.stringify(row.after ?? null),
      row.ip,
      row.traceId,
      row.userAgent,
    ],
  );
}

/** Latest persisted ledger generation (sign-offs always reference THIS). */
async function readLatestLedger(pool: pg.Pool): Promise<LedgerAuditRow | null> {
  const q = await pool.query<LedgerAuditRow>(
    `SELECT id, created_at, after_state
       FROM audit_log
      WHERE action = $1 AND target_kind = $2
      ORDER BY created_at DESC
      LIMIT 1`,
    [LEDGER_GENERATED_ACTION, GO_NO_GO_TARGET_KIND],
  );
  return q.rows[0] ?? null;
}

function ledgerHashOf(row: LedgerAuditRow | null): string | null {
  const h = row?.after_state?.ledger_hash;
  return typeof h === "string" && h.length > 0 ? h : null;
}

function summaryOf(row: LedgerAuditRow | null): {
  unresolved_blockers: number | null;
  paper_safe: boolean | null;
} {
  const s = row?.after_state?.summary;
  const ub = s?.unresolved_blockers;
  const ps = s?.paper_safe;
  return {
    unresolved_blockers:
      typeof ub === "number" && Number.isFinite(ub) && ub >= 0 ? ub : null,
    paper_safe: typeof ps === "boolean" ? ps : null,
  };
}

async function readSignOffs(pool: pg.Pool, ledgerHash: string): Promise<SignOffRow[]> {
  const q = await pool.query<SignOffRow>(
    `SELECT actor, decision, signed_at
       FROM go_no_go_signoffs
      WHERE ledger_hash = $1
      ORDER BY signed_at ASC, id ASC`,
    [ledgerHash],
  );
  return q.rows;
}

function goLiveEligible(state: GoNoGoState, summary: { unresolved_blockers: number | null; paper_safe: boolean | null }): boolean {
  return (
    state === "signed_go" && summary.unresolved_blockers === 0 && summary.paper_safe === true
  );
}

// ---------------------------------------------------------------------------
// Route mounting.
// ---------------------------------------------------------------------------

export function mountGoNoGo(app: Application, deps: GoNoGoDeps): void {
  const auth = deps.requireAdminToken(deps.adminToken);

  // ── GET /api/v1/go-no-go/ledger — generate (+persist) the ledger ────────
  app.get("/api/v1/go-no-go/ledger", async (_req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }
    let facts: LedgerFacts;
    try {
      facts = await deps.buildLedgerFacts();
    } catch (e) {
      deps.logger.warn({ event: "go_no_go.facts_failed", err: (e as Error).message });
      res.status(503).json({ error: "facts_source_failed", detail: (e as Error).message });
      return;
    }
    const ledgerHash = computeLedgerHash(facts, LEDGER_SCHEMA_VERSION);
    const summary = {
      unresolved_blockers: unresolvedBlockersOf(facts),
      paper_safe: paperSafeOf(facts),
    };

    let latest: LedgerAuditRow | null;
    try {
      latest = await readLatestLedger(deps.pool);
    } catch (e) {
      deps.logger.warn({ event: "go_no_go.ledger_read_failed", err: (e as Error).message });
      res.status(503).json({ error: "ledger_read_failed", detail: (e as Error).message });
      return;
    }

    // Deduplicate: identical facts → identical hash → the generation is
    // already on the audit trail. Never insert a second copy (a polled GET
    // must not flood audit_log).
    if (ledgerHashOf(latest) === ledgerHash && latest) {
      res.status(200).json({
        schema_version: LEDGER_SCHEMA_VERSION,
        ledger_hash: ledgerHash,
        generated_at: isoOrNull(latest.created_at),
        deduplicated: true,
        summary,
        facts,
      });
      return;
    }

    // The generation MUST be durable before it can be signed — a failed
    // persist means sign-offs would reference an unrecorded document, so
    // this write is load-bearing and fails the request (unlike the
    // fire-and-forget admin audits elsewhere).
    try {
      await insertAuditRow(deps.pool, {
        actor: "system:go-no-go",
        action: LEDGER_GENERATED_ACTION,
        targetId: ledgerHash,
        before: null,
        after: { ledger_hash: ledgerHash, schema_version: LEDGER_SCHEMA_VERSION, summary, facts },
        ip: null,
        traceId: null,
        userAgent: null,
      });
    } catch (e) {
      deps.logger.warn({ event: "go_no_go.ledger_persist_failed", err: (e as Error).message });
      res.status(503).json({ error: "ledger_persist_failed", detail: (e as Error).message });
      return;
    }

    res.status(200).json({
      schema_version: LEDGER_SCHEMA_VERSION,
      ledger_hash: ledgerHash,
      generated_at: new Date().toISOString(),
      deduplicated: false,
      summary,
      facts,
    });
  });

  // ── GET /api/v1/go-no-go/status — read of RECORDED state ────────────────
  app.get("/api/v1/go-no-go/status", async (_req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }
    let latest: LedgerAuditRow | null;
    try {
      latest = await readLatestLedger(deps.pool);
    } catch (e) {
      deps.logger.warn({ event: "go_no_go.status_read_failed", err: (e as Error).message });
      res.status(503).json({ error: "ledger_read_failed", detail: (e as Error).message });
      return;
    }
    const ledgerHash = ledgerHashOf(latest);
    if (!ledgerHash) {
      res.status(200).json({
        ledger_hash: null,
        sign_offs: [],
        state: "no_ledger" as GoNoGoState,
        go_live_eligible: false,
      });
      return;
    }

    let rows: SignOffRow[];
    try {
      rows = await readSignOffs(deps.pool, ledgerHash);
    } catch (e) {
      // Migration 110 not applied is a deployment gap, not an empty registry
      // — fail loud (R8), never present "awaiting_first" over a missing table.
      deps.logger.warn({ event: "go_no_go.signoffs_read_failed", err: (e as Error).message });
      res.status(503).json({ error: "signoffs_read_failed", detail: (e as Error).message });
      return;
    }

    const state = deriveGoNoGoState(rows);
    const summary = summaryOf(latest);
    res.status(200).json({
      ledger_hash: ledgerHash,
      generated_at: isoOrNull(latest?.created_at ?? null),
      sign_offs: rows.map((r) => ({
        actor: r.actor,
        decision: r.decision,
        signed_at: isoOrNull(r.signed_at),
      })),
      state,
      go_live_eligible: goLiveEligible(state, summary),
      ledger_summary: summary,
    });
  });

  // ── POST /admin/go-no-go/sign-off — record ONE operator decision ─────────
  const SignOffBody = z.object({
    decision: z.enum(["GO", "NO_GO"]),
    // sha256 hex (64 chars); lowercase-normalized before comparison.
    ledger_hash: z.string().regex(/^[0-9a-fA-F]{64}$/, "expected 64-char sha256 hex"),
  });

  app.post("/admin/go-no-go/sign-off", auth, async (req: Request, res: Response) => {
    const parsed = SignOffBody.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
      return;
    }
    const actor = req.header("x-arbx-actor")?.trim() ?? "";
    if (!actor) {
      res.status(400).json({
        error: "missing_actor",
        detail: "x-arbx-actor header is required — the sign-off must name the human operator",
      });
      return;
    }
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }

    const submittedHash = parsed.data.ledger_hash.toLowerCase();
    let latest: LedgerAuditRow | null;
    try {
      latest = await readLatestLedger(deps.pool);
    } catch (e) {
      deps.logger.warn({ event: "go_no_go.signoff_ledger_read_failed", err: (e as Error).message });
      res.status(503).json({ error: "ledger_read_failed", detail: (e as Error).message });
      return;
    }
    const currentHash = ledgerHashOf(latest);
    if (!currentHash) {
      res.status(400).json({
        error: "no_ledger_generated",
        detail: "no ledger generation exists yet — GET /api/v1/go-no-go/ledger first",
      });
      return;
    }
    if (submittedHash !== currentHash) {
      res.status(400).json({
        error: "stale_ledger_hash",
        detail:
          "submitted ledger_hash does not match the current ledger generation — the facts changed after review; regenerate the ledger and re-review",
        submitted_ledger_hash: submittedHash,
        current_ledger_hash: currentHash,
      });
      return;
    }

    // Application-level distinct-actor check (the UNIQUE constraint is the
    // race backstop, handled below).
    try {
      const existing = await deps.pool.query(
        `SELECT actor, decision, signed_at
           FROM go_no_go_signoffs
          WHERE ledger_hash = $1 AND actor = $2`,
        [submittedHash, actor],
      );
      const prior = existing.rows[0] as SignOffRow | undefined;
      if (prior) {
        res.status(409).json({
          error: "duplicate_signoff",
          detail: `two DISTINCT operators must sign this ledger — actor "${actor}" already signed it (${prior.decision} at ${isoOrNull(prior.signed_at)})`,
          existing_decision: prior.decision,
        });
        return;
      }

      const inserted = await deps.pool.query<SignOffRow>(
        `INSERT INTO go_no_go_signoffs (ledger_hash, actor, decision)
         VALUES ($1,$2,$3)
         RETURNING actor, decision, signed_at`,
        [submittedHash, actor, parsed.data.decision],
      );
      const row = inserted.rows[0]!;

      const rows = await readSignOffs(deps.pool, submittedHash);
      const state = deriveGoNoGoState(rows);
      const summary = summaryOf(latest);

      // The sign-off row (above) is the durable record; the audit entry is
      // the secondary trail, so a failure here is logged, never fatal —
      // erroring AFTER persistence would push the operator into a doomed
      // retry that 409s.
      try {
        await insertAuditRow(deps.pool, {
          actor,
          action: SIGN_OFF_ACTION,
          targetId: submittedHash,
          before: null,
          after: {
            ledger_hash: submittedHash,
            decision: parsed.data.decision,
            resulting_state: state,
          },
          ip: req.ip ?? req.socket.remoteAddress ?? null,
          traceId: (req as Request & { traceId?: string }).traceId ?? null,
          userAgent: req.header("user-agent") || null,
        });
      } catch (e) {
        deps.logger.warn({ event: "go_no_go.signoff_audit_failed", err: (e as Error).message });
      }

      res.status(201).json({
        ok: true,
        ledger_hash: submittedHash,
        actor: row.actor,
        decision: row.decision,
        signed_at: isoOrNull(row.signed_at),
        state,
        go_live_eligible: goLiveEligible(state, summary),
      });
    } catch (e) {
      if (isPgUniqueViolation(e)) {
        res.status(409).json({
          error: "duplicate_signoff",
          detail: `two DISTINCT operators must sign this ledger — actor "${actor}" already signed it (unique constraint)`,
        });
        return;
      }
      deps.logger.warn({ event: "go_no_go.signoff_write_failed", err: (e as Error).message });
      res.status(500).json({ error: "signoff_write_failed", detail: (e as Error).message });
    }
  });
}

// ---------------------------------------------------------------------------
// Default facts composer — injected from index.ts as deps.buildLedgerFacts.
// Every section carries its source; unavailable sources are recorded as
// unavailable (RULE 00 / R8) and fail closed for eligibility.
// ---------------------------------------------------------------------------

/** Realized paper PnL per row (same expression as paper-shadow-metrics). */
const PNL_EXPR = "COALESCE(actual_profit_usd, sim_expected_profit_usd, 0)";
/** Paper-shadow surface is chain 1 (ethereum), matching paper-shadow-metrics. */
const PAPER_CHAIN_ID = 1;

export async function buildDefaultLedgerFacts(deps: {
  pool: pg.Pool | null;
  /** Structural { mget } is enough (resolvePaperModeState contract). */
  redis: { mget: (...keys: string[]) => Promise<(string | null)[]> } | null;
  enabledChainIds: number[];
  logger: { warn: (obj: object, msg?: string) => void };
}): Promise<LedgerFacts> {
  // (1) Readiness — the canonical 17-verifier report; every non-green item is
  // an unresolved blocker. This is the SAME source /api/v1/readiness serves.
  let blockers: LedgerFacts["blockers"];
  try {
    const report = await verifyAll({ pool: deps.pool });
    const items = report.items ?? [];
    const nonGreen = items.filter((i) => i.status !== "green");
    const critical = items.filter((i) => i.status === "red");
    blockers = {
      unresolved_count: nonGreen.length,
      critical_count: critical.length,
      source: "verifyAll (backend/api-server/src/readiness/verifiers)",
      flip_blocked: report.flip_blocked,
      non_green_ids: nonGreen.map((i) => i.id),
    };
  } catch (e) {
    blockers = {
      unresolved_count: null,
      critical_count: null,
      source: "verifyAll (backend/api-server/src/readiness/verifiers)",
      available: false,
      reason: (e as Error).message,
    };
  }

  // (2) Paper-shadow accumulation — direct SQL over paper_trade_runs (same
  // table + PnL expression as GET /api/metrics/paper-shadow).
  let paperShadow: Record<string, unknown>;
  if (deps.pool) {
    try {
      const totals = await deps.pool.query<{
        total_trades: string;
        green_trades: string;
        red_trades: string;
        pnl_accumulated_usd: string;
        last_trade_at: Date | null;
      }>(
        `SELECT COUNT(*)::text AS total_trades,
                COALESCE(SUM(CASE WHEN ${PNL_EXPR} > 0 THEN 1 ELSE 0 END), 0)::text AS green_trades,
                COALESCE(SUM(CASE WHEN ${PNL_EXPR} <= 0 THEN 1 ELSE 0 END), 0)::text AS red_trades,
                COALESCE(SUM(${PNL_EXPR}), 0)::text AS pnl_accumulated_usd,
                MAX(created_at) AS last_trade_at
           FROM paper_trade_runs
          WHERE chain_id = $1`,
        [PAPER_CHAIN_ID],
      );
      const days = await deps.pool.query<{ day_pnl: string }>(
        `SELECT COALESCE(SUM(${PNL_EXPR}), 0)::text AS day_pnl
           FROM paper_trade_runs
          WHERE chain_id = $1
          GROUP BY date_trunc('day', created_at)
          ORDER BY date_trunc('day', created_at) DESC`,
        [PAPER_CHAIN_ID],
      );
      let consecutiveGreenDays = 0;
      for (const row of days.rows) {
        if (Number(row.day_pnl) > 0) consecutiveGreenDays += 1;
        else break;
      }
      const t = totals.rows[0]!;
      paperShadow = {
        available: true,
        source: "paper_trade_runs (chain 1, SQL)",
        total_trades: Number(t.total_trades),
        green_trades: Number(t.green_trades),
        red_trades: Number(t.red_trades),
        pnl_accumulated_usd: Number(t.pnl_accumulated_usd),
        consecutive_green_days: consecutiveGreenDays,
        last_trade_at: isoOrNull(t.last_trade_at),
      };
    } catch (e) {
      paperShadow = {
        available: false,
        source: "paper_trade_runs (chain 1, SQL)",
        reason: (e as Error).message,
      };
    }
  } else {
    paperShadow = {
      available: false,
      source: "paper_trade_runs (chain 1, SQL)",
      reason: "db_unavailable",
    };
  }

  // (3) Paper safety — the canonical per-chain resolver over Redis
  // arbx:papermode:* (same source as G-PAP-1). Unavailable → null → fails
  // closed. capital_exposure_usd=0 is the STRUCTURAL shadow invariant (same
  // precedent as /api/capital-gates), not an observation.
  let paperSafety: LedgerFacts["paper_safety"];
  try {
    const pm = await resolvePaperModeState({
      redis: deps.redis,
      enabledChainIds: deps.enabledChainIds,
      logger: deps.logger,
    });
    paperSafety = {
      paper_mode_active: pm.enabled,
      capital_exposure_usd: ZERO_CAPITAL_USD,
      source: "resolvePaperModeState (Redis arbx:papermode:*) + structural shadow invariant",
      confidence: pm.confidence,
      degraded: pm.degraded,
    };
  } catch (e) {
    paperSafety = {
      paper_mode_active: null,
      capital_exposure_usd: ZERO_CAPITAL_USD,
      source: "resolvePaperModeState (Redis arbx:papermode:*)",
      available: false,
      reason: (e as Error).message,
    };
  }

  // (4) Circuit breakers — live CB state lives on selector-api /metrics
  // (arbx_cb_state) and is NOT importable from the api-server. Recorded as
  // unavailable — never summarized from a source we cannot read (RULE 00).
  const circuitBreakers: Record<string, unknown> = {
    available: false,
    source: "selector-api /metrics (arbx_cb_state) — not importable from api-server",
    reason: "live circuit-breaker state is exported by selector-api Prometheus metrics; no importable read path exists",
  };

  return {
    schema_version: LEDGER_SCHEMA_VERSION,
    generated_by: "api-server:go-no-go",
    blockers,
    paper_shadow: paperShadow,
    paper_safety: paperSafety,
    circuit_breakers: circuitBreakers,
  };
}
