"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { AlertCircle, RefreshCw, ToggleLeft, ToggleRight, Database, Plus, Trash2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { DexDetailDialog } from "@/components/DexDetailDialog";
import { useChains, type ChainInfo } from "@/lib/chains";
import {
  createDex,
  deleteDex,
  getDexes,
  toggleDexActive,
  type CreateDexFactory,
  type DexRow,
} from "@/lib/api/dexes";

// Protocol types known by the spine's `default_fee_bps_for_adapter` lookup.
// Free-text is allowed in the form so the operator can register a new protocol
// adapter ahead of the Rust side, but the suggestions cover today's universe.
const KNOWN_PROTOCOLS = ["UNISWAP_V2", "UNISWAP_V3", "CURVE", "BALANCER", "SUSHISWAP"] as const;

export interface DexRegistrySnapshot {
  dexes: DexRow[];
  source: string;
}

interface Props {
  initialSnapshot: DexRegistrySnapshot;
}

const EDGE_URL =
  process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";

function EndpointNotice({ message }: { message: string }) {
  return (
    <div className="flex items-start gap-3 rounded-xl border border-warning/40 bg-warning/10 p-4 text-sm text-warning">
      <AlertCircle className="mt-0.5 size-4 shrink-0" />
      <div>
        <p className="font-semibold">Endpoint not available</p>
        <p className="text-xs mt-0.5 font-mono">{message}</p>
      </div>
    </div>
  );
}

function fmtUsd(v: number | null): string {
  if (v == null) return "—";
  if (v >= 1_000_000_000) return `$${(v / 1_000_000_000).toFixed(1)}B`;
  if (v >= 1_000_000) return `$${(v / 1_000_000).toFixed(1)}M`;
  if (v >= 1_000) return `$${(v / 1_000).toFixed(1)}K`;
  return `$${v.toFixed(2)}`;
}

const ALL_CHAINS = "all";

