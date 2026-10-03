/**
 * =============================================================================
 * OPPORTUNITY STREAM — ordered, idempotent event envelope for the live card feed
 * =============================================================================
 *
 * STREAM-SEQ-01 (§11.4 of PROMPT_ARBITRAGEX_BUSQUEDA_AUTONOMA_FEES_264).
 *
 * WHY THIS EXISTS — the measured defect
 * -------------------------------------
 * `broadcastOpportunity` emits the RAW PG row as `new_opportunity`
 * (websocket.ts:506-515 documents that contract deliberately). That leg carries
 * NO order authority: a NOTIFY row, a reconnect replay and an overlapping REST
 * snapshot all arrive as identical-looking objects, so the consumer can only
 * apply them in ARRIVAL order — which is exactly what §11.4 forbids ("no
 * permitas que un evento antiguo sobrescriba uno nuevo"). The frontend buffer
 * documented that limit in its own header ("last ARRIVAL wins the CONTENT").
 *
 * This module adds the missing authority WITHOUT touching the raw leg: the same
 * row is also published as an `opportunity_stream_event` envelope carrying
 *
 *   · `strategy_key` / `plan_key` — stable keys per strategy and per plan,
 *   · `seq`                       — monotonic per strategy (the order authority),
 *   · `snapshot_id` / `emitted_at`,
 *   · `progress`                  — what the server can certify over its own
 *                                   session, and nothing else.
 *
 * RULE 00 / R8 — no fabrication, and this is the part that matters most here:
 *   · the envelope NEVER substitutes a figure. `payload` is the byte-identical
 *     row the raw leg already carried; the envelope only stamps identity+order.
 *   · a metric the server cannot measure is OMITTED from `progress`, so the
 *     card renders ABSENT with its reason. It is never sent as 0 — a zero over
 *     a metric nobody measured is the false-zero failure (R10 E2E-COMPUTE
 *     GUARD, extended from the drift precedent to this wire).
 *   · key derivation mirrors the frontend contract bit-for-bit
 *     (`frontend/lib/store/stream-contract.ts` + `route-key.ts`, itself the twin
 *     of the LIVE_QUERY `concat_ws` at opportunities-live.ts:295-303).
 *
 * The counters are honest about their scope: they are counts of what THIS
 * PROCESS emitted during its lifetime ("server_session"). They are not a
 * statement about the searcher's internal search budget, which this wire does
 * not carry — hence `budget_exhausted` is only ever declared from an explicit
 * rejection reason in the vocabulary below.
 */

/** Wire version. MUST equal `STREAM_SCHEMA_VERSION` in the frontend contract. */
export const STREAM_SCHEMA_VERSION = 2;

/** Socket.IO event name (room `opportunities`, same room as the raw leg). */
export const STREAM_EVENT_NAME = 'opportunity_stream_event';

export type StreamEventKind =
  | 'plan.updated'
  | 'plan.rejected'
  | 'strategy.progress'
  | 'strategy.cleared';

export interface StreamPublisherProgress {
  routes_explored?: number;
  sizes_quoted?: number;
  quotes_exact?: number;
  quotes_incomplete?: number;
  quotes_error?: number;
  budget_exhausted?: boolean;
  budget_reason?: string;
  snapshot_id?: string | null;
}

export interface OpportunityStreamEvent {
  schema_version: number;
  event_id: string;
  seq: number;
  strategy_key: string;
  plan_key: string;
  snapshot_id: string | null;
  emitted_at: string;
  kind: StreamEventKind;
  payload: Record<string, unknown>;
  progress?: StreamPublisherProgress;
}

/**
 * Rejection reasons that genuinely mean a search budget ran out. Deliberately a
 * closed vocabulary: inferring "budget exhausted" from a generic rejection
 * would turn an unrelated cause into a fabricated budget statement.
 * `expansion_budget_exhausted` / `path_budget_exhausted` are the two strings
 * `searcher-rs/src/agent_graph.rs:101,121` writes as its own `stopping_reason`;
 * `budget_exhausted` is the adapter-skip vocabulary of
 * `route_discovery/triangular_adapter.rs:131`.
 */
const BUDGET_EXHAUSTED_REASONS: ReadonlySet<string> = new Set([
  'budget_exhausted',
  'expansion_budget_exhausted',
  'path_budget_exhausted',
  'adapter_budget_exhausted',
]);

/** Marker for a row whose producer declared no notional (a STATE, not a 0). */
export const UNDECLARED_SIZE = 'undeclared';

function asString(v: unknown): string | null {
  return typeof v === 'string' && v.length > 0 ? v : null;
}

