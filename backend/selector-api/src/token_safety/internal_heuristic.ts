/**
 * Internal heuristic — when no external provider is available or all providers
 * are circuit-broken. This is intentionally conservative: it scores based on
 * structural signals of the address (not call-site deep checks). It marks
 * `source = 'internal'` so consumers know the grade is heuristic.
 *
 * Signals:
 *   - valid 40-hex address (required; non-hex → 0).
 *   - excludes zero-address (0x000…000) → 0.
 *   - excludes obvious burn patterns (0xdead…, 0xbeef…) → 0.
 *   - canonical-verified mainnet blue-chip → clears the floor (see below).
 *   - everything else → 50 (neutral "unknown-ish"). Caller may reject if it's
 *     below their min_acceptable_score.
 *
 * Why the canonical-verified branch exists: in `provider = "internal_only"`
 * mode there is no external oracle, so every structurally-valid unknown address
 * scores a neutral 50 and is rejected by the policy floor (default 70). That
 * makes the safety gate mathematically unpassable — every opportunity is
 * rejected `safety_below_threshold`, so `/opportunities?viable_only=true`
 * returns 0 cards even though the search path emits positive opportunities. A
 * token on the canonical list is, by definition, a real non-scam contract, so
 * it short-circuits to a verified score that ALWAYS clears the operator's own
 * floor. This keeps the gate meaningful (unknown/scam tokens still get 50 →
 * blocked) instead of blocking everything.
 */

import type { TokenSafetyRecord } from "./cache.js";
import { normalizeAddress } from "./cache.js";

const ZERO = "0x0000000000000000000000000000000000000000";
const SUSPICIOUS_PREFIXES = ["0xdead", "0xbeef", "0xdeadbeef"];

/**
 * Canonical-verified mainnet (chain_id=1) token addresses.
 *
 * Doctrine exception (same class as `canonical_token_decimals` in shared-rs):
 * these are IMMUTABLE CONTRACT CONSTANTS — well-known, non-scam addresses of
 * blue-chip / widely-traded ERC-20s — NOT market data (price/liquidity/tvl) and
 * NOT operator config. Listing them violates neither RULE 00 (zero-mocks: no
 * data is fabricated, a verdict is just produced earlier) nor the no-hardcode
 * doctrine (which forbids hardcoding OPERATOR values / config).
 *
 * Scoped to mainnet (chainId === 1): the same hex address on another chain is a
 * different (possibly scam) contract, so it must NOT be trusted. Other chains
 * need their own canonical sets (future work).
 *
 * Addresses verified from the repo's trusted `uniswap-tokenlist.json`
 * (backend/api-server/src/services/uniswap-tokenlist.json) — not recalled.
 */
const CANONICAL_MAINNET: ReadonlySet<string> = new Set([
  "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", // WETH
  "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", // USDC
  "0xdac17f958d2ee523a2206206994597c13d831ec7", // USDT
  "0x6b175474e89094c44da98b954eedeac495271d0f", // DAI
  "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599", // WBTC
  "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984", // UNI
  "0x514910771af9ca656af840dff83e8264ecf986ca", // LINK
  "0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9", // AAVE
  "0x6982508145454ce325ddbe47a25d4ec3d2311933", // PEPE
  "0x4d224452801aced8b2f0aebe155379bb5d594381", // APE
  "0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce", // SHIB
  "0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2", // MKR
  "0xd533a949740bb3306d119cc777fa900ba034cd52", // CRV
  "0x6b3595068778dd592e39a122f4f5a5cf09c90fe2", // SUSHI
  "0xc00e94cb662c3520282e6f5717214004a7f26888", // COMP
  "0xc18360217d8f7ab5e7c516566761ea12ce7f9d72", // ENS
  "0xc944e90c64b2c07662a292be6244bdf05cda44a7", // GRT
  "0x5a98fcbea516cf06857215779fd812ca3bef1b32", // LDO
  "0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0", // MATIC
  "0x853d955acef822db058eb8505911ed77f175b99e", // FRAX
  "0x4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b", // CVX
  "0x3432b6a60d23ca0dfca7761b7ab56459d9c964d0", // FXS
  "0xc011a73ee8576fb46f5e1c5751ca3b9fe0af2a6f", // SNX
  "0xba100000625a3754423978a60c9317c58a424e3d", // BAL
  "0x0bc529c00c6401aef6d220be8c6ea1667f6ad93e", // YFI
  "0x111111111117dc0aa78b770fa6a738034120c302", // 1INCH
]);

