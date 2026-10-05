/**
 * E2E-BASEURL-01 (t27) — PROOF A runner.
 *
 * Ejecuta la MISMA llamada que frontend/playwright.config.ts hace en tiempo de
 * carga del módulo, con E2E_BASE_URL ausente, y afirma que LANZA.
 *
 * Por qué no se carga el config entero: frontend/playwright.config.ts importa
 * `@playwright/test`, que NO es resoluble desde frontend/ en este checkout
 * (frontend/node_modules/@playwright/test ausente, raíz ausente; el único
 * instalado está en tests/e2e/node_modules). Eso se declara como frontera; este
 * runner cubre la función exacta que el config invoca verbatim.
 *
 * exit 0 = el guard LANZÓ como se exige (PASS de la prueba de mutación)
 * exit 1 = el guard resolvió algo sin lanzar (DEFECTO)
 */
'use strict';

const guard = require('../base-url-guard.cjs');

const CONFIG_OPTS = {
  envVar: 'E2E_BASE_URL',
  configPath: 'frontend/playwright.config.ts',
  remediation:
    "declare the app under test, e.g.  $env:E2E_BASE_URL='http://127.0.0.1:5173'; npx playwright test",
};

// Estado REAL del entorno, sin simular: se borra por si el shell la trae puesta.
delete process.env.E2E_BASE_URL;
const raw = process.env.E2E_BASE_URL;

let threw = false;
let message = '';
try {
  const resolved = guard.resolveExplicitBaseUrl(raw, CONFIG_OPTS);
  console.log(`DEFECTO: resolvio sin lanzar -> ${resolved}`);
} catch (err) {
  threw = true;
  message = err && err.message ? err.message : String(err);
  console.log(`THROWN: ${err && err.name ? err.name : 'Error'}`);
  console.log(message);
}

console.log('');
console.log(`E2E_BASE_URL was ${raw === undefined ? 'UNDEFINED' : JSON.stringify(raw)}`);
console.log(`threw = ${threw}`);
console.log(
  threw
    ? 'PROOF A: PASS — el oraculo falla ruidosamente en vez de degradar a un puerto ajeno.'
    : 'PROOF A: FAIL — el oraculo resolvio silenciosamente. DEFECTO.',
);

process.exit(threw ? 0 : 1);
