/** @type {import('next').NextConfig} */

// ─── Environment validation ───
// NEXT_PUBLIC_EDGE_URL: browser→edge REST/API calls (mandatory in prod).
// NEXT_PUBLIC_WS_URL:   browser→api-server WebSocket (Socket.IO) (mandatory in prod).
const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL;
const WS_URL = process.env.NEXT_PUBLIC_WS_URL;

// ARBX-HARDENING: Prevent production builds from being generated with localhost endpoints.
// This physically prevents the #425 / API Base URL mismatch cascade if the operator forgets
// to pass --env-file .env during a docker build.
//
// Opt-out: ARBX_BUILD_FOR_LOCAL_E2E=1 explicitly permits localhost—only used by the
// e2e/playwright CI workflow which runs Playwright on the same host as the docker
// compose stack and needs the browser to reach edge via http://localhost:8787.
// The opt-out is intentionally noisy (prints a warning) and never the default.
if (process.env.NODE_ENV === "production") {
  const allowLocalForE2E = process.env.ARBX_BUILD_FOR_LOCAL_E2E === "1";
  if (EDGE_URL && /localhost|127\.0\.0\.1|0\.0\.0\.0/.test(EDGE_URL)) {
    if (!allowLocalForE2E) {
      throw new Error(`[CRITICAL] next build failed: NEXT_PUBLIC_EDGE_URL (${EDGE_URL}) cannot point to localhost in production.`);
    }
    console.warn(`[ARBX-HARDENING] localhost NEXT_PUBLIC_EDGE_URL accepted under ARBX_BUILD_FOR_LOCAL_E2E=1 — this build is NOT suitable for staging/prod.`);
  }
}
// ─── CSP ───
// CSP-Report-Only: strict policy that LOGS violations but does not enforce.
// Switch to "Content-Security-Policy" once the report stream is clean ≥7 days.
//
// 'unsafe-inline'/'unsafe-eval' on script-src are required by Next 14 RSC
// hydration. Will be replaced with a per-request nonce via middleware.ts.
//
// SEC-3: build connect-src by filtering undefined env vars. The previous
// template literal `connect-src 'self' ${edgeUrl} ${wsUrl}` produced
// "connect-src 'self'  " (trailing spaces) when either var was undefined,
// which some CSP parsers reject as malformed.
const csp = (edgeUrl, wsUrl) => {
  const connectSrcParts = ["'self'", edgeUrl, wsUrl].filter(Boolean);
  const connectSrc = connectSrcParts.join(" ");
  return [
    "default-src 'self'",
    "script-src 'self' 'unsafe-inline' 'unsafe-eval'",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: blob:",
    "font-src 'self' data:",
    // Parameterized connect-src: only declared endpoints, no hardcoded localhost.
    `connect-src ${connectSrc}`,
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'",
    "object-src 'none'",
    // NOTE: Do NOT add 'upgrade-insecure-requests' here until the VPS has HTTPS.
    // It forces the browser to convert http:// to https://, breaking HTTP-only deployments.
  ].join("; ");
};

// ARBX-HARDENING runtime opt-in: surface the E2E opt-out flag to the client
// bundle so the runtime validators in lib/api-client.ts can mirror the build
// gate. Only set to "1" when build was opted-in via ARBX_BUILD_FOR_LOCAL_E2E=1;
// otherwise it stays undefined and runtime keeps refusing localhost in prod.
const E2E_LOCAL_OK = process.env.ARBX_BUILD_FOR_LOCAL_E2E === "1" ? "1" : "";

const nextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  env: {
    NEXT_PUBLIC_EDGE_URL: EDGE_URL,
    NEXT_PUBLIC_WS_URL: WS_URL,
    NEXT_PUBLIC_ARBX_E2E_LOCAL_OK: E2E_LOCAL_OK,
  },
  async headers() {
    const resolvedEdge = EDGE_URL || "";
    const resolvedWs = WS_URL || "";
    const headers = [
      { key: "x-frame-options", value: "DENY" },
      { key: "x-content-type-options", value: "nosniff" },
      { key: "referrer-policy", value: "no-referrer" },
      { key: "permissions-policy", value: "camera=(), microphone=(), geolocation=()" },
      { key: "content-security-policy-report-only", value: csp(resolvedEdge, resolvedWs) },
    ];
    // SEC-3: HSTS enabled when TLS is configured. 1y, includeSubDomains, NO preload
    // (audit recommendation — preload is irrevocable for ≥1y, defer until TLS is stable).
    if (process.env.NODE_ENV === "production" && process.env.ARBX_TLS_ENABLED === "true") {
      headers.push({
        key: "strict-transport-security",
        value: "max-age=31536000; includeSubDomains",
      });
    }
    return [{ source: "/:path*", headers }];
  },
};
module.exports = nextConfig;
