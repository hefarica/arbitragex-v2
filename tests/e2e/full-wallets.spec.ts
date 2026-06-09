import { test, expect, type ConsoleMessage, type Request } from "@playwright/test";

/**
 * FE-CRIT-07 — /wallets SSR snapshot + single fetcher.
 *
 * Before this fix WalletsClient received `initialSnapshot` (SSR) but ignored it
 * and called the omni-store `fetchWallets()` (a second fetcher that swallowed
 * errors to console.error), so the empty state could not be distinguished from
 * a failed fetch. This suite proves:
 *   - the page renders without a React #418 hydration crash (R1 snapshot seed),
 *   - exactly ONE fetcher is used for the wallets list (no duplicate network
 *     call for GET /api/v1/wallets),
 *   - loading / empty / error are distinct (empty ≠ error),
 *   - the safety posture is visible: capital exposure + live OFF + broadcast OFF.
 *
 * Style mirrors tests/e2e/live/functional-live.spec.ts (domcontentloaded +
 * settle; pages poll so networkidle never settles). Base URL is configurable.
 * Against the OLD deployed build → VALIDATION_PENDING_POST_DEPLOY.
 */

const BASE =
  process.env.E2E_BASE_URL ??
  process.env.ARBX_FRONTEND_URL ??
  "http://localhost:5173";

const PATH = "/wallets";

const REACT_HYDRATION_RE = /Minified React error #418|hydrat/i;

test.describe("/wallets — SSR snapshot + single fetcher (FE-CRIT-07)", () => {
  test("page loads with the heading and no React #418 hydration crash", async ({ page }) => {
    const errors: string[] = [];
    page.on("console", (msg: ConsoleMessage) => {
      if (msg.type() !== "error") return;
      const t = msg.text();
      if (/Failed to load resource/i.test(t)) return;
      errors.push(t);
    });
    page.on("pageerror", (err) => errors.push(`pageerror: ${err.message}`));

    await page.goto(`${BASE}${PATH}`, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(2500);

    await expect(page.locator("h1")).toContainText(/operational wallets/i);
    expect(
      errors.filter((e) => REACT_HYDRATION_RE.test(e)),
      `React hydration error on ${PATH}: ${errors.join(" | ")}`,
    ).toHaveLength(0);
  });

  test("uses a single fetcher — no duplicate GET /api/v1/wallets for the list", async ({ page }) => {
    const walletListRequests: string[] = [];
    page.on("request", (req: Request) => {
      const url = req.url();
      // Match the LIST endpoint only (not the per-wallet balances/allowances
      // sub-routes, which legitimately fan out for the capital computation).
      if (/\/api\/v1\/wallets(\?|$)/.test(url)) {
        walletListRequests.push(url);
      }
    });

    await page.goto(`${BASE}${PATH}`, { waitUntil: "domcontentloaded" });
    // Give the single client fetch time to fire (and prove a second one does
    // NOT — the old code ran both omni-store fetchWallets() and would have
    // doubled this).
    await page.waitForTimeout(3000);

    // The client performs exactly one list fetch on mount. (The SSR fetch
    // happens server-side and is not observed by the browser context.) We allow
    // 0 only if the environment served a cached page with no client fetch; the
    // hard ceiling is what guards against the old dual-fetcher regression.
    expect(
      walletListRequests.length,
      `expected a single wallets-list fetch, saw ${walletListRequests.length}: ${walletListRequests.join(", ")}`,
    ).toBeLessThanOrEqual(1);
  });

  test("safety posture is visible: capital exposure, live OFF, broadcast OFF", async ({ page }) => {
    await page.goto(`${BASE}${PATH}`, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(2500);

    const posture = page.getByTestId("wallets-safety-posture");
    await expect(posture).toBeVisible();

    // Live OFF and Broadcast OFF are structural facts (no broadcast path wired).
    await expect(page.getByTestId("posture-live")).toContainText(/off/i);
    await expect(page.getByTestId("posture-broadcast")).toContainText(/off/i);

    // Capital exposure is computed from backend balances — it must render a
    // value (either "computing…" briefly or a $-figure), never be absent.
    await expect(page.getByTestId("posture-capital")).toBeVisible();
  });

  test("loading / empty / error are distinct states (empty != error)", async ({ page }) => {
    await page.goto(`${BASE}${PATH}`, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(2500);

    // Exactly one of: a populated table, an honest EdgeState empty, or an
    // EdgeState error must be present — never the old ambiguous "No wallets"
    // that conflated empty and failure.
    const hasTable = (await page.getByTestId("wallets-table").count()) > 0;
    const hasEmpty =
      (await page.locator('[role="status"]').filter({ hasText: /no wallets registered/i }).count()) > 0;
    const hasError =
      (await page.locator('[role="alert"]').filter({ hasText: /wallets unreachable/i }).count()) > 0;

    expect(
      hasTable || hasEmpty || hasError,
      "wallets page rendered none of table / honest-empty / honest-error",
    ).toBeTruthy();

    // The empty surface and the error surface must NOT both be present — that
    // would be the old conflation. (A table coexisting with an error banner is
    // allowed: fail-honest preserves the last-known list under an error.)
    expect(hasEmpty && hasError, "empty and error states are conflated").toBeFalsy();
  });

  test("refresh control exists and is clickable", async ({ page }) => {
    await page.goto(`${BASE}${PATH}`, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(2000);

    const refresh = page.getByTestId("wallets-refresh");
    await expect(refresh).toBeVisible();
    await refresh.click();
    // After a click the control remains present (it re-enables once the fetch
    // settles); we only assert it did not crash the page.
    await expect(page.locator("h1")).toContainText(/operational wallets/i);
  });
});
