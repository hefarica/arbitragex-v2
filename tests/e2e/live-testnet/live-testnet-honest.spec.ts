import { test, expect } from "@playwright/test";

/**
 * LT-HONEST: Live-testnet config surface must be honest.
 *
 * These tests assert that the new TS route returns:
 *   - mode = LIVE_TESTNET
 *   - paper_mode = true
 *   - can_execute = false
 *   - mainnet_blocked = true
 *   - blockers are present (readiness decision still says NO_GO)
 *
 * The admin token is read from ARBX_ADMIN_TOKEN env var so CI can run against
 * a real deployment without hardcoding secrets.
 */

const ADMIN_TOKEN = process.env["ARBX_ADMIN_TOKEN"] ?? "dev_admin_token_change_me_0123456789";
const EDGE_URL = process.env["ARBX_EDGE_URL"] ?? "http://localhost:8787";

test("LT-HONEST-001: GET /api/v1/live-testnet/config is honest", async ({ request }) => {
  const res = await request.get(`${EDGE_URL}/api/v1/live-testnet/config`);
  expect(res.status(), `GET status (body preview)`).toBe(200);
  const body = await res.json();
  expect(String(body.mode), "mode LIVE_TESTNET").toMatch(/^live_testnet$/i);
  expect(body.paper_mode).toBe(true);
  expect(body.can_execute).toBe(false);
  expect(body.mainnet_blocked).toBe(true);
  expect(Array.isArray(body.allowed_chain_ids)).toBe(true);
  expect(body.allowed_chain_ids).not.toContain(1);
  expect(Array.isArray(body.blockers)).toBe(true);
});

test("LT-HONEST-002: POST /admin/config/live-testnet blocks mainnet", async ({ request }) => {
  const res = await request.post(`${EDGE_URL}/admin/config/live-testnet`, {
    data: { enabled: true, chain_id: 1 },
    headers: { "x-arbx-admin-token": ADMIN_TOKEN },
  });
  expect(res.status()).toBe(403);
  const body = await res.json();
  expect(String(body.error)).toMatch(/^mainnet_blocked$/i);
});

test("LT-HONEST-003: POST /admin/config/live-testnet rejects unsupported chain", async ({ request }) => {
  const res = await request.post(`${EDGE_URL}/admin/config/live-testnet`, {
    data: { enabled: true, chain_id: 999999999 },
    headers: { "x-arbx-admin-token": ADMIN_TOKEN },
  });
  expect(res.status()).toBe(400);
  const body = await res.json();
  expect(String(body.error)).toMatch(/^unsupported_chain$/i);
});

test("LT-HONEST-004: POST /admin/config/live-testnet accepts Sepolia and stays honest", async ({ request }) => {
  const res = await request.post(`${EDGE_URL}/admin/config/live-testnet`, {
    data: { enabled: true, chain_id: 11155111 },
    headers: { "x-arbx-admin-token": ADMIN_TOKEN },
  });
  expect(res.status()).toBe(200);
  const body = await res.json();
  expect(String(body.mode), "mode LIVE_TESTNET").toMatch(/^live_testnet$/i);
  expect(body.paper_mode).toBe(true);
  expect(body.can_execute).toBe(false);
  expect(body.chain_id).toBe(11155111);
});
