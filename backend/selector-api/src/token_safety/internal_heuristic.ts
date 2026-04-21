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
 *   - everything else → 50 (neutral "unknown-ish"). Caller may reject if it's
 *     below their min_acceptable_score.
 */

import type { TokenSafetyRecord } from "./cache.js";
import { normalizeAddress } from "./cache.js";

const ZERO = "0x0000000000000000000000000000000000000000";
const SUSPICIOUS_PREFIXES = ["0xdead", "0xbeef", "0xdeadbeef"];

export function scoreInternal(chainId: number, addr: string, ttlSecondsOk: number, ttlSecondsBad: number): Omit<TokenSafetyRecord, "updated_at"> {
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

  // Neutral. Real signal requires an external provider.
  return {
    chain_id: chainId, token_address: a, safety_score: 50,
    flags: { reason: "internal_heuristic_neutral", note: "external provider required for authoritative score" },
    source: "internal",
    ttl_expires_at: new Date(Date.now() + ttlSecondsOk * 1000),
  };
}
