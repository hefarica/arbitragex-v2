// Token Icon route — resolves a reliable icon URL for a token, fail-honest.
//
//   GET /api/v1/token-icon/:chainId/:address   → public read
//
// Cascade (Redis is the hot path; everything else is fallback):
//   1. Redis  arbx:token-icons:{chainId}:{address}  — hot cache. Populated by
//      the GeckoTerminal producer (SETEX) AND by tiers 2-4 caching their hits
//      back into this key at the long ICON_TTL_SECS (default 30d — logos are
//      near-immutable; a resolved icon must NOT evaporate when a rate-limited
//      upstream (GeckoTerminal 429 / DexScreener) can't be re-queried).
//   2. Registry (known) — `tokenRegistry.logo_uri` (curated Uniswap+CoinGecko
//      snapshot, ~1.4k tokens). Instant, no network. Result is cached into (1).
//   3. PostgreSQL `tokens.logo_url` — DURABLE backstop populated by the
//      token-enricher (TrustWallet-verified, re-resolved every 7d). Survives a
//      Redis flush/restart and outlives every rate-limited external tier. This
//      is the tier that stops logos from "disappearing": even with Redis empty
//      and GeckoTerminal 429'd, any token the enricher resolved still resolves.
//      Result is cached into (1).
//   4. DexScreener — env-gated outbound fallback for the long tail. Picks the
//      highest-liquidity pair carrying an image for our token. Cached into (1);
//      a miss is negatively cached (short TTL) to avoid hammering the API.
//   5. Jazzicon — iconUrl=null, source="jazzicon"; the frontend renders a
//      deterministic SVG avatar. ALWAYS available.
//
// Doctrine: this is a read/observe + cache-population layer only — no capital,
// no signer, no broadcast. It NEVER returns 500 for a missing icon (R8): a down
// Redis or PG or a failing DexScreener degrades the cascade, it does not error.

import { Router, type Express, type Request, type Response } from "express";
import type { Redis } from "ioredis";
import type { Pool } from "pg";

import { lookupRegistry } from "../services/tokenRegistry.js";
import { chainSlug } from "../services/tokenValidation/liquidityReality.js";

/** Redis key the icon producer (GeckoTerminal/enricher) writes via SETEX. */
export const tokenIconKey = (chainId: number, address: string): string =>
  `arbx:token-icons:${chainId}:${address}`;

/** Short-lived negative-cache key: "DexScreener had no image for this token". */
const tokenIconMissKey = (chainId: number, address: string): string =>
  `arbx:token-icons:miss:${chainId}:${address}`;

const EVM_ADDRESS_RE = /^0x[0-9a-f]{40}$/;

// ── Env-driven config (no-hardcode: operator-overridable; public-API defaults) ──
const DEXSCREENER_BASE_URL =
  process.env["DEXSCREENER_BASE_URL"] ??
  "https://api.dexscreener.com/latest/dex/tokens";
const DEXSCREENER_ENABLED =
  (process.env["ARBX_TOKEN_ICON_DEXSCREENER"] ?? "on").toLowerCase() !== "off";
// Resolved icons are cached for 30d (logos are near-immutable). The short-lived
// external tiers (GeckoTerminal/DexScreener) are rate-limited, so a 1h TTL made
// icons evaporate the moment they expired and the upstream couldn't be re-hit.
const ICON_TTL_SECS = positiveInt(process.env["ARBX_TOKEN_ICON_TTL_SECS"], 2592000, 60);
const NEG_TTL_SECS = positiveInt(process.env["ARBX_TOKEN_ICON_NEG_TTL_SECS"], 60, 15);
const DEX_TIMEOUT_MS = positiveInt(process.env["ARBX_TOKEN_ICON_DEX_TIMEOUT_MS"], 4000, 1000);
const CACHE_MAX_AGE_SECS = 240;

function positiveInt(raw: string | undefined, dflt: number, min: number): number {
  const n = parseInt(raw ?? "", 10);
  return Number.isFinite(n) && n >= min ? n : dflt;
}

type Logger = {
  warn: (obj: object, msg?: string) => void;
  info: (obj: object, msg?: string) => void;
};

