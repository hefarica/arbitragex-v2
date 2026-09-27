/**
 * ECON-DECLARE-01 (2026-09-27) — "hay muchos valores que no se ven, no están
 * declarados."
 *
 * ── The defect, measured ────────────────────────────────────────────────────
 *
 * GET /api/opportunities/live, 33 live rows, 2026-09-27T02:06Z:
 *
 *   amount_in_wei            33/33 present
 *   expected_profit_usd      18/33 present
 *   net_expected_profit_usd  15/33 present
 *   roi_pct                   0/33 present
 *   risk_score                0/33 present
 *   route_metadata           33/33 present — and NOT ONE row declared
 *                            `economics_amount_in_wei` or `economics_basis`.
 *
 * The figures that ARE present could not be read, because three producers write
 * economic numbers onto one row, each at its OWN size:
 *
 *   · `expected_profit_usd`     the engine fast-filter GROSS, at the engine
 *                               probe (one native unit of `token_in`).
 *   · `net_expected_profit_usd` the sizing kernel / spine NET, at the kernel's
 *                               own snapped size.
 *   · `simulated_*`             the api-server TS forward-sim, at the notional
 *                               derived from `amount_in_wei`.
 *
 * With nothing declared, a card could not tell whether two figures belonged to
 * ONE arithmetic, so `CARDS-NOTIONAL-01` made it HIDE them (`—`) rather than
 * risk the `IN $0.00 / GROSS $1.47M` contradiction the operator photographed.
 * Hiding a computed value is the second half of the same defect.
 *
 * ── The rule this module enforces ───────────────────────────────────────────
 *
 * A value that exists on the wire is SHOWN. It is never replaced by `—` because
 * its provenance was inconvenient. What changes is that the cell also declares
 * WHICH producer computed it and at WHICH notional, so two figures of different
 * provenance can sit on one card without being read as one ladder:
 *
 *     in   $2,699.72 @intent      Gross   $0.0000 @probe
 *
 * The basis is NEVER inferred here: `@probe` is claimed only when the producer
 * DECLARED it. A figure whose producer declared nothing renders `@undeclared`
 * with the machine reason — honest, not guessed (R8 / RULE 00).
 *
 * Two CI-enforceable gates live here:
 *   (i)  `auditServedWireKeys` — every key the API serves must be DECLARED in
 *        `OPPORTUNITY_FIELD_REGISTRY` (provenance, surface, meaning). A new wire
 *        key with no declaration fails the gate.
 *   (ii) `auditDashOverValue` — a card cell must never render `—` while the wire
 *        carries a value for the field that cell renders.
 */

import type { OmniOpportunity, RouteMetadataWire } from "@/lib/store/types";

/** The one honest empty state. Reserved for "the wire carries no value". */
export const DECLARATION_DASH = "—";

// =============================================================================
// 1. BASIS VOCABULARY — the words the producer writes
// =============================================================================

/**
 * WHERE a figure was computed. Mirrors `shared_rs::candidates::economics_basis`
 * PLUS the two states no producer writes: `sim` (the api-server TS forward-sim,
 * whose basis is `simulated_*` itself) and `undeclared` (no producer said).
 */
export type FigureBasis =
  | "probe"
  | "kernel"
  | "intent"
  | "stamped_probe"
  | "sim"
  | "wire"
  | "undeclared";

/** Compact DOM label per basis, rendered verbatim beside the figure. */
export const BASIS_LABEL: Record<FigureBasis, string> = {
  probe: "@probe",
  kernel: "@kernel",
  intent: "@intent",
  stamped_probe: "@stamped",
  sim: "@sim",
  wire: "@wire",
  undeclared: "@undeclared",
};

/** The producer's wire word → the typed basis. Unknown words are NOT guessed. */
export function basisFromWire(raw: string | null | undefined): FigureBasis | null {
  switch ((raw ?? "").trim().toLowerCase()) {
    case "probe":
      return "probe";
    case "kernel":
      return "kernel";
    case "intent":
      return "intent";
    case "stamped":
    case "stamped_probe":
      return "stamped_probe";
    case "sim":
    case "simulated":
      return "sim";
    default:
      return null;
  }
}

// =============================================================================
// 2. THE DECLARATION — what the producer stated about one row
// =============================================================================

