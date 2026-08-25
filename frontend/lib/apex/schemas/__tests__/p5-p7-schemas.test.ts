import { describe, expect, it } from 'vitest';

import {
  DetectorCatalogResponseSchema,
  PairViewSchema,
  PairsResponseSchema,
  StrategyCatalogResponseSchema,
  TokenKeyRefSchema,
} from '../index';

// ── Verbatim fixtures (generated backend table, byte-exact wire samples) ──
// MEV-01-001 row as served by QUOTEBASE_STRATEGY_CATALOG (EMIT-07).
const STRATEGY_ROW_MEV_01_001 = {
  mev_id: 'MEV-01-001',
  group: 1,
  name: 'DEX–DEX arbitrage',
  family: 'Arbitrajes spot DEX dentro de una misma cadena',
  surface: 'DEX_AMM',
  backend_module: 'route_graph_engine',
  detector_id: 'R_CLOSED_CYCLE',
  min_legs: 2,
  max_legs: 8,
  allowed_hops: [2, 3, 4, 5, 6, 7],
  graph_model: 'TOKEN_MULTIGRAPH',
  quotebase_role: 'PRIMARY_PAIR+NUMERAIRE',
  search_policy: 'dirty pair/edge → closed-cycle/order route search',
  execution_class: 'DETERMINISTIC_EXECUTABLE',
  primary_ops: ['op_27 Path Ordering', 'op_21 Newton-Raphson', 'op_15 Golden Section', 'op_16 Kelly'],
  discovery_equation:
    'Q_R(x)=q_n(...q_2(q_1(x))); Π_R(x)=Q_R(x)-x-C_R(x). Opportunity iff max_x Π_R(x)>0. Marginal prefilter: Σ_e[-ln((1-fee_e)·rate_e)]<0.',
  gate_live: 'Sim PASS + net_profit>0 + risk_score>=70 + private route cuando aplique',
  status: 'ROUTE_READY',
} as const;

// R_CLOSED_CYCLE row as served by QUOTEBASE_DETECTOR_CATALOG (EMIT-08).
const DETECTOR_ROW_R_CLOSED_CYCLE = {
  detector_id: 'R_CLOSED_CYCLE',
  strategies_count: 25,
  example_surface: 'DEX_AMM',
  example_mev: 'MEV-01-001',
  execution_class: 'DETERMINISTIC_EXECUTABLE',
  primary_ops: ['op_27 Path Ordering', 'op_21 Newton-Raphson', 'op_15 Golden Section', 'op_16 Kelly'],
  secondary_ops: ['op_01 SVD', 'op_22 Monte Carlo', 'op_26 Flash Loan', 'op_30 GNN Encoder'],
  exact_discovery_criterion:
    'Q_R(x)=q_n(...q_2(q_1(x))); Π_R(x)=Q_R(x)-x-C_R(x). Opportunity iff max_x Π_R(x)>0. Marginal prefilter: Σ_e[-ln((1-fee_e)·rate_e)]<0.',
  required_data:
    'Full ordered route legs; per-leg protocol adapter; reserves/slot0/ticks/bins; token decimals; pool fees; same-block state; gas; optional flash fee.',
  frontend_config: [
    'enabled',
    'min_profit_usd',
    'min_roi_pct',
    'simulation_capital_usd',
    'min/max legs from cartridge',
    'require_same_block=true',
    'max_slippage_pct',
    'max_price_impact_pct',
    'max_gas_usd',
    'allowed base/quote tokens',
    'pool/DEX/protocol allowlists',
  ],
  graph_policy: 'dirty pair/edge → closed-cycle/order route search',
  hop_envelope: { min: 2, max: 7 },
  hot_seed: 'SEED_CANDIDATE',
  do_not_do: 'Do not replace detector math with generic spot-price spread.',
} as const;

