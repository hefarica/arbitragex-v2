/** @type {import('next').NextConfig} */

// No-hardcode doctrine: in production builds, NEXT_PUBLIC_EDGE_URL must be set
// explicitly (operator's public edge). In development we allow the well-known
// local compose port so `npm run dev` works with no env setup.
const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL;
if (!EDGE_URL) {
  if (process.env.NODE_ENV === "production") {
    throw new Error(
      "NEXT_PUBLIC_EDGE_URL is required for production builds. See docs/governance/DATA-MATRIX.md (M6)."
    );
  }
  console.warn("[arbx] NEXT_PUBLIC_EDGE_URL not set — defaulting to http://localhost:8787 (dev only)");
}

// CSP-Report-Only: strict policy that LOGS violations but does not enforce.
// Switch the header key to "Content-Security-Policy" once the report stream
// is clean for ≥7 days (planned PR-1.b). Until then, breaking the app on a
// CSP regression would be worse than the marginal protection.
//
// 'unsafe-inline'/'unsafe-eval' on script-src are required by Next 14 RSC
// hydration. PR-1.b will replace them with a per-request nonce via a
// `middleware.ts`. Tracked in MEMORY.
const csp = (edgeUrl) => [
  "default-src 'self'",
  "script-src 'self' 'unsafe-inline' 'unsafe-eval'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self' data:",
  `connect-src 'self' ${edgeUrl}`,
  "frame-ancestors 'none'",
  "base-uri 'self'",
  "form-action 'self'",
  "object-src 'none'",
  "upgrade-insecure-requests",
].join("; ");

const nextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  env: {
    NEXT_PUBLIC_EDGE_URL: EDGE_URL || "http://localhost:8787",
  },
  async headers() {
    const headers = [
      { key: "x-frame-options", value: "DENY" },
      { key: "x-content-type-options", value: "nosniff" },
      { key: "referrer-policy", value: "no-referrer" },
      { key: "permissions-policy", value: "camera=(), microphone=(), geolocation=()" },
      { key: "content-security-policy-report-only", value: csp(EDGE_URL || "http://localhost:8787") },
    ];
    // HSTS only in production (HTTPS-only environments). Dev runs over http://.
    if (process.env.NODE_ENV === "production") {
      headers.push({
        key: "strict-transport-security",
        value: "max-age=63072000; includeSubDomains; preload",
      });
    }
    return [{ source: "/:path*", headers }];
  },
};
module.exports = nextConfig;
