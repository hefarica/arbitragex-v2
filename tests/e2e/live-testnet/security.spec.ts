import { test, expect } from "@playwright/test";

test("LT-301: Block mainnet", async ({ request }) => {
  const res = await request.post("/admin/config/live-testnet", {
    data: { enabled: true, chain_id: 1 }
  });
  expect(res.status()).toBe(403);
});
