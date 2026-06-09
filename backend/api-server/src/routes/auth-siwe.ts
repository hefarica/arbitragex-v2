/**
 * auth-siwe — Sign-In With Ethereum (EIP-4361) IDENTITY-ONLY authentication.
 *
 *   GET  /api/auth/siwe/nonce   → mint a one-time, expiring nonce
 *   POST /api/auth/siwe/verify  → verify a signed SIWE message (siwe library)
 *   GET  /api/auth/session      → report the current session (redacted)
 *   POST /api/auth/logout       → clear the session cookie
 *   POST /api/wallet/signature/verify → standalone message-signature check
 *
 * SECURITY DOCTRINE (HARD INVARIANTS — never violated):
 *   • This is WALLET-IDENTITY-ONLY. A session proves the operator controls an
 *     address; it grants NOTHING capital-adjacent. live_enabled is structurally
 *     false; capital_exposed is structurally 0; broadcast is structurally false.
 *   • NO private key, seed, or mnemonic ever exists server-side. We verify a
 *     signature the wallet produced client-side; we never sign anything.
 *   • Signature verification uses the `siwe` library (EIP-4361 + EIP-191/EIP-1271
 *     via ethers under the hood) — NEVER hand-rolled crypto.
 *   • Nonces are single-use and expiring; a replayed/expired nonce is rejected.
 *   • The session is an HMAC-signed, httpOnly + SameSite=Strict cookie with a
 *     finite TTL. The signing secret comes from env (RULE 00 / no-hardcode); if
 *     it is absent the route degrades to a fail-honest `unavailable` state and
 *     issues NO session — it never falls back to an insecure default.
 *
 * Fail-honest (RULE 00 / R8): when the wallet runtime isn't configured we return
 *   { status:"unavailable", reason:"wallet_runtime_not_configured",
 *     live_enabled:false, capital_exposed:0, broadcast:false }
 * rather than fabricating a session.
 */

import crypto from "node:crypto";
import type { Application, Request, Response } from "express";
import { SiweMessage, generateNonce } from "siwe";

// ---------------------------------------------------------------------------
// Constants (non-secret protocol constants — not operator-tunable productive
// data, so not a no-hardcode violation). Secrets/domains come from env.
// ---------------------------------------------------------------------------

export const WALLET_SESSION_COOKIE = "arbx_wallet_session";
const NONCE_TTL_MS = 5 * 60 * 1000; // 5 minutes — one-time nonce lifetime.
const DEFAULT_SESSION_TTL_S = 60 * 60; // 1 hour identity session.
const MAX_MESSAGE_BYTES = 8 * 1024; // reject oversized payloads.

/** Env that, when present, enables real session issuance. */
const SESSION_SECRET_ENV_VARS = ["ARBX_WALLET_SESSION_SECRET", "JWT_SECRET"] as const;
/** Optional expected domain binding (host the SIWE message must declare). */
const EXPECTED_DOMAIN_ENV_VARS = ["ARBX_WALLET_SIWE_DOMAIN", "PUBLIC_EDGE_HOST"] as const;

// ---------------------------------------------------------------------------
// Env resolution — presence only; the raw secret never leaves this module.
// ---------------------------------------------------------------------------

function firstEnv(names: readonly string[]): string | undefined {
  for (const n of names) {
    const v = process.env[n];
    if (typeof v === "string" && v.length > 0) return v;
  }
  return undefined;
}

function sessionSecret(): string | undefined {
  const v = firstEnv(SESSION_SECRET_ENV_VARS);
  // Reject obviously-too-weak secrets so we never sign sessions with a
  // placeholder. Absence/weakness → fail-honest unavailable (never a default).
  return v && v.length >= 32 ? v : undefined;
}

function sessionTtlS(): number {
  const raw = process.env["ARBX_WALLET_SESSION_TTL_S"];
  const n = raw ? parseInt(raw, 10) : NaN;
  return Number.isFinite(n) && n > 0 ? n : DEFAULT_SESSION_TTL_S;
}

function expectedDomain(): string | undefined {
  return firstEnv(EXPECTED_DOMAIN_ENV_VARS);
}

