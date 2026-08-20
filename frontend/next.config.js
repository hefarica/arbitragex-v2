/** @type {import('next').NextConfig} */

// ─── Environment validation ───
// NEXT_PUBLIC_EDGE_URL: browser→edge REST/API calls (mandatory in prod).
// NEXT_PUBLIC_WS_URL:   browser→api-server WebSocket (Socket.IO) (mandatory in prod).
const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL;
const WS_URL = process.env.NEXT_PUBLIC_WS_URL;

// ARBX-HARDENING: Prevent production builds from being generated with localhost endpoints.
// This physically prevents the #425 / API Base URL mismatch cascade if the operator forgets
// to pass --env-file .env during a docker build.
if (process.env.NODE_ENV === "production" && process.env.ARBX_ALLOW_LOCALHOST_PROD !== "true" && process.env.CI !== "true") {
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
//
// SEC-3: build connect-src by filtering undefined env vars. The previous
// template literal `connect-src 'self' ${edgeUrl} ${wsUrl}` produced
// "connect-src 'self'  " (trailing spaces) when either var was undefined,
// which some CSP parsers reject as malformed.
const csp = () => {
  const connectSrcParts = [
    "'self'",
    "ws:",
    "wss:",
    // Reown AppKit (WalletConnect) cloud endpoints — required by the
    // @rainbow-me/rainbowkit / AppKit SDK for remote config + telemetry
    // when NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID is set. The wss: entry
    // above already covers wss://relay.walletconnect.org.
    "https://api.web3modal.org",
    "https://pulse.walletconnect.org",
  ];
  const connectSrc = connectSrcParts.join(" ");
  return [
    "default-src 'self'",
    "script-src 'self' 'unsafe-inline' 'unsafe-eval'",
    "style-src 'self' 'unsafe-inline'",
    // CSP-IMG-1 (2026-08-20): token logos are third-party hosted — TrustWallet
    // GitHub raw (tokens.logo_url in PG) and CoinGecko asset CDNs (token-icons
    // Redis cache). Hosts sourced from live PG/Redis inventory, NOT wildcarded
    // to https: — tight allowlist only.
    "img-src 'self' data: blob: https://raw.githubusercontent.com https://assets.coingecko.com https://coin-images.coingecko.com",
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

// N5 fix (2026-06-13): Next.js server-side rewrites — when the browser accesses
// the frontend directly (e.g. http://VPS_IP:5173) and NEXT_PUBLIC_EDGE_URL was
// not baked into the build, getApiBaseUrl() falls back to window.location.origin
// (the frontend origin), which has no /api/* or /socket.io routes → 404 →
// completedCount stays 0/4 in the UI (Dashboard Readiness 0/4 bug).
//
// These rewrites make the Next.js server proxy /api/* and /socket.io/* to the
// edge at INTERNAL_EDGE_URL (server-side runtime env, always available).
// The browser still fetches from its own origin; Next.js forwards the request
// to the edge transparently. This is a defense-in-depth fallback — when the
// user accesses the app through the edge (https://edge-arbx.ape-tv.net), the
// edge handles /api/* before they reach the frontend, so these rewrites are
// never triggered in the normal path.
//
// INTERNAL_EDGE_URL is a runtime env var (not baked), so it resolves correctly
// even when NEXT_PUBLIC_EDGE_URL was missing at build time.
const INTERNAL_EDGE = process.env.INTERNAL_EDGE_URL || "http://edge:8787";
// WS-POLL-1 (2026-08-20): /socket.io must terminate at the api-server WS
// gateway — the edge worker (Hono) has NO socket.io route, so proxying there
// 404'd every handshake and the feed degraded to HTTP polling. RULE 02:
// WebSocket goes DIRECT to api-server, never via Edge. Same runtime-env
// pattern as INTERNAL_EDGE_URL.
const INTERNAL_API = process.env.INTERNAL_API_URL || "http://api-server:8080";

const nextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  env: {
    NEXT_PUBLIC_EDGE_URL: EDGE_URL,
    NEXT_PUBLIC_WS_URL: WS_URL,
  },
  async rewrites() {
    return [
      {
        // Proxy all /api/* requests to the edge (server-side, no CORS issues).
        source: "/api/:path*",
        destination: `${INTERNAL_EDGE}/api/:path*`,
      },
      {
        // WS-POLL-1: proxy /socket.io/* DIRECTLY to the api-server gateway
        // (RULE 02). The previous target (edge) 404'd — the Hono worker has no
        // socket.io route — so every handshake failed and the feed fell back
        // to HTTP polling ("FEED POLLING" chip). Next rewrites are HTTP-only:
        // socket.io will run its POLLING TRANSPORT through this proxy (a LIVE
        // connection); the true websocket upgrade needs the nginx path
        // (nginx /socket.io/ → api-server:8080, fixed on the VPS 2026-08-20).
        source: "/socket.io/:path*",
        destination: `${INTERNAL_API}/socket.io/:path*`,
      },
      {
        // H2 fix: proxy /admin/* (killswitch toggle, trading-config PUT,
        // onboarding complete, admin session) to the edge. Without this,
        // direct browser access to the frontend origin (:3000/:5173) 404s on
        // every admin mutation — only /api/* and /socket.io/* were proxied.
        source: "/admin/:path*",
        destination: `${INTERNAL_EDGE}/admin/:path*`,
      },
    ];
  },
  async headers() {
    const headers = [
      { key: "x-frame-options", value: "DENY" },
      { key: "x-content-type-options", value: "nosniff" },
      { key: "referrer-policy", value: "no-referrer" },
      { key: "permissions-policy", value: "camera=(), microphone=(), geolocation=()" },
      { key: "content-security-policy-report-only", value: csp() },
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
