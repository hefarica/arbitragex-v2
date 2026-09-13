// CB-02 (2026-09-07)
/**
 * control-board — operator Control Board runtime control plane (WO CB-02).
 *
 * Endpoints (both gated by V-AT-1 admin token — the edge adminProxy on
 * `edge/worker/src/index.ts` translates the browser httpOnly session cookie
 * into the upstream `x-arbx-admin-token` and proxies these verbatim):
 *
 *   GET  /api/v1/control-board
 *     Live snapshot of the module/switch census. Serves the CB-03 wire
 *     contract (`ControlBoardLed.tsx` ControlBoardSnapshotSchema) EXACTLY:
 *     { generated_at, modules[] } — nothing more, nothing invented.
 *
 *   PUT  /api/v1/control-board   body { id, on, reason }
 *     Operator toggle of a CLASS-A (runtime-pollable) module. Sovereignty
 *     gates, in order:
 *       1. admin token (requireAdminToken — server is the law, the UI lock
 *          is only defense in depth);
 *       2. body validated server-side, `reason` non-empty AFTER TRIM
 *          (it is the mandatory quién/cuándo/por qué audit record);
 *       3. module must exist in the CB-01 census (unknown module ⇒ deny —
 *          without classification the server cannot know what it toggles);
 *       4. §34.3 terminus denylist (CB-05-PROPUESTA-B §7) ⇒ 403 ALWAYS,
 *          even if the census mislabels it class A (defense in depth);
 *       5. class C ⇒ 403 (capital/broadcast/live-flip is operator-gated
 *          §34.3 — NEVER toggleable from the board);
 *       6. class B ⇒ 409 (boot-time env — CB-05 proposal flow, restart);
 *       7. class A control_key must resolve INSIDE the board's own Redis
 *          namespace `arbx:controlboard:*` (CB-02-DISENO §1.3-4/INV-CB02-4,
 *          single-writer sovereignty — defense in depth: a census that
 *          points a class-A row at a foreign key, e.g. the JSON
 *          `arbx:killswitch` state, can never make this board corrupt it);
 *       7b. CB-02-DISENO §4.3/GATE-CB02-1 not-spawned gate: a census row
 *           bound to the route-scanner toggle key with `declared_on:false`
 *           means the worker was never spawned (spawn is env-gated at boot,
 *           ARBX_ROUTE_SCANNER_MODE) — 409 `module_not_spawned`: the toggle
 *           key would have no consumer (the CB-05 flow owns enabling the
 *           spawn). Explicit false only; an unknown boot state stays
 *           toggleable and the LED honestly shows DESCONOCIDO;
 *       8. INSERT into `audit_log` (migration 011 — CB-02-DISENO §5/§15-R1)
 *          BEFORE any Redis write. Audit failure ⇒ 500 and NO Redis
 *          mutation. The write itself is the idempotent SET of the toggle
 *          key PLUS the approval-registry HSET (§4.3, below). If either
 *          Redis write fails, a COMPENSATORY row
 *          `action='control_board.toggle_failed'` is INSERTed (R8: the
 *          ledger never claims a change that did not land) — the original
 *          row is NEVER UPDATEd: audit_log is append-only by doctrine and
 *          by `REVOKE UPDATE, DELETE` (011:21).
 *
 *   GET  /api/v1/control-board/drift          [CB-04 (2026-09-07)]
 *     Full sovereign drift-guard diff (the banner's diff_href target):
 *     per-module approved vs observed, exact-diff audit rows and class-A
 *     reverts. See services/control-board-drift.ts for the semantics
 *     ("ni Dios mueva si no es mi deseo" — the board is the operator's
 *     approval registry). Both snapshot responses (GET and PUT) also carry
 *     an optional `drift` banner field (ControlBoardDriftSchema, CB-03).
 *
 * Data sources (RULE 00 — fail-honest R8, zero fabrication):
 *   - Census identity/classification/verified state: Redis key
 *     `arbx:config:control_board`, published by the CB-01 census publisher
 *     (same pattern as `arbx:config:canonical_knobs` ← searcher-rs boot,
 *     canonical-knobs.ts). KEY ABSENT ⇒ snapshot is served EMPTY
 *     (`modules: []` — charter: "snapshot vacío si el censo no publica"),
 *     never fabricated LEDs; corrupted/invalid census ⇒ loud 503.
 *   - `declared_on` (operator desired state) for class A (CB-02-DISENO §4.2
 *     step 5): `approved[field] ?? census_default` — the operator's LAST
 *     APPROVED value from the hash `arbx:controlboard:approved` (field per
 *     module_id → JSON {on, reason, actor, updated_at}, written ONLY by
 *     this PUT alongside the toggle SET — §1.2/§7, the approval registry
 *     CB-04 defends). Without an approval on record the census-carried
 *     default stands. The approval DELIBERATELY wins over the live toggle
 *     key value: an external write to the key is DRIFT to be exposed
 *     (declared≠verified badge), never re-labeled as the operator's
 *     declaration ("ni Dios mueva si no es mi deseo"). Foreign/garbage hash
 *     entries ⇒ null (never guessed). Non-namespace keys (killswitch JSON,
 *     `api:` links) ⇒ census declared ?? null (CB-02-DISENO §15-R5).
 *   - `verified_on/verified_source/verified_at` (CB-02-DISENO §15-R3 —
 *     "LED = realidad, no foto"): for class-A modules whose control_key
 *     resolves to the board namespace, verified comes LIVE from the
 *     worker heartbeat `<key>:hb` (`{"state":"run"|"halted",block,ts}`,
 *     TTL 75s — absent/expired ⇒ verified_on null DESCONOCIDO, keeping the
 *     last-known verified_at, NEVER a stale true/false); for the killswitch
 *     link row it comes LIVE from the `arbx:killswitch` JSON (enabled=false
 *     ⇒ detection ACTIVE — inverted projection, CB-01 census precedent);
 *     for every other row it is CB-01's census observation, verbatim.
 *     null ⇒ DESCONOCIDO, NEVER false.
 *   - `last_actor/last_reason/updated_at`: latest operator toggle row in
 *     `audit_log` for the module (both `control_board.toggle` and the
 *     compensatory `control_board.toggle_failed`). PG unavailable ⇒ nulls
 *     (the board still renders honestly — enrichment degrades, state does not).
 *
 * Mode-safety (§34): this control plane NEVER touches the executor, wallets,
 * signing or broadcast paths. The terminus remains governed by
 * relays-client `live_exec_policy` default-deny (`MainnetRefused`) —
 * untouchable from here by construction (see terminus denylist above).
 *
 * Retargets applied per CB-02-DISENO §15 (the design adjudicated the
 * pre-design apply — see audits/control-board-2026-09-07/CB-02-API-APPLY.md):
 *   R1 — the board ledger IS the EXISTING `audit_log` (SINGULAR, migration
 *        011: actor/action/target_kind/target_id/before_state/after_state,
 *        monthly partitions 019, append-only via REVOKE UPDATE/DELETE 011:21).
 *        Migration 121 (`audit_logs` plural) is NOT created (design §5:
 *        duplicate audit surface refuted). Failed Redis SETs are recorded
 *        with a compensatory INSERT `control_board.toggle_failed`, never an
 *        UPDATE of the row already written.
 *   R3 — verified_* of class A is computed LIVE in this GET (heartbeat /
 *        live key), not served verbatim from the census photo.
 *   R4/R5/R9 — B⇒409, terminus denylist BEFORE the class check, reason
 *        trim().min(1).max(1000): adopted from the pre-design apply as-is.
 */
