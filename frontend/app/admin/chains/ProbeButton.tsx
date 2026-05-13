/**
 * B1.b — ProbeButton
 *
 * Triggers POST /api/admin/chains/:chain_id/probe via the edge proxy. The
 * backend executes a JSON-RPC batch (eth_chainId + eth_blockNumber) against
 * the registered rpc_http_url and returns latency + chain-id match + block
 * number. The button surfaces the result inline with color-coded latency.
 *
 * R8 fail-honest: latency_ms === null when the probe could not be completed
 * (timeout / network error). We render "—" rather than fabricating zero.
 *
 * Color coding (latency_ms):
 *   < 100ms  → green   (HFT-grade)
 *   < 500ms  → yellow  (acceptable for control-plane)
 *   ≥ 500ms  → red     (too slow for hot-path)
 *   null     → muted   (no measurement)
 */
"use client";

import * as React from "react";
import { useCallback, useState } from "react";
import { AlertCircle, CheckCircle2, Radio, RefreshCw, XCircle } from "lucide-react";

import { Button } from "@/components/ui/button";
import { probeAdminChain } from "@/lib/api-client";
import type { AdminChainProbeResult } from "@/lib/schemas";

interface Props {
  chainId: number;
  adminToken: string;
}

type ProbeState =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "done"; data: AdminChainProbeResult }
  | { kind: "error"; message: string };

function latencyClass(latency: number | null): string {
  if (latency == null) return "text-muted-foreground";
  if (latency < 100) return "text-success";
  if (latency < 500) return "text-warning";
  return "text-destructive";
}

export function ProbeButton({ chainId, adminToken }: Props) {
  const [state, setState] = useState<ProbeState>({ kind: "idle" });

  const run = useCallback(async () => {
    if (!adminToken) {
      setState({ kind: "error", message: "no admin session — unlock at /killswitch" });
      return;
    }
    setState({ kind: "running" });
    const res = await probeAdminChain(chainId, adminToken);
    if (!res.ok) {
      setState({ kind: "error", message: res.error });
      return;
    }
    setState({ kind: "done", data: res.data });
  }, [chainId, adminToken]);

  return (
    <div className="flex flex-col gap-1 text-xs">
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={run}
        disabled={state.kind === "running"}
        className="gap-1 h-7 px-2"
        title={`Probe RPC for chain ${chainId} (eth_chainId + eth_blockNumber)`}
      >
        {state.kind === "running" ? (
          <RefreshCw className="size-3 animate-spin" />
        ) : (
          <Radio className="size-3" />
        )}
        {state.kind === "running" ? "Probing…" : "Probe"}
      </Button>

      {state.kind === "done" && (
        <ProbeResultLine data={state.data} />
      )}

      {state.kind === "error" && (
        <span className="inline-flex items-center gap-1 text-destructive">
          <AlertCircle className="size-3" />
          <span className="font-mono break-all">{state.message}</span>
        </span>
      )}
    </div>
  );
}

function ProbeResultLine({ data }: { data: AdminChainProbeResult }) {
  if (!data.reachable) {
    return (
      <span className="inline-flex items-center gap-1 text-destructive">
        <XCircle className="size-3" />
        <span>unreachable</span>
        {data.error && <span className="font-mono break-all">· {data.error}</span>}
      </span>
    );
  }

  const lat = data.latency_ms;
  const okIcon = data.matches ? <CheckCircle2 className="size-3 text-success" /> : <XCircle className="size-3 text-destructive" />;
  const matchLabel = data.matches === null
    ? "unknown"
    : data.matches
      ? "chain_id ok"
      : `mismatch (observed ${data.observed_chain_id ?? "—"})`;

  return (
    <span className="inline-flex items-center gap-1.5 font-mono">
      {okIcon}
      <span className={latencyClass(lat)}>{lat == null ? "—" : `${lat}ms`}</span>
      <span className="text-muted-foreground">·</span>
      <span className="text-muted-foreground">{matchLabel}</span>
      {data.block_number != null && (
        <>
          <span className="text-muted-foreground">·</span>
          <span className="text-muted-foreground">blk {data.block_number}</span>
        </>
      )}
    </span>
  );
}
