/**
 * POST /admin/alertmanager/webhook
 *
 * Receives Prometheus Alertmanager webhook POSTs and persists each alert to the
 * audit_log (migration 011) via the injected writeAudit. Shadows the A8 admin-
 * gated stub in stubs.ts (mounted earlier in index.ts so Express dispatches this
 * real handler). Admin-token gated, same as the stub — probing must not be
 * unauthenticated.
 *
 * R8 fail-honest:
 *   - 401 (requireAdminToken) before anything else.
 *   - malformed payload (no `alerts` array) → 400.
 *   - writeAudit no-ops if the DB pool is null (it guards internally); per-alert
 *     failures are logged and counted in `persisted` rather than hidden.
 */

import type { Request, Response, RequestHandler } from "express";

interface AmAlert {
  status?: string;
  labels?: Record<string, string>;
  annotations?: Record<string, string>;
  startsAt?: string;
  endsAt?: string;
}

interface Deps {
  requireAdminToken: (expected: string) => RequestHandler;
  adminToken: string;
  writeAudit: (
    action: string,
    actor: string,
    targetKind: string | null,
    targetId: string | null,
    before: unknown,
    after: unknown,
    ip: string | null,
    traceId: string | null,
    userAgent?: string | null,
  ) => Promise<void>;
  logger: {
    warn: (obj: object, msg?: string) => void;
    info: (obj: object, msg?: string) => void;
  };
}

export function mountAlertmanagerWebhook(app: import("express").Express, deps: Deps): void {
  app.post(
    "/admin/alertmanager/webhook",
    deps.requireAdminToken(deps.adminToken),
    async (req: Request, res: Response): Promise<void> => {
      const body = (req.body ?? {}) as { alerts?: unknown };
      const alerts = Array.isArray(body.alerts) ? (body.alerts as AmAlert[]) : null;
      if (!alerts) {
        res.status(400).json({ error: "invalid_payload", detail: "expected { alerts: [...] }" });
        return;
      }

      const ip = req.ip ?? null;
      const traceId = (req as Request & { traceId?: string }).traceId ?? null;
      const ua = req.header("user-agent") ?? null;

      let persisted = 0;
      for (const alert of alerts) {
        const status = typeof alert?.status === "string" ? alert.status : "unknown";
        const alertname = alert?.labels?.["alertname"] ?? null;
        try {
          await deps.writeAudit(
            `alert.${status}`,
            "alertmanager",
            "alert",
            alertname,
            null,
            alert,
            ip,
            traceId,
            ua,
          );
          persisted += 1;
        } catch (e) {
          deps.logger.warn({
            event: "alertmanager_webhook.audit_failed",
            alertname,
            err: (e as Error).message,
          });
        }
      }

      deps.logger.info(
        { event: "alertmanager_webhook.received", count: alerts.length, persisted },
        "alertmanager alerts ingested to audit_log",
      );
      res.status(200).json({ received: true, count: alerts.length, persisted });
    },
  );
}
