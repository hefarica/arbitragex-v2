import { test, expect } from "@playwright/test";

const BASE = process.env.E2E_BASE_URL ?? "http://localhost:3000";

test.describe("/wallet ContractAdminPanel", () => {
  test("renders ContractAdminPanel heading", async ({ page }) => {
    const response = await page.goto(`${BASE}/wallet`);
    expect(response?.status()).toBe(200);
    await expect(
      page.getByRole("heading", { name: "Contract Administration" })
    ).toBeVisible({ timeout: 15_000 });
  });

  test("prompts to connect wallet when no wallet is connected", async ({ page }) => {
    const response = await page.goto(`${BASE}/wallet`);
    expect(response?.status()).toBe(200);
    await expect(
      page.getByText("Connect your wallet to access contract administration.")
    ).toBeVisible({ timeout: 15_000 });
  });
});
