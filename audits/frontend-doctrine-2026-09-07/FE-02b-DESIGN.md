# FE-02b — DESIGN: bugs de RENDER / HIDRATACIÓN / A11Y / OVERFLOW / MEMORIA (mitad B — UI)

> **WO:** FE-02b · **kind:** design · **agente:** ecc:a11y-architect (Gang Omniscience)
> **Fecha:** 2026-09-07 · **Rubric:** ecc:a11y-architect sobre bugs cross-page de render.
> **Claim:** `frontend/components`, `frontend/app`, `frontend/components/LocalTime.tsx`,
> `frontend/components/last-updated.tsx`. **CERO código de producción editado** (NO-GIT,
> protocolo operador 2026-08-23); los diffs de este archivo son artefactos de diseño marcados
> `// WO-FE-02b (2026-09-07)` — los aplica el orquestador vía PR con ID (P-∅, §37).
> VPS: no tocado (§32/§33 read-only). Lexicon OMEGA en todo el texto.

## 0. Sincronía de mesa redonda (obligatoria)

Leídos ANTES de opinar:
- `audits/frontend-doctrine-2026-09-07/GOAL-WORKORDERS.md` (completo — FE-01..FE-05).
- `.claude/skills/arbitragex-omniscience/LEARNINGS.md` — BR-11: *"la API live devuelve multihop
  (30/100 triangular) — el 'desaparecen cuando hay USD' es del FRONTEND"* → **la tarjeta
  multihop/aguaflal por hop es charter de BR-11 (cerebro), NO mío: citado, no duplicado.**
- `.agents/skills/01-hydration-forensics-expert/skill.md` (reglas inmutables #418/#423/#425;
  `suppressHydrationWarning` sólo `<html>` o casos aislados justificados) y
  `.agents/skills/101-react-hydration-mounted-snapshot.md` (canon R1: estado inicial
  determinista + gate `isMounted` + fallback `"--"`).
- **Pares (boards hermanos, 2026-09-07):**
  - `audits/control-board-2026-09-07/CB-VERIFY-FRONTEND-VERIFY.md` (ecc:react-reviewer) —
    verificó `/control` con gates R1+R5+a11y: **PASS 3/3 con advisory F-1** (panel confirm sin
    mover foco, `cb-confirm-error` sin `role="alert"`) **y F-2** (dot fluor lime-400 ≈1.5:1 sobre
    card claro, mitigado por label textual). Mis hallazgos **FE-02b-02/03 generalizan la misma
    clase** a `WalletDetailDialog`; NO contradigo nada — confirmo y extiendo.
  - `audits/cerebro-2026-09-07/BR-01-DESIGN.md` (ecc:database-reviewer) — etapas S1..S9 del
    embudo; su `PipelineFunnelCard` renderiza esas cuotas → mi O3 (celda 64px) cae justo ahí.
- **FE-02a (mitad A)** posee honestidad/pérdida de datos WS (`lib/websocket-client.ts` hoy
  modificado sin commitear por un peer): **citado, no auditado aquí.** Yo audito el lado
  componentes (cleanup/consumo), no la semántica de los payloads.
- `audits/frontend-doctrine-2026-09-07/` estaba vacío salvo el GOAL al iniciar (soy de los
  primeros): **el censo FE-01 NO existe** → apliqué la metodología del charter: audité las
  páginas de mayor tráfico (evidencia de uso: screenshots/postdeploys 08-25→09-06 en git status)
  y declaro el resto **PENDIENTE R8** (§1).

## 1. Cobertura (censo de auditoría)

**Auditados a fondo (file:line verificado a ojo, no inferido):** layout raíz
(`app/layout.tsx` + `HeroSphere`, `OpportunityTicker`, `RuntimePostureBar`, `app-sidebar`,
`theme-toggle`, `WebSocketIndicator`, `providers/ArbxRealtimeProvider`, `grain-overlay`) ·
`/opportunities` (`OpportunitiesClient`, `OpportunityTradeCard`, `OpportunityDetailDialog`,
`live-ticker`) · `/opportunities/exchange` (`OpportunitiesExchangeClient`,
`OpportunityExchangeCard`, `PriceTicker`) · `/opportunities/by-strategy` · `/wallet`
(`SiweAuthPanel`, `WalletStatusCard`, `WalletDetailDialog`) · `/dex-registry`
(`DexDetailDialog`) · `/control` (cité verificación del peer) · `/config` · `/operations`
(`PipelineFunnelCard`, `ArchivePanel`) · `/live-readiness` · `/monitor` (`ServiceStatus`) ·
`/status` · `/readiness` (`GoNoGoPanel`, `BlockersPanel`, `last-updated`, `SourceMeta`) ·
`/apex/allocator` · `/omega-s5/drift` · `/onboarding/2-connect` (`Phase2Client`) ·
`/admin/chains` · primitivas `ui/card`, `ui/sheet`, `ui/alert` · `features/table/DataTable` ·
defensas overflow de `features/executions` y `features/paper-history`.

