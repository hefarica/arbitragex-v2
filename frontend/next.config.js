/** @type {import('next').NextConfig} */

// ─── Environment validation ───
// NEXT_PUBLIC_EDGE_URL: browser→edge REST/API calls (mandatory in prod).
// NEXT_PUBLIC_WS_URL:   browser→api-server WebSocket (Socket.IO) (mandatory in prod).
const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL;
const WS_URL = process.env.NEXT_PUBLIC_WS_URL;

// ARBX-HARDENING: Prevent production builds from being generated with localhost endpoints.
// This physically prevents the #425 / API Base URL mismatch cascade if the operator forgets
// to pass --env-file .env during a docker build.
if (process.env.NODE_ENV === "production") {
  if (EDGE_URL && /localhost|127\.0\.0\.1|0\.0\.0\.0/.test(EDGE_URL)) {
    throw new Error(`[CRITICAL] next build failed: NEXT_PUBLIC_EDGE_URL (${EDGE_URL}) cannot point to localhost in production.`);
  }
}
// ─── CSP ───
// CSP-Report-Only: strict policy that LOGS violations but does not enforce.
// Switch to "Content-Security-Policy" once the report stream is clean ≥7 days.
//
// 'unsafe-inline'/'unsafe-eval' on script-src are required by Next 14 RSC
// hydration. Will be replaced with a per-request nonce via middleware.ts.
const csp = (edgeUrl, wsUrl) => [
  "default-src 'self'",
  "script-src 'self' 'unsafe-inline' 'unsafe-eval'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self' data:",
  // Parameterized connect-src: only declared endpoints, no hardcoded localhost.
  `connect-src 'self' ${edgeUrl} ${wsUrl}`,
  "frame-ancestors 'none'",
  "base-uri 'self'",
  "form-action 'self'",
  "object-src 'none'",
  // NOTE: Do NOT add 'upgrade-insecure-requests' here until the VPS has HTTPS.
  // It forces the browser to convert http:// to https://, breaking HTTP-only deployments.
].join("; ");

const nextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  env: {
    NEXT_PUBLIC_EDGE_URL: EDGE_URL,
    NEXT_PUBLIC_WS_URL: WS_URL,
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
    // HSTS requires a valid TLS certificate. Enable only when HTTPS is configured.
    // if (process.env.NODE_ENV === "production" && process.env.ARBX_TLS_ENABLED === "true") {
    //   headers.push({
    //     key: "strict-transport-security",
    //     value: "max-age=63072000; includeSubDomains; preload",
    //   });
    // }
    return [{ source: "/:path*", headers }];
  },
};
module.exports = nextConfig;