export interface TokenIconDeps {
  redis: Redis;
  /** PostgreSQL pool for the durable `tokens.logo_url` tier. Null/absent = skip. */
  pool?: Pool | null;
  logger?: Logger;
}

type IconSource = "redis" | "known" | "db" | "dexscreener" | "jazzicon";

interface IconPayload {
  ok: true;
  chainId: string;
  address: string;
  iconUrl: string | null;
  source: IconSource;
  cached: boolean;
}

// Minimal shape of the DexScreener token response — only the fields we read.
interface DsPair {
  chainId?: string;
  liquidity?: { usd?: number };
  baseToken?: { address?: string };
  info?: { imageUrl?: string };
}
interface DsResponse {
  pairs?: DsPair[] | null;
}

function isHttpUrl(s: string): boolean {
  return /^https?:\/\//i.test(s);
}

/** Best-effort cache write; never throws, never blocks the response. */
async function safeSetex(
  redis: Redis,
  key: string,
  ttl: number,
  val: string,
  logger?: Logger,
): Promise<void> {
  try {
    await redis.setex(key, ttl, val);
  } catch (e) {
    logger?.warn(
      { event: "token_icon.cache_write_failed", key, err: (e as Error).message },
      "token-icon cache write failed (non-fatal)",
    );
  }
}

/**
 * Query DexScreener for the highest-liquidity pair that (a) is on the requested
 * chain, (b) has our token as baseToken, and (c) carries an `info.imageUrl`.
 * Returns the image URL, or null on any miss/error/timeout (fail-honest).
 */
async function fetchDexScreenerIcon(
  chainId: number,
  address: string,
  timeoutMs: number,
): Promise<string | null> {
  const slug = chainSlug(chainId);
  if (!slug) return null;

  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  let raw: unknown;
  try {
    const res = await fetch(`${DEXSCREENER_BASE_URL}/${address}`, {
      signal: ctrl.signal,
      headers: { accept: "application/json" },
    });
    if (!res.ok) return null;
    raw = await res.json();
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }

  const parsed = raw as DsResponse | null;
  const pairs = parsed && Array.isArray(parsed.pairs) ? parsed.pairs : [];
  let best: string | null = null;
  let bestLiq = -1;
  for (const p of pairs) {
    if (!p || p.chainId !== slug) continue;
    const img = p.info?.imageUrl;
    if (typeof img !== "string" || !isHttpUrl(img)) continue;
    const base = p.baseToken?.address?.toLowerCase();
    if (base && base !== address) continue; // only the side that IS our token
    const liq = typeof p.liquidity?.usd === "number" ? p.liquidity.usd : -1;
    if (liq > bestLiq) {
      bestLiq = liq;
      best = img;
    }
  }
  return best;
}

/**
 * Mount GET /api/v1/token-icon/:chainId/:address on `app`.
 *
 * Adapted to the repo's `mountX(app, deps)` convention (cf. `mountPools`); the
 * spec's `mountTokenIconRoutes(app, redis)` is honored via `deps.redis`.
 */
