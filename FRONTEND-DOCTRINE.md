# FRONTEND-DOCTRINE — EL PLAN (por todos, para todos)

> **WO:** FE-04 · Gang Omniscience · 2026-09-07 · agente compositor: ecc:react-reviewer
> **Fuente de síntesis:** `audits/frontend-doctrine-2026-09-07/{FE-01,FE-02a,FE-02b,FE-03}-DESIGN.md` + verificación
> propia en árbol. **Este documento RIGE los PRs frontend futuros de la mesa redonda** (contrato de adopción §7).
> **Lexicon OMEGA:** TLS (flash loan) · Holonomic Loop Resolution (triangular) · Topological Yield (ganancia neta) ·
> Decoherencia de Estado (slippage) · Variedad de Liquidez (pool/DEX).
>
> **Estado del árbol al componer (verificado por mí):** branch `feat/hops-live-01` @ `27aca289`; dirty:
> `frontend/lib/drift/useDriftDetection.ts`*, `frontend/app/control/` + `ControlBoardLed.tsx` +
> `ControlBoard.test.tsx` (programa CB-03 en vuelo, untracked — NO tocarlos sin coordinar). El árbol compartido
> ya rotó una vez esta sesión (`a6-cbprom-01` → `feat/hops-live-01`): todo diff de este plan se aplica tras
> `git branch --show-current` + re-anchoring de líneas (§36 disciplina de branches).

---

## 1. Resumen ejecutivo para el operador

**La DApp tiene 58 páginas** (18 client + 28 patrón R1-snapshot + 12 server-con-islas; FE-01-DESIGN.md §0 —
el "57" del goal es ambigüedad contable, drift histórico 56→58 documentado), servidas por **6 topologías de
transporte** (REST edge, WS socket.io, WS raw ×2, SSE, sin red), con **0 API routes propias**. El tronco
económico (`/`, `/opportunities`, `/opportunities/exchange`, `/opportunities/by-strategy`) es **un solo wire**
(`/api/opportunities/live`) con 4 consumidores.

**Lo que está sano y se congela como canon** (no se re-audita — FE-02b-DESIGN.md §3): disciplina de hidratación
R1 extendida, 28/28 intervals con cleanup, a11y estructural (skip-link, Radix Sheet, `role="alert"` en 19
archivos, DataTable `role="grid"`), overflow curado donde el histórico mordió (`alert.tsx minmax(0,1fr)` pineado),
ticker 30s + `React.memo` con comparador por campo (precedente 3ac27560), flush WS 1 Hz + pruneStale (MEM-RENDER-01).

**Lo que está roto y duele YA** (evidencia PG viva 2026-09-07, FE-02a-DESIGN.md §2): la percepción BR-11 —
*"cuando aparecen valores USD, los hops>2 desaparecen"* — es TRES defectos frontend componiendo:
1. `routeKeyOf` colapsa todo ciclo cerrado a UNA React key (8,288 filas/30 min compartían la key idéntica → React
   dropea cards hermanas) — BUG-01 CRITICAL;
2. la ventana cliente `limit=50` bajo ráfaga cotizada (flood XEN/AGLD, 183/s histórico) desplaza el multihop del
   único frame que lee — BUG-02 (letal mientras D-11 mantenga el dominio público en POLLING);
3. el push WS entrega la fila PG cruda y BORRA la economía enriquecida que el snapshot REST sí trajo (BUG-03),
   mientras el comparador de memo omite el bloque `simulated_*` y congela el dinero que quedó (BUG-04).
Además el ticker global **fabrica** Topological Yield % (`profit*0.1`, "Rough scaling") en el header de TODAS las
páginas (BUG-06 = FE-02b-01, hallazgo convergente de dos pares independientes) — violación RULE 00 en la superficie
más visible del producto.

**La meta** (goal del operador): *streaming actualizado vía snapshot y SOLO se actualiza el número que cambió*.
FE-03 ya especificó el patrón — **SDQ: Snapshot SSR veraz + Delta Quirúrgico por Campo** — normalizando el store
en `order/entities/metrics` con hojas `<DeltaValue>` que subscriben UN primitivo. Este plan lo adopta como
arquitectura objetivo (§5) y añade lo que FE-03 no tenía: la **identidad de RUTA** de FE-02a (una card por ruta,
no por detección) como clave del store normalizado, la **secuencia de PRs** con gate Y rollback (§6), y las
**mejoras P0/P1/P2 con diffs listos** (§3).

**BR-11 (aceptación del operador, §8):** todo el dinero por hop + financiamiento TLS veraz
(`OWN_CAPITAL|AAVE_FL|BALANCER_FL|V2_FLASH_SWAP` — canon `docs/ROUTES_CROWN_JEWEL_DOCTRINE.md:42-52`,
`backend/searcher-rs/src/financing.rs:70-77`) + waterfall + entrega final. El frontend YA puede renderizar el
waterfall cuando el ledger existe (`deriveLegLedger`); lo que falta es (a) el interim honesto del financiamiento,
(b) que el backend persista/sirva `financing_mode` y el ledger por hop del kernel triangular (dependencias
declaradas §8.3 — NO son frontend), (c) que el dinero que llega deje de borrarse/congelarse (P0).

**CB-03 (/control):** el Control Board es R1-verificado (CB-VERIFY PASS 3/3) pero **huérfano total** — cero links
entrantes en todo el árbol (verificado por grep esta sesión; FE-01-DESIGN.md §4.1). P0-5 lo incorpora al sidebar.
Su LED de feed debe leer el estado honesto del pipeline (P1-7).

**Adjudicación de conflicto entre pares (la única contradicción real encontrada):** FE-03 §1.2#5 ordena
*eliminar* `lib/websocket-client.ts` ("DEAD, cero consumidores"); FE-02a §0 reporta que WO-01 (commit `325e3154`,
2026-09-06) lo realineó al contrato real del servidor (`row_to_json(NEW)`). **Verificado por mí en HEAD
`27aca289`: el archivo sigue siendo importado SÓLO por su test** — WO-01 corrigió el contrato del adapter pero NO
montó consumidor de producción. Resolución (§6 ola 1): la eliminación de FE-03 PROCEDE, condicionada a trasplantar
el comentario-documento del contrato `row_to_json(NEW)` a `socket-lifecycle.ts` (para no perder el conocimiento
que WO-01 grabó) y con gate de salida que aborta la ola si un consumidor de producción aterrizó mientras tanto
(en ese caso: aplicar FE-02a FIX-9 y rutear por la política del provider). Detalle en §6.2.

