/**
 * Curated token-list snapshots — SINGLE SOURCE OF TRUTH for both consumers:
 *
 *   - api-server `tokenRegistry` (dashboard verification badges)
 *   - selector-api `token_safety` internal heuristic (pre-scoring safety gate)
 *
 * Two committed snapshots, loaded once via fs (no `resolveJsonModule` — a 2 MB
 * JSON import would blow up tsc's literal-type inference):
 *
 *   1. Uniswap Labs Default Token List (~1,451 tokens across major chains,
 *      governance-curated, includes logoURI + decimals). High-trust source.
 *   2. CoinGecko `/coins/list?include_platform=true` filtered to EVM chains
 *      (~9,465 coins across 18 chains). Broad coverage; for an Ethereum
 *      mainnet token to be missing from BOTH lists is a strong scam signal.
 *
 * Doctrine exception (same class as `canonical_token_decimals` in shared-rs):
 * these are IMMUTABLE CONTRACT IDENTITY snapshots — verified addresses — NOT
 * market data (price/liquidity/tvl) and NOT operator config. RULE 00 intact:
 * no data is fabricated, a verdict is just produced earlier.
 *
 * Why the tier function exists (2026-08-18, vivid-grove audit): the selector-api
 * safety gate previously trusted a hand-picked 26-token subset of the Uniswap
 * list. With `provider = "internal_only"` every OTHER token scored a neutral 50
 * against a floor of 70 — mathematically unpassable, 100% of detections rejected
 * `safety_below_threshold` BEFORE any economics ran, and the whole dashboard
 * rendered hollow cards. The full curated lists (the same SSOT the api-server
 * registry already used) restore the gate's meaning: real tokens pass, tokens
 * absent from both stay blocked.
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const UNISWAP_PATH = path.join(__dirname, "data", "uniswap-tokenlist.json");
const COINGECKO_PATH = path.join(__dirname, "data", "coingecko-coinslist.json");

// ── Snapshot shapes ─────────────────────────────────────────────────────────

export interface RawTokenEntry {
  chainId: number;
  address: string;
  symbol: string;
  name: string;
  decimals: number;
  logoURI?: string;
}

export interface UniswapTokenlist {
  name: string;
  version: { major: number; minor: number; patch: number };
  tokens: RawTokenEntry[];
}

export interface CoinGeckoCoin {
  id: string;
  symbol: string;
  name: string;
  /** coingecko platform key (chain name) → lowercase address. */
  platforms: Record<string, string>;
}

export interface CoinGeckoSnapshot {
  source: string;
  fetched_at_utc: string;
  /** platform key → EVM chain_id. */
  platform_mapping: Record<string, number>;
  coins: CoinGeckoCoin[];
}

// ── Parsed-snapshot getters (cached, fail-fast on corrupt data) ─────────────

let uniswapCache: UniswapTokenlist | null = null;
let coingeckoCache: CoinGeckoSnapshot | null = null;

export function getUniswapTokenlist(): UniswapTokenlist {
  if (!uniswapCache) {
    const parsed = JSON.parse(readFileSync(UNISWAP_PATH, "utf8")) as UniswapTokenlist;
    if (!parsed || !Array.isArray(parsed.tokens)) {
      throw new Error("tokenlists: invalid Uniswap tokenlist shape");
    }
    uniswapCache = parsed;
  }
  return uniswapCache;
}

export function getCoinGeckoSnapshot(): CoinGeckoSnapshot {
  if (!coingeckoCache) {
    const parsed = JSON.parse(readFileSync(COINGECKO_PATH, "utf8")) as CoinGeckoSnapshot;
    if (!parsed || !Array.isArray(parsed.coins) || !parsed.platform_mapping) {
      throw new Error("tokenlists: invalid CoinGecko snapshot shape");
    }
    coingeckoCache = parsed;
  }
  return coingeckoCache;
}

// ── Curated-verification index ──────────────────────────────────────────────

export type VerificationTier =
  /** In the Uniswap default list — tight editorial standards. */
  | "uniswap"
  /** Not in Uniswap but indexed by CoinGecko — real, weaker provenance. */
  | "coingecko_only"
  /** In neither curated list. For mainnet this is a strong scam signal. */
  | "unverified";

/** chain_id → Set(lowercased address) for the Uniswap snapshot. */
const uniswapByChain = new Map<number, Set<string>>();
/** `${chain_id}:${address}` for CoinGecko-indexed addresses. */
const coingeckoKeys = new Set<string>();
let indexBuilt = false;

function buildIndex(): void {
  if (indexBuilt) return;
  for (const t of getUniswapTokenlist().tokens) {
    let set = uniswapByChain.get(t.chainId);
    if (!set) {
      set = new Set<string>();
      uniswapByChain.set(t.chainId, set);
    }
    set.add(t.address.toLowerCase());
  }
  // NOTE (verified against the snapshot): `coins[].platforms` keys are NUMERIC
  // chain-id strings ("1", "137", "8453") — the same convention api-server's
  // registry has always parsed. `platform_mapping` (name → id) is metadata for
  // humans, NOT the key convention of `platforms`.
  for (const coin of getCoinGeckoSnapshot().coins) {
    for (const [chainIdStr, addr] of Object.entries(coin.platforms ?? {})) {
      const chainId = Number(chainIdStr);
      if (!Number.isFinite(chainId) || chainId <= 0) continue;
      if (typeof addr !== "string" || !addr.startsWith("0x") || addr.length !== 42) continue;
      coingeckoKeys.add(`${chainId}:${addr.toLowerCase()}`);
    }
  }
  indexBuilt = true;
}

/**
 * Which curated tier covers a (chain_id, address)? Deterministic, no I/O.
 * `address` may be checksummed; it is normalized here.
 */
export function curatedVerificationTier(chainId: number, address: string): VerificationTier {
  let a: string;
  try {
    a = address.toLowerCase();
    if (!/^0x[0-9a-f]{40}$/.test(a)) return "unverified";
  } catch {
    return "unverified";
  }
  buildIndex();
  if (uniswapByChain.get(chainId)?.has(a)) return "uniswap";
  if (coingeckoKeys.has(`${chainId}:${a}`)) return "coingecko_only";
  return "unverified";
}

/** Total sizes (observability / boot log). */
export function curatedIndexSizes(): { uniswapTokens: number; coingeckoCoins: number; chains: number } {
  buildIndex();
  return {
    uniswapTokens: getUniswapTokenlist().tokens.length,
    coingeckoCoins: getCoinGeckoSnapshot().coins.length,
    chains: uniswapByChain.size,
  };
}
