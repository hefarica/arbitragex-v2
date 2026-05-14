/**
 * Ω-S5++ C9.∞ APEX · Operational entity schemas
 * DEX, Pool, Token, Wallet, RPC, Strategy, RiskGate, CapitalGate, CircuitBreaker
 */
import { z } from 'zod';
import {
  ChainIdSchema,
  EvmAddressSchema,
  GhostBalanceSchema,
  Bytes32Schema,
  IsoTimestampSchema,
  SequenceIdSchema,
  ReadinessStateSchema,
  IdempotencyKeySchema,
  OperatorIdSchema,
  BpsSchema,
  UsdDecimalSchema,
  BlockedResponseSchema,
  UnavailableResponseSchema,
  ErrorResponseSchema,
} from './_primitives';

// ─── DEX ────────────────────────────────────────────────────────────────
export const DexFamilySchema = z.enum([
  'UNISWAP_V2',
  'UNISWAP_V3',
  'CURVE_STABLESWAP',
  'BALANCER_WEIGHTED',
  'MAKER_DSS',
]);

export const DexEntitySchema = z.object({
  dex_id: z.string().uuid(),
  chain_id: ChainIdSchema,
  name: z.string().min(1),
  family: DexFamilySchema,
  factory_address: EvmAddressSchema,
  router_address: EvmAddressSchema.nullable(),
  fee_bps: BpsSchema.nullable(),
  enabled: z.boolean(),
  verified_on_chain: z.boolean(),
  readiness: ReadinessStateSchema,
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type DexEntity = z.infer<typeof DexEntitySchema>;

// ─── Pool ───────────────────────────────────────────────────────────────
export const PoolEntitySchema = z.object({
  pool_id: z.string().uuid(),
  chain_id: ChainIdSchema,
  dex_id: z.string().uuid(),
  pool_address: EvmAddressSchema,
  token0: EvmAddressSchema,
  token1: EvmAddressSchema,
  fee_bps: BpsSchema,
  reserves_token0_wei: z.string().regex(/^\d+$/),
  reserves_token1_wei: z.string().regex(/^\d+$/),
  enabled: z.boolean(),
  last_sync_at: IsoTimestampSchema.nullable(),
  readiness: ReadinessStateSchema,
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type PoolEntity = z.infer<typeof PoolEntitySchema>;

// ─── Token ──────────────────────────────────────────────────────────────
export const TokenEntitySchema = z.object({
  token_id: z.string().uuid(),
  chain_id: ChainIdSchema,
  address: EvmAddressSchema,
  symbol: z.string().min(1).max(16),
  decimals: z.number().int().min(0).max(36),
  is_stable: z.boolean(),
  price_usd: UsdDecimalSchema.nullable(),
  price_source: z.enum(['CHAINLINK', 'TWAP', 'WORKSPACE_VERIFIED', 'UNAVAILABLE']),
  enabled: z.boolean(),
  readiness: ReadinessStateSchema,
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type TokenEntity = z.infer<typeof TokenEntitySchema>;

// ─── Wallet (Ghost: balance ≡ 0) ────────────────────────────────────────
export const WalletKindSchema = z.enum([
  'EXECUTION_SIGNER',
  'CAPITAL_PROVIDER',
  'OPERATOR_ADMIN',
]);

export const WalletEntitySchema = z.object({
  wallet_id: z.string().uuid(),
  chain_id: ChainIdSchema,
  address: EvmAddressSchema,
  kind: WalletKindSchema,
  // INV-7: si kind=EXECUTION_SIGNER ⇒ balance_wei ≡ "0"
  balance_wei: z.union([GhostBalanceSchema, z.string().regex(/^\d+$/)]),
  enabled: z.boolean(),
  rotated_at: IsoTimestampSchema.nullable(),
  readiness: ReadinessStateSchema,
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict().refine(
  (w) => w.kind !== 'EXECUTION_SIGNER' || w.balance_wei === '0',
  { message: 'INV-7 Ghost: EXECUTION_SIGNER debe tener balance_wei = "0"' },
);
export type WalletEntity = z.infer<typeof WalletEntitySchema>;

// ─── RPC ────────────────────────────────────────────────────────────────
export const RpcKindSchema = z.enum(['HTTP', 'WS']);

export const RpcEntitySchema = z.object({
  rpc_id: z.string().uuid(),
  chain_id: ChainIdSchema,
  kind: RpcKindSchema,
  url: z.string().url(),
  provider: z.string().min(1),
  enabled: z.boolean(),
  latency_p50_ms: z.number().min(0).nullable(),
  latency_p95_ms: z.number().min(0).nullable(),
  last_health_check_at: IsoTimestampSchema.nullable(),
  readiness: ReadinessStateSchema,
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type RpcEntity = z.infer<typeof RpcEntitySchema>;

// ─── Strategy ───────────────────────────────────────────────────────────
export const StrategyFamilySchema = z.enum([
  'TOPOLOGICAL_DUAL',
  'TOPOLOGICAL_TRIPLE',
  'CROSS_DOMAIN_HOLONOMIC',
  'LIQUIDATION_STEWARD',
]);

export const StrategyEntitySchema = z.object({
  strategy_id: z.string().uuid(),
  name: z.string().min(1),
  family: StrategyFamilySchema,
  chain_id: ChainIdSchema.nullable(), // null = multichain
  enabled: z.boolean(),
  paper_only: z.boolean(),
  readiness: ReadinessStateSchema,
  blockers: z.array(z.string()),
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type StrategyEntity = z.infer<typeof StrategyEntitySchema>;

// ─── Risk Gate ──────────────────────────────────────────────────────────
export const RiskGateSchema = z.object({
  gate_id: z.string().uuid(),
  name: z.string().min(1),
  chain_id: ChainIdSchema.nullable(),
  threshold_value: z.string(), // decimal-string
  threshold_unit: z.enum(['bps', 'percent', 'usd', 'count', 'ms']),
  triggered: z.boolean(),
  enabled: z.boolean(),
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type RiskGate = z.infer<typeof RiskGateSchema>;

// ─── Capital Gate ───────────────────────────────────────────────────────
export const CapitalGateSchema = z.object({
  gate_id: z.string().uuid(),
  chain_id: ChainIdSchema,
  cap_usd: UsdDecimalSchema, // empieza en "0.00" — INV-A
  current_exposure_usd: UsdDecimalSchema,
  enabled: z.boolean(),
  // mainnet escalation requires sovereign signature
  sovereign_signature: z.string().nullable(),
  signature_payload_hash: Bytes32Schema.nullable(),
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict().refine(
  (cg) => cg.cap_usd === '0.00' || cg.cap_usd === '0' || cg.sovereign_signature !== null,
  { message: 'INV-A: cap_usd > 0 requiere sovereign_signature payload-bound' },
);
export type CapitalGate = z.infer<typeof CapitalGateSchema>;

// ─── Circuit Breaker ────────────────────────────────────────────────────
export const CircuitBreakerSchema = z.object({
  breaker_id: z.string().uuid(),
  name: z.string().min(1),
  chain_id: ChainIdSchema.nullable(),
  state: z.enum(['CLOSED', 'HALF_OPEN', 'OPEN']),
  failure_count: z.number().int().min(0),
  failure_threshold: z.number().int().positive(),
  cooldown_ms: z.number().int().min(0),
  opened_at: IsoTimestampSchema.nullable(),
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type CircuitBreaker = z.infer<typeof CircuitBreakerSchema>;

// ─── Universal mutation response (discriminated) ────────────────────────
export const MutationOkBaseSchema = z.object({
  status: z.literal('OK'),
  config_hash_before: Bytes32Schema,
  config_hash_after: Bytes32Schema,
  audit_event_id: z.string().uuid(),
  reload_channel: z.string(),
  ack_pending: z.boolean(),
  evaluated_at: IsoTimestampSchema,
}).strict();

export const MutationResponseSchema = z.discriminatedUnion('status', [
  MutationOkBaseSchema,
  BlockedResponseSchema,
  UnavailableResponseSchema,
  ErrorResponseSchema,
]);
export type MutationResponse = z.infer<typeof MutationResponseSchema>;
