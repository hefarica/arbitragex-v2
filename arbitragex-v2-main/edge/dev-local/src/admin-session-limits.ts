/**
 * Pure rate-limit and lockout helpers for /admin/session.
 *
 * Extracted from index.ts so they can be unit-tested without booting the
 * Express app (which loads configs/app.toml at import time).
 *
 * In-memory only (single-process). For production CF Worker, see
 * edge/worker/src/index.ts which mirrors the same algorithms.
 */

// Rate-limit: 5 attempts per 60s window per IP.
const ADMIN_SESSION_WINDOW_MS = 60_000;
const ADMIN_SESSION_MAX = 5;
const adminSessionRl = new Map<string, { count: number; windowStart: number }>();

export function hitAdminSession(
  ip: string,
  now: number = Date.now()
): { ok: boolean; remaining: number } {
  const cur = adminSessionRl.get(ip);
  if (!cur || now - cur.windowStart > ADMIN_SESSION_WINDOW_MS) {
    adminSessionRl.set(ip, { count: 1, windowStart: now });
    return { ok: true, remaining: ADMIN_SESSION_MAX - 1 };
  }
  cur.count++;
  if (cur.count > ADMIN_SESSION_MAX) return { ok: false, remaining: 0 };
  return { ok: true, remaining: ADMIN_SESSION_MAX - cur.count };
}

// Lockout: 10 consecutive 401s from the same IP → block for 15 min.
export const LOCKOUT_THRESHOLD = 10;
export const LOCKOUT_MS = 15 * 60_000;
const lockouts = new Map<string, { fails: number; blockedUntil: number }>();

export function isLockedOut(ip: string, now: number = Date.now()): boolean {
  const s = lockouts.get(ip);
  return !!s && s.blockedUntil > now;
}

export function recordAuthFailure(ip: string, now: number = Date.now()): void {
  const s = lockouts.get(ip);
  if (!s) {
    lockouts.set(ip, { fails: 1, blockedUntil: 0 });
    return;
  }
  // If the IP was locked out and that lockout has now elapsed, start fresh.
  // (blockedUntil === 0 means "never locked yet" — keep counting.)
  if (s.blockedUntil > 0 && s.blockedUntil < now) {
    lockouts.set(ip, { fails: 1, blockedUntil: 0 });
    return;
  }
  s.fails++;
  if (s.fails >= LOCKOUT_THRESHOLD) {
    s.blockedUntil = now + LOCKOUT_MS;
  }
}

export function recordAuthSuccess(ip: string): void {
  lockouts.delete(ip);
}

// Test-only — reset state between unit tests.
export function __resetAdminSessionState(): void {
  adminSessionRl.clear();
  lockouts.clear();
}
