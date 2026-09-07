# BROWSER-AUDITOR-R8 — Auditoría de honestidad (caza de datos vacíos o decorativos)

- **Persona:** Auditora externa R8 (humano usuario de la DApp, desconfía de los dashboards)
- **Fecha/Hora del journey:** 2026-09-07, 05:36–05:50 UTC (12:36–12:50 a. m. hora local del navegador, UTC-5)
- **Dominio:** https://arbx.ape-tv.net
- **Paneles recorridos:** `/opportunities` (2 visitas), `/operations`, `/live-readiness`, `/opportunities/exchange`
- **Presupuesto:** 6 navegaciones, 0 requests HTTP manuales (solo journeys del navegador; las cifras se cruzaron contra el DOM y los snapshots de accesibilidad)
- **Nota de método:** el navegador Playwright es COMPARTIDO con otros agentes del gang (apareció una pestaña extra en `/live-readiness` y `/operations`). Toda la evidencia citada abajo se tomó de snapshots/logs cuyo `Page URL` se verificó en el momento; la tormenta 500/502 antigua que aparecía en el historial de consola NO se atribuye a este journey salvo la del episodio 05:47:35–55 UTC, que consta en el log de la pestaña propia (`console-2026-09-07T05-46-48-859Z.log`).

---

## 1. Cruce de cifras entre paneles (coherencia)

| Cifra | Panel A | Panel B | Veredicto |
|---|---|---|---|
| Rechazos 24h por familia vs total declarado | `/operations` "61,011 rechazadas de 61,011 oportunidades · 24 h" con familias 44,408 + 11,253 + 2,668 + 1,054 + 504 + 475 + 280 + 193 + 130 + 40 + 6 | Suma exacta = **61,011** | **COHERENTE** (aritmética cierra al digito) |
| Inundación por token vs familia token_not_allowed | XEN "22,596" (0x06450d…9a6fb8) + AGLD "21,812" (0x32353a…489a20) = **44,408** | familia `token_not_allowed` = **44,408** (72.8%) | **COHERENTE** (cierre exacto; además coincide con los motivos visibles en las tarjetas del feed: `TokenNotAllowed:0x06450dee…` y `TokenNotAllowed:0x32353a6c…`, mismas direcciones) |
| Feed totals vs §31 cuarentena | `/opportunities` 05:36 UTC: "0 viable / 107 total (107 rejected)" y §31 "25 filas + '+82 evento(s) en cuarentena no mostrado(s) — cap 25 por vista'" = 107 | 05:49 UTC: "0 viable / 50 total (50 rejected)" y §31 "25 + '+25 no mostrado(s)'" = 50 | **COHERENTE en ambas visitas** (total = mostrados + ocultos, y el total rueda — ver §3) |
| 100% rechazadas | Feed: 0 viable / 107 (107 rejected) | Breakdown: "100% rechazadas" · 61,011/61,011 (ventana 24h, etiquetada) | **COHERENTE** (ventanas distintas, ambas etiquetadas; el 0 viable no es decorativo: cada tarjeta declara su razón) |
| Cadenas | `/live-readiness` Paso 3: "6 chain(s), 23 dex(es), 1023 pool(s), 4097 token(s)" | Filtro de cadena de `/exchange`: exactamente 6 opciones (ETH, OP, BSC, MATIC, BASE, ARB) | **COHERENTE** |
| Postura de seguridad | Guard en `/opportunities`: Live OFF · relay OFF · submit OFF · broadcast OFF · paper ON · Capital $0 | `/live-readiness`: "Live trading: OFF", "go_live_eligible NO", "paper_safe YES", "Capital $0", breakers "Mode paper_only" | **COHERENTE** (§34.3 intacto: default-deny visible, NO-GO) |
| Menor coerencia | `/live-readiness` ledger A.9: "unresolved_blockers 0" | "Readiness blockers: 1 critical + 2 high · blocks: LIVE" | **TENSIÓN MENOR**: dos endpoints distintos (`/api/go-no-go/status` vs `/api/readiness/blockers`) con semántica de "blocker" diferente; el UI no lo reconcilia. No es mentira (go_live_eligible=NO es lo conservador), pero invita a confusión. |

**Conclusión §1:** no encontré cifras decorativas ni contradicciones duras entre paneles. Los cruces cierran aritméticamente (61,011 y 44,408 exactos).

## 2. Estados vacíos — ¿cada vacío declara SU razón? (R8: None ≠ 0.0)

**Ejemplar.** Ejemplos textuales exactos (todos observados en vivo):

