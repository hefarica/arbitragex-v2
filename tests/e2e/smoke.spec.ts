import { test, expect } from "@playwright/test";

/**
 * Smoke — every page in the operator console renders and does not show
 * the "edge unreachable" error banner that our pages render when the
 * edge is down.
 *
 * This test does NOT assert data correctness OR heading copy. It only
 * asserts that the frontend → edge → api-server chain is healthy end-to-end:
 * the route responds <400, mounts its h1, and shows no upstream-error banner.
 * Empty tables / "No opportunities yet" states ARE accepted — they are
 * honest states per the no-hardcode doctrine.
 */

const PAGES = [
  "/",
  "/status",
  "/opportunities",
  "/executions",
  "/risk",
  "/recon",
  "/config",
  "/killswitch",
  "/onboarding",
];

for (const path of PAGES) {
  test(`page ${path} renders`, async ({ page }) => {
    const response = await page.goto(path);
    expect(response?.status(), `HTTP status for ${path}`).toBeLessThan(400);

    // Health-check only: the page mounts an h1. We deliberately do NOT assert
    // the h1 text — headings are cosmetic and change with rebrands; coupling
    // the smoke suite to copy makes it brittle without testing chain health.
    await expect(page.locator("h1").first()).toBeVisible();

    // Forbid the two error banners our pages use when upstream is unhealthy.
    // (Empty-state strings like "No opportunities" ARE allowed.)
    await expect(page.getByText(/edge unreachable/i)).toHaveCount(0);
    await expect(page.getByText(/edge error:/i)).toHaveCount(0);
  });
}

test("nav sidebar lists every page we registered", async ({ page }) => {
  await page.goto("/");
  for (const path of PAGES) {
    // Assert at least one link to each route exists. "/" legitimately has two
    // (the logo + the Home nav item), so match the first to avoid strict-mode
    // violations — the smoke check is "the route is reachable from the nav".
    const link = page.locator(`a[href="${path}"]`).first();
    await expect(link, `sidebar link for ${path}`).toBeVisible();
  }
});
