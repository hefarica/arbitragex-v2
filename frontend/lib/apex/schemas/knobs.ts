/**
 * Ω FE-MASTER · Canonical knobs snapshot contract (FE-0061 / XLS-CANON-01)
 *
 * Wire mirror of GET /api/config/canonical-knobs (api-server
 * routes/canonical-knobs.ts → Redis `arbx:config:canonical_knobs`):
 * the EXACT snapshot searcher-rs publishes at boot
 * (CanonicalKnobs::to_json(), env ARBX_KNOB_* > deploy yaml > workbook).
 *
 * R8 Fail-Honest: the endpoint 503s (`redis_unavailable` /
 * `knobs_not_published`) until the searcher has booted — the panel renders
 * the honest error, never a fabricated knob value. `knobs` values stay
 * `unknown` on purpose: the boot snapshot mixes numbers/strings/bools and
 * the surface is read-only display (mode authority is relays-client
 * live_exec_policy, never this surface — §34).
 */
import { z } from 'zod';
import { IsoTimestampSchema } from './_primitives';

export const CanonicalKnobsResponseSchema = z.object({
  /** Boot-snapshot generation stamp (per call, ISO-8601 with offset). */
  generated_at: IsoTimestampSchema,
  source: z.string().min(1),
  knobs: z.record(z.string(), z.unknown()),
}).strict();
export type CanonicalKnobsResponse = z.infer<typeof CanonicalKnobsResponseSchema>;

// ── ARBX-0011 (REQ-DASH-BY-MODE): by-mode KPI scope view ──────────────────
// The three canonical trading modes (searcher-rs canonical_knobs.rs
// `EXEC_MODES` — same order). Mode-invariant doctrine §34.1: the math is ONE
// pipeline; the mode labels which terminus the surfaced KPIs reflect.

/** Canonical trading modes, workbook order (00_MANUAL modos). */
export const EXECUTION_MODES = ["LIVE_MAINNET", "TESTNET", "PAPER_SHADOW"] as const;
export type ExecutionMode = (typeof EXECUTION_MODES)[number];

/**
 * Typed view of the two mode fields the knobs record carries
 * (`execution_mode` / `selected_execution_mode`, both validated against
 * EXECUTION_MODES on the Rust side before publication). DECLARATIVE display
 * only — the execution authority is relays-client `live_exec_policy` (§34).
 */
export type CanonicalModeView = {
  execution_mode: ExecutionMode;
  selected_execution_mode: ExecutionMode;
  /** Boot mode and selected knob agree — a mismatch is surfaced, never hidden. */
  coherent: boolean;
};

/**
 * Extracts the mode view from the raw knobs record. Missing/non-canonical
 * values → null (NOT a default mode — R8 fail-honest; the strip renders the
 * honest absence, mirroring `knobsToQuoteWeights` in QuoteBasePanel).
 */
export function extractCanonicalMode(knobs: Record<string, unknown>): CanonicalModeView | null {
  const read = (key: string): ExecutionMode | null => {
    const v = knobs[key];
    return typeof v === "string" && (EXECUTION_MODES as readonly string[]).includes(v)
      ? (v as ExecutionMode)
      : null;
  };
  const execution_mode = read("execution_mode");
  const selected_execution_mode = read("selected_execution_mode");
  if (execution_mode === null || selected_execution_mode === null) {
    return null;
  }
  return {
    execution_mode,
    selected_execution_mode,
    coherent: execution_mode === selected_execution_mode,
  };
}
