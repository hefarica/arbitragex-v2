/**
 * B1.b — ChainsAdminClient (orchestrator)
 *
 * Renders the operator admin console for `chains_runtime`. Receives the
 * server-side snapshot (which may be an auth error when the operator has
 * no admin session at SSR time) and refreshes on mount using the httpOnly
 * admin cookie that the browser carries via credentials: "include".
 *
 * Mounted Snapshot R1: initial state derives only from the server-provided
 * snapshot. Cookie/localStorage reads happen exclusively in useEffect to
 * keep the SSR markup deterministic.
 *
 * R8 fail-honest: when the list endpoint fails we render the verbatim
 * error and the empty state — we never fabricate rows. When a row's
 * `enabled` is null we surface "Unknown" rather than guess.
 */
"use client";

import * as React from "react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { AlertCircle, KeyRound, Plus, RefreshCw, ToggleLeft, ToggleRight, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { getAdminToken, hasAdminSession } from "@/lib/admin-token";
import { deleteAdminChain, getAdminChains } from "@/lib/api-client";
import type { AdminChainRow } from "@/lib/schemas";

import { ChainStatusBadge } from "./ChainStatusBadge";
import { CreateChainDialog } from "./CreateChainDialog";
import { EditChainDialog } from "./EditChainDialog";
import { ProbeButton } from "./ProbeButton";

export interface ChainsAdminSnapshot {
  rows: AdminChainRow[];
  source: "server-snapshot" | "server-fetch-failed" | "server-auth-required" | "endpoint-not-implemented";
  error?: string;
}

interface Props {
  initialSnapshot: ChainsAdminSnapshot;
}

type Filter = "all" | "active" | "disabled";

function redactRpcUrl(url: string): string {
  // Strip trailing API key path segment so screenshots do not leak the
  // Alchemy/Infura secret. We keep host + first path segment for context.
  try {
    const u = new URL(url);
    const segments = u.pathname.split("/").filter(Boolean);
    if (segments.length === 0) return `${u.protocol}//${u.host}`;
    const head = segments.slice(0, Math.min(1, segments.length)).join("/");
    const masked = segments.length > 1 ? `${head}/…` : head;
    return `${u.protocol}//${u.host}/${masked}`;
  } catch {
    return url.length > 40 ? `${url.slice(0, 40)}…` : url;
  }
}

function fmtTimestamp(iso: string): string {
  // ISO already; we just trim millis + Z for readability. Locale formatting
  // is deferred (would break SSR consistency without mounted guard).
  return iso.replace(/\.\d+Z$/, "Z").replace("T", " ");
}

