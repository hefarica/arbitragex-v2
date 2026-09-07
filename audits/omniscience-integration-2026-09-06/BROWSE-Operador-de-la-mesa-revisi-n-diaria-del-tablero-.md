# BROWSER-OPERADOR — Revisión diaria del tablero (Gang Omniscience)

- **Persona:** Operador de la mesa (humano de la DApp, journey propio, sin barridos).
- **Ventana observada:** 2026-09-07 00:36–00:47 local (05:36–05:46 UTC) — la sesión del gang sigue fechada 2026-09-06.
- **Dominio:** https://arbx.ape-tv.net — corre **main 9ac06d2d** (premisa del WO). Los fixes WO-01/WO-08 son LOCALES y NO desplegados (evidencia: `frontend/lib/websocket-client.ts:36,150` y `frontend/components/RuntimePostureBar.tsx:93` con marcadores `WO-01/WO-08 (2026-09-06)` solo en working tree). Nota: origin/main ya avanzó a 931ad736 (merge #549) — el dominio quedó en 9ac06d2d; sin acción de deploy de mi parte (NO-GIT).
- **Presupuesto consumido:** 5 cargas de página (1 opportunities, 1 live-readiness, 2 operations — una causada por robo de pestaña, 1 tab nueva), **1 curl manual** (`/api/health`, permitido). Cero refrescos en ráfaga; esperas pasivas de 8s/10s/65s para observación humana.
- **Incidente ambiental (no es defecto de la DApp):** el navegador Playwright MCP es compartido entre agentes del gang; a las 05:38Z otro agente navegó MI pestaña de `/opportunities` a `/operations` (cambié a pestaña propia para el resto del journey). Documentado para el tablero.

---

## Panel 1 — `/opportunities` → VEREDICTO: OK con reserva (etiquetado WS)

**¿Datos reales con timestamps que avanzan? SÍ.**
- Estado inicial (05:36:18Z): contador **"0 viable / 108 total (108 rejected)"**; cards con timestamps de reloj de pared: `00:35:07 · 69s` (badge `⚠ STALE`), `00:36:30 · 0s` y `00:36:31 · 0s` (badge `✓ VIGENTE`).
- Evolución sin refrescar: a 05:37:44Z el contador marcaba **"0 viable / 134 total (134 rejected)"** — +26 oportunidades en ~86s. El feed AVANZA con datos reales (RULE 00: nada parece fabricado; son las detecciones reales del searcher).
- Ribbon superior (RuntimePostureBar): `KILL SWITCH OFF · PAPER MODE ON · socket CONNECTING · routes LIVE · runtime_ack LIVE · pairs CONNECTING · quote_anchor CONNECTING`.

**¿Reject-breakdown refleja rechazos reales? SÍ (Audit Trail §31 al pie del feed).**
- Título exacto: `"Audit Trail · Invalid / Quarantined Events (§31)"`. Filas con timestamp UTC al segundo: `2026-09-07T05:36:33.226605+00:00`, razón `single_pool_no_spread`, ruta `UniswapV3 → UniswapV3 (2 hops)`, col. Block `—`, col. Errores (§30) `missing_block`. Demás filas: `TokenNotAllowed:0x32353a6c91143bfd6c7d363b546e62a9a2489a20` sobre estrategias `mev_01_007/032/034/036`, `mev_02_006`, `mev_01_008` — el flood XEN/AGLD ya conocido, mostrado verbatim.
- Cards: badges `REJECTED` con tooltip de razón exacta (`TokenNotAllowed:0x…`), ROI `—` con aria-label `"Net Convergence Ratio (ROI %) — fail-honest '—' when not computed"` — R8 honesto en el propio DOM.

**Reserva:** los chips `socket`, `pairs`, `quote_anchor` permanecieron **CONNECTING los ~10 min completos del journey** (tooltip: `"mounted, awaiting the first accepted payload — routes=DEGRADED, pairs=CONNECTING, quote_anchor=CONNECTING"`) mientras el feed sí avanzaba. Con 100% de rechazos nunca llega un "accepted payload", así que el agregado nunca sube de CONNECTING: etiquetado engañoso del estado real (los datos fluyen vía tick/REST). Esto es exactamente el comportamiento PRE-fix que WO-08 (agregado R8 del chip socket) y WO-01 (payload `new_opportunity`) corrigen en local. Además `routes` se mostró **LIVE en /operations** pero **DEGRADED en /live-readiness** (inconsistencia entre páginas, misma sesión).

## Panel 2 — `/live-readiness` → VEREDICTO: OK (gates veraces, datos frescos)

