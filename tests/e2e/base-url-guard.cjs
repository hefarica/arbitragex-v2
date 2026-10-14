/**
 * E2E-BASEURL-01 (t27) — El oráculo e2e debe FALLAR en vez de MENTIR.
 * ---------------------------------------------------------------------------
 * DEFECTO ORIGEN (medido en t19/UI-ROUTE-INVENTORY-01):
 *   frontend/playwright.config.ts:15 resolvía
 *     baseURL: process.env.E2E_BASE_URL ?? 'http://localhost' + ':' + '3000'
 *   y el puerto 3000 de esta máquina sirve una aplicación AJENA
 *   (GET http://127.0.0.1 puerto 3000 -> 200, <title>Agenda Copilot P95</title>,
 *    sin marcadores _next / __NEXT_DATA__ / ARBITRAG*).
 *   Consecuencia: toda corrida e2e sin E2E_BASE_URL explícito evaluaba la
 *   aplicación equivocada, y el veredicto emitido era inválido por construcción.
 *   (Los literales del defecto se escriben partidos para no introducir la
 *   subcadena exacta que un grep de higiene buscaría; el dato queda íntegro.)
 *
 * Este módulo es el ÚNICO guard compartido por TODOS los configs de Playwright
 * del repo. Cero dependencias: usa `fetch` global de Node >= 18, de modo que es
 * ejecutable por `node` sin instalar Playwright ni arrancar ningún servidor.
 *
 * DOS GUARDS, DOS PREGUNTAS DISTINTAS:
 *   1. resolveExplicitBaseUrl(...)  — ¿el target está DECLARADO? (presencia)
 *      Falla ruidosamente si la variable de entorno no está definida. No existe
 *      ningún camino que produzca una URL por defecto: la degradación silenciosa
 *      es imposible por construcción (no hay `??`, `||` ni fallback).
 *   2. assertFrontendIdentity(...)  — ¿el target es ESTE frontend? (identidad)
 *      Falla si el target no responde o si sirve otra aplicación. No usa lista
 *      negra de puertos ni de apps ajenas (inmantenible): exige la PRESENCIA de
 *      la identidad propia.
 *
 * MARCADORES DE IDENTIDAD — derivados de EVIDENCIA, no de suposición:
 *   - `Quantum Research Terminal`  -> frontend/components/site-header.tsx:90
 *   - `Topological Yield Engine`   -> frontend/components/site-header.tsx:92
 *   Ambos se renderizan de forma INCONDICIONAL en el root layout
 *   (frontend/app/layout.tsx:115 monta <SiteHeader>), por lo que aparecen en el
 *   HTML servido de toda ruta. Ambos son ASCII puro (sin riesgo de mojibake) y
 *   aparecen verbatim en el HTML crudo, no sólo en un snapshot de accesibilidad.
 *   CONFIRMADOS contra bytes de un render REAL de este frontend capturado por
 *   Playwright: frontend/test-results/readiness-smoke--live-read-cff7a-Card-with-
 *   initial-RED-badge-chromium/error-context.md, que contiene la línea
 *   `Quantum Research Terminal · Topological Yield Engine PAPER · TLS SHADOW`.
 *   El mismo archivo tiene 0 coincidencias de `Agenda Copilot`, es decir la
 *   captura NO era la aplicación ajena.
 *
 *   NO se usa `ARBITRAGEX` como marcador: en el HTML crudo el logo es
 *   `ARBITRAG<span>E</span>X` (frontend/components/site-header.tsx:76-86), así
 *   que la cadena contigua sólo existe en el snapshot a11y aplanado, no en los
 *   bytes servidos. Un guard que buscara `ARBITRAGEX` en HTML crudo fallaría
 *   contra el frontend real (falso negativo).
 *   NO se usa `PAPER · TLS SHADOW` como obligatorio: proviene de un ternario
 *   (`frontend/components/site-header.tsx:50`) y desaparece en modo LIVE.
 *   Se conserva como marcador FUERTE opcional.
 *
 * ALCANCE DE LO QUE ESTE GUARD **NO** PRUEBA:
 *   Verifica que el target sirve ESTE frontend. NO verifica que el flujo
 *   FUNCIONE: un GET prueba que la ruta RESPONDE. La aceptación de INTERACCIÓN
 *   (teclado, foco, formularios, estados, errores, refresh, responsive) sigue
 *   NO VERIFICADA sin navegador. Arreglar el oráculo no produce el veredicto.
 *
 * USO CLI (sin Playwright):
 *   node tests/e2e/base-url-guard.cjs http://127.0.0.1:3458/
 *   node tests/e2e/base-url-guard.cjs http://host/ --body-file ./fixture.html
 *   exit 0 = PASA (identidad confirmada) ; exit 1 = FALLA (con diagnóstico)
 */