**PENDIENTE R8 (no auditados — sin hallazgo afirmado ni negado):** `/admin/signin`,
`/admin/topology`, `/agent-insights`, `/audit-logs`, `/chains`, `/config/trading`,
`/deploy-pipeline`, `/executions` (página completa), `/killswitch`, `/live-testnet`,
`/omega-s5/{core,crucible,factory,operator,registry,registry/[entity],wallets,adapters}`,
`/onboarding/{1-init,3-advanced,4-testing,5-production}` (y página índice),
`/operator/{page,presets,self-test}` (self-test parcial vía grep de suppress — spans OK),
`/paper/history` (página completa), `/pools`, `/recon`, `/risk`, `/route-outcomes`,
`/routes/discovery`, `/rpcs`, `/sed`, `/settings`, `/settings/credentials` (parcial vía grep),
`/strategies` (+12 tabs), `/translator`, `/wallets`, `/worker-health`. `app_backup/` y
`components_backup/` **excluidos** (no routeados).

## 2. Hallazgos (cada uno: clase · severidad · file:line · repro · diff exacto)

Convención de severidad: **P1** = usuario operador ve dato falso o UI rota en flujo principal;
**P2** = degrada UX/a11y de un flujo real; **P3** = violación doctrinal/latent sin síntoma hoy.

---

### FE-02b-01 · RENDER/RULE 00 · **P1** — El ticker global FABRICA Topological Yield %
**Archivo:** `frontend/components/OpportunityTicker.tsx:56-59` (+ render `:140-153`).
```tsx
  const yieldPct = opp.roi_pct ?? (profit > 0 ? profit * 0.1 : profit * 0.1); // Rough scaling
```
Cuando el payload no trae `roi_pct`, se inventa un porcentaje (`profit * 0.1`, literalmente
comentado "Rough scaling" — ambos brazos del ternario son idénticos, además). Ese número se
renderiza como `+X.XX% ▲` — visual (`OpportunityTicker.tsx:151-153`) **y** en el resumen
`sr-only` (`:140`) — y el ticker monta en el **layout raíz** (`app/layout.tsx:139`) → el
porcentaje fabricado aparece en el header de **TODAS las páginas de la DApp**. Violación
directa de RULE 00 (nada de datos decorativos) y del fail-honest R8: el ticker ya tiene el
estado honesto "—" para otros campos.
**Repro:** cualquier ítem de `/api/opportunities/live?limit=20` con `net_expected_profit_usd`
presente y `roi_pct: null` → el marquee global muestra `+0.42%` (4.22×0.1) que ningún motor
computó.
**Diff (D-01):**
```tsx
// frontend/components/OpportunityTicker.tsx — ANTES (líneas 7-14 y 56-59)
interface TickerItem {
  pair: string; from: string; to: string;
  yield: number;          // ← línea 11
  ago: string;
}
...
  const yieldPct = opp.roi_pct ?? (profit > 0 ? profit * 0.1 : profit * 0.1); // Rough scaling

// DESPUÉS
interface TickerItem {
  pair: string; from: string; to: string;
  yield: number | null;   // WO-FE-02b (2026-09-07): RULE 00 — sin roi_pct real no hay %.
  ago: string;
}
...
  // WO-FE-02b (2026-09-07): RULE 00 — jamás fabricar Topological Yield: sin roi_pct del
  // payload el ticker renderiza "—" (fail-honest R8). El escalado profit*0.1 se elimina.
  const yieldPct = opp.roi_pct ?? null;
```
Y los 3 sitios de render (mismo archivo):
```tsx
// :140 (resumen sr-only) — ANTES
{latest.yield >= 0 ? "+" : ""}{latest.yield.toFixed(2)}%.
// DESPUÉS
{latest.yield == null ? "yield not computed" : `${latest.yield >= 0 ? "+" : ""}${latest.yield.toFixed(2)}%`}.
```
```tsx
// :149-153 (marquee) — ANTES
          const isPositive = item.yield >= 0;
          ...
              <span className={isPositive ? "pos" : "neg"}>
                {isPositive ? "+" : ""}{item.yield.toFixed(2)}%
              </span>
              <span className={`arr ${isPositive ? "pos" : "neg"}`}>
                {isPositive ? "▲" : "▼"}
              </span>
// DESPUÉS
          const y = item.yield;                       // WO-FE-02b (2026-09-07)
          const isPositive = (y ?? 0) >= 0 && y !== null;
          ...
              <span className={isPositive ? "pos" : "neg"}>
                {y == null ? "—" : `${isPositive ? "+" : ""}${y.toFixed(2)}%`}
              </span>
              {y !== null && (
                <span className={`arr ${isPositive ? "pos" : "neg"}`}>
                  {isPositive ? "▲" : "▼"}
                </span>
              )}
```
*(si `y == null` la flecha direccional también miente — se oculta junto con el %).*