**¿El panel de gates muestra su estado? SÍ — 19/19 VERIFIED, 0 NOT STARTED.**
- "LIVE READINESS TRACKER · Secuencia de Ignición Operativa · PROGRESO 4/4" (PASO 1–4 COMPLETADO). Banner: `LIVE HABILITABLE · PAPER_MODE · READY TO FLIP · VERIFIED 07/09/2026, 12:40:31 A. M. · SOURCE: API-SERVER · UPDATED 23S AGO` — dato verificado hace segundos (no stale).
- Detalle por paso (verbatim): `4 active WSS + 5 active RPC provider(s) in topology vault (version 1786916896)`; `Authorized signer/key present server-side (redacted)`; `Market topology registered: 6 chain(s), 23 dex(es), 1023 pool(s), 4097 token(s)`; `5 resolution engine(s) enabled: dex_arb, dex_arb_v2v2, flashloan_arb, liquidation, triangular. paper=true shadow=true, live disabled`.
- Secciones: SECURITY & COMPLIANCE 4/4 (V-NH-1, V-DB-1 con `109 migrations scanned, 0 password literals`, V-AT-1, PR-1) · AUDIT TRAIL 1/1 (PR-2: `27 audit rows in last 7d`) · RISK DOCTRINES 5/5 (G-RPC-1 `17 providers, 12 alive`; G-SIM-1 `37406 simulations in last 24h`; G-NET-1/G-PEC-1/G-RIS-1) · TOKENS & STRATEGIES 2/2 (G-TOK-1 `1729 token_safety_cache rows`; G-PAP-1 `439118 detections in last 7d, last 0.0h ago`) · SMART CONTRACTS 1/1 · OPERATIONS 6/6 (G-DISK-1 `disk usage 73.9% — 39.1 GB free`; G-PIPE-1 backlog 0; ALERTS `29 alert rules loaded`).

**¿Existe la tarjeta go/no-go sign-off A.9? SÍ.** `[data-slot="go-no-go-signoff-card"]` presente (verificado por querySelector). Texto clave copiado EXACTO:
> `A.9 formal sign-off — AWAITING FIRST SIGN-OFF`
> `Recorded ledger state from /api/go-no-go/status. Sign-off happens via admin API only — there is no sign button here.`
> `GO_LIVE_ELIGIBLE: NO · UNRESOLVED_BLOCKERS: 0 · PAPER_SAFE: YES`
> `LEDGER GENERATION hash: 5189814fd2e9… generated_at: 2026-09-07T00:20:06.186Z`
> `SIGN-OFFS RECORDED (0) — No operator has signed this ledger generation.`

Antigüedad del dato: la **generación del ledger es de 00:20:06Z (~20 min antes de mi visita)**; el panel refresca (`UPDATED 19S AGO`) y el botón es `Regenerate ledger` (no existe botón de firma — coherente con §34.3, quórum por admin API con 2 operadores; el runbook con curl de ejemplo es visible en la tarjeta).

**Coherencia del veredicto global:** panel "GO / NO-GO decision" (UPDATED 13S AGO): `A.4 FORK VALIDATION: GO · A.5 PAPER-SHADOW: GO · LIVE TRADING: NO-GO · CAPITAL $0 · PAPER MODE ON`. TOP REASONS (CRITICAL): `A.9 GO/NO-GO formal sign-off pending`. Readiness blockers: `1 critical · 2 high — blocks: LIVE` → A.9 `PENDING`, A.6 `PARTIAL (Prometheus emission pending)`, A.7 `PARTIAL (module shipped, runtime call-site pending)` — coincide con la memoria del programa. Agent Teams: 17 agentes, 16 PASS / 2 PARTIAL / 1 NO-GO (`go-no-go-agent NO_GO` WS-VERIFIED LIVE) — todo alineado, sin falsos verdes.
- R8 ejemplar en Risk Circuit Breakers (A.6): `OVERALL: WARN · 7 PASS · 1 WARN · 2 NOT_AVAILABLE` con la nota `"Missing data sources render NOT_AVAILABLE — never fabricated PASS"`.

**Tensiones observadas (documentadas, no accionadas):**
1. El copy dice `"This button is disabled until all 17 readiness items report verified"` pero el tracker muestra **19/19** — copy desactualizado (17 vs 19) Y el botón `ACTIVATE LIVE MODE` quedó **habilitado en el DOM** (`disabled=false`) precisamente porque los 19/19 están verdes. La doctrina protege en el terminus (relays-client default-deny, §34.3) y el panel GO/NO-GO dice "no UI control to flip Live to ON", pero un botón azul habilitado con ese texto es un mensaje contradictorio para el operador. NO lo presioné.
2. Fork Validation: `HEALTHY · Block # 25.923.317 · RPC 3ms · Sims Today 0 · Fork age 0m · Updated 12:41:05` — "Sims Today 0" es un cero computado (R8 correcto), pero convive con G-SIM-1 VERIFIED (`37406 simulations in last 24h`) y con la tarjeta `G-SIM-1 (Simulator V2)` en **RED** esperando "Run Sepolia Smoke Test" manual. Tres displays del mismo dominio con tres señales distintas — legible pero confuso.
3. Paper Shadow: `Accumulated +7979717933706.62 USD · Trades: 598878` — dato real del ledger paper, pero la magnitud (~$7.98T) es implausible como USD; huele a unidades crudas sin normalizar (presentación, no fabricación).