export interface EconomicsDeclaration {
  /** Exact wei at which the row's economic figures were computed, or null. */
  notionalWei: string | null;
  /** Per-figure basis, as declared. `null` per figure = not declared. */
  gross: FigureBasis | null;
  net: FigureBasis | null;
  amount: FigureBasis | null;
  /** True when the producer declared at least one basis or a notional. */
  declared: boolean;
}

/**
 * Read the producer's declaration off the row. PURE, and never inferential: a
 * figure whose basis is not on the wire reads `null` here and the caller renders
 * `@undeclared`. This is the whole contract — downstream code must not "repair"
 * an absent basis into a plausible one (R8).
 */
export function readEconomicsDeclaration(
  rm: RouteMetadataWire | null | undefined,
): EconomicsDeclaration {
  const basis = rm?.economics_basis ?? null;
  const gross = basisFromWire(basis?.gross);
  const net = basisFromWire(basis?.net);
  const amount = basisFromWire(basis?.amount);
  const notionalWei =
    typeof rm?.economics_amount_in_wei === "string" && /^[0-9]{1,78}$/.test(rm.economics_amount_in_wei)
      ? rm.economics_amount_in_wei
      : null;
  return {
    notionalWei,
    gross,
    net,
    amount,
    declared: gross != null || net != null || amount != null || notionalWei != null,
  };
}

/** Convenience: the declaration of an opportunity row. */
export function declarationOf(opp: OmniOpportunity): EconomicsDeclaration {
  return readEconomicsDeclaration(opp.route_metadata);
}

/**
 * The basis a cell must display for one figure of a row.
 *
 * `declared` — the producer said so. `undeclared` — no producer said, and this
 * module refuses to guess. `sim` — the figure is one of the `simulated_*` fields,
 * whose producer is the api-server TS forward-sim by construction (the field name
 * IS the declaration), so it is declared rather than undeclared.
 */
export function figureBasis(
  opp: OmniOpportunity,
  figure: "gross" | "net" | "roi" | "risk" | "amount" | "simulated",
): FigureBasis {
  const d = declarationOf(opp);
  switch (figure) {
    case "gross":
      return d.gross ?? (opp.expected_profit_usd != null ? "undeclared" : "undeclared");
    case "net":
      return d.net ?? "undeclared";
    case "amount":
      return d.amount ?? "undeclared";
    // roi_pct / risk_score are written by the spine/kernel phase, but NO
    // vocabulary word has ever been declared for them on the wire, so they are
    // honestly undeclared until a producer states one.
    case "roi":
    case "risk":
      return "undeclared";
    case "simulated":
      return "sim";
  }
}

// =============================================================================
// 3. FIELD REGISTRY — the contract, as DATA (never code branches)
// =============================================================================

/**
 * Where a wire field is displayed. `"none"` is an honest declaration that the
 * route serves the field and no surface renders it today — declared so the
 * undeclared-key scan stays meaningful instead of silently passing.
 */
export type DisplaySurface =
  | "card-cell"
  | "card-ledger"
  | "card-header"
  | "dialog"
  | "none";

export interface FieldRegistryEntry {
  /** Dotted wire path rooted at one opportunity item. `*` = open map. */
  field: string;
  /** Human label as rendered. */
  label: string;
  /** Where it is displayed. */
  surface: readonly DisplaySurface[];
  /**
   * The figure's basis, or `null` when the field is not an economic figure.
   * `"undeclared"` marks an economic field whose producer MAY omit the basis.
   */
  basis: FigureBasis | null;
  /**
   * True ⇒ gate (ii) asserts this field reaches the DOM (with its basis label)
   * whenever the wire carries a value.
   */
  required: boolean;
  /** One-line meaning (rendered as the `title` tooltip). */
  meaning: string;
}

/**
 * EVERY key `GET /api/v1/opportunities/live` serves per item, its surface, and
 * its basis.
 *
 * Gate (i) (`auditServedWireKeys`) fails when the API serves a key that is not
 * here — a wire key with no declared provenance is exactly what produced the
 * operator's "no están declarados".
 */