- `/opportunities` tarjeta: `roi_pct no computado (R8)` → "—"; `amount_in_wei=… · USD solo cuando la simulación lo computa (R8)` → "—"; `latencia por-candidato no emitida aún (nivel-(b)) — emisión pendiente ARBX-FE-EMIT-09` → "no emitido"; `detector_id no es columna del feed` → "no emitido".
- `/opportunities/exchange` tarjeta: `Buy px / Sell px —` con tooltip "Per-leg execution prices are not persisted on this row (R8 fail-honest)".
- `/operations` funnel §46: `fe_prefilter_evaluated` → "—" con etiqueta "absent when the knob is OFF (honest gap)"; disclaimer: "Contadores del tick y ventanas 24h NO comparten escala — cada valor es verbatim de su wire y su ventana está etiquetada. Nulo ⇒ ausencia real (R8), jamás un cero."
- `/live-readiness` breakers A.6: "Missing data sources render NOT_AVAILABLE — never fabricated PASS" con tally honesto: PASS 7 · WARN 1 · NOT_AVAILABLE 2 · TOTAL 10, Overall WARN (no verde falso).
- `/live-readiness` A.9: "Sign-offs recorded (0) · No operator has signed this ledger generation." + hash `5189814fd2e9…` + `generated_at 2026-09-07T00:20:06.186Z` — vacío con identidad y razón.
- `/operations` feed widget: "No topological convergence detected — waiting for market topology..." (vacío con causa).
- Ceros COMPUTADOS declarados como tales: `/operations` KPIs "CPI 0.0000 · yield/capital", "Ops completed today 0 · target 100/day" — cero con contexto, no "—" disfrazado.

**Excepciones encontradas (menores):**

1. **Tarjeta `/exchange`:** `Gross out —` (no computado) junto a `Net Yield $0.00` y costos `-$0.00` (Interés/Gas/LP fees/Decoherencia de Estado). Si el gross no fue computado, presentar el Net Topological Yield como `$0.00` calculado mezcla None con 0.0 en la MISMA tarjeta. Riesgo R8 de presentación (el wire probablemente trae net=0.0 computado, pero el lector no puede distinguirlo).
2. **Archivo Frío (`/operations`):** fila "Cargando estado de archivo…" perpetua + "total —" + "Sin archivos aún." cuando el motivo REAL es `edge HTTP 401: {"error":"missing_admin_token"}` (que sí se muestra al lado, y la consola repite `401 /api/admin/archive/status` cada 30s). Decir "Cargando…" cuando ya falló es un estado vacío mal etiquetado (el error es honesto; el label no).

## 3. ¿Hardcode/mock? (cifras congeladas o redondas idénticas)

- **Visita 1 (05:36:40 UTC):** "0 viable / **107** total (107 rejected)"; §31 timestamps `2026-09-07T05:36:31.072265+00:00`…`05:36:33.226605+00:00` (segundos antes del snapshot).
- **Visita 2 (05:49:08 UTC, +13 min):** "0 viable / **50** total (50 rejected)"; §31 timestamps `2026-09-07T05:47:20.035Z`…`05:47:20.079Z`.
- **Veredicto: NO hardcode.** El buffer rueda (107→50), los timestamps avanzan (05:36→05:47), y el encogimiento es consistente con el episodio de degradación 05:47:35–55 (buffer re-llenándose tras la tormenta). `Last refresh: 12:36:18 a. m.` → `12:49:08 a. m.` confirma refresco real.
- Cifras redondas observadas: `routes found 500 / dispatched 200` por tick en el funnel §46 — redondez sospechosa a simple vista, pero plausiblemente son caps/límites por tick del wire; el propio panel advierte que los contadores son verbatim del wire. Lo dejo anotado, no es mock.
- `VAC $5000.00` = target − forecast con EAC $0.00 → aritmética internamente consistente (target diario 5000, no decorativo).

## 4. Consola del navegador y red

- **CSP report-only (única warning):** `Loading the image 'https://assets.geckoterminal.com/...' violates "img-src 'self' data: blob: https://raw.githubusercontent.com https://assets.coingecko.com https://coin-images.coingecko.com https://cdn.dexscreener.com". The policy is report-only…` (log de consola de la carga de `/opportunities`, 05:36 UTC). Las imágenes de iconos de token aún cargan desde geckoterminal fuera del allowlist → añadir al CSP o cambiar de fuente.
- **Episodio de degradación real:** entre **05:47:35 y 05:47:55 UTC** el edge devolvió **500/502 en TODOS los endpoints** sondeados por la página (`/api/opportunities/live`, `/api/readiness`, `/api/readiness/decision`, `/api/readiness/steps`, `/api/paper-mode/state`, `/api/scanner/heartbeat`, `/api/strategies/runtime-status`, `/api/quote/anchor`, `/api/route-discovery/tick`, `/api/prices/live`, `/api/pairs`, `/api/status`, `/socket.io/`) + fallos de handshake WebSocket `502`. La página sobrevivió honesta: en la revisita de 05:49 la barra de postura declaraba `socket CONNECTING` (no fake-LIVE) y las tarjetas mostraban edad real (136s) con bandera "stale". También en la carga inicial de 05:36 el UI reportó "Edge connection error · Loading..." y `socket DISCONNECTED` antes de recuperar — fail-honest bajo falla.
- **401 admin esperado:** `GET /api/admin/archive/status → 401` cada 30s (panel Archivo Frío sin token admin); el error ES visible en el panel.
- **Ruido benigno:** ráfaga de `token-icon … net::ERR_ABORTED` (dedupe del browser en iconos repetidos de AGLD; los anteriores devolvieron 200).

