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
 *   - AbortController cancels the in-flight request on unmount AND on
 *     arg changes → no setState-after-unmount (React strict-mode safe).
 *   - Fail-honest (RULE 00 / R8): any failure degrades to a deterministic
 *     jazzicon and surfaces the error string; it never fabricates an icon.
 *   - No window/document/Date.now/Math.random outside the effect.
 */

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import {
  getCachedIcon,
  normalizeAddress,
  resolveKnownIcon,
  setCachedIcon,
  type IconResolution,
  type IconSource,
} from "@/lib/known-tokens";

export interface UseTokenIconArgs {
  address: string | null | undefined;
  chainId: number | string;
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

export function useTokenIcon(args: UseTokenIconArgs): UseTokenIconResult {
  const { address, symbol } = args;
  const enabled = args.enabled !== false;
  const chainId = Number(args.chainId);
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

    const ctrl = new AbortController();
    const timeoutId = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
    let alive = true;
    setState((s) => ({ ...s, loading: true, error: null }));

    void (async () => {
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
        if (alive) setState({ res: resolution, loading: false, error: null });
      } catch (e) {
        if ((e as Error).name === "AbortError") return;
        // Fail-honest: deterministic jazzicon + surfaced error. No fabrication.
        if (alive)
          setState({ res: FALLBACK, loading: false, error: (e as Error).message });
      }
    })();

    return () => {
      alive = false;
      clearTimeout(timeoutId);
      ctrl.abort();
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
