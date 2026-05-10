import { z } from "zod";

export const KillSwitchStateSchema = z.object({
  enabled: z.boolean(),
  reason: z.string().nullable(),
  triggered_by: z.string().nullable(),
  updated_at: z.string(),
});

export const StatusResponseSchema = z.object({
  ok: z.boolean(),
  services: z.record(
    z.string(),
    z.object({
      ok: z.boolean(),
      status: z.number().optional(),
      detail: z.string().optional(),
    }),
  ),
  killswitch: KillSwitchStateSchema.nullable(),
  env: z.string(),
  version: z.string(),
  ts: z.string(),
});

// C5 fix (audit 2026-05-10): paper_status surfaces whether a candidate is
// viable for the paper P&L (rejection_reason IS NULL) or was rejected.
export const PaperStatusSchema = z.enum(["paper_viable", "paper_rejected"]);
export type PaperStatus = z.infer<typeof PaperStatusSchema>;

export const OpportunityRowSchema = z.object({
  id: z.string(),
  chain_id: z.number(),
  strategy_kind: z.string(),
  dex_a: z.string(),
  dex_b: z.string().nullable(),
  pair_symbol: z.string().nullable(),
  token_in: z.string(),
  token_out: z.string(),
  amount_in_wei: z.string(),
  // GROSS profit (pre-cost). The dashboard MUST NOT label this as "Net".
  expected_profit_usd: z.number().nullable(),
  // C5 fix (audit 2026-05-10): NET profit (gross - gas - slippage - relay
  // fee - flashloan fee - failure buffer). This is the column the
  // /opportunities table now displays under "Net Profit".
  // R8 fail-honest: null when data was not yet computed.
  // .optional() so older payloads without this field still parse.
  net_expected_profit_usd: z.number().nullable().optional(),
  roi_pct: z.number().nullable(),
  risk_score: z.number().nullable(),
  rejection_reason: z.string().nullable().optional(),
  paper_status: PaperStatusSchema.optional(),
  chains_used: z.array(z.number()).optional(),
  dexes_used: z.array(z.string()).optional(),
  block_number: z.number().nullable(),
  status: z.string(),
  detected_at: z.string(),
  trace_id: z.string(),
});

export const OpportunitiesLiveSchema = z.object({
  count: z.number(),
  window: z.string(),
  items: z.array(OpportunityRowSchema),
  ts: z.string(),
});

export const RiskAlertRowSchema = z.object({
  id: z.string(),
  event_type: z.enum(["circuit_breaker", "kill_switch", "blacklist_hit", "degradation", "manual"]),
  severity: z.enum(["info", "warning", "critical"]),
  source_service: z.string(),
  payload: z.unknown(),
  trace_id: z.string().nullable(),
  opportunity_id: z.string().nullable(),
  created_at: z.string(),
});

export const RiskAlertsResponseSchema = z.object({
  window_hours: z.number(),
  killswitch: KillSwitchStateSchema.nullable(),
  alerts: z.array(RiskAlertRowSchema),
  ts: z.string(),
});

export const ExecutionRowSchema = z.object({
  id: z.string(),
  tx_hash: z.string().nullable(),
  bundle_hash: z.string().nullable(),
  relay_name: z.string(),
  status: z.enum(["submitted", "included", "reverted", "dropped", "replaced", "not_implemented"]),
  block_included: z.number().nullable(),
  gas_used_wei: z.string().nullable(),
  gas_price_effective_wei: z.string().nullable(),
  expected_profit_usd: z.number().nullable(),
  actual_profit_usd: z.number().nullable(),
  error_message: z.string().nullable(),
  trace_id: z.string(),
  submitted_at: z.string(),
  confirmed_at: z.string().nullable(),
  chain_id: z.number(),
  strategy_kind: z.string(),
  pair_symbol: z.string().nullable(),
});

export const ExecutionsRecentSchema = z.object({
  count: z.number(),
  items: z.array(ExecutionRowSchema),
  ts: z.string(),
});

