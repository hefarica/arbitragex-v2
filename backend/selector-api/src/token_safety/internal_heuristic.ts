/**
 * Internal heuristic — when no external provider is available or all providers
 * are circuit-broken. This is intentionally conservative: it scores based on
 * structural signals of the address (not call-site deep checks). It marks
 * `source = 'internal'` so consumers know the grade is heuristic.
 *
 * Signals:
 *   - valid 40-hex address (required; non-hex → 0).
 *   - excludes zero-address (0x000…000) → 0.
 *   - excludes obvious burn patterns (0xdead…, 0xbeef…) → 10.
 *   - curated-verified token (Uniswap default list / CoinGecko index) → tiered
 *     verified score that clears the operator's floor (see below).
 *   - everything else → 50 (neutral "unknown-ish"). Caller may reject if it's
 *     below their min_acceptable_score.
 *
 * 2026-08-18 (vivid-grove audit): the curated branch previously trusted a
 * hand-picked 26-token subset of the Uniswap list. With `provider =
 * "internal_only"` every OTHER token scored neutral 50 against a floor of 70 —
 * the gate was mathematically unpassable, 100% of detections were rejected
 * `safety_below_threshold` BEFORE any economics ran, and the dashboard rendered
 * hollow cards (no USD, no prices). The curated tiers now consult the SAME
 * committed snapshots the api-server registry uses (@arbx/shared/tokenlists —
 * single source of truth): Uniswap default list (~1,451 tokens, tight editorial)
 * and CoinGecko index (~9,465 coins, broad coverage). Verified against the live
 * rejected feed: 97/100 token occurrences are in the Uniswap list alone.
 *
 * Tiering (honest provenance, not a blanket pass):
 *   - uniswap         → 95 (tight editorial standards)
 *   - coingecko_only  → 75 (real but weaker provenance; passes the default
 *                          floor 70, respects a stricter operator floor)
 *   - unverified      → 50 (absent from BOTH lists — for mainnet this is a
 *                          strong scam signal per the registry doctrine)
 *
 * These are IMMUTABLE CONTRACT IDENTITY snapshots — verified addresses — NOT
 * market data and NOT operator config (same doctrine exception as
 * `canonical_token_decimals` in shared-rs). RULE 00 intact: no data is
 * fabricated, a verdict is just produced earlier.
 */

import { curatedVerificationTier } from "@arbx/shared/tokenlists";
import type { TokenSafetyRecord } from "./cache.js";
import { normalizeAddress } from "./cache.js";

const ZERO = "0x0000000000000000000000000000000000000000";
const SUSPICIOUS_PREFIXES = ["0xdead", "0xbeef", "0xdeadbeef"];

/// Internal confidence level for a Uniswap-curated token. `scoreInternal`
/// raises it to at least the operator's floor, so the gate can never become
/// unpassable for curated tokens again (regression guard for the 0-opps bug).
const CANONICAL_SCORE = 95;
/// CoinGecko-indexed but not Uniswap-curated: real, weaker provenance. Above
/// the default floor (70) so legitimate long-tail tokens flow; below 95 so the
/// tier stays distinguishable. A stricter operator floor (>75) still blocks it.
const COINGECKO_TIER_SCORE = 75;

/**
 * Curated-verified verdict for a (chainId, addr), or `null` if the pair is in
 * NEITHER curated list. Tiered by provenance (see header). Multi-chain — the
 * snapshots cover 24 EVM chains, not just mainnet. Pure / deterministic — no
 * I/O — so `checkToken` calls it BEFORE the cache read to (a) take effect
 * immediately on deploy and (b) overwrite any stale sub-floor cache entry
 * written by the old neutral heuristic. `scoreInternal` also calls it so the
 * heuristic stays self-complete and directly testable.
 *
 * `addr` may be checksummed; it is normalized here. Returns `null` (not
 * curated) for an unparseable address — the caller's invalid-path handles it.
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
  const tier = curatedVerificationTier(chainId, a);
  if (tier === "unverified") return null;
  const score =
    tier === "uniswap"
      ? Math.max(CANONICAL_SCORE, minAcceptableScore)
      : COINGECKO_TIER_SCORE;
  return {
    chain_id: chainId, token_address: a,
    safety_score: score,
    flags: {
      reason: tier === "uniswap" ? "canonical_verified" : "coingecko_indexed",
      note: tier === "uniswap"
        ? "Uniswap default list; internal verification (no external oracle)"
        : "CoinGecko-indexed only; weaker provenance than Uniswap-curated",
    },
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

  // Curated-verified token (Uniswap tier or CoinGecko tier) → verified score
  // that clears the floor. Delegated to `canonicalSafetyRecord`, which
  // `checkToken` also calls pre-cache so a stale sub-floor entry can never
  // suppress a curated token.
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