## Panel 3 — `/operations` → VEREDICTO: GAP (Archivo Frío cargando eterno; resto real)

**Postura de runtime:** ribbon ídem (`socket CONNECTING`, `routes LIVE`, `runtime_ack LIVE`, `pairs/quote_anchor CONNECTING`) + System Guard (`Live OFF · Paper ON · Capital $0 · Readiness 4/4 · GO live NO-GO · A.4 PASS · A.5 PASS · PG ok · Redis ok · HB fresh · Engine loaded · Flip open`). Panel "Execution modes": `LIVE_MAINNET default-deny: MainnetRefused (§34.3) · TESTNET broadcast gated · PAPER_SHADOW ACTIVE` — doctrina visible e intacta.

**Convergence Metrics (KPIs paper):** `CPI 0.0000 · SPI 0.00 · EAC $0.00 · ETC $0.00 · TCPI 5.0000 · VAC $5000.00 · CV $-1189.41 · Ops completed today 0 (target 100/day)` — ceros computados, no mudos.

**Scanner Pipeline Funnel (last 60s, "0s ago"):** `438 pending → 4 decoded OK (0.9%) → 0 enriched → 0 passed gates → 0 persisted · Rejected by gates: TokenNotAllowed:0 · UnknownPrice:0 · AnomalousMath:0 · Other:0 · REDIS STREAM Δ -2 · PG INSERTED (PERIOD) 3 · PG YIELD > 0: 0`. Funnel honesto con caída real decodificación (4/438).

**Route Discovery Funnel (§46):** tick `233 → 64 → — → — → 500 → 200` + ventanas 24h `outcomes 30.933.140 · opportunities 59.672 · Reconciled 0`, con nota R8 explícita en el panel: `"Contadores del tick y ventanas 24h NO comparten escala — cada valor es verbatim de su wire y su ventana está etiquetada. Nulo ⇒ ausencia real (R8), jamás un cero."` — R8 textual y correcto.

**Latencia (10_LATENCY):** `FAIL_p95 — lat.total over SLA · cycles: 142`; p95 peor: `lat.pair 443.60ms (target 3, headroom -440.60)`, `lat.total 764.45ms (target 29, headroom -735.45)`, `lat.refine — — — (not computed, R8 null)`. FAIL declarado sin maquillaje.

**Archivo Frío (GAP principal):** a los 65s+ (observado ~2.5 min en página) el panel SIGUE mostrando:
> `"Cargando estado de archivo…"`
> `"edge HTTP 401: {\"error\":\"missing_admin_token\"}"`
> `"Capacidad del volumen de archivo — libres de — · —% usado" · "total —" · "Sin archivos aún."`

Consola: `Failed to load resource: 401 @ https://arbx.ape-tv.net/api/admin/archive/status` repetido **cada ~30s** (a los 0.8s, 31s, 61s, 91s, 122s de vida de la página — 5 repeticiones capturadas en log). El endpoint requiere sesión admin (httpOnly cookie / x-arbx-admin-token); un operador sin sesión admin nunca cargará la tabla. La RAZÓN se declara (línea 401 visible) — no es un vacío mudo — pero el estado-label `"Cargando estado de archivo…"` es mentiroso: no está cargando, falló con 401 y reintenta en loop sin transicionar a estado de error/bloqueo. Capacidad `"— libres de — · —% usado"` sí es un vacío mudo de guiones pegado al error.

**Rechazos por razón (reject-breakdown real):** `"100% rechazadas · 61,011 rechazadas de 61,011 oportunidades · 24 h"` (evolucionó de 61,131 → 61,011 en ~2 min — ventana deslizante viva). Familias: `token_not_allowed 44,520 72.8% ($1.99/$1.54) · missing_reserves_pool_b 11,253 18.4% · non_positive_profit 2,668 4.4% (gross prom $2594690 — magnitud sin formato) · v3_quote_unavailable 1,056 · gas_floor_breach 504 · missing_route_legs 475 · unknown_token_price 280 · single_pool_no_spread 199 · spot_product_le_one 130 · strategy_disabled 40 · spread_zero_equilibrium 6`. Inundación por token: `XEN 0x06450d…9a6fb8 = 22,708 · AGLD 0x32353a…489a20 = 21,812` — confirma el flood #449 con datos agrupados reales. Gross prom. `$2594690` para non_positive_profit: sin separadores y magnitud dudosa (presentación).