export const OPPORTUNITY_FIELD_REGISTRY: readonly FieldRegistryEntry[] = [
  // ── Identity / topology (not economic figures) ────────────────────────────
  { field: "id", label: "id", surface: ["card-header"], basis: null, required: false, meaning: "Logical opportunity id (DOM data-opp-id, dialog key)" },
  { field: "chain_id", label: "chain", surface: ["card-header"], basis: null, required: false, meaning: "Source chain id" },
  { field: "chain_id_out", label: "chain out", surface: ["card-header"], basis: null, required: false, meaning: "Destination chain id (cross-chain rows)" },
  { field: "chain_base_token_symbol", label: "base token", surface: ["card-header"], basis: null, required: false, meaning: "Chain's base token symbol badge" },
  { field: "strategy_kind", label: "strategy", surface: ["card-cell", "card-header"], basis: null, required: true, meaning: "Strategy family of the row" },
  { field: "cartridge_id", label: "cartridge", surface: ["dialog"], basis: null, required: false, meaning: "Rhai cartridge id when the row came from the cartridge layer" },
  { field: "detector_id", label: "detector", surface: ["card-cell"], basis: null, required: true, meaning: "Which detector produced the row" },
  { field: "dex_a", label: "ruta", surface: ["card-cell"], basis: null, required: true, meaning: "First venue of the route" },
  { field: "dex_b", label: "ruta", surface: ["card-cell"], basis: null, required: false, meaning: "Second venue of the route" },
  { field: "dexes_used", label: "dexes", surface: ["dialog"], basis: null, required: false, meaning: "Every venue touched by the route" },
  { field: "pair_symbol", label: "pair", surface: ["card-header"], basis: null, required: false, meaning: "Traded pair symbol" },
  { field: "token_in", label: "token in", surface: ["card-header"], basis: null, required: false, meaning: "Route input token address" },
  { field: "token_out", label: "token out", surface: ["card-header"], basis: null, required: false, meaning: "Route output token address" },
  { field: "token_in_info", label: "token in info", surface: ["card-header"], basis: null, required: false, meaning: "Validated metadata for token_in" },
  { field: "token_out_info", label: "token out info", surface: ["card-header"], basis: null, required: false, meaning: "Validated metadata for token_out" },
  { field: "token_prices_usd", label: "prices", surface: ["card-header"], basis: null, required: false, meaning: "Live PriceBus USD price per symbol" },
  { field: "leg_symbols", label: "leg symbols", surface: ["card-header"], basis: null, required: false, meaning: "Resolved symbols for intermediate legs" },
  { field: "chains_used", label: "chains", surface: ["dialog"], basis: null, required: false, meaning: "Every chain the route touches" },
  { field: "block_number", label: "block", surface: ["dialog"], basis: null, required: false, meaning: "Block the route was detected on" },
  { field: "tx_hash", label: "tx hash", surface: ["dialog"], basis: null, required: false, meaning: "Triggering transaction hash when known" },
  { field: "trace_id", label: "trace", surface: ["dialog"], basis: null, required: false, meaning: "Correlation id across the pipeline" },
  { field: "detected_at", label: "detected", surface: ["card-header"], basis: null, required: true, meaning: "Detection timestamp" },
  { field: "first_seen_at", label: "1ª", surface: ["card-header"], basis: null, required: false, meaning: "First detection of this route group" },
  { field: "last_seen_at", label: "✓", surface: ["card-header"], basis: null, required: false, meaning: "Last ratification of this route group" },
  { field: "confirmations", label: "×n", surface: ["card-header"], basis: null, required: false, meaning: "Re-detections inside the window" },
  { field: "route_group_key", label: "route group", surface: ["dialog"], basis: null, required: false, meaning: "Dedup key of the route group" },
  { field: "status", label: "status", surface: ["card-header"], basis: null, required: true, meaning: "Pipeline status of the row" },
  { field: "rejection_reason", label: "rejection", surface: ["card-header", "dialog"], basis: null, required: true, meaning: "R8 reason the row was rejected" },
  { field: "paper_status", label: "paper", surface: ["dialog"], basis: null, required: false, meaning: "Paper-shadow execution state" },
  { field: "pipeline_latency_ms", label: "latencia", surface: ["card-cell"], basis: null, required: true, meaning: "Detection → emission latency" },
  { field: "semantic_violations", label: "violations", surface: ["card-header"], basis: null, required: false, meaning: "Semantic quarantines of the row" },
  { field: "bridge", label: "bridge", surface: ["dialog"], basis: null, required: false, meaning: "Bridge used by a cross-chain route" },
  { field: "bridge_fee_usd", label: "bridge fee", surface: ["card-ledger"], basis: "undeclared", required: false, meaning: "Bridge fee in USD (cost component)" },

  // ── Route topology ────────────────────────────────────────────────────────
  { field: "route_metadata", label: "route", surface: ["card-header"], basis: null, required: false, meaning: "Persisted route topology (JSONB)" },
  { field: "route_metadata.dex_adapters", label: "hops", surface: ["card-cell", "card-ledger"], basis: null, required: true, meaning: "One adapter per leg — its length IS hop_count" },
  { field: "route_metadata.token_addresses", label: "token path", surface: ["card-header"], basis: null, required: false, meaning: "Full token path including intermediates" },
  { field: "route_metadata.pool_addresses", label: "pools", surface: ["dialog"], basis: null, required: false, meaning: "Pool address per hop" },
  { field: "route_metadata.decimals", label: "decimals", surface: ["card-ledger"], basis: null, required: false, meaning: "Token decimals for exact wei rendering" },
  { field: "route_metadata.leg_amounts_in", label: "leg in", surface: ["card-ledger"], basis: "kernel", required: false, meaning: "Kernel per-leg exact wei in" },
  { field: "route_metadata.leg_amounts_out", label: "leg out", surface: ["card-ledger"], basis: "kernel", required: false, meaning: "Kernel per-leg exact wei out" },
  { field: "route_metadata.leg_zero_for_one", label: "leg dir", surface: ["dialog"], basis: null, required: false, meaning: "Uniswap token0→token1 orientation per leg" },
  { field: "route_metadata.economics_amount_in_wei", label: "notional", surface: ["card-ledger", "card-cell"], basis: null, required: false, meaning: "ECON-DECLARE-01: notional the economics were computed at" },
  { field: "route_metadata.economics_basis", label: "basis", surface: ["card-ledger", "card-cell"], basis: null, required: false, meaning: "ECON-DECLARE-01: which producer computed each figure" },
  { field: "route_metadata_invalid", label: "route invalid", surface: ["dialog"], basis: null, required: false, meaning: "route_metadata failed to parse" },

  // ── Economic figures — the ones the operator could not see ────────────────
  { field: "amount_in_wei", label: "in (wei)", surface: ["card-cell", "card-ledger"], basis: "intent", required: true, meaning: "Route input amount in exact wei" },
  { field: "expected_profit_usd", label: "Gross", surface: ["card-cell", "card-ledger"], basis: "probe", required: true, meaning: "GROSS of the engines' fast filter, at the engine probe" },
  { field: "net_expected_profit_usd", label: "Net", surface: ["card-cell", "card-ledger", "card-header"], basis: "kernel", required: true, meaning: "NET after all costs, from the sizing kernel / spine" },
  { field: "roi_pct", label: "bps", surface: ["card-cell", "card-header"], basis: "undeclared", required: true, meaning: "Net convergence ratio of the canonical spine" },
  { field: "risk_score", label: "Risk", surface: ["card-cell"], basis: "undeclared", required: true, meaning: "Risk score of the row (0-1 fraction)" },
  { field: "simulated_net_profit_usd", label: "Sim", surface: ["card-cell", "card-ledger", "card-header"], basis: "sim", required: true, meaning: "TS forward-sim net at the row's amount" },
  { field: "simulated_amount_in_usd", label: "in", surface: ["card-cell", "card-ledger"], basis: "sim", required: true, meaning: "Notional of the TS forward-sim, priced from amount_in_wei" },
  { field: "simulated_gross_usd", label: "Gross (sim)", surface: ["card-ledger"], basis: "sim", required: false, meaning: "Gross of the same forward-sim call" },
  { field: "simulated_cost_breakdown", label: "costs", surface: ["card-ledger"], basis: "sim", required: false, meaning: "Per-component costs of the same forward-sim call" },
  { field: "simulated_costs_total_usd", label: "Total cost", surface: ["card-ledger"], basis: "sim", required: false, meaning: "Σ of the forward-sim cost components" },
  { field: "simulated_roi_pct", label: "bps (sim)", surface: ["card-cell"], basis: "sim", required: false, meaning: "ROI of the forward-sim" },
  { field: "simulated_target", label: "target", surface: ["card-header"], basis: "sim", required: false, meaning: "Operator target verdict WITH its arithmetic" },
  { field: "simulated_at", label: "sim at", surface: ["dialog"], basis: "sim", required: false, meaning: "Timestamp of the simulation activity" },
  { field: "simulated_notes", label: "sim notes", surface: ["dialog"], basis: "sim", required: false, meaning: "Machine notes of the simulation" },
] as const;