---

## 2. Mejoras pequeñas priorizadas (P0/P1/P2 — esfuerzo×impacto, diff listo-para-PR)

Convención: **Esfuerzo** S(<30 min)/M(≤1 día)/L(>1 día) · **Impacto** medido contra BR-11/CB-03/RULE 00/UX.
Todo diff lleva el marcador `// FE-04 (2026-09-07)` (adición del plan; el origen del hallazgo se cita).
Cada ítem = UN PR (P-∅ §37: un PR = un ID de anomalía). NINGUNO está aplicado — este documento es design.

| ID | Mejora | Origen | Esf×Imp | Ola |
|---|---|---|---|---|
| **P0-1** | Key por topología de ciclo + una card por ruta | FE-02a FIX-1 (BUG-01) | S×CRÍTICO | 0 |
| **P0-2** | Ticker fail-honest (sin % fabricado) | FE-02a FIX-7 = FE-02b D-01 (BUG-06/FE-02b-01) | S×CRÍTICO | 0 |
| **P0-3** | Comparador de memo cubre el dinero que renderiza | FE-02a FIX-4 (BUG-04) | S×ALTO | 0 |
| **P0-4** | El push crudo no borra la economía enriquecida | FE-02a FIX-3 (BUG-03) | M×ALTO | 0 |
| **P0-5** | `/control` al sidebar (CB-03 des-huérfana) | FE-01 §4.1 + FE-04 (nuevo) | S×ALTO | 0 |
| **P1-1** | Ventana 200 + truncamiento declarado | FE-02a FIX-2 (BUG-02+09) | S×ALTO | 0 |
| **P1-2** | Financiamiento veraz interim (BR-11) | FE-02a FIX-6a (GAP-11) | S×ALTO | 0 |
| **P1-3** | Waterfall por hop + entrega final (BR-11) | FE-02a FIX-6b/6c (GAP-12/13) | M×ALTO | 0 |
| **P1-4** | WalletDetailDialog: spinner eterno + `role="alert"` | FE-02b D-02/D-03 | S×MEDIO | 0 |
| **P1-5** | Overflow: /config KV + funnel 64px + CardHeader 1fr | FE-02b D-05/D-06/D-07 | S×MEDIO | 0 |
| **P1-6** | El X-Ray lee la fila viva del store | FE-02a FIX-5 (BUG-05) | M×MEDIO | 2 |
| **P1-7** | Badge del feed: CONNECTING real (CB-03) | FE-02a FIX-8 (BUG-08) | S×MEDIO | 2 |
| **P2-1** | `OpportunityRowSchema`: preservar route_metadata/leg_symbols | FE-02a FIX-10 (BUG-10) | S×MEDIO | 1 |
| **P2-2** | `suppressHydrationWarning` sólo en `<span>` (7 sitios) | FE-02b D-04a..e | M×LATENTE | 3 |
| **P2-3** | Icon-only button con `aria-label` + `aria-hidden` | FE-02b D-08 | S×BAJO | 3 |
| **P2-4** | `LastUpdated`: tick perezoso con visibilidad | FE-02b D-09 | S×BAJO | 3 |
| **P2-5** | `/translator` adopta tokens del design system | FE-01 §4.9 | S×BAJO | 5 |

### P0-1 — Key por topología de ciclo + una card por ruta (BUG-01, CRITICAL)

Diff canónico: **FE-02a-DESIGN.md §3 FIX-1** (verbatim, marcado `WO-FE-02a`; el PR lo aplica tal cual).
Esencia verificada por mí en `frontend/app/opportunities/OpportunitiesClient.tsx:40-50` (key actual
`chain|chain_id_out|strategy|token_in|token_out|dex_a|dex_b` — todo ciclo cerrado degenera porque
`token_in===token_out` y `dex_b` vacío en triangular) y `:436-449` (render `.map` con esa key):

```tsx
// WO-FE-02a (2026-09-07) — adoptado por FE-04 P0-1 (2026-09-07): BR-11 BUG-01.
function routeKeyOf(opp: OmniOpportunity): string {
  const rm = opp.route_metadata;
  if (rm && rm.dex_adapters.length > 0) {
    return [opp.chain_id, opp.strategy_kind ?? "", ...rm.dex_adapters, ...rm.pool_addresses].join(">");
  }
  return [opp.chain_id, opp.chain_id_out ?? "", opp.strategy_kind ?? "", opp.token_in,
          opp.token_out, opp.dex_a, opp.dex_b ?? ""].join("|");
}
function latestByRoute(opps: OmniOpportunity[]): OmniOpportunity[] { /* FE-02a §3, verbatim */ }
// render: const routeCards = useMemo(() => latestByRoute(opportunities), [opportunities]);
//         {routeCards.map((opp) => <OpportunityTradeCard key={routeKeyOf(opp)} … />)}
```

**Invariante (INV-A, biyección):** `new Set(routeCards.map(routeKeyOf)).size === routeCards.length`.
**Gate:** unit test — dos filas 3-hop con mismo `(chain,kind,token_in,token_out,dex_a,dex_b)` y distinto 3er
pool ⇒ 2 keys; dos detecciones de la MISMA ruta ⇒ 1 card con la más reciente.
**Nota de composición (FE-04):** `routeKeyOf` con topología pasa a ser la **identidad de ruta del store
normalizado de la ola 2** (§5.2) — este PR la introduce como función exportada para reuso.

### P0-2 — Ticker fail-honest (BUG-06 = FE-02b-01; convergencia de dos pares)