## 5. Hallazgo de seguridad-UI (NO se cliqueó, §32/§33 read-only)

En `/live-readiness`, sección "Activate live mode": el botón `activate live mode` está **habilitado** (`disabled: false`, `aria-disabled: "false"`, verificado por evaluate a las 05:45 UTC) mientras la propia sección dice *"This button is disabled until all 17 readiness items report verified. The honesty doctrine forbids overrides while items are red, yellow, or pending"* y el estado global es `GO live: NO-GO` con blocker crítico `A.9 GO/NO-GO formal sign-off pending` (pending) + 1 ítem BLOCKED en breakers. La página muestra 19/19 verified (≥17), así que el botón cumple su condición literal, pero la coexistencia "botón clicable + NO-GO + pending" es un affordance contradictorio para el operador. La red de fondo sigue protegida (`go_live_eligible NO`, relays default-deny, §34.3), por lo que lo clasifico como **inconsistencia de UI, no riesgo de flip**. No lo cliqueé.

Otra observación: `LIVE lock: LOCKED` (05:36, durante carga del runtime, readiness aún 0/4) → `LIVE lock: REVIEW` (05:42, con 4/4). Atribuible a hidratación progresiva del guard; anotado por trazabilidad.

## 6. Veredictos por panel

| Panel | Veredicto | Base |
|---|---|---|
| `/opportunities` | **HONESTO (FAIL-HONEST)** | 0 viable con razón en cada tarjeta; §31 = total exacto; datos ruedan entre visitas; "—" con causa R8 por campo |
| `/operations` | **HONESTO (FAIL-HONEST) con 1 gap** | Breakdown 61,011 cierra exacto; funnel con gaps declarados; KPIs cero con contexto; gap: "Cargando…" perpetuo en Archivo Frío cuando la causa es 401 |
| `/live-readiness` | **HONESTO (FAIL-HONEST) con 1 gap de affordance** | NOT_AVAILABLE jamás falso PASS; A.9 con hash/0 sign-offs; NO-GO coherente; gap: botón activate-live habilitado con A.9 pending |
| `/opportunities/exchange` | **HONESTO (FAIL-HONEST) con 1 gap** | FEED/PRICES LIVE con "—" para tokens sin precio; timestamps frescos (23s); tooltips R8; gap: Net Yield $0.00 junto a Gross "—" |

**Veredicto global: FAIL-HONEST (la DApp no miente).** Los vacíos declaran su razón, los ceros son computados y etiquetados, las cifras cruzadas cierran, y los datos rotan entre visitas. Los gaps son de presentación/afordancia e infra (episodios 500/502 del edge), no de fabricación de datos (RULE 00 respetada en lo observado).

## 7. Gaps accionables

1. [agent-fixable] Deshabilitar (o re-etiquetar con estado/gate) el botón "activate live mode" mientras `go_live_eligible=NO` o existan blockers pending (§5).
2. [agent-fixable] En tarjetas `/exchange`: si `gross` es "—", el Net Topological Yield y los costos unitarios deben renderizar "—" o marcar explícitamente "computado = 0.00" (R8 None≠0.0).
3. [agent-fixable] Archivo Frío: reemplazar "Cargando estado de archivo…" por el estado de error real (401 missing_admin_token) cuando el poll falla.
4. [agent-fixable] Añadir `https://assets.geckoterminal.com` a `img-src` del CSP (o migrar la fuente de iconos) — hoy solo report-only.
5. [operator-gated] Investigar en VPS los episodios 500/502 edge→api-server (~05:36 y 05:47:35–55 UTC del 2026-09-07; sospecha: recarga/contención de contenedor o túnel CF durante la ventana de auditoría del gang — varios agentes navegando simultáneamente).

## 8. Evidencia fotográfica

- `audits/omniscience-integration-2026-09-06/r8-opportunities-feed-0viable-107total.png` — visita 1: header LIVE + "0 viable / 107 total (107 rejected)" (05:36 UTC).
- `audits/omniscience-integration-2026-09-06/r8-operations-reject-breakdown-61011-24h.png` — desglose "61,011 rechazadas de 61,011 · 24h" + inundación XEN/AGLD (05:40 UTC).
- `audits/omniscience-integration-2026-09-06/r8-readiness-4of4-nogo-a9-pending.png` — /live-readiness: pasos 4/4 + A.9 "awaiting first sign-off" + GO/NO-GO NO-GO (05:45 UTC).
- `audits/omniscience-integration-2026-09-06/r8-exchange-106-matching-prices-live.png` — MATCHING 106 / TOTAL DETECTED 106 / PRICES LIVE con "—" en XEN/AGLD (05:47 UTC).
- `audits/omniscience-integration-2026-09-06/r8-opportunities-visita2-50total-rotacion.png` — visita 2: "0 viable / 50 total (50 rejected)" — rotación real del buffer (05:49 UTC).

*Lexicón: Topological Yield (net yield), Variedad de Liquidez (pool/DEX), TLS (flash loan), Decoherencia de Estado (slippage), Holonomic Loop Resolution (loops 2-hop XEN→WETH→XEN observados en el feed).*