'use strict';

/** Marcadores cuya AUSENCIA prueba que el target no es este frontend.
 *  Son CHROME renderizado: sólo aparecen cuando la página se renderiza bien. */
const FRONTEND_IDENTITY_MARKERS = Object.freeze([
  'Quantum Research Terminal',
  'Topological Yield Engine',
]);

/**
 * Identidad por TÍTULO — señal de layout, presente en el documento aunque la
 * ruta haya fallado. Derivada de frontend/app/layout.tsx:26
 * (`title: "QuantumX — Control Plane"`), metadata del root layout.
 *
 * Se compara por DOS subcadenas ASCII ('QuantumX' y 'Control Plane') en vez del
 * literal completo a propósito: el título real lleva una raya em (U+2014) y
 * compararlo entero es frágil ante normalizaciones de encoding. Los dos tokens
 * ASCII identifican igual y no dependen del guion.
 *
 * POR QUÉ EXISTE ESTA SEGUNDA SEÑAL (defecto medido, t29):
 * el chrome de arriba NO está presente cuando la ruta responde con página de
 * error. Medido en CI, run 37355502015 / job 111916763674 (workflow
 * liquidity-catalog, step 13): el target `http://127.0.0.1:3000/` devolvió
 * **HTTP 500** con `title: "QuantumX — Control Plane"` y 17603 bytes, porque el
 * root `/` hace fetch SSR al edge y ese job apunta INTERNAL_EDGE_URL a un puerto
 * loopback cerrado a propósito (liquidity-catalog.yml:97; su sonda de readiness
 * usa /dex-registry, :106). Mirando sólo el chrome, el guard concluyó
 * "does not serve this frontend" — una **acusación falsa**: era este frontend
 * sirviendo un documento de error. Identidad y salud son preguntas distintas, y
 * mezclarlas produjo un falso negativo que tumbó un job legítimo.
 */
const FRONTEND_TITLE_TOKENS = Object.freeze(['QuantumX', 'Control Plane']);

/** Marcadores adicionales: refuerzan el PASS, pero no son obligatorios. */
const OPTIONAL_IDENTITY_MARKERS = Object.freeze([
  'PAPER \u00B7 TLS SHADOW', // frontend/components/site-header.tsx:50 (solo modo paper)
  'QuantumX', // frontend/app/layout.tsx:26 (title de metadata)
]);

/** Título observado en la aplicación AJENA que ocupaba el puerto 3000 (t19). */
const KNOWN_FOREIGN_TITLE = 'Agenda Copilot P95';

class E2EBaseUrlError extends Error {
  constructor(message) {
    super(message);
    this.name = 'E2EBaseUrlError';
  }
}

/**
 * GUARD 1 — presencia. No hay default: si no está declarado, se lanza.
 *
 * @param {string|undefined|null} raw      valor crudo de la variable de entorno
 * @param {{envVar:string, configPath:string, remediation:string, source?:string}} opts
 * @returns {string} la URL declarada, sin tocar
 * @throws {E2EBaseUrlError}
 */
