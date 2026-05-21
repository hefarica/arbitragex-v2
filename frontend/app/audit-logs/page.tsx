import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { AuditLogsTable } from "@/features/audit-logs/AuditLogsTable";
import { AuditLogsEmpty } from "@/features/audit-logs/AuditLogsEmpty";
import { getAuditLogs } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function AuditLogsPage({
  searchParams,
}: {
  searchParams: { [key: string]: string | string[] | undefined };
}) {
  const limit = searchParams.limit ? Number(searchParams.limit) : 100;
  const action = typeof searchParams.action === "string" ? searchParams.action : undefined;
  const actor = typeof searchParams.actor === "string" ? searchParams.actor : undefined;
  const targetKind = typeof searchParams.target_kind === "string" ? searchParams.target_kind : undefined;

  const res = await getAuditLogs(limit, undefined, action, actor, targetKind);

  return (
    <>
      <PageHeader
        title="Audit Logs"
        lede="Immutable record of administrative and edge auth actions. Strict append-only architecture."
        meta={res.ok ? [
          `${res.data.items.length} rows fetched`,
          `snapshot ${fmtTime(res.data.ts)}`,
        ] : undefined}
        showRefresh
      />

      {!res.ok ? (
        <FocusOnMount>
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>edge error</AlertTitle>
            <AlertDescription>
              <code className="font-mono text-xs">{res.error}</code>
            </AlertDescription>
          </Alert>
        </FocusOnMount>
      ) : res.data.items.length === 0 ? (
        <AuditLogsEmpty />
      ) : (
        <AuditLogsTable items={res.data.items} />
      )}
    </>
  );
}
