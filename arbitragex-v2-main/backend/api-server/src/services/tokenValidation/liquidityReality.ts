/**
 * LiquidityRealityValidator — Phase 1 of the Token Validation Engine.
 *
 * Queries DEX Screener's public API for every pair the token participates
 * in on EVM chains, then aggregates liquidity + volume signals into a
 * single market-reality snapshot. This is the validator that defeats the
 * 95% of scam-token patterns the operator sees in the live feed:
 *
 *   - "honeypot with wash trading": pair exists, $0.01 liquidity, $4k
 *     volume → liquidity_usd ≈ 0 → ILLIQUID.
 *   - "rugpull post-extraction": pair exists, liquidity withdrawn → ditto.
 *   - "ghost token": no pair at all → NO_DATA.
 *   - "legitimate small-cap": multiple pairs, $50k+ liquidity, real
 *     spread of volume → high score.
 *
 * Why DEX Screener
 *   - Free, no API key, ~300 req/min from a single IP (more than enough
 *     for our async worker).
 *   - Schema is stable, well-documented at docs.dexscreener.com/api.
 *   - Indexes pairs from every major DEX (Uniswap v2/v3, Sushi, Curve,
 *     Balancer, PancakeSwap, Aerodrome, Velodrome, Camelot…).
 *   - Returns liquidity + volume across windows (5m, 1h, 6h, 24h) so we
 *     can detect wash-trade signatures (volume disproportionate to
 *     liquidity).
 *
 * R8 fail-honest
 *   - HTTP error / timeout → returns `error: true`, no fabricated values.
 *   - Empty `pairs` array on the chain we asked about → `no_data: true`,
 *     `liquidity_usd: null`.
 *   - Pair exists but `liquidity.usd` is null → preserved as null, not 0.
 *
 * Not implemented (Phase 2+):
 *   - Reading reserves directly from the pair contract for cross-check
 *     (DEX Screener can lag; on-chain reserves are ground truth). Adds
 *     RPC dependency; defer until we wire the multi-RPC fallback module.
 */

/** Top-level DEX Screener API response. */
interface DexScreenerResponse {
  schemaVersion: string;
  pairs: DexScreenerPair[] | null;
}

interface DexScreenerPair {
  chainId: string;
  dexId: string;
  pairAddress: string;
  baseToken?: { address: string; name: string; symbol: string };
  quoteToken?: { address: string; name: string; symbol: string };
  priceUsd?: string;
  liquidity?: { usd?: number; base?: number; quote?: number };
  volume?: { h24?: number; h6?: number; h1?: number; m5?: number };
}

/**
 * Result of the LiquidityRealityValidator. All numeric fields are nullable
 * to honor R8 — when DEX Screener doesn't have a signal we say `null`
 * rather than fabricating zero.
 */
export interface LiquidityRealityResult {
  /** True when the validator reached DEX Screener and got a response. */
  ran: boolean;
  /** True when an HTTP / parse error prevented data collection. */
  error: boolean;
  /** Specific error message — for diagnostics; never shown to operator. */
  error_message: string | null;
  /** True when DEX Screener returned no pair for this chain+address. */
  no_data: boolean;
  /** Pairs the token participates in on this specific chain. */
  pair_count: number;
  /** Sum of liquidity.usd across all chain-local pairs. NULL if no pair returns liquidity. */
  liquidity_usd: number | null;
  /** Sum of 24h volume across chain-local pairs (USD). NULL if all pairs return null. */
  volume_24h_usd: number | null;
  volume_1h_usd: number | null;
  volume_5m_usd: number | null;
  /** USD price reported by the most-liquid pair. NULL if all pairs lack pricing. */
  price_usd: number | null;
  /** The dexId of the deepest pool (most liquidity). */
  primary_dex: string | null;
  /** Address of the deepest pool. */
  primary_pair_address: string | null;
}

const DEXSCREENER_BASE = "https://api.dexscreener.com/latest/dex/tokens";
const HTTP_TIMEOUT_MS = 4000;

/**
 * Map EVM chain_id → DEX Screener's `chainId` slug. DEX Screener uses
 * string slugs ("ethereum", "arbitrum") rather than numeric IDs; we must
 * translate and also filter the multi-chain response down to the chain
 * we asked about. Conservative whitelist — only chains the operator
 * actively operates on.
 */
const CHAIN_ID_TO_DEXSCREENER_SLUG: Record<number, string> = {
  1:       "ethereum",
  10:      "optimism",
  56:      "bsc",
  100:     "gnosischain",
  137:     "polygon",
  250:     "fantom",
  324:     "zksyncera",
  1101:    "polygonzkevm",
  5000:    "mantle",
  8453:    "base",
  42161:   "arbitrum",
  42220:   "celo",
  43114:   "avalanche",
  59144:   "linea",
  81457:   "blast",
  534352:  "scroll",
};