import type { Application, Request, Response } from "express";
import type pg from "pg";
import type { Redis } from "ioredis";
import { z } from "zod";
// CB-04 (2026-09-07) — sovereign drift guard (scan + revert + banner source).
import {
  createControlBoardDriftGuard,
  CONTROL_BOARD_DRIFT_ENDPOINT_PATH,
  type ControlBoardDriftBanner,
} from "../services/control-board-drift.js";

// CB-02 (2026-09-07) — census publication key (CB-01 publisher writes it;
// mirrors the `arbx:config:canonical_knobs` snapshot pattern).
export const CONTROL_BOARD_CENSUS_REDIS_KEY = "arbx:config:control_board";

// CB-02 (2026-09-07) — audit row action for board toggles (CB-05 §6.1).
export const CONTROL_BOARD_TOGGLE_AUDIT_ACTION = "control_board.toggle";

// CB-02 (2026-09-07) — CB-02-DISENO §15-R1: compensatory row for a toggle
// whose Redis SET failed (append-only ledger — the original row is never
// UPDATEd; 011:21 REVOKEs it). Reserved vocabulary shared with CB-04.
export const CONTROL_BOARD_TOGGLE_FAILED_AUDIT_ACTION = "control_board.toggle_failed";

// CB-02 (2026-09-07) — CB-02-DISENO §1.2/§1.3-4: the board's OWN Redis
// namespace. `arbx:controlboard:*` keys are written EXCLUSIVELY by this
// endpoint's PUT (single writer — sovereignty); nothing else on the fleet
// may write them. A census control_key outside this namespace is never
// written by the board (see PUT gate 7 / INV-CB02-4).
export const CONTROL_BOARD_REDIS_NAMESPACE = "arbx:controlboard:";

// CB-02 (2026-09-07) — CB-02-DISENO §1.2/R3/R5: live verification keys. The
// worker heartbeat lives at `<toggle-key>:hb` (SETEX 75s, JSON
// {"state":"run"|"halted",block,ts}); the kill-switch link row reads the
// live `arbx:killswitch` JSON (its canonical toggle stays at /admin/killswitch).
const CONTROL_BOARD_HB_SUFFIX = ":hb";
const KILLSWITCH_REDIS_KEY = "arbx:killswitch";

// CB-02 (2026-09-07) — CB-02-DISENO §1.2/§7 (D7): the operator APPROVAL
// REGISTRY hash. Field per module_id → JSON {on, reason, actor, updated_at};
// written EXCLUSIVELY by this PUT alongside the toggle SET (§4.3 — INV-CB02-4:
// no other writer). The GET serves class-A `declared_on` from it
// (approved ?? census default, §4.2 step 5) and CB-04 defends it as the
// record of the operator's approval (drift-guard/revert).
export const CONTROL_BOARD_APPROVED_HASH = "arbx:controlboard:approved";

// CB-02 (2026-09-07) — CB-02-DISENO §1.2/§3.3: THE route-scanner board toggle
// key (the Rust RuntimeToggleClient wiring target). The not-spawned gate keys
// on the resolved control_key — not the module id — so every census dialect
// that toggles the scanner (id `route_scanner_multihop` per CB-01, or the
// design's `route_scanner`) gets the gate. (The boot census key
// `arbx:config:boot_census` is deliberately NOT read here — §15-R10: it is a
// CB-01 input, not a TS dependency; the gate reads the census `declared_on`.)
const ROUTE_SCANNER_BOARD_TOGGLE_KEY = `${CONTROL_BOARD_REDIS_NAMESPACE}route_scanner`;