// ---------------------------------------------------------------------------
// One-time expiring nonce store. In-memory is sufficient for an identity-only
// flow on a single api-server instance; a multi-instance deployment would back
// this with Redis. Each nonce is consumed on first verify and self-evicts.
// ---------------------------------------------------------------------------

interface NonceRecord {
  expiresAt: number;
}
const nonceStore = new Map<string, NonceRecord>();

function pruneNonces(now: number): void {
  for (const [k, v] of nonceStore) {
    if (v.expiresAt <= now) nonceStore.delete(k);
  }
}

export function issueNonce(now = Date.now()): string {
  pruneNonces(now);
  const nonce = generateNonce(); // siwe-supplied alphanumeric nonce.
  nonceStore.set(nonce, { expiresAt: now + NONCE_TTL_MS });
  return nonce;
}

/** Consume a nonce. Returns true exactly once for a valid, unexpired nonce. */
export function consumeNonce(nonce: string, now = Date.now()): boolean {
  pruneNonces(now);
  const rec = nonceStore.get(nonce);
  if (!rec) return false;
  nonceStore.delete(nonce); // single-use: delete regardless of expiry outcome.
  return rec.expiresAt > now;
}

// ---------------------------------------------------------------------------
// HMAC-signed session token. Payload is base64url(JSON) + "." + base64url(hmac).
// Carries ONLY identity (address, chainId, issued/expiry, mode). NO secrets.
// ---------------------------------------------------------------------------

export interface SessionClaims {
  address: string;
  chainId: number;
  iat: number; // issued-at (ms epoch)
  exp: number; // expiry (ms epoch)
  mode: "wallet_identity_only";
}

