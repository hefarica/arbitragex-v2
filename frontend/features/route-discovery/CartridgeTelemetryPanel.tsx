"use client";

/**
 * CartridgeTelemetryPanel — live cartridge (Rhai "PlayStation MEV") telemetry.
 *
 * Consumes the "telemetry" WebSocket room via useCartridgeTelemetry (the hook
 * that shipped orphaned — this panel finally surfaces it). Shows the recent
 * log_quantum stream the Rust cartridges publish to arbx:cartridge:telemetry,
 * highlighting v3_source_priced messages.
 *
 * R8 fail-honest: empty/STALE rendered explicitly; never fabricated. Shadow-only.
 */

import * as React from "react";
import { CpuIcon, ActivityIcon, ClockIcon, AlertCircleIcon, ShieldCheckIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import type { CartridgeTelemetry, TelemetryStatus } from "@/lib/hooks/useCartridgeTelemetry";
import { useCartridgeRest } from "@/lib/hooks/useRouteDiscoveryRest";

function StatusBadge({ status }: { status: TelemetryStatus }) {
  if (status === "LIVE")
    return (
      <Badge variant="outline" className="text-success border-success">
        <ActivityIcon className="w-3 h-3 mr-1" />
        LIVE
      </Badge>
    );
  if (status === "CONNECTING")
    return (
      <Badge variant="outline" className="text-warning border-warning">
        <ClockIcon className="w-3 h-3 mr-1" />
        CONNECTING
      </Badge>
    );
  return (
    <Badge variant="outline" className="text-destructive border-destructive">
      STALE
    </Badge>
  );
}

function MsgRow({ m }: { m: CartridgeTelemetry }) {
  const message = typeof m.message === "string" ? m.message : "";
  const isV3 = message.includes("v3_source_priced");
  return (
    <div className="grid grid-cols-12 gap-2 px-2 py-1 text-[11px] border-b border-border/30 items-center">
      <div className="col-span-2 font-mono truncate">
        {typeof m.cartridge_id === "string" ? m.cartridge_id : "—"}
      </div>
      <div className="col-span-1">
        <span className="font-mono">{typeof m.level === "string" ? m.level : "—"}</span>
      </div>
      <div className="col-span-9 font-mono truncate" title={message}>
        {isV3 ? (
          <Badge variant="outline" className="mr-1 text-primary border-primary">
            v3_source_priced
          </Badge>
        ) : null}
        {message || "—"}
      </div>
    </div>
  );
}

export function CartridgeTelemetryPanel() {
  const { messages, status } = useCartridgeRest();
  const recent = messages.slice(-40).reverse();

  return (
    <Card data-slot="cartridge-telemetry-panel">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base flex-wrap">
          <CpuIcon className="w-5 h-5 text-primary" />
          Strategy Forge — Cartridge Telemetry
          <StatusBadge status={status} />
          <Badge variant="outline" className="text-muted-foreground">
            <ShieldCheckIcon className="w-3 h-3 mr-1" />
            shadow-only
          </Badge>
        </CardTitle>
      </CardHeader>
      <CardContent>
        {status === "STALE" && messages.length === 0 && (
          <Alert variant="destructive">
            <AlertCircleIcon className="w-4 h-4" />
            <AlertTitle>No telemetry snapshot</AlertTitle>
            <AlertDescription className="text-sm">
              The cartridge telemetry snapshot is unavailable — the api-server/edge may be
              unreachable, or cartridges are not running in shadow
              (ARBX_CARTRIDGE_MODE=shadow). Polling /api/cartridges every 5s.
            </AlertDescription>
          </Alert>
        )}
        {messages.length === 0 && status !== "STALE" && (
          <p className="text-sm text-muted-foreground">
            Connected — waiting for the first cartridge log_quantum message…
          </p>
        )}
        {messages.length > 0 && (
          <div className="rounded border border-border/40">
            <div className="grid grid-cols-12 gap-2 px-2 py-1.5 text-[11px] uppercase text-muted-foreground border-b border-border/60">
              <div className="col-span-2">cartridge</div>
              <div className="col-span-1">level</div>
              <div className="col-span-9">message</div>
            </div>
            <div className="max-h-80 overflow-y-auto">
              {recent.map((m, i) => (
                <MsgRow key={i} m={m} />
              ))}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
