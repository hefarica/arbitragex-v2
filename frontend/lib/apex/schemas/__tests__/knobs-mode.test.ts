/**
 * ARBX-0011 (REQ-DASH-BY-MODE) — canonical mode view extraction tests.
 *
 * The knobs record stays `z.unknown()` (boot snapshot shape belongs to the
 * searcher); `extractCanonicalMode` is the ONLY typed reading of the two
 * mode fields. Missing / non-canonical values → null (R8 fail-honest — the
 * strip renders the honest absence, never a default mode).
 */
import { describe, expect, it } from 'vitest';

import {
  CanonicalKnobsResponseSchema,
  EXECUTION_MODES,
  extractCanonicalMode,
} from '../knobs';

describe('EXECUTION_MODES (canonical order)', () => {
  it('is the searcher-rs EXEC_MODES trio, workbook order', () => {
    expect(EXECUTION_MODES).toEqual(['LIVE_MAINNET', 'TESTNET', 'PAPER_SHADOW']);
  });
});

describe('extractCanonicalMode', () => {
  it('extracts a coherent paper view from a real-shaped snapshot', () => {
    const view = extractCanonicalMode({
      execution_mode: 'PAPER_SHADOW',
      selected_execution_mode: 'PAPER_SHADOW',
      beam_k: 4,
    });
    expect(view).toEqual({
      execution_mode: 'PAPER_SHADOW',
      selected_execution_mode: 'PAPER_SHADOW',
      coherent: true,
    });
  });

  it('surfaces a boot-vs-selected MISMATCH instead of reconciling it', () => {
    const view = extractCanonicalMode({
      execution_mode: 'PAPER_SHADOW',
      selected_execution_mode: 'TESTNET',
    });
    expect(view?.coherent).toBe(false);
    expect(view?.execution_mode).toBe('PAPER_SHADOW');
  });

  it('null when either field is absent (never a default mode)', () => {
    expect(extractCanonicalMode({ selected_execution_mode: 'PAPER_SHADOW' })).toBeNull();
    expect(extractCanonicalMode({ execution_mode: 'PAPER_SHADOW' })).toBeNull();
    expect(extractCanonicalMode({})).toBeNull();
  });

  it('null on non-canonical values (typo / future mode never renders as data)', () => {
    expect(
      extractCanonicalMode({ execution_mode: 'paper_shadow', selected_execution_mode: 'paper_shadow' }),
    ).toBeNull();
    expect(
      extractCanonicalMode({ execution_mode: 3, selected_execution_mode: 'TESTNET' }),
    ).toBeNull();
  });

  it('composes with the wire schema (typed knobs record in, view out)', () => {
    const parsed = CanonicalKnobsResponseSchema.parse({
      generated_at: '2026-08-24T00:00:00Z',
      source: 'boot',
      knobs: { execution_mode: 'TESTNET', selected_execution_mode: 'TESTNET' },
    });
    expect(extractCanonicalMode(parsed.knobs)?.execution_mode).toBe('TESTNET');
  });
});
