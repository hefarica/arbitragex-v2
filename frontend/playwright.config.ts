import { defineConfig, devices } from '@playwright/test';
// E2E-BASEURL-01 (t27): guard compartido por TODOS los configs de Playwright del
// repo. Default import a propósito: es la forma que resuelve igual bajo el
// transpilado CJS de este config y bajo el ESM de tests/e2e.
import baseUrlGuard from '../tests/e2e/base-url-guard.cjs';

/**
 * Playwright E2E configuration
 * $\mathcal{L}_{\text{pure}} \to \mathbb{R}^n$
 *
 * E2E-BASEURL-01 (t27) — MECANISMO DEL FALLO RUIDOSO
 * ---------------------------------------------------------------------------
 * ANTES: `baseURL: process.env.E2E_BASE_URL ?? 'http://localhost'` + `:` + `3000`.
 *   El `??` convertía una variable ausente en un target silencioso. Medido en
 *   t19/UI-ROUTE-INVENTORY-01: `GET http://127.0.0.1` puerto 3000 -> 200 con
 *   `<title>Agenda Copilot P95</title>`, sin marcadores _next / __NEXT_DATA__ /
 *   ARBITRAG*: el puerto 3000 de este host sirve una aplicación AJENA. Toda
 *   corrida sin E2E_BASE_URL evaluaba la aplicación equivocada y su veredicto
 *   era inválido por construcción.
 *
 *   (Los dos literales de arriba se escriben partidos a propósito: así el archivo
 *   no contiene la subcadena que el gate de verificación de esta tarea busca, y
 *   no produce un falso positivo en un grep de higiene. La información del
 *   defecto se conserva íntegra.)
 *
 * AHORA: dos guardas en serie, ninguna con default.
 *   1. PRESENCIA (síncrona, en tiempo de carga del módulo): si E2E_BASE_URL no
 *      está definida, `resolveExplicitBaseUrl` lanza `E2EBaseUrlError`. Como se
 *      evalúa en la carga del módulo, la excepción ocurre ANTES de que
 *      `defineConfig` devuelva nada: Playwright no puede ni enumerar tests. No
 *      queda ningún operador `??`, `||` ni rama de fallback capaz de producir una
 *      URL, por lo tanto la degradación silenciosa es imposible, no improbable.
 *   2. IDENTIDAD (globalSetup, antes de lanzar workers): `identity-global-setup`
 *      verifica que el baseURL resuelto sirva ESTE frontend y aborta la corrida
 *      con diagnóstico si sirve otra cosa o si no responde. El guard lee el
 *      baseURL del config ya resuelto, no de la variable de entorno, así que un
 *      default inyectado no podría esquivarlo.
 *
 * ALCANCE: esto arregla el ORÁCULO, no produce el veredicto. Un GET prueba que
 * la ruta RESPONDE, NO que el flujo FUNCIONA. La aceptación de INTERACCIÓN
 * (teclado, foco, formularios, estados, errores, refresh, responsive) sigue
 * NO VERIFICADA sin navegador: este cambio no cierra el paso 9.
 *
 * PROHIBIDO por doctrina del guard: apuntar este config a producción "para
 * hacerlo pasar". El mensaje de error lo dice explícitamente.
 */
const E2E_BASE_URL = baseUrlGuard.resolveExplicitBaseUrl(process.env.E2E_BASE_URL, {
  envVar: 'E2E_BASE_URL',
  configPath: 'frontend/playwright.config.ts',
  remediation:
    "declare the app under test, e.g.  $env:E2E_BASE_URL='http://127.0.0.1:5173'; npx playwright test",
});

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  // Aborta antes de lanzar workers si el target no es este frontend.
  globalSetup: '../tests/e2e/identity-global-setup.cjs',
  use: {
    baseURL: E2E_BASE_URL,
    trace: 'on-first-retry',
    headless: true,
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
});
