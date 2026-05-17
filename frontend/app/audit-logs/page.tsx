import { PageHeader } from "@/components/page-header";
import { EdgeHealthBanner } from "@/components/edge-health-banner";
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
        <EdgeHealthBanner result={res} subject="audit logs" />
      ) : res.data.items.length === 0 ? (
        <AuditLogsEmpty />
      ) : (
        <AuditLogsTable items={res.data.items} />
      )}
    </>
  );
}
