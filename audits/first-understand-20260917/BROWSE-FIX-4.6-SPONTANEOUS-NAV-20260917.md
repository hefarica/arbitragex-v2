# BROWSE-FIX-4.6 — Navegación espontánea /opportunities → `/` (1×, ~8 s) — fixer gang ronda 1 · 2026-09-17

> Charter: gap 4.6 del BROWSE del Operador de mesa
> (`BROWSE-Operador-de-mesa-…DApp-VPS-read-only….md:141-144`): "tras la primera
> carga de /opportunities y una espera de 8 s, el router client-side navegó solo
> a `/` sin interacción ni error de consola (1×). No volvió a ocurrir en el
> segundo intento. Registro como observación sin hipótesis confirmada (INFERRED:
> posible race de prefetch RSC)".
> Misión del fixer: confirmar/refutar la hipótesis, cerrar el gap con evidencia,
> y dejar el fenómeno observable para una recurrencia. Cero git/cargo/build/VPS/
> HTTP-al-dominio (0/5 presupuesto).

---

## 1. Resultado ejecutivo

**EXCULPACIÓN COMPLETA DEL CÓDIGO DE APLICACIÓN (CANONICAL_REPO).** Barrido
exhaustivo del árbol frontend: NO existe ninguna ruta de código que navegue a
`/`. El mecanismo queda acotado (no confirmado) a
{recuperación interna del App Router de Next 14.2.35, redirect HTTP upstream
sobre un document fetch duro, capa browser/automatización} — los tres
clasificados **UNKNOWN** hasta nueva observación CON atribución. La hipótesis
del observador ("race de prefetch RSC") queda **HYPOTHESIS** — ni confirmada ni
refutada: la investigación pública no muestra un bug confirmado con esta firma
exacta, solo clases relacionadas (ver §4).

**Fix aplicado:** `NavigationSentinel` (null-render, montado en el root layout,
marcado `// BROWSE-FIX-4.6 (2026-09-17)`) que atribuye CADA navegación
same-document con (a) mecanismo (`navigation-api:<type>` en Chromium;
`popstate` como fallback) y (b) ms desde el último input real de usuario
(pointerdown/keydown). Una recurrencia imprime una línea que separa de inmediato
"push soft del framework" vs "hard redirect" vs "hubo input". Sin este
instrumento, el fenómeno sigue siendo una anécdota no falsable.

## 2. Evidencia de exculpación (reproducible, RULE 00)

Barrido sobre `frontend/` (excl. `node_modules`, `app_backup`, `components_backup`,
`playwright-report` — backups/HTML no cargados en runtime):

| Vector | Método | Resultado |
|---|---|---|
| `router.push/replace` | grep `router\.(push\|replace)\(` | Solo `/admin/signin…`, `/admin/topology` (click-gated, `CredentialsClient.tsx:744-845`, `TopologyVaultClient.tsx:227`, `admin/signin/page.tsx:46`). **Cero a `/`** |
| `redirect()` server | grep `redirect\(` | 3 hits, rutas ajenas: `operator/page.tsx:10` (→/operator/self-test), `omega-s5/registry/[entity]/page.tsx:43` (+ backup) |
| `window.location` / `history.*State` | grep | `AdminSessionBadge.tsx:55` `reload()` SOLO en onClick logout (montado solo en /admin/chains y /omega-s5); **cero asignaciones href, cero pushState/replaceState** |
| middleware / meta-refresh / SW | glob + grep `middleware\|http-equiv\|serviceWorker` | `middleware.ts` NO existe; `next.config.js` tiene SOLO rewrites (no `redirects()`); cero service worker; cero meta refresh |
| Hotkeys globales / popstate handlers | grep `addEventListener("(keydown\|popstate\|…)` | 1 hit: `PairDetailDrawer.tsx:206-210` — Escape cierra un Sheet, solo en /strategies |
| Router en superficie /opportunities | lectura `page.tsx` + grep en `OpportunitiesClient.tsx` | page = Server Component puro (fetch snapshot); client NO usa `useRouter` (grep 0). `loading.tsx` = skeletons |
| Layout raíz | lectura completa | `layout.tsx`: ninguno de los 11 montajes globales (SiteHeader/AppSidebar/Banner/PostureBar/Ticker/RealtimeProvider…) navega; el único `<Link href="/">` es el logo del header (`site-header.tsx:76`) — requiere click |
| Guards de onboarding | grep | Ninguno en layout/header/sidebar |
| Root page | lectura `app/page.tsx` | Dashboard normal; no redirige |

