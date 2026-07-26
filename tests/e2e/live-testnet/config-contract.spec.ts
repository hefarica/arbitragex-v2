import { test, expect } from "@playwright/test";

/**
 * LT-CONTRACT: API contract for the live-testnet config surface.
 *
 * These tests protect against accidental regressions in the response shape
 * consumed by `frontend/hooks/useLiveTestnetStatus.ts`.
 *
 * The admin token is read from ARBX_ADMIN_TOKEN env var so CI can run against
 * a real deployment without hardcoding secrets.
 */

const ADMIN_TOKEN = process.env["ARBX_ADMIN_TOKEN"] ?? "dev_admin_token_change_me_0123456789";
const EDGE_URL = process.env["ARBX_EDGE_URL"] ?? "http://localhost:8787";

test("LT-CONTRACT-001: GET response contains required fields", async ({ request }) => {
  const res = await request.get(`${EDGE_URL}/api/v1/live-testnet/config`);
  expect(res.status()).toBe(200);
  const body = await res.json();
  const required = [
    "mode",
    "enabled",
    "chain_id",
    "allowed_chain_ids",
    "mainnet_blocked",
    "can_execute",
    "paper_mode",
    "blockers",
    "generated_at",
  ];
  for (const key of required) {
    expect(body).toHaveProperty(key);
  }
  expect(typeof body.generated_at).toBe("string");
});

test("LT-CONTRACT-002: POST without admin token is rejected", async ({ request }) => {
  const res = await request.post(`${EDGE_URL}/admin/config/live-testnet`, {
    data: { enabled: true, chain_id: 11155111 },
  });
  expect(res.status()).toBe(401);
});

test("LT-CONTRACT-003: POST with admin token returns contract shape", async ({ request }) => {
  const res = await request.post(`${EDGE_URL}/admin/config/live-testnet`, {
    data: { enabled: true, chain_id: 11155111 },
    headers: { "x-arbx-admin-token": ADMIN_TOKEN },
  });
  expect(res.status()).toBe(200);
  const body = await res.json();
  expect(String(body.mode), "mode LIVE_TESTNET").toMatch(/^live_testnet$/i);
  expect(typeof body.chain_id).toBe("number");
  expect(Array.isArray(body.allowed_chain_ids)).toBe(true);
});
