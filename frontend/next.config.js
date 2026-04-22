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

const nextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  env: {
    NEXT_PUBLIC_EDGE_URL: EDGE_URL || "http://localhost:8787",
  },
  async headers() {
    return [{
      source: "/:path*",
      headers: [
        { key: "x-frame-options", value: "DENY" },
        { key: "x-content-type-options", value: "nosniff" },
        { key: "referrer-policy", value: "no-referrer" },
        { key: "permissions-policy", value: "camera=(), microphone=(), geolocation=()" },
      ],
    }];
  },
};
module.exports = nextConfig;
