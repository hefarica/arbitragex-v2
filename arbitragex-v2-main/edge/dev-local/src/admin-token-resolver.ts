/**
 * V-AT-1 admin token resolution helper, shared between the edge runtime and
 * its unit tests. The browser flow cannot read the real arbx_admin_session
 * cookie (httpOnly), so the frontend sends the public sentinel
 * "__session_active__" in the x-arbx-admin-token header. When we see the
 * sentinel we MUST fall back to the cookie — otherwise upstream validates
 * the sentinel literal and rejects every browser write.
 *
 * Returns the resolved token (a string suitable for forwarding upstream as
 * x-arbx-admin-token) or null when neither input carries a usable value.
 */

export const SESSION_COOKIE = "arbx_admin_session";
export const SESSION_SENTINEL = "__session_active__";

export function parseCookies(header: string | undefined): Record<string, string> {
  if (!header) return {};
  const out: Record<string, string> = {};
  for (const pair of header.split(";")) {
    const idx = pair.indexOf("=");
    if (idx < 0) continue;
    out[pair.slice(0, idx).trim()] = pair.slice(idx + 1).trim();
  }
  return out;
}

export interface TokenInput {
  readonly headerToken?: string | string[] | undefined;
  readonly cookieHeader?: string | undefined;
}

export function resolveAdminToken(input: TokenInput): string | null {
  const raw = Array.isArray(input.headerToken) ? input.headerToken[0] : input.headerToken;
  if (raw && raw !== SESSION_SENTINEL) return raw;
  const cookies = parseCookies(input.cookieHeader);
  const fromCookie = cookies[SESSION_COOKIE];
  return fromCookie && fromCookie.length > 0 ? fromCookie : null;
}
