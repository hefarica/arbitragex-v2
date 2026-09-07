# WO-09 — DESIGN: endurecimiento CSP en 2 fases (P0-6)

- **Work-order:** WO-09 (GOAL-WORKORDERS.md:17 · roadmap P0-6 `00-PREDATOR-ROADMAP.md:104-108` · remedio informe `01-frontend-web.md:168-173`)
- **Kind:** apply (este documento = diseño completo; la **fase 1 mecánica** se aplica gated-OFF en `frontend/next.config.js`; el **flip real es operador**, §4)
- **Owner:** security-auditor (gang omniscience) · 2026-09-06
- **Archivos claim:** `frontend/next.config.js` + este documento + `WO-09-APPLY.md`
- **Reglas activas:** RULE 00 (zero mocks — todo dato citado es medido) · NO-GIT (cero commit/push/deploy) · §32/§33 read-only

---

## 1. Evidencia D-1 — re-verificada (código + live)

### 1.1 En código (build `9ac06d2d`, re-leído hoy)

| file:line | hecho |
|---|---|
| `frontend/next.config.js:149` | ÚNICA cabecera CSP emitida: `{ key: "content-security-policy-report-only", value: csp() }` — report-only |
| `frontend/next.config.js:43` | `script-src 'self' 'unsafe-inline' 'unsafe-eval'` — nace débil |
| `frontend/next.config.js:44` | `style-src 'self' 'unsafe-inline'` |
| `frontend/next.config.js:18-19` | Comentario propio vencido: *"Switch to Content-Security-Policy once the report stream is clean ≥7 days"* — pendiente desde N5 (2026-06-13) |
| `frontend/next.config.js:28-64` | `csp()` — política sin directiva `report-uri`/`report-to` (sin collector) |
| `frontend/next.config.js:153-158` | Precedente de gate build-arg: `ARBX_TLS_ENABLED === "true"` (SEC-3) |
| `frontend/Dockerfile:12-17` | **Semántica clave**: *"next.config.js headers() runs during `next build` … a compose `environment:` entry never reaches the bake"* — los gates de `headers()` se evalúan en BUILD, no en runtime del contenedor |
| `frontend/next.config.guard.test.ts:52-76` | Guards CSP (CSP-IMG-1) pinean la existencia de la cabecera report-only |

### 1.2 En el dominio público (verificación live, 1 request del presupuesto de 2)

`curl -sI https://arbx.ape-tv.net/` — 2026-09-07T01:48Z (edge CF-RAY `a3720d2279fb488a-MIA`):

```
content-security-policy-report-only: default-src 'self'; script-src 'self'
  'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self'
  data: blob: https://raw.githubusercontent.com https://assets.coingecko.com
  https://coin-images.coingecko.com https://cdn.dexscreener.com; font-src 'self'
  data:; connect-src 'self' ws: wss: https://api.web3modal.org
  https://pulse.walletconnect.org; frame-ancestors 'none'; base-uri 'self';
  form-action 'self'; object-src 'none'
strict-transport-security: max-age=31536000; includeSubDomains      ← TLS gate ON en prod
Report-To: {"group":"cf-nel","max_age":604800,"endpoints":[{"url":"https://a.nel.cloudflare.com/report/v4?..."}]}
Nel: {"report_to":"cf-nel","success_fraction":0.0,"max_age":604800}
Cache-Control: private, no-cache, no-store, max-age=0, must-revalidate   ← HTML YA es dynamic
```

Confirmaciones de esta sola respuesta:
1. **D-1 vigente**: la única CSP del dominio sigue siendo report-only (protección XSS por CSP = 0). Coincide con `01-frontend-web.md:94-116` (validación end-to-end código→origen→tunnel→dominio).
2. **NEL ACTIVO en la zona**: Cloudflare YA inyecta `Report-To: cf-nel` + `Nel` (success_fraction 0) — la colecta NEL existe hoy para esta zona; ver §2.4 para dónde se observa EXACTO.
3. **HSTS presente** → el patrón build-arg (SEC-3) está probado en prod: precedente directo para el gate de la fase 1.
4. **El HTML raíz ya es dynamic** (`no-store`) — requisito de la fase 2 ya satisfecho en la página raíz (ver §3.3).