export const ReconSummarySchema = z.object({
  window_hours: z.number(),
  totals: z.object({
    total: z.number(),
    included: z.number(),
    reverted: z.number(),
    dropped: z.number(),
    avg_pnl_included_usd: z.number().nullable(),
    avg_confirm_latency_ms: z.number().nullable(),
  }),
  revert_rate: z.number().nullable(),
  top_strategies: z.array(
    z.object({
      strategy_kind: z.string(),
      chain_id: z.number(),
      sample_count: z.number(),
      success_rate: z.number().nullable(),
      revert_rate: z.number().nullable(),
      avg_profit_usd: z.number().nullable(),
      score: z.number().nullable(),
      window_end: z.string(),
    }),
  ),
  critical_anomalies_24h: z.array(
    z.object({
      event_type: z.string(),
      severity: z.string(),
      source_service: z.string(),
      payload: z.unknown(),
      created_at: z.string(),
    }),
  ),
  ts: z.string(),
});

export const ReconTimeseriesPointSchema = z.object({
  bucket_start: z.string(),
  attempts: z.number(),
  included: z.number(),
  reverted: z.number(),
  avg_pnl_included_usd: z.number().nullable(),
  revert_rate: z.number().nullable(),
});

export const ReconTimeseriesResponseSchema = z.object({
  window_hours: z.number(),
  bucket_minutes: z.number(),
  points: z.array(ReconTimeseriesPointSchema),
  ts: z.string(),
});

export const AppConfigViewSchema = z.object({
  system: z.object({
    env: z.string(),
    kill_switch_enabled_default: z.boolean(),
    service_name_prefix: z.string(),
  }),
  risk: z.record(z.string(), z.union([z.number(), z.boolean()])),
  execution: z.record(z.string(), z.union([z.number(), z.boolean(), z.string()])),
  observability: z.record(z.string(), z.union([z.boolean(), z.string()])),
  chains: z.array(
    z.object({
      chain_id: z.number(),
      name: z.string(),
      enabled: z.boolean(),
    }),
  ),
  relays: z.array(
    z.object({
      name: z.string(),
      enabled: z.boolean(),
      chains: z.array(z.number()),
      endpoint: z.string(),
    }),
  ),
  scoring: z.record(z.string(), z.number()),
  token_safety: z.record(z.string(), z.union([z.string(), z.number()])),
  circuit_breakers: z.array(
    z.object({
      name: z.string(),
      threshold: z.number(),
      window_ms: z.number(),
      cooldown_ms: z.number(),
    }),
  ),
});

export const RelayRowSchema = z.object({
  name: z.string(),
  chain_id: z.number(),
  endpoint: z.string().nullable(),
  auth_scheme: z.enum(["none", "x-flashbots-signature", "bearer", "header-auth", "custom"]),
  enabled: z.boolean(),
  priority: z.number(),
});

export const RelaysResponseSchema = z.object({
  count: z.number(),
  items: z.array(RelayRowSchema),
  ts: z.string(),
});

export const OnboardingStatusSchema = z.object({
  org_id: z.string(),
  phase_1_completed_at: z.string().nullable(),
  phase_1_completed_by: z.string().nullable(),
  phase_1_vault_sealed_healthy: z.boolean(),
  phase_2_completed_at: z.string().nullable(),
  phase_2_completed_by: z.string().nullable(),
  phase_2_rpc_probe_ok: z.boolean(),
  phase_3_completed_at: z.string().nullable(),
  phase_3_completed_by: z.string().nullable(),
  phase_4_completed_at: z.string().nullable(),
  phase_4_completed_by: z.string().nullable(),
  phase_4_signer_zero_balance_verified: z.boolean(),
  phase_5_completed_at: z.string().nullable(),
  phase_5_completed_by: z.string().nullable(),
  phase_5_paper_mode_off_at: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});

export const OnboardingPhase1ResultSchema = z.object({
  phase_1_completed_at: z.string(),
  phase_1_completed_by: z.string(),
  phase_1_vault_sealed_healthy: z.boolean(),
});

export const OnboardingPhase2ResultSchema = z.object({
  phase_2_completed_at: z.string(),
  phase_2_completed_by: z.string(),
  phase_2_rpc_probe_ok: z.boolean(),
});

export const OnboardingPhase3ResultSchema = z.object({
  phase_3_completed_at: z.string(),
  phase_3_completed_by: z.string(),
});

