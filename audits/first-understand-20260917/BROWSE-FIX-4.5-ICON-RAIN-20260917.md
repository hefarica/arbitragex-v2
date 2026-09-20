# BROWSE-FIX-4.5 — ICON-RAIN-20260917 (fixer gang ronda 1)

> Gap asignado: BROWSE §4.5 (`BROWSE-Operador-de-mesa-PhD-en-observabilidad-de-pipeline-fe-del-PIPELINE-en-vivo-v-a-DApp-VPS-read-only-no-del-dominio-p-blico.md:137-140`)
> — "Lluvia de GET /api/v1/token-icon duplicados (misma dirección 0x6982… PEPE
> ~30×, 0xd7ef… ~20×, con ráfaga de ERR_ABORTED): el feed re-pide iconos por
> render en vez de cachear por URL. Coste de ancho de banda del propio edge."
> Clasificación del descubridor: INFERRED del network log — este fix la
> CONFIRMA contra el código (CANONICAL_REPO, file:line abajo).

## 1. Diagnóstico de causa raíz (re-derivado, RULE 00)

El frontend YA tenía una cascada de 4 niveles con caché en memoria
(`frontend/lib/hooks/useTokenIcon.ts` original):
nivel 1 known-map (`lib/known-tokens.ts:62-95`) → nivel 2 caché módulo-scope
(`known-tokens.ts:126-145`) → nivel 3 `GET /api/v1/token-icon/:chain/:addr`
→ nivel 4 jazzicon. Además el api-server YA emitía `Cache-Control`
(`backend/api-server/src/routes/token-icon.ts:203` — `public, max-age=240`
en hits, `max-age=60` en misses jazzicon :303, NEG_TTL_SECS :56). La lluvia
NO era falta de caché servidor: eran DOS defectos de cliente en el effect
original (`useTokenIcon.ts` pre-fix, effect :104-160):

1. **Sin single-flight.** Cada instancia del hook montada para un mismo
   token disparaba su PROPIO `fetch` (effect :118-124). El feed renderiza
   ~752 tarjetas (BROWSE §1) y un long-tail token aparece en ~30 → 30
   requests idénticos CONCURRENTES por render. El HTTP cache no deduplica
   requests concurrentes en vuelo, y el nivel 2 solo se puebla DESPUÉS de
   que el primero resuelve.
2. **Abort-on-unmount + sin negative-cache de fallos.** El cleanup del
   effect abortaba el `AbortController` al desmontar (:155-159). En un feed
   que re-renderiza con stream vivo, tarjetas viven menos que el round-trip
   → ráfaga `ERR_ABORTED` (exactamente la observada), el request abortado
   NUNCA completa → `setCachedIcon` (:145) no ejecuta → el nivel 2 queda
   vacío para ese token → el próximo montaje vuelve a disparar el request.
   Lluvia autosostenida. Los fallos (HTTP ≠ 200, timeout) tampoco dejaban
   rastro caché (catch :147-152 solo setea estado local).

Clasificación: **defecto de Decoherencia de Estado del cliente** — la
resolución no era estable entre montajes. Sin impacto de datos (RULE 00
intacta: el fallback jazzicon determinístico siempre renderizó); el coste
era ancho de banda del edge + ruido de consola.

## 2. Fix aplicado (frontend-only, marcado `// ICON-RAIN-20260917`)

Archivo: `frontend/lib/hooks/useTokenIcon.ts` (+151/−41, `git diff --stat`
verificado). Tres cambios, todos en el nivel de red del hook:

1. **Single-flight módulo-scope** — `fetchIconResolution(chainId, addr)`
   (nuevo, :144-202): `Map<key, Promise>` de in-flight; N montajes
   concurrentes del mismo token comparten UN request. Key = `iconCacheKey`
   de known-tokens (`chainId:address`) — per-token, sin colisión cruzada
   (test 4 lo pinea).
2. **El fetch compartido SOBREVIVE unmounts** — se eliminó el
   `AbortController` por instancia (el cleanup del effect solo voltea
   `alive` :267-269). Un request iniciado por una tarjeta que se fue del
   viewport igualmente completa y puebla el nivel 2, así el próximo montaje
   resuelve sincrónico en el `useState` initializer (R1 intacto: tiers 1+2
   siguen síncronos). El único abort restante es el timeout de 5 s
   (invariante preexistente). `ERR_ABORTED` por desmonte: eliminado por
   construcción.
