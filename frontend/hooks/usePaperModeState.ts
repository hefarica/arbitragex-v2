"use client";

/**
 * usePaperModeState — canonical paper-mode resolver hook.
 *
 * Fetches the server-evaluated paper-mode state on mount and polls every 15s.
 * AbortController cancels in-flight requests on unmount or when a new fetch
 * supersedes the previous one. Fail-safe: any error surfaces as `error` and
 * `data` reverts to `DEFAULT_SAFE_STATE` (enabled=false, degraded=true).
 *
 * This is read-only / paper-shadow code. No capital, no broadcast.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import {
  PaperModeStateSchema,
  DEFAULT_SAFE_STATE,
  type PaperModeState,
} from "@/lib/schemas";

export interface UsePaperModeStateResult {
  data: PaperModeState;
  isLoading: boolean;
  isRefreshing: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

const POLL_MS = 15_000;

async function fetchPaperModeState(
  chainId: number | undefined,
  signal: AbortSignal,
): Promise<PaperModeState> {
  const base = getApiBaseUrl();
  const path =
    chainId != null
      ? `${base}/api/paper-mode/state?chain_id=${chainId}`
      : `${base}/api/paper-mode/state`;

  const res = await fetch(path, {
    signal,
    credentials: "include",
    headers: { accept: "application/json" },
  });

  if (!res.ok) {
    throw new Error(`HTTP ${res.status}`);
  }

  const parsed = await res.json();
  const result = PaperModeStateSchema.safeParse(parsed);
  if (!result.success) {
    throw new Error(
      `schema: ${result.error.issues
        .map((i) => `${i.path.join(".") || "<root>"}: ${i.message}`)
        .join("; ")}`,
    );
  }
  return result.data;
}

export function usePaperModeState(chainId?: number): UsePaperModeStateResult {
  const [data, setData] = useState<PaperModeState>(DEFAULT_SAFE_STATE);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const hasFetchedRef = useRef(false);
  const abortCtrlRef = useRef<AbortController | null>(null);

  const refetch = useCallback(async () => {
    // Cancel any in-flight request before starting a new one.
    abortCtrlRef.current?.abort();
    const ctrl = new AbortController();
    abortCtrlRef.current = ctrl;

    const isFirstFetch = !hasFetchedRef.current;
    if (isFirstFetch) {
      setIsLoading(true);
    } else {
      setIsRefreshing(true);
    }
    setError(null);

    try {
      const result = await fetchPaperModeState(chainId, ctrl.signal);
      setData(result);
      setError(null);
    } catch (e) {
      const err = e as Error;
      if (err.name === "AbortError") {
        // Race-abort from unmount or superseded fetch — state untouched.
        return;
      }
      setError(err);
      setData(DEFAULT_SAFE_STATE);
    } finally {
      // Only update loading flags if this controller is still current.
      if (abortCtrlRef.current === ctrl) {
        if (isFirstFetch) {
          setIsLoading(false);
          hasFetchedRef.current = true;
        } else {
          setIsRefreshing(false);
        }
      }
    }
  }, [chainId]);

  useEffect(() => {
    let alive = true;
    const tick = async () => {
      if (!alive) return;
      await refetch();
    };
    void tick();
    const interval = window.setInterval(tick, POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(interval);
      abortCtrlRef.current?.abort();
    };
  }, [refetch]);

  return { data, isLoading, isRefreshing, error, refetch };
}
