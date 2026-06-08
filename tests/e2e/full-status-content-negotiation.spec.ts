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
 * BASE defaults to the compose stack's edge (CI sets ARBX_FRONTEND_URL to the
 * freshly-built edge :8787).
 */

const BASE =
  process.env["E2E_BASE_URL"] ??
  process.env["ARBX_FRONTEND_URL"] ??
  "http://localhost:8787";

test.describe.configure({ mode: "serial" });

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
