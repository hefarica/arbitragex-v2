/**
 * Ω FE-MASTER · Workbook 01_CONFIG spec + runtime-binding classification
 * (FE-0061 / FE-CFG-001..003)
 *
 * Static-per-canon display table for the CanonicalKnobsPanel: the 17
 * parameters of workbook sheet 01_CONFIG mapped against the searcher's
 * canonical knob snapshot. RULE 00: this is WORKBOOK CANON (reference
 * data, like ChainIdSchema's chain list), not fabricated runtime data —
 * runtime values come exclusively from the snapshot payload.
 *
 * Bindings are validated against searcher-rs/src/canonical_knobs.rs
 * (2026-08-24): 12 of 17 parameters map to a real published knob — the
 * overlay reference mapped only 5; the extension is the port-with-
 * validation delta. The 5 unbound rows are DYNAMIC graph metrics / count
 * formulas (not knobs) and render honestly as DERIVED / NOT EXPOSED —
 * never zero (R8).
 */
import type { CanonicalKnobsResponse } from './schemas/knobs';

// ─── Workbook 01_CONFIG rows (17) ─────────────────────────────────────────
export interface ExcelConfigSpecRow {
  Parameter: string;
  Value: number | string | null;
  Unit: string;
  Meaning: string;
  'Runtime binding': string;
  Editable: string;
}

export const EXCEL_CONFIG_SPEC: readonly ExcelConfigSpecRow[] = [
  { Parameter: 'Allowed_Symbol_Count', Value: 0, Unit: 'tokens', Meaning: 'COUNT dinámico de símbolos habilitados', 'Runtime binding': 'allowed_symbols.len()', Editable: 'formula' },
  { Parameter: 'N_Active_Chain', Value: 22, Unit: 'nodes', Meaning: 'Tokens efectivos de la chain seleccionada; sustituir por registry real', 'Runtime binding': 'chain_graph.node_count', Editable: 'YES/demo' },
  { Parameter: 'Avg_Active_Degree', Value: 6, Unit: 'neighbors/node', Meaning: 'Grado medio tras filtros de liquidez/freshness', 'Runtime binding': 'graph.avg_degree', Editable: 'YES' },
  { Parameter: 'Avg_Parallel_Pools', Value: 2.5, Unit: 'edges/pair', Meaning: 'Pools/venues ejecutables promedio por pareja', 'Runtime binding': 'pair_bucket.avg_edges', Editable: 'YES' },
  { Parameter: 'Dirty_Seeds', Value: 4, Unit: 'pairs/event-window', Meaning: 'Pares/aristas afectados que entran al hot queue', 'Runtime binding': 'dirty_queue.len', Editable: 'YES' },
  { Parameter: 'Beam_K', Value: 4, Unit: 'branches/node', Meaning: 'Top-K outgoing branches retained per expansion', 'Runtime binding': 'route.beam_k', Editable: 'YES' },
  { Parameter: 'Max_Hops', Value: 7, Unit: 'hops', Meaning: 'Techo global; cada estrategia aplica su propio mask', 'Runtime binding': 'route.max_hops', Editable: 'YES' },
  { Parameter: 'Min_Hops', Value: 2, Unit: 'hops', Meaning: 'Piso solicitado', 'Runtime binding': 'route.min_hops', Editable: 'YES' },
  { Parameter: 'Min_Liquidity_USD', Value: 100000, Unit: 'USD', Meaning: 'Gate de liquidez', 'Runtime binding': 'gates.min_liquidity_usd', Editable: 'YES' },
  { Parameter: 'Min_Net_bps', Value: 5, Unit: 'bps', Meaning: 'Gate mínimo beneficio neto', 'Runtime binding': 'gates.min_net_bps', Editable: 'YES' },
  { Parameter: 'Max_State_Age_Blocks', Value: 2, Unit: 'blocks', Meaning: 'Freshness', 'Runtime binding': 'gates.max_state_age_blocks', Editable: 'YES' },
  { Parameter: 'Quote_w_Prior', Value: 0.3, Unit: 'weight', Meaning: 'Peso prior estructural', 'Runtime binding': 'quote.weights.prior', Editable: 'YES' },
  { Parameter: 'Quote_w_Liquidity', Value: 0.3, Unit: 'weight', Meaning: 'Peso liquidez', 'Runtime binding': 'quote.weights.liquidity', Editable: 'YES' },
  { Parameter: 'Quote_w_VenueCoverage', Value: 0.2, Unit: 'weight', Meaning: 'Peso cobertura venues', 'Runtime binding': 'quote.weights.venues', Editable: 'YES' },
  { Parameter: 'Quote_w_Stability', Value: 0.1, Unit: 'weight', Meaning: 'Peso estabilidad', 'Runtime binding': 'quote.weights.stability', Editable: 'YES' },
  { Parameter: 'Quote_w_CrossDex', Value: 0.1, Unit: 'weight', Meaning: 'Peso cobertura cross-DEX', 'Runtime binding': 'quote.weights.crossdex', Editable: 'YES' },
  { Parameter: 'Discovery_SLA_ms', Value: 30, Unit: 'ms', Meaning: 'Objetivo p95 discovery/ranking, no simulación remota', 'Runtime binding': 'telemetry.discovery_sla_ms', Editable: 'YES' },
];