export const OnboardingPhase4ResultSchema = z.object({
  phase_4_completed_at: z.string(),
  phase_4_completed_by: z.string(),
  phase_4_signer_zero_balance_verified: z.boolean(),
});

export const OnboardingPhase5ResultSchema = z.object({
  phase_5_completed_at: z.string(),
  phase_5_completed_by: z.string(),
  phase_5_paper_mode_off_at: z.string().nullable(),
});

// Live readiness checklist (PR Live-Readiness, option C dynamic verifiers)
export const ReadinessStatusSchema = z.enum(["green", "yellow", "red", "pending"]);
export const ReadinessGroupSchema = z.enum([
  "security_compliance",
  "audit_trail",
  "risk_doctrines",
  "tokens_strategies",
  "contracts",
  "operations",
]);
export const ReadinessEvidenceSchema = z.object({
  kind: z.enum(["commit", "file", "endpoint", "db_query", "shell", "config"]),
  ref: z.string(),
});
export const ReadinessItemSchema = z.object({
  id: z.string(),
  group: ReadinessGroupSchema,
  label: z.string(),
  status: ReadinessStatusSchema,
  reason: z.string(),
  evidence: ReadinessEvidenceSchema.optional(),
  doctrine: z.string().optional(),
  verified_at: z.string(),
});
export const ReadinessReportSchema = z.object({
  items: z.array(ReadinessItemSchema),
  summary: z.object({
    green: z.number().int().nonnegative(),
    yellow: z.number().int().nonnegative(),
    red: z.number().int().nonnegative(),
    pending: z.number().int().nonnegative(),
    total: z.number().int().nonnegative(),
  }),
  flip_blocked: z.boolean(),
  generated_at: z.string(),
});

// ─────── Trading Config (operator-tunable strategy parameters) ───────

export const GasPriceStrategySchema = z.enum(["fixed", "dynamic_basefee_plus_tip", "percentile_75"]);

// Migration 056 — per-strategy fine-grained config (mirrors backend Zod 1:1).
//
// Design note: NO `.default()` on inner fields. Zod's `.default()` produces an
// asymmetric type (input optional, output required) which propagates through
// `z.record(...)` value types and triggers TS2719 "Two different types with
// this name exist, but they are unrelated" at consumer sites that infer the
// schema type. The backend is responsible for returning fully-populated rows
// (its own Zod has `.default()` on input parsing); the frontend treats every
// field as required-on-output. Forms construct concrete defaults explicitly.
export const RouteConstraintsSchema = z.object({
  min_legs: z.number().int().min(1).max(8),
  max_legs: z.number().int().min(1).max(8),
  require_atomic: z.boolean(),
  allow_cross_chain: z.boolean(),
  allowed_base_tokens: z.array(z.string()),
  allowed_quote_tokens: z.array(z.string()),
  min_pool_tvl_usd: z.number().nonnegative().nullable(),
  min_pool_volume_24h_usd: z.number().nonnegative().nullable(),
});

export const StrategyRuntimeConfigSchema = z.object({
  enabled: z.boolean(),
  allowed_chain_ids: z.array(z.number().int().positive()),
  enabled_dex_ids: z.array(z.string().uuid()).nullable(),
  // Audit 2026-05-10 follow-up: per-strategy pool allowlist (UUIDs from
  // pools.id). Reserved for the next sprint's PoolsTab writeback UI.
  enabled_pool_ids: z.array(z.string().uuid()).nullable(),
  enabled_protocol_types: z.array(z.string()),
  min_profit_usd: z.number().nonnegative().nullable(),
  min_roi_pct: z.number().nonnegative().nullable(),
  max_slippage_pct: z.number().min(0).max(50).nullable(),
  max_price_impact_pct: z.number().min(0).max(50).nullable(),
  max_gas_usd: z.number().nonnegative().nullable(),
  simulation_capital_usd: z.number().positive().nullable(),
  per_token_caps_usd: z.record(z.string(), z.number().positive()),
  route_constraints: RouteConstraintsSchema,
});

export const StrategyConfigsSchema = z.record(z.string(), StrategyRuntimeConfigSchema);

export type RouteConstraints = z.infer<typeof RouteConstraintsSchema>;
export type StrategyRuntimeConfig = z.infer<typeof StrategyRuntimeConfigSchema>;
export type StrategyConfigs = z.infer<typeof StrategyConfigsSchema>;

