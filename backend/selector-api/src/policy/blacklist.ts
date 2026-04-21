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

import type Redis from "ioredis";

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
export async function isWhitelisted(redis: Redis, chainId: number, addr: string): Promise<boolean> {
  const r = await redis.sismember(keyWhite(chainId), normalize(addr));
  return r === 1;
}

/** True if the opportunity's tokens are safe against both blacklists. */
export async function pairAllowed(
  redis: Redis, chainId: number, tokenIn: string, tokenOut: string,
): Promise<{ allowed: boolean; reason?: string; blocked_token?: string }> {
  if (await isBlacklisted(redis, chainId, tokenIn)) {
    return { allowed: false, reason: "blacklist_hit", blocked_token: normalize(tokenIn) };
  }
  if (await isBlacklisted(redis, chainId, tokenOut)) {
    return { allowed: false, reason: "blacklist_hit", blocked_token: normalize(tokenOut) };
  }
  return { allowed: true };
}
