import { test, expect } from "@playwright/test";

const EDGE_URL = process.env["ARBX_EDGE_URL"] ?? "http://localhost:8787";
const ADMIN_TOKEN = process.env["ARBX_ADMIN_TOKEN"] ?? "dev_admin_token_change_me_0123456789";

test("LT-301: Block mainnet", async ({ request }) => {
  // Must authenticate first — without admin token the edge returns 401 before
  // the mainnet gate can fire. With a valid token, chain_id=1 is MAINNET_BLOCKED.
  const res = await request.post(`${EDGE_URL}/admin/config/live-testnet`, {
    data: { enabled: true, chain_id: 1 },
    headers: { "x-arbx-admin-token": ADMIN_TOKEN },
  });
  expect(res.status()).toBe(403);
  const body = await res.json();
  expect(String(body.error)).toMatch(/^mainnet_blocked$/i);
});