function asNumber(v: unknown): number | null {
  if (typeof v === 'number' && Number.isFinite(v)) return v;
  if (typeof v === 'string' && v.trim() !== '' && Number.isFinite(Number(v))) return Number(v);
  return null;
}

/** `chain_id|strategy_kind` — the frontend's `strategyKeyOf` twin. */
export function strategyKeyOfRow(row: Record<string, unknown>): string {
  const chain = row.chain_id == null ? '' : String(row.chain_id);
  const kind = row.strategy_kind == null ? '' : String(row.strategy_kind);
  return [chain, kind].join('|');
}

/**
 * The LIVE_QUERY `concat_ws('|', …)` twin (opportunities-live.ts:295-303):
 * every nullable segment COALESCEd to '' so the server and the SQL GROUP BY
 * cannot disagree about which detections are one route.
 */
export function routeGroupKeyOfRow(row: Record<string, unknown>): string {
  const s = (v: unknown): string => (v == null ? '' : String(v));
  return [
    s(row.chain_id),
    s(row.chain_id_out),
    s(row.strategy_kind),
    s(row.token_in),
    s(row.token_out),
    s(row.dex_a),
    s(row.dex_b),
  ].join('|');
}

/**
 * The notional the producer DECLARED for this row's economics
 * (`route_metadata.economics_amount_in_wei`, ECON-DECLARE-01). Exact wei string
 * only; anything else is "undeclared" — never a parsed float.
 */
export function declaredSizeWeiOfRow(row: Record<string, unknown>): string | null {
  const rm = row.route_metadata;
  if (rm == null || typeof rm !== 'object' || Array.isArray(rm)) return null;
  const wei = (rm as Record<string, unknown>).economics_amount_in_wei;
  if (typeof wei !== 'string') return null;
  const trimmed = wei.trim();
  return /^-?\d+$/.test(trimmed) ? trimmed : null;
}

/** `strategy#route@size` — the frontend's `planKeyOf` twin. */
export function planKeyOfRow(row: Record<string, unknown>): string {
  return `${strategyKeyOfRow(row)}#${routeGroupKeyOfRow(row)}@${
    declaredSizeWeiOfRow(row) ?? UNDECLARED_SIZE
  }`;
}

/**
 * Content digest used ONLY for server-side replay suppression: a byte-identical
 * re-emission of the same application must not consume a new `seq`, so the
 * client sees the SAME `event_id` and drops it as a duplicate. Covers identity +
 * lifecycle + the economics blob — the fields a real update changes.
 */
export function rowDigest(row: Record<string, unknown>): string {
  return JSON.stringify([
    row.id ?? null,
    row.status ?? null,
    row.rejection_reason ?? null,
    row.paper_status ?? null,
    row.detected_at ?? null,
    row.economics ?? null,
    row.simulated_net_profit_usd ?? null,
    row.expected_profit_usd ?? null,
    row.net_expected_profit_usd ?? null,
  ]);
}

interface StrategyProgressState {
  routes: Set<string>;
  sizes: Set<string>;
  quotesExact: number;
  quotesIncomplete: number;
  quotesError: number;
  /** Number of rows observed whose `economics` object was actually present. */
  withEconomics: number;
  lastDigest: string | null;
  lastEventId: string | null;
  lastSeq: number;
}

function blankProgressState(): StrategyProgressState {
  return {
    routes: new Set<string>(),
    sizes: new Set<string>(),
    quotesExact: 0,
    quotesIncomplete: 0,
    quotesError: 0,
    withEconomics: 0,
    lastDigest: null,
    lastEventId: null,
    lastSeq: -1,
  };
}

export interface OpportunityStreamProducerOptions {
  /** Injectable clock (ms epoch). Defaults to `Date.now`. */
  now?: () => number;
  /** Monotonic counter for `event_id` uniqueness across strategies. */
  eventCounterSeed?: number;
}

export interface OpportunityStreamProducer {
  /**
   * Build the envelope for one row, ALLOCATING a new `seq` for a new
   * application and REUSING the previous one for a byte-identical replay.
   * Returns `null` when the input is not a row object (nothing to stamp).
   */
  build(row: unknown): OpportunityStreamEvent | null;
  /** Name the source-of-truth snapshot the next events belong to (resync). */
  setSnapshotId(id: string | null): void;
  /** Progress the server can certify for one strategy (never a fabricated 0). */
  progressOf(strategyKey: string, snapshotId: string | null): StreamPublisherProgress;
  /** Test/ops seam: forget every counter. */
  reset(): void;
}

