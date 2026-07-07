/**
 * VacuumDecoherenceCost — modelo termodinámico de fricción de red por chain.
 *
 * Cierra B-VDC (S4 — Paper-Shadow Simulator).
 *
 * Modelo físico:
 *   VDC(chain) = gas_cost_usd                   // L1 term: fricción atómica
 *              + base_fee_premium_usd            // Phase transition premium
 *              + slippage_usd                    // Variance manifold cost
 *              + relay_fee_usd                   // Mempool extraction cost
 *              + bridge_fee_usd                  // Cross-chain term (solo CC)
 *              + priority_tip_usd                // JIT Dirac impulse cost
 *
 * Esperanza positiva ⟺ Topological Yield bruto > VDC(chain).
 *
 * Cada chain tiene su propia firma termodinámica (ChainProfile):
 *  - Ethereum    : EIP-1559 con base_fee dinámico, relay privado obligatorio.
 *  - BSC         : modelo de gas plano (gas_price constante), sin relay.
 *  - Polygon     : EIP-1559 atenuado, priority_fee_min vigente.
 *  - Arbitrum    : L1 data + L2 exec con compresión brotli.
 *  - Optimism    : L1 publishing + L2 exec, modelo Bedrock.
 *  - Base        : Bedrock derivado, fee_scalar de Optimism.
 *  - Avalanche   : EIP-1559 nativo C-chain.
 */

export type ChainKey =
  | 'ethereum'
  | 'bsc'
  | 'polygon'
  | 'arbitrum'
  | 'optimism'
  | 'base'
  | 'avalanche';

export interface ChainProfile {
  chain_id: number;
  name: string;
  fee_model: 'eip1559' | 'flat' | 'bedrock';
  /** Multiplicador para convertir el base_fee a USD (oráculo nativo). */
  native_token_usd_oracle_id: string;
  /** Gas mínimo en gwei (priority tip). */
  priority_fee_min_gwei: number;
  /** Multiplicador sobre el base_fee para urgencia institucional. */
  base_fee_urgency_multiplier: number;
  /** Si requiere relay privado (true → relay_fee_usd > 0). */
  requires_private_relay: boolean;
  /** Relay fee como fracción del gross profit. */
  relay_fee_fraction: number;
  /** Slippage tolerable en bps (1 bps = 0.01%). */
  slippage_bps_default: number;
  /** Componente fijo de L1 publishing (sólo bedrock/L2). */
  l1_publishing_fixed_usd: number;
  /** Para CC: fee de bridge canónico. */
  bridge_fee_usd: number;
}

/**
 * Catálogo termodinámico. Valores DEBEN actualizarse vía /api/admin/registries/chain
 * cuando el operador soberano sube el cap_usd_ceiling.
 *
 * Estos defaults reflejan baseline conservador 2025-Q1.
 */
export const CHAIN_PROFILES: Record<ChainKey, ChainProfile> = {
  ethereum: {
    chain_id: 1,
    name: 'Ethereum',
    fee_model: 'eip1559',
    native_token_usd_oracle_id: 'eth-usd',
    priority_fee_min_gwei: 2.0,
    base_fee_urgency_multiplier: 1.25,
    requires_private_relay: true,
    relay_fee_fraction: 0.05,
    slippage_bps_default: 10,
    l1_publishing_fixed_usd: 0.0,
    bridge_fee_usd: 0.0,
  },
  bsc: {
    chain_id: 56,
    name: 'BSC',
    fee_model: 'flat',
    native_token_usd_oracle_id: 'bnb-usd',
    priority_fee_min_gwei: 3.0,
    base_fee_urgency_multiplier: 1.10,
    requires_private_relay: false,
    relay_fee_fraction: 0.0,
    slippage_bps_default: 25,
    l1_publishing_fixed_usd: 0.0,
    bridge_fee_usd: 0.0,
  },
  polygon: {
    chain_id: 137,
    name: 'Polygon',
    fee_model: 'eip1559',
    native_token_usd_oracle_id: 'matic-usd',
    priority_fee_min_gwei: 30.0,
    base_fee_urgency_multiplier: 1.20,
    requires_private_relay: false,
    relay_fee_fraction: 0.0,
    slippage_bps_default: 20,
    l1_publishing_fixed_usd: 0.0,
    bridge_fee_usd: 0.0,
  },
  arbitrum: {
    chain_id: 42161,
    name: 'Arbitrum',
    fee_model: 'bedrock',
    native_token_usd_oracle_id: 'eth-usd',
    priority_fee_min_gwei: 0.1,
    base_fee_urgency_multiplier: 1.05,
    requires_private_relay: false,
    relay_fee_fraction: 0.0,
    slippage_bps_default: 15,
    l1_publishing_fixed_usd: 0.15,
    bridge_fee_usd: 0.0,
  },
  optimism: {
    chain_id: 10,
    name: 'Optimism',
    fee_model: 'bedrock',
    native_token_usd_oracle_id: 'eth-usd',
    priority_fee_min_gwei: 0.1,
    base_fee_urgency_multiplier: 1.05,
    requires_private_relay: false,
    relay_fee_fraction: 0.0,
    slippage_bps_default: 15,
    l1_publishing_fixed_usd: 0.12,
    bridge_fee_usd: 0.0,
  },
  base: {
    chain_id: 8453,
    name: 'Base',
    fee_model: 'bedrock',
    native_token_usd_oracle_id: 'eth-usd',
    priority_fee_min_gwei: 0.1,
    base_fee_urgency_multiplier: 1.05,
    requires_private_relay: false,
    relay_fee_fraction: 0.0,
    slippage_bps_default: 15,
    l1_publishing_fixed_usd: 0.10,
    bridge_fee_usd: 0.0,
  },
  avalanche: {
    chain_id: 43114,
    name: 'Avalanche',
    fee_model: 'eip1559',
    native_token_usd_oracle_id: 'avax-usd',
    priority_fee_min_gwei: 25.0,
    base_fee_urgency_multiplier: 1.15,
    requires_private_relay: false,
    relay_fee_fraction: 0.0,
    slippage_bps_default: 20,
    l1_publishing_fixed_usd: 0.0,
    bridge_fee_usd: 0.0,
  },
};

