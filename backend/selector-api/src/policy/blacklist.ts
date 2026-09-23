/**
 * Dynamic blacklist / whitelist over Redis SETs.
 *
 * Keys:
 *   arbx:blacklist:tokens:<chain_id>    — SET of lowercase addresses
 *   arbx:whitelist:tokens:<chain_id>    — SET of lowercase addresses
 *
 * Per-entry TTL is not natively supported by Redis SETs, so entries added with
 * TTL also get a mirror key `arbx:blacklist:tokens:<chain_id>:<addr>:ttl` that
 * expires; a background sweeper (S6) removes members whose mirror key vanished.
 * For S3, we support immediate add/remove and list; TTL sweeper is deferred.
 */

import type { Redis } from "ioredis";

function keyBlack(chainId: number): string { return `arbx:blacklist:tokens:${chainId}`; }
function keyWhite(chainId: number): string { return `arbx:whitelist:tokens:${chainId}`; }

function normalize(addr: string): string {
  const s = addr.trim().toLowerCase();
  if (!/^0x[0-9a-f]{40}$/.test(s)) throw new Error("invalid address");
  return s;
}

export async function addToBlacklist(redis: Redis, chainId: number, addr: string, _reason?: string): Promise<void> {
  await redis.sadd(keyBlack(chainId), normalize(addr));
}
export async function removeFromBlacklist(redis: Redis, chainId: number, addr: string): Promise<number> {
  return redis.srem(keyBlack(chainId), normalize(addr));
}
export async function isBlacklisted(redis: Redis, chainId: number, addr: string): Promise<boolean> {
  const r = await redis.sismember(keyBlack(chainId), normalize(addr));
  return r === 1;
}
export async function listBlacklist(redis: Redis, chainId: number): Promise<string[]> {
  return redis.smembers(keyBlack(chainId));
}

export async function addToWhitelist(redis: Redis, chainId: number, addr: string): Promise<void> {
  await redis.sadd(keyWhite(chainId), normalize(addr));
}
/** SEL-02 note (2026-09-24): this whitelist has ZERO call-sites in the\n * decision pipeline (grep verified — only defined here). The gate is\n * default-ALLOW: any token not in the blacklist passes. To switch to\n * default-deny, wire this into prefilter — until then it is dead surface. */\nexport async function isWhitelisted(redis: Redis, chainId: number, addr: string): Promise<boolean> {
  const r = await redis.sismember(keyWhite(chainId), normalize(addr));
  return r === 1;
}

/** True if the opportunity's tokens are safe against both blacklists.
 *
 * SEL-01 fix (2026-09-24): the previous version THREW on a malformed address
 * (normalize() → "invalid address"), and the consumer's catch block does NOT
 * XACK — the message became a permanent poison pill (error loop + growing lag
 * with no cleanup). Now: a malformed address is an honest REJECT with a typed
 * reason (fail-closed — the token cannot be verified safe), and the caller
 * acks it. Never throws for a data-quality issue.
 */
export async function pairAllowed(
  redis: Redis, chainId: number, tokenIn: string, tokenOut: string,
): Promise<{ allowed: boolean; reason?: string; blocked_token?: string }> {
  for (const [label, addr] of [["token_in", tokenIn], ["token_out", tokenOut]] as const) {
    const norm = normalizeOrNull(addr);
    if (norm === null) {
      return { allowed: false, reason: `invalid_token_address:${label}`, blocked_token: addr };
    }
    if (await isBlacklisted(redis, chainId, norm)) {
      return { allowed: false, reason: "blacklist_hit", blocked_token: norm };
    }
  }
  return { allowed: true };
}

/** normalize() without the throw — null on malformed (for read-side gates). */
function normalizeOrNull(addr: string): string | null {
  const s = addr?.trim?.()?.toLowerCase?.() ?? "";
  return /^0x[0-9a-f]{40}$/.test(s) ? s : null;
}
