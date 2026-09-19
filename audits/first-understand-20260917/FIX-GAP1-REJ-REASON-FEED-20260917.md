# FIX — GAP1-REJ-REASON-FEED (2026-09-17, fixer gang ronda 1)

> Gap asignado: "rejection_reason not rendered on /opportunities feed cards (API returns
> it, DOM shows 0 occurrences) — operator sees REJECTED without WHY in that view; reason
> only visible aggregated in /operations taxonomy".
> Origen del gap: `BROWSE-Auditor-de-honestidad-R8-...md` §2.2 GAP-1 (:55-60, :104) —
> medición contra `document.body.innerText`, 0 ocurrencias de las razones con la API
> trayéndolas (20/20 items con `rejection_reason` poblado en `GET /api/opportunities/live`).
> Reglas respetadas: RULE 00 / R8 (cero datos fabricados), §32/§33 (cero VPS),
> NO-GIT (cero commit/push/PR/deploy — diff en working tree, marcado
> `// GAP1-REJ-REASON-FEED (2026-09-17)`).

## 1. Causa raíz — CANONICAL_REPO, con reconciliación de la contradicción de mesa

**El dato YA viajaba hasta el componente; el defecto era de VISIBILIDAD, no de wiring.**

Cadena completa re-derivada del código (no heredada):
- Wire → store: `mapToOmniOpportunity` mapea `rejection_reason` (string|null) —
  `frontend/lib/store/types.ts:288` (tipo) y `:407` (mapper, null-preservador: R8).
- Store → card: `OpportunitiesClient.tsx:436-449` monta `OpportunityTradeCard` desde el
  omni-store; `OpportunityTradeCard.tsx:276` ya pasaba la prop:
  `<StatusPill status={opp.status} rejection_reason={opp.rejection_reason} />`.
- **El defecto**: `frontend/components/StatusPill.tsx` (pre-fix :112-125) ponía la razón
  SOLO en el atributo `title` del `<span>` (tooltip on-hover). Un atributo `title` NUNCA
  aparece en `document.body.innerText` — exactamente lo que el R8 midió ("0 ocurrencias
  aunque la API las trae"). Para un operador que no pase el mouse por cada una de ~750
  tarjetas, el feed mostraba "REJECTED" sin POR QUÉ.

### 1.1 Contradicción con `BROWSE-Operador-de-mesa-...md` §1 — REFUTADA para texto visible

El BROWSE de mesa (:34-36) afirmó "Cada tarjeta trae razón de rechazo como badge
(ej. `spot_product_le_one`)". **Refutado con triple evidencia** (mismo deploy a06a968d,
misma ventana ~06:20Z):
1. **Código**: pre-fix, la única vía de render de la razón era el atributo `title`
   (StatusPill.tsx:114-125 pre-fix) — no existía nodo de texto con la razón en ninguna
   tarjeta. Reconciliación del error del par: un snapshot de accesibilidad (Playwright
   a11y tree) SÍ expone el `title` como descripción, lo que explica que el agente BROWSE
   "viera" la razón sin que fuera texto visible. Clase de error: instrumento de lectura,
   no de datos.
2. **Screenshot del propio R8** (`screenshots/r8-opportunities-feed-rejected-no-reason.png`,
   re-inspeccionado en este fix con análisis de imagen): los badges dicen solo
   `REJECTED` + tag `WETH`; cero razones visibles en las tarjetas.
3. **Test pre-fix** (`__tests__/StatusPill.test.tsx:38-43`): la única aserción de razón
   era `title="[^"]*TokenNotAllowed` — el contrato de prueba mismo documentaba la
   superficie como title-only.

## 2. Fix aplicado (working tree, marcador `// GAP1-REJ-REASON-FEED (2026-09-17)`)

`frontend/components/StatusPill.tsx` (única pieza de código tocada):
- Cuando `status === "rejected" && rejection_reason != null`, el pill ahora renderiza la
  razón como TEXTO VISIBLE dentro del propio badge: `REJECTED · v3_quote_unavailable`
  (span font-mono, `max-w-[220px] truncate` por higiene de layout en razones largas).
- El atributo `title` se CONSERVA con el valor completo (el truncate visual nunca oculta
  el dato: hover revela el reason íntegro).
- **Fail-honest R8 intacto**: `rejection_reason` null/ausente en el wire → el pill
  muestra solo `REJECTED`, sin texto extra fabricado (nada se inventa, nada se maquilla).
- Estados no-rejected NO renderizan razón (guard explícito `status === "rejected"`).
- R1 intacta: StatusPill sigue siendo componente puro sin hooks/window/Date — SSR render
  === CSR render; la razón llega por props desde el snapshot inicial y del store, no hay
  no-determinismo nuevo.