const TOKEN_A = {
  chain_id: 1,
  address: '0x' + 'a'.repeat(40),
  symbol: 'WETH',
  decimals: 18,
};
const TOKEN_B = {
  chain_id: 1,
  address: '0x' + 'b'.repeat(40),
  symbol: 'USDC',
  decimals: 6,
};

const PAIR_VIEW = {
  chain_id: 1,
  token_a: TOKEN_A,
  token_b: TOKEN_B,
  pools: [
    {
      pool_address: '0x' + 'c'.repeat(40),
      venue: 'uniswap_v2',
      fee_bps: 30,
      reserves_a: '123456789012345678901234567890',
      reserves_b: '9876543210',
    },
  ],
  venue_count: 1,
  alpha_forward: 1.0004,
  alpha_reverse: 0.9997, // r15: independently computed, NEVER −forward
  dirty: true,
  last_reserve_update: null, // R8: never synced yet — honest null
};

describe('TokenKeyRef (P5 leg identity)', () => {
  it('parses a resolved leg', () => {
    expect(TokenKeyRefSchema.safeParse(TOKEN_A).success).toBe(true);
  });

  it('rejects negative decimals', () => {
    expect(TokenKeyRefSchema.safeParse({ ...TOKEN_A, decimals: -1 }).success).toBe(false);
  });
});

describe('StrategyCatalogResponse (P6 / EMIT-07)', () => {
  it('parses the verbatim MEV-01-001 wire row inside { entries }', () => {
    const r = StrategyCatalogResponseSchema.safeParse({ entries: [STRATEGY_ROW_MEV_01_001] });
    expect(r.success).toBe(true);
  });

  it('rejects a { rows } envelope — the convention is { entries }', () => {
    const r = StrategyCatalogResponseSchema.safeParse({ rows: [STRATEGY_ROW_MEV_01_001] });
    expect(r.success).toBe(false);
  });

  it('accepts an empty catalog (honest empty ≠ 264 hardcoded — counts are contract tests)', () => {
    expect(StrategyCatalogResponseSchema.safeParse({ entries: [] }).success).toBe(true);
  });

  it('strict: unknown key fails', () => {
    const r = StrategyCatalogResponseSchema.safeParse({
      entries: [{ ...STRATEGY_ROW_MEV_01_001, active: true }],
    });
    expect(r.success).toBe(false);
  });

  it('min_legs > max_legs fails (superRefine)', () => {
    const r = StrategyCatalogResponseSchema.safeParse({
      entries: [{ ...STRATEGY_ROW_MEV_01_001, min_legs: 9, max_legs: 8 }],
    });
    expect(r.success).toBe(false);
  });

  it('legs canon up to 16 — the hot-path cap 7 is runtime policy, not schema', () => {
    const r = StrategyCatalogResponseSchema.safeParse({
      entries: [{ ...STRATEGY_ROW_MEV_01_001, min_legs: 3, max_legs: 16 }],
    });
    expect(r.success).toBe(true);
    expect(
      StrategyCatalogResponseSchema.safeParse({
        entries: [{ ...STRATEGY_ROW_MEV_01_001, max_legs: 17 }],
      }).success,
    ).toBe(false);
  });

  it('allowed_hops already expanded — raw mask ints (e.g. 0b1111111=254) fail; empty fails', () => {
    expect(
      StrategyCatalogResponseSchema.safeParse({
        entries: [{ ...STRATEGY_ROW_MEV_01_001, allowed_hops: [254] }],
      }).success,
    ).toBe(false);
    expect(
      StrategyCatalogResponseSchema.safeParse({
        entries: [{ ...STRATEGY_ROW_MEV_01_001, allowed_hops: [] }],
      }).success,
    ).toBe(false);
  });

  it('status is the SCREAMING enum — telemetry census slugs (route_ready) belong to another surface', () => {
    expect(
      StrategyCatalogResponseSchema.safeParse({
        entries: [{ ...STRATEGY_ROW_MEV_01_001, status: 'route_ready' }],
      }).success,
    ).toBe(false);
  });

  it('mev_id must be the workbook id shape', () => {
    expect(
      StrategyCatalogResponseSchema.safeParse({
        entries: [{ ...STRATEGY_ROW_MEV_01_001, mev_id: 'MEV-1-001' }],
      }).success,
    ).toBe(false);
  });
});