/// Concrete default used when constructing a fresh strategy entry in the UI.
/// Mirrors the Rust `StrategyRuntimeConfig::Default` semantics. Operator
/// then mutates fields via the editor before saving.
export const DEFAULT_STRATEGY_RUNTIME_CONFIG: StrategyRuntimeConfig = {
  enabled: true,
  allowed_chain_ids: [],
  enabled_dex_ids: null,
  enabled_pool_ids: null,
  enabled_protocol_types: [],
  min_profit_usd: null,
  min_roi_pct: null,
  max_slippage_pct: null,
  max_price_impact_pct: null,
  max_gas_usd: null,
  simulation_capital_usd: null,
  per_token_caps_usd: {},
  route_constraints: {
    min_legs: 1,
    max_legs: 8,
    require_atomic: false,
    allow_cross_chain: false,
    allowed_base_tokens: [],
    allowed_quote_tokens: [],
    min_pool_tvl_usd: null,
    min_pool_volume_24h_usd: null,
  },
};

const TradingConfigBaseFields = {
  chain_id: z.number().int().positive(),
  capital_usd: z.number().nonnegative(),
  base_token_symbol: z.string().min(1).max(16),
  base_token_price_usd: z.number().positive(),
  allowed_token_symbols: z.array(z.string().min(1).max(16)),
  // BUG-2 fix: per-token USD prices (operator-managed). Optional in frontend
  // type so legacy components that don't manage this field still typecheck;
  // backend Zod has .default({}) so unset arrives as {}. Same pattern below
  // for the simulation maps — keeps the inferred type ergonomic.
  token_prices_usd: z.record(z.string(), z.number().positive()).optional(),
  // Simulation knobs (paper-trade preview).
  simulation_capital_usd: z.number().positive().nullable().optional(),
  simulation_per_token_amounts_usd: z.record(z.string(), z.number().positive()).optional(),
  simulation_per_strategy_caps_usd: z.record(z.string(), z.number().positive()).optional(),
  simulation_target_profit_usd: z.number().nonnegative().nullable().optional(),
  simulation_target_roi_pct: z.number().nonnegative().nullable().optional(),
  min_profit_usd: z.number().nonnegative(),
  min_roi_pct: z.number().nonnegative(),
  min_landing_probability: z.number().min(0).max(1),
  min_liquidity_confidence: z.number().min(0).max(1),
  max_token_risk_score: z.number().min(0).max(1),
  gas_price_strategy: GasPriceStrategySchema,
  fixed_gas_price_gwei: z.number().nullable(),
  gas_estimate_units: z.number().int().positive(),
  max_slippage_pct: z.number().min(0).max(50),
  failure_risk_buffer_pct: z.number().min(0),
  flashloan_fee_pct: z.number().min(0),
  enabled_strategies: z.array(z.string()),
  // Phase 2 route finder: which DEX IDs the searcher should scan.
  // null = all DEXes enabled (default). Array of UUIDs restricts the scan.
  enabled_dex_ids: z.array(z.string().uuid()).nullable().optional(),
  // Migration 056 — per-strategy fine-grained config keyed by strategy_kind.
  // Empty object {} = no per-strategy overrides; chain-level enabled_strategies
  // / enabled_dex_ids govern instead. Default is `{}` (input optional, output
  // always present) to keep the inferred type strict — `.optional()` here would
  // bleed `| undefined` through every Record<string, T> consumer.
  strategy_configs: StrategyConfigsSchema,
  enabled: z.boolean(),
  // ── Net Profit Gate — Sprint A+B+C (migrations 047+048) ──────────────
  // Capital cost fields (Sprint A).
  // capital_cost_rate_annual_pct: hurdle rate for capital deployed.
  //   0 = ignore (appropriate for flash-loan arb where capital isn't locked).
  //   Set > 0 to require profit > (capital × rate/8760) per opportunity.
  capital_cost_rate_annual_pct: z.number().min(0).max(50).optional(),
  // ops_overhead_usd_per_attempt: amortised infra/RPC cost per scored attempt.
  //   Deducted from gross profit before the net_profit gate comparison.
  ops_overhead_usd_per_attempt: z.number().min(0).max(100).optional(),
  // Spread sanity gate (Sprint B).
  // spread_sanity_mult: reject if AMM-reported spread > oracle reference × mult.
  //   Default 3.0 = flag anything that looks 3× wider than oracle says it should be.
  spread_sanity_mult: z.number().min(1).max(100).optional(),
  // p_copied probability model (Sprint C).
  // p_copied_volume_threshold_usd: 24h pool volume at which copy probability = 0.5.
  //   Lower value → model assumes more copy-trading at low volumes.
  p_copied_volume_threshold_usd: z.number().positive().optional(),
  // p_copied_max: hard ceiling on P(opportunity is a copy-trade front-run).
  //   0.5 = reject any opportunity where model assigns ≥50% copy probability.
  p_copied_max: z.number().min(0).max(1).optional(),
  updated_at: z.string(),
  updated_by: z.string().nullable(),
};

