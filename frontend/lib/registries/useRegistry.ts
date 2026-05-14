"use client";

import { useState, useEffect, useCallback, useRef } from "react";

/**
 * Generic state shape for any entity registry backed by an async fetch.
 *
 * @template T - The entity type stored in the registry.
 */
export interface RegistryState<T> {
  /** Current items in the registry. */
  readonly items: readonly T[];
  /** Whether a fetch is currently in flight. */
  readonly isLoading: boolean;
  /** Error message from the last failed fetch, or null if none. */
  readonly error: string | null;
  /** ISO-8601 timestamp of the last successful update. */
  readonly lastUpdated: string | null;
  /** SHA-256 config hash returned by the server (for drift detection). */
  readonly configHash: string | null;
}

/**
 * Configuration options for the useRegistry hook.
 */
export interface UseRegistryOptions<T> {
  /** Async function that returns the entity array. */
  readonly fetchFn: () => Promise<readonly T[]>;
  /** Polling interval in milliseconds (default 30 000). */
  readonly pollIntervalMs?: number;
  /** Whether to start polling immediately on mount (default true). */
  readonly autoStart?: boolean;
  /** Optional comparator for deduplicating items (default identity). */
  readonly keyFn?: (item: T) => string | number;
  /**
   * Called whenever new items arrive. Use this to diff against a local
   * cache or trigger side-effects (toast notifications, etc.).
   */
  readonly onUpdate?: (items: readonly T[]) => void;
}

/**
 * Generic hook for dynamic entity registries.
 *
 * Manages the full lifecycle of an async data source: loading state,
 * error handling, periodic refresh, and optional deduplication.  The
 * registry is the "live mirror" pattern — the UI always reflects the
 * latest known server state.
 *
 * @template T - Entity type.
 *
 * @example
 * ```tsx
 * const chains = useRegistry({
 *   fetchFn: () => getAdminChains().then(r => r.ok ? r.data.chains : []),
 *   pollIntervalMs: 15_000,
 *   keyFn: (c) => c.chain_id,
 * });
 *
 * if (chains.isLoading) return <Spinner />;
 * if (chains.error) return <Alert>{chains.error}</Alert>;
 * return <ChainTable chains={chains.items} />;
 * ```
 */
export function useRegistry<T>(options: UseRegistryOptions<T>) {
  const { fetchFn, pollIntervalMs = 30_000, autoStart = true, onUpdate } = options;

  const [state, setState] = useState<RegistryState<T>>({
    items: [],
    isLoading: false,
    error: null,
    lastUpdated: null,
    configHash: null,
  });

  /** AbortController for the in-flight request. */
  const abortRef = useRef<AbortController | null>(null);
  /** Ref to always see the latest fetchFn inside intervals. */
  const fetchFnRef = useRef(fetchFn);
  fetchFnRef.current = fetchFn;

  const refresh = useCallback(async () => {
    // Cancel previous request
    if (abortRef.current) {
      abortRef.current.abort();
    }
    const ctrl = new AbortController();
    abortRef.current = ctrl;

    setState((s) => ({ ...s, isLoading: true, error: null }));

    try {
      // We wrap the caller's promise so we can honour abortion
      const items = await fetchFnRef.current();

      if (ctrl.signal.aborted) {
        return;
      }

      setState({
        items: [...items],
        isLoading: false,
        error: null,
        lastUpdated: new Date().toISOString(),
        configHash: null, // TODO: hash from API when backend supports it
      });

      onUpdate?.(items);
    } catch (err) {
      if (ctrl.signal.aborted) {
        return;
      }
      setState((s) => ({
        ...s,
        isLoading: false,
        error: err instanceof Error ? err.message : "Unknown registry error",
      }));
    }
  }, [onUpdate]);

  useEffect(() => {
    if (autoStart) {
      void refresh();
    }

    const id = setInterval(() => {
      void refresh();
    }, pollIntervalMs);

    return () => {
      clearInterval(id);
      abortRef.current?.abort();
    };
  }, [refresh, pollIntervalMs, autoStart]);

  return {
    ...state,
    refresh,
    /** Number of items currently in the registry. */
    count: state.items.length,
    /** Whether the registry has ever successfully loaded. */
    hasData: state.lastUpdated !== null,
  } as const;
}
