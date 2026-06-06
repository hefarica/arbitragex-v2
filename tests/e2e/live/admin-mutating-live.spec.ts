import { test, expect } from "@playwright/test";

/**
 * LIVE mutating admin flow — SELF-REVERTING, token-gated.
 *
 * SKIPPED unless ARBX_ADMIN_TOKEN is set. We never fabricate a token.
 *
 * This exercises the ONE control action that is fully reversible by design:
 * the kill-switch round-trip (arm -> verify on /status -> disarm -> verify).
 * It leaves the platform in the SAME posture it found it in.
 *
 * It does NOT flip the paper feed to live, does NOT change trading-config
 * values, and does NOT touch capital. Per memo, a trading-config PUT that
 * activates the paper feed is an operator-gated decision and is intentionally
 * excluded from automated tests.
 *
 * Run (PowerShell):
 *   $env:ARBX_ADMIN_TOKEN="<token>"
 *   npx playwright test --config=playwright.live.config.ts admin-mutating-live
 */

const ADMIN_TOKEN = process.env["ARBX_ADMIN_TOKEN"];
const gated = ADMIN_TOKEN ? test : test.skip;

test.describe.configure({ mode: "serial" });

gated("kill-switch round-trip: arm -> /status reflects -> disarm -> /status reflects", async ({
  page,
}) => {
  // --- ARM -------------------------------------------------------------
  await page.goto("/killswitch", { waitUntil: "domcontentloaded" });
  await expect(page.locator("h1")).toHaveText(/kill.switch/i);
  await page.getByLabel(/admin token/i).fill(ADMIN_TOKEN!);
  await page.getByLabel(/reason/i).fill("e2e-live: arm (auto-reverting)");
  await page.getByRole("button", { name: /arm/i }).click();
  await expect(page.getByText(/armed/i)).toBeVisible({ timeout: 12_000 });

  // --- /status reflects ARMED -----------------------------------------
  await page.goto("/status", { waitUntil: "domcontentloaded" });
  await expect(page.getByText(/armed/i)).toBeVisible({ timeout: 12_000 });

  // --- DISARM (always restore) ----------------------------------------
  await page.goto("/killswitch", { waitUntil: "domcontentloaded" });
  await page.getByLabel(/admin token/i).fill(ADMIN_TOKEN!);
  await page.getByLabel(/reason/i).fill("e2e-live: disarm (restore baseline)");
  await page.getByRole("button", { name: /^disable$/i }).click();
  await expect(page.getByText(/disabled/i)).toBeVisible({ timeout: 12_000 });

  // --- /status reflects DISARMED --------------------------------------
  await page.goto("/status", { waitUntil: "domcontentloaded" });
  await expect(page.getByText(/disabled/i)).toBeVisible({ timeout: 12_000 });
});

gated("kill-switch refuses a reasonless arm (audit-log guard)", async ({ page }) => {
  await page.goto("/killswitch", { waitUntil: "domcontentloaded" });
  await page.getByLabel(/admin token/i).fill(ADMIN_TOKEN!);
  // Reason intentionally empty.
  await page.getByRole("button", { name: /arm/i }).click();
  await expect(page.getByText(/reason is required/i)).toBeVisible();
});
