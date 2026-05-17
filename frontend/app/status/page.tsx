import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { StatusClient } from "@/app/status/StatusClient";
import { getStatus } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";
import { classifyEdgeResult } from "@/lib/edge-health";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function StatusPage() {
  const res = await getStatus();

  if (!res.ok) {
    // Doctrine: distinguish "edge unreachable" (we never saw an HTTP
    // status) from "upstreams degraded" (edge responded with 5xx).
    // The literal "edge unreachable" string is reserved for the
    // former; the spec enforces this via tests/e2e/smoke.spec.ts.
    const health = classifyEdgeResult(res);
    return (
      <>
        <PageHeader
          title="System status"
          lede="Live operator view of every hot-path service + kill-switch state."
          showRefresh
        />
        <FocusOnMount>
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>{health.title}</AlertTitle>
            <AlertDescription>
              <code className="rounded bg-destructive/10 px-1.5 py-0.5 font-mono text-xs">{health.diagnostic}</code>
              <p className="mt-2">
                {health.kind === "EDGE_UNREACHABLE"
                  ? "This view displays only real edge data. There is no fallback. If the edge gateway is down, this page shows the error — it never synthesizes values."
                  : "The edge gateway is reachable but an upstream dependency returned an error. This view shows the verbatim diagnostic — values are never synthesized."}
              </p>
            </AlertDescription>
          </Alert>
        </FocusOnMount>
      </>
    );
  }

  const s = res.data;

  return (
    <>
      <PageHeader
        title="System status"
        lede="Live operator view of every hot-path service + kill-switch state. Auto-refreshes every 5s."
        meta={[`env: ${s.env}`, `api-server v${s.version}`, `as of ${fmtTime(s.ts)}`]}
        showRefresh
      />
      {/* FE-11: StatusClient polls getStatus every 5s; uses R1 Mounted Snapshot Pattern. */}
      <StatusClient initialStatus={s} />
    </>
  );
}
