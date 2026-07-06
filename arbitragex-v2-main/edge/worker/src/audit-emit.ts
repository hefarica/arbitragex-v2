/**
 * Fire-and-forget audit event emitter for Cloudflare Worker edge.
 *
 * Same API as dev-local/src/audit-emit.ts but uses Web Crypto API
 * (available in CF Worker isolates) instead of Node's crypto module.
 */

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
  userAgent?: string;
  targetId?: string;
  afterState?: Record<string, unknown>;
  traceId?: string;
};

const EMIT_TIMEOUT_MS = 2000;

/**
 * Compute a non-reversible fingerprint of the admin token for audit actor field.
 * Uses Web Crypto API (CF Worker compatible).
 */
export async function tokenFingerprint(token: string): Promise<string> {
  const data = new TextEncoder().encode(token);
  const hashBuffer = await crypto.subtle.digest("SHA-256", data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const hex = hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
  return `tok:${hex.substring(0, 8)}`;
}

/**
 * Fire-and-forget emit to api-server's /internal/audit/auth.
 * Never throws — logs to console.error on failure.
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
      console.error(JSON.stringify({
        event: "audit.emit_failed",
        action: input.action,
        status: resp.status,
        reason: resp.status === 401 ? "bad_edge_token" : `http_${resp.status}`,
      }));
    }
  } catch (e) {
    const reason = (e as Error).name === "AbortError" ? "timeout" : "network_error";
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