Dos pares independientes cazaron el mismo defecto con la misma cura (`roi_pct ?? null`) — FE-02a FIX-7 y
FE-02b D-01. El PR aplica UNA vez la unión: el diff completo con los 3 sitios de render (sr-only `:140`,
marquee `:149-153`, flecha direccional oculta si `y==null`) está en **FE-02b-DESIGN.md §FE-02b-01** — es el más
completo (cubre render); el estado vacío con razón verdadera ("N detecciones sin precio aún") está en
FE-02a §3 FIX-7. Regla que queda (INV-FE02B-3): prohibido derivar/escalar un valor monetario en el cliente.

### P0-3 — Comparador de memo cubre el dinero (BUG-04)

Diff canónico: **FE-02a-DESIGN.md §3 FIX-4** — añade al comparador (`OpportunityTradeCard.tsx:539-584`):
`risk_score`, `paper_status`, `rejection_reason`, `simulated_net_profit_usd`, `simulated_amount_in_usd`,
`sameJson(simulated_cost_breakdown)`, `sameJson(simulated_target)`. **Gate:** props idénticas salvo
`simulated_cost_breakdown` ⇒ `comparator === false`. Este ítem es prerrequisito de P0-4 (sin él, el merge
preservado no se PINTA).

### P0-4 — El push crudo no borra la economía enriquecida (BUG-03)

Diff canónico: **FE-02a-DESIGN.md §3 FIX-3** — `mergeRawPush` (helper puro en `lib/store/types.ts`) +
wiring en `useOmniOpportunities.ts` `onOpportunity`. **Invariante INV-B:** un push WS para id X nunca deja
en null un campo que el store tenía no-null, salvo que el propio wire lo afirme. Es CONTENCIÓN; el remedio
estructural (enriquecer el puente LISTEN `backend/api-server/src/index.ts:1847-1863` o persistir el bloque
simulated) es dependencia backend declarada (§8.3) — el PR frontend lo declara en su descripción.

### P0-5 — `/control` al sidebar (CB-03; diff NUEVO de FE-04)

`/control` (Control Board CB-03, R1-verificado PASS por CB-VERIFY) es **huérfana total**: `grep href="/control"`
en `frontend/` = 0 matches (verificado esta sesión; FE-01 §4.1). Una página de control de capital inaccesible
por navegación es un hallazgo de producto, no de estilo. El grupo `control` ya existe en
`frontend/components/nav-items.ts:70-77`:

```ts
// frontend/components/nav-items.ts — grupo RISK & CONTROL
  { href: "/risk",                 label: "Entropy & alerts",       icon: AlertTriangleIcon,      group: "control" },
  { href: "/killswitch",           label: "Kill-switch",            icon: PowerIcon,              group: "control" },
+ // FE-04 (2026-09-07): CB-03 — /control era huérfana total (grep href="/control" = 0 en app+components+features;
+ // // FE-01-DESIGN.md §4.1). El Control Board (LED por módulo real, /api/v1/control-board) entra al grupo
+ // control junto a kill-switch: quien opera el kill-switch debe ver el board en el mismo bloque visual.
+ { href: "/control",              label: "Control board",          icon: GaugeIcon,               group: "control" },
  { href: "/live-readiness",       label: "Live readiness",         icon: ListChecksIcon,         group: "control" },
```

`GaugeIcon` ya está importado (`nav-items.ts:6`). **Gate:** (a) render del sidebar contiene el link;
(b) `grep -c 'href: "/control"' frontend/components/nav-items.ts` = 1; (c) FE-01-GATE-1 sigue OK (el censo
no cambia — la página ya existía); (d) census-test del sidebar (si existe) actualizado.
**Rollback:** revert del hunk (1 línea + comentario). **Coordinación:** `app/control/` está UNTRACKED
(programa CB en vuelo) — el PR se abre DESPUÉS de que el programa CB aterrice sus archivos, o el nav apunta a
una ruta que aún no viaja en main. Orden explícito: **P0-5 espera al merge del programa CB-03.**

### P1-1 — Ventana 200 + truncamiento declarado (BUG-02+09)

Diff canónico: **FE-02a §3 FIX-2** — `limit=200` en 3 sitios (`app/opportunities/page.tsx:14`,
`OpportunitiesClient.tsx:213`, `useOmniOpportunities.ts:120`) + copy del empty con razón por estado
(LIVE/POLLING × viableOnly). Presupuesto declarado por FE-02a: payload worst-case ~0.5 MB cada 5 s en
POLLING — aceptable, tunnable por env. **Nota FE-04:** mientras D-11 (tunnel → :5173, remediación = operador
en dashboard CF) mantenga el transporte público en POLLING, este fix ES el frame del feed — prioridad máxima
dentro de P1.

### P1-2 — Financiamiento veraz interim (BR-11 / GAP-11)

Diff canónico: **FE-02a §3 FIX-6a** — el renglón de la escalera deja de afirmar "Flash loan in (TLS)"
incondicionalmente (`OpportunityTradeCard.tsx:408-412`): si el bloque simulado trae flash fee > 0 ⇒
"Financiamiento TLS (flash fee $X)"; si no ⇒ "Capital in · (modo financiamiento: no emitido en el wire)".
JAMÁS se afirma `OWN_CAPITAL` sin wire (RULE 00). Cuando el backend sirva `financing_mode` (§8.3): chip
VERBATIM con el token canónico `OWN_CAPITAL|AAVE_FL|BALANCER_FL|V2_FLASH_SWAP` (`financing.rs:70-77`),
sufijo físico "(TLS)" sólo para los modos flash — palabra del operador: sin traducción que esconda la
configuración. **Gate:** snapshot-test — fixture sin flashFee NO contiene el string "Flash loan in (TLS)".

### P1-3 — Waterfall por hop + entrega final + capital configurado (BR-11 / GAP-12+13)

Diff canónico: **FE-02a §3 FIX-6b/6c** — la escalera consume `deriveLegLedger` (`lib/store/types.ts:613`,
ya existe, hoy sólo en el tab Ledger del diálogo) con `weiToHuman` (hoist del dialog a util compartido);
renglón "Entrega final (Δ ciclo)" SOLO en el hop de cierre; `amount_in_wei` visible en forma humana en el
SummaryGrid cuando `route_metadata.decimals` conoce los decimals. **INV-D (invariante de aceptación):**
CERO conversión wei→USD derivada en el frontend — por hop se muestra el monto EXACTO humanizado; el USD
por hop exige ancla USD del wire por token, que NO existe (ítem backend §8.3). **Gate:** snapshot tests con
fixture triangular 3-hop con ledger + sin ledger (dos estados honestos).

