import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { PageHeader } from "@/components/page-header";
import { ExecutionsTable } from "@/features/executions/ExecutionsTable";
import { ExecutionsEmpty } from "@/features/executions/ExecutionsEmpty";
import { getExecutionsRecent } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function ExecutionsPage() {
  const res = await getExecutionsRecent(100);

  return (
    <>
      <PageHeader
        title="Recent executions"
        lede="Bundle submissions and their outcomes. Paper-mode is ON until S9; real relays only record submitted / not_implemented until the safety rail is flipped."
        meta={res.ok ? [
          `${res.data.count} rows`,
          `snapshot ${fmtTime(res.data.ts)}`,
        ] : undefined}
        showRefresh
      />

      {!res.ok ? (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>edge error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{res.error}</code></AlertDescription>
        </Alert>
      ) : res.data.count === 0 ? (
        <ExecutionsEmpty />
      ) : (
        <ExecutionsTable items={res.data.items} />
      )}
    </>
  );
}
