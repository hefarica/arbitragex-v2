import { test, expect } from "@playwright/test";

/**
 * Smoke — every page in the operator console renders and does not show
 * the "edge unreachable" error banner that our pages render when the
 * edge is down.
 *
 * This test does NOT assert data correctness. It only asserts that the
 * frontend → edge → api-server chain is healthy end-to-end.
 *
 * Empty tables / "No opportunities yet" states ARE accepted — they are
 * honest states per the no-hardcode doctrine.
 */

const PAGES: Array<{ path: string; heading: RegExp }> = [
  { path: "/",              heading: /operator console|arbitragex/i },
  { path: "/status",        heading: /system status/i },
  { path: "/opportunities", heading: /live opportunities|opportunities/i },
  { path: "/executions",    heading: /executions/i },
  { path: "/risk",          heading: /risk.*alerts|risk/i },
  { path: "/recon",         heading: /recon/i },
  { path: "/config",        heading: /current config|config/i },
  { path: "/killswitch",    heading: /kill.switch/i },
  { path: "/onboarding",    heading: /onboarding/i },
];

for (const { path, heading } of PAGES) {
  test(`page ${path} renders`, async ({ page }) => {
    const response = await page.goto(path);
    expect(response?.status(), `HTTP status for ${path}`).toBeLessThan(400);

    // The h1 is always set on every page per our PageHeader pattern.
    await expect(page.locator("h1").first()).toBeVisible();
    await expect(page.locator("h1").first()).toHaveText(heading);

    // Forbid the two error banners our pages use when upstream is unhealthy.
    // (Empty-state strings like "No opportunities" ARE allowed.)
    const edgeUnreachable = page.getByText(/edge unreachable/i);
    const edgeError = page.getByText(/edge error:/i);
    await expect(edgeUnreachable).toHaveCount(0);
    await expect(edgeError).toHaveCount(0);
  });
}

test("nav sidebar lists every page we registered", async ({ page }) => {
  await page.goto("/");
  for (const { path } of PAGES) {
    // /onboarding is in the Setup group; others in Observe/Control.
    // We just assert each href exists in the sidebar. Mobile viewports may
    // collapse it into a sheet — expand if needed.
    const link = page.locator(`a[href="${path}"]`).first();
    await expect(link, `sidebar link for ${path}`).toBeVisible();
  }
});