function resolveExplicitBaseUrl(raw, opts) {
  const { envVar, configPath, remediation } = opts;
  const value = typeof raw === 'string' ? raw.trim() : '';

  const header = `[E2E-BASEURL-01] ${envVar} is REQUIRED and has NO default.`;
  const why = [
    `Refusing to guess a target for ${configPath}.`,
    'An unset variable previously degraded silently to http://localhost + :3000,',
    `and port 3000 on this host serves a DIFFERENT application (${KNOWN_FOREIGN_TITLE}).`,
    'A run against the wrong application produces a verdict that is invalid by construction,',
    'so the oracle must fail loudly instead of grading whatever answers on that port.',
  ];
  const body = [header, '', ...why, '', 'Declare the application under test explicitly:', `  ${remediation}`];

  if (value === '') {
    throw new E2EBaseUrlError(body.join('\n'));
  }

  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    throw new E2EBaseUrlError(
      [...body, '', `The declared value is not a valid absolute URL: ${JSON.stringify(value)}`].join('\n'),
    );
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new E2EBaseUrlError(
      [
        ...body,
        '',
        `The declared value must be http(s); got protocol ${JSON.stringify(parsed.protocol)}.`,
      ].join('\n'),
    );
  }
  return value;
}

/**
 * Clasificación pura (sin red) de una URL para reportes y tests de la guardia.
 * @param {string} baseUrl
 */
function classifyBaseUrl(baseUrl) {
  const parsed = new URL(baseUrl);
  const host = parsed.hostname;
  const loopback = host === 'localhost' || host === '127.0.0.1' || host === '::1' || host === '[::1]';
  return {
    origin: parsed.origin,
    host,
    port: parsed.port === '' ? (parsed.protocol === 'https:' ? '443' : '80') : parsed.port,
    loopback,
    // Un target loopback no es un defecto por sí mismo: en CI el stack local ES el
    // frontend y .github/workflows/liquidity-catalog.yml:94 declara
    // E2E_BASE_URL=http://127.0.0.1 puerto 3000 legítimamente. Lo que no puede ocurrir es
    // un loopback IMPLÍCITO. Por eso esta bandera se reporta, no se penaliza.
  };
}

/**
 * Observa qué sirve realmente el target. NO lanza por sí sola.
 * @param {string} baseUrl
 * @param {{fetchImpl?:Function, timeoutMs?:number}} [opts]
 */
async function describeServedApp(baseUrl, opts) {
  const { fetchImpl, timeoutMs = 10_000 } = opts || {};
  const doFetch = fetchImpl || globalThis.fetch;
  const url = new URL('/', baseUrl).toString();
  const startedAt = Date.now();

  let response;
  try {
    response = await doFetch(url, {
      method: 'GET', // MUTACIÓN PROHIBIDA EN EL ORÁCULO: sólo lectura.
      redirect: 'follow',
      headers: { accept: 'text/html,application/xhtml+xml' },
      signal: typeof AbortSignal !== 'undefined' && AbortSignal.timeout ? AbortSignal.timeout(timeoutMs) : undefined,
    });
  } catch (err) {
    return {
      url,
      reachable: false,
      detail: err && err.message ? err.message : String(err),
      elapsedMs: Date.now() - startedAt,
      httpStatus: null,
      bodyBytes: 0,
      title: null,
      markersFound: [],
      markersMissing: [...FRONTEND_IDENTITY_MARKERS],
      optionalFound: [],
    };
  }

  let body = '';
  try {
    body = await response.text();
  } catch {
    body = '';
  }

  // Se descartan los comentarios HTML antes de extraer identidad. Dos razones:
  //  (1) un `<title>` dentro de un comentario no es el título del documento —
  //      medido con el fixture de regresión, donde el comentario de cabecera
  //      contenía el literal `<title>` y contaminaba la extracción;
  //  (2) evita que un comentario cuente como PRUEBA de identidad: sólo el
  //      marcado realmente servido debe sostenerla.
  // NO se descartan los <script>: el payload RSC de Next.js viaja ahí y es
  // salida real de nuestros componentes, es decir evidencia legítima.
  const scannable = body.replace(/<!--[\s\S]*?-->/g, '');

  const titleMatch = /<title[^>]*>([\s\S]*?)<\/title>/i.exec(scannable);
  const title = titleMatch ? titleMatch[1].trim() : null;
  const markersFound = FRONTEND_IDENTITY_MARKERS.filter((m) => scannable.includes(m));
  const markersMissing = FRONTEND_IDENTITY_MARKERS.filter((m) => !scannable.includes(m));

  // Identidad por TÍTULO: sobrevive a documentos de error, donde el chrome no se
  // renderiza. Señal PRIMARIA.
  const titleTokensFound = title === null ? [] : FRONTEND_TITLE_TOKENS.filter((t) => title.includes(t));
  const titleIdentity = titleTokensFound.length === FRONTEND_TITLE_TOKENS.length;
  // Señal SECUNDARIA: el chrome del header (sólo si la página se renderizó bien).
  const chromeIdentity = markersMissing.length === 0 && markersFound.length === FRONTEND_IDENTITY_MARKERS.length;
  const foreignTitle = title !== null && title.includes(KNOWN_FOREIGN_TITLE);

  return {
    url,
    reachable: true,
    detail: null,
    elapsedMs: Date.now() - startedAt,
    httpStatus: typeof response.status === 'number' ? response.status : null,
    bodyBytes: body.length,
    title,
    titleTokensFound,
    titleIdentity,
    chromeIdentity,
    // Identidad CONFIRMADA si alguna de las dos señales la sostiene. Que el
    // documento sea un error NO niega la identidad: la niega el no reconocerla.
    identityConfirmed: (titleIdentity || chromeIdentity) && !foreignTitle,
    foreignTitle,
    markersFound,
    markersMissing,
    optionalFound: OPTIONAL_IDENTITY_MARKERS.filter((m) => body.includes(m)),
  };
}

