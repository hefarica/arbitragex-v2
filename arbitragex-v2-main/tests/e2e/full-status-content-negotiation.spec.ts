import { test, expect } from "@playwright/test";

/**
 * FE-CRIT-01 — /status content-negotiation (READ-ONLY).
 *
 * The edge historically proxied the api-server's JSON /status UNCONDITIONALLY,
 * shadowing the Next.js SPA /status page (a browser navigation got raw JSON).
 * The fix negotiates on the request:
 *   • Accept: text/html (browser navigation) → the SPA HTML /status page.
 *   • Accept: application/json, ?format=json, or a CLI User-Agent → backend JSON.
 *
 * BASE must point at the EDGE (8787) — NOT the frontend — because /status
 * content-negotiation lives at the edge (the frontend always serves the SPA).
 * Set E2E_BASE_URL (public edge) or ARBX_EDGE_URL (CI compose); default
 * localhost:8787. If the edge is unreachable the suite SKIPS honestly.
 */

const BASE =
  process.env["E2E_BASE_URL"] ??
  process.env["ARBX_EDGE_URL"] ??
  "http://localhost:8787";

test.describe.configure({ mode: "serial" });

// Honest skip when the edge isn't reachable — never a fake green.
test.beforeEach(async ({ request }) => {
  let up = false;
  try {
    const r = await request.get(`${BASE}/api/health`, { failOnStatusCode: false, timeout: 8000 });
    up = r.ok();
  } catch {
    up = false;
  }
  test.skip(!up, `edge not reachable at ${BASE} — VALIDATION_PENDING_POST_DEPLOY`);
});

test("/status with Accept: text/html serves the SPA (text/html)", async ({
  request,
}) => {
  const res = await request.get(`${BASE}/status`, {
    headers: { Accept: "text/html,application/xhtml+xml" },
    // Browser-like UA so the CLI-UA short-circuit does not fire.
    failOnStatusCode: false,
  });
  expect(res.status(), "/status (html) reachable").toBeLessThan(400);
  const ct = res.headers()["content-type"] ?? "";
  expect(ct, "html request gets text/html (SPA, not shadowed JSON)").toContain(
    "text/html",
  );
});

test("/status with Accept: application/json returns JSON", async ({
  request,
}) => {
  const res = await request.get(`${BASE}/status`, {
    headers: { Accept: "application/json" },
    failOnStatusCode: false,
  });
  expect(res.status(), "/status (json) reachable").toBe(200);
  const ct = res.headers()["content-type"] ?? "";
  expect(ct, "json request gets application/json").toContain("application/json");
  // Must be parseable JSON with the api-server /status shape (ok + version).
  const body = await res.json();
  expect(typeof body, "body is a JSON object").toBe("object");
  expect(body, "JSON /status carries an ok flag").toHaveProperty("ok");
});

test("/status with curl User-Agent returns JSON", async ({ request }) => {
  const res = await request.get(`${BASE}/status`, {
    headers: { "User-Agent": "curl/8.4.0", Accept: "*/*" },
    failOnStatusCode: false,
  });
  expect(res.status(), "/status (curl) reachable").toBe(200);
  const ct = res.headers()["content-type"] ?? "";
  expect(ct, "curl UA gets application/json").toContain("application/json");
  const body = await res.json();
  expect(body, "JSON /status carries an ok flag").toHaveProperty("ok");
});

test("/status?format=json forces JSON even from a browser UA", async ({
  request,
}) => {
  const res = await request.get(`${BASE}/status?format=json`, {
    headers: {
      Accept: "text/html,application/xhtml+xml",
      "User-Agent":
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
    },
    failOnStatusCode: false,
  });
  expect(res.status(), "/status?format=json reachable").toBe(200);
  const ct = res.headers()["content-type"] ?? "";
  expect(ct, "?format=json overrides Accept: text/html").toContain(
    "application/json",
  );
});