Precisión metodológica: greps con `-r --include=*.ts,*.tsx` sobre
`app components lib features hooks`; el patrón de location cubría
`location\.(href|assign|replace)`. Comandos reproducibles en el transcript del
fixer (bash grep, sesión 2026-09-17).

## 3. Acotación del mecanismo (lo que QUEDA, clasificado)

1. **Recuperación interna del App Router (Next 14.2.35, `react 18.3.1`)** —
   p.ej. recuperación ante desync del árbol RSC/Router Cache. INFERRED-plausible
   por la ventana temporal: al primer pinto el badge decía `socket CONNECTING ·
   "Edge connection error · Loading…"` y resolvió en <10 s
   (BROWSE §1, :25-27); la navegación ocurrió a los ~8 s, DENTRO de esa ventana,
   y solo en la PRIMERA carga. Correlación temporal NO implica causa (fall-honest).
2. **Redirect HTTP upstream sobre un document fetch duro** (nginx/tunnel) —
   requeriría un fetch de documento full 8 s después de cargar sin acción de
   usuario; en el repo nada dispara tal fetch (`reload()` solo en click de
   logout admin). INFERRED débil.
3. **Capa browser/automatización** (la sesión BROWSE era un journey
   instrumentado). UNKNOWN, no verificable desde el repo.

La hipótesis charter "race de prefetch RSC" (INFERRED del observador) queda
**HYPOTHESIS**: el prefetch del `<Link href="/">` del logo NUNCA despacha
navegación por sí mismo en el contrato de Next, y no hallé bug confirmado con
esta firma exacta (§4). No la declaro confirmada — el estándar de evidencia de
la mesa (§34.5.3) prohíbe sentenciar sin artefacto.

## 4. Contraste con conocimiento público (1 búsqueda, no toca el dominio)

