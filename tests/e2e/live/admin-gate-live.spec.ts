import { test, expect } from "@playwright/test";

/**
 * LIVE admin-gate sweep (READ-ONLY).
 *
 * Asserts that admin / mutating endpoints REJECT unauthenticated callers.
 * This is the executable proof of the "no open control surface" invariant:
 * a GET/POST without a valid ARBX_ADMIN_TOKEN must not be honored.
 *
 * We only ASSERT REJECTION here — nothing is armed, written, or flipped.
 */

const EDGE =
  process.env["ARBX_EDGE_URL"] ??
  process.env["ARBX_FRONTEND_URL"] ??
  "https://edge-arbx.ape-tv.net";

// Endpoints that MUST reject an unauthenticated read.
// (Paths verified against edge/worker/src/index.ts.)
const GUARDED_GET = [
  "/api/admin/topology/snapshot",
  "/api/admin/chains",
  "/admin/audit",
];

// Mutating endpoints that MUST reject an unauthenticated write attempt.
// We send a deliberately empty body; a correct guard rejects on auth BEFORE
// touching state, so this never mutates anything.
const GUARDED_MUTATION = [
  { method: "POST" as const, path: "/admin/killswitch" },
  { method: "PUT" as const, path: "/admin/trading-config/1" },
  { method: "PUT" as const, path: "/admin/cartridge-filters/1" },
  { method: "POST" as const, path: "/api/admin/topology/mutations" },
  { method: "POST" as const, path: "/api/admin/chains" },
];

const REJECT_CODES = [401, 403];

test.describe.configure({ mode: "serial" });

for (const p of GUARDED_GET) {
  test(`guarded GET rejects unauth — ${p}`, async ({ request }) => {
    const res = await request.get(`${EDGE}${p}`, { failOnStatusCode: false });
    expect(
      REJECT_CODES.includes(res.status()),
      `${p} returned ${res.status()}, expected one of ${REJECT_CODES.join("/")}`,
    ).toBeTruthy();
  });
}

for (const m of GUARDED_MUTATION) {
  test(`guarded ${m.method} rejects unauth — ${m.path}`, async ({ request }) => {
    const res = await request.fetch(`${EDGE}${m.path}`, {
      method: m.method,
      data: {},
      failOnStatusCode: false,
    });
    // A correct guard rejects on auth. 404/405 are also acceptable (route not
    // exposed unauth) — what is NOT acceptable is a 2xx (silent acceptance).
    expect(
      res.status() < 300,
      `${m.method} ${m.path} was ACCEPTED unauth (status ${res.status()}) — open control surface!`,
    ).toBeFalsy();
  });
}
