/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  env: {
    NEXT_PUBLIC_EDGE_URL: process.env.NEXT_PUBLIC_EDGE_URL || "http://localhost:8787",
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