/// Internal confidence level for a canonical-verified token. `scoreInternal`
/// raises it to at least the operator's floor, so the gate can never become
/// unpassable for canonical tokens again (regression guard for the 0-opps bug).
const CANONICAL_SCORE = 95;

/**
 * Canonical-verified verdict for a (chainId, addr), or `null` if the pair is
 * not a known mainnet blue-chip. Pure / deterministic — no I/O — so `checkToken`
 * calls it BEFORE the cache read to (a) take effect immediately on deploy and
 * (b) overwrite any stale sub-floor cache entry written by the old neutral
 * heuristic. `scoreInternal` also calls it so the heuristic stays self-complete
 * and directly testable.
 *
 * `addr` may be checksummed; it is normalized here. Returns `null` (not canonical)
 * for an unparseable address — the caller's invalid-path handles it.
 */
export function canonicalSafetyRecord(
  chainId: number,
  addr: string,
  ttlSecondsOk: number,
  minAcceptableScore: number,
): Omit<TokenSafetyRecord, "updated_at"> | null {
  let a: string;
  try { a = normalizeAddress(addr); }
  catch { return null; }
  if (chainId !== 1 || !CANONICAL_MAINNET.has(a)) return null;
  return {
    chain_id: chainId, token_address: a,
    safety_score: Math.max(CANONICAL_SCORE, minAcceptableScore),
    flags: { reason: "canonical_verified", note: "mainnet blue-chip; internal verification (no external oracle)" },
    source: "internal",
    ttl_expires_at: new Date(Date.now() + ttlSecondsOk * 1000),
  };
}

export function scoreInternal(chainId: number, addr: string, ttlSecondsOk: number, ttlSecondsBad: number, minAcceptableScore = 70): Omit<TokenSafetyRecord, "updated_at"> {
  let a: string;
  try { a = normalizeAddress(addr); }
  catch {
    return {
      chain_id: chainId, token_address: addr, safety_score: 0,
      flags: { reason: "invalid_address_format" },
      source: "internal",
      ttl_expires_at: new Date(Date.now() + ttlSecondsBad * 1000),
    };
  }

  if (a === ZERO) {
    return {
      chain_id: chainId, token_address: a, safety_score: 0,
      flags: { reason: "zero_address" },
      source: "internal",
      ttl_expires_at: new Date(Date.now() + ttlSecondsBad * 1000),
    };
  }

  const looksSuspicious = SUSPICIOUS_PREFIXES.some(p => a.startsWith(p));
  if (looksSuspicious) {
    return {
      chain_id: chainId, token_address: a, safety_score: 10,
      flags: { reason: "suspicious_prefix" },
      source: "internal",
      ttl_expires_at: new Date(Date.now() + ttlSecondsBad * 1000),
    };
  }

  // Canonical-verified mainnet token → verified score that always clears the
  // floor. Delegated to `canonicalSafetyRecord`, which `checkToken` also calls
  // pre-cache so a stale sub-floor entry can never suppress a canonical token.
  const canon = canonicalSafetyRecord(chainId, a, ttlSecondsOk, minAcceptableScore);
  if (canon) return canon;

  // Neutral. Real signal requires an external provider.
  return {
    chain_id: chainId, token_address: a, safety_score: 50,
    flags: { reason: "internal_heuristic_neutral", note: "external provider required for authoritative score" },
    source: "internal",
    ttl_expires_at: new Date(Date.now() + ttlSecondsOk * 1000),
  };
}
