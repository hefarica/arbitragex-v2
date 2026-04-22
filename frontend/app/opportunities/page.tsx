import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { PageHeader } from "@/components/page-header";
import { OpportunitiesTable } from "@/features/opportunities/OpportunitiesTable";
import { OpportunitiesEmpty } from "@/features/opportunities/OpportunitiesEmpty";
import { getOpportunitiesLive } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function OpportunitiesPage() {
  const res = await getOpportunitiesLive(100);

  return (
    <>
      <PageHeader
        title="Live opportunities"
        lede="Recent candidates detected from the mempool. Nothing here is synthesized; if the scanner has no RPC, the list stays empty — that's honest."
        meta={res.ok ? [
          `${res.data.count} rows`,
          `window: ${res.data.window}`,
          `snapshot ${fmtTime(res.data.ts)}`,
        ] : undefined}
        showRefresh
      />

      {!res.ok ? (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>edge error</AlertTitle>
          <AlertDescription>
            <code className="font-mono text-xs">{res.error}</code>
          </AlertDescription>
        </Alert>
      ) : res.data.count === 0 ? (
        <OpportunitiesEmpty />
      ) : (
        <OpportunitiesTable items={res.data.items} />
      )}
    </>
  );
}