function b64url(buf: Buffer): string {
  return buf.toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function fromB64url(s: string): Buffer {
  return Buffer.from(s.replace(/-/g, "+").replace(/_/g, "/"), "base64");
}

export function signSession(claims: SessionClaims, secret: string): string {
  const payload = b64url(Buffer.from(JSON.stringify(claims), "utf8"));
  const mac = b64url(crypto.createHmac("sha256", secret).update(payload).digest());
  return `${payload}.${mac}`;
}

export function verifySession(token: string, secret: string, now = Date.now()): SessionClaims | null {
  const dot = token.indexOf(".");
  if (dot <= 0) return null;
  const payload = token.slice(0, dot);
  const mac = token.slice(dot + 1);
  const expected = b64url(crypto.createHmac("sha256", secret).update(payload).digest());
  // Constant-time compare (length-guarded so timingSafeEqual never throws).
  const macBuf = Buffer.from(mac);
  const expBuf = Buffer.from(expected);
  if (macBuf.length !== expBuf.length || !crypto.timingSafeEqual(macBuf, expBuf)) return null;
  try {
    const claims = JSON.parse(fromB64url(payload).toString("utf8")) as SessionClaims;
    if (claims.mode !== "wallet_identity_only") return null;
    if (typeof claims.exp !== "number" || claims.exp <= now) return null;
    if (typeof claims.address !== "string" || !/^0x[a-fA-F0-9]{40}$/.test(claims.address)) return null;
    return claims;
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Cookie helpers (manual, mirroring the codebase's no-cookie-parser convention).
// ---------------------------------------------------------------------------

function parseCookies(header: string | undefined): Record<string, string> {
  if (!header) return {};
  const out: Record<string, string> = {};
  for (const pair of header.split(";")) {
    const idx = pair.indexOf("=");
    if (idx < 0) continue;
    out[pair.slice(0, idx).trim()] = decodeURIComponent(pair.slice(idx + 1).trim());
  }
  return out;
}

function isSecureRequest(req: Request): boolean {
  // Hardening: in production the wallet session cookie MUST carry Secure regardless of how
  // the reverse proxy forwards the scheme — never let a missing/misconfigured
  // x-forwarded-proto downgrade it to plain HTTP and expose it to interception.
  if (process.env.NODE_ENV === "production") return true;
  if (req.secure) return true;
  const xf = (req.header("x-forwarded-proto") ?? "").toLowerCase();
  return xf.includes("https");
}

function setSessionCookie(req: Request, res: Response, token: string, ttlS: number): void {
  const parts = [
    `${WALLET_SESSION_COOKIE}=${encodeURIComponent(token)}`,
    "HttpOnly",
    "SameSite=Strict",
    "Path=/",
    `Max-Age=${ttlS}`,
  ];
  if (isSecureRequest(req)) parts.push("Secure");
  res.append("Set-Cookie", parts.join("; "));
}

function clearSessionCookie(req: Request, res: Response): void {
  const parts = [
    `${WALLET_SESSION_COOKIE}=`,
    "HttpOnly",
    "SameSite=Strict",
    "Path=/",
    "Max-Age=0",
  ];
  if (isSecureRequest(req)) parts.push("Secure");
  res.append("Set-Cookie", parts.join("; "));
}

// ---------------------------------------------------------------------------
// Shared safety posture — every wallet/auth response carries it verbatim.
// ---------------------------------------------------------------------------

const SAFE_POSTURE = {
  live_enabled: false as const,
  capital_exposed: 0 as const,
  broadcast: false as const,
};

function unavailable(res: Response, reason: string): void {
  res.status(200).json({
    status: "unavailable",
    reason,
    ...SAFE_POSTURE,
    session_mode: "wallet_identity_only" as const,
  });
}

// ---------------------------------------------------------------------------
// Verify body schema (hand-validated; zod is available but the surface is tiny).
// ---------------------------------------------------------------------------

interface VerifyBody {
  message?: string | undefined;
  signature?: string | undefined;
}

function readVerifyBody(body: unknown): VerifyBody {
  if (!body || typeof body !== "object") return {};
  const b = body as Record<string, unknown>;
  return {
    message: typeof b.message === "string" ? b.message : undefined,
    signature: typeof b.signature === "string" ? b.signature : undefined,
  };
}

// ---------------------------------------------------------------------------
// Route mounting.
// ---------------------------------------------------------------------------

export function mountAuthSiwe(
  app: Application,
  deps: { logger: { warn: (obj: object, msg?: string) => void; info?: (obj: object, msg?: string) => void } },
): void {
  // ----- GET /api/auth/siwe/nonce -----------------------------------------
  app.get("/api/auth/siwe/nonce", (_req: Request, res: Response) => {
    if (!sessionSecret()) return unavailable(res, "wallet_runtime_not_configured");
    const nonce = issueNonce();
    res.status(200).json({
      status: "ok",
      nonce,
      expires_in_s: Math.floor(NONCE_TTL_MS / 1000),
      session_mode: "wallet_identity_only",
      ...SAFE_POSTURE,
    });
  });

  // ----- POST /api/auth/siwe/verify ---------------------------------------
  app.post("/api/auth/siwe/verify", async (req: Request, res: Response) => {
    const secret = sessionSecret();
    if (!secret) return unavailable(res, "wallet_runtime_not_configured");

    const { message, signature } = readVerifyBody(req.body);
    if (!message || !signature) {
      res.status(400).json({ status: "error", reason: "message_and_signature_required", ...SAFE_POSTURE });
      return;
    }
    if (message.length > MAX_MESSAGE_BYTES) {
      res.status(400).json({ status: "error", reason: "message_too_large", ...SAFE_POSTURE });
      return;
    }

    let siwe: SiweMessage;
    try {
      siwe = new SiweMessage(message);
    } catch (e) {
      res.status(400).json({ status: "error", reason: "siwe_message_unparseable", detail: (e as Error).message, ...SAFE_POSTURE });
      return;
    }

    // Nonce must be one we minted and not yet consumed (single-use, unexpired).
    if (!siwe.nonce || !consumeNonce(siwe.nonce)) {
      res.status(401).json({ status: "error", reason: "nonce_invalid_or_expired", ...SAFE_POSTURE });
      return;
    }

    // Verify via the siwe library — NEVER hand-rolled. Domain binding is MANDATORY:
    // if no expected domain is configured we fail-honest (issue no session) rather than
    // verifying without it, which would allow a signature minted for a phishing origin to
    // be replayed here (cross-domain replay). The library then checks the cryptographic
    // signature, expiry, not-before, the consumed nonce, AND this domain binding.
    const expDomain = expectedDomain();
    if (!expDomain) return unavailable(res, "wallet_siwe_domain_not_configured");
    try {
      const result = await siwe.verify({
        signature,
        nonce: siwe.nonce,
        domain: expDomain,
      });
      if (!result.success) {
        deps.logger.warn({ event: "siwe.verify_failed", reason: result.error?.type ?? "unknown" });
        res.status(401).json({ status: "error", reason: "siwe_verification_failed", ...SAFE_POSTURE });
        return;
      }
    } catch (e) {
      // Hard failures (bad signature, malformed) throw — honest 401, never a pass.
      deps.logger.warn({ event: "siwe.verify_threw", err: (e as Error).message });
      res.status(401).json({ status: "error", reason: "siwe_verification_failed", ...SAFE_POSTURE });
      return;
    }

    const now = Date.now();
    const ttlS = sessionTtlS();
    const claims: SessionClaims = {
      address: siwe.address,
      chainId: siwe.chainId,
      iat: now,
      exp: now + ttlS * 1000,
      mode: "wallet_identity_only",
    };
    const token = signSession(claims, secret);
    setSessionCookie(req, res, token, ttlS);

    res.status(200).json({
      status: "ok",
      authenticated: true,
      session_mode: "wallet_identity_only",
      address: siwe.address,
      chain_id: siwe.chainId,
      expires_at: claims.exp,
      ...SAFE_POSTURE,
    });
  });

  // ----- POST /api/wallet/signature/verify --------------------------------
  // Standalone signature check (does NOT issue a session, does NOT consume a
  // SIWE nonce). Useful to prove control of an address without logging in.
  app.post("/api/wallet/signature/verify", async (req: Request, res: Response) => {
    const { message, signature } = readVerifyBody(req.body);
    if (!message || !signature) {
      res.status(400).json({ status: "error", reason: "message_and_signature_required", ...SAFE_POSTURE });
      return;
    }
    if (message.length > MAX_MESSAGE_BYTES) {
      res.status(400).json({ status: "error", reason: "message_too_large", ...SAFE_POSTURE });
      return;
    }
    let siwe: SiweMessage;
    try {
      siwe = new SiweMessage(message);
    } catch (e) {
      res.status(400).json({ status: "error", valid: false, reason: "siwe_message_unparseable", detail: (e as Error).message, ...SAFE_POSTURE });
      return;
    }
    try {
      const result = await siwe.verify({ signature });
      res.status(result.success ? 200 : 401).json({
        status: result.success ? "ok" : "error",
        valid: result.success,
        ...(result.success ? { address: siwe.address, chain_id: siwe.chainId } : { reason: "signature_invalid" }),
        ...SAFE_POSTURE,
      });
    } catch (e) {
      deps.logger.warn({ event: "wallet.signature_verify_threw", err: (e as Error).message });
      res.status(401).json({ status: "error", valid: false, reason: "signature_invalid", ...SAFE_POSTURE });
    }
  });

  // ----- GET /api/auth/session --------------------------------------------
  app.get("/api/auth/session", (req: Request, res: Response) => {
    const secret = sessionSecret();
    if (!secret) {
      res.status(200).json({
        status: "ok",
        authenticated: false,
        reason: "wallet_runtime_not_configured",
        session_mode: "wallet_identity_only",
        ...SAFE_POSTURE,
      });
      return;
    }
    const cookies = parseCookies(req.headers.cookie);
    const token = cookies[WALLET_SESSION_COOKIE];
    const claims = token ? verifySession(token, secret) : null;
    if (!claims) {
      res.status(200).json({
        status: "ok",
        authenticated: false,
        session_mode: "wallet_identity_only",
        ...SAFE_POSTURE,
      });
      return;
    }
    res.status(200).json({
      status: "ok",
      authenticated: true,
      session_mode: "wallet_identity_only",
      address: claims.address,
      chain_id: claims.chainId,
      issued_at: claims.iat,
      expires_at: claims.exp,
      ...SAFE_POSTURE,
    });
  });

  // ----- POST /api/auth/logout --------------------------------------------
  app.post("/api/auth/logout", (req: Request, res: Response) => {
    clearSessionCookie(req, res);
    res.status(200).json({ status: "ok", authenticated: false, session_mode: "wallet_identity_only", ...SAFE_POSTURE });
  });
}

// Test-only exports — pure functions asserted without Express/IO.
export const __forTesting = {
  issueNonce,
  consumeNonce,
  signSession,
  verifySession,
  parseCookies,
  SAFE_POSTURE,
  NONCE_TTL_MS,
};
