import { getApiBaseUrl } from "@/lib/api-client";
/**
 * Admin token session management via httpOnly cookie.
 *
 * SECURITY HARDENING (V-AT-1): The admin token (classified T1 in
 * secrets.policy.md) is no longer stored in localStorage. Instead:
 *
 *   setAdminToken(token) → POSTs to edge /admin/session which:
 *     - validates the token against api-server
 *     - sets an httpOnly; SameSite=Strict cookie; Secure is enabled on HTTPS requests (JS cannot read it)
 *     - sets a companion non-httpOnly cookie `arbx_admin_session_ttl` with
 *       only the expiry timestamp (no secret) for UI display
 *
 *   getAdminToken() → Returns "" always. The token travels via cookie header
 *     automatically on every request when credentials: "include" is used.
 *     Pages should check hasAdminSession() instead.
 *
 *   clearAdminToken() → POSTs to edge /admin/session/logout to clear cookies.
 *
 * The token is opaque to this module — we never inspect, trim, or store it.
 */

const SESSION_TTL_COOKIE = "arbx_admin_session_ttl";
export const DEFAULT_TTL_MS = 8 * 60 * 60 * 1000; // 8 hours

function now(): number {
  return Date.now();
}

/**
 * Establishes an admin session by sending the token to the edge, which
 * validates it and sets the httpOnly cookie. Returns true on success.
 */
export async function setAdminToken(token: string, _ttlMs?: number): Promise<boolean> {
  try {
    const res = await fetch(`${getApiBaseUrl()}/admin/session`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ token }),
      credentials: "include",
    });
    return res.ok;
  } catch {
    return false;
  }
}

/**
 * Returns "" always. The admin token now travels via httpOnly cookie which
 * JavaScript cannot read. Use hasAdminSession() to check if a session exists.
 *
 * This function is kept for backward compatibility with call sites that
 * check `if (getAdminToken())`. They should migrate to hasAdminSession().
 */
export function getAdminToken(): string {
  // Check the TTL companion cookie to see if a session exists.
  // The actual secret is in the httpOnly cookie — inaccessible to JS.
  if (typeof document === "undefined") return "";
  const match = document.cookie.match(new RegExp(`(?:^|;\\s*)${SESSION_TTL_COOKIE}=([^;]*)`));
  if (!match) return "";
  const exp = Number(match[1]);
  if (!Number.isFinite(exp) || exp <= now()) return "";
  // Return a sentinel indicating "session active" — never the real token.
  return "__session_active__";
}

/**
 * Whether the operator has an active admin session (httpOnly cookie set,
 * companion TTL cookie not expired).
 */
export function hasAdminSession(): boolean {
  return getAdminToken() !== "";
}

/**
 * Clears the admin session by calling the edge logout endpoint which
 * clears the httpOnly cookie server-side.
 */
export async function clearAdminToken(): Promise<void> {
  try {
    await fetch(`${getApiBaseUrl()}/admin/session/logout`, {
      method: "POST",
      credentials: "include",
    });
  } catch {
    // Best-effort. Also clear the companion cookie client-side.
  }
  // Clear companion TTL cookie client-side as backup.
  if (typeof document !== "undefined") {
    document.cookie = `${SESSION_TTL_COOKIE}=; Max-Age=0; Path=/; SameSite=Strict`;
  }
}

/**
 * Milliseconds remaining until the admin session expires. Reads from the
 * companion (non-httpOnly) TTL cookie. Returns 0 when no session exists.
 */
export function adminTokenExpiresInMs(): number {
  if (typeof document === "undefined") return 0;
  const match = document.cookie.match(new RegExp(`(?:^|;\\s*)${SESSION_TTL_COOKIE}=([^;]*)`));
  if (!match) return 0;
  const exp = Number(match[1]);
  if (!Number.isFinite(exp)) return 0;
  return Math.max(0, exp - now());
}

export function fmtRemaining(ms: number): string {
  if (ms <= 0) return "expired";
  const totalMin = Math.floor(ms / 60_000);
  const h = Math.floor(totalMin / 60);
  const m = totalMin % 60;
  if (h <= 0) return `${m}m`;
  return `${h}h ${m}m`;
}