/**
 * Map an EVM chain_id to DEX Screener's `chainId` slug, or null when the chain
 * is not in the conservative whitelist. Exported for reuse by other DexScreener
 * readers (e.g. the token-icon route) so the slug table lives in one place.
 */
export function chainSlug(chain_id: number): string | null {
  return CHAIN_ID_TO_DEXSCREENER_SLUG[chain_id] ?? null;
}

/**
 * Fetch DEX Screener data for a single token on a single chain. Filters
 * the multi-chain response down to pairs on the requested chain, then
 * aggregates liquidity + volume + picks the primary pair.
 *
 * Defaults timeout to 4s — DEX Screener typically responds in <500ms but
 * we want to keep the worker queue moving even when their CDN is having
 * a bad day. The hot-path code never calls this directly; the async
 * worker queues addresses for background validation.
 */
export async function validateLiquidityReality(
  chain_id: number,
  address: string,
): Promise<LiquidityRealityResult> {
  const base = {
    ran: true,
    error: false,
    error_message: null,
    no_data: false,
    pair_count: 0,
    liquidity_usd: null,
    volume_24h_usd: null,
    volume_1h_usd: null,
    volume_5m_usd: null,
    price_usd: null,
    primary_dex: null,
    primary_pair_address: null,
  } as LiquidityRealityResult;

  const slug = chainSlug(chain_id);
  if (!slug) {
    return {
      ...base,
      ran: false,
      error: true,
      error_message: `unsupported chain_id=${chain_id}`,
    };
  }

  let raw: unknown;
  try {
    const ctrl = new AbortController();
    const timeoutId = setTimeout(() => ctrl.abort(), HTTP_TIMEOUT_MS);
    let res: Response;
    try {
      res = await fetch(`${DEXSCREENER_BASE}/${address}`, {
        signal: ctrl.signal,
        headers: { "accept": "application/json" },
      });
    } finally {
      clearTimeout(timeoutId);
    }
    if (!res.ok) {
      return { ...base, error: true, error_message: `HTTP ${res.status}` };
    }
    raw = await res.json();
  } catch (err) {
    return {
      ...base,
      error: true,
      error_message: err instanceof Error ? err.message : String(err),
    };
  }

  const parsed = raw as DexScreenerResponse | null;
  if (!parsed || typeof parsed !== "object") {
    return { ...base, error: true, error_message: "non-object response" };
  }
  const allPairs = Array.isArray(parsed.pairs) ? parsed.pairs : [];

  // Filter to pairs on the chain we asked about (DEX Screener returns
  // cross-chain matches when the same address exists on multiple chains).
  const chainPairs = allPairs.filter((p) => p && p.chainId === slug);
  if (chainPairs.length === 0) {
    return { ...base, no_data: true };
  }

  // Aggregate liquidity + volume. NULL preserved when ALL pairs have null;
  // when at least one pair reports a number we sum non-null contributions.
  function sumOrNull(values: (number | null | undefined)[]): number | null {
    let anyDefined = false;
    let total = 0;
    for (const v of values) {
      if (typeof v === "number" && Number.isFinite(v)) {
        total += v;
        anyDefined = true;
      }
    }
    return anyDefined ? total : null;
  }

  const liquidity_usd = sumOrNull(chainPairs.map((p) => p.liquidity?.usd));
  const volume_24h_usd = sumOrNull(chainPairs.map((p) => p.volume?.h24));
  const volume_1h_usd  = sumOrNull(chainPairs.map((p) => p.volume?.h1));
  const volume_5m_usd  = sumOrNull(chainPairs.map((p) => p.volume?.m5));

  // Primary pair: the one with the highest liquidity. Falls back to the
  // first pair when nobody reports liquidity (rare; safety).
  let primaryPair = chainPairs[0]!;
  let bestLiq = primaryPair.liquidity?.usd ?? -1;
  for (const p of chainPairs) {
    const liq = p.liquidity?.usd ?? -1;
    if (liq > bestLiq) {
      bestLiq = liq;
      primaryPair = p;
    }
  }

  const priceUsdRaw = primaryPair.priceUsd;
  let price_usd: number | null = null;
  if (typeof priceUsdRaw === "string" && priceUsdRaw.length > 0) {
    const n = Number(priceUsdRaw);
    if (Number.isFinite(n)) price_usd = n;
  } else if (typeof priceUsdRaw === "number") {
    price_usd = priceUsdRaw;
  }

  return {
    ran: true,
    error: false,
    error_message: null,
    no_data: false,
    pair_count: chainPairs.length,
    liquidity_usd,
    volume_24h_usd,
    volume_1h_usd,
    volume_5m_usd,
    price_usd,
    primary_dex: primaryPair.dexId ?? null,
    primary_pair_address: primaryPair.pairAddress ?? null,
  };
}