const CONTROL_BOARD_CLASS_VALUES = ["A", "B", "C"] as const;
type ControlBoardClass = (typeof CONTROL_BOARD_CLASS_VALUES)[number];

/**
 * CB-02 (2026-09-07) — §34.3 terminus denylist (defense in depth).
 * CB-05-PROPUESTA-B §7 documents the terminus/secret exclusion set; the
 * board-side hard floor is the terminus vocabulary of
 * `backend/relays-client/src/live_exec_policy.rs` (default-deny /
 * MainnetRefused — §34.3, untouchable). Any module id or control_key that
 * names the live-exec terminus or the signer is 403 regardless of the
 * census class label: a census mistake can never unlock the terminus.
 */
const TERMINUS_DENYLIST_EXACT = new Set([
  "ARBX_LIVE_EXEC_ENABLED",
  "ARBX_LIVE_EXEC_CHAINS",
  "SIM_SIGNER_ADDRESS",
]);
const TERMINUS_DENYLIST_SUBSTRINGS = ["live_exec", "live_mainnet", "mainnet_flip"];

function isTerminusDenylisted(id: string, controlKey: string | null | undefined): boolean {
  const values = [id, controlKey];
  for (const v of values) {
    if (!v) continue;
    const s = v.trim();
    if (TERMINUS_DENYLIST_EXACT.has(s.toUpperCase())) return true;
    const lower = s.toLowerCase();
    if (TERMINUS_DENYLIST_SUBSTRINGS.some((frag) => lower.includes(frag))) return true;
  }
  return false;
}

// ─── Census schema (published by CB-01) ─────────────────────────────────────

const CensusModuleSchema = z.object({
  /** stable module key, e.g. "route_scanner_multihop" */
  id: z.string().min(1).max(200),
  /** deployment-visible name */
  name: z.string().min(1).max(300),
  description: z.string().max(2000).nullish(),
  module_class: z.enum(CONTROL_BOARD_CLASS_VALUES),
  /** class A → runtime Redis control_key; B → boot env var; C → §34.3 gate */
  control_key: z.string().min(1).max(500).nullish(),
  telemetry_href: z.string().max(1000).nullish(),
  /** operator-declared desired state for B/C (census-carried). For class A
   *  the LIVE control_key value wins (the board is the writer). */
  declared_on: z.boolean().nullish(),
  /** CB-01 verified state — absent ⇒ DESCONOCIDO (null), never false. */
  verified_on: z.boolean().nullish(),
  verified_source: z.string().max(500).nullish(),
  verified_at: z.string().max(64).nullish(),
});
type CensusModule = z.infer<typeof CensusModuleSchema>;

const CensusSchema = z.object({
  modules: z.array(CensusModuleSchema),
});

// ─── PUT body schema — reason non-empty AFTER TRIM (server-side law) ────────

const PutToggleSchema = z.object({
  id: z.string().min(1).max(200),
  on: z.boolean(),
  reason: z.string().trim().min(1).max(1000),
});

// ─── CB-03 wire module (ControlBoardLed.tsx contract, explicit nulls) ───────

interface ControlBoardModuleWire {
  id: string;
  name: string;
  description: string | null;
  module_class: ControlBoardClass;
  control_key: string | null;
  declared_on: boolean | null;
  verified_on: boolean | null;
  verified_source: string | null;
  verified_at: string | null;
  updated_at: string | null;
  last_reason: string | null;
  last_actor: string | null;
  telemetry_href: string | null;
}

interface ControlBoardSnapshotWire {
  generated_at: string;
  modules: ControlBoardModuleWire[];
}

// ─── Audit row (audit_log, migration 011 — CB-02-DISENO §15-R1) ─────────────

interface ToggleAuditPayload {
  wo: "CB-02";
  module: string;
  on: boolean;
  reason: string;
  control_key: string;
  at: string;
  /** true at the toggle INSERT; the compensatory toggle_failed row carries
   *  false + redis_error when the SET did not land (append-only: the original
   *  row is never UPDATEd — 011:21 REVOKEs it) */
  applied: boolean;
}

// ─── Shared helpers ─────────────────────────────────────────────────────────

/**
 * CB-02 (2026-09-07) — parse the declared state written by this board.
 * Strict: exactly "true"/"false" (the only values PUT writes). Anything
 * else — including a foreign value someone else wrote — is NOT interpreted
 * (RULE 00: return null = no declarado, never guessed).
 */
export function parseDeclaredValue(raw: string | null): boolean | null {
  if (raw === "true") return true;
  if (raw === "false") return false;
  return null;
}

// ─── CB-02-DISENO §15-R3/R5/G4 — control_key resolution + live verification ──

/** CB-02 (2026-09-07) — what a class-A `control_key` resolves to. */
export type ResolvedControlKey =
  /** board-owned toggle key, e.g. `arbx:controlboard:route_scanner` */
  | { kind: "board_toggle"; key: string }
  /** kill-switch link row (live LED, toggle lives at /admin/killswitch — R5) */
  | { kind: "killswitch"; key: string }
  /** everything else (`api:…`, `env:…`, `db:…`, foreign redis keys, null) */
  | { kind: "external"; key: null };

