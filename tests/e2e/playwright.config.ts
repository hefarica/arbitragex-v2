import { defineConfig, devices } from "@playwright/test";
// E2E-BASEURL-01 (t27): guard compartido con frontend/playwright.config.ts.
import baseUrlGuard from "./base-url-guard.cjs";

/**
 * Playwright config for ArbitrageX v2 E2E.
 *
 * The tests assume a compose stack is already running. CI brings it up in
 * .github/workflows/e2e.yml; locally you can do:
 *
 *   docker compose -f docker/compose.dev.yml up -d
 *   cd tests/e2e && npm install && npm run install-browsers
 *   ARBX_FRONTEND_URL=http://localhost:5173 npm test
 *
 * E2E-BASEURL-01 (t27) — ESTADO DE ESTE CONFIG EN LA MISMA PREGUNTA QUE EL OTRO
 * ---------------------------------------------------------------------------
 * ANTES: `const FRONTEND_URL = process.env["ARBX_FRONTEND_URL"] ?? "http://localhost:5173"`.
 * Mismo patrón de degradación silenciosa que el config de frontend, con una
 * diferencia MEDIDA (t27, 2026-10-05):
 *   - `frontend/playwright.config.ts` caía a `:3000`, y `:3000` SÍ sirve una
 *     aplicación ajena -> veredicto falso con apariencia de verde.
 *   - este config caía a `:5173`, y `:5173` (y `:5174`) RECHAZAN LA CONEXIÓN:
 *     no hay listener. El modo de fallo era por tanto ruidoso *por accidente*,
 *     no por diseño: no medía contra una app ajena HOY, pero resolvería contra
 *     cualquier cosa que ocupara el puerto mañana, sin verificación alguna.
 *     "No hay nada escuchando" no es una garantía: es una coincidencia.
 * Por eso se aplica la MISMA corrección: sin default, y con verificación de
 * identidad antes de lanzar workers.
 * CI es seguro: .github/workflows/e2e.yml:196,214,240 ya declara
 * `ARBX_FRONTEND_URL=http://localhost:5173` explícitamente (y :208 `ARBX_API_URL`),
 * así que exigir la variable no rompe la corrida de CI.
 *
 * ALCANCE: arregla el ORÁCULO, no produce el veredicto. La aceptación de
 * INTERACCIÓN sigue NO VERIFICADA sin navegador.
 */

const FRONTEND_URL = baseUrlGuard.resolveExplicitBaseUrl(process.env["ARBX_FRONTEND_URL"], {
  envVar: "ARBX_FRONTEND_URL",
  configPath: "tests/e2e/playwright.config.ts",
  remediation:
    "declare the app under test, e.g.  ARBX_FRONTEND_URL=http://localhost:5173 npm test",
});

// live/ tests have their own playwright.live.config.ts and require the full
// VPS stack (Chains, RPCs, Pools admin endpoints, topology/snapshot API, etc.).
// They must never run via `npm test` — only via `npm run test:live`.
//
// rpc-down tests require searcher-rs to be running in the compose stack.
// The CI e2e.yml only starts postgres/redis/frontend/edge/api-server, so
// searcher-rs is absent and the UP assertion would always fail. Exclude when
// ARBX_ASSUME_NO_RPC=1 (the CI partial-compose signal).
const NO_RPC = process.env["ARBX_ASSUME_NO_RPC"] === "1";
const ALWAYS_IGNORE = ["**/live/**"];
const CI_IGNORE = NO_RPC
  ? [...ALWAYS_IGNORE, "rpc-down.spec.ts"]
  : ALWAYS_IGNORE;

export default defineConfig({
  testDir: ".",
  testIgnore: CI_IGNORE,
  timeout: 30_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,          // operator console is stateful; serialize
  forbidOnly: Boolean(process.env["CI"]),
  retries: process.env["CI"] ? 1 : 0,
  workers: 1,
  reporter: [
    ["list"],
    ["html", { open: "never", outputFolder: "playwright-report" }],
  ],
  // E2E-BASEURL-01: aborta antes de lanzar workers si el target no es este frontend.
  globalSetup: "./identity-global-setup.cjs",
  use: {
    baseURL: FRONTEND_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    viewport: { width: 1440, height: 900 },
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
});
