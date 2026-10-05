import { defineConfig, devices } from "@playwright/test";
// E2E-BASEURL-01 (t27): guard compartido con frontend/playwright.config.ts.
import baseUrlGuard from "./base-url-guard.cjs";

/**
 * Playwright config for ArbitrageX v2 E2E — PROYECTOS QUE PRUEBAN EL FRONTEND.
 *
 * The tests assume a compose stack is already running. CI brings it up in
 * .github/workflows/e2e.yml; locally you can do:
 *
 *   docker compose -f docker/compose.dev.yml up -d
 *   cd tests/e2e && npm install && npm run install-browsers
 *   ARBX_FRONTEND_URL=http://localhost:5173 npm test
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * E2E-BASEURL-01 (t27) — SIN DEFAULT, FALLO RUIDOSO
 * ═══════════════════════════════════════════════════════════════════════════
 * ANTES: `const FRONTEND_URL = process.env["ARBX_FRONTEND_URL"] ?? "http://localhost:5173"`.
 * Mismo patrón de degradación silenciosa que el config de frontend, con una
 * diferencia MEDIDA (t27, 2026-10-05):
 *   - `frontend/playwright.config.ts` caía a `:3000`, y `:3000` SÍ sirve una
 *     aplicación ajena -> veredicto falso con apariencia de verde.
 *   - este config caía a `:5173`, y `:5173` (y `:5174`) RECHAZAN LA CONEXIÓN:
 *     no hay listener. El modo de fallo era ruidoso *por accidente*, no por
 *     diseño: no medía contra una app ajena HOY, pero resolvería contra
 *     cualquier cosa que ocupara el puerto mañana, sin verificación alguna.
 * Por eso: sin default, y con verificación de identidad antes de lanzar workers.
 * CI es seguro: .github/workflows/e2e.yml:196,214,240 declara
 * `ARBX_FRONTEND_URL=http://localhost:5173` explícitamente.
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * E2E-BASEURL-03 (t29) — EL REQUISITO ES DEL PROYECTO, NO DEL DIRECTORIO
 * ═══════════════════════════════════════════════════════════════════════════
 * DEFECTO QUE ESTA ACOTACIÓN CORRIGE (medido):
 * el requisito de t27 se aplicaba a TODO spec que cargara este archivo, incluido
 * `hot-path-pipeline.spec.ts` — que NO prueba el frontend. El job `playwright`
 * de CI (run 37355501611, job 111916760044, paso 9 "Run hot path pipeline
 * tests") quedó en `failure` por un `E2EBaseUrlError` sobre un spec que nunca
 * navega la UI. La guarda era CORRECTA en su premisa y EXCESIVA en su alcance:
 * bloqueaba un job legítimo.
 *
 * `hot-path-pipeline.spec.ts` usa sólo `request` sobre URLs ABSOLUTAS
 * construidas desde `ARBX_EDGE_URL` (:27) y socket.io sobre `ARBX_WS_URL` (:28);
 * no usa `page` ni URLs relativas, así que `baseURL` le es irrelevante. Su
 * proyecto vive ahora en `playwright.hotpath.config.ts` y NO exige
 * `ARBX_FRONTEND_URL`.
 *
 * MECANISMO — y por qué no pudo ser un condicional dentro de UN config:
 * se MIDIÓ la semántica de Playwright 1.61 con una sonda instrumentada
 * (`__fixtures__/lazy-probe.config.ts`, reejecutable): con `--project=<uno>`,
 * Playwright lee el `use.baseURL` de TODOS los proyectos — la sonda registró
 * `frontend-project:baseURL-read` y `hotpath-project:baseURL-read` incluso
 * filtrando por un solo proyecto. El `use` de los proyectos es EAGER en la carga
 * del config. Por lo tanto un requisito de presencia en tiempo de carga NO puede
 * depender del proyecto seleccionado: la única forma de conservarlo ruidoso
 * (excepción en carga, sin enumerar tests) y a la vez acotarlo es separar el
 * proyecto que no prueba el frontend en su propio config.
 *
 * POR QUÉ NO DEJA UN HUECO: este config sólo sirve specs que navegan la UI, y
 * `testIgnore` excluye explícitamente los que no lo hacen. Cualquier invocación
 * que cargue este archivo falla ruidosamente sin la variable. El proyecto de
 * hot-path tiene su propio config con su propio target declarado y su propio
 * requisito. No hay camino por el que un spec de frontend corra sin target
 * verificado, ni por el que un spec que no toca el frontend quede bloqueado.
 *
 * COBERTURA EN CI (contabilidad explícita): `hot-path-pipeline.spec.ts` sale de
 * `npm test` (paso 3, "Run full suite") y entra por `npm run test:hotpath`
 * (paso 2), que ya lo ejecutaba con `ARBX_EDGE_URL`/`ARBX_WS_URL`/`ARBX_API_URL`.
 * El job sigue cubriendo exactamente los mismos specs: no se pierde cobertura.
 *
 * ALCANCE: arregla el ORÁCULO, no produce el veredicto. La aceptación de
 * INTERACCIÓN sigue NO VERIFICADA sin navegador.
 */

const FRONTEND_URL = baseUrlGuard.resolveExplicitBaseUrl(process.env["ARBX_FRONTEND_URL"], {
  envVar: "ARBX_FRONTEND_URL",
  configPath: 'tests/e2e/playwright.config.ts :: project "frontend"',
  remediation:
    "declare the app under test, e.g.  ARBX_FRONTEND_URL=http://localhost:5173 npm test",
});

// Specs que NO prueban el frontend y por eso NO pertenecen a este config.
// Clasificación MEDIDA sobre el árbol: estos archivos no usan `page.*` ni URLs
// relativas, así que `baseURL` les es ajeno. El único que CI invoca por su
// cuenta —y por tanto el único que este cambio mueve— es hot-path-pipeline.
const NON_FRONTEND_SPECS = ["**/hot-path-pipeline.spec.ts"];

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
  testIgnore: [...CI_IGNORE, ...NON_FRONTEND_SPECS],
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
    // El requisito de ARBX_FRONTEND_URL pertenece a ESTE proyecto: sólo specs
    // que navegan el frontend corren bajo él.
    { name: "frontend", use: { ...devices["Desktop Chrome"] } },
  ],
});
