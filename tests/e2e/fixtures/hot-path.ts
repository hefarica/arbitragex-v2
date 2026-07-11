/**
 * OMEGA Hot Path Pipeline Test Fixtures
 *
 * Provides synthetic data generators for E2E testing.
 * All data follows OMEGA lexicon and fail-honest principles.
 */

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

export interface HolonomicLoopOpportunity {
  id: string;
  chain_id: number;
  strategy_kind: StrategyKind;
  token_a: string;
  token_b: string;
  token_c?: string;
  dex_a: string;
  dex_b?: string;
  amount_in_wei: string;
  expected_topological_yield_usd?: number;
  net_expected_topological_yield_usd?: number;
  roi_pct?: number;
  risk_score?: number;
  detected_at: string; // ISO 8601
  trace_id: string;
}

export type StrategyKind =
  | "holonomic_loop"
  | "triangular"
  | "backrun"
  | "liquidation"
  | "dex_arb"
  | "flashloan_loop";

export interface SimulationOutcome {
  opportunity_id: string;
  passed: boolean;
  net_topological_yield_wei: string;
  gas_used: number;
  decoherencia_pct?: number;
  latency_ms: number;
  simulated_at: string;
}

export interface PaperExecution {
  opportunity_id: string;
  simulation_id: string;
  status: "completed" | "failed" | "timeout";
  actual_pnl_usd: number;
  execution_time_ms: number;
  executed_at: string;
}

export interface PipelineLatencySample {
  stage: PipelineStage;
  timestamp_ms: number;
  duration_ms: number;
  opportunity_id: string;
}

export type PipelineStage =
  | "detection"
  | "emission"
  | "websocket_broadcast"
  | "simulation"
  | "paper_execution"
  | "end_to_end";

// ─────────────────────────────────────────────────────────────────────────────
// Generators
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Generates a unique test opportunity ID
 */