/**
 * CB-02 (2026-09-07) — resolve a census `control_key` to a raw Redis key.
 * Accepts BOTH census dialects: the bare key (`arbx:controlboard:…` — CB-01
 * convention) and the `redis:`-prefixed registry form (CB-02-DISENO §8);
 * the optional `redis:` prefix is stripped, everything else is verbatim.
 * A control_key naming a key OUTSIDE the board namespace resolves to
 * `external` — the board never reads its declared state from, and never
 * writes to, a foreign key (INV-CB02-4).
 */
export function resolveControlKey(controlKey: string | null | undefined): ResolvedControlKey {
  if (!controlKey) return { kind: "external", key: null };
  const key = controlKey.startsWith("redis:") ? controlKey.slice("redis:".length) : controlKey;
  if (key.startsWith(CONTROL_BOARD_REDIS_NAMESPACE)) return { kind: "board_toggle", key };
  if (key === KILLSWITCH_REDIS_KEY) return { kind: "killswitch", key };
  return { kind: "external", key: null };
}

/** CB-02 (2026-09-07) — CB-02-DISENO §1.2/§3.3 worker heartbeat payload
 * (SETEX 75s ≈ 6 blocks of grace). Strict: only "run"/"halted" count —
 * anything else is a foreign/garbage value and is NOT interpreted (R8). */
export function parseHeartbeat(raw: string | null): { on: boolean | null; at: string | null } {
  if (raw === null) return { on: null, at: null };
  try {
    const hb = JSON.parse(raw) as { state?: unknown; ts?: unknown; updated_at?: unknown };
    if (hb.state !== "run" && hb.state !== "halted") return { on: null, at: null };
    const at = typeof hb.ts === "string" ? hb.ts : typeof hb.updated_at === "string" ? hb.updated_at : null;
    return { on: hb.state === "run", at };
  } catch {
    return { on: null, at: null };
  }
}

/**
 * CB-02 (2026-09-07) — strict parse of an `arbx:controlboard:approved` hash
 * field (JSON `{on: boolean, reason, actor, updated_at}` — the only shape
 * this PUT writes). Anything else — including a value some foreign writer
 * put in the hash — is NOT interpreted (RULE 00: null = no approval on
 * record, never guessed).
 */
export function parseApprovedEntry(raw: string | null | undefined): boolean | null {
  if (raw === null || raw === undefined) return null;
  try {
    const a = JSON.parse(raw) as { on?: unknown };
    if (typeof a.on !== "boolean") return null;
    return a.on;
  } catch {
    return null;
  }
}

/**
 * CB-02 (2026-09-07) — live verified state from the `arbx:killswitch` JSON
 * (`{"enabled":bool,…}` — observed byte-exact on the VPS, CB-01-CROSS-EXAM
 * §1e). INVERTED projection (CB-01 census precedent): enabled=false ⇒ the
 * module LED is ON (detection ACTIVE); enabled=true ⇒ armed ⇒ detection
 * stopped ⇒ LED off. Unparseable/absent ⇒ null DESCONOCIDO (never guessed).
 */
export function parseKillswitchVerified(raw: string | null): { on: boolean | null; at: string | null } {
  if (raw === null) return { on: null, at: null };
  try {
    const ks = JSON.parse(raw) as { enabled?: unknown; updated_at?: unknown };
    if (typeof ks.enabled !== "boolean") return { on: null, at: null };
    return {
      on: !ks.enabled,
      at: typeof ks.updated_at === "string" ? ks.updated_at : null,
    };
  } catch {
    return { on: null, at: null };
  }
}

type CensusLoad =
  | { ok: true; modules: CensusModule[] }
  | { ok: false; absent: true }
  | { ok: false; absent: false; status: number; body: Record<string, unknown> };

// CB-04 (2026-09-07) — exported for the sovereign drift-guard service
// (services/control-board-drift.ts): ONE census loader so the snapshot and
// the drift scan can never disagree about what the census says.
export async function loadCensus(
  redis: Redis,
  logger: { warn: (obj: object, msg?: string) => void },
): Promise<CensusLoad> {
  let raw: string | null;
  try {
    raw = await redis.get(CONTROL_BOARD_CENSUS_REDIS_KEY);
  } catch (e) {
    logger.warn({ event: "control_board.census_redis_get_failed", err: (e as Error).message });
    return { ok: false, absent: false, status: 503, body: { error: "redis_unavailable" } };
  }
  if (raw === null) {
    // Charter: "snapshot vacío si el censo no publica" — GET serves [],
    // PUT refuses (cannot classify what it would toggle).
    return { ok: false, absent: true };
  }
  let parsedJson: unknown;
  try {
    parsedJson = JSON.parse(raw);
  } catch (e) {
    logger.warn({ event: "control_board.census_parse_failed", err: (e as Error).message });
    return {
      ok: false,
      absent: false,
      status: 503,
      body: { error: "census_corrupted", detail: "census JSON unparseable — never served partially" },
    };
  }
  const parsed = CensusSchema.safeParse(parsedJson);
  if (!parsed.success) {
    const issues = parsed.error.issues
      .slice(0, 3)
      .map((i) => `${i.path.join(".") || "<root>"}: ${i.message}`)
      .join("; ");
    logger.warn({ event: "control_board.census_schema_invalid", issues });
    return {
      ok: false,
      absent: false,
      status: 503,
      body: { error: "census_schema_invalid", detail: issues },
    };
  }
  return { ok: true, modules: parsed.data.modules };
}

interface LastToggleRow {
  actor: string | null;
  reason: string | null;
  updated_at: string | null;
}