### P1-4 · P1-5 — Wallets honesto + overflow (FE-02b)

Diffs canónicos: **FE-02b-DESIGN.md §FE-02b-02/03 (D-02/D-03)** — `Promise.allSettled` en
`WalletDetailDialog.tsx:72-88` (spinner eterno hoy: `.then` sin catch) + `role="alert"` en `EndpointNotice`
(patrón ya adoptado en 19 archivos). **§FE-02b-05/06/07 (D-05/D-06/D-07)** — `/config` KV
`grid-cols-[max-content_minmax(0,1fr)]` + `break-all`; `PipelineFunnelCard` track `64px→5rem`;
`ui/card.tsx:22` `1fr→minmax(0,1fr)` con pin de test espejo del de `alert.tsx` (lección #514-#518: cura por
raíz, no por síntoma).

### P1-6 · P1-7 — Dialog vivo + CONNECTING real (se aplican con la ola 2)

**P1-6** (FE-02a FIX-5): el diálogo renderiza la fila VIVA del store (`selectedId` + `useOmniStore.find`),
conservando la última vista si la fila salió por TTL (R8 as-of). Se aplica en ola 2 porque su forma final es
la suscripción del store normalizado (§5.2) — aplicarlo antes y re-migrar después es doble trabajo.
**P1-7** (FE-02a FIX-8): `setWsStatus("CONNECTING")` tras obtener wsUrl válido, antes de
`createOpportunitySocket` — el badge del feed deja de decir "IDLE" durante el handshake. Alimenta CB-03: el
Control Board audita el estado del feed; un LED cuya fuente jamás pasa por CONNECTING no puede representar
reconnects. WO-08 (badge `RuntimePostureBar`, commit `325e3154`) ya está honesto en SU fuente — verificado
PASA por FE-02a §2.4; P1-7 cura la OTRA fuente (slice `wsStatus`).

### P2 (higiene y latentes)

**P2-1** (FE-02a FIX-10): `OpportunityRowSchema` añade opcionales-permisivos `route_metadata`/`leg_symbols`/
`cartridge_id` (hoy strip silencioso en `getValidated`); NO relajar los required (son tripwire de deriva).
**P2-2** (FE-02b D-04a..e): migrar los 7 `suppressHydrationWarning` de contenedor a `<span>` individual —
gate grep INV-1 de FE-02b §5 (re-ejecutable; hoy devuelve exactamente las 7 líneas). **P2-3** (D-08):
`aria-label` + `aria-hidden` en botón icon-only "Remove this RPC". **P2-4** (D-09): tick de `LastUpdated`
perezoso con `document.visibilityState` (advisory — hoy 4 consumidores a nivel panel, sin leak).
**P2-5**: `/translator` `zinc-950` hardcoded → tokens del shell (FE-01 §4.9).

---

## 3. Estandarizaciones exactas — checklist validable en pull-request

Todo PR frontend es revisado contra ESTA lista. Cada ítem es mecánicamente verificable (grep/test/build) —
el reviewer NO necesita contexto adicional. ★ = bloqueante (el PR no mergea sin él).

### Hidratación y montaje (R1/R5)
- ★ **R1-forma:** página SSR = Server Component puro + `*Client.tsx` con `useState(initialSnapshot)`; todo
  no-determinismo (`Date.now`, `window`, `navigator`, `localStorage`, WS) SOLO en `useEffect`. (Canon: CLAUDE.md §3 R1.)
- ★ **suppressHydrationWarning** sólo en `<html>` del layout o `<span>` individual con comentario de
  justificación (patrón `app/omega-s5/drift/page.tsx:21-26`). 0 en `<p>/<div>/<dd>/<Button>`.
  Verificación: el grep INV-1 de FE-02b-DESIGN.md §5.1 debe devolver 0 líneas.
- Si el PR añade una página nueva: auditoría transitiva R5 de TODOS los componentes importados (page + layout).

### Contrato de datos y estados (Pilar C de FE-03)
- ★ Toda superficie de datos renderiza EXACTAMENTE una variante `{skeleton shape-matched, error-verbatim,
  empty-con-razón, data}` (`DataSurfaceState`, FE-03 §4.2). Prohibido: spinner genérico en superficie de datos,
  "No data" desnudo, empty sin `reason`.
- ★ **Error verbatim**: el string del servidor (`edge HTTP {status}: {body}`) se pinta en `<code>`, jamás
  traducido/maquillado. Precedente: test de /control que pina el string exacto.
- ★ **Filtro honesto (guard BR-11):** todo `filter`/`sort` que pueda ocultar filas declara
  `hiddenByFilter: {label, count}` visible (`data-testid="hidden-by-filter"`). El empty declara si la fuente
  tiene 0 filas o si el filtro las ocultó.
- **Fetch honesto:** ningún `loading` terminal — todo `Promise.all`/fetch con spinner cubre rejection
  (`allSettled`/`.catch`) y desemboca en error visible `role="alert"` (INV-FE02B-2).

### Reglas RULE 00 / R8 en el render
- ★ Prohibido derivar/escalar/aproximar un valor económico en el cliente (INV-FE02B-3). El fallback de un
  campo ausente es `null` → "—". `null` = no computado, `0` = exactamente cero (R8) — jamás se confunden.
- ★ Prohibido wei→USD derivado en el frontend (INV-D de P1-3): por hop se muestra el monto humanizado exacto
  con los `decimals` del wire; USD sólo si viene del servidor.
- **Financiamiento:** el modo se muestra VERBATIM (`OWN_CAPITAL|AAVE_FL|BALANCER_FL|V2_FLASH_SWAP`) cuando
  el wire lo trae; sin wire, el interim honesto de P1-2 — jamás afirmar TLS incondicionalmente.

### Estado y transporte (Pilar B de FE-03)
- ★ Datos de servidor compartidos por ≥2 superficies ⇒ slice del omni-store con `FetchStatus`
  (`idle|loading|ready|error`); page-local ⇒ patrón R1 con el mismo vocabulario. Prohibido fetch duplicado
  de un mismo endpoint en 2 superficies sin justificación en comentario.
