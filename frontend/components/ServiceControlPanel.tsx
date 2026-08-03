"use client";

/**
 * FE-5 — ServiceControlPanel
 *
 * Per-service start/stop controls dispatched to the edge.
 *
 * R8 (Fail-Honest): If the api-server endpoint returns 404 (route missing) or
 * 501 (admin-gated stub, control-plane not wired) the component surfaces an
 * actionable toast rather than silently swallowing the error. The backend
 * endpoint (POST /api/v1/admin/services/:name/stop|start) is not yet
 * implemented; that gap is honest and visible to the operator.
 *
 * R1 (Mounted Snapshot Pattern): No localStorage / window access outside
 * useEffect. Component is pure client — imported only by Client Components.
 */

import { useState, useCallback } from "react";
import { toast } from "sonner";
import { Square, Play, Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// ─── Types ────────────────────────────────────────────────────────────────────

/** The canonical list of ArbX managed services. */
const MANAGED_SERVICES = [
  "searcher-rs",
  "sim-ctl",
  "relays-client",
  "recon",
  "token-enricher",
] as const;

type ServiceName = (typeof MANAGED_SERVICES)[number];
type ControlAction = "start" | "stop";
type PendingKey = `${ServiceName}:${ControlAction}`;

// ─── API helper ──────────────────────────────────────────────────────────────

async function dispatchServiceControl(
  edgeUrl: string,
  service: ServiceName,
  action: ControlAction,
): Promise<void> {
  const res = await fetch(
    `${edgeUrl}/api/v1/admin/services/${service}/${action}`,
    {
      method: "POST",
      credentials: "include",
      headers: { "content-type": "application/json", accept: "application/json" },
      signal: AbortSignal.timeout(8_000),
    },
  );

  if (res.status === 404) {
    throw new Error(
      `endpoint not yet implemented (backend Sprint TBD) — POST /api/v1/admin/services/${service}/${action} → 404`,
    );
  }
  if (res.status === 501) {
    // The api-server mounts this as an admin-gated 501 stub by design: the
    // control plane (docker socket vs systemd unit vs k8s) is an operator
    // strategy decision that is not yet wired. Surface an actionable runbook
    // hint instead of the raw stub JSON body (R8 fail-honest, not silent).
    throw new Error(
      `Service control plane not wired (HTTP 501). ${service} ${action} must be performed ` +
        `on the host via the deploy runbook (docker compose / systemd). This button activates ` +
        `once POST /api/v1/admin/services/:name/${action} replaces the stub.`,
    );
  }
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status}${text ? `: ${text.slice(0, 200)}` : ""}`);
  }
}

// ─── Component ────────────────────────────────────────────────────────────────

interface ServiceControlPanelProps {
  /**
   * Live service status map from the last /status poll, keyed by service name.
   * Keys present in MANAGED_SERVICES that are also in this map display a
   * live status badge. Unknown services show "UNKNOWN".
   */
  liveStatus?: Record<string, { ok: boolean }>;
  /** Called after a successful dispatch so the parent can refetch /status and
   * flip the badge immediately (otherwise it waits for the next poll). */
  onAfterControl?: () => void;
}

export function ServiceControlPanel({ liveStatus = {}, onAfterControl }: ServiceControlPanelProps) {
  const [pending, setPending] = useState<Set<PendingKey>>(new Set());

  const edgeUrl =
    process.env.NEXT_PUBLIC_EDGE_URL ?? "";

  const handleControl = useCallback(
    async (service: ServiceName, action: ControlAction) => {
      // Stop can interrupt the live pipeline (e.g. searcher-rs) — require an
      // explicit confirm. Start is safe/beneficial and needs no confirm.
      if (action === "stop") {
        const ok = window.confirm(
          `Detener ${service} puede interrumpir el pipeline. ¿Continuar?`,
        );
        if (!ok) return;
      }
      const key: PendingKey = `${service}:${action}`;
      setPending((prev) => new Set(prev).add(key));
      try {
        await dispatchServiceControl(edgeUrl, service, action);
        toast.success(`${service} ${action} dispatched`, {
          description: `The ${action} signal was accepted by the edge.`,
        });
        onAfterControl?.();
      } catch (e) {
        const err = e as Error;
        if (err.name === "AbortError") {
          toast.error(`${service} ${action} timed out`, {
            description: "Edge did not respond within 8s.",
          });
        } else {
          toast.error(`${service} ${action} failed`, {
            description: err.message,
          });
        }
      } finally {
        setPending((prev) => {
          const next = new Set(prev);
          next.delete(key);
          return next;
        });
      }
    },
    [edgeUrl, onAfterControl],
  );

  return (
    <Card>
      <CardHeader className="pb-3">
        <h2 className="text-base font-semibold">Service Controls</h2>
        <p className="text-xs text-muted-foreground">
          Start/stop managed ArbX services via the edge control plane. Actions are
          admin-gated, allowlist-restricted, and audit-logged.{" "}
          <span className="text-warning font-medium">
            Requires <code>ARBX_SERVICE_CONTROL=on</code> — returns 501 while the
            flag is off, 404 if the edge route is absent.
          </span>
        </p>
      </CardHeader>
      <CardContent className="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Service</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {MANAGED_SERVICES.map((svc) => {
              const statusEntry = liveStatus[svc];
              const isUp = statusEntry?.ok ?? null;
              const stopKey: PendingKey = `${svc}:stop`;
              const startKey: PendingKey = `${svc}:start`;
              const stopPending = pending.has(stopKey);
              const startPending = pending.has(startKey);

              return (
                <TableRow key={svc}>
                  <TableCell className="font-mono text-sm">{svc}</TableCell>
                  <TableCell>
                    {isUp === null ? (
                      <Badge variant="secondary">UNKNOWN</Badge>
                    ) : (
                      <Badge variant={isUp ? "success" : "destructive"}>
                        <span
                          className={`size-1.5 rounded-full ${isUp ? "bg-success" : "bg-destructive"}`}
                          aria-hidden
                        />
                        {isUp ? "UP" : "DOWN"}
                      </Badge>
                    )}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-2">
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={startPending || stopPending}
                        onClick={() => handleControl(svc, "start")}
                        aria-label={`Start ${svc}`}
                        className="gap-1.5 text-success border-success/40 hover:bg-success/10 hover:text-success"
                      >
                        {startPending ? (
                          <Loader2 size={12} className="animate-spin" />
                        ) : (
                          <Play size={12} />
                        )}
                        Start
                      </Button>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={stopPending || startPending}
                        onClick={() => handleControl(svc, "stop")}
                        aria-label={`Stop ${svc}`}
                        className="gap-1.5 text-destructive border-destructive/40 hover:bg-destructive/10 hover:text-destructive"
                      >
                        {stopPending ? (
                          <Loader2 size={12} className="animate-spin" />
                        ) : (
                          <Square size={12} />
                        )}
                        Stop
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}