export function ChainsAdminClient({ initialSnapshot }: Props) {
  const [rows, setRows] = useState<AdminChainRow[]>(initialSnapshot.rows);
  const [snapshotError, setSnapshotError] = useState<string | null>(
    initialSnapshot.source === "server-snapshot" ? null : initialSnapshot.error ?? null,
  );
  const [refreshing, setRefreshing] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const [isMounted, setIsMounted] = useState(false);
  const [adminToken, setAdminToken] = useState("");
  const [actor, setActor] = useState("operator");
  const [hasSession, setHasSession] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [editTarget, setEditTarget] = useState<AdminChainRow | null>(null);
  const [disableTarget, setDisableTarget] = useState<AdminChainRow | null>(null);
  const [disabling, setDisabling] = useState(false);
  const [disableError, setDisableError] = useState<string | null>(null);

  useEffect(() => {
    setIsMounted(true);
    setAdminToken(getAdminToken());
    setHasSession(hasAdminSession());
    if (typeof window !== "undefined") {
      const stored = window.localStorage.getItem("arbx-actor");
      if (stored) setActor(stored);
    }
    // Refresh session presence every 30s so the banner reflects expiry.
    const id = setInterval(() => {
      setAdminToken(getAdminToken());
      setHasSession(hasAdminSession());
    }, 30_000);
    return () => clearInterval(id);
  }, []);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    const res = await getAdminChains();
    setRefreshing(false);
    if (!res.ok) {
      setSnapshotError(res.error);
      return;
    }
    setSnapshotError(null);
    setRows(res.data.items);
  }, []);

  // After mount, if the server snapshot was an auth error and we DO have
  // a session, re-fetch immediately so the operator sees rows without
  // having to click refresh.
  useEffect(() => {
    if (!isMounted) return;
    if (initialSnapshot.source === "server-snapshot") return;
    if (!hasSession) return;
    void refresh();
  }, [isMounted, hasSession, initialSnapshot.source, refresh]);

  const upsertRow = useCallback((next: AdminChainRow) => {
    setRows((prev) => {
      const idx = prev.findIndex((r) => r.chain_id === next.chain_id);
      if (idx === -1) return [next, ...prev];
      const out = prev.slice();
      out[idx] = next;
      return out;
    });
  }, []);

  const confirmDisable = useCallback(async () => {
    if (!disableTarget) return;
    setDisableError(null);
    setDisabling(true);
    const res = await deleteAdminChain(disableTarget.chain_id, adminToken, actor);
    setDisabling(false);
    if (!res.ok) {
      setDisableError(res.error);
      return;
    }
    upsertRow(res.data);
    setDisableTarget(null);
  }, [disableTarget, adminToken, actor, upsertRow]);

  const filtered = useMemo(() => {
    const sorted = [...rows].sort((a, b) => a.chain_id - b.chain_id);
    if (filter === "active") return sorted.filter((r) => r.enabled);
    if (filter === "disabled") return sorted.filter((r) => !r.enabled);
    return sorted;
  }, [rows, filter]);

  const counts = useMemo(
    () => ({
      all: rows.length,
      active: rows.filter((r) => r.enabled).length,
      disabled: rows.filter((r) => !r.enabled).length,
    }),
    [rows],
  );

  return (
    <div className="p-8 space-y-6">
      <header className="flex flex-wrap items-start justify-between gap-3 border-b border-border pb-4">
        <div>
          <h1 className="text-3xl font-extrabold tracking-tight text-foreground">
            Chains Admin
          </h1>
          <p className="text-sm text-muted-foreground mt-1 max-w-2xl">
            Operator console for <code className="font-mono">chains_runtime</code>. Mutations are admin-gated
            (V-AT-1 cookie), audited, and publish <code className="font-mono">arbx:config:chains:reload</code>
            so the searcher hot-reloads in ≤1s when B1.c lands.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            size="sm"
            onClick={() => setCreateOpen(true)}
            disabled={!hasSession}
            className="gap-1"
            title={hasSession ? "Register a new chain" : "Admin session required"}
          >
            <Plus className="size-3.5" /> Add chain
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={refresh}
            disabled={refreshing}
            className="gap-1"
          >
            <RefreshCw className={`size-3.5 ${refreshing ? "animate-spin" : ""}`} /> Refresh
          </Button>
        </div>
      </header>

      {isMounted && !hasSession && (
        <div className="flex items-start gap-2 rounded-xl border border-warning/40 bg-warning/10 p-3 text-sm text-warning">
          <KeyRound className="mt-0.5 size-4 shrink-0" />
          <div>
            <p className="font-semibold">Admin session required</p>
            <p className="text-xs mt-0.5">Unlock the operator session at <code className="font-mono">/killswitch</code> to enable mutations.</p>
          </div>
        </div>
      )}

      {snapshotError && (
        <div className="flex items-start gap-2 rounded-xl border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
          <AlertCircle className="mt-0.5 size-4 shrink-0" />
          <div>
            <p className="font-semibold">Admin chains endpoint error</p>
            <p className="text-xs mt-0.5 font-mono break-all">{snapshotError}</p>
          </div>
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        {(["all", "active", "disabled"] as const).map((f) => (
          <button
            key={f}
            type="button"
            onClick={() => setFilter(f)}
            className={`px-3 py-1 rounded-full text-xs font-semibold border transition-colors ${
              filter === f
                ? "bg-primary text-primary-foreground border-primary"
                : "border-border text-muted-foreground hover:text-foreground hover:border-foreground/30"
            }`}
          >
            {f === "all" ? "All" : f === "active" ? "Active" : "Disabled"} ({counts[f]})
          </button>
        ))}
      </div>

      {filtered.length === 0 && !snapshotError && (
        <div className="flex flex-col items-center justify-center py-16 text-muted-foreground gap-2">
          <p className="text-sm">No chains in <code className="font-mono">chains_runtime</code>.</p>
          <p className="text-xs">
            Click <strong>Add chain</strong> to register the first one (admin session required).
          </p>
        </div>
      )}

      {filtered.length > 0 && (
        <div className="rounded-2xl border border-border overflow-hidden shadow-sm">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-muted text-muted-foreground text-xs uppercase tracking-wider">
                <th className="px-4 py-3 border-b border-border">Chain</th>
                <th className="px-4 py-3 border-b border-border">RPC HTTP</th>
                <th className="px-4 py-3 border-b border-border text-right">Block time</th>
                <th className="px-4 py-3 border-b border-border text-center">Status</th>
                <th className="px-4 py-3 border-b border-border">Probe</th>
                <th className="px-4 py-3 border-b border-border">Last update</th>
                <th className="px-4 py-3 border-b border-border text-center">Actions</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((r) => (
                <tr key={r.chain_id} className="border-b border-border/50 hover:bg-muted/40 transition-colors">
                  <td className="px-4 py-3 align-top">
                    <div className="font-semibold text-sm">{r.name}</div>
                    <div className="text-xs text-muted-foreground font-mono">
                      <Badge variant="outline" className="text-[10px]">id {r.chain_id}</Badge>
                      <span className="ml-2">{r.native_currency}</span>
                    </div>
                  </td>
                  <td className="px-4 py-3 align-top">
                    <div className="font-mono text-xs break-all">{redactRpcUrl(r.rpc_http_url)}</div>
                    {r.rpc_ws_url && (
                      <div className="font-mono text-[10px] text-muted-foreground break-all mt-1">
                        ws: {redactRpcUrl(r.rpc_ws_url)}
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-3 align-top text-right font-mono text-xs">{r.block_time_ms}ms</td>
                  <td className="px-4 py-3 align-top text-center">
                    <ChainStatusBadge enabled={r.enabled} />
                  </td>
                  <td className="px-4 py-3 align-top">
                    <ProbeButton chainId={r.chain_id} adminToken={adminToken} />
                  </td>
                  <td className="px-4 py-3 align-top text-xs text-muted-foreground">
                    {/* Avoid Date.toLocaleString during SSR — render only after mount */}
                    {isMounted ? fmtTimestamp(r.updated_at) : "…"}
                    {r.updated_by && (
                      <div className="text-[10px] mt-0.5">by {r.updated_by}</div>
                    )}
                  </td>
                  <td className="px-4 py-3 align-top text-center">
                    <div className="flex items-center justify-center gap-1">
                      <Button
                        type="button"
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2 gap-1"
                        onClick={() => setEditTarget(r)}
                        disabled={!hasSession}
                        title={hasSession ? "Edit chain" : "Admin session required"}
                      >
                        Edit
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2 gap-1"
                        onClick={() => {
                          setDisableError(null);
                          setDisableTarget(r);
                        }}
                        disabled={!hasSession || !r.enabled}
                        title={!r.enabled ? "Already disabled" : "Soft-disable this chain"}
                      >
                        {r.enabled ? <ToggleRight className="size-3.5 text-success" /> : <ToggleLeft className="size-3.5" />}
                        {r.enabled ? "Disable" : "Disabled"}
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {createOpen && (
        <CreateChainDialog
          adminToken={adminToken}
          actor={actor}
          onClose={() => setCreateOpen(false)}
          onCreated={(row) => {
            upsertRow(row);
            setCreateOpen(false);
          }}
        />
      )}

      {editTarget && (
        <EditChainDialog
          row={editTarget}
          adminToken={adminToken}
          actor={actor}
          onClose={() => setEditTarget(null)}
          onSaved={(row) => {
            upsertRow(row);
            setEditTarget(null);
          }}
        />
      )}

      {disableTarget && (
        <ConfirmDisable
          row={disableTarget}
          busy={disabling}
          errorMsg={disableError}
          onCancel={() => {
            setDisableTarget(null);
            setDisableError(null);
          }}
          onConfirm={confirmDisable}
        />
      )}
    </div>
  );
}

interface ConfirmDisableProps {
  row: AdminChainRow;
  busy: boolean;
  errorMsg: string | null;
  onCancel: () => void;
  onConfirm: () => void;
}

function ConfirmDisable({ row, busy, errorMsg, onCancel, onConfirm }: ConfirmDisableProps) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
    >
      <div className="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-md">
        <div className="flex items-center justify-between px-5 py-3 border-b border-border">
          <h2 className="text-lg font-bold flex items-center gap-2">
            <Trash2 className="size-4 text-destructive" /> Disable chain
          </h2>
          <button type="button" onClick={onCancel} className="text-muted-foreground hover:text-foreground" aria-label="Close">
            <X className="size-4" />
          </button>
        </div>
        <div className="p-5 space-y-3 text-sm">
          <p>
            Soft-disable <strong className="font-mono">{row.name}</strong> (chain {row.chain_id})?
          </p>
          <p className="text-xs text-muted-foreground">
            The row stays in <code className="font-mono">chains_runtime</code> with{" "}
            <code className="font-mono">enabled=false</code>. The searcher will abort this chain's
            scanner via the reload pub/sub. Re-enable any time from the edit dialog.
          </p>
          {errorMsg && (
            <div className="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span className="font-mono break-all">{errorMsg}</span>
            </div>
          )}
        </div>
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border bg-muted/30">
          <Button type="button" variant="ghost" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button type="button" variant="destructive" onClick={onConfirm} disabled={busy}>
            {busy ? "Disabling…" : "Disable"}
          </Button>
        </div>
      </div>
    </div>
  );
}
