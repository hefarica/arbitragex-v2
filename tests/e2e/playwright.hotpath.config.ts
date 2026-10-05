import { defineConfig, devices } from "@playwright/test";
// E2E-BASEURL-01 (t27) / E2E-BASEURL-03 (t29): mismo guard compartido.
import baseUrlGuard from "./base-url-guard.cjs";

/**
 * Playwright config para el proyecto HOT-PATH — el que NO prueba el frontend.
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * E2E-BASEURL-03 (t29) — PROYECTO PROPIO CON SU TARGET
 * ═══════════════════════════════════════════════════════════════════════════
 * POR QUÉ EXISTE ESTE ARCHIVO
 * `hot-path-pipeline.spec.ts` fue arrastrado por el requisito global de t27 y el
 * job `playwright` de CI quedó en `failure` (run 37355501611, job 111916760044,
 * paso 9 "Run hot path pipeline tests"). El spec NO prueba el frontend: usa sólo
 * `request` sobre URLs ABSOLUTAS construidas desde `ARBX_EDGE_URL` (:27) y
 * socket.io sobre `ARBX_WS_URL` (:28); no usa `page` ni URLs relativas, así que
 * un `baseURL` de frontend le es irrelevante. Exigírselo era alcance excesivo:
 * la guarda es correcta en su premisa, no en su radio de acción.
 *
 * POR QUÉ UN CONFIG APARTE Y NO UN CONDICIONAL DENTRO DE UNO SOLO
 * Medido, no supuesto: Playwright 1.61 evalúa el `use` de TODOS los proyectos al
 * cargar el config, incluso filtrando con `--project`. La sonda instrumentada
 * `__fixtures__/lazy-probe.config.ts` (reejecutable) registró la lectura de
 * `baseURL` en AMBOS proyectos al correr `--project=probe-hotpath`. Con `use`
 * eager, un requisito de presencia en carga no puede depender del proyecto
 * seleccionado: o es global, o vive en otro archivo. Se eligió otro archivo para
 * conservar el fallo ruidoso EN CARGA (excepción, sin enumerar tests) allí donde
 * importa, que es justo lo que este cambio no debía debilitar.
 *
 * POR QUÉ NO DEJA UN HUECO — el target del hot-path también se declara
 * El spec construye sus URLs desde `ARBX_EDGE_URL`, así que ése ES su target y
 * aquí se exige explícitamente. NO se declara `ARBX_FRONTEND_URL` para "cumplir":
 * sería declarar una URL que este spec no usa, es decir la misma degradación
 * silenciosa disfrazada. CI ya declara el edge en .github/workflows/e2e.yml:203.
 *
 * QUÉ SE CONSERVA DE LA GUARDA
 * La guarda de identidad (E2EBaseUrlError) sigue viva para los proyectos que sí
 * prueban el frontend (`playwright.config.ts` y `frontend/playwright.config.ts`)
 * y su sensibilidad a mutación está demostrada por ejecución. Aquí se usa el
 * guard 1 —presencia, sin default— sobre el target que el spec realmente usa.
 *
 * ALCANCE: arregla el ORÁCULO, no produce el veredicto. Un GET prueba que la ruta
 * RESPONDE, no que el flujo FUNCIONA. Esto no cierra el paso 9.
 */

const EDGE_URL = baseUrlGuard.resolveExplicitBaseUrl(process.env["ARBX_EDGE_URL"], {
  envVar: "ARBX_EDGE_URL",
  configPath: 'tests/e2e/playwright.hotpath.config.ts :: project "hot-path"',
  remediation:
    "declare the hot-path target, e.g.  ARBX_EDGE_URL=http://localhost:8787 npm run test:hotpath",
});

export default defineConfig({
  testDir: ".",
  // Sólo el pipeline de hot-path. Nada de specs de UI bajo este config.
  testMatch: ["hot-path-pipeline.spec.ts"],
  timeout: 30_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  forbidOnly: Boolean(process.env["CI"]),
  retries: process.env["CI"] ? 1 : 0,
  workers: 1,
  reporter: [
    ["list"],
    ["html", { open: "never", outputFolder: "playwright-report-hotpath" }],
  ],
  use: {
    // El target declarado es el EDGE: es lo que el spec usa de verdad.
    // No hay verificación de identidad de frontend aquí a propósito: este
    // proyecto no prueba el frontend, y afirmar lo contrario sería mentir.
    baseURL: EDGE_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    viewport: { width: 1440, height: 900 },
  },
  projects: [
    { name: "hot-path", use: { ...devices["Desktop Chrome"] } },
  ],
});