/** O(1) lookup by wire path. */
const BY_FIELD = new Map(OPPORTUNITY_FIELD_REGISTRY.map((e) => [e.field, e]));

export function registryEntry(field: string): FieldRegistryEntry | undefined {
  return BY_FIELD.get(field);
}

// =============================================================================
// 4. GATE (i) — every served wire key must be DECLARED
// =============================================================================

export interface UndeclaredKeyFinding {
  field: string;
  reason: string;
}

/**
 * GATE (i). Fails when the API serves a key that the registry does not declare.
 *
 * `servedKeys` is the observed key set of one live opportunity item (dotted
 * paths for nested objects; open maps like `token_in_info`/`token_prices_usd`
 * are matched by their `*`-free prefix). A key present on the wire with no
 * registry entry has NO declared provenance — the defect this gate exists for.
 */
export function auditServedWireKeys(servedKeys: readonly string[]): UndeclaredKeyFinding[] {
  const findings: UndeclaredKeyFinding[] = [];
  for (const key of servedKeys) {
    if (BY_FIELD.has(key)) continue;
    // A declared ancestor covers an open map (`token_prices_usd.USDC`).
    const dotted = key.lastIndexOf(".");
    if (dotted > 0 && BY_FIELD.has(key.slice(0, dotted))) continue;
    findings.push({
      field: key,
      reason:
        `wire key "${key}" is served by the API but has NO entry in ` +
        `OPPORTUNITY_FIELD_REGISTRY — its provenance (surface + basis + meaning) ` +
        `is undeclared, which is exactly the operator's "no está declarado" (R10)`,
    });
  }
  return findings;
}

