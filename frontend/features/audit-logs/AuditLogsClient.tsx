"use client";

/**
 * AuditLogsClient — client-side, session-gated audit log viewer (A-01).
 *
 * Previously /audit-logs was an SSR server component that injected
 * process.env.ARBX_ADMIN_TOKEN, publishing admin rows (full IPs + emails) to
 * anonymous visitors. This component fetches via the httpOnly session cookie
 * (getValidated credentials:"include"): no session → admin gate (never rows),
 * session → rows (with IP/email already redacted at the edge).
 *
 * R8 fail-honest: loading / gate / error / empty / rows are explicit states.
 * R1: all non-determinism (cookie read, fetch) lives here in the client.
 */

import * as React from "react";
import { AlertCircleIcon, LockIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { hasAdminSession } from "@/lib/admin-token";
import { getAuditLogs } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";
import type { AuditLogRow } from "@/lib/schemas";
import { AuditLogsEmpty } from "@/features/audit-logs/AuditLogsEmpty";
import { AuditLogsTable } from "@/features/audit-logs/AuditLogsTable";

interface Props {
  limit: number;
  action?: string;
  actor?: string;
  targetKind?: string;
}

type State =
  | { kind: "loading" }
  | { kind: "gate" }
  | { kind: "error"; error: string }
  | { kind: "empty" }
  | { kind: "rows"; items: AuditLogRow[]; ts: string; count: number };

export function AuditLogsClient({ limit, action, actor, targetKind }: Props) {
  const [state, setState] = React.useState<State>({ kind: "loading" });

  React.useEffect(() => {
    // No httpOnly session cookie → never attempt the admin-gated fetch; show gate.
    if (!hasAdminSession()) {
      setState({ kind: "gate" });
      return;
    }
    let cancelled = false;
    void (async () => {
      const r = await getAuditLogs(limit, undefined, action, actor, targetKind);
      if (cancelled) return;
      if (r.ok) {
        const items = r.data.items;
        setState(
          items.length === 0
            ? { kind: "empty" }
            : { kind: "rows", items, ts: r.data.ts, count: items.length },
        );
      } else {
        const authFail = /401|missing_admin_token/i.test(r.error);
        setState(authFail ? { kind: "gate" } : { kind: "error", error: r.error });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [limit, action, actor, targetKind]);

  return (
    <>
      <PageHeader
        title="Audit Logs"
        lede="Immutable record of administrative and edge auth actions. Strict append-only architecture. Admin session required; IPs are truncated (/48) and actors hashed."
        meta={state.kind === "rows" ? [`${state.count} rows fetched`, `snapshot ${fmtTime(state.ts)}`] : undefined}
        showRefresh
      />

      {state.kind === "loading" && <p className="text-sm text-muted-foreground">Loading audit logs…</p>}

      {state.kind === "gate" && (
        <FocusOnMount>
          <Alert data-testid="audit-logs-gate">
            <LockIcon />
            <AlertTitle>Admin session required</AlertTitle>
            <AlertDescription>
              Audit logs are admin-gated. <a className="underline" href="/admin/signin">Sign in</a>{" "}
              (or unlock a session at <a className="underline" href="/killswitch">/killswitch</a>) to view
              them. Anonymous visitors never receive rows, IPs, or emails.
            </AlertDescription>
          </Alert>
        </FocusOnMount>
      )}

      {state.kind === "error" && (
        <FocusOnMount>
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>edge error</AlertTitle>
            <AlertDescription>
              <code className="font-mono text-xs">{state.error}</code>
            </AlertDescription>
          </Alert>
        </FocusOnMount>
      )}

      {state.kind === "empty" && (
        <div data-testid="audit-logs-empty">
          <AuditLogsEmpty />
        </div>
      )}

      {state.kind === "rows" && (
        <div data-testid="audit-logs-panel">
          <AuditLogsTable items={state.items} />
        </div>
      )}
    </>
  );
}
