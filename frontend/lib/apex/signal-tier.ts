/**
 * Ω ARBX-DP-005 · Execution_Class → emission-tier taxonomy (TS mirror)
 *
 * EXACT mirror of `backend/searcher-rs/src/signal_tier.rs`
 * (`tier_for_execution_class`): the closed 29-token workbook vocabulary
 * (sheets 11+13) folds into four emission tiers — distinct FEEDS, never one
 * flattened "arbitrage" feed:
 *
 *   OBSERVATION — informational only; never an Opportunity{confidence}
 *                 (OBSERVE_ONLY).
 *   SIGNAL      — precondition outside the atomic execution envelope
 *                 (SIGNAL_UNLESS_FIRM_EXIT / *_DATA_REQUIRED / NONATOMIC_* /
 *                 LATENCY / SETTLEMENT_DELAY): signal until firm evidence.
 *   CANDIDATE   — deterministic under a runtime-observable precondition
 *                 (DETERMINISTIC_IF_* / POST_* / WITH_* / SETTLEMENT /
 *                 AUCTION / LIQUIDATION / POSITION + AUTHORIZED_FLOW_ONLY).
 *   EXECUTABLE  — DETERMINISTIC_EXECUTABLE only.
 *
 * Parity is pinned differentially: `__tests__/signal-tier.test.ts` sweeps
 * the REAL generated catalog (QUOTEBASE_DETECTOR_CATALOG) and asserts the
 * same 1/15/33/11 partition the Rust sweep test pins — if either side
 * drifts, the two alarms fire together.
 *
 * `null` = token outside the closed vocabulary (fail-closed, mirroring the
 * Rust `None`): the UI renders an honest "unknown" bucket, never a default
 * tier (§28/R8).
 */

export const SIGNAL_TIER_TOKENS = [
  'observation',
  'signal',
  'candidate',
  'executable',
] as const;

export type SignalTier = (typeof SIGNAL_TIER_TOKENS)[number];

const SIGNAL_CLASSES = new Set<string>([
  'SIGNAL_UNLESS_FIRM_EXIT',
  'EXTERNAL_DATA_REQUIRED',
  'EXTERNAL_SETTLEMENT_REQUIRED',
  'DERIVATIVE_DATA_REQUIRED',
  'NONATOMIC_BRIDGE_REQUIRED',
  'NONATOMIC_INVENTORY_REQUIRED',
  'LATENCY_SENSITIVE',
  'SETTLEMENT_DELAY_SENSITIVE',
]);

const CANDIDATE_CLASSES = new Set<string>([
  'AUTHORIZED_FLOW_ONLY',
  'DETERMINISTIC_AUCTION',
  'DETERMINISTIC_IF_ADAPTER',
  'DETERMINISTIC_IF_COMPLETE_SET',
  'DETERMINISTIC_IF_CONVERTIBLE',
  'DETERMINISTIC_IF_FIRM_BID',
  'DETERMINISTIC_IF_FIRM_EXIT',
  'DETERMINISTIC_IF_MATCHED_CLAIM',
  'DETERMINISTIC_IF_PAYOFF_MODEL',
  'DETERMINISTIC_IF_POSITIONS',
  'DETERMINISTIC_IF_REDEEMABLE',
  'DETERMINISTIC_IF_SETTLEABLE',
  'DETERMINISTIC_LIQUIDATION',
  'DETERMINISTIC_POSITION_STRATEGY',
  'DETERMINISTIC_POST_ORACLE',
  'DETERMINISTIC_POST_STATE',
  'DETERMINISTIC_SETTLEMENT',
  'DETERMINISTIC_WITH_DERIVATIVE_STATE',
  'DETERMINISTIC_WITH_ORACLE',
]);

/** Workbook Execution_Class token → tier. null = outside the closed 29. */
export function tierForExecutionClass(executionClass: string): SignalTier | null {
  if (executionClass === 'OBSERVE_ONLY') return 'observation';
  if (executionClass === 'DETERMINISTIC_EXECUTABLE') return 'executable';
  if (SIGNAL_CLASSES.has(executionClass)) return 'signal';
  if (CANDIDATE_CLASSES.has(executionClass)) return 'candidate';
  return null;
}
