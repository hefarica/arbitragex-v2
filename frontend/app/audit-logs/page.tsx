import { AuditLogsClient } from "@/features/audit-logs/AuditLogsClient";

// A-01: this page no longer fetches audit rows server-side (which required
// injecting process.env.ARBX_ADMIN_TOKEN and leaked admin rows to anonymous
// visitors). It is now a thin Server Component that forwards search params to a
// client component; the client fetches via the httpOnly session cookie.
export const dynamic = "force-dynamic";
export const revalidate = 0;

function firstParam(v: string | string[] | undefined): string | undefined {
  if (Array.isArray(v)) return v[0];
  return v;
}

export default function AuditLogsPage({
  searchParams,
}: {
  searchParams: { [key: string]: string | string[] | undefined };
}) {
  const limitRaw = firstParam(searchParams.limit);
  const limit = limitRaw ? Number(limitRaw) : 100;
  const action = firstParam(searchParams.action);
  const actor = firstParam(searchParams.actor);
  const targetKind = firstParam(searchParams.target_kind);

  return (
    <AuditLogsClient
      limit={Number.isFinite(limit) ? limit : 100}
      action={action}
      actor={actor}
      targetKind={targetKind}
    />
  );
}
