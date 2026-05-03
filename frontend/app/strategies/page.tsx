/**
 * Sprint 2 Task 2.3 — /strategies Server Component.
 *
 * Mounted Snapshot Pattern (R1): fetches initial trading_config + strategy
 * catalog at SSR time, hands serialised snapshots to StrategiesClient.
 * The client reads adminToken from the operator's httpOnly session cookie
 * after mount and forwards to all PUT mutations.
 */
import { PageHeader } from "@/components/page-header";
import { getStrategyCatalog, getTradingConfig } from "@/lib/api-client";

import { StrategiesClient } from "./StrategiesClient";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const DEFAULT_CHAIN_ID = 1;

export default async function StrategiesPage() {
  const [cfgRes, catRes] = await Promise.all([
    getTradingConfig(DEFAULT_CHAIN_ID),
    getStrategyCatalog(),
  ]);

  const initialConfig = cfgRes.ok ? cfgRes.data : null;
  const initialCatalog = catRes.ok ? catRes.data.entries : [];
  const initialError = !cfgRes.ok ? cfgRes.error : !catRes.ok ? catRes.error : null;

  return (
    <>
      <PageHeader
        title="Strategies"
        lede="Capital, risk gates, MEV strategy catalog, token allowlist, audit trail. Hot-reloads to searcher in ≤1s on save."
        showRefresh
      />
      <StrategiesClient
        initialConfig={initialConfig}
        initialCatalog={initialCatalog}
        initialError={initialError}
      />
    </>
  );
}
