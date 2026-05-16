import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { classifyEdgeResult, type EdgeResult } from "@/lib/edge-health";

/**
 * Unified honest-failure banner.
 *
 * Replaces the legacy "edge error" AlertTitle that every page used to
 * render verbatim regardless of whether the edge was reachable. The
 * spec in tests/e2e/smoke.spec.ts and tests/e2e/rpc-down.spec.ts
 * forbids the strings "edge unreachable" and "edge error:" appearing
 * on a page where the edge IS reachable; this component routes through
 * `classifyEdgeResult` so the banner copy reflects the true root cause.
 *
 * Usage:
 *
 *   if (!res.ok) {
 *     return (
 *       <>
 *         <PageHeader title="..." />
 *         <EdgeHealthBanner result={res} subject="onboarding progress" />
 *       </>
 *     );
 *   }
 */
export function EdgeHealthBanner({
  result,
  subject,
  focus = true,
}: {
  result: EdgeResult<unknown>;
  /** Short noun phrase: "trading config", "executions", "onboarding progress". */
  subject?: string;
  /** Whether to autofocus the banner on mount (default true). */
  focus?: boolean;
}) {
  if (result.ok) return null;
  const health = classifyEdgeResult(result);

  const body = (
    <Alert variant="destructive">
      <AlertCircleIcon />
      <AlertTitle>{health.title}</AlertTitle>
      <AlertDescription>
        {subject ? <p className="mb-2 text-sm">Could not load {subject}.</p> : null}
        <code className="rounded bg-destructive/10 px-1.5 py-0.5 font-mono text-xs">
          {health.diagnostic}
        </code>
        <p className="mt-2 text-sm">
          {health.kind === "EDGE_UNREACHABLE"
            ? "The frontend could not reach the edge gateway. This view shows the diagnostic verbatim — values are never synthesized."
            : health.kind === "EDGE_REACHABLE_SCHEMA_DRIFT"
              ? "The edge gateway responded with a payload that violates the contract. This is surfaced verbatim instead of rendered."
              : "The edge gateway is reachable but an upstream dependency (RPC, provider, indexer) returned an error. This view shows the diagnostic verbatim — values are never synthesized."}
        </p>
      </AlertDescription>
    </Alert>
  );

  return focus ? <FocusOnMount>{body}</FocusOnMount> : body;
}
