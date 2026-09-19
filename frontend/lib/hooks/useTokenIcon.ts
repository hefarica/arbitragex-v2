"use client";

/**
 * useTokenIcon — resolves a reliable token icon through the 4-tier cascade:
 *
 *   1. Known tokens (lib/known-tokens) — instant, no network.
 *   2. In-memory cache                 — instant, no network (per tab session).
 *   3. GET /api/v1/token-icon/:chain/:addr (api-server) — Redis → DexScreener.
 *   4. Jazzicon (DeterministicAvatar)  — deterministic SVG, always available.
 *
 * Design notes:
 *   - Tiers 1+2 resolve SYNCHRONOUSLY in the useState initializer, so a known
 *     or already-cached token renders its icon on the FIRST paint (SSR === CSR,
 *     no shimmer flash, R1-friendly). Only an unknown, uncached, valid token
 *     triggers the network tier in an effect.
 *   - Network tier (ICON-RAIN-20260917): a module-scoped single-flight fetch
 *     shared by every hook instance — concurrent cards of the same token share
 *     ONE request, the fetch survives unmounts (so the cache still populates;
 *     no ERR_ABORTED rain), and failures are negatively cached for
 *     FAILURE_TTL_MS. An `alive` flag prevents setState-after-unmount (React
 *     strict-mode safe); only the 5s timeout aborts the request.
 *   - Fail-honest (RULE 00 / R8): any failure degrades to a deterministic
 *     jazzicon and surfaces the error string; it never fabricates an icon.
 *   - No window/document/Date.now/Math.random outside the effect.
 */

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import {
  getCachedIcon,
  iconCacheKey,
  normalizeAddress,
  resolveKnownIcon,
  setCachedIcon,
  type IconResolution,
  type IconSource,
} from "@/lib/known-tokens";

export interface UseTokenIconArgs {
  address: string | null | undefined;
  /** FE-0029 (§28): null = payload had no chain — resolves NOTHING (NaN
   *  short-circuits every isFinite guard), avatar fallback only. */
  chainId: number | string | null;
  symbol?: string | null;
  /** When false, no resolution/network happens; returns a jazzicon fallback. */
  enabled?: boolean;
}

export interface UseTokenIconResult {
  iconUrl: string | null;
  source: IconSource;
  loading: boolean;
  error: string | null;
  /** true ⇔ no image; caller renders the deterministic avatar with `seed`. */
  isFallback: boolean;
  /** Stable seed for DeterministicAvatar (normalized address when valid). */
  seed: string;
}

const FETCH_TIMEOUT_MS = 5000;
// ICON-RAIN-20260917: negative-cache TTL for FAILED lookups (network error /
// timeout / non-200). Mirrors the api-server's NEG_TTL_SECS=60 for miss
// responses (token-icon.ts:56) so both layers agree on how long a failure is
// remembered before a retry is allowed. A failed lookup degrades to the
// deterministic avatar for the TTL window — the failure is real and surfaced,
// never fabricated into an icon (RULE 00 / R8).
const FAILURE_TTL_MS = 60_000;
const FALLBACK: IconResolution = {
  iconUrl: null,
  source: "jazzicon",
  isFallback: true,
};

/** Synchronous tiers 1+2 (known map + in-memory cache). No network. */
function resolveSync(
  chainId: number,
  addr: string | null,
): IconResolution | null {
  if (addr == null || !Number.isFinite(chainId)) return null;
  return resolveKnownIcon(chainId, addr) ?? getCachedIcon(chainId, addr) ?? null;
}

interface ApiIconResponse {
  ok?: boolean;
  iconUrl?: string | null;
  source?: string;
  cached?: boolean;
}

// ── Network-tier single-flight + failure cache (ICON-RAIN-20260917) ─────────
//
// Defect (BROWSE §4.5, audits/first-understand-20260917): the live feed mounts
// many cards for the SAME long-tail token in one render (~30x PEPE observed).
// Each mount fired its OWN GET /api/v1/token-icon/:chain/:addr and aborted it
// on unmount, so (a) the edge absorbed N identical concurrent requests per
// token per render, (b) the aborted requests (ERR_ABORTED burst) never
// completed, so the in-memory cache never populated and every remount re-hit
// the network — a sustained icon rain.
//
// Fix — dedupe at the URL level, module-scoped (shared by every hook instance
// of the tab session):
//   1. Single-flight: concurrent resolutions of the same (chainId, address)
//      share ONE in-flight promise → 1 network request per token per render.
//   2. The shared fetch SURVIVES component unmounts (the per-instance
//      AbortController is gone; only the 5s timeout aborts). A fetch started
//      by a card that scrolled away still completes and populates the cache,
//      so the next mount resolves synchronously (tier 2).
//   3. Failed lookups are negatively cached for FAILURE_TTL_MS: remounts
//      degrade to the deterministic avatar without re-hammering the edge.

interface IconFetchOutcome {
  res: IconResolution;
  error: string | null;
}

interface FailureEntry {
  /** Epoch ms until which this (chainId, address) is treated as failed. */
  until: number;
  /** Honest error string from the real failure (surfaced to the UI). */
  message: string;
}

const inflightIconFetches = new Map<string, Promise<IconFetchOutcome>>();
const failedIconFetches = new Map<string, FailureEntry>();

