/**
 * FE-0059 (§64) → DAPP-DEPLOYLOCK-TRUTH (2026-09-04): the deploy-pipeline
 * console reflects the REAL workflow state.
 *
 * The 2026-08-23 PROTOCOLO ABSOLUTO ran its terminal WP-F gate on 2026-08-25
 * (PR #464 → main cfafa012, deploy verified); since 2026-08-29 the operative
 * protocol is the operator's PR-per-anomaly flow. Rendering "DEPLOY LOCKED"
 * after that would misstate reality (RULE 00 covers stale state too).
 *
 * Pure component over DEPLOY_LOCK (data.ts): no runtime claims. Each reason
 * keeps its ORIGINAL unlock condition + source anchor, and adds a STATIC,
 * DATED evidence status (verifiable against git/API/probes) — the live
 * per-reason status is nivel-(b) and is declared, never fabricated.
 */
import * as React from "react";
import { ShieldCheckIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { DEPLOY_LOCK, type LockReasonStatus } from "./data";

const STATUS_LABEL: Record<LockReasonStatus, string> = {
  satisfied: "CUMPLIDA",
  superseded: "SUPERADA",
  "operator-pending": "PENDIENTE OPERADOR",
};

function statusVariant(s: LockReasonStatus): "success" | "secondary" | "warning" {
  if (s === "satisfied") return "success";
  if (s === "superseded") return "secondary";
  return "warning";
}

export function DeployLockBanner(): React.ReactElement {
  return (
    <section className="mb-8" data-testid="deploy-lock-banner">
      <Card className="border-success/40">
        <CardHeader>
          <CardTitle className="flex flex-wrap items-center gap-2">
            <ShieldCheckIcon className="size-4 text-success" aria-hidden />
            PROTOCOLO DEPLOY — GATE FINAL EJECUTADO
            <Badge variant="outline" className="border-success/40 bg-success/10 text-success text-xs">
              protocolo desde {DEPLOY_LOCK.since}
            </Badge>
            <Badge variant="success" className="text-xs">
              resuelto {DEPLOY_LOCK.resolved_at}
            </Badge>
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            {DEPLOY_LOCK.protocol}. {DEPLOY_LOCK.current_flow}
          </p>

          <ul className="space-y-2" data-testid="deploy-lock-reasons">
            {DEPLOY_LOCK.reasons.map((r) => (
              <li
                key={r.id}
                className="rounded-md border border-border/60 bg-muted/20 p-3"
                data-testid={`deploy-lock-reason-${r.id}`}
                data-status={r.status}
              >
                <p className="flex flex-wrap items-center gap-2 text-sm font-medium">
                  {r.label}
                  <Badge variant={statusVariant(r.status)} className="text-xs">
                    {STATUS_LABEL[r.status]}
                  </Badge>
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  desbloquea cuando: {r.unlock_condition}
                </p>
                <p className="mt-1 font-mono text-[11px] text-muted-foreground/80">
                  fuente: {r.source}
                </p>
                <p className="mt-1 text-xs text-foreground/80">
                  evidencia ({r.status_date}): {r.status_evidence}
                </p>
              </li>
            ))}
          </ul>

          {/* nivel-(b): the live per-reason status does not exist on any wire —
              what renders above is STATIC, DATED evidence (verifiable), never a
              fabricated live claim. */}
          <p className="text-xs text-muted-foreground" data-testid="deploy-lock-live-status">
            {DEPLOY_LOCK.live_status}
          </p>
        </CardContent>
      </Card>
    </section>
  );
}
