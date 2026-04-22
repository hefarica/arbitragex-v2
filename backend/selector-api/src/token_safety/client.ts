/**
 * Token safety facade.
 *
 * Fetch order:
 *   1. DB cache (if fresh → return)
 *   2. External provider (under CircuitBreaker + timeout)
 *      - on success → upsert cache → return
 *      - on CB open / timeout / non-200 → fall through
 *   3. Internal heuristic fallback (always returns, marked source='internal')
 *
 * The final record is always persisted with an appropriate TTL so subsequent
 * calls short-circuit at step 1.
 */

import type pg from "pg";
import type { AppConfig, CircuitBreaker } from "@arbx/shared";
import { CircuitBreakerOpenError } from "@arbx/shared";
import {
  getCached, upsertCached, type TokenSafetyRecord,
} from "./cache.js";
import { fetchGoPlus } from "./goplus.js";
import { scoreInternal } from "./internal_heuristic.js";

export async function checkToken(
  pool: pg.Pool,
  cb: CircuitBreaker,
  cfg: AppConfig,
  chainId: number,
  address: string,
): Promise<TokenSafetyRecord> {
  // 1. Cache hit
  const cached = await getCached(pool, chainId, address);
  if (cached) return cached;

  // 2. External provider (only if configured)
  if (cfg.token_safety.provider !== "internal_only") {
    try {
      const fetched = await cb.execute(async () => {
        if (cfg.token_safety.provider === "goplus") {
          const apiKey = process.env["GOPLUS_API_KEY"];
          const baseUrl = cfg.token_safety.goplus_base_url;
          if (!baseUrl) {
            // Config schema's refine should block this; second line of defence.
            throw new Error("goplus_base_url missing — operator has not configured GoPlus endpoint");
          }
          return fetchGoPlus(chainId, address, {
            ...(apiKey ? { apiKey } : {}),
            timeoutMs: cfg.token_safety.api_call_timeout_ms,
            baseUrl,
          });
        }
        // honeypot_is provider not implemented yet; fall through to internal.
        return null;
      });

      if (fetched) {
        const ttl = fetched.safety_score >= cfg.token_safety.min_acceptable_score
          ? cfg.token_safety.ttl_seconds_ok
          : cfg.token_safety.ttl_seconds_bad;
        fetched.ttl_expires_at = new Date(Date.now() + ttl * 1000);
        await upsertCached(pool, fetched);
        return { ...fetched, updated_at: new Date() };
      }
    } catch (err) {
      if (!(err instanceof CircuitBreakerOpenError)) {
        // swallow; we'll fall through to heuristic (do not re-throw, never block decisions)
      }
    }
  }

  // 3. Internal heuristic fallback
  const internal = scoreInternal(
    chainId, address,
    cfg.token_safety.ttl_seconds_ok,
    cfg.token_safety.ttl_seconds_bad,
  );
  await upsertCached(pool, internal);
  return { ...internal, updated_at: new Date() };
}