/**
 * Create the producer. Stateful ONLY in ordering/counters — it never mutates the
 * row it is handed (the raw leg's byte-identity is a documented contract).
 */
export function createOpportunityStreamProducer(
  options: OpportunityStreamProducerOptions = {},
): OpportunityStreamProducer {
  const now = options.now ?? (() => Date.now());
  let counter = options.eventCounterSeed ?? 0;
  let snapshotId: string | null = null;
  const byStrategy = new Map<string, StrategyProgressState>();

  const stateOf = (key: string): StrategyProgressState => {
    let s = byStrategy.get(key);
    if (s === undefined) {
      s = blankProgressState();
      byStrategy.set(key, s);
    }
    return s;
  };

  const progressOf = (
    strategyKey: string,
    snapshot: string | null,
  ): StreamPublisherProgress => {
    const s = byStrategy.get(strategyKey);
    // No observation of this strategy yet ⇒ NO progress keys at all. The client
    // renders ABSENT with a reason; sending zeros here would be the false-zero
    // this module exists to prevent.
    if (s === undefined) return {};
    const out: StreamPublisherProgress = {
      routes_explored: s.routes.size,
      sizes_quoted: s.sizes.size,
    };
    // The quote census is only honest over rows that actually carry `economics`.
    if (s.withEconomics > 0) {
      out.quotes_exact = s.quotesExact;
      out.quotes_incomplete = s.quotesIncomplete;
      out.quotes_error = s.quotesError;
    }
    if (snapshot != null) out.snapshot_id = snapshot;
    return out;
  };

  return {
    setSnapshotId(id) {
      snapshotId = id;
    },

    reset() {
      byStrategy.clear();
      snapshotId = null;
      counter = 0;
    },

    progressOf,

    build(row) {
      if (row == null || typeof row !== 'object' || Array.isArray(row)) return null;
      const r = row as Record<string, unknown>;
      const strategyKey = strategyKeyOfRow(r);
      const planKey = planKeyOfRow(r);
      const s = stateOf(strategyKey);

      // ── Counters (server-session census of what was actually emitted) ──────
      s.routes.add(routeGroupKeyOfRow(r));
      const size = declaredSizeWeiOfRow(r);
      if (size != null) s.sizes.add(size);

      const economics = r.economics;
      let kind: StreamEventKind =
        asString(r.status) === 'rejected' ? 'plan.rejected' : 'plan.updated';
      let budgetReason: string | null = null;
      const rejection = asString(r.rejection_reason);
      if (rejection != null && BUDGET_EXHAUSTED_REASONS.has(rejection)) {
        budgetReason = rejection;
      }
      if (economics != null && typeof economics === 'object' && !Array.isArray(economics)) {
        const status = asString((economics as Record<string, unknown>).computation_status);
        if (status !== null) {
          s.withEconomics += 1;
          if (status === 'computed') s.quotesExact += 1;
          else if (status === 'partial') s.quotesIncomplete += 1;
          else if (status === 'error') s.quotesError += 1;
        }
      } else if (rejection != null) {
        // A rejected row with no economics object is still a plan observation;
        // it is simply not classifiable as an exact/incomplete quote.
      }

      // ── Ordering: reuse (seq, event_id) for a byte-identical replay ────────
      const digest = rowDigest(r);
      let seq: number;
      let eventId: string;
      if (s.lastDigest === digest && s.lastEventId !== null) {
        seq = s.lastSeq;
        eventId = s.lastEventId;
      } else {
        counter += 1;
        seq = counter;
        eventId = `${planKey}#${seq}`;
        s.lastDigest = digest;
        s.lastEventId = eventId;
        s.lastSeq = seq;
      }

      const progress = progressOf(strategyKey, snapshotId);
      if (budgetReason != null) {
        progress.budget_exhausted = true;
        progress.budget_reason = budgetReason;
      }

      return {
        schema_version: STREAM_SCHEMA_VERSION,
        event_id: eventId,
        seq,
        strategy_key: strategyKey,
        plan_key: planKey,
        snapshot_id: snapshotId,
        emitted_at: new Date(now()).toISOString(),
        kind,
        // Byte-identical row: the envelope stamps identity and order, it never
        // rewrites a figure (RULE 00).
        payload: r,
        progress,
      };
    },
  };
}

/**
 * Process-wide producer used by `broadcastOpportunity`. Exported so ops/tests
 * can name the snapshot (resync boundary) and reset the counters.
 */
export const opportunityStreamProducer = createOpportunityStreamProducer();