/**
 * Latest operator toggle row per module (one DISTINCT ON query).
 * CB-02-DISENO §15-R1: the ledger is `audit_log` (SINGULAR, migration 011)
 * with target_kind/target_id/before_state/after_state columns; the operator
 * columns include BOTH the successful toggles and the compensatory
 * `control_board.toggle_failed` rows (design §7 fine-tune) while CB-04's
 * guard events (`control_board.drift*`) stay excluded. Fail-honest: PG
 * unavailable/query error ⇒ per-module nulls — the board still renders
 * state; only the quién/por qué enrichment degrades.
 */
async function loadLastToggles(
  pool: pg.Pool | null,
  moduleIds: string[],
  logger: { warn: (obj: object, msg?: string) => void },
): Promise<Map<string, LastToggleRow>> {
  const out = new Map<string, LastToggleRow>();
  if (!pool || moduleIds.length === 0) return out;
  try {
    const q = await pool.query(
      `SELECT DISTINCT ON (target_id) target_id, actor, after_state, created_at
         FROM audit_log
        WHERE target_kind = 'control_board_module'
          AND action = ANY($1::text[])
          AND target_id = ANY($2::text[])
        ORDER BY target_id, created_at DESC`,
      [[CONTROL_BOARD_TOGGLE_AUDIT_ACTION, CONTROL_BOARD_TOGGLE_FAILED_AUDIT_ACTION], moduleIds],
    );
    for (const row of q.rows as Array<{
      target_id: string;
      actor: string | null;
      after_state: unknown;
      created_at: Date | string | null;
    }>) {
      let afterState: unknown = row.after_state;
      if (typeof afterState === "string") {
        try {
          afterState = JSON.parse(afterState);
        } catch {
          afterState = null;
        }
      }
      const p = (afterState ?? {}) as { reason?: unknown };
      out.set(row.target_id, {
        actor: row.actor ?? null,
        reason: typeof p.reason === "string" ? p.reason : null,
        updated_at:
          row.created_at instanceof Date
            ? row.created_at.toISOString()
            : row.created_at !== null && row.created_at !== undefined
              ? String(row.created_at)
              : null,
      });
    }
  } catch (e) {
    logger.warn({ event: "control_board.audit_read_failed", err: (e as Error).message });
  }
  return out;
}

type SnapshotResult =
  | { ok: true; snapshot: ControlBoardSnapshotWire }
  | { ok: false; status: number; body: Record<string, unknown> };

/**
 * Build the live snapshot. Charter RULE 00: census absent ⇒ EMPTY modules
 * (never fabricated); a module without verified data carries verified_on
 * null (DESCONOCIDO), never false.
 */