describe('DetectorCatalogResponse (P7 / EMIT-08)', () => {
  it('parses the verbatim R_CLOSED_CYCLE wire row inside { entries }', () => {
    const r = DetectorCatalogResponseSchema.safeParse({ entries: [DETECTOR_ROW_R_CLOSED_CYCLE] });
    expect(r.success).toBe(true);
  });

  it('AMENDMENT REGRESSION (d9 2026-08-24): FrontendKnobSpec-shaped frontend_config FAILS — phrases only', () => {
    const knobSpecPayload = [
      { key: 'solver_timeout', kind: 'number', unit: 'ms' },
      { key: 'reserve_safety_floor', kind: 'number', unit: 'usd' },
    ];
    const r = DetectorCatalogResponseSchema.safeParse({
      entries: [{ ...DETECTOR_ROW_R_CLOSED_CYCLE, frontend_config: knobSpecPayload }],
    });
    expect(r.success).toBe(false);
  });

  it('freeform phrases survive verbatim (mixed keys, sentences, "k=v")', () => {
    const phrases = ['solver timeout', 'reserve safety floor', 'require_same_block=true'];
    const r = DetectorCatalogResponseSchema.safeParse({
      entries: [{ ...DETECTOR_ROW_R_CLOSED_CYCLE, frontend_config: phrases }],
    });
    expect(r.success).toBe(true);
  });

  it('hop_envelope min > max fails', () => {
    const r = DetectorCatalogResponseSchema.safeParse({
      entries: [
        { ...DETECTOR_ROW_R_CLOSED_CYCLE, hop_envelope: { min: 8, max: 7 } },
      ],
    });
    expect(r.success).toBe(false);
  });

  it('hot_seed is the 2-valued may_seed() projection', () => {
    expect(
      DetectorCatalogResponseSchema.safeParse({
        entries: [{ ...DETECTOR_ROW_R_CLOSED_CYCLE, hot_seed: 'SEED' }],
      }).success,
    ).toBe(false);
    expect(
      DetectorCatalogResponseSchema.safeParse({
        entries: [{ ...DETECTOR_ROW_R_CLOSED_CYCLE, hot_seed: 'OBSERVE_EVIDENCE' }],
      }).success,
    ).toBe(true);
  });
});

describe('PairView / PairsResponse (P5 / EMIT-06)', () => {
  it('parses a full pair with null alpha honesty (R8) and independent alphas (r15)', () => {
    const r = PairViewSchema.safeParse(PAIR_VIEW);
    expect(r.success).toBe(true);
    if (r.success && r.data.alpha_forward !== null && r.data.alpha_reverse !== null) {
      // r15: independently computed — the pair is not forced to −forward.
      expect(r.data.alpha_forward).not.toBe(-r.data.alpha_reverse);
    }
  });

  it('§62: reserves as IEEE numbers FAIL — decimal strings only', () => {
    const r = PairViewSchema.safeParse({
      ...PAIR_VIEW,
      pools: [{ ...PAIR_VIEW.pools[0], reserves_a: 1.23e27 }],
    });
    expect(r.success).toBe(false);
  });

  it('both alphas nullable — not computed this tick is a real state', () => {
    const r = PairViewSchema.safeParse({
      ...PAIR_VIEW,
      alpha_forward: null,
      alpha_reverse: null,
    });
    expect(r.success).toBe(true);
  });

  it('envelope { entries } with empty array = honest empty universe', () => {
    expect(PairsResponseSchema.safeParse({ entries: [] }).success).toBe(true);
    expect(PairsResponseSchema.safeParse({ pairs: [] }).success).toBe(false);
  });
});
