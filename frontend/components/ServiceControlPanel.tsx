"use client";

/**
 * FE-5 — ServiceControlPanel
 *
 * Per-service start/stop controls dispatched to the edge.
 *
 * SERVICE-CTRL-01 (2026-09-01): the endpoint POST /api/v1/admin/services/:name/
 * {start,stop} IS implemented (edge proxy → api-server → socket-proxy → docker).
 * Errors are TYPED JSON, and this panel must translate them truthfully instead
 * of guessing from the status code — the previous "every 404 = endpoint not
 * implemented" message masked a real production fault (socket-proxy DOCKER_GID
 * drift surfaced as 404 container_not_found while all six services were
 * healthy). Error contract:
 *   401 missing_admin_token / unauthorized   → admin session (re-sign-in)
 *   400 invalid_action                       → edge rejected the action param
 *   400 service_not_controllable             → name outside the allowlist
 *   404 container_not_found                  → proxy healthy, no such container
 *   501 not_implemented                      → ARBX_SERVICE_CONTROL flag off
 *   502 control_plane_error                  → socket-proxy/daemon failure
 *
 * R1 (Mounted Snapshot Pattern): No localStorage / window access outside
 * useEffect. Component is pure client — imported only by Client Components.
 */

import { useState, useCallback } from "react";
import { toast } from "sonner";
import { Square, Play, Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { getApiBaseUrl } from "@/lib/api-client";
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

/** Error body contract of the edge + api-server service-control routes. */
interface ServiceControlApiError {
  error?: string;
  service?: string;
  message?: string;
  detail?: string;
  compose_project?: string;
  /** api-server 401 shape. */
  source?: string;
  /** edge 400 invalid_action shape. */
  valid_actions?: string[];
}

/**
 * Translate a failed service-control response into an actionable, truthful
 * message. Exported for unit tests (SERVICE-CTRL-01 regression guard).
 */
export function describeControlFailure(
  status: number,
  body: ServiceControlApiError | null,
): string {
  const code = body?.error ?? null;
  switch (`${status}:${code ?? ""}`) {
    case "401:missing_admin_token":
    case "401:unauthorized":
      return "Admin session missing or expired — sign in again at /admin/signin. " +
        "The admin token lives in the httpOnly arbx_admin_session cookie; this UI never reads it.";
    case "400:invalid_action":
      return "Invalid action — the edge only accepts start|stop.";
    case "400:service_not_controllable":
      return `${body?.service ?? "service"} is not in the control allowlist (ARBX_SERVICE_CONTROL_ALLOWLIST on api-server).`;
    case "404:container_not_found":
      return `Control plane is healthy but no container matched ${body?.service ?? "this service"} ` +
        `(compose project "${body?.compose_project ?? "?"}") — check the service is deployed and its ` +
        `com.docker.compose.service/project labels (or COMPOSE_PROJECT_NAME) match.`;
    case "501:not_implemented":
      return "Service control is disabled: set ARBX_SERVICE_CONTROL=on in api-server's environment (off is the shadow-safe default).";
    case "502:control_plane_error":
      return `Control plane (socket-proxy) failed${body?.detail ? `: ${body.detail}` : ""} — ` +
        "check socket-proxy logs and DOCKER_GID (host docker group gid) in the VPS .env.";
    default: {
      const raw = body ? JSON.stringify(body).slice(0, 200) : "";
      return `HTTP ${status}${raw ? `: ${raw}` : " (non-JSON error body)"}`;
    }
  }
}

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

  if (res.ok) return;
  // Typed JSON contract (see header) — parse before judging; a bare status
  // code cannot distinguish container_not_found from a broken proxy.
  const body = (await res.json().catch(() => null)) as ServiceControlApiError | null;
  throw new Error(describeControlFailure(res.status, body));
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

  const edgeUrl = getApiBaseUrl();

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
            Requires an admin session and <code>ARBX_SERVICE_CONTROL=on</code> —
            501 = flag off, 404 = container not resolvable, 502 = control plane
            (socket-proxy) failure.
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