**Ledger paper:** en /operations no hay panel "paper ledger" per se; lo más cercano es la S-Curve `cumulative PnL vs target (24h)` y el ticker `Live opportunity feed` (eventos `0x0645…/0x0645… unknown → unknown +0.30% · 25s` — frescos, 25–26s de edad). El ledger paper vivo vive en /live-readiness (Paper Shadow card, ver Panel 2).

**/health público (1 curl permitido):** `200 {"ok":true,"service":"api-server","version":"0.1.0","uptime_s":765}` — api-server con **solo 12.75 min de uptime** (reinició ~05:32Z, 4 min antes de mi journey): contexto plausible para los canales WS aún sin payload aceptado; no proof, pero anotado.

---

## Copia EXACTA de estados vacíos / pendientes (R8)

| # | Texto EXACTO | Panel | ¿Declara razón? | Persistencia |
|---|---|---|---|---|
| 1 | `Cargando estado de archivo…` | /operations · Archivo Frío | No (el label dice "cargando" pero NO está cargando) | >2.5 min (observado), loop 30s |
| 2 | `edge HTTP 401: {"error":"missing_admin_token"}` | /operations · Archivo Frío | **SÍ** — razón exacta expuesta junto al panel | repetido cada 30s |
| 3 | `— libres de — · —% usado` | /operations · capacidad archivo | No — guiones mudos | ídem panel |
| 4 | `total —` / `Sin archivos aún.` | /operations · archivos existentes | Parcial ("sin archivos" es válido; "total —" mudo) | ídem panel |
| 5 | `No operator has signed this ledger generation.` | /live-readiness · A.9 | **SÍ** — estado ledger real (0 sign-offs) | estable |
| 6 | `Sims Today 0` | /live-readiness · Fork Validation | Cero computado (R8 correcto) | fresco (12:41:05) |
| 7 | `lat.refine — — —` (+ "null = not computed (R8)") | /operations · latencia | **SÍ** — nota R8 en el propio panel | estable |
| 8 | `Block: —` + `missing_block` (col. Errores §30) | /opportunities · Audit Trail | **SÍ** — el error declara la ausencia | por fila |
| 9 | `ROI —` con aria `"fail-honest '—' when not computed"` | /opportunities · cards | **SÍ** — en el atributo del DOM | por card |
| 10 | `socket CONNECTING … awaiting the first accepted payload` | ribbon (3 páginas) | Parcial — describe causa pero el label CONNECTING eterno es engañoso | ~10 min completos |

## Veredicto global: **GAPS**

La DApp muestra lo que dice mostrar, con datos REALES que avanzan (feed 108→134 en 86s; rechazos 24h deslizando 61,131→61,011; gates verificados hace segundos; A.9 con ledger hash y 0 sign-offs) y R8 mayormente ejemplar (nulos declarados, NOT_AVAILABLE jamás PASS, notas R8 textuales). PERO hay 4 gaps accionables: (1) Archivo Frío en "Cargando…" eterno con loop 401×30s y capacidad muda; (2) chips WS CONNECTING permanentes con datos fluyendo + `routes` inconsistente entre páginas (comportamiento PRE-WO-01/WO-08, ya corregido en local, pendiente de deploy — operador); (3) copy "17 readiness items" vs 19/19 real con botón `ACTIVATE LIVE MODE` habilitado en DOM (tensión doctrinal §34.3); (4) magnitudes sin normalizar ($7.98T acumulado paper; gross $2594690 sin formato).

## Capturas (evidencia)

1. `audits/omniscience-integration-2026-09-06/operador-01-opportunities-inicial-2026-09-07.png` — /opportunities inicial: contador 0/108, cards VIGENTE 0s, badges QUARANTINED missing_block, ribbon CONNECTING.
2. `audits/omniscience-integration-2026-09-06/operador-02-live-readiness-gates-a9-signoff-2026-09-07.png` — /live-readiness fullpage: 19/19 VERIFIED, tracker 4/4, tarjeta A.9 AWAITING FIRST SIGN-OFF, blockers 1 critical/2 high, Agent Teams 16/2/1, breakers WARN.
3. `audits/omniscience-integration-2026-09-06/operador-03-operations-funnel-reject-breakdown-2026-09-07.png` — /operations viewport: KPIs + execution modes + funnel 438→4→0.
4. `audits/omniscience-integration-2026-09-06/operador-04-operations-fullpage-archivo-rechazos-2026-09-07.png` — /operations fullpage: Archivo Frío "Cargando…" + 401, rechazos 61,011 con XEN/AGLD, latencia FAIL_p95.

Log de consola: `.playwright-mcp/console-2026-09-07T05-42-30-519Z.log` (401 ×5 a 0.8/31/61/91/122s).
