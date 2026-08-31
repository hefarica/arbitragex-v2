/**
 * /agent-insights — ArbitrageX Agent Teams Status (R1 Mounted-Snapshot).
 *
 * Surfaces the 17 agent-team verdicts from /api/agents/status.
 * Pure Server Component: fetches initial snapshot at SSR time, passes to
 * AgentInsightsClient for live polling. Fail-honest: null snapshot renders
 * explicit unavailable state — never fabricated data.
 *
 * Safety posture: observe-only. No capital, no execution, no broadcast.
 */
import { AlertCircleIcon, BotIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { getApiBaseUrl } from "@/lib/api-client";
import { AgentInsightsClient } from "@/features/agent-insights/AgentInsightsClient";

export const metadata = {
  title: "Agent Insights — ArbitrageX",
};

export const dynamic = "force-dynamic";
export const revalidate = 0;

async function getInitialAgentStatus(): Promise<unknown | null> {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  try {
    const res = await fetch(`${EDGE_URL}/api/agents/status`, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!res.ok) return null;
    return await res.json();
  } catch {
    return null;
  }
}

export default async function AgentInsightsPage() {
  const initialData = await getInitialAgentStatus();

  if (!initialData) {
    return (
      <>
        <PageHeader
          title="Agent Insights"
          lede="ArbitrageX agent-team verdicts — dated workspace-verified audit ledger."
          showRefresh
        />
        <FocusOnMount>
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>edge / upstream failure</AlertTitle>
            <AlertDescription>
              <p className="mt-1 text-sm">
                Could not reach <code className="rounded bg-destructive/10 px-1 py-0.5 font-mono text-xs">/api/agents/status</code>.
                The edge or api-server may be temporarily unavailable.
              </p>
            </AlertDescription>
          </Alert>
        </FocusOnMount>
      </>
    );
  }

  return (
    <>
      <PageHeader
        title="Agent Insights"
        lede="ArbitrageX agent-team verdicts — dated workspace-verified audit ledger. Observe-only."
        showRefresh
      />
      <AgentInsightsClient initialData={initialData} />
    </>
  );
}
