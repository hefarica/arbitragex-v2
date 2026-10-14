/**
 * E2E-BASEURL-01 (t27) — globalSetup de identidad para los configs de Playwright.
 * ---------------------------------------------------------------------------
 * Se ejecuta ANTES de que se lance cualquier worker o browser. Si el baseURL
 * resuelto por el config no sirve ESTE frontend (o no responde), la corrida se
 * aborta con el diagnóstico de `base-url-guard.cjs`. Ningún test corre, por lo
 * tanto ningún veredicto puede emitirse contra una aplicación ajena.
 *
 * Lee el baseURL del CONFIG RESUELTO que Playwright entrega como argumento, no
 * de la variable de entorno: se vigila exactamente el valor que las pruebas van
 * a usar, así que un config que inyectara un default no podría esquivar el guard.
 *
 * ALCANCE: esto arregla el ORÁCULO, no produce el veredicto. La aceptación de
 * INTERACCIÓN sigue NO_VERIFICADA sin navegador.
 */

'use strict';

const { assertFrontendIdentity } = require('./base-url-guard.cjs');

module.exports = async function globalSetup(config) {
  const projects = Array.isArray(config && config.projects) ? config.projects : [];
  const perProject = projects.map((p) => p && p.use && p.use.baseURL).filter(Boolean);
  const fallback = config && config.use && config.use.baseURL;
  const resolved = perProject.length > 0 ? perProject[0] : fallback;

  if (!resolved) {
    throw new Error(
      [
        '[E2E-BASEURL-01] The resolved Playwright config has NO baseURL.',
        '',
        'Refusing to run: with no declared target, every navigation would resolve',
        'relative to nothing and the suite would grade an undefined application.',
        'Declare the application under test via the config\'s env var.',
      ].join('\n'),
    );
  }

  const observed = await assertFrontendIdentity(resolved, {
    configPath: (config && config.configFile) || '(config file unknown)',
  });

  const title = observed.title === null ? '(sin <title>)' : JSON.stringify(observed.title);
  const degraded = !observed.chromeIdentity || (observed.httpStatus !== null && observed.httpStatus >= 400);
  process.stdout.write(
    `[E2E-BASEURL-01] identity OK — ${observed.url} ` +
      `(HTTP ${observed.httpStatus}, title=${title}, ${observed.bodyBytes} bytes, ` +
      `basis=${observed.titleIdentity ? 'layout-title' : 'rendered-chrome'}` +
      `${observed.optionalFound.length > 0 ? `, strong=${observed.optionalFound.join('|')}` : ''})\n` +
      (degraded
        ? `[E2E-BASEURL-01] OBSERVED DEGRADATION — identity confirmed, but the probed document is NOT a healthy render ` +
          `(chrome markers missing: ${observed.markersMissing.length}). Health is NOT identity: this guard refuses to ` +
          `grade the WRONG app; grading the UI is the spec's job and it fails honestly on its own assertions.\n`
        : '') +
      '[E2E-BASEURL-01] NOTE: a GET proves the route RESPONDS. It does NOT prove the flow FUNCTIONS.\n',
  );
};
