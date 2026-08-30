import * as React from "react";
import { GitCommitHorizontalIcon, RocketIcon } from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { StatusResponse } from "@/lib/api-client";

/**
 * DeploymentCard — AUDIT-2026-08-29 P0-1 (deployment coherence).
 *
 * Answers the audit's first question — "WHAT SHA IS PRODUCTION RUNNING?" —
 * with runtime data from the api-server /status `deploy` block: the exact
 * commit the deploy workflow anchored + verified on the VPS (G4
 * deploy-veraz: `git reset --hard TARGET_SHA` before build) plus the run id
 * and timestamp. All services deploy from that same anchored checkout.
 *
 * R8 fail-honest:
 *   - deploy absent (api-server older than this field, deploy-skew window)
 *     → "not reported by this api-server build" — never guessed.
 *   - field === "unknown" (manual `docker compose up` without the workflow
 *     exports) → shown verbatim with an explanation.
 *   - Never a fabricated SHA; no client-side git inference.
 */
export function DeploymentCard({ status }: { status: StatusResponse }) {
  const d = status.deploy;
  const absent = d === undefined;
  const sha = d?.sha ?? null;
  const id = d?.id ?? null;
  const at = d?.at ?? null;
  const unstamped = !absent && (sha === "unknown" || sha === null);
  const short = sha && sha !== "unknown" ? sha.slice(0, 7) : null;
  const runUrl =
    id && /^\d+$/.test(id)
      ? `https://github.com/hefarica/arbitragex-v2/actions/runs/${id}`
      : null;

  return (
    <Card data-slot="deployment-card">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <RocketIcon className="size-4 text-info" aria-hidden="true" />
          Deployment
        </CardTitle>
        <CardDescription>
          The commit the deploy workflow anchored + verified on the VPS before building this stack.
          Every service (searcher, sim-ctl, edge, api-server, frontend) deploys from that same
          checkout — image layers may be cache-reused when a service&apos;s source is unchanged.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {absent ? (
          <p className="font-mono text-xs text-muted-foreground">
            not reported by this api-server build (deploy.skew — predates AUDIT-2026-08-29 P0-1)
          </p>
        ) : unstamped ? (
          <p className="font-mono text-xs text-warning">
            unknown — stack was created by a manual <code>compose up</code> without the workflow
            deploy exports. The running SHA cannot be asserted from inside the stack (R8: reported
            verbatim, never guessed).
          </p>
        ) : (
          <div className="space-y-2">
            <div className="flex flex-wrap items-baseline gap-2">
              <GitCommitHorizontalIcon className="size-4 text-muted-foreground" aria-hidden="true" />
              <span
                className="break-all font-mono text-sm font-semibold"
                data-testid="deploy-sha-full"
              >
                {sha}
              </span>
              {short && (
                <span className="rounded border bg-muted/40 px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground">
                  {short}
                </span>
              )}
            </div>
            <div className="flex flex-wrap gap-x-6 gap-y-1 font-mono text-[11px] text-muted-foreground">
              <span data-testid="deploy-run">
                run:{" "}
                {runUrl ? (
                  <a
                    href={runUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="underline decoration-dotted underline-offset-2 hover:text-foreground"
                  >
                    {id}
                  </a>
                ) : (
                  (id ?? "—")
                )}
              </span>
              <span data-testid="deploy-at">at: {at ?? "—"}</span>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