### 1.3 Fuera de la ruta pública (observación, no claim)

`frontend/nginx.conf:23-26` emite su PROPIA CSP **enforcing** estática (con `unsafe-inline`, sin allowlist de CDNs ni web3modal). Hoy está FUERA de la ruta pública (el dominio demostró servir solo la report-only de Next; cf. memoria: ruta pública = CF tunnel→5173). **Advertencia de flip:** si ese nginx entrara alguna vez en la ruta pública, DOS CSP enforcing se intersectan y la más restrictiva rompe los logos/conexiones. Condición previa del flip: la ruta pública se mantiene tunnel→5173.

---

## 2. FASE 1 — CSP enforcing duplicada idéntica, gated (48-72 h)

### 2.1 Principio

Duplicar la política report-only **byte a byte** como cabecera `content-security-policy` ENFORCING, manteniendo la report-only durante la ventana. Al ser idénticas, todo lo que la stream report-only ha venido señalando es EXACTAMENTE lo que la enforcing bloqueará; la ventana dual existe para poder abortar con un rebuild. **El flip NO se aplica a ciegas** (§4): default OFF.

### 2.2 Diff aplicado en `frontend/next.config.js` (este WO — mecánica, gated OFF)

```js
      { key: "content-security-policy-report-only", value: csp() },
      // WO-09 (2026-09-06): P0-6 fase 1 — CSP ENFORCING idéntica, gated OFF.
      // Duplica la política report-only byte a byte como header enforcing cuando
      // ARBX_CSP_ENFORCE === "true" (contrato exact-string, fail-closed). Mismo
      // patrón build-arg que SEC-3/ARBX_TLS_ENABLED: headers() se evalúa durante
      // `next build` (Dockerfile:12-17), NO en runtime del contenedor — el flip
      // exige rebuild (RULE 03). NUNCA habilitar a ciegas: ventana de deploy
      // quieta (P0-5) + smoke e2e post-deploy, AMBOS operador. Abort/rollback y
      // monitoreo NEL: audits/omniscience-integration-2026-09-06/WO-09-DESIGN.md §2.
      ...(process.env.ARBX_CSP_ENFORCE === "true"
        ? [{ key: "content-security-policy", value: csp() }]
        : []),
```

Propiedades del gate:
- **Contrato**: `ARBX_CSP_ENFORCE === "true"` exact-string. Cualquier otro valor (unset, `True`, `1`, `false`) = OFF. Fail-closed por diseño (misma lección que `ARBX_LIVE_EXEC_ENABLED` del roadmap item 12: contrato documentado, sin coerción).
- **Byte-identical garantizado estructuralmente**: ambas cabeceras llaman a la misma `csp()` — no hay segunda copia de la política que pueda derivar.
- **Guard tests intactos**: `next.config.guard.test.ts` busca la cabecera report-only (líneas 56, 69) que permanece incondicional.

### 2.3 Wiring del flip (diffs propuestos — NO aplicados: archivos fuera de claim; los ejecuta el PR del operador)

`frontend/Dockerfile` (builder, junto a `ARG ARBX_TLS_ENABLED`, línea 17):

```dockerfile
# WO-09 (2026-09-06): P0-6 fase 1 — build arg del gate CSP enforcing (SEC-3 class:
# headers() runs during `next build`; a compose `environment:` entry never reaches it).
# Default false — flip ONLY in a quiet deploy window (P0-5) with post-deploy smoke e2e.
ARG ARBX_CSP_ENFORCE=false
ENV ARBX_CSP_ENFORCE=$ARBX_CSP_ENFORCE
```

`docker/compose.prod.yml` (servicio `frontend`, `build.args`, espejo de `ARBX_TLS_ENABLED`):

