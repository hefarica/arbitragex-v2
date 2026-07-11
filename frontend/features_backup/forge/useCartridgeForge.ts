"use client";
// Idea 2 — list + inject + lifecycle for Strategy Forge cartridges.
//
// GET    /api/cartridges                 (public list; counters)
// POST   /admin/cartridges               (admin inject — publishes hot-load to searcher-rs)
// POST   /admin/cartridges/:slug/pause   (admin)
// POST   /admin/cartridges/:slug/resume  (admin)
// DELETE /admin/cartridges/:slug         (admin)
//
// Admin auth via the httpOnly session cookie (credentials:include), translated by the edge.
// R8 fail-honest: surfaces upstream errors verbatim; never fabricates a cartridge or a result.

import { useCallback, useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import type { CartridgeSummary, InjectCartridgeInput } from "./cartridge-forge-types";

const POLL_MS = 10_000;
const FETCH_TIMEOUT_MS = 9000;

export type ForgeStatus = "loading" | "idle" | "working" | "error";

export interface MutationResult {
  ok: boolean;
  error?: string;
  message?: string;
}

export interface UseCartridgeForgeResult {
  cartridges: CartridgeSummary[];
  status: ForgeStatus;
  error: string | null;
  reload: () => void;
  inject: (input: InjectCartridgeInput, actor: string) => Promise<MutationResult>;
  pause: (slug: string, actor: string) => Promise<MutationResult>;
  resume: (slug: string, actor: string) => Promise<MutationResult>;
  remove: (slug: string, actor: string) => Promise<MutationResult>;
}

function toNum(v: unknown): number {
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (typeof v === "string" && v.trim() !== "") {
    const n = Number(v);
    return Number.isFinite(n) ? n : 0;
  }
  return 0;
}

function coerce(row: Record<string, unknown>): CartridgeSummary {
  return {
    id: String(row["id"] ?? ""),
    slug: String(row["slug"] ?? ""),
    name: String(row["name"] ?? ""),
    version: String(row["version"] ?? ""),
    author: String(row["author"] ?? ""),
    description: String(row["description"] ?? ""),
    category: String(row["category"] ?? ""),
    target_chains: Array.isArray(row["target_chains"]) ? (row["target_chains"] as number[]) : [],
    state: String(row["state"] ?? "unknown"),
    min_eval_interval_ms: toNum(row["min_eval_interval_ms"]),
    total_evaluations: toNum(row["total_evaluations"]),
    total_opportunities: toNum(row["total_opportunities"]),
    total_errors: toNum(row["total_errors"]),
    created_at: String(row["created_at"] ?? ""),
    ...(row["updated_at"] != null ? { updated_at: String(row["updated_at"]) } : {}),
    ...(row["last_evaluation_at"] != null ? { last_evaluation_at: String(row["last_evaluation_at"]) } : {}),
  };
}

export function useCartridgeForge(): UseCartridgeForgeResult {
  const [cartridges, setCartridges] = useState<CartridgeSummary[]>([]);
  const [status, setStatus] = useState<ForgeStatus>("loading");
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    if (typeof window === "undefined") return;
    const base = getApiBaseUrl();
    const ctrl = new AbortController();
    const t = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
    void fetch(`${base}/api/cartridges`, { credentials: "include", signal: ctrl.signal })
      .then(async (r) => {
        const body = (await r.json()) as { cartridges?: unknown; error?: string };
        if (!r.ok) {
          setError(typeof body?.error === "string" ? body.error : `http_${r.status}`);
          setStatus("error");
          return;
        }
        const rows = Array.isArray(body.cartridges) ? (body.cartridges as Array<Record<string, unknown>>) : [];
        setCartridges(rows.map(coerce));
        setError(null);
        setStatus("idle");
      })
      .catch((e: unknown) => {
        setError((e as Error).name === "AbortError" ? "timeout" : (e as Error).message);
        setStatus("error");
      })
      .finally(() => clearTimeout(t));
  }, []);

  useEffect(() => {
    reload();
    const id = setInterval(reload, POLL_MS);
    return () => clearInterval(id);
  }, [reload]);

  const mutate = useCallback(
    async (path: string, method: string, actor: string, jsonBody?: unknown): Promise<MutationResult> => {
      if (typeof window === "undefined") return { ok: false, error: "no_window" };
      const base = getApiBaseUrl();
      setStatus("working");
      try {
        const r = await fetch(`${base}${path}`, {
          method,
          credentials: "include",
          headers: { "content-type": "application/json", "x-arbx-actor": actor },
          ...(jsonBody !== undefined ? { body: JSON.stringify(jsonBody) } : {}),
        });
        const body = (await r.json().catch(() => ({}))) as { error?: string; message?: string };
        setStatus("idle");
        if (!r.ok) {
          return { ok: false, error: typeof body?.error === "string" ? body.error : `http_${r.status}` };
        }
        reload();
        return { ok: true, ...(typeof body?.message === "string" ? { message: body.message } : {}) };
      } catch (e: unknown) {
        setStatus("idle");
        return { ok: false, error: (e as Error).message };
      }
    },
    [reload],
  );

  const inject = useCallback(
    (input: InjectCartridgeInput, actor: string) =>
      mutate("/admin/cartridges", "POST", actor, input),
    [mutate],
  );
  const pause = useCallback(
    (slug: string, actor: string) => mutate(`/admin/cartridges/${slug}/pause`, "POST", actor),
    [mutate],
  );
  const resume = useCallback(
    (slug: string, actor: string) => mutate(`/admin/cartridges/${slug}/resume`, "POST", actor),
    [mutate],
  );
  const remove = useCallback(
    (slug: string, actor: string) => mutate(`/admin/cartridges/${slug}`, "DELETE", actor),
    [mutate],
  );

  return { cartridges, status, error, reload, inject, pause, resume, remove };
}