/**
 * GATE (i, runtime half). Every ECONOMIC field the registry marks `required`
 * must carry a declared basis ON THE WIRE, or the row must carry a stated reason
 * for its absence. This is what keeps `@undeclared` from becoming permanent
 * wallpaper: a rendered `@undeclared` with no reason in the registry's vocabulary
 * is a producer that has not finished its contract.
 *
 * Returns the economic fields present on the row whose basis is undeclared.
 */
export function auditUndeclaredEconomicFigures(opp: OmniOpportunity): string[] {
  const d = declarationOf(opp);
  const out: string[] = [];
  if (opp.expected_profit_usd != null && d.gross == null) out.push("expected_profit_usd");
  if (opp.net_expected_profit_usd != null && d.net == null) out.push("net_expected_profit_usd");
  // roi_pct / risk_score have no vocabulary word at all yet — reported so the
  // gap is visible rather than silently tolerated.
  if (opp.roi_pct != null) out.push("roi_pct");
  if (opp.risk_score != null) out.push("risk_score");
  return out;
}

// =============================================================================
// 5. GATE (ii) — a cell must never render — over a value the wire carries
// =============================================================================

/**
 * The registry `field` of each summary-grid cell that renders an economic or
 * contextual wire value. The gate pairs a cell's `data-testid` with the field
 * whose presence obliges the cell to show a number.
 */
export const CELL_FIELD_BINDING: Readonly<Record<string, string>> = {
  ruta: "dex_a",
  strategy: "strategy_kind",
  detector: "detector_id",
  hops: "route_metadata.dex_adapters",
  in: "simulated_amount_in_usd",
  Gross: "expected_profit_usd",
  Net: "net_expected_profit_usd",
  bps: "roi_pct",
  Risk: "risk_score",
  Sim: "simulated_net_profit_usd",
  latencia: "pipeline_latency_ms",
};

/**
 * GATE (ii). Fails when a card cell renders `—` while the wire carries a value
 * for the field that cell is declared to render.
 *
 * `cellText` maps a cell label → the text its DOM renders. The gate reads only
 * what the operator reads: if the wire has a number and the cell shows the dash,
 * a computed value is being hidden — the operator's "no se ven".
 *
 * Deliberately NOT a "the cell must be non-empty" check: a field the wire does
 * not carry has no value, and `—` is then the honest state (R8).
 */
