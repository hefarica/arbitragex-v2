// RPC catalog sync panel for /rpcs — the rpc_endpoints registry (distinct from
// the legacy RPC-health table below). Excel = SSOT, one-way:
//   • "Importar catálogo RPC" — upload the *RPC Providers* sheet exported as CSV
//     (NEVER the secrets workbook). Parsed CLIENT-SIDE → JSON {rows} → POST the
//     admin import endpoint (idempotent upsert + per-chain hot-reload).
//   • "Recargar" — POST a bare reload signal (arbx:config:rpcs) + refresh status.
// Source-of-truth note: this updates the registry/UI layer (hot-reload, no
// restart); the searcher hot-path reads RPC from .env (updates on next deploy).
"use client";

import React, { useCallback, useEffect, useRef, useState } from "react";
import { RefreshCw, UploadCloud, KeyRound } from "lucide-react";
import { getApiBaseUrl } from "@/lib/api-client";
import { hasAdminSession } from "@/lib/admin-token";

// Mainnet chain-name → chainId. Mirrors gen_rpc_env_from_xlsx.py — universal
// protocol constants (chainlist.org), not operator config. Testnets are skipped
// (they use dedicated *_RPC_URL env vars, not the multi-provider registry).
const CHAIN_IDS: Record<string, number> = {
  "Ethereum Mainnet": 1,
  "Optimism": 10,
  "BSC Mainnet": 56,
  "Gnosis": 100,
  "Polygon Mainnet": 137,
  "Base": 8453,
  "Arbitrum One": 42161,
  "Avalanche": 43114,
  "Linea": 59144,
  "Blast": 81457,
  "Scroll": 534352,
};

type ChainStat = {
  chain_id: number;
  http: number;
  ws: number;
  enabled: number;
  total: number;
  last_updated: string | null;
};
type Status = { chains: ChainStat[]; chain_count: number; total: number; generated_at: string };

function transportFromUrl(u: string): "http" | "https" | "ws" | "wss" | null {
  const s = u.trim().toLowerCase();
  if (s.startsWith("wss://")) return "wss";
  if (s.startsWith("ws://")) return "ws";
  if (s.startsWith("https://")) return "https";
  if (s.startsWith("http://")) return "http";
  return null;
}

// Minimal CSV parse for the RPC Providers export (Chain · Protocolo · Proveedor ·
// URL; no embedded commas in this data). Maps Chain→chain_id and derives transport
// from the URL scheme. Returns rows ready for the import endpoint + skip telemetry.
function parseCatalogCsv(text: string): {
  rows: Array<{ chain_id: number; url: string; transport: string }>;
  skipped: number;
  unmapped: string[];
} {
  const lines = text.split(/\r?\n/).filter((l) => l.trim().length > 0);
  if (lines.length === 0) return { rows: [], skipped: 0, unmapped: [] };
  const cellsOf = (line: string) => line.split(",").map((c) => c.trim().replace(/^"|"$/g, ""));
  const header = cellsOf(lines[0]!).map((h) => h.toLowerCase());
  const ci = header.indexOf("chain");
  const ui = header.indexOf("url");
  const hasHeader = ci >= 0 && ui >= 0;
  const chainCol = ci >= 0 ? ci : 0;
  const urlCol = ui >= 0 ? ui : 3;
  const rows: Array<{ chain_id: number; url: string; transport: string }> = [];
  const unmapped: string[] = [];
  let skipped = 0;
  for (let i = hasHeader ? 1 : 0; i < lines.length; i++) {
    const cells = cellsOf(lines[i]!);
    const chainName = cells[chainCol] ?? "";
    const url = cells[urlCol] ?? "";
    const chain_id = CHAIN_IDS[chainName];
    const transport = url ? transportFromUrl(url) : null;
    if (!chain_id || !url || !transport) {
      skipped++;
      if (chainName && !chain_id) unmapped.push(chainName);
      continue;
    }
    rows.push({ chain_id, url: url.replace(/\/+$/, ""), transport });
  }
  return { rows, skipped, unmapped };
}

