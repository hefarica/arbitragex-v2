/**
 * G-SIM-1 PR-B2b — OpportunityCandidate with complete route metadata.
 *
 * Shared contract between:
 * - searcher-rs (producer, persists route_metadata JSONB)
 * - api-server (enricher/proxy, constructs candidate from PG)
 * - sim-ctl (consumer → sim-core encoder)
 * - frontend (selector UI for route_source choice)
 *
 * This schema carries multi-hop route details that the minimal Opportunity
 * contract lacks: pool_addresses[], token_addresses[], dex_adapters[],
 * and decimals map.
 */

import { z } from 'zod';
import { EvmAddressSchema, ChainIdSchema, WeiAmountSchema, UsdDecimalSchema } from './_primitives';

/**
 * Decimals map: token address → decimals (0-255).
 */
export const DecimalsMapSchema = z.record(
  EvmAddressSchema,
  z.number().int().min(0).max(255),
);
export type DecimalsMap = z.infer<typeof DecimalsMapSchema>;

/**
 * Complete route topology for simulation.
 */
export const OpportunityCandidateSchema = z
  .object({
    opportunity_id: z.string().uuid(),
    chain_id: ChainIdSchema,
    token_addresses: z.array(EvmAddressSchema).min(2),
    pool_addresses: z.array(EvmAddressSchema).min(1),
    dex_adapters: z.array(z.string().min(1)).min(1),
    amount_in: z.number().positive(),
    expected_amount_out: z.number().nonnegative(),
    gross_profit: UsdDecimalSchema,
    decimals: DecimalsMapSchema,
    block_number: z.number().int().nonnegative().nullable().optional(),
    route_fingerprint: z.string().min(1),
  })
  .strict();
export type OpportunityCandidate = z.infer<typeof OpportunityCandidateSchema>;

/**
 * Route source selector for POST /api/v1/opportunities/:id/simulate.
 *
 * A1 = PG route_metadata (persistent, source of truth)
 * A2 = searcher-rs HTTP API (memory, fast)
 * A3 = sim-ctl PG lookup (autonomous, independent)
 */
export const RouteSourceSchema = z.enum(['pg_metadata', 'searcher_api', 'simctl_lookup']);
export type RouteSource = z.infer<typeof RouteSourceSchema>;

/**
 * Simulation request body with route source selector.
 */
export const SimulateRequestSchema = z
  .object({
    route_source: RouteSourceSchema,
    candidate: OpportunityCandidateSchema.nullable().optional(),
  })
  .strict();
export type SimulateRequest = z.infer<typeof SimulateRequestSchema>;

/**
 * Simulation response with wrapped_calldata for G-SIM-1.
 */
export const SimulateResponseSchema = z
  .object({
    opportunity_id: z.string().uuid(),
    passed: z.boolean(),
    gas_used: z.number().int().nonnegative(),
    gas_price_wei: WeiAmountSchema,
    net_profit_wei: z.string(),
    revert_reason: z.string().nullable(),
    wrapped_calldata: z.string(),
    simulated_at: z.string(),
    trace_id: z.string().uuid(),
  })
  .strict();
export type SimulateResponse = z.infer<typeof SimulateResponseSchema>;
