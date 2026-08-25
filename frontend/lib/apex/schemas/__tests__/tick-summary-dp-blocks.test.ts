/**
 * Ω ARBX-DP-002/003/004 · RouteDiscoveryTickSummary — DP block vectors.
 *
 * The two blocks the DP series added to the flat tick_summary wire:
 *
 *   required_data_gate (DP-002 + DP-003 `tier`): Option on the wire — null
 *     when no multi-hop policy was selected this tick; a real gate carries
 *     the DP-003 tier token, null when the class drifted outside the closed
 *     vocabulary (honest unknown, never a default tier).
 *
 *   detector_mask (DP-004): ALWAYS an object — {event, admitted, total,
 *     selected_admitted}; selected_admitted null when no policy selected.
 *
 * Vectors mirror the worker's json! literals (route_discovery_worker.rs
 * required_data_gate ~L519 / detector_mask ~L531). RULE 00: absence of the
 * whole group is legal (.partial() — knob/path-conditional emission); a
 * malformed shape is NOT.
 */
import { describe, expect, it } from 'vitest';

import { RouteDiscoveryTickSummarySchema } from '../telemetry';

const GATE_FULL = {
  detector: 'X_BRIDGE',
  surface: 'bridge inventory',
  verdict: 'needs_data',
  reason: 'missing external inventory',
  required_data: 'bridge_inventory',
  tier: 'signal',
} as const;

const MASK_FULL = {
  event: 'pool_reserve_update',
  admitted: 51,
  total: 60,
  selected_admitted: true,
} as const;

describe('required_data_gate block (DP-002/DP-003)', () => {
  it('the full vector parses, tier included', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      required_data_gate: GATE_FULL,
      detector_mask: MASK_FULL,
    });
    expect(r.success).toBe(true);
    expect(r.data!.required_data_gate!.tier).toBe('signal');
  });

  it('null (no policy selected this tick) parses — Option on the wire', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      required_data_gate: null,
      detector_mask: { ...MASK_FULL, selected_admitted: null },
    });
    expect(r.success).toBe(true);
    expect(r.data!.required_data_gate).toBeNull();
  });

  it('tier null (class outside the closed vocabulary) parses — honest unknown', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      required_data_gate: { ...GATE_FULL, tier: null },
      detector_mask: MASK_FULL,
    });
    expect(r.success).toBe(true);
    expect(r.data!.required_data_gate!.tier).toBeNull();
  });

  it('a sidecar key inside the gate FAILS .strict()', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      required_data_gate: { ...GATE_FULL, confidence: 0.9 },
      detector_mask: MASK_FULL,
    });
    expect(r.success).toBe(false);
  });

  it('a tier OUTSIDE the four tokens FAILS the wire (fail-closed at the schema too)', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      required_data_gate: { ...GATE_FULL, tier: 'probably' },
      detector_mask: MASK_FULL,
    });
    expect(r.success).toBe(false);
  });
});

describe('detector_mask block (DP-004)', () => {
  it('admitted > total FAILS (mask cannot admit more detectors than exist)', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      detector_mask: { ...MASK_FULL, admitted: 61 },
    });
    expect(r.success).toBe(false);
  });

  it('selected_admitted null (no policy selected) parses', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      detector_mask: { ...MASK_FULL, selected_admitted: null },
    });
    expect(r.success).toBe(true);
  });

  it('a numeric selected_admitted FAILS — it is a boolean flag', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      detector_mask: { ...MASK_FULL, selected_admitted: 1 },
    });
    expect(r.success).toBe(false);
  });
});
