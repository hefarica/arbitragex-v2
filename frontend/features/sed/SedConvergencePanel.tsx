"use client";

/**
 * SedConvergencePanel — SED pipeline telemetry surface (OMEGA Nivel 0).
 *
 * Consumes the "convergence" WebSocket room via useConvergenceStream.
 * Displays real-time entropy snapshot + pipeline metrics from the
 * Spectral Economic Drive (SED) core.
 *
 * R8 fail-honest: null values render as "—", never "0".
 * Ghost Protocol: capital_exposure_usd = 0.0000000000 (not displayed here).
 * Zero-Mocks: all data comes from the WS stream; no fabricated defaults.
 */

import * as React from "react";
import {
  ActivityIcon,
  ZapIcon,
  ClockIcon,
  BarChart3Icon,
  CpuIcon,
  AlertCircleIcon,
} from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  useConvergenceStream,
  type ConvergenceSignal,
} from "@/lib/hooks/useConvergenceStream";
import { sanitizeForDisplay } from "@/lib/omega-lexicon";

// ─── Child components ─────────────────────────────────────────────────────────

function MetricCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-muted/40 rounded p-2">
      <div className="text-xs text-muted-foreground">
        {sanitizeForDisplay(label)}
      </div>
      <div className="font-mono font-semibold">{value}</div>
    </div>
  );
}

function SignalBody({ signal }: { signal: ConvergenceSignal }) {
  const entropy = signal.entropy_snapshot;
  return (
    <div className="space-y-4">
      {/* Status badge */}
      <div className="flex items-center gap-2">
        <Badge variant="default" className="bg-success/20 text-success">
          <ActivityIcon className="w-3 h-3 mr-1" />
          Signal received
        </Badge>
        <span className="text-xs text-muted-foreground">
          {new Date(signal.timestamp).toLocaleTimeString()}
        </span>
      </div>

      {/* Entropy snapshot */}
      <div>
        <h4 className="text-sm font-semibold mb-2 flex items-center gap-2">
          <BarChart3Icon className="w-4 h-4" />
          Mempool Entropy
        </h4>
        <div className="grid grid-cols-2 gap-2 text-sm">
          <MetricCard
            label="TX/sec"
            value={Number.isFinite(entropy.mempool_tx_per_sec) ? entropy.mempool_tx_per_sec.toFixed(1) : "—"}
          />
          <MetricCard
            label="Gas Price (Gwei)"
            value={Number.isFinite(entropy.mempool_avg_gas_price_gwei) ? entropy.mempool_avg_gas_price_gwei.toFixed(2) : "—"}
          />
          <MetricCard
            label="Entropy Score"
            value={Number.isFinite(entropy.mempool_entropy_score) ? entropy.mempool_entropy_score.toFixed(3) : "—"}
          />
          <MetricCard
            label="Reserve Divergence"
            value={Number.isFinite(entropy.reserve_divergence_max) ? entropy.reserve_divergence_max.toFixed(4) : "—"}
          />
        </div>
      </div>

      {/* Pipeline metrics */}
      <div>
        <h4 className="text-sm font-semibold mb-2 flex items-center gap-2">
          <CpuIcon className="w-4 h-4" />
          {sanitizeForDisplay("SED Pipeline")}
        </h4>
        <div className="grid grid-cols-4 gap-2 text-sm">
          <MetricCard
            label="Latency (ms)"
            value={Number.isFinite(signal.pipeline_latency_ms) ? String(signal.pipeline_latency_ms) : "—"}
          />
          <MetricCard
            label="Detected"
            value={Number.isFinite(signal.opportunities_detected) ? String(signal.opportunities_detected) : "—"}
          />
          <MetricCard
            label="Run"
            value={Number.isFinite(signal.simulations_run) ? String(signal.simulations_run) : "—"}
          />
          <MetricCard
            label="Success"
            value={Number.isFinite(signal.simulations_success) ? String(signal.simulations_success) : "—"}
          />
        </div>
      </div>
    </div>
  );
}

// ─── Loading skeleton ─────────────────────────────────────────────────────────

function LoadingRows() {
  return (
    <div className="space-y-2">
      <Skeleton className="h-4 w-3/4" />
      <Skeleton className="h-4 w-1/2" />
      <Skeleton className="h-4 w-2/3" />
      <Skeleton className="h-4 w-5/6" />
    </div>
  );
}

// ─── Main panel component ─────────────────────────────────────────────────────

export function SedConvergencePanel() {
  const { signal, status } = useConvergenceStream();

  return (
    <Card data-slot="sed-convergence-panel">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <ZapIcon className="w-5 h-5 text-primary" />
          {sanitizeForDisplay("SED Pipeline Telemetry")}
          {status === "LIVE" && (
            <Badge
              variant="outline"
              className="text-success border-success"
            >
              <ActivityIcon className="w-3 h-3 mr-1" />
              LIVE
            </Badge>
          )}
          {status === "CONNECTING" && (
            <Badge
              variant="outline"
              className="text-warning border-warning"
            >
              <ClockIcon className="w-3 h-3 mr-1" />
              CONNECTING
            </Badge>
          )}
          {status === "STALE" && (
            <Badge
              variant="outline"
              className="text-destructive border-destructive"
            >
              STALE
            </Badge>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {status === "CONNECTING" && !signal && <LoadingRows />}
        {status === "STALE" && !signal && (
          <Alert variant="destructive">
            <AlertCircleIcon className="w-4 h-4" />
            <AlertTitle>WebSocket disconnected</AlertTitle>
            <AlertDescription className="text-sm">
              {sanitizeForDisplay(
                "Waiting for convergence signals. Admin session required for WebSocket access."
              )}
            </AlertDescription>
          </Alert>
        )}
        {signal && <SignalBody signal={signal} />}
      </CardContent>
    </Card>
  );
}