async function buildSnapshot(deps: {
  pool: pg.Pool | null;
  redis: Redis | null;
  logger: { warn: (obj: object, msg?: string) => void };
}): Promise<SnapshotResult> {
  const { pool, redis, logger } = deps;
  if (!redis) {
    return { ok: false, status: 503, body: { error: "redis_unavailable" } };
  }
  const census = await loadCensus(redis, logger);
  if (!census.ok) {
    if (census.absent) {
      return { ok: true, snapshot: { generated_at: new Date().toISOString(), modules: [] } };
    }
    return { ok: false, status: census.status, body: census.body };
  }
  const lastToggles = await loadLastToggles(pool, census.modules.map((m) => m.id), logger);

  // CB-02 (2026-09-07) — CB-02-DISENO §4.2 step 2/step 5: the approval-registry
  // hash is the DECLARED source for class-A board-toggle rows
  // (`approved[field] ?? census_default`). Fail-soft like the PG enrichment:
  // an unreadable hash degrades declared to the census default and never
  // blocks the snapshot (the state itself lives in census + heartbeat).
  const approvedRaw = new Map<string, string>();
  try {
    const hash = await redis.hgetall(CONTROL_BOARD_APPROVED_HASH);
    for (const [field, value] of Object.entries(hash)) approvedRaw.set(field, value);
  } catch (e) {
    logger.warn({ event: "control_board.approved_read_failed", err: (e as Error).message });
  }

  // CB-02 (2026-09-07) — CB-02-DISENO §15-R3: class-A verified state is
  // computed LIVE per resolved control_key, NOT served from the census
  // photo ("LED = realidad, no foto" — RULE 00):
  //   board_toggle key → verified = the worker heartbeat `<key>:hb`
  //     (absent/expired ⇒ verified_on null DESCONOCIDO keeping the census
  //     last-known verified_at, R8). declared comes from the approved hash
  //     (§4.2 step 5 — see above), NEVER from the live toggle key: an
  //     external write to the key is drift to EXPOSE, not a declaration;
  //   killswitch link key (R5) → verified = live `arbx:killswitch` JSON with
  //     the inverted projection (enabled=false ⇒ detection ACTIVE);
  //   external/absent control_key → NO live signal the api-server can read —
  //     the CB-01 census observation stands, verbatim.
  // Parallel, non-blocking (C-S-E asincronía paralela). An individual key
  // failure degrades THAT module honestly (null), never a 500 for the board.
  interface LiveVerified {
    on: boolean | null;
    source: string;
    at: string | null;
  }
  interface LiveState {
    /** null ⇒ no live signal was readable ⇒ census fallback below */
    verified: LiveVerified | null;
  }
  const live = await Promise.all(
    census.modules.map(async (m): Promise<LiveState> => {
      if (m.module_class !== "A") return { verified: null };
      const resolved = resolveControlKey(m.control_key);
      if (resolved.kind === "board_toggle") {
        const hbKey = `${resolved.key}${CONTROL_BOARD_HB_SUFFIX}`;
        try {
          // The channel EXISTS by construction (the census declared a board
          // key) — so verified is ALWAYS live-channel data: absent/expired
          // heartbeat ⇒ on null DESCONOCIDO (never the census photo), and
          // the assembly keeps the census last-known verified_at.
          const hb = parseHeartbeat(await redis.get(hbKey));
          return { verified: { on: hb.on, source: `redis:${hbKey}`, at: hb.at } };
        } catch (e) {
          logger.warn({
            event: "control_board.live_read_failed",
            module: m.id,
            control_key: m.control_key,
            err: (e as Error).message,
          });
          return { verified: null };
        }
      }
      if (resolved.kind === "killswitch") {
        try {
          // Channel exists by construction (R5 link row) — absent key ⇒ on
          // null DESCONOCIDO, never the census photo.
          const ks = parseKillswitchVerified(await redis.get(resolved.key));
          return { verified: { on: ks.on, source: `redis:${resolved.key}`, at: ks.at } };
        } catch (e) {
          logger.warn({
            event: "control_board.live_read_failed",
            module: m.id,
            control_key: m.control_key,
            err: (e as Error).message,
          });
          return { verified: null };
        }
      }
      return { verified: null };
    }),
  );

  const modules: ControlBoardModuleWire[] = census.modules.map((m, i) => {
    const last = lastToggles.get(m.id) ?? { actor: null, reason: null, updated_at: null };
    const liveRow: LiveState = live[i] ?? { verified: null }; // noUncheckedIndexedAccess guard
    // CB-02 (2026-09-07) — §4.2 step 5: declared = approved ?? census_default
    // for class-A board-toggle rows; census ?? null for everything else. The
    // live toggle key is deliberately NOT a declared source (sovereignty: a
    // foreign write is drift, never the operator's declaration).
    const declaredOn =
      m.module_class === "A" && resolveControlKey(m.control_key).kind === "board_toggle"
        ? (parseApprovedEntry(approvedRaw.get(m.id)) ?? m.declared_on ?? null)
        : (m.declared_on ?? null);
    return {
      id: m.id,
      name: m.name,
      description: m.description ?? null,
      module_class: m.module_class,
      control_key: m.control_key ?? null,
      declared_on: declaredOn,
      verified_on: liveRow.verified ? liveRow.verified.on : (m.verified_on ?? null),
      verified_source: liveRow.verified ? liveRow.verified.source : (m.verified_source ?? null),
      // Live read without a timestamp (e.g. foreign hb payload) keeps the
      // census last-known ts — "última verificación <ts>, ahora DESCONOCIDO".
      verified_at: liveRow.verified ? (liveRow.verified.at ?? m.verified_at ?? null) : (m.verified_at ?? null),
      updated_at: last.updated_at,
      last_reason: last.reason,
      last_actor: last.actor,
      telemetry_href: m.telemetry_href ?? null,
    };
  });
  return { ok: true, snapshot: { generated_at: new Date().toISOString(), modules } };
}

// ─── Route mounting (mountAdminChains pattern, index.ts mount site) ─────────

export interface ControlBoardDeps {
  pool: pg.Pool | null;
  redis: Redis | null;
  requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
  adminToken: string;
  logger: {
    warn: (obj: object, msg?: string) => void;
    info: (obj: object, msg?: string) => void;
  };
  /** CB-04 (2026-09-07) — drift-guard scan cadence (default 60s). Optional:
   *  absent keeps the sovereign default; 0 keeps it too (additive wiring). */
  driftGuardIntervalMs?: number;
}

/** CB-04 (2026-09-07) — what mountControlBoard hands back so the wiring site
 *  can stop the drift guard on shutdown. Existing callers may ignore it. */
export interface MountedControlBoard {
  stopDriftGuard: () => void;
}

