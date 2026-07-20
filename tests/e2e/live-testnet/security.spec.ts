import { test, expect } from "@playwright/test";

const EDGE_URL = process.env["ARBX_EDGE_URL"] ?? "http://localhost:8787";

test("LT-301: Block mainnet", async ({ request }) => {
  const res = await request.post(`${EDGE_URL}/admin/config/live-testnet`, {
    data: { enabled: true, chain_id: 1 }
  });
  expect(res.status()).toBe(403);
});
