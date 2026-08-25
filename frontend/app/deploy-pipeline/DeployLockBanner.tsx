/**
 * FE-0059 (§64) — DeployLockBanner: the deploy-pipeline console reflects the
 * REAL workflow state — DEPLOY LOCKED under the operator's PROTOCOLO ABSOLUTO,
 * with the lock REASONS and their unlock conditions.
 *
 * Pure component over DEPLOY_LOCK (data.ts): no runtime claims. `locked: true`
 * is grounded in the operator directive; each reason states its UNLOCK
 * CONDITION + source anchor — the live per-reason status is nivel-(b) and is
 * declared, never fabricated (RULE 00 / §28).
 */
import * as React from "react";
import { LockIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { DEPLOY_LOCK } from "./data";

export function DeployLockBanner(): React.ReactElement {
  return (
    <section className="mb-8" data-testid="deploy-lock-banner">
      <Card className="border-warning/40">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <LockIcon className="size-4 text-warning" aria-hidden />
            DEPLOY LOCKED
            <Badge variant="outline" className="border-warning/40 bg-warning/10 text-warning text-xs">
              desde {DEPLOY_LOCK.since}
            </Badge>
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            {DEPLOY_LOCK.protocol}. {DEPLOY_LOCK.effect}.
          </p>

          <ul className="space-y-2" data-testid="deploy-lock-reasons">
            {DEPLOY_LOCK.reasons.map((r) => (
              <li
                key={r.id}
                className="rounded-md border border-warning/20 bg-warning/5 p-3"
                data-testid={`deploy-lock-reason-${r.id}`}
              >
                <p className="text-sm font-medium">
                  {r.label}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  desbloquea cuando: {r.unlock_condition}
                </p>
                <p className="mt-1 font-mono text-[11px] text-muted-foreground/80">
                  fuente: {r.source}
                </p>
              </li>
            ))}
          </ul>

          {/* nivel-(b): the live per-reason status does not exist on any wire —
              declaring it is the honest rendering; showing one would fabricate. */}
          <p className="text-xs text-muted-foreground" data-testid="deploy-lock-live-status">
            {DEPLOY_LOCK.live_status}
          </p>
        </CardContent>
      </Card>
    </section>
  );
}
