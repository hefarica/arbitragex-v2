/**
 * Pipeline Funnel Card — visual heartbeat counters from /api/scanner/heartbeat.
 *
 * Renders the per-period scanner funnel as a cascading bar chart, exposing
 * where mempool throughput converges vs drops (allowlist filter, math gate,
 * pool coverage gap, etc.). Operator can answer "why am I not seeing profit?"
 * in one glance instead of grepping logs.
 *
 * Snapshot is poll-fetched by the parent OperationsClient every 30s. Card
 * shows three states: loading (no data yet), error (404 from API → searcher
 * down or restarted, R8 fail-honest), and rendered (latest snapshot).
 *
 * Snapshot data persisted server-side every period_secs (default 60s) by
 * searcher-rs::workers::heartbeat_worker; TTL = 3× period_secs.
 */
"use client";

import { AlertCircleIcon, ActivityIcon } from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { ScannerHeartbeatSnapshot } from "@/lib/operations-schemas";

interface Props {
  snapshot: ScannerHeartbeatSnapshot | null;
  error: string | null;
  fetchedAt: string | null;
}

interface FunnelStage {
  label: string;
  value: number;
  hint: string;
  // Tailwind text colour token (consistent with KPICard tones).
  tone: "emerald" | "amber" | "rose" | "muted";
}

function buildFunnelStages(s: ScannerHeartbeatSnapshot): FunnelStage[] {
  const totalRejects =
    s.gate_token_not_allowed +
    s.gate_strategy_disabled +
    s.gate_no_config +
    s.gate_unknown_token_price +
    s.gate_anomalous_math +
    s.gate_other_rejected;

  return [
    {
      label: "1. Pending received",
      value: s.pending_received,
      hint: "Mempool tx delivered by Alchemy WS sample (per period)",
      tone: "muted",
    },
    {
      label: "2. Decoded OK",
      value: s.decoded_ok,
      hint: `Uniswap V2/V3 router calldata decoded (${s.pending_received > 0 ? ((s.decoded_ok / s.pending_received) * 100).toFixed(1) : "0"}%)`,
      tone: "muted",
    },
    {
      label: "3. Enriched (V2 + V3)",
      value: s.enriched_v2 + s.enriched_v3,
      hint: `V2-only: ${s.enriched_v2} · V2+V3 multicall: ${s.enriched_v3}`,
      tone: s.enriched_v2 + s.enriched_v3 > 0 ? "emerald" : "muted",
    },
    {
      label: "4. Passed all gates",
      value: s.passed_all_gates,
      hint: "Allowlist + strategy + price oracle + sanity + risk all passed",
      tone: s.passed_all_gates > 0 ? "emerald" : "muted",
    },
    {
      label: "5. Persisted to PG",
      value: s.db_persisted,
      hint: `DB writes succeeded (${s.db_errors} errors this period)`,
      tone: s.db_errors > 0 ? "amber" : "muted",
    },
    {
      label: "↳ Rejected by gates",
      value: totalRejects,
      hint: `TokenNotAllowed:${s.gate_token_not_allowed} · UnknownPrice:${s.gate_unknown_token_price} · AnomalousMath:${s.gate_anomalous_math} · Other:${s.gate_other_rejected}`,
      tone: s.gate_anomalous_math > 0 || s.gate_unknown_token_price > 0 ? "amber" : "muted",
    },
  ];
}

const TONE_CLASSES: Record<FunnelStage["tone"], { text: string; bar: string }> = {
  emerald: { text: "text-success", bar: "bg-success/40" },
  amber: { text: "text-warning", bar: "bg-warning/40" },
  rose: { text: "text-destructive", bar: "bg-destructive/40" },
  muted: { text: "text-muted-foreground", bar: "bg-muted-foreground/40" },
};

export function PipelineFunnelCard({ snapshot, error, fetchedAt }: Props) {
  if (error && !snapshot) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <ActivityIcon className="size-4" /> Scanner Pipeline Funnel
          </CardTitle>
          <CardDescription>
            <span className="flex items-center gap-1.5 text-warning">
              <AlertCircleIcon className="size-3.5" />
              <span className="font-mono text-xs">{error}</span>
            </span>
            Searcher may be down or recently restarted. Snapshot expires after 3× period_secs.
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  if (!snapshot) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <ActivityIcon className="size-4 animate-pulse" /> Scanner Pipeline Funnel
          </CardTitle>
          <CardDescription>Loading latest heartbeat snapshot…</CardDescription>
        </CardHeader>
      </Card>
    );
  }

  const stages = buildFunnelStages(snapshot);
  const maxValue = Math.max(...stages.map((s) => s.value), 1);

  const fmtAge = (iso: string | null): string => {
    if (!iso) return "";
    const ms = Date.now() - new Date(iso).getTime();
    if (ms < 60_000) return `${Math.round(ms / 1000)}s ago`;
    return `${Math.round(ms / 60_000)}m ago`;
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center justify-between text-base">
          <span className="flex items-center gap-2">
            <ActivityIcon className="size-4 text-success" />
            Scanner Pipeline Funnel · last {snapshot.period_secs}s
          </span>
          <span className="font-mono text-xs text-muted-foreground">{fmtAge(fetchedAt)}</span>
        </CardTitle>
        <CardDescription>
          Mempool funnel across decoder → enrichment → gate → persistence. Shows where throughput
          converges vs drops. Hot-refreshes every 30s (poll endpoint{" "}
          <code className="text-xs">/api/scanner/heartbeat</code>).
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-2">
        {stages.map((stage) => {
          const widthPct = Math.max(2, (stage.value / maxValue) * 100);
          const tone = TONE_CLASSES[stage.tone];
          return (
            <div key={stage.label} className="grid grid-cols-[160px_1fr_64px] items-center gap-3">
              <div className="text-xs text-muted-foreground truncate" title={stage.label}>
                {stage.label}
              </div>
              <div className="relative h-6 rounded bg-muted/30 overflow-hidden" title={stage.hint}>
                <div
                  className={`h-full transition-all duration-500 ${tone.bar}`}
                  style={{ width: `${widthPct}%` }}
                />
                <div className="absolute inset-0 flex items-center px-2 text-xs text-muted-foreground truncate">
                  {stage.hint}
                </div>
              </div>
              <div className={`font-mono text-sm tabular-nums text-right ${tone.text}`}>
                {stage.value.toLocaleString()}
              </div>
            </div>
          );
        })}

        <div className="grid grid-cols-3 gap-3 pt-3 border-t mt-3 text-xs">
          <Stat label="Redis stream Δ" value={snapshot.redis_stream_delta} signed />
          <Stat label="PG inserted (period)" value={snapshot.pg_period_inserted} />
          <Stat label="PG profit > 0" value={snapshot.pg_period_profit_pos} positive />
        </div>
      </CardContent>
    </Card>
  );
}

function Stat({
  label,
  value,
  signed = false,
  positive = false,
}: {
  label: string;
  value: number;
  signed?: boolean;
  positive?: boolean;
}) {
  const display = signed ? (value >= 0 ? `+${value}` : `${value}`) : value.toString();
  const tone = positive && value > 0 ? "text-success" : "text-muted-foreground";
  return (
    <div>
      <div className="text-muted-foreground text-[10px] uppercase tracking-wider">{label}</div>
      <div className={`font-mono text-sm tabular-nums ${tone}`}>{display}</div>
    </div>
  );
}
