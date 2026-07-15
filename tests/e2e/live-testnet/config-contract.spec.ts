import { test, expect } from "@playwright/test";

/**
 * LT-CONTRACT: API contract for the live-testnet config surface.
 *
 * These tests protect against accidental regressions in the response shape
 * consumed by `frontend/hooks/useLiveTestnetStatus.ts`.
 */

test("LT-CONTRACT-001: GET response contains required fields", async ({ request }) => {
  const res = await request.get("/api/v1/live-testnet/config");
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
  const res = await request.post("/admin/config/live-testnet", {
    data: { enabled: true, chain_id: 11155111 },
  });
  expect(res.status()).toBe(401);
});
