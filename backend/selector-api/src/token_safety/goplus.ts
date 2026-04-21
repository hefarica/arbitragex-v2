/**
 * GoPlus token_security client.
 *
 * API: https://api.gopluslabs.io/api/v1/token_security/<chain_id>?contract_addresses=<addr>
 *
 * Without API key, GoPlus serves reduced rate limits but works for basic use.
 * The client below supports an optional Authorization header when GOPLUS_API_KEY
 * is set.
 *
 * Score derivation from GoPlus signals (0..100):
 *   + 40 base
 *   + 20 if is_open_source == "1"
 *   + 10 if holder_count > 100
 *   + 10 if anti_whale_modifiable == "0"
 *   - 50 if is_honeypot == "1"
 *   - 30 if cannot_sell_all == "1"
 *   - 20 if is_blacklisted == "1"
 *   - 20 if selfdestruct == "1"
 * Final clamped to [0,100].
 */

import type { TokenSafetyRecord } from "./cache.js";
import { normalizeAddress } from "./cache.js";

export interface GoPlusClientOpts {
  apiKey?: string;
  timeoutMs: number;
  baseUrl?: string;
}

export async function fetchGoPlus(
  chainId: number,
  address: string,
  opts: GoPlusClientOpts,
): Promise<Omit<TokenSafetyRecord, "updated_at"> | null> {
  const a = normalizeAddress(address);
  const url = `${opts.baseUrl ?? "https://api.gopluslabs.io"}/api/v1/token_security/${chainId}?contract_addresses=${a}`;

  const ctrl = new AbortController();
  const to = setTimeout(() => ctrl.abort(), opts.timeoutMs);
  const headers: Record<string, string> = { "accept": "application/json" };
  if (opts.apiKey) headers["Authorization"] = opts.apiKey;

  let body: unknown;
  try {
    const res = await fetch(url, { headers, signal: ctrl.signal });
    if (!res.ok) return null;
    body = await res.json();
  } finally {
    clearTimeout(to);
  }

  // GoPlus returns { code, message, result: { "<addr>": { ... flags ... } } }
  const result = (body as { result?: Record<string, Record<string, string | number>> })?.result;
  if (!result) return null;
  const flags = result[a] ?? result[a.toLowerCase()];
  if (!flags) return null;

  let score = 40;
  if (flags["is_open_source"] === "1") score += 20;
  const holderCount = Number(flags["holder_count"] ?? 0);
  if (holderCount > 100) score += 10;
  if (flags["anti_whale_modifiable"] === "0") score += 10;
  if (flags["is_honeypot"] === "1") score -= 50;
  if (flags["cannot_sell_all"] === "1") score -= 30;
  if (flags["is_blacklisted"] === "1") score -= 20;
  if (flags["selfdestruct"] === "1") score -= 20;

  const clamped = Math.max(0, Math.min(100, score));
  return {
    chain_id: chainId,
    token_address: a,
    safety_score: clamped,
    flags: flags as Record<string, unknown>,
    source: "goplus",
    ttl_expires_at: new Date(), // caller sets based on score
  };
}