/** True when the last network attempt for this token failed recently. */
export function recentIconFetchFailure(
  chainId: number,
  address: string,
): FailureEntry | null {
  const entry = failedIconFetches.get(iconCacheKey(chainId, address));
  if (entry == null) return null;
  if (entry.until <= Date.now()) {
    failedIconFetches.delete(iconCacheKey(chainId, address));
    return null;
  }
  return entry;
}

/**
 * Single-flight network tier. Resolves (and caches) the icon for
 * (chainId, address). Concurrent calls with the same key share one request.
 */
export function fetchIconResolution(
  chainId: number,
  addr: string,
): Promise<IconFetchOutcome> {
  const key = iconCacheKey(chainId, addr);
  const inflight = inflightIconFetches.get(key);
  if (inflight) return inflight;

  const p = (async (): Promise<IconFetchOutcome> => {
    const ctrl = new AbortController();
    const timeoutId = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
    try {
      const url = `${getApiBaseUrl()}/api/v1/token-icon/${chainId}/${addr}`;
      const res = await fetch(url, {
        headers: { accept: "application/json" },
        signal: ctrl.signal,
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = (await res.json()) as ApiIconResponse;
      const iconUrl =
        typeof json.iconUrl === "string" && json.iconUrl.length > 0
          ? json.iconUrl
          : null;
      const source: IconSource =
        json.source === "redis" ||
        json.source === "known" ||
        json.source === "dexscreener" ||
        json.source === "jazzicon"
          ? json.source
          : iconUrl
            ? "redis"
            : "jazzicon";
      const resolution: IconResolution = {
        iconUrl,
        source,
        isFallback: iconUrl == null,
      };
      setCachedIcon(chainId, addr, resolution);
      failedIconFetches.delete(key);
      return { res: resolution, error: null };
    } catch (e) {
      const err = e as Error;
      // ICON-RAIN-20260917: negative-cache the failure so remounts do not
      // re-fetch on every render. Fail-honest: the avatar fallback stays and
      // the real error string is surfaced; nothing is fabricated.
      failedIconFetches.set(key, {
        until: Date.now() + FAILURE_TTL_MS,
        message: err.name === "AbortError" ? `timeout after ${FETCH_TIMEOUT_MS}ms` : err.message,
      });
      return { res: FALLBACK, error: err.message };
    } finally {
      clearTimeout(timeoutId);
      inflightIconFetches.delete(key);
    }
  })();

  inflightIconFetches.set(key, p);
  return p;
}

/** Test-only — clear the single-flight + failure state. */
export function _resetIconFetchStateForTests(): void {
  inflightIconFetches.clear();
  failedIconFetches.clear();
}

export function useTokenIcon(args: UseTokenIconArgs): UseTokenIconResult {
  const { address, symbol } = args;
  const enabled = args.enabled !== false;
  const chainId = args.chainId == null ? NaN : Number(args.chainId);
  const addr = normalizeAddress(address);

  // Stable jazzicon seed: normalized address when valid, else the raw input or
  // symbol — so even malformed addresses get a deterministic (if arbitrary) art.
  const seed =
    addr ??
    (typeof address === "string" && address.trim().length > 0
      ? address.trim().toLowerCase()
      : (symbol ?? "unknown"));

  const [state, setState] = useState<{
    res: IconResolution;
    loading: boolean;
    error: string | null;
  }>(() => {
    const sync = resolveSync(chainId, addr);
    if (sync) return { res: sync, loading: false, error: null };
    // Disabled / invalid / unknown-not-cached.
    const willFetch = enabled && addr != null && Number.isFinite(chainId);
    return { res: FALLBACK, loading: willFetch, error: null };
  });

  useEffect(() => {
    if (!enabled || addr == null || !Number.isFinite(chainId)) return;
    // Tiers 1+2 already cover this synchronously → never hit the network.
    const sync = resolveSync(chainId, addr);
    if (sync) {
      setState({ res: sync, loading: false, error: null });
      return;
    }

    // ICON-RAIN-20260917: recently-failed lookup → degrade to the deterministic
    // avatar WITHOUT re-hitting the edge on every remount (fail-honest: the real
    // error string is surfaced; the avatar is the declared fallback, not data).
    const failure = recentIconFetchFailure(chainId, addr);
    if (failure) {
      setState({ res: FALLBACK, loading: false, error: failure.message });
      return;
    }

    let alive = true;
    setState((s) => ({ ...s, loading: true, error: null }));

    // ICON-RAIN-20260917: shared single-flight fetch — concurrent mounts of the
    // same token attach to ONE request, and the request survives this
    // component's unmount so it still populates the cache for the next mount.
    // Only `alive` guards the setState; there is no per-instance abort anymore.
    void fetchIconResolution(chainId, addr).then((outcome) => {
      if (alive) {
        setState({ res: outcome.res, loading: false, error: outcome.error });
      }
    });

    return () => {
      alive = false;
    };
  }, [chainId, addr, enabled]);

  return {
    iconUrl: state.res.iconUrl,
    source: state.res.source,
    loading: state.loading,
    error: state.error,
    isFallback: state.res.isFallback,
    seed,
  };
}
