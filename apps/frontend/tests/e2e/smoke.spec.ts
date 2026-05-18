import { test, expect } from "@playwright/test";

const BASE_URL = process.env.E2E_BASE_URL || "http://localhost:5173";

test.describe("ArbitrageX Frontend Smoke Tests", () => {
  test("homepage loads with navigation", async ({ page }) => {
    await page.goto(BASE_URL);
    await expect(page).toHaveTitle(/ArbitrageX/i);

    const nav = page.locator("nav");
    await expect(nav).toBeVisible();
    await expect(nav.getByText(/opportunities/i)).toBeVisible();
    await expect(nav.getByText(/executions/i)).toBeVisible();
    await expect(nav.getByText(/risk/i)).toBeVisible();
  });

  test("opportunities page displays data grid", async ({ page }) => {
    await page.goto(`${BASE_URL}/opportunities`);
    const grid = page.locator("[data-testid='opportunities-grid']");
    await expect(grid).toBeVisible();

    const headers = grid.locator("th");
    await expect(headers.getByText(/chain/i)).toBeVisible();
    await expect(headers.getByText(/profit/i)).toBeVisible();
  });

  test("executions page loads with filters", async ({ page }) => {
    await page.goto(`${BASE_URL}/executions`);
    const filterPanel = page.locator("[data-testid='execution-filters']");
    await expect(filterPanel).toBeVisible();

    const statusSelect = filterPanel.locator("select[name='status']");
    await expect(statusSelect).toBeVisible();
    await statusSelect.selectOption("confirmed");

    await expect(page.locator("[data-testid='executions-table']")).toBeVisible();
  });

  test("risk dashboard shows guard state", async ({ page }) => {
    await page.goto(`${BASE_URL}/risk`);
    const guardBadge = page.locator("[data-testid='guard-state-badge']");
    await expect(guardBadge).toBeVisible();

    const guardText = await guardBadge.textContent();
    expect(guardText).toMatch(/open|closed|half-open/i);
  });

  test("kill-switch page requires auth", async ({ page }) => {
    await page.goto(`${BASE_URL}/admin/killswitch`);
    const submitButton = page.locator("button[type='submit']");
    await expect(submitButton).toBeDisabled();
  });

  test("navigation between pages works", async ({ page }) => {
    await page.goto(BASE_URL);

    await page.locator("nav").getByText(/executions/i).click();
    await expect(page).toHaveURL(/\/executions/);

    await page.locator("nav").getByText(/risk/i).click();
    await expect(page).toHaveURL(/\/risk/);

    await page.locator("nav").getByText(/opportunities/i).click();
    await expect(page).toHaveURL(/\/opportunities/);
  });

  test("responsive layout on mobile viewport", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto(BASE_URL);

    const menuButton = page.locator("[aria-label='menu']");
    await expect(menuButton).toBeVisible();
  });
});