export interface VdcInputs {
  chain_id: number;
  gross_profit_usd: number;
  gas_units_used: number;
  base_fee_gwei: number;
  native_usd_price: number;
  trade_notional_usd: number;
  observed_slippage_bps?: number;
  cross_chain?: boolean;
}

export interface VdcResult {
  chain: ChainKey | 'unknown';
  chain_id: number;
  gas_cost_usd: number;
  base_fee_premium_usd: number;
  slippage_usd: number;
  relay_fee_usd: number;
  bridge_fee_usd: number;
  priority_tip_usd: number;
  total_vdc_usd: number;
  net_topological_yield_usd: number;
  passes_thermo_floor: boolean;
}

function lookupChain(chainId: number): { key: ChainKey | 'unknown'; profile: ChainProfile | null } {
  const entry = Object.entries(CHAIN_PROFILES).find(([, p]) => p.chain_id === chainId);
  if (!entry) return { key: 'unknown', profile: null };
  return { key: entry[0] as ChainKey, profile: entry[1] };
}

/**
 * Cálculo principal. Devuelve siempre un VdcResult; nunca lanza —
 * si la chain es desconocida, marca `passes_thermo_floor = false`.
 */
export function computeVacuumDecoherenceCost(input: VdcInputs): VdcResult {
  const { key, profile } = lookupChain(input.chain_id);
  if (!profile) {
    return {
      chain: 'unknown',
      chain_id: input.chain_id,
      gas_cost_usd: 0,
      base_fee_premium_usd: 0,
      slippage_usd: 0,
      relay_fee_usd: 0,
      bridge_fee_usd: 0,
      priority_tip_usd: 0,
      total_vdc_usd: Infinity,
      net_topological_yield_usd: -Infinity,
      passes_thermo_floor: false,
    };
  }

  const gweiToEth = 1e-9;
  const basefee_eth = input.base_fee_gwei * gweiToEth;
  const gas_cost_usd =
    input.gas_units_used * basefee_eth * input.native_usd_price;

  const base_fee_premium_usd =
    gas_cost_usd * (profile.base_fee_urgency_multiplier - 1.0);

  const slippage_bps = input.observed_slippage_bps ?? profile.slippage_bps_default;
  const slippage_usd = (slippage_bps / 10_000) * input.trade_notional_usd;

  const relay_fee_usd = profile.requires_private_relay
    ? profile.relay_fee_fraction * Math.max(input.gross_profit_usd, 0)
    : 0;

  const priority_tip_eth = profile.priority_fee_min_gwei * gweiToEth * input.gas_units_used;
  const priority_tip_usd = priority_tip_eth * input.native_usd_price;

  const bridge_fee_usd = input.cross_chain ? profile.bridge_fee_usd : 0;

  const total_vdc_usd =
    gas_cost_usd +
    base_fee_premium_usd +
    slippage_usd +
    relay_fee_usd +
    priority_tip_usd +
    bridge_fee_usd +
    profile.l1_publishing_fixed_usd;

  const net = input.gross_profit_usd - total_vdc_usd;
  return {
    chain: key,
    chain_id: profile.chain_id,
    gas_cost_usd,
    base_fee_premium_usd,
    slippage_usd,
    relay_fee_usd,
    bridge_fee_usd,
    priority_tip_usd,
    total_vdc_usd,
    net_topological_yield_usd: net,
    passes_thermo_floor: net > 0,
  };
}

/** Helper para integración con `computeSimulatedNet`. */
export function vdcForChain(chainId: number): ChainProfile | null {
  return lookupChain(chainId).profile;
}