Alcance del consumidor verificado: el StatusPill de `components/StatusPill.tsx` tiene
UN solo consumidor vivo (`OpportunityTradeCard.tsx:276` — el feed `/opportunities`). Los
`StatusPill` de `features/route-discovery/premium-ui.tsx:21` son un componente DISTINTO
(firma `{ status: FeedStatus }`, sin rejection_reason) usado por 5 paneles de
route-discovery — NO tocado, cero regresión posible ahí.

## 3. Verificación

- `npx --no-install vitest run components/__tests__/StatusPill.test.tsx
  components/__tests__/OpportunityTradeCard.test.tsx` → **19/19 PASS**
  (StatusPill 13 — 10 preexistentes + 3 nuevos; OpportunityTradeCard 6, sin cambios).
  Tests NUEVOS (contrato del fix): razón como texto visible vía strip-tags (no title);
  null → innerText del pill == `REJECTED` exacto (fail-honest); non-rejected no renderiza
  razón.
- `npx --no-install vitest run` (suite completa): **126 archivos / 1268 tests PASS**
  (baseline de mesa era 1265; +3 = los tests nuevos de este fix. El error preexistente
  en `useTokenIcon.test.ts` reportado por el par RDO-503 §3 YA NO está — zona del par
  4.5, presumiblemente cerrada por su fix; 0 errores ahora).
- `npx --no-install tsc --noEmit -p tsconfig.json` → **0 errores** (árbol completo).
- Verificación visual en vivo NO ejecutada: el fix NO está desplegado (NO-GIT; VPS corre
  a06a968d). Post-deploy para WO-06/operador: abrir /opportunities y confirmar el texto
  `REJECTED · <reason>` en tarjetas rechazadas (regla RULE 03: rebuild frontend
  --no-cache con --env-file).

## 4. Presupuesto y límites

- HTTP manual: **0/5** navegaciones + 1 análisis de imagen (asset local ya capturado por
  el R8, no un request a dominio público).
- VPS: NO tocado (0 lecturas, 0 mutaciones). Cero cargo/build Rust. Cero git.

## 5. Sincronía de mesa redonda

- Construye sobre: `BROWSE-Auditor-de-honestidad-R8-...md` §2.2 GAP-1 (gap original) ·
  snapshot `r8-opportunities-feed-rejected-no-reason.png` (evidencia visual re-usada).
- **Refuta con evidencia** a `BROWSE-Operador-de-mesa-...md` §1:34-36 ("cada tarjeta
  trae razón como badge") — la razón NO era texto visible; era title-attr invisible a
  innerText (ver §1.1). Nota para futuros BROWSE: no usar a11y-snapshot como proxy de
  "texto visible para el operador"; contrastar con innerText o screenshot.
- Eco del mismo patrón "R8 en wire, perdido en view" ya fichado por el par
  `FIX-RDO-503-REASON-UI-20260917.md` §5 (patrón hook→card que descarta la razón) —
  aquí la variante es prop presente → render title-only. Ambos cierran la misma clase
  en superficies distintas, sin solapamiento de archivos.
- Para WO-04 (fichas TS): `OpportunityDetailDialog` NO muestra `rejection_reason`
  (grep 0 matches) — el detalle completo de una detección rechazada tampoco explica el
  por qué fuera del pill. Fuera del charter de este fix (feed cards); registrado como
  candidato de mejora para quien fiche el dialog.
- Estado VPS vs local (heredado de mesa): producción corre a06a968d; este fix requiere
  deploy gated del operador para ser visible en la DApp.

## 6. Claims de archivo (conflictos)

- `frontend/components/StatusPill.tsx` + `frontend/components/__tests__/StatusPill.test.tsx`:
  superficie frontend pura, sin claims de otros WO (WO-04 no iniciado). Sin conflicto.
- `OpportunityTradeCard.tsx`: NO tocado (ya pasaba la prop correctamente).
- `components_backup/StatusPill.tsx`: backup muerto con el patrón title-only —
  deliberadamente intocado (no es superficie viva; precedente del par RDO-503 §6).

## Clasificación de afirmaciones

- Cadena wire→store→card→pill y render title-only pre-fix: CANONICAL_REPO
  (types.ts:288/:407, OpportunityTradeCard.tsx:276, StatusPill.tsx pre-fix :112-125,
  test pre-fix :38-43).
- "0 ocurrencias en innerText" (gap original): PRIMARY_SOURCE del R8 (browser, 06:21Z)
  + confirmado por análisis del screenshot del propio R8 (este fix).
- Contradicción BROWSE §1: REFUTADA para texto visible (código + screenshot + test
  pre-fix); explicada como lectura de a11y-snapshot (INFERRED la mecánica exacta del
  error del par — no reproducible desde aquí).
- Tests/tsc post-fix: PRIMARY_SOURCE (salidas de este run, reproducibles con los
  comandos de §3).