// ─── Excel parameter → published canonical knob key ──────────────────────
/**
 * Every key below exists in searcher-rs canonical_knobs.rs (validated
 * 2026-08-24). NOTE the weight-name asymmetry: the canonical-knobs surface
 * uses serde snake_case (`quote_w_venue_coverage`, `quote_w_cross_dex`)
 * while the trading-config QuoteWeights wire uses `venues`/`cross_dex` —
 * different surfaces, both backend-named.
 */
export const CANONICAL_KNOB_BINDINGS: Readonly<Record<string, string>> = {
  Beam_K: 'beam_k',
  Max_Hops: 'max_hops',
  Min_Hops: 'min_hops',
  Min_Liquidity_USD: 'min_pool_liquidity_usd',
  Min_Net_bps: 'min_net_bps',
  Max_State_Age_Blocks: 'max_state_age_blocks',
  Quote_w_Prior: 'quote_w_prior',
  Quote_w_Liquidity: 'quote_w_liquidity',
  Quote_w_VenueCoverage: 'quote_w_venue_coverage',
  Quote_w_Stability: 'quote_w_stability',
  Quote_w_CrossDex: 'quote_w_cross_dex',
  Discovery_SLA_ms: 'discovery_sla_ms',
};

// ─── Row classification (pure — FE-CFG-002/003) ──────────────────────────
export type KnobRowStatus = 'EFFECTIVE' | 'DERIVED' | 'NOT_EXPOSED';

export interface KnobRow {
  spec: ExcelConfigSpecRow;
  /** Published knob key, or null when the parameter is not a knob. */
  knobKey: string | null;
  /** Snapshot value — undefined when absent (rendered "—", never 0). */
  effective: unknown;
  status: KnobRowStatus;
}

/**
 * EFFECTIVE  = the parameter binds to a knob the snapshot actually carries.
 * DERIVED    = no knob by design (Editable "formula" — computed upstream).
 * NOT_EXPOSED = no published knob for this parameter (honest gap).
 * A null snapshot (503 pre-boot) classifies WITHOUT ever claiming
 * EFFECTIVE — absence never over-claims (R8).
 */
export function buildKnobRows(
  snapshot: CanonicalKnobsResponse | null,
): KnobRow[] {
  const knobs = snapshot?.knobs ?? {};
  return EXCEL_CONFIG_SPEC.map((spec) => {
    const knobKey = CANONICAL_KNOB_BINDINGS[spec.Parameter] ?? null;
    const effective = knobKey !== null ? knobs[knobKey] : undefined;
    const effectiveExists = knobKey !== null && effective !== undefined;
    const status: KnobRowStatus = effectiveExists
      ? 'EFFECTIVE'
      : spec.Editable === 'formula'
        ? 'DERIVED'
        : 'NOT_EXPOSED';
    return { spec, knobKey, effective, status };
  });
}
