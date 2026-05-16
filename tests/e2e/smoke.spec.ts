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
 *
 * Heading assertion doctrine
 * --------------------------
 * We deliberately query by ARIA role (`heading`, level=1) instead of the
 * raw CSS selector `h1`. The CSS selector caught false positives in two
 * historical incidents:
 *
 *  1. SSR-rendered <h1> orphaned by a client-side hydration crash. The
 *     element existed in the DOM but `visibility:hidden`, so
 *     `locator("h1").first()` matched it and then failed `toBeVisible`.
 *  2. Pre-render skeleton states that briefly emit a hidden duplicate
 *     <h1>, causing the same first-match-hidden race.
 *
 * `getByRole("heading", { level: 1, name })` matches what an assistive
 * technology would announce: the *visible*, *accessible* level-1
 * heading the user actually perceives. This is stricter, not laxer —
 * it still requires the heading to be rendered, visible, in the
 * accessibility tree, and to carry the expected text.
 *
 * In addition we assert the page never falls back to Next.js' generic
 * client-error overlay (`Application error: a client-side exception …`).
 * That overlay is a level-2 heading, so an `h1` query alone could not
 * detect it; we check for it explicitly.
 */

const PAGES: Array<{ path: string; heading: RegExp }> = [
  { path: "/",              heading: /operator console|arbitragex/i },
  { path: "/status",        heading: /system status/i },
  { path: "/opportunities", heading: /live opportunities|opportunities|live network/i },
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

    // Forbid the Next.js generic client-error overlay. If hydration
    // crashed, this heading is the only one in the DOM and every other
    // assertion below would silently regress.
    const clientErrorOverlay = page.getByRole("heading", {
      name: /application error: a client-side exception/i,
    });
    await expect(
      clientErrorOverlay,
      `client-side exception overlay present on ${path}`,
    ).toHaveCount(0);

    // Semantic, accessibility-tree assertion: the page's level-1
    // heading must be visible AND announce the expected name. This
    // replaces the older `locator("h1").first()` pattern which matched
    // hidden / orphaned SSR nodes.
    const pageHeading = page.getByRole("heading", { level: 1, name: heading });
    await expect(pageHeading, `level-1 heading on ${path}`).toBeVisible();

    // Belt-and-braces: an explicit data-testid on PageHeader provides a
    // stable hook for richer assertions (e.g. on degraded states) without
    // losing the semantic guarantee above.
    const titleNode = page.getByTestId("page-title").first();
    if (await titleNode.count()) {
      await expect(titleNode, `page-title testid on ${path}`).toBeVisible();
    }

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

  // Same guard as above — if the home page crashed in hydration, none of
  // the sidebar links exist.
  const clientErrorOverlay = page.getByRole("heading", {
    name: /application error: a client-side exception/i,
  });
  await expect(clientErrorOverlay).toHaveCount(0);

  for (const { path } of PAGES) {
    // /onboarding is in the Setup group; others in Observe/Control.
    // We just assert each href exists in the sidebar. Mobile viewports may
    // collapse it into a sheet — expand if needed.
    const link = page.locator(`a[href="${path}"]`).first();
    await expect(link, `sidebar link for ${path}`).toBeVisible();
  }
});