Clases RELACIONADAS pero sin síntoma idéntico confirmado:
- [vercel/next.js discussion #57565 — Link sometimes stops working with app router in production](https://github.com/vercel/next.js/discussions/57565)
- [Why Your Next.js Links Freeze on Stale Tabs](https://dev.to/ameer-pk/why-your-nextjs-links-freeze-on-stale-tabs-and-how-to-fix-it-33ji)
- [Next.js 14 client Router Cache (30s) — Stack Overflow](https://stackoverflow.com/questions/79165854/nextjs-14-client-route-caching-for-30-seconds-and-unable-to-override)
- [Vercel — Common mistakes with the App Router](https://vercel.com/blog/common-mistakes-with-the-next-js-app-router-and-how-to-fix-them)
Ninguna documenta "navega solo a / sin input"; son fallas de NO-navegar o
estados stale. Esto REFUTA la tentativa de citar un bug upstream confirmado —
el mecanismo queda abierto.

## 5. Fix aplicado (local, working tree, NO-GIT)

- **Nuevo** `frontend/components/NavigationSentinel.tsx` (~120 LOC):
  - `formatNavSentinelLine(entry)` — línea de atribución pura (testeable).
  - `getNavigationApi(w)` — probe defensivo del Navigation API (Chromium).
  - `NavigationSentinel()` — null-render; TODO el acceso a `window` vive en
    `useEffect` (R1 Mounted Snapshot, cero riesgo de hydration). Listeners
    capture-phase `pointerdown`/`keydown` marcan el último input real; el
    evento `navigate` (Navigation API) y `popstate` loguean
    `mechanism → destination (ms since last user input)` vía `console.info`.
    Volumen: 1 línea por navegación (user-rate, no hot-loop — disciplina R9).
    RULE 00: reporta SOLO lo observado; API ausente = se reporta por su
    ausencia (fallback popstate), jamás se fabrica causa.
- **Edit** `frontend/app/layout.tsx` — montaje junto a `ArbxRealtimeProvider`
  (+ import, ambos con marcador `// BROWSE-FIX-4.6 (2026-09-17)`).
- **Nuevo** `frontend/components/__tests__/NavigationSentinel.test.tsx` —
  patrón repo (node env + `renderToStaticMarkup` + funciones puras).

**Limitación declarada (fail-honest):** atribución completa de navegaciones
SOFT requiere el Navigation API (Chromium ≥105). En navegadores sin él, el
sentinel solo ve back/forward (popstate) — un push soft del framework sería
invisible ahí. La anomalía fue observada en un browser Chromium (journey
instrumentado), exactamente donde el instrumento cubre.

## 6. Verificación

- `npx vitest run components/__tests__/NavigationSentinel.test.tsx` → **6/6 PASS**
  (SSR null-render "" = cero acceso a window en render; formatter: input
  registrado / null honesto / unknown verbatim; probe: ausente→null,
  parcial→null, válido→objeto).
- `npx tsc --noEmit` (árbol completo) → **0 errores en superficie propia**.
  3 errores pre-existentes/ajenos, NO tocados: `components/OpportunityTicker.tsx(88)`
  + `lib/schemas.test.ts(109,130)` — fallout del diff EN VUELO de un par
  (`WO-G2-PARITY (2026-09-17)` en `lib/schemas.ts`+`schemas.test.ts`, hardening
  block_number — verificado `git status --porcelain`: M en esos 2 archivos,
  ajenos a este fix). Documentado, no pisado.
- `npx eslint` sobre los 3 archivos tocados → **exit 0**.
- Verificación EN VIVO (la línea real ante una recurrencia): queda post-deploy,
  operator-gated (NO-GIT; RULE 03 rebuild frontend). NO reproducida aquí — el
  evento es 1× y no fabrico reproducciones (RULE 00).

## 7. Sincronía de mesa redonda

- **Construye sobre** BROWSE Operador de mesa §4.6 (anomalía original) y §1
  (ventana "Edge connection error" <10 s que enmarca los 8 s).
- **No contradice** al QA-WS: su FEED-SCHEMA-01 (block_number string, retry
  30 s) es un timer DISTINTO (30 s ≠ 8 s) y un síntoma REST, sin mecanismo de
  navegación — sin vínculo causal establecido. Nota: el diff en vuelo
  `WO-G2-PARITY` de otro par parece apuntar justo a ese FEED-SCHEMA-01.
- **Relevante para WO-04 (fichas TS, no iniciado)**: `frontend/app/layout.tsx`
  no estaba reclamado por ningún WO al momento de este fix (verificado contra
  board + git status); si WO-04 ficha el layout, debe encontrar ya montado el
  sentinel y citar este reporte.
- Protocolo de recurrencia para cualquier par de browse posterior: si el URL
  salta solo, buscar en consola `[arbx] nav-sentinel:` —
  `navigation-api:push` con "no user input recorded" = framework/soft;
  `navigation-api:replace/traverse` = recuperación de estado;
  sin línea + URL cambiado = hard redirect upstream (nginx/tunnel) → leer
  headers del document fetch en el network log. Con eso la anomalía pasa de
  "observación sin hipótesis" a fenómeno clasificado.

## 8. Cuentas de disciplina

Cero git/commit/push/PR/deploy. Cero cargo/npm-build (solo vitest+tsc+eslint de
verificación, permitidos). Cero VPS. Cero HTTP al dominio (0/5); 1 websearch a
fuente pública ajena al dominio (documentada §4). Marcadores
`// BROWSE-FIX-4.6 (2026-09-17)` en los 3 archivos tocados.

Clasificación de afirmaciones: exculpación = CANONICAL_REPO (greps
reproducibles); ventana temporal 8s⊆<10s = OBSERVED (BROWSE §1) + correlación
INFERRED; mecanismo residual = UNKNOWN (acotado); bug upstream confirmado =
NO EXISTE (refutado hallarlo en §4).