- ★ `useOmniStore()` sin selector = prohibido (regla existente `omni-store.ts:11-14`; lint `no-restricted-syntax`).
- ★ UN `io()` por gateway: `socket-lifecycle.ts` (opportunities), `ArbxRealtimeProvider` (los canales
  `REALTIME_CHANNELS`), + allowlist comentada de ciclos page-local. Todo `io()` nuevo fuera del allowlist
  requiere amend del allowlist EN el mismo PR.
- Todo `setInterval`/`addEventListener`/`subscribe` vive en `useEffect` con cleanup `return` (INV-FE02B-5 —
  hoy 28/28; el PR no puede romper el 28/28).
- React 18.3.1 pinneado ⇒ prohibido `useOptimistic`/`use()`/Form Actions (gate grep FE-03-E). El estado
  optimista manual por-card (`SimEvidence`) es el patrón canónico 18.x.

### Render y overflow
- ★ Todo track `1fr` que aloja contenido inquebrable (mono/URL/hash) usa `minmax(0,1fr)` o hijo con
  `min-w-0`/`break-*` (INV-FE02B-4). Tracks fijos con `toLocaleString` dimensionados para ≥8 dígitos.
- ★ React keys: la key de una card/lista es la **identidad de RUTA** (`routeKeyOf`, P0-1), nunca un campo que
  colapse en ciclos (`token_in===token_out`). INV-A: cero keys duplicadas entre hermanos.
- `React.memo` con comparador: el comparador debe cubrir TODO campo que el componente renderiza (BUG-04 es
  el precedente de por qué). Si añades un campo renderizado, añádelo al comparador en el mismo PR.

### A11y mínimo
- Notificación de falla ⇒ `role="alert"`; estado de carga de superficie ⇒ `role="status"` + `aria-busy`.
- Botón icon-only ⇒ `aria-label` + icono `aria-hidden="true"`.
- Diálogo ⇒ Radix Sheet/Dialog con `SheetTitle`+`SheetDescription` (focus-trap nativo).

### Censo y presupuesto
- ★ Página/ruta nueva ⇒ fila nueva en el censo (FE-01-DESIGN.md §3) y `FE-01-GATE-1` sigue cuadrando
  (`58 = 18+28+12`; si el total cambia, el PR actualiza la cifra adjudicada Y el gate).
- ★ `perf-budgets.json` (ola 4): `next build` bajo techo por ruta (`fe03-budget-check.mjs`); regresión >20%
  bloquea merge.
- **Marcador:** todo hunco que aplique este plan lleva `// FE-04 (2026-09-07)` (o el del WO de origen si
  proviene de un diff peer) — el reviewer lo verifica para atribución P-∅.

---

## 4. Esquema delta-streaming — especificación A NIVEL COMPONENTE

Arquitectura objetivo: **SDQ** (FE-03 Pilar A) + **identidad de ruta** (FE-02a). Regla en una frase:
*la página nace de un snapshot SSR veraz; cada evento del servidor se reduce a un diff por campo en la costura
de ingesta; sólo el campo que cambió se escribe; el DOM se actualiza por unidad hoja (`<DeltaValue>`); la lista
re-renderiza únicamente cuando cambia el ORDEN (entradas/salidas), jamás por un número.*

### 4.1 La cadena de ingesta (de punta a punta)

```
servidor (mig 107: sólo filas que REALMENTE cambiaron, row_to_json(NEW))
  → socket-lifecycle.ts (createOpportunitySocket, room "opportunities", evento new_opportunity)
    [WS LIVE] ──┐
    [POLLING 5s / snapshot SSR] ──┤→ mapToOmniOpportunity (mapper permisivo, null=null)
  → ws-ingest-buffer (1 Hz, Map por id, última llegada gana)
  → applySnapshotAndDeltas(rows):  resuelve rows→rutas (routeKeyOf);  gana la detección más reciente
                                   por ruta (INV-A);  diffOmniOpportunity por campo;
                                   delta vacío = CERO escritura (no-op contado, R8)
                                   mergeRawPush (P0-4) preserva lo enriquecido-sólo del REST
  → store normalizado:  order[routeKey] · entities[routeKey] · metrics[routeKey]{v,net_usd,gross_usd,roi_pct,sim_in_usd,status}
  → hojas <DeltaValue> / <AgeLeaf> subscriben UN primitivo
```

La normalización de FE-03 §2.2 se re-keyed **por ruta** (composición FE-04): `order: routeKey[]`,
`entities: Record<routeKey, OmniOpportunity>` (la detección vigente de cada ruta),
`metrics: Record<routeKey, OppHotMetrics>`. Con esto INV-A vive DENTRO del store (imposible renderizar dos
cards de una ruta) y no como dedupe de render — el PR de ola 2 reemplaza el `latestByRoute` de P0-1 por esta
resolución en la costura (P0-1 sigue siendo el hotfix correcto ANTES de la ola).

### 4.2 Tabla por componente (suscripción · diff · unidad DOM · memo boundary)

