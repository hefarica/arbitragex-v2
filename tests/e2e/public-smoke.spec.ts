// tests/e2e/public-smoke.spec.ts
// Smoke test publico — valida que el sitio carga correctamente
// desde el navegador real sin caer en chrome-error://chromewebdata/
//
// Doctrina OMEGA: Zero-Mocks · Fail-Closed · R8 Fail-Honest · El Remoto Manda
// No usar .first() · No datos fabricados · No RPC falso

import { test, expect } from "@playwright/test";

const PUBLIC_URL = process.env.PUBLIC_URL || "http://195.201.235.70/";

test.describe("Public Site Smoke", () => {
  test("site loads without chrome-error", async ({ page }) => {
    const errors: string[] = [];
    const consoleErrors: string[] = [];

    page.on("pageerror", (err) => errors.push(err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });

    await page.goto(PUBLIC_URL, { waitUntil: "domcontentloaded", timeout: 30000 });

    // No debe navegar a chrome-error://chromewebdata/
    await expect(page).not.toHaveURL(/chrome-error/);

    // Body debe ser visible
    await expect(page.locator("body")).toBeVisible();

    // No debe contener textos de error conocidos
    await expect(page.locator("body")).not.toContainText(/Application error/i);
    await expect(page.locator("body")).not.toContainText(/edge unreachable/i);
    await expect(page.locator("body")).not.toContainText(/killswitch/i);
  });

  test("site has correct title", async ({ page }) => {
    await page.goto(PUBLIC_URL, { waitUntil: "domcontentloaded", timeout: 30000 });
    await expect(page).toHaveTitle(/QuantumX/);
  });

  test("no critical console errors", async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });

    await page.goto(PUBLIC_URL, { waitUntil: "domcontentloaded", timeout: 30000 });
    await page.waitForTimeout(2000); // dar tiempo a que cargue JS

    // Filtrar errores no criticos (favicon, recursos abortados, 404 menores)
    const criticalErrors = consoleErrors.filter(e =>
      !e.includes("favicon") &&
      !e.includes("net::ERR_ABORTED") &&
      !e.includes("404") &&
      !e.includes("killswitch")
    );

    expect(criticalErrors).toHaveLength(0);
  });
});
