"use client";

/**
 * CartridgeTelemetryPanel — live cartridge (Rhai) telemetry (enterprise-premium).
 *
 * Reads the public REST snapshot (useCartridgeRest, 5s poll). Highlights
 * v3_source_priced. R8 fail-honest: explicit STALE/empty states, no fabricated
 * values. Shadow-only.
 */

import * as React from "react";
import { CpuIcon, AlertCircleIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useCartridgeRest } from "@/lib/hooks/useRouteDiscoveryRest";
import { StatusPill, ReadOnlyBadge, Freshness } from "./premium-ui";

function levelTone(level?: string): string {
  switch ((level ?? "").toLowerCase()) {
    case "error":
      return "text-destructive border-destructive/50";
    case "warn":
    case "warning":
      return "text-warning border-warning/50";
    default:
      return "text-muted-foreground";
  }
}

export function CartridgeTelemetryPanel() {
  const { messages, status, updatedAt } = useCartridgeRest();
  const recent = messages.slice(-50).reverse();

  return (
    <Card data-slot="cartridge-telemetry-panel">
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <CpuIcon className="h-5 w-5 text-primary" />
            Strategy Forge — Cartridge Telemetry
          </CardTitle>
          <div className="flex items-center gap-2">
            <Freshness at={updatedAt} />
            <StatusPill status={status} />
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="secondary" className="text-[10px] uppercase tracking-wide">
            shadow
          </Badge>
          <ReadOnlyBadge label="observe-only" />
        </div>
      </CardHeader>
      <CardContent>
        {status === "STALE" && messages.length === 0 && (
          <Alert variant="destructive">
            <AlertCircleIcon className="h-4 w-4" />
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
          <div className="rounded-lg border">
            <div className="max-h-[22rem] overflow-auto">
              <Table>
                <TableHeader className="sticky top-0 bg-card">
                  <TableRow>
                    <TableHead className="text-[11px] uppercase tracking-wide">Cartridge</TableHead>
                    <TableHead className="text-[11px] uppercase tracking-wide">Level</TableHead>
                    <TableHead className="text-[11px] uppercase tracking-wide">Message</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {recent.map((m, i) => {
                    const message = typeof m.message === "string" ? m.message : "";
                    const isV3 = message.includes("v3_source_priced");
                    return (
                      <TableRow key={i}>
                        <TableCell className="font-mono text-xs">
                          {typeof m.cartridge_id === "string" ? m.cartridge_id : "—"}
                        </TableCell>
                        <TableCell>
                          <Badge variant="outline" className={`text-[10px] ${levelTone(m.level as string)}`}>
                            {typeof m.level === "string" ? m.level : "—"}
                          </Badge>
                        </TableCell>
                        <TableCell className="max-w-[28rem] truncate font-mono text-[11px]" title={message}>
                          {isV3 ? (
                            <Badge variant="outline" className="mr-1 border-primary/50 text-[10px] text-primary">
                              v3_source_priced
                            </Badge>
                          ) : null}
                          {message || "—"}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
