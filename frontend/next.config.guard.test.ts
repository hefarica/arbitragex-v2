// EDGE-HARD-1 — configuration regression guards.
//
// Two production incidents (2026-08-20) came from silent config drift:
//   WS-POLL-1: /socket.io/* was rewritten to the EDGE worker, which has no
//   socket.io route (404 × every handshake → "FEED POLLING" forever). RULE 02
//   says WebSocket terminates at api-server, never via the edge.
//   CSP-IMG-1: img-src didn't allowlist the token-logo CDNs (violation spam;
//   would break logos under enforcing mode).
//
// These tests pin BOTH invariants to the config itself so any regression
// fails CI before it ships.

import { describe, it, expect } from "vitest";
// `require` of the CJS next.config is intentional here; the frontend eslint
// config has no no-require-imports rule, so a disable pragma for it is itself
// a lint error ("Definition for rule ... was not found").
const nextConfig = require("./next.config.js");

const INTERNAL_EDGE = process.env.INTERNAL_EDGE_URL || "http://edge:8787";
const INTERNAL_API = process.env.INTERNAL_API_URL || "http://api-server:8080";

describe("next.config rewrites (WS-POLL-1 regression guard)", () => {
  it("/socket.io/* terminates at the api-server gateway, NEVER at the edge", async () => {
    const rewrites = await nextConfig.rewrites();
    const socketio = rewrites.find((r: { source: string }) => r.source === "/socket.io/:path*");
    expect(socketio, "/socket.io/:path* rewrite must exist").toBeDefined();
    expect(socketio.destination).toContain(`${INTERNAL_API}/socket.io/`);
    expect(socketio.destination).not.toContain(":8787");
  });

  it("/api/* keeps routing through the edge perimeter (RULE 02 REST role)", async () => {
    const rewrites = await nextConfig.rewrites();
    const api = rewrites.find((r: { source: string }) => r.source === "/api/:path*");
    expect(api, "/api/:path* rewrite must exist").toBeDefined();
    expect(api.destination).toContain(`${INTERNAL_EDGE}/api/`);
  });
});

describe("next.config CSP (CSP-IMG-1 regression guard)", () => {
  it("img-src allowlists the token-logo CDNs observed in live inventory", async () => {
    const headers = await nextConfig.headers();
    const all = headers[0].headers as Array<{ key: string; value: string }>;
    const csp = all.find((h) => h.key === "content-security-policy-report-only")?.value ?? "";
    const imgSrc = csp.split(";").find((d) => d.trim().startsWith("img-src")) ?? "";
    expect(imgSrc).toContain("https://raw.githubusercontent.com");
    expect(imgSrc).toContain("https://assets.coingecko.com");
    expect(imgSrc).toContain("https://coin-images.coingecko.com");
  });
});
