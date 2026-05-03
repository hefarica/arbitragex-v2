import { AlertCircleIcon, InfoIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { TradingConfigForm } from "@/components/trading-config-form";
import { getTradingConfig } from "@/lib/api-client";

// Server Component — pulls initial snapshot from edge, passes to client form.
// Mounted Snapshot Pattern (R1): all non-deterministic state lives in the
// client component; this page is pure SSR over a serialised snapshot.
export const dynamic = "force-dynamic";
export const revalidate = 0;

const DEFAULT_CHAIN_ID = 1;

export default async function TradingConfigPage() {
  const res = await getTradingConfig(DEFAULT_CHAIN_ID);

  if (!res.ok) {
    return (
      <>
        <PageHeader
          title="Trading config"
          lede="Per-chain operator-tunable strategy parameters. Live in Redis with <1s hot-reload to searcher-rs."
          showRefresh
        />
        <FocusOnMount>
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>edge error</AlertTitle>
            <AlertDescription>
              <code className="font-mono text-xs">{res.error}</code>
            </AlertDescription>
          </Alert>
        </FocusOnMount>
      </>
    );
  }

  const initial = res.data;

  return (
    <>
      <PageHeader
        title="Trading config"
        lede="Capital, token allowlist, profit thresholds, gas strategy. searcher-rs reads these in ≤1s; the system stays idle for any chain without a row."
        showRefresh
      />

      {!initial.configured && (
        <Alert className="mb-6">
          <InfoIcon />
          <AlertTitle>Chain {initial.chain_id} is not configured yet</AlertTitle>
          <AlertDescription>
            The scanner is observing pending transactions but is not scoring opportunities.
            Submit the form below to seed strategy parameters and start net-profit gating.
          </AlertDescription>
        </Alert>
      )}

      <TradingConfigForm chainId={DEFAULT_CHAIN_ID} initial={initial} />
    </>
  );
}