| Componente | Suscripción | Diff que consume | Unidad DOM de update | Memo boundary |
|---|---|---|---|---|
| `OpportunitiesClient` (grid shell) | `useOmniStore(s => s.order)` + callbacks `useCallback` | ninguno (sólo orden) | el grid re-render SOLO si `order` cambió (entrada/salida/TTL) | shell fuera del hot path; header/contadores memoizados aparte |
| `OpportunityTradeCard` | `useOmniStore(s => s.entities[routeKey])` — contenido FRÍO | `cold` del delta (topología, símbolos, ledger, rejection_reason) | la card re-renderiza SOLO por campo frío (rara vez) | `React.memo` + comparador completo (P0-3) como defense-in-depth — queda casi ocioso |
| `DeltaValue` (hoja nueva, FE-03 §2.4) | `useOmniStore(s => s.metrics[routeKey]?.[field] ?? null)` — UN primitivo | `hot` del delta (sólo el campo que cambió) | `<span data-delta="{routeKey}:{field}">` — UNA hoja por campo cambiado | ninguna (ya es el átomo) |
| `AgeLeaf` (hoja nueva, FE-03 §2.5) | `useOmniStore(s => s.clock30s)` + prop `detected_at` | ninguno (reloj) | `<span data-age>` — re-renderiza la hoja al cambiar el bucket de edad | el escritor del reloj es UN interval del store (mata la cascada `now` de `OpportunitiesClient.tsx:283-286`) |
| `OpportunitiesExchangeClient` | ídem grid shell (misma señal, proyección distinta) | ídem | ídem + `PriceTicker` como hojas del slice prices | ídem; `viable_only=false` declarado (ya lo es) |
| `OpportunitiesByStrategyClient` | `useOpportunityList()` derivado (FE-03 §3.3) durante migración | agregado por strategy_kind — recomputa SOLO si order/entities cambió | contador por grupo = hoja | `useMemo` del agrupado sobre `[order, entities]` |
| `HomeStoreAggregation` | `useOpportunityList()` derivado | ídem | KPI = hoja | ídem |
| `OpportunityDetailDialog` | `useOmniStore(s => s.entities[routeKey])` + snapshot as-of si la fila salió por TTL (P1-6) | el mismo delta | el Sheet re-renderiza al cambiar SU fila | sin re-selección al click — la fila es viva |
| `OpportunityTicker` (layout, TODAS las páginas) | **ola 5:** lee top-N del store sembrado por el provider (hoy: 5ª vía de fetch propia 30s, `OpportunityTicker.tsx:71-89`) | hot fields del top-N | ítem del marquee | items `useMemo`; honestidad P0-2 ANTES de migrar |
| `ControlBoardLed` (/control, CB-03) | slice de feed-status (`wsStatus` honesto vía P1-7) + su snapshot `/api/v1/control-board` | estado del feed + estado de módulos | LED por módulo — unidad hoja por módulo | R1 ya verificado (CB-VERIFY 3/3); adopta `DataSurfaceState` en ola 3 |
| `LastUpdated` | interno (tick 1s) | ninguno | `<span>` relativo | P2-4 hace el tick perezoso por visibilidad |
| `RuntimePostureBar` | slices runtime (canal declarado) | estados de canal | chip por canal | WO-08 verificado PASA (FE-02a §2.4) — no tocar |

### 4.3 Invariantes del esquema (numeradas, testeables)

- **INV-DS-1** (= FE-03-A1, RULE 00): el delta JAMÁS inventa números — sólo propaga cambios reales del
  servidor; delta sin fuente = observación vacía (no-op contado en `flushNoOps`); ausencia = `null`.
- **INV-DS-2** (= FE-03-A2 + INV-A): un cambio de UN campo de UNA ruta produce exactamente UNA escritura de
  métrica y UN re-render de UNA hoja; la lista/página re-renderizan SOLO por cambio de `order`; cero rutas
  duplicadas en `order` (biyección).
- **INV-DS-3** (transport-agnóstico): WS push, snapshot POLLING y SSR semilla usan el MISMO
  `applySnapshotAndDeltas`; tras reconnect NO se asume delta — reconcilia el snapshot; jamás se estira un
  delta sobre un gap (R8). Esto es obligatorio mientras D-11 mantenga el dominio público en POLLING.
- **INV-DS-4** (= INV-B): el push crudo nunca borra lo enriquecido (P0-4) — y el comparador de memo siempre
  cubre todo campo renderizado (P0-3), o el diff silencioso se convierte en pintado silencioso.
- **GATE-DS** (de FE-03 §2.6, adoptado + extensión de ruta): unit tests (semilla 200 filas; 1 campo caliente
  cambiado en 1 ruta ⇒ `Object.is(order)` estable, 199 entidades con referencia intacta, `deltaWrites +1`;
  batch idéntico ⇒ no-op; dos detecciones de una misma ruta ⇒ UNA entidad, la más reciente); render-probe
  (hoja ×1, shell ×0); browser journey FE-05 (commits/s bajo presupuesto §perf, atributo `data-delta` auditable).

---

## 5. Contrato de aceptación BR-11 (dinero por hop + financiamiento + waterfall + entrega final)

### 5.1 Aceptación operativa (qué ve el operador en la card)

1. **Capital configurado visible:** `amount_in_wei` humanizado (con `route_metadata.decimals` del token base)
   en el SummaryGrid — no sólo en tooltip (GAP-13).
2. **Modo de financiamiento VERBATIM:** chip con el token canónico `OWN_CAPITAL|AAVE_FL|BALANCER_FL|V2_FLASH_SWAP`
   cuando el wire lo sirva (§8.3); hasta entonces, el interim honesto de P1-2 (TLS sólo si flash fee > 0 en el
   bloque simulado; si no, "no emitido en el wire"). Canon: `docs/ROUTES_CROWN_JEWEL_DOCTRINE.md:42-52`
   ("Financing = dimensión de ruta"; fees SIEMPRE leídos on-chain; `financing_mode` fail-honest R8) y
   `backend/searcher-rs/src/financing.rs:70-77`.
3. **Waterfall por hop:** un renglón por hop (`Hop i/N · symA→symB`) con in/out EXACTOS humanizados en wei
   (`weiToHuman`) cuando el ledger existe (filas Sized); degradado honesto (`muted`) cuando no — jamás `value`
   inventado (GAP-12).
4. **Entrega final:** renglón "Entrega final (Δ ciclo)" con el delta en wei del token base SOLO en el hop de
   cierre (`deriveLegLedger` ya lo restringe así).
5. **Persistencia:** el dinero que aparece SOBREVIVE al siguiente push WS (INV-B) y se re-pinta al cambiar
   (P0-3) — la aceptación incluye "y no se congela ni se borra".
6. **Visibilidad estructural:** N rutas multihop simultáneas visibles en el grid (BUG-01+02 reparados) — el
   frame no colapsa a flood cuando aparecen valores USD.

### 5.2 Gate de aceptación (re-ejecutable)

- **Local (CI):** snapshot tests de la card con fixture triangular 3-hop (con ledger y sin ledger — dos estados
  honestos); assertion de NO-existencia del string "Flash loan in (TLS)" en fixture sin flash fee; INV-A con
  dos rutas 3-hop que comparten la key legada; INV-B enriquecida+push-crudo.