```yaml
      # WO-09 (2026-09-06): P0-6 fase 1 — CSP enforcing flip (default false; operator-only).
      ARBX_CSP_ENFORCE: ${ARBX_CSP_ENFORCE:-false}
```

`.env` del VPS (operador, al momento del flip): `ARBX_CSP_ENFORCE=true`.

### 2.4 Monitoreo de la ventana — ¿dónde se observa el feed NEL? EXACTO

**Respuesta corta:** dashboard Cloudflare de la zona `ape-tv.net` → sección **Network Error Logging → View Reports**; el volumen crudo viaja al endpoint de colecta `https://a.nel.cloudflare.com/report/v4` (verificado inyectado en la zona, §1.2) y en planes Enterprise puede exportarse con **Logpush** como dataset zone-scoped **`nel_reports`** (API `.../logpush/datasets/nel_reports/fields`). Documentación: developers.cloudflare.com/network-error-logging/ (overview / get-started / how-to "View Reports").

**Matiz que el roadmap omite (corrección honesta, RULE 00):** NEL colecta **errores de red** (fases dns/connection/application; "why a request failed, country, last-mile, responsible party") — **NO violaciones CSP**. Una petición bloqueada por CSP no llega a la capa de red, así que NEL no la reporta. Las violaciones CSP viven en la Reporting API (`report type csp-violation`) y SOLO se colectan si la política declara `report-to`/`report-uri` apuntando a un collector — **ausente en `csp()` hoy (next.config.js:41-63)**; Cloudflare ofrece el dataset `csp_reports` (Logpush, Enterprise) como canal equivalente. Consecuencia práctica: durante la ventana, la señal de "reportes CSP" no vendrá de NEL; los canales REALES disponibles son:

| Canal | Qué atrapa | Disponibilidad hoy |
|---|---|---|
| **(1) Report-only retenido** | Toda violación de la política idéntica se sigue logueando a consola del navegador | Activo (la cabecera se conserva en la ventana) |
| **(2) Smoke e2e post-deploy (Playwright)** | `page.on('console')` + evento `securitypolicyviolation` en journeys sobre `E2E_BASE_URL=https://arbx.ape-tv.net` (`e2e/readiness-smoke.spec.ts`, `e2e/opportunities-honest-display.spec.ts`; config `playwright.config.ts:11`) | Requiere ejecución (operador) |
| **(3) Browser-verifiers del gang** | Consola de navegadores reales durante journeys 48-72 h (doble reportería: enforcing bloquea + report-only lo anuncia en consola) | Activo por metodología §9 omniscience |
| **(4) NEL Cloudflare** | Ruptura de RED de la ventana misma (tunnel/origen caído tras el redeploy): `View Reports` + `nel_reports` | Activo en la zona (verificado §1.2) |
| **(5) `csp_reports` Logpush** | Violaciones CSP crudas | **NO disponible** (Enterprise-only + sin `report-to`; opcional futuro) |

### 2.5 Criterio de abort (X) y rollback exacto

Con la política duplicada idéntica, cualquier violación bajo enforcing es una violación que la report-only YA estaba señalando — el sistema lleva semanas con la stream limpia en lo operativo (CSP-IMG-1 cerró el último flood conocido el 09-01, `next.config.js:49-53`). Umbrales:

- **ABORT inmediato** (canal 2): CUALQUIER `securitypolicyviolation` en smoke e2e sobre una ruta crítica (`/`, `/opportunities`, `/live-readiness`, `/operations`) → rollback sin esperar tasa.
- **ABORT por tasa** (canales 3/5 cuando existan): >5 reportes de violación/min sostenidos ≥10 min, o cualquier reporte que afecte un script de carga de página.
- **ABORT por regresión de red** (canal 4): errores fase application en NEL > 2× el baseline pre-ventana durante ≥10 min (el redeploy mismo es el sospechoso, no la CSP).

**Rollback (= revert del header enforcing, ~un ciclo de redeploy):**

