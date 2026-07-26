import { test, expect } from "@playwright/test";

/**
 * Web3 safe-gated wallet — API-contract sweep (READ-ONLY, at the EDGE).
 *
 * Pins the doctrine for the wallet/auth read surface:
 *   • Safety posture is HARDCODED-safe: live_enabled=false, capital_exposed=0,
 *     broadcast=false on every endpoint, regardless of data.
 *   • Intent ALWAYS terminates at BROADCAST_DISABLED with broadcast_allowed=false.
 *   • Simulation never enables broadcast (fail-honest when no runtime).
 *   • SIWE nonce/verify/session respond honestly (invalid signature → honest
 *     fail, unauthenticated session → honest unauthenticated state).
 *
 * BASE must point at the EDGE (8787) — the /api/* proxying lives there. If the
 * edge is not reachable the suite SKIPS honestly (VALIDATION_PENDING_POST_DEPLOY).
 */

const BASE =
  process.env["E2E_BASE_URL"] ??
  process.env["ARBX_EDGE_URL"] ??
  "http://localhost:8787";

test.describe.configure({ mode: "serial" });

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

function assertSafePosture(body: Record<string, unknown>): void {
  expect(body.live_enabled, "live_enabled false").toBe(false);
  expect(body.capital_exposed, "capital_exposed 0").toBe(0);
  expect(body.broadcast, "broadcast false").toBe(false);
}

test("GET /api/wallet/status — read-only safe posture", async ({ request }) => {
  const res = await request.get(`${BASE}/api/wallet/status`, { failOnStatusCode: false });
  expect(res.status(), "wallet status reachable").toBe(200);
  const body = await res.json();
  expect(body.read_only, "read_only true").toBe(true);
  expect(body.broadcast_allowed, "broadcast_allowed false").toBe(false);
  assertSafePosture(body);
});

test("GET /api/wallet/safety — gate matrix, broadcast disabled", async ({ request }) => {
  const res = await request.get(`${BASE}/api/wallet/safety`, { failOnStatusCode: false });
  expect(res.status(), "wallet safety reachable").toBe(200);
  const body = await res.json();
  assertSafePosture(body);
  expect(body.submit_enabled, "submit_enabled false").toBe(false);
  expect(body.simulation_required, "simulation_required true").toBe(true);
  expect(Array.isArray(body.gates), "gates is an array").toBeTruthy();
  const exposure = body.gates.find((g: { name?: string }) => g.name === "capital_exposure");
  expect(exposure, "capital_exposure gate present").toBeTruthy();
  expect(exposure.status.toLowerCase(), "capital_exposure PASS").toBe("pass");
  expect(exposure.value, "capital_exposure value 0").toBe(0);
});

test("POST /api/wallet/intent — terminates at BROADCAST_DISABLED", async ({ request }) => {
  const res = await request.post(`${BASE}/api/wallet/intent`, {
    failOnStatusCode: false,
    data: { kind: "swap", note: "e2e-contract" },
  });
  expect(res.status(), "intent reachable").toBe(200);
  const body = await res.json();
  // Accept either canonical UPPER_SNAKE or accidental lower-case drift from proxies.
  expect(String(body.state), "terminal state BROADCAST_DISABLED").toMatch(/^broadcast_disabled$/i);
  expect(body.broadcast_allowed, "broadcast_allowed false").toBe(false);
  expect(String(body.reason), "reason broadcast_disabled_by_policy").toMatch(/^broadcast_disabled_by_policy$/i);
  assertSafePosture(body);
});

test("POST /api/wallet/simulate — never enables broadcast", async ({ request }) => {
  const res = await request.post(`${BASE}/api/wallet/simulate`, {
    failOnStatusCode: false,
    data: { kind: "swap" },
  });
  expect(res.status(), "simulate reachable").toBe(200);
  const body = await res.json();
  expect(body.broadcast_allowed, "broadcast_allowed false").toBe(false);
  assertSafePosture(body);
});

test("GET /api/auth/siwe/nonce — honest (nonce or unavailable)", async ({ request }) => {
  const res = await request.get(`${BASE}/api/auth/siwe/nonce`, { failOnStatusCode: false });
  expect(res.status(), "nonce reachable").toBe(200);
  const body = await res.json();
  // Either a real nonce (runtime configured) or honest unavailable — both safe.
  expect(
    body.status === "ok" || body.status === "unavailable",
    "nonce status ok|unavailable",
  ).toBeTruthy();
  assertSafePosture(body);
  if (body.status === "ok") {
    expect(typeof body.nonce, "nonce is a string").toBe("string");
    expect(body.nonce.length, "nonce non-trivial").toBeGreaterThanOrEqual(8);
  } else {
    expect(body.reason, "unavailable carries a reason").toBeTruthy();
  }
});

test("POST /api/auth/siwe/verify — invalid signature fails honestly", async ({ request }) => {
  const res = await request.post(`${BASE}/api/auth/siwe/verify`, {
    failOnStatusCode: false,
    data: { message: "not-a-valid-siwe-message", signature: "0xdeadbeef" },
  });
  // 400 (unparseable / missing) or 401 (verification failed) or 200 unavailable
  // (runtime not configured) — NEVER a 200 authenticated:true on junk input.
  const body = await res.json();
  expect([200, 400, 401]).toContain(res.status());
  expect(body.authenticated, "never authenticated on junk").not.toBe(true);
  assertSafePosture(body);
});

test("GET /api/auth/session — unauthenticated state is honest", async ({ request }) => {
  const res = await request.get(`${BASE}/api/auth/session`, { failOnStatusCode: false });
  expect(res.status(), "session reachable").toBe(200);
  const body = await res.json();
  expect(body.authenticated, "unauthenticated by default").toBe(false);
  expect(body.session_mode, "identity-only session mode").toBe("wallet_identity_only");
  assertSafePosture(body);
});

test("no private-key material anywhere in the wallet API surface", async ({ request }) => {
  const paths = ["/api/wallet/status", "/api/wallet/safety", "/api/auth/session"];
  const forbidden = /private[_-]?key|mnemonic|seed phrase|eth_sendRawTransaction|eth_sendBundle/i;
  for (const p of paths) {
    const res = await request.get(`${BASE}${p}`, { failOnStatusCode: false });
    const txt = await res.text();
    expect(forbidden.test(txt), `${p} must not leak key/broadcast terms`).toBeFalsy();
  }
});
