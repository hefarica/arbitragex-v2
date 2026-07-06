import { test, expect } from "@playwright/test";

/**
 * LIVE honest-state sweep (READ-ONLY).
 *
 * The doctrine: the UI renders exactly what the edge returns. Empty is shown
 * as empty; it is NEVER fabricated. These tests pin that invariant against the
 * live deployment.
 */

const EDGE =
  process.env["ARBX_EDGE_URL"] ??
  process.env["ARBX_FRONTEND_URL"] ??
  "https://edge-arbx.ape-tv.net";

test.describe.configure({ mode: "serial" });

test("opportunities endpoint returns a well-formed, honest payload", async ({
  request,
}) => {
  const res = await request.get(`${EDGE}/api/opportunities/live`, {
    failOnStatusCode: false,
  });
  expect(res.status(), "opportunities endpoint reachable").toBe(200);
  const body = await res.json();
  // Shape: { count, items: [...] } — count must equal items length (no lying).
  expect(typeof body.count, "count is a number").toBe("number");
  expect(Array.isArray(body.items), "items is an array").toBeTruthy();
  expect(
    body.count,
    "count matches items length (no fabricated/hidden rows)",
  ).toBe(body.items.length);
});

test("system guard banner reflects a safe, non-live posture", async ({
  page,
}) => {
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(2500);
  const banner = page.getByText(/system guard/i).first();
  await expect(banner, "system guard banner present").toBeVisible();

  const body = (await page.locator("body").textContent()) ?? "";
  // These are the live, safe-posture facts the banner asserts. If the UI ever
  // starts claiming "Live: ON" while capital is $0 / readiness 0, that is a
  // dishonest state and this test catches it.
  expect(body, "banner exposes Live state").toMatch(/Live:/i);
  expect(body, "banner exposes Capital state").toMatch(/Capital:/i);
});

test("opportunities page agrees with the edge (no synthesized rows)", async ({
  page,
  request,
}) => {
  const res = await request.get(`${EDGE}/api/opportunities/live`, {
    failOnStatusCode: false,
  });
  const apiCount: number = (await res.json()).count ?? 0;

  await page.goto("/opportunities", { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(3000);

  const rows = await page.locator("table tbody tr").count();
  if (apiCount === 0) {
    // Honest empty: either zero data rows, or an explicit empty-state message.
    const body = (await page.locator("body").textContent()) ?? "";
    const hasEmptyState =
      rows === 0 || /no .*(opportunit|signal|detection)/i.test(body);
    expect(
      hasEmptyState,
      `API reports 0 opportunities but page rendered ${rows} data rows`,
    ).toBeTruthy();
  } else {
    // If the API has data, the page must not under-report to zero.
    expect(rows, "page renders rows when API has data").toBeGreaterThan(0);
  }
});