---

### FE-02b-02 · RENDER/A11Y/fail-honest · **P1** — `WalletDetailDialog`: rejection del fetch = spinner eterno (sin `.catch`)
**Archivo:** `frontend/components/WalletDetailDialog.tsx:72-88`.
`Promise.all([...]).then(...)` **sin `.catch` ni `allSettled`**: si alguno de los dos fetch
**throw** (p. ej. `TypeError: Failed to fetch` con edge caído, DNS, CORS), la promesa rechaza,
el `.then` jamás corre → `loading` queda `true` **para siempre** y ambas secciones muestran
"Fetching balances… / Fetching allowances…" eternas + unhandled promise rejection en consola.
El estado de error honesto (`EndpointNotice`) existe pero es **inalcanzable** por esta vía.
**Repro:** `/wallets` → abrir el detalle de cualquier wallet con DevTools en Offline (o edge
devuelto conexión rota a mitad de vuelo) → el Sheet queda clavado en los dos spinners; el
`EndpointNotice` jamás aparece.
**Diff (D-02):**
```tsx
// ANTES (WalletDetailDialog.tsx:72-88)
    Promise.all([
      getWalletBalances(EDGE_URL, wallet.address),
      getWalletAllowances(EDGE_URL, wallet.address),
    ]).then(([balRes, allowRes]) => {
      if (cancelled) return;
      if (balRes.ok) { setBalances(balRes.data.balances); } else { setBalError(balRes.error); }
      if (allowRes.ok) { setAllowances(allowRes.data.allowances); } else { setAllowError(allowRes.error); }
      setLoading(false);
    });

// DESPUÉS
    // WO-FE-02b (2026-09-07): allSettled — una rejection de red dejaba `loading=true`
    // eterno y sin EndpointNotice (fail-honest R8 roto). Nunca más "Fetching…" terminal.
    Promise.allSettled([
      getWalletBalances(EDGE_URL, wallet.address),
      getWalletAllowances(EDGE_URL, wallet.address),
    ]).then(([balRes, allowRes]) => {
      if (cancelled) return;
      const reason = (r: PromiseRejectedResult) =>
        r.reason instanceof Error ? r.reason.message : String(r.reason);
      if (balRes.status === "fulfilled") {
        if (balRes.value.ok) setBalances(balRes.value.data.balances);
        else setBalError(balRes.value.error);
      } else {
        setBalError(reason(balRes));
      }
      if (allowRes.status === "fulfilled") {
        if (allowRes.value.ok) setAllowances(allowRes.value.data.allowances);
        else setAllowError(allowRes.value.error);
      } else {
        setAllowError(reason(allowRes));
      }
      setLoading(false);
    });
```

---

### FE-02b-03 · A11Y · **P2** — `EndpointNotice` sin `role="alert"`: el lector de pantalla jamás anuncia la falla del edge
**Archivo:** `frontend/components/WalletDetailDialog.tsx:37-44`.
El codebase YA adoptó `role="alert"` en 19 archivos (censo grep: `QuarantineStrip`,
`ControlBoardLed`, `ui/alert`, tabs de strategies, panels de operations…). `EndpointNotice`
es la notificación de falla más crítica del flujo wallets y quedó fuera del patrón — misma
clase que el advisory **F-1 del peer en `/control`** (`cb-confirm-error` sin alert). Con D-02
aplicado, el error será alcanzable pero inaudible para AT.
**Repro:** con NVDA/VoiceOver activo, abrir un wallet con edge caído (post D-02) → el AT no
interrumpe; el error sólo se descubre al tabular a ciegas.
**Diff (D-03):**
```tsx
// ANTES (WalletDetailDialog.tsx:39)
    <div className="flex items-start gap-2 rounded-md border border-warning/40 bg-warning/10 p-3 text-xs text-warning">
// DESPUÉS
    {/* WO-FE-02b (2026-09-07): patrón role="alert" ya adoptado en 19 archivos — el fallo
        del edge se anuncia a AT en el momento en que aparece. */}
    <div role="alert" className="flex items-start gap-2 rounded-md border border-warning/40 bg-warning/10 p-3 text-xs text-warning">
```

---

### FE-02b-04 · HIDRATACIÓN (R1 letra) · **P3** — `suppressHydrationWarning` en CONTENEDORES (7 sitios activos)
R1/proyecto: *"suppressHydrationWarning solo en `<span>` individual, NUNCA en contenedores"*
(skill 01 §4 ídem). Censo completo (excl. `*_backup`; ejecutado como gate INV-1, §5): todos
los usos son spans individuales o el `<html>` del layout (excepción sancionada) **excepto 7**:

| # | file:line | elemento | ¿mismatch activo hoy? |
|---|---|---|---|
| a | `app/opportunities/OpportunitiesClient.tsx:301` | `<p>` con 5 hijos de estado + reloj | No (todo gated por `isMounted`) — pero enmascara cualquier regresión futura dentro del `<p>` |
| b | `components/OpportunityTradeCard.tsx:296` | `<div>` (sus 2 spans internos :298/:311 YA tienen suppress propio) | No — el del div es **redundante** además de prohibido |
| c | `app/wallet/SiweAuthPanel.tsx:135` | `<div>` (badges/botones gated por `mounted`) | No |
| d | `components/theme-toggle.tsx:64` | `<Button>` (icono placeholder pre-mount determinista) | No |
| e | `components/wallet/WalletStatusCard.tsx:58` | `<dd>` (valor address gated por `mounted`) | No — leaf de valor único: pasa el "caso justificado" de skill-01 §4 pero no la letra R1 span-only |
| f | `components/wallet/WalletStatusCard.tsx:64` | `<dd>` (red; una rama renderiza un `<Badge>` compuesto) | No — ídem; y al contener un elemento compuesto se aleja más del "individual" |
| g | `components/wallet/WalletStatusCard.tsx:76` | `<dd>` (balance con 4 ramas gated) | No — ídem |

