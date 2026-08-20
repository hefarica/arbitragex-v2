"use client";

/**
 * useRpcBackend — per-service RPC backend toggle state (alloy FASE 4).
 *
 * Reads the ethers/alloy/shadow selection from GET /api/admin/rpc-backend and
 * polls every 30s so external changes surface. Same pattern as usePaperModeState:
 * AbortController cancels superseded/unmounted fetches, responses are
 * schema-validated, and any error reverts `data` to DEFAULT_RPC_BACKEND_STATE
 * (updated_at=null → the UI gates changes on a live read).
 *
 * setBackend issues the admin-gated PUT (service or "all") and refetches the
 * canonical state afterwards — the component only renders and toasts.
 *
 * This is a control-plane toggle (RPC implementation track). It never touches
 * trading mode, capital, or broadcast gates (§34 mode-invariant).
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import { getAdminToken } from "@/lib/admin-token";
import {
  RpcBackendStateSchema,
  DEFAULT_RPC_BACKEND_STATE,
  type RpcBackendKind,
  type RpcBackendState,
} from "@/lib/schemas";

export interface UseRpcBackendResult {
  data: RpcBackendState;
  isLoading: boolean;
  isRefreshing: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
  /** PUT {service, backend} → throws on non-2xx; state refetched on success. */
  setBackend: (service: string, backend: RpcBackendKind) => Promise<void>;
}

const POLL_MS = 30_000;

async function fetchRpcBackendState(signal: AbortSignal): Promise<RpcBackendState> {
  const res = await fetch(`${getApiBaseUrl()}/api/admin/rpc-backend`, {
    signal,
    credentials: "include",
    headers: {
      accept: "application/json",
      "x-arbx-admin-token": getAdminToken() || "",
    },
  });

  if (!res.ok) {
    throw new Error(`HTTP ${res.status}`);
  }

  const parsed = await res.json();
  const result = RpcBackendStateSchema.safeParse(parsed);
  if (!result.success) {
    throw new Error(
      `schema: ${result.error.issues
        .map((i) => `${i.path.join(".") || "<root>"}: ${i.message}`)
        .join("; ")}`,
    );
  }
  return result.data;
}

export function useRpcBackend(): UseRpcBackendResult {
  const [data, setData] = useState<RpcBackendState>(DEFAULT_RPC_BACKEND_STATE);
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
      const result = await fetchRpcBackendState(ctrl.signal);
      setData(result);
      setError(null);
    } catch (e) {
      const err = e as Error;
      if (err.name === "AbortError") {
        // Race-abort from unmount or superseded fetch — state untouched.
        return;
      }
      setError(err);
      setData(DEFAULT_RPC_BACKEND_STATE);
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
  }, []);

  const setBackend = useCallback(
    async (service: string, backend: RpcBackendKind) => {
      const res = await fetch(`${getApiBaseUrl()}/api/admin/rpc-backend`, {
        method: "PUT",
        credentials: "include",
        headers: {
          "content-type": "application/json",
          "x-arbx-admin-token": getAdminToken() || "",
        },
        body: JSON.stringify({ service, backend }),
      });
      if (!res.ok) {
        let detail = "";
        try {
          const body = (await res.json()) as { error?: string; detail?: string };
          detail = body?.error ? ` (${body.error})` : "";
        } catch {
          // Non-JSON error body — the status code is enough.
        }
        throw new Error(`HTTP ${res.status}${detail}`);
      }
      await refetch();
    },
    [refetch],
  );

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

  return { data, isLoading, isRefreshing, error, refetch, setBackend };
}
