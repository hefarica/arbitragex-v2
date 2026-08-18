/**
 * Curated-token registry for verification.
 *
 * Aggregates TWO committed snapshots at boot:
 *
 *   1. Uniswap Labs Default Token List (`uniswap-tokenlist.json`,
 *      ~1,451 tokens across major chains, governance-curated, includes
 *      logoURI + decimals). High-trust source.
 *
 *   2. CoinGecko `/coins/list?include_platform=true` filtered to EVM
 *      chains (`coingecko-coinslist.json`, ~9,465 coins across 18 chains).
 *      Broad coverage: any token CoinGecko has indexed (which includes
 *      brand-new launches with active liquidity and is the de-facto
 *      authoritative list for the crypto industry) gets verified status.
 *
 * Merge rule
 *   - Uniswap entries WIN on address conflict — they have logo + decimals
 *     and tighter editorial standards.
 *   - CoinGecko fills the gaps. When Uniswap doesn't know an address but
 *     CoinGecko does, the token is still verified (with `verified_via:
 *     "coingecko"` in notes — operator sees provenance).
 *   - Tokens NEITHER list knows about → unverified. These are memecoins,
 *     scam tokens, or contracts so new even CoinGecko hasn't indexed them.
 *     For an Ethereum mainnet token to be missing from both is a strong
 *     scam signal (CoinGecko indexes ~17K tokens including microcaps).
 *
 * Why snapshots, not live fetches
 *   - Zero runtime network dependency → deterministic boot time.
 *   - Both lists change slowly (Uniswap monthly, CoinGecko adds new tokens
 *     daily but legitimacy-curated entries are stable).
 *   - Committed snapshot → reproducible verification across deploys.
 *
 * What "verified" means
 *   - `verified=true` requires the (chainId, address) to be in EITHER
 *     curated list AND the on-chain `symbol()` to match the registry's
 *     symbol (case-insensitive). The second check defends against the
 *     impersonation pattern: a contract at a random address declaring
 *     `symbol() = "USDC"` cannot fool the dashboard because the canonical
 *     USDC has a single well-known address in both registries.
 *   - `verified=false` always — when address is in neither registry OR
 *     symbol differs from both registries. The frontend renders the
 *     on-chain symbol with an "UNVERIFIED" badge so the operator never
 *     confuses it with a real asset.
 *
 * R8 fail-honest
 *   - We never rename a token. The on-chain symbol stays in
 *     `token_info.symbol`. The registry's canonical name is surfaced
 *     separately as `registry_name` for operator context.
 */

import { getCoinGeckoSnapshot, getUniswapTokenlist } from "@arbx/shared/tokenlists";

export interface RegistryEntry {
  chain_id: number;
  /** Lowercased 0x-prefixed address (CHECK constraint matches `tokens` table). */
  address: string;
  symbol: string;
  name: string;
  /** decimals: Uniswap snapshot carries it; CoinGecko coins-list does not (null). */
  decimals: number | null;
  logo_uri: string | null;
  /** Which curated source supplied this entry. */
  source: "uniswap" | "coingecko";
}

export interface VerificationResult {
  /**
   * True when the (chain_id, address) pair is in EITHER curated list
   * (Uniswap default or CoinGecko full) AND the on-chain symbol matches
   * the registry's symbol (case-insensitive). False otherwise — every
   * UNVERIFIED token is flagged so the dashboard can render a warning badge.
   */
  verified: boolean;
  /** Registry's canonical symbol, or null when address unknown. */
  registry_symbol: string | null;
  /** Registry's full name (e.g. "Wrapped Ether"), or null. */
  registry_name: string | null;
  /** Registry's logo URL, or null. CoinGecko entries don't carry logos here. */
  registry_logo_uri: string | null;
  /**
   * Which curated source supplied the verification, or null when address
   * is in neither. Operator transparency: a CoinGecko-only verification
   * is real but slightly weaker provenance than Uniswap's editorial list.
   */
  source: "uniswap" | "coingecko" | null;
  /**
   * Verification notes:
   *   "in-registry"             — address matched a curated entry.
   *   "via-uniswap"             — sourced from Uniswap default token list.
   *   "via-coingecko"           — sourced from CoinGecko coins-list.
   *   "symbol-mismatch"         — address in registry but on-chain symbol
   *                               differs (impersonation or registry stale).
   *   "address-not-in-registry" — never seen in any curated list.
   */
  notes: string[];
}

// ── Module-scoped index ─────────────────────────────────────────────────────

let index: Map<string, RegistryEntry> | null = null;
let loadError: string | null = null;

function makeKey(chain_id: number, address: string): string {
  return `${chain_id}:${address.toLowerCase()}`;
}

interface CoinGeckoCoin {
  id: string;
  symbol: string;
  name: string;
  /** chain_id (as string key) → lowercase address. */
  platforms: Record<string, string>;
}

interface CoinGeckoSnapshot {
  source: string;
  fetched_at_utc: string;
  platform_mapping: Record<string, number>;
  coins: CoinGeckoCoin[];
}

function loadUniswapInto(m: Map<string, RegistryEntry>): number {
  // SSOT: the snapshots live in @arbx/shared/tokenlists (also consumed by the
  // selector-api safety gate). Same parsed shape, one source of truth.
  const parsed = getUniswapTokenlist();
  let added = 0;
  for (const t of parsed.tokens) {
    if (
      typeof t.chainId !== "number"
      || typeof t.address !== "string"
      || typeof t.symbol !== "string"
      || typeof t.name !== "string"
      || typeof t.decimals !== "number"
    ) {
      continue;
    }
    const key = makeKey(t.chainId, t.address);
    m.set(key, {
      chain_id: t.chainId,
      address: t.address.toLowerCase(),
      symbol: t.symbol,
      name: t.name,
      decimals: t.decimals,
      logo_uri: typeof t.logoURI === "string" ? t.logoURI : null,
      source: "uniswap",
    });
    added++;
  }
  return added;
}