export function mountTokenIconRoutes(app: Express, deps: TokenIconDeps): void {
  const { redis, pool, logger } = deps;
  const r = Router();

  r.get(
    "/api/v1/token-icon/:chainId/:address",
    async (req: Request, res: Response) => {
      const chainId = Number(req.params["chainId"]);
      const addressRaw = String(req.params["address"] ?? "");
      const address = addressRaw.trim().toLowerCase();

      if (!Number.isFinite(chainId) || chainId < 1) {
        res.status(400).json({ ok: false, error: "invalid_chain_id" });
        return;
      }
      if (!EVM_ADDRESS_RE.test(address)) {
        logger?.warn(
          { event: "token_icon.invalid_address", chain_id: chainId, address: addressRaw.slice(0, 64) },
          "token-icon invalid address",
        );
        res.status(400).json({ ok: false, error: "invalid_address" });
        return;
      }

      const send = (
        partial: Pick<IconPayload, "iconUrl" | "source" | "cached">,
        cacheSecs: number,
      ): void => {
        res.setHeader("Cache-Control", `public, max-age=${cacheSecs}`);
        const body: IconPayload = {
          ok: true,
          chainId: String(chainId),
          address,
          ...partial,
        };
        res.status(200).json(body);
      };

      // (1) Redis cache — producer writes a raw imageUrl (or, defensively, JSON).
      try {
        const cached = await redis.get(tokenIconKey(chainId, address));
        if (cached && cached.length > 0) {
          let url: string | null = null;
          if (isHttpUrl(cached)) {
            url = cached;
          } else {
            try {
              const j = JSON.parse(cached) as { iconUrl?: string; imageUrl?: string };
              const u = j.iconUrl ?? j.imageUrl;
              if (typeof u === "string" && isHttpUrl(u)) url = u;
            } catch {
              /* not JSON — ignore, fall through the cascade */
            }
          }
          if (url) {
            send({ iconUrl: url, source: "redis", cached: true }, CACHE_MAX_AGE_SECS);
            return;
          }
        }
      } catch (e) {
        // Redis down → continue the cascade (registry/jazzicon still resolve).
        logger?.warn(
          { event: "token_icon.redis_error", chain_id: chainId, err: (e as Error).message },
          "token-icon redis read failed (degrading to registry/jazzicon)",
        );
      }

      // (2) Registry (known) — curated logo_uri, instant, no network.
      const reg = lookupRegistry(chainId, address);
      if (reg && reg.logo_uri && isHttpUrl(reg.logo_uri)) {
        const url = reg.logo_uri;
        void safeSetex(redis, tokenIconKey(chainId, address), ICON_TTL_SECS, url, logger);
        send({ iconUrl: url, source: "known", cached: false }, CACHE_MAX_AGE_SECS);
        return;
      }

      // (3) PostgreSQL `tokens.logo_url` — durable backstop. Survives a Redis
      // flush/restart and outlives the rate-limited external tiers, so a logo
      // the enricher already resolved never "disappears". Cached into (1). A
      // null pool (no DATABASE_URL) or a query failure degrades, never 500 (R8).
      if (pool) {
        try {
          const result = await pool.query(
            "SELECT logo_url FROM tokens WHERE chain_id = $1 AND address = $2 AND logo_url IS NOT NULL",
            [chainId, address],
          );
          const dbUrl = result.rows[0]?.logo_url;
          if (typeof dbUrl === "string" && isHttpUrl(dbUrl)) {
            void safeSetex(redis, tokenIconKey(chainId, address), ICON_TTL_SECS, dbUrl, logger);
            send({ iconUrl: dbUrl, source: "db", cached: false }, CACHE_MAX_AGE_SECS);
            return;
          }
        } catch (e) {
          logger?.warn(
            { event: "token_icon.db_error", chain_id: chainId, err: (e as Error).message },
            "token-icon db read failed (degrading to dexscreener/jazzicon)",
          );
        }
      }

      // (4) DexScreener fallback — env-gated, timeout-bounded, negative-cached.
      if (DEXSCREENER_ENABLED) {
        let negCached = false;
        try {
          negCached = (await redis.get(tokenIconMissKey(chainId, address))) != null;
        } catch {
          /* negative-cache read failure is non-fatal */
        }
        if (!negCached) {
          const dsUrl = await fetchDexScreenerIcon(chainId, address, DEX_TIMEOUT_MS);
          if (dsUrl) {
            void safeSetex(redis, tokenIconKey(chainId, address), ICON_TTL_SECS, dsUrl, logger);
            logger?.info(
              { event: "token_icon.dexscreener.hit", chain_id: chainId, address },
              "token-icon resolved via dexscreener",
            );
            send({ iconUrl: dsUrl, source: "dexscreener", cached: false }, CACHE_MAX_AGE_SECS);
            return;
          }
          void safeSetex(redis, tokenIconMissKey(chainId, address), NEG_TTL_SECS, "1", logger);
          logger?.info(
            { event: "token_icon.dexscreener.miss", chain_id: chainId, address },
            "token-icon dexscreener returned no image",
          );
        }
      }

      // (5) Jazzicon — deterministic client-side fallback. NEVER a 500.
      send({ iconUrl: null, source: "jazzicon", cached: false }, NEG_TTL_SECS);
    },
  );

  app.use(r);
}
