// Idea 1 (Phase-1) — cartridge route pre-filter preference shape.
// Mirrors the api-server CartridgeFilterSchema (backend/api-server/src/routes/cartridge-filters.ts).
// HONESTY: this is a STORED operator preference. Nothing in searcher-rs consumes it yet —
// the route pre-filter consumer is a Phase-2 (hot-path) follow-up.

export interface CartridgeFilterState {
  chain_id: number;
  /** Strategy allowlist. [] = all strategies (no filter). */
  enabled_strategies: string[];
  /** Minimum net profit (USD) for a route to pass. 0 = no filter. */
  min_profit_usd: number;
  /** Maximum slippage (bps). 10000 = no filter. */
  max_slippage_bps: number;
  /** Optional custom cartridge as base64 (.rhai/.wasm). null = none. */
  custom_cartridge_b64: string | null;
  enabled: boolean;
  updated_at?: string;
  updated_by?: string;
}

export type CartridgeFilterInput = Omit<
  CartridgeFilterState,
  "chain_id" | "updated_at" | "updated_by"
>;

export const DEFAULT_CARTRIDGE_FILTER: CartridgeFilterInput = {
  enabled_strategies: [],
  min_profit_usd: 0,
  max_slippage_bps: 10_000,
  custom_cartridge_b64: null,
  enabled: true,
};