export function mountControlBoard(app: Application, deps: ControlBoardDeps): MountedControlBoard {
  const { pool, redis, requireAdminToken, adminToken, logger } = deps;

  // CB-04 (2026-09-07) — sovereign drift guard: periodic approved-vs-observed
  // scan (audit diff + class-A revert). Interval-only start (first tick fires
  // AFTER intervalMs, unref'd) so mounting never performs IO by itself.
  const driftGuard = createControlBoardDriftGuard(
    { pool, redis, logger },
    { intervalMs: deps.driftGuardIntervalMs },
  );

  // CB-04 (2026-09-07) — attach the CB-03 banner field (ControlBoardDriftSchema)
  // to a snapshot response. null until the first scan — CB-03 renders the
  // honest "sin dato" state, never a fabricated green.
  const withDriftBanner = (
    snapshot: ControlBoardSnapshotWire,
  ): ControlBoardSnapshotWire & { drift: ControlBoardDriftBanner | null } => {
    return { ...snapshot, drift: driftGuard.getBanner() };
  };

  // GET — census snapshot (admin-gated: the edge adminProxy forwards the
  // session token for both verbs; direct callers must present it too).
  app.get("/api/v1/control-board", requireAdminToken(adminToken), async (_req: Request, res: Response) => {
    const r = await buildSnapshot({ pool, redis, logger });
    if (r.ok) {
      res.status(200).json(withDriftBanner(r.snapshot));
    } else {
      res.status(r.status).json(r.body);
    }
  });

  // CB-04 (2026-09-07) — full drift diff (the banner's diff_href target).
  // Admin-gated like the board itself. scanIfDue is TTL-throttled (5s) so
  // CB-03 polling this can never stampede the scan; the response is the
  // complete per-module diff payload (RULE 00: statuses census_absent /
  // audit_unreadable are served verbatim — "no computable", never green).
  app.get(CONTROL_BOARD_DRIFT_ENDPOINT_PATH, requireAdminToken(adminToken), async (_req: Request, res: Response) => {
    const report = await driftGuard.scanIfDue();
    if (report === null) {
      // Fail-honest (R8): no scan has completed yet — never a fabricated report.
      res.status(503).json({
        error: "drift_report_unavailable",
        detail: "no drift scan has completed yet — retry after the guard's first tick",
      });
      return;
    }
    res.status(200).json(report);
  });

  // PUT — operator toggle (class A only; sovereignty gates in docblock).
  app.put("/api/v1/control-board", requireAdminToken(adminToken), async (req: Request, res: Response) => {
    // Gate 1: dependencies. Without PG there is NO audit row possible, and
    // without audit there is NO toggle (audit-first is a hard precondition,
    // not a best-effort side-effect like the legacy writeAudit swallow).
    if (!pool) {
      res.status(503).json({
        error: "db_unavailable",
        detail: "audit_log lives in PG; without PG the toggle cannot be audited, so it is refused",
      });
      return;
    }
    if (!redis) {
      res.status(503).json({ error: "redis_unavailable" });
      return;
    }

    // Gate 2: body — reason non-empty AFTER TRIM, validated server-side.
    const parsed = PutToggleSchema.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({ error: "invalid_body", issues: parsed.error.flatten() });
      return;
    }
    const { id, on, reason } = parsed.data;

    // Gate 3: census — unknown module can never be classified ⇒ deny.
    const census = await loadCensus(redis, logger);
    if (!census.ok) {
      if (census.absent) {
        res.status(503).json({
          error: "census_not_published",
          detail:
            "CB-01 census not published at arbx:config:control_board — refusing to toggle an unclassifiable module (fail-safe)",
        });
        return;
      }
      res.status(census.status).json(census.body);
      return;
    }
    const module = census.modules.find((m) => m.id === id);
    if (!module) {
      res.status(404).json({ error: "module_not_in_census", detail: `module '${id}' not found in the CB-01 census` });
      return;
    }

    // Gate 4: §34.3 terminus denylist — ALWAYS 403, even mislabeled class A.
    if (isTerminusDenylisted(module.id, module.control_key)) {
      logger.warn({ event: "control_board.terminus_denied", module: module.id, control_key: module.control_key ?? null });
      res.status(403).json({
        error: "terminus_denied_c343",
        detail:
          "§34.3 capital/broadcast terminus (live_exec_policy default-deny / MainnetRefused): operator-gated, NEVER toggleable from the control board",
      });
      return;
    }

    // Gate 5/6: class C locked §34.3; class B requires restart (CB-05 flow).
    if (module.module_class === "C") {
      res.status(403).json({
        error: "module_class_c_locked",
        detail:
          "categoría C (capital/broadcast/live-flip) — candado del operador §34.3; cambio solo con autorización operativa explícita, nunca desde el board",
      });
      return;
    }
    if (module.module_class === "B") {
      res.status(409).json({
        error: "module_class_b_restart_required",
        detail:
          "clase B (boot-time env): el toggle runtime no aplica — usa el flujo de propuesta CB-05 (diff .env + restart por el pipeline de deploy)",
      });
      return;
    }

    // Class A: the census MUST declare the runtime control_key the workers
    // poll. Refusing to write an undeclared key keeps the board from
    // inventing a knob nobody consumes.
    if (!module.control_key) {
      res.status(503).json({
        error: "census_invalid_class_a",
        detail: `census declares class-A module '${module.id}' without control_key — census defect, refusing to write`,
      });
      return;
    }
    // CB-02 (2026-09-07) — CB-02-DISENO §1.3-4/INV-CB02-4 (cross-exam G4):
    // the resolved key MUST live inside the board's OWN namespace
    // `arbx:controlboard:*` — the single-writer sovereignty floor. A census
    // defect that points a class-A row at a foreign key (the killswitch JSON
    // state, an `api:` link, a worker channel) can never make this board
    // write garbage over it. Defense in depth: the UI blocks it too, but the
    // server is the law.
    const resolvedControl = resolveControlKey(module.control_key);
    if (resolvedControl.kind !== "board_toggle") {
      res.status(503).json({
        error: "control_key_not_board_writable",
        detail:
          `class-A control_key '${module.control_key}' resolves outside the board namespace ` +
          `'${CONTROL_BOARD_REDIS_NAMESPACE}*' — this board never writes foreign keys ` +
          "(single-writer sovereignty, CB-02-DISENO §1.3-4); census defect, refusing to write",
      });
      return;
    }
    const controlKey = resolvedControl.key;

    // CB-02 (2026-09-07) — CB-02-DISENO §4.3/GATE-CB02-1: route-scanner
    // not-spawned gate. The worker's SPAWN is env-gated at boot
    // (ARBX_ROUTE_SCANNER_MODE — spawn gate route_scanner_worker.rs:808-810;
    // the run gate is THIS board's toggle key). A census whose `declared_on`
    // is false for the module bound to the scanner toggle key says the
    // worker was never spawned: writing the run-gate key would feed a
    // consumer that does not exist. Keyed on the RESOLVED toggle key (not
    // the module id) so every census dialect that binds the scanner gets the
    // gate. Explicit false only — an unknown/null boot state stays
    // toggleable and the LED then honestly shows DESCONOCIDO until the
    // worker exists (fail-honest, not fail-closed: the design gate is
    // "census mode=off", §4.3).
    if (controlKey === ROUTE_SCANNER_BOARD_TOGGLE_KEY && module.declared_on === false) {
      res.status(409).json({
        error: "module_not_spawned",
        detail:
          "route_scanner worker not spawned (census declared_on:false — ARBX_ROUTE_SCANNER_MODE=off at boot): " +
          "the toggle key would have no consumer; enable the spawn via the CB-05 proposal flow (env + restart)",
      });
      return;
    }

    // Before-state for the audit trail (strict parse; a Redis read failure
    // aborts — writing without knowing the previous state is not honest).
    let beforeDeclared: boolean | null;
    try {
      beforeDeclared = parseDeclaredValue(await redis.get(controlKey));
    } catch (e) {
      logger.warn({ event: "control_board.before_read_failed", module: id, err: (e as Error).message });
      res.status(503).json({ error: "redis_unavailable" });
      return;
    }

    // Gate 8: audit FIRST — INSERT into `audit_log` (migration 011 columns,
    // CB-02-DISENO §15-R1) BEFORE the Redis write. Failure here means NO
    // state change, ever.
    const actor = req.header("x-arbx-actor") ?? "admin";
    const at = new Date().toISOString();
    const beforeState = { on: beforeDeclared };
    const payload: ToggleAuditPayload = {
      wo: "CB-02",
      module: id,
      on,
      reason,
      control_key: controlKey,
      at,
      applied: true,
    };
    try {
      await pool.query(
        `INSERT INTO audit_log (actor, action, target_kind, target_id, before_state, after_state)
         VALUES ($1, $2, 'control_board_module', $3, $4::jsonb, $5::jsonb)`,
        [
          actor,
          CONTROL_BOARD_TOGGLE_AUDIT_ACTION,
          id,
          JSON.stringify(beforeState),
          JSON.stringify(payload),
        ],
      );
    } catch (e) {
      logger.warn({ event: "control_board.audit_insert_failed", module: id, err: (e as Error).message });
      res.status(500).json({
        error: "audit_write_failed",
        detail: "audit_log INSERT failed — Redis NOT written (audit-first sovereignty gate)",
      });
      return;
    }

    // Redis write — idempotent SET of exactly "true"/"false" (the format
    // parseDeclaredValue reads; the board is the only writer) PLUS the
    // approval-registry HSET (CB-02-DISENO §4.3/§1.2/§7): field per
    // module_id → JSON {on, reason, actor, updated_at}, the operator's last
    // approved value (the GET's declared source and CB-04's drift/revert
    // record). Both writes are `arbx:controlboard:*` namespace writes that
    // the audit row above already covers (INV-CB02-2); a failure of EITHER
    // is a failed toggle → compensatory row below (the ledger never claims
    // a partially-landed approval; a SET that landed without its approval
    // hash is honestly re-assertable by re-toggling — idempotent).
    try {
      await redis.set(controlKey, on ? "true" : "false");
      await redis.hset(
        CONTROL_BOARD_APPROVED_HASH,
        id,
        JSON.stringify({ on, reason, actor, updated_at: at }),
      );
    } catch (e) {
      // R8 + CB-02-DISENO §15-R1: the ledger must never claim a change that
      // did not land — and the ledger is APPEND-ONLY (011:21 REVOKE UPDATE,
      // DELETE), so the mechanism is a COMPENSATORY INSERT row
      // `control_board.toggle_failed`, never an UPDATE of the row above
      // (best-effort; failure to record it is logged loudly for CB-04
      // drift forensics).
      const redisError = (e as Error).message;
      logger.warn({ event: "control_board.redis_set_failed", module: id, err: redisError });
      try {
        await pool.query(
          `INSERT INTO audit_log (actor, action, target_kind, target_id, before_state, after_state)
           VALUES ($1, $2, 'control_board_module', $3, $4::jsonb, $5::jsonb)`,
          [
            actor,
            CONTROL_BOARD_TOGGLE_FAILED_AUDIT_ACTION,
            id,
            JSON.stringify(beforeState),
            JSON.stringify({ ...payload, applied: false, redis_error: redisError }),
          ],
        );
      } catch (e2) {
        logger.warn({
          event: "control_board.audit_compensate_failed",
          module: id,
          err: (e2 as Error).message,
        });
      }
      res.status(500).json({
        error: "redis_write_failed",
        detail:
          "audited as control_board.toggle_failed (applied:false) — the toggle did not fully land " +
          "(SET/HSET of the toggle key + approval hash); re-assert idempotently once Redis recovers",
      });
      return;
    }

    logger.info({ event: "control_board.toggled", module: id, on, actor, control_key: controlKey });
    // Fresh snapshot in the response — CB-03 re-renders from it directly.
    const snapshot = await buildSnapshot({ pool, redis, logger });
    if (snapshot.ok) {
      // CB-04 (2026-09-07) — banner rides along (last-scan truth; the guard
      // clears a module's drift on its next tick once the revert lands).
      res.status(200).json(withDriftBanner(snapshot.snapshot));
    } else {
      // Toggle DID land; the snapshot read failed — honest partial state,
      // surfaced verbatim (the client keeps its last real snapshot).
      res.status(snapshot.status).json(snapshot.body);
    }
  });

  // CB-04 (2026-09-07) — hand the wiring site a shutdown handle.
  return { stopDriftGuard: () => driftGuard.stop() };
}
