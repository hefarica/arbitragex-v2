/**
 * Admin token storage with TTL.
 *
 * Why this exists: the operator pastes the admin token once and the UI keeps
 * it in localStorage so they don't have to paste it on every action. That
 * convenience expires automatically — 8 hours by default — to limit the
 * blast radius of a shared workstation, an unattended browser, or stolen
 * disk state.
 *
 * The token itself is opaque to this module; we never inspect or trim it.
 */

const TOKEN_KEY = "arbx_admin_token";
const EXPIRES_KEY = "arbx_admin_token_expires_at";

export const DEFAULT_TTL_MS = 8 * 60 * 60 * 1000; // 8 hours

function now(): number {
  return Date.now();
}

function safeWindow(): Storage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function setAdminToken(token: string, ttlMs: number = DEFAULT_TTL_MS): void {
  const ls = safeWindow();
  if (!ls) return;
  ls.setItem(TOKEN_KEY, token);
  ls.setItem(EXPIRES_KEY, String(now() + ttlMs));
}

/**
 * Returns the stored token if it has not expired, otherwise clears the
 * record and returns "" (the killswitch / onboarding pages treat empty
 * as "operator has not provided a token yet").
 */
export function getAdminToken(): string {
  const ls = safeWindow();
  if (!ls) return "";
  const expRaw = ls.getItem(EXPIRES_KEY);
  if (expRaw) {
    const exp = Number(expRaw);
    if (!Number.isFinite(exp) || exp <= now()) {
      // Expired or corrupted — wipe both keys.
      ls.removeItem(TOKEN_KEY);
      ls.removeItem(EXPIRES_KEY);
      return "";
    }
  }
  return ls.getItem(TOKEN_KEY) ?? "";
}

export function clearAdminToken(): void {
  const ls = safeWindow();
  if (!ls) return;
  ls.removeItem(TOKEN_KEY);
  ls.removeItem(EXPIRES_KEY);
}

/**
 * Milliseconds remaining until the stored token expires. Returns 0 when no
 * token is stored, or when the record is past its TTL. Useful for surfacing
 * an "expires in 4h 12m" hint next to the input.
 */
export function adminTokenExpiresInMs(): number {
  const ls = safeWindow();
  if (!ls) return 0;
  const expRaw = ls.getItem(EXPIRES_KEY);
  if (!expRaw) return 0;
  const exp = Number(expRaw);
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