- **Browser (FE-05, post-deploy de la ola que cierra P0+P1):** journey en `/opportunities` con feed vivo —
  (a) >1 card triangular simultánea visible, (b) escalera con dinero persistente tras ≥2 pushes WS (o ciclos
  de polling), (c) ticker sin "%" fabricado, (d) por hop montos humanizados (no "—" en filas Sized).
- **Criterio de cierre BR-11 frontend:** (a)-(d) verdes. Lo que NO puede cerrar el frontend está en §8.3.

---

## 6. Roadmap incremental SIN big-bang (olas de PRs pequeños: gate + rollback)

Principios: cada PR = UN ID de anomalía (P-∅ §37); orden sagrado local→repo→VPS→dominio vivo; NO-GIT hasta
gate 100% (protocolo 2026-08-23); cada ola termina con medición antes/después en la descripción del PR.
Baseline del disco (FE-02b §5): `tsc --noEmit` EXIT 0 · vitest 120 files/1,114 tests/0 fails.

| Ola | PRs (contenido) | Gate de salida | Rollback |
|---|---|---|---|
| **0 — Hotfixes P0/P1** | 0a: P0-1 · 0b: P0-2 · 0c: P0-3 · 0d: P0-4 · 0e: P1-1 · 0f: P1-2 · 0g: P1-3 · 0h: P1-4+P1-5 (un PR "superficie honesta" con los 3 overflow+wallet) · 0i: P0-5 (DESPUÉS del merge CB-03) | cada PR: vitest suite afectada + tests nuevos del ítem verdes; `tsc --noEmit` EXIT 0; grep INV-1 sin regresión; browser FE-05 muestrea /opportunities+/wallets+/config | cada PR es client-only y presentacional: `git revert` del PR + redeploy (Parte 5 §37: restaurar primero, entender después). Sin cambios de wire ⇒ sin riesgo NS |
| **1 — Limpieza muerta (adjudicada)** | eliminar `lib/websocket-client.ts` + `lib/useWebSocket.ts` + sus tests; import muerto `startTransition` (`useOmniOpportunities.ts:18`); trasplantar el comentario-contrato `row_to_json(NEW)` (WO-01, `325e3154`) a `socket-lifecycle.ts` | state-lint: `grep -rE "useHotOpportunities\|HotOpportunityWebSocket\|from '@/lib/useWebSocket'" frontend/{app,components,lib,features}` = 0 hits; suite verde SIN los tests retirados; **gate condicional FE-04:** si un consumidor de producción de `useHotOpportunities` aterrizó mientras tanto (grep antes de abrir el PR), NO eliminar — aplicar FE-02a FIX-9 (setConnected en el evento connect) y rutear por la política del provider | revert del PR (los archivos vuelven); cero consumidores ⇒ cero runtime impact |
| **2 — SDQ core (Pilar A+B)** | `diffOmniOpportunity`+`OppHotMetrics` (types) → slice normalizado **por ruta** (order/entities/metrics + `applySnapshotAndDeltas` con resolución latest-by-route) → `DeltaValue`/`AgeLeaf` + reloj del store → migración de los 4 consumidores (Opportunities/Exchange/ByStrategy/Home) vía `useOpportunityList()` derivado temporal → P1-6 (dialog vivo) + P1-7 (CONNECTING) en el mismo tren → `arbx-card-grid` content-visibility (Pilar E) | GATE-DS completo (§4.3): unit + render-probe; FE-03-B lint; journey FE-05: heap/nodos/commits bajo presupuesto; medición antes/después OBLIGATORIA | el PR conserva `useOpportunityList()` como proyección del estado normalizado ⇒ revert individual posible; flag de feature NO necesario (el store deriva el mismo contenido); rollback = revert del PR + redeploy |
| **3 — Contrato de superficie (Pilar C)** | `DataSurfaceState`+`surfaceStateOf` + adopción por superficie (orden: /opportunities, /operations, /exchange, /control, luego el resto por censo FE-01) + guard BR-11 (`hiddenByFilter`) + P2-2 (suppress migration) + P2-3 + P2-4 | tests por superficie (empty-con-razón, error-verbatim, filtro-honesto); grep INV-1 = 0; cada página migrada = UN PR chico | aditivo por página: revert de la página sin afectar las demás |
| **4 — Presupuesto (Pilar D)** | `perf-budgets.json` + `fe03-budget-check.mjs` en CI + baseline FE-05 documentada | budget-check verde con baseline real medida; tabla de rutas `next build` bajo techo | el check es aditivo: bajar/revertir el techo es un PR; el baseline queda versionado |
| **5 — Consolidación de transporte** | canales del provider (rooms `metrics`/`convergence`/telemetry ya existen server-side — auditar shape ANTES de migrar; `prices` auditar) → `OpportunityTicker` al store (5ª vía eliminada) + P2-5 (translator tokens) + Suspense por panel (/operations, /readiness) | state-lint io()-allowlist sin extras; ticker consume store (grep fetch propio = 0); CLS ≤ 0.1 en panels con Suspense | cada canal = UN PR; revert por canal |
| — | **Post-operatorio (decisión del operador, NO PRs de la mesa):** D-11 (public hostname del tunnel CF → :80 con el upgrade socket.io — mientras no, el dominio público corre POLLING y P1-1 es el frame completo) · migración Next 15/React 19 (reabre tabla E de FE-03) · CF cache rule `/_next/static` (dashboard) · virtualización JS SOLO si la medición §ola-4 supera el techo con content-visibility puesto | — | — |

**Orden dentro de la ola 0:** 0a (key) y 0d (merge) son independientes; 0c (comparador) DEBE aterrizar con o
antes de 0d; 0e (ventana 200) multiplicará el costo del render si 0a no está (más filas visibles) — secuencia
recomendada: 0a → 0c → 0d → 0b → 0e → 0f → 0g → 0h → 0i.

---

## 7. Contrato de adopción de la mesa redonda (rige TODO PR frontend futuro)