3. **Negative-cache de fallos con TTL** — `recentIconFetchFailure`
   (:127-138) + `FAILURE_TTL_MS = 60_000` (:67): un fallo real (network /
   timeout / HTTP ≠ 200) queda recordado 60 s; los remontajes en esa ventana
   degradan al avatar determinístico SIN re-golpear el edge, y el string de
   error REAL se sigue surfacing (fail-honest, no se fabrica icono — RULE
   00/R8). 60 s espeja el `NEG_TTL_SECS=60` del api-server (:56) — ambas
   capas acuerdan cuánto vive un fallo.

Sin cambios en: TokenChip.tsx, TokenIcon.tsx, known-tokens.ts,
api-server, edge. La semántica del payload (parseo `ApiIconResponse`,
normalización de `source`, jazzicon en `iconUrl:null`) es byte-idéntica al
original — solo se movió DÓNDE vive el fetch y QUÉ pasa al fallar.

## 3. Verificación (ejecutada, no afirmada)

- **Test nuevo** `frontend/lib/hooks/useTokenIcon.test.ts` (5 tests, node
  env, `vi.stubGlobal("fetch")` con contador — reproduce la lluvia de 30
  montajes concurrentes):
  1. 30 resoluciones concurrentes del mismo token → **fetch llamado 1 vez**,
     URL exacta `/api/v1/token-icon/1/<addr>` (el defecto producía 30).
  2. éxito → `getCachedIcon` poblado (próximo montaje = tier 2, 0 fetch).
  3. tokens distintos → flights independientes (2 fetches — el dedupe no
     colapsa entre tokens).
  4. fallo 503 → negative-cache con mensaje real "HTTP 503"; 1 solo fetch.
  5. respuesta `iconUrl:null` (jazzicon) → cachéada como resolución real,
     NO como fallo.
- **Suite completa**: `npx vitest run` → **126 archivos / 1265 tests PASS**
  (incluye los 21 preexistentes del dominio icon: known-tokens 10,
  TokenIcon 6, TokenChip 5).
- **Typecheck**: `npx tsc --noEmit` → **exit 0, 0 errores** (frontend
  completo).
- Comandos reproducibles desde `frontend/`: `npx vitest run
  lib/hooks/useTokenIcon.test.ts` · `npx tsc --noEmit`.

## 4. Sincronía de mesa redonda

- **Confirma** el INFERRED §4.5 del BROWSE (Operador de mesa): la lluvia era
  cliente, el servidor ya cachaba bien. NO requiere cambios de
  edge/api-server.
- **Sin colisión de files**: mis archivos son exclusivamente
  `frontend/lib/hooks/useTokenIcon.ts` + el test nuevo. En el working tree
  hay OTROS archivos modificados que NO toqué (otros pares de la mesa,
  gaps 4.1/4.2 del mismo BROWSE): `frontend/app/operations/components/ArchivePanel.tsx`,
  `RouteDiscoveryFunnelCard.tsx`, `features/route-discovery/RouteDiscoveryOutcomesPanel.tsx`,
  `app/operations/components/__tests__/ArchivePanel.test.tsx` (nuevo). Sin
  pisarse.
- **Anomalía transitoria documentada (fail-honest)**: a mitad de la sesión
  una lectura de `useTokenIcon.ts` devolvió el contenido PRE-fix (race de
  escritura en disco — varios agentes comparten el clone; un `tsc` intermedio
  reportó mis exports como faltantes). Re-verificado con lectura completa
  post-race + tests + tsc: el fix está íntegro en disco. Quien verifique
  después: si ve síntomas similares, releer antes de concluir.
- **Verificación en vivo pendiente (operator-gated)**: el efecto solo es
  observable en la DApp tras deploy (prohibido aquí — NO-GIT). Comando para
  el verificador post-deploy: cargar `/opportunities`, network log →
  `GET /api/v1/token-icon` debe aparecer 1× por token desconocido por
  sesión de pestaña (o 0× si falló hace <60 s), sin ráfagas ERR_ABORTED.

## 5. Presupuesto y reglas

- Cero git (commit/push/PR), cero deploy, cero VPS, cero HTTP manual
  (0/5 requests — solo tests locales con fetch stubeado), cero cargo/npm
  build (vitest + tsc son verificación permitida por el charter del fixer).
- Cero mocks en producto (RULE 00): el único stub de fetch vive en el TEST
  (estándar del repo — cf. enfoque de los tests existentes).
- §32/§33/§34.3 intactos: cambio de presentación/observabilidad FE, sin
  executor/capital/terminus.
