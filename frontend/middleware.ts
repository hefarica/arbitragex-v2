/**
 * OMEGA-8 / M5 Capa 4 Fase 12 (P2-S-1) — Edge middleware auth gate.
 *
 * Centralises route gating for admin / mutation surfaces so each page does
 * NOT have to roll its own `if (!hasAdminSession())` check (a pattern that
 * drifted inconsistently across the codebase).
 *
 * Doctrine:
 *   - The middleware MUST NEVER read or compare an admin token plaintext.
 *     It only checks the *presence* of the companion non-secret TTL cookie
 *     `arbx_admin_session_ttl` whose value is the unix-ms expiry. The actual
 *     admin token lives in an httpOnly cookie that the middleware cannot
 *     read — that is intentional (V-AT-1 / secrets policy T1).
 *   - If the TTL cookie is missing or expired → redirect to `/admin/signin`
 *     with `?from=<original>` so post-signin returns the operator to where
 *     they were.
 *   - Public routes pass through. Static assets (`/_next`, `/public`,
 *     `/favicon.ico`) are excluded via `matcher`.
 *   - API routes are out of scope here — the api-server and edge enforce
 *     their own admin gating server-side (PR #73 `runtimeAckAllowed` etc.).
 */

import { NextRequest, NextResponse } from "next/server";

const SESSION_TTL_COOKIE = "arbx_admin_session_ttl";

const PROTECTED_PREFIXES: ReadonlyArray<string> = [
  "/admin",
  "/killswitch",
  "/config/trading",
  "/strategies",
  "/onboarding/1-init",
  "/onboarding/2-connect",
  "/onboarding/3-advanced",
  "/onboarding/4-testing",
  "/onboarding/5-production",
  "/omega-s5/operator",
  "/settings/credentials",
];

// `/admin/signin` is itself under `/admin` but MUST be reachable without a
// session — otherwise the operator can never establish one. Add other
// public-under-protected exceptions here.
const PUBLIC_EXCEPTIONS: ReadonlyArray<string> = ["/admin/signin"];

function isProtected(pathname: string): boolean {
  if (PUBLIC_EXCEPTIONS.some((p) => pathname === p || pathname.startsWith(p + "/"))) {
    return false;
  }
  return PROTECTED_PREFIXES.some((p) => pathname === p || pathname.startsWith(p + "/"));
}

function hasValidSession(req: NextRequest): boolean {
  const ttl = req.cookies.get(SESSION_TTL_COOKIE)?.value;
  if (!ttl) return false;
  const exp = Number(ttl);
  if (!Number.isFinite(exp)) return false;
  return exp > Date.now();
}

export function middleware(req: NextRequest): NextResponse {
  const { pathname, search } = req.nextUrl;
  if (!isProtected(pathname)) {
    return NextResponse.next();
  }
  if (hasValidSession(req)) {
    return NextResponse.next();
  }
  // No session — redirect to signin with return target.
  const signin = req.nextUrl.clone();
  signin.pathname = "/admin/signin";
  signin.search = `?from=${encodeURIComponent(pathname + search)}`;
  return NextResponse.redirect(signin);
}

export const config = {
  // Exclude static assets and Next internals from middleware execution.
  matcher: ["/((?!_next/|api/|favicon\\.ico|public/|.*\\.(?:png|jpg|jpeg|svg|webp|ico|css|js|map|woff2?|ttf)$).*)"],
};
