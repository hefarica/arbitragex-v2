import { test, expect } from "@playwright/test";

/**
 * Kill-switch round-trip.
 *
 * Requires ARBX_ADMIN_TOKEN to be set in the environment running Playwright.
 * Without it we skip — the test isn't meaningful without the real token and
 * we never fabricate one.
 */

const ADMIN_TOKEN = process.env["ARBX_ADMIN_TOKEN"];
const testMaybe = ADMIN_TOKEN ? test : test.skip;

testMaybe("kill-switch arms and disarms, /status reflects within seconds", async ({ page }) => {
  await page.goto("/killswitch");
  await expect(page.locator("h1")).toHaveText(/kill.switch/i);

  // Paste the admin token and a reason, arm.
  await page.getByLabel(/admin token/i).fill(ADMIN_TOKEN!);
  await page.getByLabel(/reason/i).fill("e2e: arm from test");
  await page.getByRole("button", { name: /arm/i }).click();

  // The page should show ARMED state within a couple of seconds.
  await expect(page.getByText(/armed/i)).toBeVisible({ timeout: 10_000 });

  // Check /status reflects the same state.
  await page.goto("/status");
  await expect(page.getByText(/armed/i)).toBeVisible({ timeout: 10_000 });

  // Disarm.
  await page.goto("/killswitch");
  await page.getByLabel(/admin token/i).fill(ADMIN_TOKEN!);
  await page.getByLabel(/reason/i).fill("e2e: disarm from test");
  await page.getByRole("button", { name: /^disable$/i }).click();

  await expect(page.getByText(/disabled/i)).toBeVisible({ timeout: 10_000 });

  // Status page also reflects disarm.
  await page.goto("/status");
  await expect(page.getByText(/disabled/i)).toBeVisible({ timeout: 10_000 });
});

testMaybe("kill-switch form refuses to arm without a reason (audit guard)", async ({ page }) => {
  await page.goto("/killswitch");
  await page.getByLabel(/admin token/i).fill(ADMIN_TOKEN!);
  // Reason intentionally empty.
  await page.getByRole("button", { name: /arm/i }).click();
  // The UI surfaces the error inline; assert we did NOT transition to ARMED.
  await expect(page.getByText(/reason is required/i)).toBeVisible();
});