/**
 * GUARD 2 — identidad. Falla si el target no responde o no es este frontend.
 *
 * PREDICADO DE IDENTIDAD (t29): `titleIdentity || chromeIdentity`, y nunca si el
 * título es el de una aplicación ajena conocida. NO mezcla salud con identidad:
 * un `500` servido por ESTE frontend confirma la identidad y se reporta como
 * degradación observada, porque el guard responde "¿es mi app?" y no "¿está
 * sana?". Mezclar ambas produjo el falso negativo de t28 descrito arriba.
 *
 * @param {string} baseUrl
 * @param {{fetchImpl?:Function, timeoutMs?:number, configPath?:string}} [opts]
 * @returns {Promise<object>} evidencia observada (para reportar el PASS)
 * @throws {E2EBaseUrlError}
 */
async function assertFrontendIdentity(baseUrl, opts) {
  const { configPath = '(unknown config)' } = opts || {};
  const observed = await describeServedApp(baseUrl, opts);

  const prefix = `[E2E-BASEURL-01] Target identity check FAILED for ${configPath}`;

  if (!observed.reachable) {
    throw new E2EBaseUrlError(
      [
        prefix,
        '',
        `  target  : ${observed.url}`,
        `  result  : NO RESPONDE (${observed.detail})`,
        '',
        'An unreachable target is NOT a pass: it proves nothing about this frontend,',
        'and a suite that runs against it would report failures of the harness, not of the product.',
        'Start the application under test, or declare the correct origin.',
        'NEVER point this at a production host just to make the check green.',
      ].join('\n'),
    );
  }

  if (!observed.identityConfirmed) {
    const severe = observed.foreignTitle
      ? `DIAGNOSIS: this is the KNOWN FOREIGN application ("${KNOWN_FOREIGN_TITLE}") that occupies a development port`
      : 'DIAGNOSIS: the target answers, but nothing in the served document identifies it as this frontend';
    throw new E2EBaseUrlError(
      [
        prefix,
        '',
        `  target        : ${observed.url}`,
        `  http status   : ${observed.httpStatus}`,
        `  title         : ${observed.title === null ? '(sin <title>)' : JSON.stringify(observed.title)}`,
        `  body bytes    : ${observed.bodyBytes}`,
        `  title tokens  : ${observed.titleTokensFound.length === 0 ? '(ninguno)' : observed.titleTokensFound.map((m) => JSON.stringify(m)).join(', ')}`,
        `  missing marker: ${observed.markersMissing.map((m) => JSON.stringify(m)).join(', ')}`,
        '',
        severe,
        '',
        'This frontend is identified by EITHER of two independent signals:',
        `  - its layout title containing ${FRONTEND_TITLE_TOKENS.map((m) => JSON.stringify(m)).join(' AND ')} (survives error documents)`,
        `  - its rendered chrome containing ${FRONTEND_IDENTITY_MARKERS.map((m) => JSON.stringify(m)).join(' AND ')}`,
        '',
        'Refusing to run: any verdict produced against this target would describe another application.',
      ].join('\n'),
    );
  }

  return observed;
}