export function RpcSyncPanel() {
  const [status, setStatus] = useState<Status | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState<null | "reload" | "import">(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [isAdmin, setIsAdmin] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const loadStatus = useCallback(async () => {
    setErr(null);
    try {
      const res = await fetch(`${getApiBaseUrl()}/api/rpc/status`, { credentials: "include", cache: "no-store" });
      if (!res.ok) {
        setErr(`registry unreachable (HTTP ${res.status})`);
        return;
      }
      setStatus((await res.json()) as Status);
    } catch (e) {
      setErr((e as Error).message);
    }
  }, []);

  useEffect(() => {
    setIsAdmin(hasAdminSession());
    void loadStatus();
  }, [loadStatus]);

  const reload = useCallback(async () => {
    setBusy("reload");
    setMsg(null);
    try {
      const res = await fetch(`${getApiBaseUrl()}/api/admin/rpcs/reload`, { method: "POST", credentials: "include" });
      if (!res.ok) setMsg(`Reload failed: HTTP ${res.status}${res.status === 401 ? " — sign in at /admin/signin" : ""}`);
      else {
        const j = (await res.json()) as { event_id?: string };
        setMsg(`Reload signal published${j.event_id ? ` (event ${j.event_id.slice(0, 8)}…)` : ""}.`);
      }
    } catch (e) {
      setMsg(`Reload error: ${(e as Error).message}`);
    } finally {
      setBusy(null);
    }
  }, []);

  const onFile = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      e.target.value = ""; // allow re-selecting the same file
      if (!file) return;
      setBusy("import");
      setMsg(null);
      try {
        const text = await file.text();
        const { rows, skipped, unmapped } = parseCatalogCsv(text);
        if (rows.length === 0) {
          setMsg(`No mappable rows (skipped ${skipped}). Expected columns: Chain, Protocolo, Proveedor, URL.`);
          return;
        }
        const res = await fetch(`${getApiBaseUrl()}/api/admin/rpcs/import`, {
          method: "POST",
          credentials: "include",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ rows }),
        });
        if (!res.ok) {
          setMsg(`Import failed: HTTP ${res.status}${res.status === 401 ? " — sign in at /admin/signin" : ""}`);
          return;
        }
        const j = (await res.json()) as {
          inserted: number;
          updated: number;
          unchanged: number;
          reloaded_chains?: number[];
        };
        const skipNote = skipped
          ? ` · skipped ${skipped}${unmapped.length ? ` (${[...new Set(unmapped)].slice(0, 4).join(", ")}…)` : ""}`
          : "";
        setMsg(
          `Imported ${rows.length}: +${j.inserted} new, ~${j.updated} updated, ${j.unchanged} unchanged · reloaded ${j.reloaded_chains?.length ?? 0} chain(s)${skipNote}`,
        );
        await loadStatus();
      } catch (e) {
        setMsg(`Import error: ${(e as Error).message}`);
      } finally {
        setBusy(null);
      }
    },
    [loadStatus],
  );

  return (
    <div data-slot="card" className="bg-card text-card-foreground border border-border rounded-xl shadow-2xl p-5 space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">Catalog registry (rpc_endpoints)</h2>
          <p className="text-xs text-muted-foreground mt-0.5">
            Multi-provider RPC registry, synced from the <span className="font-mono">RPC Providers</span> catalog.
            Hot-reload via <span className="font-mono">arbx:config:rpcs</span> (no restart). The searcher hot-path
            reads RPC from <span className="font-mono">.env</span>; that updates on the next deploy.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={reload}
            disabled={busy !== null}
            title="Publish a reload signal so consumers re-pull the registry"
            className="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted px-3 py-1.5 text-sm font-medium hover:bg-accent disabled:opacity-50"
          >
            <RefreshCw size={14} className={busy === "reload" ? "animate-spin" : ""} />
            Recargar
          </button>
          <button
            type="button"
            onClick={() => fileRef.current?.click()}
            disabled={busy !== null}
            title="Upload the RPC Providers sheet exported as CSV (never the secrets workbook)"
            className="inline-flex items-center gap-1.5 rounded-md border border-primary/40 bg-primary/10 px-3 py-1.5 text-sm font-medium text-primary hover:bg-primary/20 disabled:opacity-50"
          >
            <UploadCloud size={14} className={busy === "import" ? "animate-pulse" : ""} />
            Importar catálogo RPC
          </button>
          <input ref={fileRef} type="file" accept=".csv,text/csv" onChange={onFile} className="hidden" />
        </div>
      </div>

      {!isAdmin && (
        <div className="flex items-center gap-2 rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-xs text-warning">
          <KeyRound size={14} />
          Admin session required for Recargar / Importar.{" "}
          <a href="/admin/signin?next=/rpcs" className="font-semibold underline">
            Sign in
          </a>
        </div>
      )}

      {err ? (
        <p className="text-sm text-destructive">Status: {err}</p>
      ) : !status ? (
        <p className="text-sm text-muted-foreground">Loading registry status…</p>
      ) : status.chains.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          Registry empty — import the catalog to populate <span className="font-mono">rpc_endpoints</span>.
        </p>
      ) : (
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
          {status.chains.map((c) => (
            <div key={c.chain_id} className="rounded-md border border-border bg-muted/30 px-3 py-2 text-sm">
              <div className="font-mono font-semibold">chain {c.chain_id}</div>
              <div className="text-xs text-muted-foreground">
                {c.http} http · {c.ws} ws · {c.enabled}/{c.total} enabled
              </div>
            </div>
          ))}
          <div className="col-span-full text-xs text-muted-foreground">
            {status.chain_count} chain(s) · {status.total} endpoint(s) total
          </div>
        </div>
      )}

      {msg && <p className="text-sm font-medium text-foreground">{msg}</p>}
    </div>
  );
}