```bash
# VPS — ARBX_CSP_ENFORCE=false (o quitar la línea) en /opt/arbitragex-v2/.env, luego:
docker compose --env-file .env -f docker/compose.prod.yml build --no-cache frontend
docker compose --env-file .env -f docker/compose.prod.yml up -d frontend
curl -sI https://arbx.ape-tv.net/ | grep -i content-security   # debe quedar SOLO report-only
```

La report-only nunca se retiró → el estado pre-flip es íntegro. NOTA: rollback = redeploy (~30 min de ciclo, P0-5); por eso el flip exige ventana quieta.

### 2.6 Runbook del flip (OPERADOR — ninguna parte es ejecutable por agentes)

1. **Precondiciones:** P0-5 activo (ventana quieta, sin merges en cola) · sin auditoría HG en curso (precedente #532) · PR de wiring §2.3 mergeado y verificado en CI.
2. `ARBX_CSP_ENFORCE=true` en `.env` del VPS (contrato exact-string §2.2).
3. Rebuild frontend RULE 03 (`--env-file .env`, `build --no-cache frontend`, `up -d frontend`).
4. **Verificación del flip:** `curl -sI https://arbx.ape-tv.net/` debe mostrar AMBAS cabeceras — `content-security-policy` y `content-security-policy-report-only` — con valores byte-idénticos.
5. Smoke e2e post-deploy (canal 2) contra el dominio público → criterio §2.5.
6. Ventana de observación 48-72 h con canales 3+4; registro diario en este directorio.
7. Cierre: ventana limpia → fase 1b (retirar report-only, §3.4) y abrir fase 2. Cualquier abort → rollback §2.5 + reporte de incidente con ID.

---

## 3. FASE 2 — migración a NONCE (eliminar `unsafe-inline`/`unsafe-eval` de script-src)

### 3.1 Auditoría "qué ROMPE hoy" — resultados medidos (RULE 00)

Superficie auditada (`app/ components/ lib/ features/ hooks/ store/ scripts/ e2e/`, package `next@14.2.35` — `frontend/package.json:26`):

| Sonda | Resultado |
|---|---|
| `dangerouslySetInnerHTML` + `<script` literal en templates | **0 ocurrencias** — no hay NI UN script inline de primera parte |
| `eval(` / `new Function` / `javascript:` | **0 ocurrencias** en primera parte |
| `style={{...}}` (atributos style de elemento) | **41 ocurrencias** → `style-src 'unsafe-inline'` DEBE conservarse en fase 2 (los atributos style requieren `style-src-attr 'unsafe-inline'` o `'unsafe-hashes'` por-propiedad; costo/beneficio desfavorable, el vector XSS por style es marginal vs script) |
| Consumidor real de `unsafe-inline` en script-src | **El propio Next**: scripts bootstrap inline del App Router (`self.__next_f.push(...)` en el stream RSC) — framework, no primera parte. Se resuelve con nonce auto-aplicado |
| Consumidor real de `unsafe-eval` | **Solo dev** (React Refresh reconstruye stack traces con eval). Docs oficiales: *"unsafe-eval is not required for production. Neither React nor Next.js use eval in production by default"* |
| Páginas NO dinámicas (el nonce exige dynamic rendering) | **1 de 57**: `app/deploy-pipeline/page.tsx:56` (`export const dynamic = "force-static"`). Las demás: 31 con `revalidate = 0`/force-dynamic explícito, resto dynamic de facto (fetch por request; evidencia live §1.2 `Cache-Control: no-store`) |
| Terceros en script-src | Ninguno hoy (no hay hosts en script-src) → sin allowlists que migrar. Riesgo latente: SDK wallet (Reown/AppKit) puede inyectar iframes/scripts dinámicos → cubrir con `'strict-dynamic'` + verificación de journey wallet |

**Conclusión del audit:** la app está inusualmente LIMPIA para nonce — el único bloqueo duro conocido es `deploy-pipeline` (force-static) y la retención deliberada de `style-src 'unsafe-inline'`.

### 3.2 Patrón de referencia (docs oficiales Next.js, traducido a 14.2.35)

Patrón documentado en nextjs.org/docs/app/guides/content-security-policy: middleware genera nonce por request → lo setea en header de REQUEST (`x-nonce` + `Content-Security-Policy`) → `NextResponse.next({ request: { headers } })` → Next **extrae el nonce del header CSP del request y lo aplica automáticamente** a scripts del framework, bundles, inline scripts/styles y `<Script nonce>`; el CSP se setea también en la RESPONSE. Notas de versión: la doc actual muestra `proxy.ts` (naming Next 15.5+/16) — **en 14.2.35 el convenio es `middleware.ts` con `export function middleware()`**; el mecanismo del nonce es el mismo desde 13.4.20 ("proper nonce handling and CSP header parsing").

Cuando un nonce/hash está presente en `script-src`, los navegadores **ignoran `'unsafe-inline'`** en esa directiva (CSP2+, MDN) — por eso el token puede eliminarse de una vez.

### 3.3 Diff propuesto — `frontend/middleware.ts` (NUEVO archivo; NO aplicado en este WO — fase 2)

```ts
// WO-09 (2026-09-06): P0-6 fase 2 — nonce CSP per-request (App Router, Next 14.2).
// Patrón: docs nextjs.org/app/guides/content-security-policy (aquí middleware.ts,
// no proxy.ts — naming de 15.5+). Next aplica el nonce automáticamente a sus
// scripts (framework, bundles, inline bootstrap) al verlo en el header CSP del
// REQUEST. Requiere dynamic rendering en TODAS las páginas (excepción conocida:
// app/deploy-pipeline/page.tsx:56 force-static — convertir a dynamic o eximir).
import { NextRequest, NextResponse } from "next/server";

export function middleware(request: NextRequest) {
  const nonce = Buffer.from(crypto.randomUUID()).toString("base64");
  const isDev = process.env.NODE_ENV === "development";
  // WO-09: misma directiva que csp() de next.config.js, MENOS unsafe-inline y
  // unsafe-eval en script-src (nonce + strict-dynamic los reemplazan; eval solo dev).
  const cspHeaderValue = [
    "default-src 'self'",
    // 'unsafe-inline' es IGNORADO por el navegador cuando hay nonce — se omite.
    // 'strict-dynamic' permite scripts inyectados por un script con nonce (SDK wallet).
    `script-src 'self' 'nonce-${nonce}' 'strict-dynamic'${isDev ? " 'unsafe-eval'" : ""}`,
    // 41 atributos style={{...}} en primera parte → conservar (§3.1).
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: blob: https://raw.githubusercontent.com https://assets.coingecko.com https://coin-images.coingecko.com https://cdn.dexscreener.com",
    "font-src 'self' data:",
    "connect-src 'self' ws: wss: https://api.web3modal.org https://pulse.walletconnect.org",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'",
    "object-src 'none'",
    // NOTA (heredada de next.config.js:61-62): sin upgrade-insecure-requests.
  ].join("; ");

  const requestHeaders = new Headers(request.headers);
  requestHeaders.set("x-nonce", nonce);
  requestHeaders.set("Content-Security-Policy", cspHeaderValue);

  const response = NextResponse.next({ request: { headers: requestHeaders } });
  response.headers.set("Content-Security-Policy", cspHeaderValue);
  return response;
}

// WO-09: matcher docs — excluye estáticos (nonce por-request no debe cachearse
// sobre assets) y prefetches.
export const config = {
  matcher: [
    {
      source: "/((?!api|_next/static|_next/image|favicon.ico).*)",
      missing: [
        { type: "header", key: "next-router-prefetch" },
        { type: "header", key: "purpose", value: "prefetch" },
      ],
    },
  ],
};
```

### 3.4 Diff propuesto — `frontend/next.config.js` fase 2a/2b (NO aplicado)

**2a (transición):** el middleware emite la enforcing con nonce; `headers()` conserva SOLO la report-only (el gate §2.2 se retira). La report-only vieja (con unsafe-inline) no señaliza nada útil contra la nueva enforcing → reemplazar su valor por la MISMA política nonce con `report-only`… imposible (el nonce vive por-request en middleware) → la report-only se retira ya en 2a y el monitoreo pasa a los canales 2/3/4 de §2.4:

```js
      // WO-09 (2026-09-06) fase 2a: CSP emigra a middleware.ts (nonce per-request,
      // runtime — ya NO bake-time). Retirar AMBAS cabeceras CSP estáticas:
      // ...(líneas content-security-policy-report-only y el bloque ARBX_CSP_ENFORCE, eliminadas)
```

**2b (limpieza):** eliminar el bloque `csp()` (next.config.js:28-64) y el comentario CSP-Report-Only (líneas 17-27); actualizar `next.config.guard.test.ts:52-76` — los guards CSP-IMG-1 deben re-dirigirse a la política del middleware (importándola o extrayendo el builder a `lib/csp.ts` compartido). **Archivo NO claimado por WO-09** → va en el PR de fase 2 con su propio ID (P-∅: un PR = un ID).

### 3.5 Qué gana la fase 2

- `script-src` sin `unsafe-inline` ni `unsafe-eval` en prod: una inyección de script arbitrario (el vector que hoy la report-only solo observa) queda **estructuralmente bloqueada** incluso si un template introduce inline.
- La CSP deja de ser build-time-baked (middleware = runtime) → futuros ajustes de política sin rebuild.

### 3.6 Alternativa documentada (no elegida)

Docs Next: **SRI experimental** (`experimental.sri.algorithm`) — hashes de integridad en build, preserva static generation. Descartada como camino principal: experimental (14.0), no cubre scripts dinámicos (el caso de AppKit), y la app ya es dynamic de facto. Útil solo si `deploy-pipeline` debe permanecer force-static.

---

## 4. WARNING EXPLÍCITO — el flip NO es automático

> **Aplicar la CSP enforcing a ciegas rompe prod.** El gate landeado en §2.2 deja el build IDÉNTICO al actual mientras `ARBX_CSP_ENFORCE` no sea exactamente `"true"` en el build. El flip real exige, SIN EXCEPCIÓN:
> 1. **Ventana de deploy quieta** — dependencia P0-5 (coalescing; hoy 3 merges/hora = flota en recreate perpetuo, y el rollback §2.5 es otro redeploy de ~30 min).
> 2. **Smoke e2e post-deploy** con listener `securitypolicyviolation` contra el dominio público (§2.4 canal 2) — operador.
> 3. **Ambos pasos son del OPERADOR** (§34-style: no inferidos de flags ni de chat; los agentes NO deployamos — protocolo NO-GIT 2026-08-23).
> Condición adicional (§1.3): la ruta pública debe seguir siendo CF tunnel→5173; si nginx entra a la ruta, SU CSP enforcing estática colisiona (intersección de políticas) y el diagnóstico del abort será falso.

---

## 5. Fuentes

- Código: `frontend/next.config.js:17-27,28-64,143-160` · `frontend/Dockerfile:12-17,17,43` · `frontend/next.config.guard.test.ts:52-76` · `frontend/nginx.conf:23-26` · `frontend/package.json:26` · `frontend/playwright.config.ts:11`
- Informe/roadmap: `audits/omniscience-integration-2026-09-06/01-frontend-web.md:40-51,94-116,150-156,168-173` · `00-PREDATOR-ROADMAP.md:104-108` · `GOAL-WORKORDERS.md:17`
- Live: `curl -sI https://arbx.ape-tv.net/` 2026-09-07T01:48Z (1/2 del presupuesto dominio de este WO)
- Docs externas: [Cloudflare NEL overview](https://developers.cloudflare.com/network-error-logging/) · [NEL View Reports](https://developers.cloudflare.com/network-error-logging/how-to/) · [Logpush zone datasets (nel_reports/csp_reports)](https://developers.cloudflare.com/logs/logpush/logpush-job/datasets/zone/) · [Next.js CSP guide](https://nextjs.org/docs/app/guides/content-security-policy) · MDN CSP (nonce ignora unsafe-inline)