export function generateOpportunityId(): string {
  return `test-opp-${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

/**
 * Generates a trace ID for correlation
 */
export function generateTraceId(): string {
  return `trace-${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

/**
 * Creates a valid Holonomic Loop opportunity for testing
 */
export function createHolonomicLoopOpportunity(
  overrides?: Partial<HolonomicLoopOpportunity>
): HolonomicLoopOpportunity {
  const now = new Date();
  return {
    id: generateOpportunityId(),
    chain_id: 1,
    strategy_kind: "holonomic_loop",
    token_a: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", // WETH
    token_b: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", // USDC
    token_c: "0xdAC17F958D2ee523a2206206994597C13D831ec7", // USDT
    dex_a: "uniswap_v3",
    dex_b: "sushiswap",
    amount_in_wei: "1000000000000000000", // 1 ETH
    expected_topological_yield_usd: 15.5,
    net_expected_topological_yield_usd: 12.3,
    roi_pct: 0.85,
    risk_score: 0.2,
    detected_at: now.toISOString(),
    trace_id: generateTraceId(),
    ...overrides,
  };
}

/**
 * Creates a Triangular opportunity (three-token holonomic loop)
 */
export function createTriangularOpportunity(
  overrides?: Partial<HolonomicLoopOpportunity>
): HolonomicLoopOpportunity {
  return createHolonomicLoopOpportunity({
    strategy_kind: "triangular",
    token_a: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", // WETH
    token_b: "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", // WBTC
    token_c: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", // USDC
    ...overrides,
  });
}

/**
 * Creates a Backrun opportunity
 */
export function createBackrunOpportunity(
  overrides?: Partial<HolonomicLoopOpportunity>
): HolonomicLoopOpportunity {
  return createHolonomicLoopOpportunity({
    strategy_kind: "backrun",
    token_a: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    token_b: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    dex_b: undefined,
    ...overrides,
  });
}

/**
 * Creates a simulation outcome for a given opportunity
 */
export function createSimulationOutcome(
  opportunityId: string,
  passed: boolean = true,
  overrides?: Partial<SimulationOutcome>
): SimulationOutcome {
  const netYield = passed ? "12300000000000000000" : "0"; // 12.3 ETH equivalent
  return {
    opportunity_id: opportunityId,
    passed,
    net_topological_yield_wei: netYield,
    gas_used: passed ? 150000 : 0,
    decoherencia_pct: 0.05,
    latency_ms: 25,
    simulated_at: new Date().toISOString(),
    ...overrides,
  };
}

/**
 * Creates a paper execution record
 */
export function createPaperExecution(
  opportunityId: string,
  simulationId: string,
  overrides?: Partial<PaperExecution>
): PaperExecution {
  return {
    opportunity_id: opportunityId,
    simulation_id: simulationId,
    status: "completed",
    actual_pnl_usd: 12.15, // Slight variance from simulation
    execution_time_ms: 35,
    executed_at: new Date().toISOString(),
    ...overrides,
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// Invalid Data Generators (for fail-honest testing)
// ─────────────────────────────────────────────────────────────────────────────

export interface InvalidOpportunityTestCase {
  name: string;
  data: Record<string, unknown>;
  expectedError: string;
}

export function createInvalidOpportunityCases(): InvalidOpportunityTestCase[] {
  return [
    {
      name: "empty_id",
      data: { id: "", chain_id: 1, strategy_kind: "holonomic_loop" },
      expectedError: "Empty opportunity ID",
    },
    {
      name: "missing_chain",
      data: { id: generateOpportunityId(), strategy_kind: "holonomic_loop" },
      expectedError: "Missing chain_id",
    },
    {
      name: "invalid_chain_negative",
      data: { id: generateOpportunityId(), chain_id: -1, strategy_kind: "holonomic_loop" },
      expectedError: "Invalid chain_id",
    },
    {
      name: "unknown_strategy",
      data: { id: generateOpportunityId(), chain_id: 1, strategy_kind: "unknown_xyz" },
      expectedError: "Unknown strategy_kind",
    },
    {
      name: "missing_tokens",
      data: { id: generateOpportunityId(), chain_id: 1, strategy_kind: "holonomic_loop" },
      expectedError: "Missing token addresses",
    },
    {
      name: "invalid_token_address",
      data: {
        id: generateOpportunityId(),
        chain_id: 1,
        strategy_kind: "holonomic_loop",
        token_a: "not_an_address",
        token_b: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      },
      expectedError: "Invalid token address format",
    },
    {
      name: "negative_yield",
      data: {
        id: generateOpportunityId(),
        chain_id: 1,
        strategy_kind: "holonomic_loop",
        token_a: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        token_b: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        expected_topological_yield_usd: -10.0,
      },
      expectedError: "Negative yield",
    },
  ];
}

// ─────────────────────────────────────────────────────────────────────────────
// Load Test Generators
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Generates a batch of opportunities for load testing
 */
export function generateOpportunityBatch(
  count: number,
  strategyKind: StrategyKind = "holonomic_loop"
): HolonomicLoopOpportunity[] {
  return Array.from({ length: count }, (_, i) =>
    createHolonomicLoopOpportunity({
      id: `load-test-${Date.now()}-${i}`,
      strategy_kind: strategyKind,
      expected_topological_yield_usd: 10 + Math.random() * 20,
      detected_at: new Date(Date.now() + i).toISOString(),
    })
  );
}

/**
 * Creates latency samples for statistical analysis
 */
export function generateLatencyDistribution(
  sampleCount: number,
  meanMs: number,
  stdDevMs: number
): PipelineLatencySample[] {
  const samples: PipelineLatencySample[] = [];
  const now = Date.now();

  for (let i = 0; i < sampleCount; i++) {
    // Box-Muller transform for normal distribution
    const u1 = Math.random();
    const u2 = Math.random();
    const z0 = Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
    const durationMs = Math.max(0, meanMs + z0 * stdDevMs);

    samples.push({
      stage: "end_to_end",
      timestamp_ms: now + i,
      duration_ms: Math.round(durationMs),
      opportunity_id: generateOpportunityId(),
    });
  }

  return samples;
}

// ─────────────────────────────────────────────────────────────────────────────
// Redis Stream Helpers
// ─────────────────────────────────────────────────────────────────────────────

export const STREAM_KEYS = {
  HOT_DETECTED: "arbx:hot:detected",
  HOT_SIMULATED: "arbx:hot:simulated",
  HOT_PAPER_EXECUTED: "arbx:hot:paper_executed",
  OPPORTUNITIES_DETECTED: "arbx:opps:detected",
  SCORING_SCORED: "arbx:scoring:scored",
} as const;

export const CONSUMER_GROUPS = {
  HOT_OPPORTUNITIES: "ws-emitter-g0",
  PAPER_EXECUTOR: "paper-executor-g0",
  SELECTOR: "selector-g0",
} as const;

export interface StreamInfo {
  length: number;
  radixTreeKeys: number;
  radixTreeNodes: number;
  groups: number;
  lastGeneratedId: string;
  firstEntry?: [string, string[]];
  lastEntry?: [string, string[]];
}

/**
 * Expected stream configuration per OMEGA spec
 */
export const STREAM_CONFIG = {
  [STREAM_KEYS.HOT_DETECTED]: {
    maxlen: 10000,
    ttl: 300, // 5 minutes
    description: "Hot path detected opportunities",
  },
  [STREAM_KEYS.HOT_SIMULATED]: {
    maxlen: 5000,
    ttl: 300,
    description: "Hot path simulation results",
  },
  [STREAM_KEYS.HOT_PAPER_EXECUTED]: {
    maxlen: 5000,
    ttl: 600, // 10 minutes
    description: "Paper execution records",
  },
  [STREAM_KEYS.OPPORTUNITIES_DETECTED]: {
    maxlen: 10000,
    ttl: 86400, // 24 hours
    description: "Legacy opportunity detection stream",
  },
} as const;

// ─────────────────────────────────────────────────────────────────────────────
// Assert Helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Validates that latency meets OMEGA <100ms p95 requirement
 */
export function validateLatencyBudget(
  latencies: number[],
  budgetMs: number = 100
): { p95: number; withinBudget: boolean } {
  if (latencies.length === 0) {
    return { p95: 0, withinBudget: false };
  }

  const sorted = [...latencies].sort((a, b) => a - b);
  const p95Index = Math.floor(sorted.length * 0.95);
  const p95 = sorted[Math.min(p95Index, sorted.length - 1)]!;

  return {
    p95,
    withinBudget: p95 <= budgetMs,
  };
}

/**
 * Validates stream boundedness (MAXLEN enforcement)
 */
export function validateStreamBoundedness(
  currentLength: number,
  expectedMaxlen: number,
  tolerance: number = 0.1
): { bounded: boolean; overage: number } {
  const maxWithTolerance = expectedMaxlen * (1 + tolerance);
  return {
    bounded: currentLength <= maxWithTolerance,
    overage: Math.max(0, currentLength - expectedMaxlen),
  };
}