*Transparencia de método:* mi primer pase de grep clasificó (e–g) como "spans OK" por leer el
comentario justificativo del span :34 del mismo archivo; el gate ejecutado (§5) los re-cazó y
los incorporo — la verificación adversarial funciona sobre uno mismo. Ninguno de los 7 produce
#425/#418 hoy (el gating `mounted` los hace deterministas — lo verifiqué archivo por archivo):
la severidad es doctrinal/latent — un suppress de contenedor convierte el **próximo** mismatch
introducido ahí en silencio en vez de error de CI/consola.
**Repro (latent):** añadir cualquier hijo no-determinista (p. ej. un `Date.now()` suelto)
dentro de uno de esos 7 contenedores → React NO reporta el mismatch → la página degrada a
client-render #423 sin alerta a nadie.
**Diffs (D-04a..d):**
```tsx
// D-04a — OpportunitiesClient.tsx:296-311 — mover el suppress AL SPAN del reloj
// (espejo exacto del patrón canónico ya presente en
//  app/opportunities/exchange/OpportunitiesExchangeClient.tsx:271-274)
// ANTES
          <p className="text-muted-foreground mt-2 text-sm" suppressHydrationWarning>
            {feedStatus === "LIVE"
              ? "Live stream via WebSocket"
              : ... }
            {" · "}
            {isMounted && lastRefresh ? `Last refresh: ${lastRefresh.toLocaleTimeString()}` : "Loading..."}
          </p>
// DESPUÉS
          <p className="text-muted-foreground mt-2 text-sm">
            {feedStatus === "LIVE"
              ? "Live stream via WebSocket"
              : ... }
            {" · "}
            {/* WO-FE-02b (2026-09-07): R1 — el suppress vive SOLO en el <span> del reloj
                (patrón canónico OpportunitiesExchangeClient.tsx:271-274), nunca en el <p>. */}
            <span suppressHydrationWarning>
              {isMounted && lastRefresh ? `Last refresh: ${lastRefresh.toLocaleTimeString()}` : "Loading..."}
            </span>
          </p>
```
```tsx
// D-04b — OpportunityTradeCard.tsx:296
// ANTES
        <div className="flex items-center gap-1.5 text-muted-foreground" suppressHydrationWarning>
// DESPUÉS
        {/* WO-FE-02b (2026-09-07): R1 — suppress de contenedor retirado; los <span>
            individuales (:298/:311) ya cubren los segmentos vivos. */}
        <div className="flex items-center gap-1.5 text-muted-foreground">
```
```tsx
// D-04c — SiweAuthPanel.tsx:135
// ANTES
        <div className="flex flex-wrap items-center gap-3" suppressHydrationWarning>
// DESPUÉS
        {/* WO-FE-02b (2026-09-07): R1 — contenido gated por `mounted` = determinista;
            el suppress de contenedor sólo enmascararía regresiones futuras. */}
        <div className="flex flex-wrap items-center gap-3">
```
```tsx
// D-04d — theme-toggle.tsx:62-65
// ANTES
      aria-label={mounted ? `Switch to ${theme === "dark" ? "light" : "dark" mode` : "Switch theme"}
      suppressHydrationWarning
    >
// DESPUÉS
      aria-label={mounted ? `Switch to ${theme === "dark" ? "light" : "dark" mode` : "Switch theme"}
      {/* WO-FE-02b (2026-09-07): R1 — pre-mount es determinista en SSR y CSR
          (placeholder opacity-0 + aria-label "Switch theme"); suppress innecesario
          y de contenedor (Button). */}
    >
```
*(nota D-04d: el `aria-label` pre-mount es idéntico SSR/CSR porque `mounted=false` en ambos —
verificado; el suppress no está protegiendo nada real).*
```tsx
// D-04e — WalletStatusCard.tsx:56-87 — patrón único para los 3 <dd> (e/f/g):
// el suppress baja AL <span> del valor; el <dd> queda limpio.
// ANTES (ejemplo, :58)
            <dd className="mt-1 font-mono text-sm" suppressHydrationWarning data-testid="wallet-address">
              {mounted && isConnected && address ? abbreviate(address) : "—"}
            </dd>
// DESPUÉS
            <dd className="mt-1 font-mono text-sm" data-testid="wallet-address">
              {/* WO-FE-02b (2026-09-07): R1 letra — suppress SOLO en el <span> del valor
                  vivo (gated por mounted), nunca en el <dd> contenedor. */}
              <span suppressHydrationWarning>
                {mounted && isConnected && address ? abbreviate(address) : "—"}
              </span>
            </dd>
```
*(misma mecánica exacta para `:64` Network y `:76` Balance — un ÚNICO `<span>` interior
envuelve la expresión condicional completa y lleva el suppress; los `data-testid` permanecen
en el `<dd>`, ningún selector de tests se rompe: los tests apuntan al dd por testid y el
contenido textual no cambia).*
**Positivo citado:** `app/omega-s5/drift/page.tsx:21-26` documenta y aplica el patrón
sancionado (span individual + comentario de justificación) — que sea el referente.

---

### FE-02b-05 · OVERFLOW · **P2** — `/config`: `dd` mono sin `break-*` en track `1fr` (= `minmax(auto,1fr)`) → scroll-x con valores largos
**Archivo:** `frontend/app/config/page.tsx:23-27` (componente `KV`, usado por las 4 cards
System/Execution/Risk/Scoring, `:94-106`).
```tsx
    <dl className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1.5 text-sm">
      ...
          <dd className="font-mono text-[13px]">{String(v)}</dd>
```
Un track `1fr` en grid = `minmax(auto, 1fr)`: el mínimo es `auto`, así que un token
inquebrable (URL de RPC, hash, path) **empuja el track más allá de la card** → scroll-x de
página (lección #514-#518: la misma familia que `alert.tsx 1fr→minmax(0,1fr)`). Los valores de
`c.execution`/`c.system` provienen de `configs/app.toml` via edge — sin control de longitud
por el frontend.
**Repro:** cualquier valor de config con un token sin espacios más largo que el ancho de
columna (p. ej. `https://eth-mainnet.g.alchemy.com/v2/<key>`) → la card System desborda y el
body hace scroll horizontal (móvil 320px: garantizado).
**Diff (D-05):**
```tsx
// ANTES (config/page.tsx:23-27)
    <dl className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1.5 text-sm">
      {Object.entries(obj).map(([k, v]) => (
        <Fragment key={k}>
          <dt className="text-muted-foreground">{k}</dt>
          <dd className="font-mono text-[13px]">{String(v)}</dd>
        </Fragment>
      ))}
    </dl>
// DESPUÉS
    {/* WO-FE-02b (2026-09-07): overflow 3-capas #514-#518 — track 1fr→minmax(0,1fr)
        + break-all en dd (los valores son tokens inquebrables: URLs/hashes de app.toml). */}
    <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-6 gap-y-1.5 text-sm">
      {Object.entries(obj).map(([k, v]) => (
        <Fragment key={k}>
          <dt className="text-muted-foreground">{k}</dt>
          <dd className="font-mono text-[13px] break-all">{String(v)}</dd>
        </Fragment>
      ))}
    </dl>
```

---

### FE-02b-06 · OVERFLOW · **P3** — `PipelineFunnelCard`: celda de valor fija `64px` con cifras de millones
**Archivo:** `frontend/app/operations/components/PipelineFunnelCard.tsx:160` (valor `:177-179`).
```tsx
            <div key={stage.label} className="grid grid-cols-[160px_1fr_64px] items-center gap-3">
      ...
              <div className={`font-mono text-sm tabular-nums text-right ${tone.text}`}>
                {stage.value.toLocaleString("en-US")}
              </div>
```
El track fijo de 64px aloja `toLocaleString` mono text-sm: `"1,257,000"` (9 chars ≈ 70-75px) ya
no cabe — y las cuotas S1..S9 que este card renderiza (BR-01) son precisamente conteos diarios
de `opportunities` (60,302/24h en el snapshot del peer; la RDO history llegó a 125.7M). El
`1fr` central está bien defendido (`overflow-hidden` + `truncate` interno) — el fallo es el
track numérico.
**Repro:** cualquier stage con value ≥ 8 dígitos (1,257,000+ en la era post-migración de
retención) → el número se clipa/overflow del track 64px.
**Diff (D-06):**
```tsx
// ANTES
            <div key={stage.label} className="grid grid-cols-[160px_1fr_64px] items-center gap-3">
// DESPUÉS
            {/* WO-FE-02b (2026-09-07): lección #514-#518 — cifra mono toLocaleString:
                64px no cubre 8+ dígitos; 5rem alinea y respira sin romper la fila. */}
            <div key={stage.label} className="grid grid-cols-[160px_1fr_5rem] items-center gap-3">
```

---

### FE-02b-07 · OVERFLOW (primitiva, latent) · **P3** — `CardHeader` aún usa `1fr` plano (raíz de la familia #514-#518)
**Archivo:** `frontend/components/ui/card.tsx:22`.
```tsx
        "@container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 px-6 has-[data-slot=card-action]:grid-cols-[1fr_auto] [.border-b]:pb-6",
```
`alert.tsx` se curó en la raíz (`minmax(0,1fr)`, pineado por
`SurfaceControlNames.test.tsx:154-155`); `card.tsx` conserva el `1fr` del mismo árbol de
causa. Hoy es latent puro: **0 consumidores de `CardAction`** (grep — sólo lo define
`ui/card.tsx`), así que el track con `1fr` ni se materializa. Duro por raíz, no por síntoma.
**Repro (latent):** cualquier card futura con `CardAction` + `CardTitle` largo inquebrable →
overflow del header.
**Diff (D-07):**
```tsx
// ANTES (ui/card.tsx:22)
        "@container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 px-6 has-[data-slot=card-action]:grid-cols-[1fr_auto] [.border-b]:pb-6",
// DESPUÉS
        // WO-FE-02b (2026-09-07): misma cura raíz que alert.tsx (#514-#518):
        // el track del título no puede expandirse más allá del contenedor.
        "@container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 px-6 has-[data-slot=card-action]:grid-cols-[minmax(0,1fr)_auto] [.border-b]:pb-6",
```
+ pin de test espejo del de alert (en `SurfaceControlNames.test.tsx` o suite de primitivas):
```tsx
  // WO-FE-02b (2026-09-07): pin — CardHeader jamás vuelve a 1fr plano.
  expect(cardSrc).toContain("has-[data-slot=card-action]:grid-cols-[minmax(0,1fr)_auto]");
```

---

### FE-02b-08 · A11Y · **P3** — Botón icon-only "Remove this RPC" con nombre accesible sólo por `title`
**Archivo:** `frontend/features/onboarding/Phase2Client.tsx:216-222`.
`title="Remove this RPC"` ES un nombre accesible válido (cómputo accname), pero es el fallback
más débil (no lo anuncian todos los AT modos táctiles, y desaparece visualmente); el
`TrashIcon` además no está `aria-hidden`. Los otros 2 botones icon del repo hacen lo correcto
(`theme-toggle.tsx:63`, `MetricsStream.tsx:274` con `aria-label`).
**Repro:** lector de pantalla sobre /onboarding/2-connect → el botón se anuncia como
"Remove this RPC, button" en desktop NVDA pero sin nombre en VoiceOver+táctil.
**Diff (D-08):**
```tsx
// ANTES
              <Button
                type="button" variant="ghost" size="icon" className="mb-0.5"
                onClick={() => removeRpc(i)} disabled={rpcs.length <= 1}
                title="Remove this RPC"
              >
                <TrashIcon className="size-4" />
              </Button>
// DESPUÉS
              <Button
                type="button" variant="ghost" size="icon" className="mb-0.5"
                onClick={() => removeRpc(i)} disabled={rpcs.length <= 1}
                title="Remove this RPC"
                aria-label="Remove this RPC" // WO-FE-02b (2026-09-07): nombre accesible real
              >
                <TrashIcon className="size-4" aria-hidden="true" />
              </Button>
```

---

### FE-02b-09 · MEMORIA (advisory) · **P3** — `LastUpdated`: tick 1s por instancia sigue vivo en background tabs
**Archivo:** `frontend/components/last-updated.tsx:18-22`.
Cada instancia corre `setInterval(force, 1000)` → 1 re-render/s. **Censo de consumidores:
4, todos a nivel panel** (`SourceMeta.tsx:41`, `GoNoGoPanel.tsx:96`, `GoNoGoSignOffCard.tsx:134`,
`BlockersPanel.tsx:80`) → hoy acotado (máx ~4 renders/s por página), **sin leak** (cleanup OK).
El riesgo es estructural: el precedente 3ac27560 (ticker 1s × 200 cards) resucita si alguien
lo consume por-fila. Además el tick sigue corriendo con la pestaña oculta (los navegadores lo
throtlean a ≥1/min, pero no lo matan).
**Diff (D-09, opcional — barato y cerrado):**
```tsx
// ANTES
  useEffect(() => {
    const id = setInterval(() => force((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, []);
// DESPUÉS
  useEffect(() => {
    // WO-FE-02b (2026-09-07): un panel oculto no re-renderiza — el check es perezoso
    // (sin listener extra). Advisory: consumidores actuales = 4/panel, ninguno por fila.
    const id = setInterval(() => {
      if (document.visibilityState !== "hidden") force((n) => n + 1);
    }, 1000);
    return () => clearInterval(id);
  }, []);
```

---

### FE-02b-10 · INFO — Dos componentes muertos con minas (mencionar, NO borrar — §3 quirúrgico)
1. `frontend/components/geometric-background.tsx` — **0 consumidores** (el vivo es `HeroSphere`,
   montado en `layout.tsx:100`). Su cleanup existe; su canvas no tiene `aria-hidden` (el div de
   HeroSphere sí, `HeroSphere.tsx:147`). Mina: montarlo tal cual hereda el aria-hidden faltante.
2. `frontend/components/GateBanner.tsx:95-113` — **0 consumidores**. Mina doble: si se monta sin
   `metrics`/`edgeUrl` ausentes, corre un `setInterval(async …, 3000)` cuyo resultado va a
   `console.log` y **jamás a estado** (`:110`: *"In production, store in state management
   instead of useState"* — TODO en producción potencial), y `polling` queda `true` para siempre.
   Se reporta; la eliminación/adecuación es decisión del operador (P-∅).

## 3. Lo que está BIEN (para que la mesa no re-audite — FE-03/FE-04 heredan esto gratis)

Verificado con evidencia, no por fama:
- **Hidratación R1 disciplina extendida:** `now` nace `0` (`OpportunitiesClient.tsx:88`),
  `lastRefresh` nace del `serverTime` del snapshot (`:91-93` — determinista server-side),
  `WebSocketIndicator` pinta neutro pre-mount (`:48-49`), `app-sidebar` gated con try/catch
  localStorage (`:21-44`), `live-ticker`/`OpportunityTicker` gate `mounted` con items `[]`,
  `StatCard` anima desde 0 post-mount, `drift/page.tsx` span-only documentado.
- **Precedente 3ac27560 vivo y correcto:** ticker 30s (`OpportunitiesClient.tsx:282-287`) +
  `React.memo` con comparator por-edad-segundos que además trata `detected_at:null` con
  `Object.is(NaN,NaN)` y compara `route_metadata` por JSON
  (`OpportunityTradeCard.tsx:539-584`). El exchange replica la disciplina
  (`OpportunitiesExchangeClient.tsx:212-221`).
- **Censo de intervals 28/28 con cleanup** (spot-check profundo de 14: OpportunitiesClient,
  exchange, RuntimePostureBar `:403-409` con `alive`, ArbxRealtimeProvider `:157-170` —
  limpia 2 timers + `socket.disconnect()` + marca 4 canales disconnected, HeroSphere
  `:139-144` cancelAF+removeEventListener+observer.disconnect, ServiceStatus 10s, live-ticker,
  OpportunityTicker, ControlBoardClient, last-updated, ArchivePanel, ByStrategy, GateBanner,
  ChainsAdmin, AllocatorClient `:67-70` `unsub()+client.close()`).
- **A11y estructural sólidо:** skip-link (`layout.tsx:96-104`) con target real
  (`<main id="main" tabIndex={-1} className="min-w-0 …">`, `layout.tsx:117`); los 3 dialogs del
  charter son Radix Sheet con `SheetTitle`+`SheetDescription`+close `sr-only`
  (`ui/sheet.tsx:55-58`) → focus-trap/Escape/aria-modal nativos; `OpportunityTicker` expone
  resumen `sr-only` + track `aria-hidden` (`:137-145`); sidebar con aria-labels y
  `aria-label={collapsed ? item.label : undefined}` (`app-sidebar.tsx:93,120,164`);
  `RuntimePostureBar` `role="status"` + errores R8 verbatim (`:411-416`); `DataTable`
  `role="grid"` + `aria-sort` + `aria-rowcount` (`features/table/DataTable.tsx:120-196`);
  `FocusOnMount` para alerts; `role="alert"` adoptado en 19 archivos.
- **Overflow ya curado donde el histórico mordió:** `ui/alert.tsx:7` `minmax(0,1fr)` pineado
  por test (`SurfaceControlNames.test.tsx:154-155`); paper-history `overflow-x-auto` (`:304`);
  executions truncate (`:77`); live-readiness rows con `min-w-0` (`page.tsx:84`).

## 4. Invariantes (INV-FE02B-*)

- **INV-FE02B-1 (R1 letra):** en `frontend/{app,components,features}`,
  `suppressHydrationWarning` sólo puede aparecer en: `<html>` del layout, o `<span>`
  individuales con comentario de justificación (patrón `drift/page.tsx:21-26`). 0 en `<p>`,
  `<div>`, `<dd>`, `<Button>` u otro contenedor.
- **INV-FE02B-2 (fail-honest fetch):** ningún estado `loading` de UI puede ser terminal: todo
  `Promise.all`/fetch en un flujo con spinner DEBE cubrir rejection (`allSettled`/`catch`) y
  desembocar en el error visible (`EndpointNotice`/`role="alert"`). "Fetching…" infinito =
  violación.
- **INV-FE02B-3 (RULE 00 render):** ningún porcentaje/valor económico se renderiza si no
  viene del payload; el fallback es `null` → `"—"`. Prohibido derivar (escalar, aproximar)
  un valor monetario en el cliente. (Extensión FE-02a: ellos custodian la honestidad del
  stream; esto custodia la honestidad del render.)
- **INV-FE02B-4 (overflow tracks):** todo track `1fr` que aloja contenido potencialmente
  inquebrable (mono/URL/hash) usa `minmax(0,1fr)` o hijo con `min-w-0`/`break-*`/
  `overflow-hidden`. Los tracks fijos que alojan `toLocaleString` se dimensionan para la cifra
  máxima plausible (≥8 dígitos), no para la actual.
- **INV-FE02B-5 (memoria):** todo `setInterval`/`addEventListener`/`subscribe` vive en
  `useEffect` con cleanup `return`. (Hoy 28/28 — el invariante lo congela.)

## 5. Gate de verificación (re-ejecutable por el orquestador/FE-05)

**Baseline del disco auditado (branch `a6-cbprom-01`, 2026-09-07 ~09:55, corrido por mí):**
`tsc --noEmit` EXIT 0 · `vitest run` **120 files / 1,114 tests / 0 fails** (33.7s). *(El peer
CB-VERIFY midió 1,098 a las 08:05 en `feat/hops-live-01` — el disco avanzó con
`websocket-client.test.ts` + tests de RuntimePostureBar no-commiteados del peer FE-02a; mi
cifra es la del árbol actual.)*

Tras aplicar D-01..D-08 (D-09 opcional), todo debe seguir en verde:
1. **INV-1 grep** (PowerShell, repo root — recursivo; matchea la FORMA-DE-ATRIBUTO
   `suppressHydrationWarning` seguida de `>`/EOL/`data-`/`className`/`aria-`, lo que excluye
   de raíz las ~5 menciones en comentarios; excluye spans legítimos y el `<html>` del layout;
   ejecutado por mí contra el árbol actual):
   ```powershell
   Get-ChildItem frontend\app,frontend\components,frontend\features -Recurse -Filter *.tsx |
     Where-Object { $_.FullName -notmatch 'backup|__tests__' } |
     Select-String -Pattern 'suppressHydrationWarning(?=\s*(>|$|data-|className|aria-|\{))' |
     Where-Object { $_.Line -notmatch '<span' -and $_.Filename -ne 'layout.tsx' } |
     ForEach-Object { "$($_.Filename):$($_.LineNumber)" }
   ```
   → debe devolver **0 líneas** tras aplicar D-04a..e. Hoy devuelve **exactamente las 7** de
   FE-02b-04 (verificado: OpportunitiesClient.tsx:301 · OpportunityTradeCard.tsx:296 ·
   SiweAuthPanel.tsx:135 · theme-toggle.tsx:64 · WalletStatusCard.tsx:58,64,76).
2. `cd frontend && npx tsc --noEmit` → EXIT 0 (D-01 cambia el tipo `yield: number|null`).
3. `npx vitest run` → 120+/1,114+/0 fails **+ tests nuevos**:
   - `OpportunityTicker.test.tsx`: ítem con `roi_pct:null` + profit>0 → render contiene `"—"`
     y NO contiene `"%"` (hoy renderiza `+0.42%` fabricado).
   - `WalletDetailDialog`: mock de `getWalletBalances` que **reject** → tras el efecto,
     `loading===false` y aparece el `EndpointNotice` con el mensaje (hoy: spinner eterno).
4. **Browser (FE-05 orquestador, Chromium headless):**
   - `/opportunities`: 0 errores console con código `418|423|425`.
   - `/wallets` → abrir detalle con edge offline (devtools) → EndpointNotice visible <2s, sin
     spinners colgados.
   - `/config`: `document.documentElement.scrollWidth <= window.innerWidth` en 1280px y 320px.
   - axe-core (`@axe-core/playwright`) en `/`, `/opportunities`, `/wallets`,
     `/onboarding/2-connect` → 0 violaciones "serious"/"critical".
5. **Sin NS deltas:** los diffs son presentacionales (client-only); no tocan endpoints, WS ni
   stores — sin riesgo para los invariantes XLEN/PG de los pares.

## 6. Clasificación de fuentes de este reporte
- CANONICAL_REPO: todos los file:line citados (leídos del árbol actual).
- CANONICAL_WORKBOOK: precedentes #514-#518 (alert.tsx minmax) y 3ac27560 (ticker 30s+memo)
  tal como viven pineados en tests/comentarios del repo.
- INFERRED (marcado en el texto): severidades "latent" de FE-02b-04/07 (sin síntoma hoy — el
  gating las hace deterministas; lo verifiqué leyendo el gating, no ejecutando hidratación).
- No medido en browser en esta pasada (presupuesto dominio público ≤5 HTTP): los gates 4 son
  del orquestador FE-05. PENDIENTE R8 §1 para las 30+ páginas restantes.
