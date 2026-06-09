"use client";

/**
 * FE-CRIT-07 — WalletsClient
 *
 * Observers & Allowances view.
 *
 * Fixes (2026-06-08):
 *   - R1 Mounted-Snapshot: state is SEEDED from `initialSnapshot` (the SSR
 *     fetch in page.tsx). First paint shows the server data, not an empty
 *     table. Previously the snapshot prop was received and discarded.
 *   - ONE fetcher: the richer fail-honest `getWallets()` from lib/api/wallets
 *     (discriminated Result with 404 / timeout / HTTP-status surfacing). The
 *     omni-store `fetchWallets()` path — which used a second fetcher and
 *     swallowed errors to console.error — is no longer used here, so the
 *     empty state can never be confused with a failed fetch.
 *   - Distinct loading / empty / error states (empty ≠ error). A failed fetch
 *     renders an honest EdgeState error, NOT the "No wallets" empty state.
 *   - Working refresh action.
 *   - Safety posture strip: live OFF / broadcast OFF / capital exposure — the
 *     capital figure is COMPUTED from the wallet balances actually returned by
 *     the backend (never hardcoded green); live/broadcast OFF are structural
 *     facts of this binary (no broadcast path is wired — see SystemGuardBanner).
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { AlertCircle, RefreshCw, ShieldOff, Wallet } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { WalletDetailDialog } from "@/components/WalletDetailDialog";
import { EdgeState } from "@/components/EdgeState";
import {
  getWallets,
  getWalletBalances,
  type WalletRow,
} from "@/lib/api/wallets";
import { getApiBaseUrl } from "@/lib/api-client";
import { cn } from "@/lib/utils";

export interface WalletsSnapshot {
  wallets: WalletRow[];
  source: string;
}

interface Props {
  initialSnapshot: WalletsSnapshot;
}

type FetchState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready" }
  | { kind: "error"; error: string };

// A server-side snapshot source string that indicates the SSR fetch itself
// failed (vs. legitimately returning zero wallets). page.tsx encodes these.
function snapshotError(source: string): string | null {
  if (source === "endpoint-not-implemented") {
    return "missing_backend_contract — GET /api/v1/wallets is not wired (404).";
  }
  if (source.startsWith("server-fetch-failed")) {
    return `Server snapshot fetch failed (${source}).`;
  }
  return null;
}

export default function WalletsClient({ initialSnapshot }: Props) {
  // R1 Mounted-Snapshot: seed from SSR so first paint is the server data.
  const [wallets, setWallets] = useState<WalletRow[]>(initialSnapshot.wallets);
  // If the SSR snapshot itself errored, start in the error state rather than
  // pretending the empty array means "no wallets".
  const initialError = snapshotError(initialSnapshot.source);
  const [fetchState, setFetchState] = useState<FetchState>(
    initialError ? { kind: "error", error: initialError } : { kind: "idle" },
  );
  const [capitalExposureUsd, setCapitalExposureUsd] = useState<number | null>(null);
  const [capitalNote, setCapitalNote] = useState<string>(
    "not yet computed",
  );
  const [selected, setSelected] = useState<WalletRow | null>(null);
  const [isMounted, setIsMounted] = useState(false);

  useEffect(() => {
    setIsMounted(true);
  }, []);

  // Compute on-chain capital exposure from the balances the backend returns.
  // In paper/shadow posture this is structurally $0 (signers hold no funds),
  // but we PROVE it from data rather than printing a hardcoded zero.
  const recomputeExposure = useCallback(async (rows: WalletRow[]) => {
    const edgeUrl = getApiBaseUrl();
    if (rows.length === 0) {
      setCapitalExposureUsd(0);
      setCapitalNote("no wallets — exposure structurally $0");
      return;
    }
    const results = await Promise.all(
      rows.map((w) => getWalletBalances(edgeUrl, w.address)),
    );
    let totalUsd = 0;
    let anyValued = false;
    let anyError = false;
    for (const r of results) {
      if (!r.ok) {
        anyError = true;
        continue;
      }
      for (const b of r.data.balances) {
        if (b.balance_usd != null) {
          totalUsd += b.balance_usd;
          anyValued = true;
        }
      }
    }
    setCapitalExposureUsd(totalUsd);
    if (anyError) {
      setCapitalNote("partial — some balance endpoints unreachable");
    } else if (!anyValued) {
      setCapitalNote("no USD-valued balances reported by backend");
    } else {
      setCapitalNote("summed from backend balances");
    }
  }, []);

  const load = useCallback(async () => {
    setFetchState({ kind: "loading" });
    const edgeUrl = getApiBaseUrl();
    const res = await getWallets(edgeUrl);
    if (!res.ok) {
      // Fail-honest: surface the error, KEEP the last known wallets (snapshot or
      // a prior good fetch). Never collapse a failed fetch into "No wallets".
      setFetchState({ kind: "error", error: res.error });
      return;
    }
    const rows = res.data.wallets ?? [];
    setWallets(rows);
    setFetchState({ kind: "ready" });
    void recomputeExposure(rows);
  }, [recomputeExposure]);

  // Initial client load (and exposure compute) once mounted. The snapshot has
  // already painted; this refreshes against the live edge and distinguishes
  // empty vs error honestly.
  useEffect(() => {
    void load();
  }, [load]);

  const refresh = useCallback(() => {
    void load();
  }, [load]);

  const isLoading = fetchState.kind === "loading";
  const isError = fetchState.kind === "error";
  // Empty ≠ error: only treat as empty when a fetch actually succeeded with
  // zero rows (or the SSR snapshot was a clean empty), and we are NOT in error.
  const isEmpty = isMounted && !isError && wallets.length === 0 && !isLoading;

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-center justify-between border-b border-border pb-4">
        <div>
          <h1 className="text-3xl font-extrabold tracking-tight text-foreground">
            Operational Wallets
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Executor signers, treasury addresses, per-chain balances and token allowances.
          </p>
        </div>
        <button
          type="button"
          onClick={refresh}
          disabled={isLoading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-border bg-muted text-muted-foreground hover:text-foreground hover:bg-accent transition-colors text-xs font-semibold disabled:opacity-50"
          data-testid="wallets-refresh"
        >
          <RefreshCw className={`size-3.5 ${isLoading ? "animate-spin" : ""}`} />
          Refresh
        </button>
      </div>

      {/* Safety posture — live OFF / broadcast OFF / capital exposure.
          live/broadcast are structural (no broadcast path wired); capital is
          computed from backend balances (proven, not hardcoded). */}
      <div
        data-testid="wallets-safety-posture"
        className="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border border-destructive/20 bg-destructive/5 px-4 py-2.5 text-xs"
      >
        <span className="inline-flex items-center gap-1.5 font-semibold uppercase tracking-wider text-destructive">
          <ShieldOff className="size-3.5" aria-hidden="true" />
          Safety posture
        </span>
        <PostureTile label="Live" value="OFF" tone="danger" testid="posture-live" />
        <PostureTile label="Broadcast" value="OFF" tone="danger" testid="posture-broadcast" />
        <PostureTile
          label="Capital exposure"
          value={
            capitalExposureUsd == null
              ? "computing…"
              : `$${capitalExposureUsd.toFixed(2)}`
          }
          tone={capitalExposureUsd === 0 ? "safe" : "warning"}
          testid="posture-capital"
        />
        <span className="text-[11px] text-muted-foreground" data-testid="posture-capital-note">
          {capitalNote}
        </span>
      </div>

      {/* ERROR state — distinct from empty. An honest, blocking surface. */}
      {isError && (
        <EdgeState
          variant="error"
          title="Wallets unreachable"
          description="The wallets endpoint refused or could not be reached. No fabricated wallet data is shown — zero-trust. The last known list (if any) is preserved below."
          endpoint="GET /api/v1/wallets"
          reasons={[fetchState.error]}
          onRetry={refresh}
          retrying={isLoading}
          ghost="table"
        />
      )}

      {/* LOADING state — only when we have nothing painted yet. */}
      {isLoading && wallets.length === 0 && (
        <EdgeState
          variant="loading"
          title="Loading wallets…"
          description="Fetching the operator wallet registry from the edge."
          endpoint="GET /api/v1/wallets"
          ghost="table"
        />
      )}

      {/* EMPTY state — a successful fetch that legitimately returned zero. */}
      {isEmpty && (
        <EdgeState
          variant="empty"
          title="No wallets registered"
          description="The registry is reachable but returned zero wallets. This is an honest empty state, not a fetch failure."
          endpoint="GET /api/v1/wallets"
          onRetry={refresh}
          retrying={isLoading}
          ghost="table"
        />
      )}

      {wallets.length > 0 && (
        <div className="rounded-2xl border border-border overflow-hidden shadow-sm" data-testid="wallets-table">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-muted text-muted-foreground text-xs uppercase tracking-wider">
                <th className="px-4 py-3 border-b border-border">Label</th>
                <th className="px-4 py-3 border-b border-border">Address</th>
                <th className="px-4 py-3 border-b border-border">Role</th>
                <th className="px-4 py-3 border-b border-border text-right">Details</th>
              </tr>
            </thead>
            <tbody>
              {wallets.map((w) => (
                <tr
                  key={w.address}
                  onClick={() => setSelected(w)}
                  className="border-b border-border/50 hover:bg-muted/40 transition-colors cursor-pointer"
                >
                  <td className="px-4 py-3 font-semibold text-sm">{w.label}</td>
                  <td className="px-4 py-3 font-mono text-xs text-muted-foreground">
                    {w.address.slice(0, 8)}…{w.address.slice(-6)}
                  </td>
                  <td className="px-4 py-3">
                    {w.role ? (
                      <Badge variant="outline">{w.role}</Badge>
                    ) : (
                      <span className="text-muted-foreground/50 italic text-xs">—</span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        setSelected(w);
                      }}
                      className="text-xs text-primary hover:underline"
                    >
                      View
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <WalletDetailDialog
        wallet={selected}
        onClose={() => setSelected(null)}
      />
    </div>
  );
}

function PostureTile({
  label,
  value,
  tone,
  testid,
}: {
  label: string;
  value: string;
  tone: "danger" | "warning" | "safe";
  testid: string;
}) {
  return (
    <span
      data-testid={testid}
      data-tone={tone}
      className={cn(
        "inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 font-mono text-[11px] uppercase",
        tone === "danger" && "border-destructive/40 bg-destructive/10 text-destructive",
        tone === "warning" && "border-amber-500/40 bg-amber-500/10 text-amber-600 dark:text-amber-300",
        tone === "safe" && "border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
      )}
    >
      <span className="font-sans normal-case text-foreground/60">{label}:</span>
      <span className="font-semibold">{value}</span>
    </span>
  );
}