export const TradingConfigConfiguredSchema = z.object({
  configured: z.literal(true),
  ...TradingConfigBaseFields,
});

export const TradingConfigUnconfiguredSchema = z.object({
  configured: z.literal(false),
  chain_id: z.number().int().positive(),
});

export const TradingConfigResponseSchema = z.discriminatedUnion("configured", [
  TradingConfigConfiguredSchema,
  TradingConfigUnconfiguredSchema,
]);

export const TradingConfigPutResultSchema = z.object({
  ok: z.literal(true),
  subscribers_notified: z.number().int().nonnegative(),
  ...TradingConfigBaseFields,
});

// ─────── Strategy Catalog (Sprint 2 — universal MEV strategy library) ───

export const LifecycleStatusSchema = z.enum([
  "live",
  "designed",
  "scaffold",
  "not_started",
  "defensive_only",
]);
export type LifecycleStatus = z.infer<typeof LifecycleStatusSchema>;

export const StrategyCatalogEntrySchema = z.object({
  kind: z.string(),
  display_name: z.string(),
  description: z.string(),
  category: z.string(),
  is_implemented: z.boolean(),
  is_default: z.boolean(),
  // Migration 035 — operative state for UI badge + toggle gating.
  // Older catalog rows may still have null here; UI treats null as "unknown".
  lifecycle_status: LifecycleStatusSchema.nullable().optional(),
  risk_level: z.enum(["low", "medium", "high", "very-high"]),
  capital_required: z.string().nullable(),
  expected_complexity: z.string().nullable(),
  competitive_advantage: z.enum(["baja", "media", "alta", "muy-alta", "extrema"]).nullable(),
  ethical_constraint: z.enum(["none", "defensive_only"]).nullable(),
  skill_refs: z.array(z.string()),
  requires_flashloan: z.boolean(),
  chains_supported: z.array(z.number().int()),
});

export const StrategyCatalogResponseSchema = z.object({
  entries: z.array(StrategyCatalogEntrySchema),
});

// ─────── Derived types ───────

export type KillSwitchState = z.infer<typeof KillSwitchStateSchema>;
export type StatusResponse = z.infer<typeof StatusResponseSchema>;
export type OpportunityRow = z.infer<typeof OpportunityRowSchema>;
export type OpportunitiesLive = z.infer<typeof OpportunitiesLiveSchema>;
export type RiskAlertRow = z.infer<typeof RiskAlertRowSchema>;
export type RiskAlertsResponse = z.infer<typeof RiskAlertsResponseSchema>;
export type ExecutionRow = z.infer<typeof ExecutionRowSchema>;
export type ExecutionsRecent = z.infer<typeof ExecutionsRecentSchema>;
export type ReconSummary = z.infer<typeof ReconSummarySchema>;
export type ReconTimeseriesPoint = z.infer<typeof ReconTimeseriesPointSchema>;
export type ReconTimeseriesResponse = z.infer<typeof ReconTimeseriesResponseSchema>;
export type AppConfigView = z.infer<typeof AppConfigViewSchema>;
export type RelayRow = z.infer<typeof RelayRowSchema>;
export type RelaysResponse = z.infer<typeof RelaysResponseSchema>;
export type OnboardingStatus = z.infer<typeof OnboardingStatusSchema>;
export type OnboardingPhase1Result = z.infer<typeof OnboardingPhase1ResultSchema>;
export type OnboardingPhase2Result = z.infer<typeof OnboardingPhase2ResultSchema>;
export type OnboardingPhase3Result = z.infer<typeof OnboardingPhase3ResultSchema>;
export type OnboardingPhase4Result = z.infer<typeof OnboardingPhase4ResultSchema>;
export type OnboardingPhase5Result = z.infer<typeof OnboardingPhase5ResultSchema>;
export type ReadinessStatus = z.infer<typeof ReadinessStatusSchema>;
export type GasPriceStrategy = z.infer<typeof GasPriceStrategySchema>;
export type TradingConfigConfigured = z.infer<typeof TradingConfigConfiguredSchema>;
export type TradingConfigResponse = z.infer<typeof TradingConfigResponseSchema>;
export type TradingConfigPutResult = z.infer<typeof TradingConfigPutResultSchema>;
export type StrategyCatalogEntry = z.infer<typeof StrategyCatalogEntrySchema>;
export type StrategyCatalogResponse = z.infer<typeof StrategyCatalogResponseSchema>;
export type ReadinessGroup = z.infer<typeof ReadinessGroupSchema>;
export type ReadinessItem = z.infer<typeof ReadinessItemSchema>;
export type ReadinessReport = z.infer<typeof ReadinessReportSchema>;