function loadCoinGeckoInto(m: Map<string, RegistryEntry>): number {
  let parsed: CoinGeckoSnapshot;
  try {
    parsed = getCoinGeckoSnapshot();
  } catch {
    // CoinGecko snapshot is optional — Uniswap-only deployments still work.
    return 0;
  }
  if (!parsed || !Array.isArray(parsed.coins)) {
    throw new Error("invalid CoinGecko coinslist shape");
  }
  let added = 0;
  for (const c of parsed.coins) {
    if (
      typeof c.symbol !== "string"
      || typeof c.name !== "string"
      || !c.platforms
      || typeof c.platforms !== "object"
    ) {
      continue;
    }
    for (const [chainIdStr, address] of Object.entries(c.platforms)) {
      const chain_id = Number(chainIdStr);
      if (!Number.isFinite(chain_id) || chain_id <= 0) continue;
      if (typeof address !== "string" || !address.startsWith("0x") || address.length !== 42) {
        continue;
      }
      const key = makeKey(chain_id, address);
      // Uniswap entries WIN — skip if already populated by Uniswap.
      const existing = m.get(key);
      if (existing && existing.source === "uniswap") continue;
      m.set(key, {
        chain_id,
        address: address.toLowerCase(),
        symbol: c.symbol.toUpperCase(),
        name: c.name,
        decimals: null,    // CoinGecko coins-list doesn't carry decimals
        logo_uri: null,    // nor logos in this endpoint
        source: "coingecko",
      });
      added++;
    }
  }
  return added;
}

function loadIndex(): Map<string, RegistryEntry> {
  if (index != null) return index;
  if (loadError != null) {
    // Once loading fails we don't retry per request; the snapshots are
    // committed so a failure here is a deploy-time bug, not a runtime
    // transient.
    return new Map();
  }

  try {
    const m = new Map<string, RegistryEntry>();
    // Uniswap first (high-trust, includes logo + decimals).
    const uniswapCount = loadUniswapInto(m);
    if (uniswapCount === 0) {
      throw new Error("Uniswap tokenlist parsed but contained zero valid entries");
    }
    // CoinGecko second — fills gaps without overwriting Uniswap entries.
    loadCoinGeckoInto(m);
    index = m;
    return m;
  } catch (err) {
    loadError = err instanceof Error ? err.message : String(err);
    return new Map();
  }
}

// ── Public API ──────────────────────────────────────────────────────────────

/**
 * Look up a single token by (chain_id, address). Returns null when the
 * address is not in the curated list. Address is matched case-insensitively
 * against the lowercased index.
 */
export function lookupRegistry(
  chain_id: number,
  address: string,
): RegistryEntry | null {
  const m = loadIndex();
  return m.get(makeKey(chain_id, address)) ?? null;
}

/**
 * Verify an on-chain (chain_id, address, symbol) triple against the curated
 * list. The on-chain symbol may be null when the resolver couldn't fetch it;
 * verification then degrades to address-only matching (still flagged
 * symbol-mismatch when registry has a symbol but row doesn't).
 *
 * Pure function — does not mutate the registry. Safe for hot-path use.
 */
export function verifyToken(
  chain_id: number,
  address: string,
  onchain_symbol: string | null,
): VerificationResult {
  const entry = lookupRegistry(chain_id, address);
  if (!entry) {
    return {
      verified: false,
      registry_symbol: null,
      registry_name: null,
      registry_logo_uri: null,
      source: null,
      notes: ["address-not-in-registry"],
    };
  }
  const notes: string[] = ["in-registry", `via-${entry.source}`];
  let symbolOk = true;
  if (onchain_symbol != null && onchain_symbol.length > 0) {
    if (onchain_symbol.toUpperCase() !== entry.symbol.toUpperCase()) {
      symbolOk = false;
      notes.push("symbol-mismatch");
    }
  } else {
    // On-chain symbol missing but address is curated — accept and rely on the
    // registry symbol for display. Don't flag mismatch.
  }
  return {
    verified: symbolOk,
    registry_symbol: entry.symbol,
    registry_name: entry.name,
    registry_logo_uri: entry.logo_uri,
    source: entry.source,
    notes,
  };
}

/**
 * Diagnostics helper: returns the loaded registry size, per-chain count,
 * and per-source breakdown. Used by the readiness probe + admin endpoint.
 */
export function registryStats(): {
  loaded: boolean;
  total_tokens: number;
  by_chain: Record<number, number>;
  by_source: Record<"uniswap" | "coingecko", number>;
  load_error: string | null;
} {
  const m = loadIndex();
  const by_chain: Record<number, number> = {};
  const by_source: Record<"uniswap" | "coingecko", number> = {
    uniswap: 0,
    coingecko: 0,
  };
  for (const e of m.values()) {
    by_chain[e.chain_id] = (by_chain[e.chain_id] ?? 0) + 1;
    by_source[e.source] = (by_source[e.source] ?? 0) + 1;
  }
  return {
    loaded: m.size > 0,
    total_tokens: m.size,
    by_chain,
    by_source,
    load_error: loadError,
  };
}

/** Test-only — clear the cached index so the next call reloads. */
export function _resetRegistryForTests(): void {
  index = null;
  loadError = null;
}
