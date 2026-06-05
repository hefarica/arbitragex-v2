"use client";
// Idea 1 (Phase-1) — load + save the per-chain cartridge filter preference.
//
// GET  /api/cartridge-filters?chain_id=N  (public read; configured:false when unset)
// PUT  /admin/cartridge-filters/N         (admin upsert; auth via httpOnly session cookie
//                                          translated by the edge adminProxy -> upstream token)
//
// R8 fail-honest: surfaces upstream error/note verbatim; never fabricates a saved state.

import { useCallback, useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import type { CartridgeFilterState, CartridgeFilterInput } from "./cartridge-filter-types";

export type FilterStatus = "loading" | "idle" | "saving" | "saved" | "error";

export interface UseCartridgeFiltersResult {
  filter: CartridgeFilterState | null;
  configured: boolean;
  status: FilterStatus;
  error: string | null;
  /** Backend honesty note (e.g. "not yet consumed by searcher-rs"). */
  note: string | null;
  reload: () => void;
  save: (next: CartridgeFilterInput, actor: string) => Promise<void>;
}

const FETCH_TIMEOUT_MS = 8000;

type RawFilter = Partial<CartridgeFilterState> & {
  ok?: boolean;
  configured?: boolean;
  error?: string;
  note?: string;
};

function coerce(chainId: number, body: RawFilter): CartridgeFilterState {
  return {
    chain_id: chainId,
    enabled_strategies: Array.isArray(body.enabled_strategies) ? body.enabled_strategies : [],
    min_profit_usd: typeof body.min_profit_usd === "number" ? body.min_profit_usd : 0,
    max_slippage_bps: typeof body.max_slippage_bps === "number" ? body.max_slippage_bps : 10_000,
    custom_cartridge_b64: typeof body.custom_cartridge_b64 === "string" ? body.custom_cartridge_b64 : null,
    enabled: typeof body.enabled === "boolean" ? body.enabled : true,
    ...(typeof body.updated_at === "string" ? { updated_at: body.updated_at } : {}),
    ...(typeof body.updated_by === "string" ? { updated_by: body.updated_by } : {}),
  };
}

export function useCartridgeFilters(chainId: number): UseCartridgeFiltersResult {
  const [filter, setFilter] = useState<CartridgeFilterState | null>(null);
  const [configured, setConfigured] = useState(false);
  const [status, setStatus] = useState<FilterStatus>("loading");
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const reload = useCallback(() => {
    if (typeof window === "undefined") return;
    const base = getApiBaseUrl();
    const ctrl = new AbortController();
    const t = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
    setStatus("loading");
    void fetch(`${base}/api/cartridge-filters?chain_id=${chainId}`, {
      credentials: "include",
      signal: ctrl.signal,
    })
      .then(async (r) => {
        const body = (await r.json()) as RawFilter;
        if (!r.ok || body?.ok === false) {
          setError(body?.error ?? `http_${r.status}`);
          setStatus("error");
          return;
        }
        if (body.configured) {
          setFilter(coerce(chainId, body));
          setConfigured(true);
        } else {
          setFilter(null);
          setConfigured(false);
        }
        setError(null);
        setStatus("idle");
      })
      .catch((e: unknown) => {
        setError((e as Error).name === "AbortError" ? "timeout" : (e as Error).message);
        setStatus("error");
      })
      .finally(() => clearTimeout(t));
  }, [chainId]);

  useEffect(() => {
    reload();
  }, [reload]);

  const save = useCallback(
    async (next: CartridgeFilterInput, actor: string) => {
      if (typeof window === "undefined") return;
      const base = getApiBaseUrl();
      setStatus("saving");
      setError(null);
      setNote(null);
      try {
        const r = await fetch(`${base}/admin/cartridge-filters/${chainId}`, {
          method: "PUT",
          credentials: "include",
          headers: { "content-type": "application/json", "x-arbx-actor": actor },
          body: JSON.stringify(next),
        });
        const body = (await r.json()) as RawFilter;
        if (!r.ok || body?.ok === false) {
          setError(body?.error ?? `http_${r.status}`);
          setStatus("error");
          return;
        }
        setFilter(coerce(chainId, body));
        setConfigured(true);
        setNote(typeof body.note === "string" ? body.note : null);
        setStatus("saved");
      } catch (e: unknown) {
        setError((e as Error).message);
        setStatus("error");
      }
    },
    [chainId],
  );

  return { filter, configured, status, error, note, reload, save };
}