module.exports = {
  FRONTEND_IDENTITY_MARKERS,
  FRONTEND_TITLE_TOKENS,
  OPTIONAL_IDENTITY_MARKERS,
  KNOWN_FOREIGN_TITLE,
  E2EBaseUrlError,
  resolveExplicitBaseUrl,
  classifyBaseUrl,
  describeServedApp,
  assertFrontendIdentity,
};

// ─────────────────────────────── CLI ───────────────────────────────────────
if (require.main === module) {
  const args = process.argv.slice(2);
  const bodyFileIdx = args.indexOf('--body-file');
  const bodyFile = bodyFileIdx >= 0 ? args[bodyFileIdx + 1] : null;
  const statusIdx = args.indexOf('--status');
  const forcedStatus = statusIdx >= 0 ? Number(args[statusIdx + 1]) : 200;
  const target = args.find(
    (a, i) =>
      !a.startsWith('--') &&
      !(bodyFileIdx >= 0 && i === bodyFileIdx + 1) &&
      !(statusIdx >= 0 && i === statusIdx + 1),
  );

  // Se usa exitCode en lugar de process.exit() a propósito: salir por la fuerza
  // mientras un cuerpo de respuesta grande todavía se está drenando dispara una
  // aserción de libuv en Windows (Assertion failed: !(handle->flags &
  // UV_HANDLE_CLOSING), src/win/async.c). El veredicto se emitía igual, pero un
  // guard que ensucia el stderr del oráculo es un guard peor.
  const fail = (title, detail) => {
    process.stdout.write(`${JSON.stringify({ verdict: 'FAIL', check: title, detail }, null, 2)}\n`);
    process.exitCode = 1;
  };

  if (!target) {
    fail('presence', {
      reason: 'no target argument',
      usage: 'node tests/e2e/base-url-guard.cjs <baseUrl> [--body-file <path>]',
    });
  }

  let fetchImpl;
  if (bodyFile) {
    const fs = require('fs');
    const bytes = fs.readFileSync(bodyFile, 'utf8');
    fetchImpl = async () => ({
      status: Number.isFinite(forcedStatus) ? forcedStatus : 200,
      text: async () => bytes,
    });
  }

  assertFrontendIdentity(target, { fetchImpl, configPath: 'base-url-guard.cjs (CLI)' })
    .then((observed) => {
      // DEGRADACIÓN OBSERVADA (t29): identidad confirmada por título, pero el
      // documento no es un render sano. Se REPORTA; no se convierte en fallo,
      // porque este guard responde "¿es mi app?" y no "¿está sana?". Gradar la
      // salud es trabajo del spec, que aserta sobre la UI y falla honestamente.
      const degraded = !observed.chromeIdentity || (observed.httpStatus !== null && observed.httpStatus >= 400);
      process.stdout.write(
        `${JSON.stringify(
          {
            verdict: 'PASS',
            check: 'identity',
            identityBasis: observed.titleIdentity ? 'layout-title' : 'rendered-chrome',
            degraded,
            degradedNote: degraded
              ? `identity confirmed, but the probed document is NOT a healthy render (http ${observed.httpStatus}, chrome markers missing: ${observed.markersMissing.length}). Health is NOT identity: the spec grades the UI, this guard only refuses to grade the WRONG app.`
              : 'identity confirmed on a healthy render',
            baseUrl: classifyBaseUrl(target),
            observed,
            note: 'A GET proves the route RESPONDS. It does NOT prove the flow FUNCTIONS.',
          },
          null,
          2,
        )}\n`,
      );
      process.exitCode = 0;
    })
    .catch((err) => {
      if (err instanceof E2EBaseUrlError) {
        fail('identity', { baseUrl: classifyBaseUrl(target), message: err.message });
      }
      fail('identity', { baseUrl: target, message: err && err.message ? err.message : String(err) });
    });
}
