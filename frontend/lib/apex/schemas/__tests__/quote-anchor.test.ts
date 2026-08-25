import { describe, expect, it } from 'vitest';

import {
  QuoteAnchorResponseSchema,
  QuoteAnchorViewSchema,
  QuotePreviewResponseSchema,
} from '../index';

const VIEW = {
  chain_id: 1,
  quote_symbol: 'USDC',
  quote_score: 87.4,
  quote_version: 12,
  graph_version: 5,
  components: { prior: 0.6, liquidity: 0.9, venues: 0.5, stability: 0.7, cross_dex: 0.4 },
  weights: { prior: 0.25, liquidity: 0.25, venues: 0.2, stability: 0.2, cross_dex: 0.1 },
};

const TOKEN_ROW = {
  symbol: 'WETH',
  address: '0x' + 'a'.repeat(40),
  components: { prior: 1.0, liquidity: 0.8, venues: 0.6, stability: 0.9, cross_dex: 0.5 },
  score: 91.2,
};

describe('QuoteAnchorResponse (EMIT-02 Layer-2, flattened)', () => {
  it('parses the flattened 8-key payload (view + tokens)', () => {
    const r = QuoteAnchorResponseSchema.safeParse({ ...VIEW, tokens: [TOKEN_ROW] });
    expect(r.success).toBe(true);
  });

  it('strict: snapshot internals (pairs_by_token…) riding the response FAIL', () => {
    const r = QuoteAnchorResponseSchema.safeParse({ ...VIEW, tokens: [], pairs_by_token: {} });
    expect(r.success).toBe(false);
  });

  it('a wrapped { anchor, tokens } envelope FAILS — the contract is flattened', () => {
    const r = QuoteAnchorResponseSchema.safeParse({ anchor: VIEW, tokens: [TOKEN_ROW] });
    expect(r.success).toBe(false);
  });

  it('the base view stays the 7-key strict contract (tokens NOT required there)', () => {
    expect(QuoteAnchorViewSchema.safeParse(VIEW).success).toBe(true);
    expect(QuoteAnchorViewSchema.safeParse({ ...VIEW, tokens: [] }).success).toBe(false);
  });

  it('empty token table = honest empty (no fabricated rows, RULE 00)', () => {
    expect(QuoteAnchorResponseSchema.safeParse({ ...VIEW, tokens: [] }).success).toBe(true);
  });
});

// ─── EMIT-03 preview envelope (co-landing d9, 2026-08-24) ─────────────────
// Vectors mirror the api-server quote-anchor.test.ts fixture (SNAPSHOT_OBJ ×
// CHANGE_W: USDC→WETH flip with affected 42+17/60+25 and version 3→4).
const IMPACT_FLIP = {
  graph_rebuild_required: false,
  quote_revaluation_required: true,
  quote_cache_invalidation_required: true,
  affected_pairs: 59,
  affected_edges: 85,
  affected_cached_routes: 0,
  current_quote_version: 3,
  proposed_quote_version: 4,
  topology_version_unchanged: true,
};
const WETH_C = { prior: 80, liquidity: 100, venues: 100, stability: 90, cross_dex: 100 };
const USDC_C = { prior: 100, liquidity: 95, venues: 90, stability: 100, cross_dex: 95 };
const PREVIEW_FLIP = {
  impact: IMPACT_FLIP,
  proposed_quote_symbol: 'WETH',
  proposed_quote_score: 100.0,
  proposed_tokens: [
    { symbol: 'WETH', address: '0x' + 'b'.repeat(40), components: WETH_C, score: 100.0 },
    { symbol: 'USDC', address: '0x' + 'a'.repeat(40), components: USDC_C, score: 94.0 },
  ],
};

describe('QuotePreviewResponse (EMIT-03 envelope)', () => {
  it('parses the 4-key envelope: impact + 3 sketch fields', () => {
    const r = QuotePreviewResponseSchema.safeParse(PREVIEW_FLIP);
    expect(r.success).toBe(true);
    expect(Object.keys(r.data!).sort()).toEqual(
      ['impact', 'proposed_quote_symbol', 'proposed_quote_score', 'proposed_tokens'].sort(),
    );
    expect(Object.keys(r.data!.impact).sort()).toHaveLength(9);
  });

  it('strict: a wrapped { preview: … } envelope FAILS — the contract is flat', () => {
    expect(QuotePreviewResponseSchema.safeParse({ preview: PREVIEW_FLIP }).success).toBe(false);
  });

  it('QB-TOPOLOGY-01 literal: graph_rebuild_required=true can NEVER parse', () => {
    const drifted = { ...PREVIEW_FLIP, impact: { ...IMPACT_FLIP, graph_rebuild_required: true } };
    expect(QuotePreviewResponseSchema.safeParse(drifted).success).toBe(false);
  });

  it('QB-TOPOLOGY-01 literal: topology_version_unchanged=false can NEVER parse', () => {
    const drifted = {
      ...PREVIEW_FLIP,
      impact: { ...IMPACT_FLIP, topology_version_unchanged: false },
    };
    expect(QuotePreviewResponseSchema.safeParse(drifted).success).toBe(false);
  });

  it('proposed_tokens rows are strict — preview sidecars on a row FAIL', () => {
    const junk = {
      ...PREVIEW_FLIP,
      proposed_tokens: [{ ...PREVIEW_FLIP.proposed_tokens[0], pairs_by_symbol: { WETH: 17 } }],
    };
    expect(QuotePreviewResponseSchema.safeParse(junk).success).toBe(false);
  });

  it('empty proposed_tokens FAILS min(1) — corrupted snapshots are 503, never a 200', () => {
    expect(
      QuotePreviewResponseSchema.safeParse({ ...PREVIEW_FLIP, proposed_tokens: [] }).success,
    ).toBe(false);
  });
});