export default function DexRegistryClient({ initialSnapshot }: Props) {
  // R1: useState initialised from server-provided snapshot
  const [dexes, setDexes] = useState<DexRow[]>(initialSnapshot.dexes);
  const [error, setError] = useState<string | null>(
    initialSnapshot.source === "endpoint-not-implemented"
      ? "API endpoint not yet implemented — wire backend route to populate"
      : null,
  );
  const [selected, setSelected] = useState<DexRow | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [toggling, setToggling] = useState<string | null>(null);
  const [toggleError, setToggleError] = useState<string | null>(null);
  // R1: isMounted to avoid SSR/CSR mismatch on derived values
  const [isMounted, setIsMounted] = useState(false);
  const [chainFilter, setChainFilter] = useState<string>(ALL_CHAINS);
  // 2026-05-10 audit follow-up: Add / Remove dialogs.
  const [addOpen, setAddOpen] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<DexRow | null>(null);
  const [removing, setRemoving] = useState(false);
  const [removeError, setRemoveError] = useState<string | null>(null);

  // Chain catalog from /api/chains (with doctrinal fallback) — single source
  // for both the filter chips and the Add dialog's chain selectors.
  const { chains: chainCatalog } = useChains();

  // Quick-lookup map: chain_id → human name, used by the filter chips and
  // the per-row chain badges. Falls back to `Chain ${id}` for unknown ids
  // so we never blank out — R8 surface honest "unknown" rather than hide.
  const chainNameById = useMemo(() => {
    const map = new Map<number, string>();
    for (const c of chainCatalog) map.set(c.chain_id, c.name);
    return map;
  }, [chainCatalog]);
  const chainLabel = useCallback(
    (id: number): string => chainNameById.get(id) ?? `Chain ${id}`,
    [chainNameById],
  );

  useEffect(() => {
    setIsMounted(true);
  }, []);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    const res = await getDexes(EDGE_URL);
    if (res.ok) {
      setDexes(res.data.dexes);
      setError(null);
    } else {
      setError(res.error);
    }
    setRefreshing(false);
  }, []);

  const handleToggle = useCallback(
    async (dex: DexRow, e: React.MouseEvent) => {
      e.stopPropagation();
      setToggling(dex.id);
      setToggleError(null);
      // V-AT-1 / OMEGA-8 M5: never read admin token from localStorage. Cookie
      // session is the only path; empty string lets the request fail-fast at
      // the api-server's admin gate if no session is active.
      const adminToken = "";
      const res = await toggleDexActive(EDGE_URL, dex.id, !dex.is_active, adminToken);
      if (res.ok) {
        setDexes((prev) =>
          prev.map((d) =>
            d.id === dex.id ? { ...d, is_active: res.data.is_active } : d,
          ),
        );
      } else {
        setToggleError(`Toggle failed: ${res.error}`);
      }
      setToggling(null);
    },
    [],
  );

  const handleRemove = useCallback(async () => {
    if (!removeTarget) return;
    setRemoving(true);
    setRemoveError(null);
    // V-AT-1 / OMEGA-8 M5: never read admin token from localStorage.
    const adminToken = "";
    const res = await deleteDex(EDGE_URL, removeTarget.id, adminToken);
    setRemoving(false);
    if (res.ok) {
      setDexes((prev) => prev.filter((d) => d.id !== removeTarget.id));
      setRemoveTarget(null);
    } else {
      setRemoveError(res.error);
    }
  }, [removeTarget]);

  const handleDexCreated = useCallback((dex: DexRow) => {
    setDexes((prev) => {
      // Insert sorted by tvl DESC then name ASC — keeps table stable.
      const next = [dex, ...prev];
      next.sort((a, b) => {
        const at = a.tvl_usd ?? -1;
        const bt = b.tvl_usd ?? -1;
        if (bt !== at) return bt - at;
        return a.name.localeCompare(b.name);
      });
      return next;
    });
  }, []);

  // Derive unique chain IDs for filter tabs
  const allChainIds: number[] = isMounted
    ? Array.from(new Set(dexes.flatMap((d) => d.chain_ids))).sort(
        (a, b) => a - b,
      )
    : [];

  const filtered =
    chainFilter === ALL_CHAINS
      ? dexes
      : dexes.filter((d) =>
          d.chain_ids.includes(Number(chainFilter)),
        );

  return (
    <div className="p-8 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border pb-4">
        <div>
          <h1 className="text-3xl font-extrabold tracking-tight text-foreground">
            DEX Registry
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Authorised DEX protocols — volume, TVL, enable/disable per
            operator.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setAddOpen(true)}
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors text-xs font-semibold"
            title="Register a new DEX with one or more chain factories"
          >
            <Plus className="size-3.5" />
            Add DEX
          </button>
          <button
            type="button"
            onClick={refresh}
            disabled={refreshing}
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-border bg-muted text-muted-foreground hover:text-foreground hover:bg-accent transition-colors text-xs font-semibold disabled:opacity-50"
            title="Refresh DEX list"
          >
            <RefreshCw
              className={`size-3.5 ${refreshing ? "animate-spin" : ""}`}
            />
            Refresh
          </button>
        </div>
      </div>

      {/* R8 fail-honest notices */}
      {error && <EndpointNotice message={error} />}
      {toggleError && (
        <div className="flex items-start gap-3 rounded-xl border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">
          <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
          {toggleError}
        </div>
      )}

      {/* Chain filter tabs — only rendered after mount to avoid SSR mismatch */}
      {isMounted && allChainIds.length > 0 && (
        <div className="flex gap-2 flex-wrap">
          <button
            type="button"
            onClick={() => setChainFilter(ALL_CHAINS)}
            className={`px-3 py-1 rounded-full text-xs font-semibold border transition-colors ${
              chainFilter === ALL_CHAINS
                ? "bg-primary text-primary-foreground border-primary"
                : "border-border text-muted-foreground hover:text-foreground hover:border-foreground/30"
            }`}
          >
            All chains
          </button>
          {allChainIds.map((c) => (
            <button
              key={c}
              type="button"
              onClick={() => setChainFilter(String(c))}
              className={`px-3 py-1 rounded-full text-xs font-semibold border transition-colors ${
                chainFilter === String(c)
                  ? "bg-primary text-primary-foreground border-primary"
                  : "border-border text-muted-foreground hover:text-foreground hover:border-foreground/30"
              }`}
              title={`Chain ID ${c}`}
            >
              {chainLabel(c)}
            </button>
          ))}
        </div>
      )}

      {/* Empty state */}
      {!error && isMounted && filtered.length === 0 && (
        <div className="flex flex-col items-center justify-center py-16 text-muted-foreground gap-3">
          <Database className="size-10 opacity-30" />
          <p className="text-sm">No DEXes returned by the API.</p>
          <p className="text-xs">
            Populate the{" "}
            <code className="font-mono text-xs">dexes</code> table via
            migration 044/045 and wire the endpoint.
          </p>
        </div>
      )}

      {/* Table */}
      {filtered.length > 0 && (
        <div className="rounded-2xl border border-border overflow-hidden shadow-sm">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-muted text-muted-foreground text-xs uppercase tracking-wider">
                <th className="px-4 py-3 border-b border-border">Name</th>
                <th className="px-4 py-3 border-b border-border">Protocol</th>
                <th className="px-4 py-3 border-b border-border">Chains</th>
                <th className="px-4 py-3 border-b border-border text-right">
                  Vol 24h
                </th>
                <th className="px-4 py-3 border-b border-border text-right">
                  TVL
                </th>
                <th className="px-4 py-3 border-b border-border text-center">
                  Status
                </th>
                <th className="px-4 py-3 border-b border-border text-center">
                  Toggle
                </th>
                <th className="px-4 py-3 border-b border-border text-center">
                  Remove
                </th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((d) => (
                <tr
                  key={d.id}
                  onClick={() => setSelected(d)}
                  className="border-b border-border/50 hover:bg-muted/40 transition-colors cursor-pointer"
                >
                  <td className="px-4 py-3 font-semibold text-sm">{d.name}</td>
                  <td className="px-4 py-3 text-xs text-muted-foreground font-mono">
                    {d.protocol_type}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex flex-wrap gap-1">
                      {d.chain_ids.map((c) => (
                        <Badge
                          key={c}
                          variant="outline"
                          className="text-[10px]"
                          title={`Chain ID ${c}`}
                        >
                          {chainLabel(c)}
                        </Badge>
                      ))}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-xs">
                    {fmtUsd(d.volume_24h_usd)}
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-xs">
                    {fmtUsd(d.tvl_usd)}
                  </td>
                  <td className="px-4 py-3 text-center">
                    {d.is_active ? (
                      <Badge variant="success">Active</Badge>
                    ) : (
                      <Badge variant="secondary">Disabled</Badge>
                    )}
                  </td>
                  <td className="px-4 py-3 text-center">
                    <button
                      type="button"
                      disabled={toggling === d.id}
                      onClick={(e) => handleToggle(d, e)}
                      className="flex items-center justify-center mx-auto text-muted-foreground hover:text-foreground transition-colors disabled:opacity-40"
                      title={d.is_active ? "Disable DEX" : "Enable DEX"}
                      aria-label={d.is_active ? `Disable ${d.name}` : `Enable ${d.name}`}
                    >
                      {d.is_active ? (
                        <ToggleRight className="size-5 text-success" />
                      ) : (
                        <ToggleLeft className="size-5" />
                      )}
                    </button>
                  </td>
                  <td className="px-4 py-3 text-center">
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        setRemoveError(null);
                        setRemoveTarget(d);
                      }}
                      className="flex items-center justify-center mx-auto text-muted-foreground hover:text-destructive transition-colors"
                      title={`Remove ${d.name} from the registry (refuses if any pool uses it)`}
                      aria-label={`Remove ${d.name}`}
                    >
                      <Trash2 className="size-4" />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Detail sheet */}
      <DexDetailDialog dex={selected} onClose={() => setSelected(null)} />

      {/* Add DEX dialog */}
      {addOpen && (
        <AddDexDialog
          chains={chainCatalog}
          onClose={() => setAddOpen(false)}
          onCreated={(dex) => {
            handleDexCreated(dex);
            setAddOpen(false);
          }}
        />
      )}

      {/* Remove confirmation dialog */}
      {removeTarget && (
        <ConfirmRemoveDialog
          dex={removeTarget}
          chainLabel={chainLabel}
          busy={removing}
          errorMsg={removeError}
          onCancel={() => {
            setRemoveTarget(null);
            setRemoveError(null);
          }}
          onConfirm={handleRemove}
        />
      )}
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────────────
// Add DEX dialog
// ────────────────────────────────────────────────────────────────────────────

interface AddDexDialogProps {
  chains: readonly ChainInfo[];
  onClose: () => void;
  onCreated: (dex: DexRow) => void;
}

interface FactoryDraft {
  /** Unique row key so React doesn't reuse stale inputs when rows reorder. */
  key: string;
  chain_id: number;
  address: string;
}

function freshDraft(chainId: number): FactoryDraft {
  // Math.random suffices for in-form row keys — never persisted.
  return { key: Math.random().toString(36).slice(2), chain_id: chainId, address: "" };
}

function AddDexDialog({ chains, onClose, onCreated }: AddDexDialogProps) {
  const [name, setName] = useState("");
  const [protocolType, setProtocolType] = useState<string>(KNOWN_PROTOCOLS[0]);
  const [factoryRows, setFactoryRows] = useState<FactoryDraft[]>(() => [
    freshDraft(chains[0]?.chain_id ?? 1),
  ]);
  const [submitting, setSubmitting] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;
  const allRowsValid = factoryRows.every((f) => ADDR_RE.test(f.address.trim()));
  const canSubmit =
    name.trim().length > 0 &&
    protocolType.trim().length > 0 &&
    factoryRows.length > 0 &&
    allRowsValid &&
    !submitting;

  const onSubmit = useCallback(async () => {
    setErr(null);
    setSubmitting(true);
    const edgeUrl = process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";
    // V-AT-1 / OMEGA-8 M5: never read admin token from localStorage.
    const adminToken = "";
    const payload: { name: string; protocol_type: string; factories: CreateDexFactory[] } = {
      name: name.trim(),
      protocol_type: protocolType.trim(),
      factories: factoryRows.map((r) => ({
        chain_id: r.chain_id,
        address: r.address.trim().toLowerCase(),
      })),
    };
    const res = await createDex(edgeUrl, payload, adminToken);
    setSubmitting(false);
    if (res.ok) {
      onCreated(res.data);
    } else {
      setErr(res.error);
    }
  }, [name, protocolType, factoryRows, onCreated]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
    >
      <div className="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-2xl max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between px-5 py-3 border-b border-border">
          <h2 className="text-lg font-bold">Register a new DEX</h2>
          <button
            type="button"
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground"
            aria-label="Close"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="p-5 space-y-5">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label htmlFor="dex-name" className="text-xs uppercase tracking-wide">
                Name
              </Label>
              <Input
                id="dex-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. Sushi"
                maxLength={100}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="dex-proto" className="text-xs uppercase tracking-wide">
                Protocol type
              </Label>
              <Input
                id="dex-proto"
                list="dex-proto-suggest"
                value={protocolType}
                onChange={(e) => setProtocolType(e.target.value)}
                placeholder="UNISWAP_V2"
                maxLength={32}
              />
              <datalist id="dex-proto-suggest">
                {KNOWN_PROTOCOLS.map((p) => (
                  <option key={p} value={p} />
                ))}
              </datalist>
            </div>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-xs uppercase tracking-wide">Factories (per chain)</Label>
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() =>
                  setFactoryRows((prev) => [
                    ...prev,
                    freshDraft(chains[0]?.chain_id ?? 1),
                  ])
                }
              >
                <Plus className="size-3 mr-1" /> Add chain
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              One factory contract address per chain the DEX operates on. The
              <code className="font-mono mx-1 text-xs">(chain_id, address)</code>
              pair must be unique across the registry.
            </p>
            <div className="space-y-2">
              {factoryRows.map((row, idx) => {
                const validAddr = ADDR_RE.test(row.address.trim());
                return (
                  <div key={row.key} className="flex items-center gap-2">
                    <select
                      aria-label={`Chain for factory row ${idx + 1}`}
                      title="Chain on which this factory contract lives"
                      value={row.chain_id}
                      onChange={(e) => {
                        const cid = Number(e.target.value);
                        setFactoryRows((prev) =>
                          prev.map((r, i) => (i === idx ? { ...r, chain_id: cid } : r)),
                        );
                      }}
                      className="h-9 w-44 rounded-md border border-input bg-transparent px-3 text-sm"
                    >
                      {chains.map((c) => (
                        <option key={c.chain_id} value={c.chain_id}>
                          {c.name} ({c.chain_id})
                        </option>
                      ))}
                    </select>
                    <Input
                      value={row.address}
                      onChange={(e) =>
                        setFactoryRows((prev) =>
                          prev.map((r, i) =>
                            i === idx ? { ...r, address: e.target.value } : r,
                          ),
                        )
                      }
                      placeholder="0xFactoryAddress…"
                      className={`flex-1 font-mono text-xs ${
                        row.address.length > 0 && !validAddr ? "border-destructive" : ""
                      }`}
                    />
                    <button
                      type="button"
                      onClick={() =>
                        setFactoryRows((prev) => prev.filter((_, i) => i !== idx))
                      }
                      disabled={factoryRows.length === 1}
                      className="text-muted-foreground hover:text-destructive disabled:opacity-30"
                      title={
                        factoryRows.length === 1
                          ? "At least one factory required"
                          : "Remove this row"
                      }
                    >
                      <Trash2 className="size-4" />
                    </button>
                  </div>
                );
              })}
            </div>
          </div>

          {err && (
            <div className="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span className="font-mono">{err}</span>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border bg-muted/30">
          <Button type="button" variant="ghost" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button type="button" onClick={onSubmit} disabled={!canSubmit}>
            {submitting ? "Creating…" : "Create DEX"}
          </Button>
        </div>
      </div>
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────────────
// Confirm remove dialog
// ────────────────────────────────────────────────────────────────────────────

interface ConfirmRemoveProps {
  dex: DexRow;
  chainLabel: (id: number) => string;
  busy: boolean;
  errorMsg: string | null;
  onCancel: () => void;
  onConfirm: () => void;
}

function ConfirmRemoveDialog({ dex, chainLabel, busy, errorMsg, onCancel, onConfirm }: ConfirmRemoveProps) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
    >
      <div className="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-md">
        <div className="px-5 py-4 border-b border-border">
          <h2 className="text-lg font-bold flex items-center gap-2">
            <Trash2 className="size-4 text-destructive" />
            Remove DEX
          </h2>
        </div>
        <div className="p-5 space-y-3">
          <p className="text-sm">
            Hard-delete <strong className="font-mono">{dex.name}</strong> from
            the registry? This removes its factory rows on{" "}
            {dex.chain_ids.map(chainLabel).join(", ")}.
          </p>
          <p className="text-xs text-muted-foreground">
            The backend refuses if any pool already references this DEX — disable
            via the toggle instead in that case.
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
          <Button
            type="button"
            variant="destructive"
            onClick={onConfirm}
            disabled={busy}
          >
            {busy ? "Removing…" : "Remove"}
          </Button>
        </div>
      </div>
    </div>
  );
}
