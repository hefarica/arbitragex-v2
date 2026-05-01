/**
 * Fire-and-forget audit event emitter for edge → api-server.
 *
 * Sends auth events (login, logout, lockout, rate-limit) to api-server's
 * /internal/audit/auth endpoint. On failure (timeout, 5xx, network), logs
 * to stderr (Loki captures) and does NOT throw — the operator's request
 * proceeds unaffected.
 *
 * Honesty doctrine: we never synthesize audit rows. If the emit fails,
 * the gap is logged but no fake row is created.
 */

import { createHash } from "node:crypto";
import { auditEmitFailedTotal } from "@arbx/shared";

export type AuditAction =
  | "auth.login_ok"
  | "auth.login_fail"
  | "auth.logout"
  | "auth.lockout_triggered"
  | "auth.rate_limited"
  | "auth.locked_attempt";

export type AuditEmitInput = {
  action: AuditAction;
  actor: string;
  ipAddress: string;
  userAgent?: string | undefined;
  targetId?: string | undefined;
  afterState?: Record<string, unknown> | undefined;
  traceId?: string | undefined;
};

const EMIT_TIMEOUT_MS = 2000;

/**
 * Compute a non-reversible fingerprint of the admin token for audit actor field.
 * Returns `"tok:<first 8 hex chars of SHA-256>"`.
 */
export function tokenFingerprint(token: string): string {
  const hash = createHash("sha256").update(token).digest("hex");
  return `tok:${hash.substring(0, 8)}`;
}

/**
 * Fire-and-forget emit to api-server's /internal/audit/auth.
 * Never throws — logs to stderr on failure.
 */
export async function emitAuditEvent(
  apiServerUrl: string,
  edgeToken: string,
  input: AuditEmitInput,
): Promise<void> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), EMIT_TIMEOUT_MS);
  try {
    const resp = await fetch(`${apiServerUrl}/internal/audit/auth`, {
      method: "POST",
      signal: controller.signal,
      headers: {
        "content-type": "application/json",
        "x-arbx-edge-token": edgeToken,
      },
      body: JSON.stringify({
        action: input.action,
        actor: input.actor,
        target_kind: "ip",
        target_id: input.targetId ?? input.ipAddress,
        ip_address: input.ipAddress,
        user_agent: input.userAgent,
        trace_id: input.traceId,
        after_state: input.afterState,
      }),
    });
    if (!resp.ok) {
      const reason = resp.status === 401 ? "bad_edge_token" : `http_${resp.status}`;
      auditEmitFailedTotal.labels({ reason }).inc();
      console.error(JSON.stringify({
        event: "audit.emit_failed",
        action: input.action,
        status: resp.status,
        reason,
      }));
    }
  } catch (e) {
    const reason = (e as Error).name === "AbortError" ? "timeout" : "network_error";
    auditEmitFailedTotal.labels({ reason }).inc();
    console.error(JSON.stringify({
      event: "audit.emit_failed",
      action: input.action,
      reason,
      error: (e as Error).message,
    }));
  } finally {
    clearTimeout(timer);
  }
}
