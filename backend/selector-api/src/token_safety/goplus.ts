/**
 * GoPlus token_security client.
 *
 * API: https://api.gopluslabs.io/api/v1/token_security/<chain_id>?contract_addresses=<addr>
 *
 * Without API key, GoPlus serves reduced rate limits but works for basic use.
 * The client below supports an optional Authorization header when GOPLUS_API_KEY
 * is set.
 *
 * FASE 2: scoring is delegated to the pure 11-signal composite in `composite.ts`.
 * The old inline +40-base heuristic is replaced by computeComposite(parseGoPlusFlags(...))
 * which consumes honeypot / fake_token / open_source / holder_count / lp_holders
 * (concentration + lock) / ownership / buy_tax / sell_tax / transfer_pausable /
 * slippage_modifiable / cannot_sell_all. token_age + tvl arrive null (GoPlus alone
 * can't compute them) — marked enrichment_pending, NEVER fabricated.
 *
 * TTL by classification (operator-tunable via opts.ttlSeconds{Ok,Warn,Bad}):
 *   SAFE → ttlSecondsOk (moderate), WARN → min(ok, 30min) (re-check sooner),
 *   DROP → ttlSecondsBad (slow; bad tokens don't need frequent refresh).
 */

import type { TokenSafetyRecord } from "./cache.js";
import { normalizeAddress } from "./cache.js";
import { computeComposite, parseGoPlusFlags, type GoPlusFlags } from "./composite.js";

export interface GoPlusClientOpts {
  /** GoPlus API key (optional — unauth is allowed but rate-limited). */
  apiKey?: string;
  timeoutMs: number;
  /**
   * GoPlus API base URL. REQUIRED. The operator opts in to external token-safety
   * calls by setting this in `configs/app.toml` → `[token_safety].goplus_base_url`
   * when they choose provider=`goplus`. We do NOT default to
   * `https://api.gopluslabs.io` — silently reaching out to a third party without
   * operator sign-off violates the no-hardcode doctrine.
   */
  baseUrl: string;
  /** TTL (seconds) for SAFE-classified tokens. Default 3600 (1h). */
  ttlSecondsOk?: number;
  /** TTL (seconds) for DROP-classified tokens. Default 86400 (24h). */
  ttlSecondsBad?: number;
}

function ttlForClassification(
  classification: "SAFE" | "WARN" | "DROP",
  ttlOk: number,
  ttlBad: number,
  now: Date,
): Date {
  const warnTtl = Math.min(ttlOk, 1800); // WARN re-checks sooner (risky)
  const secs = classification === "SAFE" ? ttlOk : classification === "WARN" ? warnTtl : ttlBad;
  return new Date(now.getTime() + secs * 1000);
}

export async function fetchGoPlus(
  chainId: number,
  address: string,
  opts: GoPlusClientOpts,
): Promise<Omit<TokenSafetyRecord, "updated_at"> | null> {
  if (!opts.baseUrl) {
    throw new Error("goplus_base_url_required: set [token_safety].goplus_base_url in config");
  }
  const a = normalizeAddress(address);
  const url = `${opts.baseUrl.replace(/\/+$/, "")}/api/v1/token_security/${chainId}?contract_addresses=${a}`;

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
  const result = (body as { result?: Record<string, GoPlusFlags> })?.result;
  if (!result) return null;
  const flags = result[a] ?? result[a.toLowerCase()];
  if (!flags) return null;

  // FASE 2: pure composite (parseGoPlusFlags + computeComposite).
  const now = new Date();
  const composite = computeComposite(parseGoPlusFlags(flags, now));
  const ttlOk = opts.ttlSecondsOk ?? 3600;
  const ttlBad = opts.ttlSecondsBad ?? 86400;
  const ttlExpiresAt = ttlForClassification(composite.classification, ttlOk, ttlBad, now);

  // Enriched flags: raw provider summary + sub-signals + composite metadata.
  // cache.ts stores this JSONB verbatim — the operator + Rust checklist read it.
  const enrichedFlags: Record<string, unknown> = {
    raw_goplus: flags,
    sub_signals: composite.sub_signals,
    composite_score: composite.score,
    classification: composite.classification,
    hard_gate: composite.hard_gate,
    enrichment_pending: composite.enrichment_pending,
    refreshed_at: now.toISOString(),
    ttl_seconds: Math.round((ttlExpiresAt.getTime() - now.getTime()) / 1000),
    paper_shadow_only: composite.paper_shadow_only,
    provider: "goplus",
  };

  return {
    chain_id: chainId,
    token_address: a,
    safety_score: composite.score,
    flags: enrichedFlags,
    source: "goplus",
    ttl_expires_at: ttlExpiresAt,
  };
}