export function auditDashOverValue(
  opp: OmniOpportunity,
  cellText: Readonly<Record<string, string>>,
): UndeclaredKeyFinding[] {
  const findings: UndeclaredKeyFinding[] = [];
  const present: Record<string, boolean> = {
    dex_a: opp.dex_a != null && String(opp.dex_a).trim() !== "",
    strategy_kind: opp.strategy_kind != null,
    detector_id: opp.detector_id != null,
    "route_metadata.dex_adapters": (opp.route_metadata?.dex_adapters?.length ?? 0) > 0,
    // The `in` cell renders the SIM notional when it exists, else the declared
    // notional. Either one present means a value exists to show.
    simulated_amount_in_usd:
      opp.simulated_amount_in_usd != null ||
      declarationOf(opp).notionalWei != null,
    expected_profit_usd: opp.expected_profit_usd != null,
    net_expected_profit_usd:
      opp.net_expected_profit_usd != null || opp.simulated_net_profit_usd != null,
    roi_pct: opp.roi_pct != null || opp.simulated_roi_pct != null,
    risk_score: opp.risk_score != null,
    simulated_net_profit_usd: opp.simulated_net_profit_usd != null,
    pipeline_latency_ms: opp.pipeline_latency_ms != null,
  };
  for (const [label, field] of Object.entries(CELL_FIELD_BINDING)) {
    if (!present[field]) continue;
    const text = (cellText[label] ?? "").trim();
    if (text === "" || text === DECLARATION_DASH) {
      findings.push({
        field,
        reason:
          `cell "${label}" renders "${text || "(empty)"}" while the wire carries ` +
          `${field} — a computed value is hidden. It must be shown with its basis ` +
          `label (@probe/@kernel/@intent/@stamped/@sim/@undeclared) instead (R10)`,
      });
    }
  }
  return findings;
}

/**
 * GATE (ii, source half). Every cell the grid can render must be declared in
 * `CELL_FIELD_BINDING`; a cell that renders a value with no declared wire source
 * is a value of unknown provenance.
 */
export function auditCellDeclarations(renderedCellLabels: readonly string[]): UndeclaredKeyFinding[] {
  const findings: UndeclaredKeyFinding[] = [];
  for (const label of renderedCellLabels) {
    if (CELL_FIELD_BINDING[label]) continue;
    findings.push({
      field: label,
      reason:
        `summary cell "${label}" renders but is not declared in CELL_FIELD_BINDING ` +
        `— its wire source is undeclared (R10)`,
    });
  }
  return findings;
}

// =============================================================================
// 6. RENDERING HELPERS — the basis suffix every economic cell must carry
// =============================================================================

/** The visible basis marker for a figure (`@probe`, `@undeclared`, …). */
export function basisSuffix(basis: FigureBasis): string {
  return BASIS_LABEL[basis];
}

/**
 * Exact wei → a readable notional, WITHOUT unit conversion.
 *
 * A power of ten collapses to engineering notation (`1e18`, `1e6`) because that
 * is how the engine probe and the kernel's snapped sizes actually read;
 * anything else is rendered VERBATIM (truncated with an exponent when it is too
 * long for a cell). Nothing is rounded and nothing is divided by a token's
 * decimals: that scaling needs a decimals value this module does not own, and
 * assuming 18 is the HOPS-UNITS-01 defect (a USDC route read at 18 decimals
 * reported 1e12 USDC) — R8 forbids the guess.
 */
export function formatWeiNotional(wei: string | null | undefined): string {
  if (wei == null) return DECLARATION_DASH;
  const s = String(wei).trim();
  if (s === "") return DECLARATION_DASH;
  if (/^10*$/.test(s) && s.length > 1) return `1e${s.length - 1}`;
  return s.length > 18 ? `${s.slice(0, 6)}…e${s.length - 1}` : s;
}

/**
 * The machine reason to put in a cell's `title` when a figure's basis was NOT
 * declared. Rendered verbatim so the gap is one hover away instead of hidden.
 */
export function undeclaredReason(field: string): string {
  return (
    `ECON-DECLARE-01: ${field} no trae basis declarada por su productor — ` +
    `se muestra @undeclared (R8: la ausencia de declaración es un estado, no se adivina)`
  );
}