> Esta sección está redactada para ser aplicada por CUALQUIER agente de la mesa SIN contexto adicional.
> Si un PR frontend viola un ★ de §3, el reviewer LO BLOQUEA — no es sugerencia.

1. **Antes de abrir el PR:** identifica tu anomalía (ID tracker o L4 con timestamp — P-∅: un PR = un ID);
   verifica `git branch --show-current` (§36 — el árbol compartido rota de branch); re-ancla los line-numbers
   de los diffs de este plan contra el árbol actual.
2. **Durante:** aplica el checklist §3 ítem por ítem. Los diffs de origen viven en
   `audits/frontend-doctrine-2026-09-07/` (FE-02a §3, FE-02b §2, FE-03 §2-§6) — cítalos en el PR. Marca cada
   hunco `// FE-04 (2026-09-07)` o el del WO de origen.
3. **Datos:** RULE 00 absoluta — el frontend renderiza EXACTAMENTE lo que devuelve la API/WS; vacío = vacío
   con razón; `null` = no computado; jamás fabricar/escalar/interpolar un valor económico; jamás wei→USD
   derivado; el delta-streaming sólo PROPAGA cambios reales del servidor (INV-DS-1).
4. **BR-11:** todo cambio que toque la card del feed se evalúa contra la aceptación §5 — el dinero por hop,
   el financiamiento veraz y la entrega final son REQUISITO, no cosmética. Un PR que "mejora" la card pero
   empeora cualquiera de §5.1.1-6 se rechaza.
5. **CB-03:** `/control` es superficie de control de capital — su estado de feed lee las MISMAS fuentes
   honestas del pipeline (wsStatus con CONNECTING real, ventana declarada); prohibido un LED verde cuya
   fuente no distinga CONNECTING de LIVE (precedente WO-08).
6. **Después:** `npx tsc --noEmit` EXIT 0 + `npx vitest run` 0 fails + gates del ítem + FE-01-GATE-1 si
   tocaste rutas. Deploy y verificación en dominio vivo = orquestador (FE-05) — el agente NO deploya.
7. **Conflicto con este plan:** si la evidencia contradice una regla de aquí, NO la ignores — nómbrala,
   refútala con file:line y propón el amend en tu reporte bajo `audits/frontend-doctrine-2026-09-07/`
   (la doctrina es versionada por la mesa, no inmutable por capricho).

---

## 8. Dependencias declaradas (NO son frontend — propiedad de otros workstreams)

### 8.1 Operador (dashboard, no repo)
- **D-11:** public hostname del tunnel CF → `localhost:80` (nginx tiene el upgrade socket.io; hoy apunta a
  :5173 y el WS del dominio público muere → POLLING). Verificado vivo 2026-09-07 (FE-02a §0,
  `journalctl cloudflared` 12:52Z). Mientras no se remedie, INV-DS-3 es la única defensa del feed.
- CF cache rule `/_next/static` (ola 5+, junto a D-11).

### 8.2 Programas hermanos (coordination)
- **CB-03** (`app/control/` untracked en este árbol): P0-5 (sidebar) se abre DESPUÉS de su merge.
- **HOPS-LIVE-01** (branch actual): FE-01-GATE-1 y los diffs de ola 0 deben re-verificarse sobre SU base al
  aterrizar en main (merge-cascade — precedentes 405 en MEMORY).

### 8.3 Backend (BR-11 requiere, el FE ya está diseñado para consumir)
1. **Persistir + servir `financing_mode`** (GAP-11): vive en `size_optimizer.rs:968,1229`
   (`crate::financing::selected_mode`), hoy sólo alcanza logs/telemetría. Columna + SELECT en
   `opportunities-live.ts` + payload del trigger. El chip verbatim (§5.1.2) lo consume directo.
2. **Ledger por leg del kernel triangular** (GAP-12): `attach_leg_ledger` (shared-rs) sólo emite para filas
   Sized 2-leg V2/V3; el waterfall por hop en triangular exige `leg_amounts_in/out` por hop del kernel.
3. **Enriquecer el puente LISTEN** (`api-server/src/index.ts:1847-1863`) o persistir el bloque `simulated_*`
   — remedio estructural de BUG-03 (P0-4 es la contención).
4. **Ancla USD por token en el wire** si algún día se exige USD por hop (hoy INV-D lo prohíbe derivarlo).

---

## 9. Atribución y trazabilidad de la mesa

| Aporte | Par (archivo) | Verificado por FE-04 |
|---|---|---|
| Censo 58 páginas + mapa de relaciones + FE-01-GATE-1 | FE-01-DESIGN.md | gate re-ejecutable citado; huérfanas confirmadas por grep propio |
| BUG-01..10, GAP-11..13, FIX-1..10, cadenas BR-11, PG vivo | FE-02a-DESIGN.md | anclajes clave re-verificados (`routeKeyOf`, comparador, websocket-client history) |
| FE-02b-01..10, D-01..D-09, INV-FE02B-1..5, baseline tsc/vitest | FE-02b-DESIGN.md | convergencia BUG-06=FE-02b-01 confirmada; grep `/control` = 0 confirmado |
| Pilares A-E (SDQ, estado único, superficie, presupuesto, vanguardia), olas | FE-03-DESIGN.md | composición §4-§6; re-key por ruta es adición FE-04 |
| P0-5 (sidebar /control), re-key del store por ruta, adjudicación websocket-client, composición BR-11/CB-03, contrato §7 | FE-04 (este documento) | evidencia propia citada inline |

**Clasificación:** todo lo citado con file:line es [CANONICAL_REPO] verificado por el par que lo reportó y,
donde se marca "verificado por mí", re-verificado por FE-04 en `feat/hops-live-01` @ `27aca289`. [UNKNOWN]
heredados de FE-03 §9.3 (shapes/cadencias rooms metrics/convergence/prices — se auditan en ola 5 ANTES de
migrar). Ninguna cifra de este documento fue medida en dominio vivo por FE-04 (presupuesto 0/5 HTTP usado).

— ecc:react-reviewer · WO FE-04 · 2026-09-07 · FRONTEND-DOCTRINE.md es el único write de producción-adyacente
de este WO (ninguna línea de código de producción fue editada).