export const AuditLogRowSchema = z.object({
  id: z.string(),
  actor: z.string(),
  action: z.string(),
  target_kind: z.string().nullable(),
  target_id: z.string().nullable(),
  before_state: z.unknown().nullable(),
  after_state: z.unknown().nullable(),
  ip_address: z.string().nullable(),
  user_agent: z.string().nullable(),
  trace_id: z.string().nullable(),
  created_at: z.string(),
});

export const AuditLogsResponseSchema = z.object({
  items: z.array(AuditLogRowSchema),
  next_cursor: z.string().nullable(),
  ts: z.string(),
});

export type AuditLogRow = z.infer<typeof AuditLogRowSchema>;
export type AuditLogsResponse = z.infer<typeof AuditLogsResponseSchema>;

// ─────── DeFi data schemas (defiRouter in api-server) ───────
// Rows use passthrough() because DB columns may change; we validate the envelope.

export const DefiChainRowSchema = z.object({
  chain_id: z.number(),
  name: z.string(),
  rpc_url: z.string().optional(),
  is_active: z.boolean().optional(),
}).passthrough();

export const DefiChainsResponseSchema = z.object({
  success: z.boolean(),
  data: z.array(DefiChainRowSchema),
});

export const DefiRpcRowSchema = z.object({
  chain_id: z.number().optional(),
  url: z.string().optional(),
  type: z.string().optional(),
  latency_ms: z.number().nullable().optional(),
  health_status: z.string().optional(),
}).passthrough();

export const DefiRpcsResponseSchema = z.object({
  success: z.boolean(),
  data: z.array(DefiRpcRowSchema),
});

export const DefiPoolRowSchema = z.object({
  address: z.string().optional(),
  token0_symbol: z.string().optional(),
  token1_symbol: z.string().optional(),
  dex: z.string().optional(),
  active: z.boolean().optional(),
}).passthrough();

export const DefiPoolsResponseSchema = z.object({
  success: z.boolean(),
  data: z.array(DefiPoolRowSchema),
});

export const DefiMetricsResponseSchema = z.object({
  success: z.boolean(),
  data: z.object({
    active_workers: z.number(),
    cpu_usage_pct: z.number(),
    memory_usage_mb: z.number(),
    uptime_seconds: z.number(),
    kernel_bypass_active: z.boolean(),
  }),
});

export type DefiChainRow = z.infer<typeof DefiChainRowSchema>;
export type DefiChainsResponse = z.infer<typeof DefiChainsResponseSchema>;
export type DefiRpcRow = z.infer<typeof DefiRpcRowSchema>;
export type DefiRpcsResponse = z.infer<typeof DefiRpcsResponseSchema>;
export type DefiPoolRow = z.infer<typeof DefiPoolRowSchema>;
export type DefiPoolsResponse = z.infer<typeof DefiPoolsResponseSchema>;
export type DefiMetricsResponse = z.infer<typeof DefiMetricsResponseSchema>;
