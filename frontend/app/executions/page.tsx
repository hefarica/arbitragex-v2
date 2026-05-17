import { PageHeader } from "@/components/page-header";
import { EdgeHealthBanner } from "@/components/edge-health-banner";
import { ExecutionsClient } from "@/features/executions/ExecutionsClient";
import { getExecutionsRecent } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function ExecutionsPage() {
  // FE-8: initial snapshot with first 50 rows; client handles Load More.
  const res = await getExecutionsRecent(50);

  return (
    <>
      <PageHeader
        title="Recent executions"
        lede="Bundle submissions and their outcomes. Paper-mode is ON until S9; real relays only record submitted / not_implemented until the safety rail is flipped."
        meta={res.ok ? [
          `${res.data.count} rows (initial 50)`,
          `snapshot ${fmtTime(res.data.ts)}`,
        ] : undefined}
        showRefresh
      />

      {!res.ok ? (
        <EdgeHealthBanner result={res} subject="recent executions" />
      ) : (
        <ExecutionsClient
          initialItems={res.data.items}
          initialTotal={50}
        />
      )}
    </>
  );
}
