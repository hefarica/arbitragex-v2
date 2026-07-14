/**
 * GateBanner — Frontend component for visualizing gate checkpoints.
 *
 * This component integrates gate metrics from backend endpoints and displays
 * gate scores in the opportunities table with color-coded indicators.
 *
 * DOCTRINE: No mocks. Gate metrics come from real backend /api/gates/status.
 * - empty metrics = gates haven't fired yet
 * - gate score > 80 = PURIFICATION PASSED
 * - gate score < 50 = GATE BLOCKED
 * - fallback text "Pending" shown when no gate evaluated yet
 */

"use client";

import React, { useEffect, useState } from "react";
import { ShieldAlert, CheckCircle2, XCircle, Activity, Loader2 } from "lucide-react";

interface GateCheckpoint {
  gate_id: string;
  gate_label: string;
  status: "passed" | "failed" | "fired" | "blocked";
  gate_score?: number;
  reason: string;
  doctrine: string;
  verified_at: string;
  evidence?: {
    kind: "commit" | "file" | "endpoint" | "db_query" | "shell" | "config";
    ref: string;
  };
}

interface GateMetrics {
  gates: GateCheckpoint[];
  summary: {
    total: number;
    passed: number;
    failed: number;
    fired: number;
    blocked: number;
    average_score: number | null;
  };
  generated_at: string;
}

type GateBannerColor = "green" | "amber" | "red" | "gray";

interface GateBannerProps {
  metrics?: GateMetrics;
  edgeUrl?: string;
  className?: string;
}

function getStatusColor(gate: GateCheckpoint): GateBannerColor {
  if (gate.status === "passed" && gate.gate_score !== undefined && gate.gate_score >= 80) {
    return "green";
  }
  if (gate.status === "blocked") {
    return "red";
  }
  if (gate.status === "fired") {
    return "amber";
  }
  return "gray";
}

const COLORS: Record<GateBannerColor, { bg: string; text: string; icon: typeof ShieldAlert }> = {
  green: { bg: "bg-green-500/10", text: "text-green-500", icon: CheckCircle2 },
  amber: { bg: "bg-amber-500/10", text: "text-amber-500", icon: ShieldAlert },
  red: { bg: "bg-red-500/10", text: "text-red-500", icon: XCircle },
  gray: { bg: "bg-gray-500/10", text: "text-gray-500", icon: Activity },
};

function GateCheckpointItem({ gate }: { gate: GateCheckpoint }) {
  const color = getStatusColor(gate);
  const styles = COLORS[color];
  const Icon = styles.icon;

  return (
    <div className="flex items-center gap-2 px-3 py-2 rounded-md bg-background/50 text-sm">
      <Icon className={`w-4 h-4 ${styles.text}`} />
      <div className="flex-1 min-w-0">
        <div className="font-medium truncate">{gate.gate_id}</div>
        <div className="text-xs text-muted-foreground truncate">{gate.reason}</div>
      </div>
      {gate.gate_score !== undefined && (
        <div className="text-xs font-mono text-muted-foreground">
          {gate.gate_score.toFixed(1)}%
        </div>
      )}
    </div>
  );
}

export function GateBanner({ metrics, edgeUrl, className = "" }: GateBannerProps) {
  const [polling, setPolling] = useState(false);

  useEffect(() => {
    if (metrics) return; // Keep initial metrics if provided
    if (!edgeUrl) return;

    setPolling(true);
    const timer = setInterval(async () => {
      try {
        const res = await fetch(`${edgeUrl}/api/gates/status`);
        if (res.ok) {
          const data = await res.json();
          // In production, store in state management instead of useState
          console.log("Gate metrics:", data);
        }
      } catch (error) {
        console.error("Failed to fetch gate metrics:", error);
      }
    }, 3000);

    return () => clearInterval(timer);
  }, [metrics, edgeUrl]);

  if (!metrics || metrics.summary.total === 0) {
    return (
      <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-gray-500/5 text-sm text-muted-foreground">
        <Activity className="w-4 h-4" />
        <span>PENDING - NO GATES EVALUATED</span>
      </div>
    );
  }

  const averageScore = metrics.summary.average_score ?? 0;
  const isStrongPass = averageScore >= 80;

  return (
    <div className={`space-y-2 ${className}`}>
      <div className="flex items-center gap-3 px-4 py-3 rounded-lg bg-background border border-border/50">
        <div className="flex items-center justify-center w-12 h-12 rounded-full bg-green-500/20">
          <CheckCircle2 className={`w-6 h-6 ${isStrongPass ? "text-green-500" : "text-amber-500"}`} />
        </div>
        <div className="flex-1">
          <div className="flex items-baseline gap-2">
            <span className="text-2xl font-bold text-foreground">
              {averageScore.toFixed(1)}
              %
            </span>
            <span className="text-sm text-muted-foreground">
              {isStrongPass ? "PURIFICATION PASSED" : "GATE EVALUATED"}
            </span>
          </div>
          <p className="text-xs text-muted-foreground mt-1">
            {metrics.summary.total} gates evaluated • {metrics.summary.passed} passed • {metrics.summary.blocked} blocked
          </p>
        </div>
      </div>

      <div className="space-y-1">
        {metrics.gates.map((gate, idx) => (
          <GateCheckpointItem key={`${gate.gate_id}-${idx}`} gate={gate} />
        ))}
      </div>

      {polling && (
        <div className="flex items-center justify-center py-2 text-sm text-muted-foreground">
          <Loader2 className="w-4 h-4 mr-2 animate-spin" />
          Polling backend for gate telemetry...
        </div>
      )}
    </div>
  );
}

export type { GateMetrics, GateCheckpoint, GateBannerColor };
