/**
 * known-tokens.ts — Tier-1 (instant, no-network) resolution + address utilities
 * for the TokenIcon cascade.
 *
 * The cascade (see `useTokenIcon`):
 *   1. Known tokens  → this file (instant, no request).
 *   2. Redis cache   → GET /api/v1/token-icon/:chain/:addr (api-server, Redis hit).
 *   3. DexScreener   → same endpoint (api-server fallback).
 *   4. Jazzicon      → DeterministicAvatar (deterministic SVG, always available).
 *
 * No-hardcode doctrine: the addresses + logo URLs below are NOT invented — they
 * are copied verbatim from the repo's committed curated source
 * `backend/api-server/src/services/uniswap-tokenlist.json` (the same TrustWallet
 * assets CDN the backend `tokenRegistry` serves). This is reference data sourced
 * from the repo (analogous to `backend/shared-rs/src/tokens.rs`), not operator
 * configuration. If a logo URL ever 404s, `TokenIcon`'s `<img onError>` degrades
 * to the deterministic avatar — a stale entry never produces a broken image.
 */

export type IconSource = "known" | "redis" | "dexscreener" | "jazzicon";

export interface IconResolution {
  /** Resolved image URL, or null when the caller should render a jazzicon. */
  iconUrl: string | null;
  source: IconSource;
  /** true ⇔ iconUrl == null ⇔ caller renders the deterministic avatar. */
  isFallback: boolean;
}

/** EVM address shape after normalization: 0x + 40 lowercase hex. */
const EVM_ADDRESS_RE = /^0x[0-9a-f]{40}$/;

/**
 * Trim + lowercase + validate an EVM address. Returns the canonical lowercased
 * address, or `null` when the input is not a well-formed 0x+40-hex address.
 */
export function normalizeAddress(
  address: string | null | undefined,
): string | null {
  if (typeof address !== "string") return null;
  const a = address.trim().toLowerCase();
  return EVM_ADDRESS_RE.test(a) ? a : null;
}

/** True when `address` is a well-formed EVM address (after trim/lowercase). */
export function isValidEvmAddress(
  address: string | null | undefined,
): boolean {
  return normalizeAddress(address) != null;
}

interface KnownToken {
  symbol: string;
  iconUrl: string;
}

/**
 * chainId → lowercased-address → known token. Mainnet (chain 1) canonical set.
 * Source: backend/api-server/src/services/uniswap-tokenlist.json (committed).
 * Extend conservatively — only addresses + URLs verifiable against that file.
 */
const KNOWN_TOKENS: Record<number, Record<string, KnownToken>> = {
  1: {
    // WETH
    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
      symbol: "WETH",
      iconUrl:
        "https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/assets/0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2/logo.png",
    },
    // USDC
    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": {
      symbol: "USDC",
      iconUrl:
        "https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/assets/0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48/logo.png",
    },
    // USDT
    "0xdac17f958d2ee523a2206206994597c13d831ec7": {
      symbol: "USDT",
      iconUrl:
        "https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/assets/0xdAC17F958D2ee523a2206206994597C13D831ec7/logo.png",
    },
    // DAI
    "0x6b175474e89094c44da98b954eedeac495271d0f": {
      symbol: "DAI",
      iconUrl:
        "https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/assets/0x6B175474E89094C44Da98b954EedeAC495271d0F/logo.png",
    },
    // WBTC
    "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599": {
      symbol: "WBTC",
      iconUrl:
        "https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/assets/0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599/logo.png",
    },
  },
};

/**
 * Tier-1 lookup. Returns a resolved icon for a known token, or `null` when the
 * (chainId, address) pair is not in the curated known set. Pure — no network.
 */
export function resolveKnownIcon(
  chainId: number,
  address: string | null | undefined,
): IconResolution | null {
  const a = normalizeAddress(address);
  if (a == null) return null;
  const known = KNOWN_TOKENS[chainId]?.[a];
  if (!known) return null;
  return { iconUrl: known.iconUrl, source: "known", isFallback: false };
}

/** Known symbol for a token, or null. Used for instant labels. */
export function knownSymbol(
  chainId: number,
  address: string | null | undefined,
): string | null {
  const a = normalizeAddress(address);
  if (a == null) return null;
  return KNOWN_TOKENS[chainId]?.[a]?.symbol ?? null;
}

// ── In-memory resolution cache (per chainId+address) ────────────────────────
// Survives navigation between dashboard pages within a single tab session,
// so a token resolved once never re-hits the network. Module-scoped Map.

const iconCache = new Map<string, IconResolution>();

export function iconCacheKey(chainId: number, address: string): string {
  return `${chainId}:${address}`;
}

export function getCachedIcon(
  chainId: number,
  address: string,
): IconResolution | undefined {
  return iconCache.get(iconCacheKey(chainId, address));
}

export function setCachedIcon(
  chainId: number,
  address: string,
  res: IconResolution,
): void {
  iconCache.set(iconCacheKey(chainId, address), res);
}

/** Test-only — clear the in-memory icon cache. */
export function _resetIconCacheForTests(): void {
  iconCache.clear();
}
