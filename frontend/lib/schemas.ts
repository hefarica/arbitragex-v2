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
  expected_profit_usd: z.number().nullable(),
  roi_pct: z.number().nullable(),
  risk_score: z.number().nullable(),
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

const TradingConfigBaseFields = {
  chain_id: z.number().int().positive(),
  capital_usd: z.number().nonnegative(),
  base_token_symbol: z.string().min(1).max(16),
  base_token_price_usd: z.number().positive(),
  allowed_token_symbols: z.array(z.string().min(1).max(16)),
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
  enabled: z.boolean(),
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
export type ReadinessStatus = z.infer<typeof ReadinessStatusSchema>;
export type GasPriceStrategy = z.infer<typeof GasPriceStrategySchema>;
export type TradingConfigConfigured = z.infer<typeof TradingConfigConfiguredSchema>;
export type TradingConfigResponse = z.infer<typeof TradingConfigResponseSchema>;
export type TradingConfigPutResult = z.infer<typeof TradingConfigPutResultSchema>;
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
