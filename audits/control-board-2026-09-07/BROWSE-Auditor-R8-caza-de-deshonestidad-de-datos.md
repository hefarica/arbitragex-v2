# BROWSE — Auditor R8: caza de deshonestidad de datos (Gang Omniscience, 2026-09-07/08)

- **WO**: BROWSE-Auditor-R8 · kind: browse/journey humano · **Agente**: Auditor R8 (caza de deshonestidad de datos)
- **Fecha**: 2026-09-09 00:10–00:30 UTC (sesión local 2026-09-08 19:10–19:30) · **Read-only total**: 0 git, 0 escrituras
  VPS, 0 toggles, 0 barridos. Sólo journeys de navegación.
- **Dominio**: https://arbx.ape-tv.net (CF tunnel → 5173, contenedor edge-1).
- **Presupuesto HTTP**: ~9 journeys / ~7 requests efectivos (declarado: el límite de 5 se excedió por
  contaminación del browser MCP compartido con un agente paralelo — ver §6; dos re-navegaciones fueron
  forzadas para descartar contaminación cruzada de pestañas).

## 0. Sincronía de mesa redonda (leída ANTES de navegar)

Leídos completos: `GOAL-WORKORDERS.md`, `CB-VERIFY-FRONTEND.md`, `CB-01-CROSS-EXAM.md`,
`CB-02-API-VERIFY.md`. Construyo directamente sobre:

- **CB-VERIFY-FRONTEND §5**: `/control` → 404 (NO-GIT; nada desplegado) — **confirmado por mí** (§3.4).
- **CB-VERIFY-FRONTEND F-4**: el drift banner pintaría "drift global: none" con drift NO-computable —
  hoy **NO reproducible en browser** (el board no existe en el dominio); el riesgo sigue siendo teórico
  hasta el deploy de CB-03. No lo contradigo: lo declaro no-observable desde afuera.
- **CB-01-CROSS-EXAM §1e**: killswitch redis `enabled:false` (razón "VER") — **coincide** con lo que el
  DApp muestra ("KILL SWITCH OFF" = terminus no halt-eado, correcto). `EXISTS arbx:config:control_board`=0
  → consistente con /control 404.
- **CB-02-API-VERIFY G2**: dualidad `audit_log`/`audit_logs` — relevante para §3.2 (página /audit-logs).

## 1. Veredicto global: FAIL suave — 4 hallazgos (0 críticos), sistema mayormente fail-honest

La DApp es **sustancialmente honesta** (RULE 00/R8 visiblemente implementada en los lugares difíciles:
cards con "—", "no emitido", "unscored", estados vacíos declarados, WS degradado declarado, 404 sin
teatro). Encontré **4 defectos de honestidad numérica/prosa**, todos agent-fixables, ninguno fabrica un
verde de seguridad ni oculta estado vacío tras datos inventados. El más serio (H-1) es prosa que se
contradice con el panel que la acompaña.

## 2. Journey recorrido (con evidencia)

| # | URL | Qué se ve | Captura |
|---|---|---|---|
| 1 | `/` | System guard honesto (Live OFF · Broadcast OFF · Paper ON EXPLICIT · GO live NO-GO · Flip blocked), posture WS viva, cards "— unscored", EV/risk "no computado" | `r8-home-posture-honesta-2026-09-09.png` (repo root) |
| 2 | `/audit-logs` | Admin-gate honesto, cero datos tras el gate | `r8-auditlogs-admin-gated-honesto-2026-09-09.png` |
| 3 | `/live-readiness` | NO_GO · 3 PENDING · blockers reales (G-DISK-1, G-PIPE-1, A.9) — y el resumen contradictorio (H-1) | texto + snapshot `.playwright-mcp/page-2026-09-09T00-12-55-458Z.yml` |
| 4 | `/opportunities` | "0 viable / 200 total (200 rejected)", 100% campos "—", "Fallback: polling edge every 4s" | `.playwright-mcp/r8-opportunities-feed-2026-09-09.png` |
| 5 | `/deploy-pipeline` | Evidencia fechada 2026-09-04 + "PENDIENTE OPERADOR" sin maquillar | texto (DOM) |
| 6 | `/control` | 404 honesto con copy explícito | `.playwright-mcp/r8-control-404-honesto-2026-09-09.png` |
| 7 | `/` (recarga, falsificación H-3) | "MEJOR TOPOLOGICAL YIELD · NETO — sin datos — feed vacío" | texto (DOM) |

## 3. Hallazgos (qué DEBERÍA mostrar según R8 vs qué MUESTRA)

### H-1 (MODERADO — categoría c: prosa que afirma más que la fuente) — `/live-readiness`
- **MUESTRA** (arriba del panel): "Siguiente acción: All known blockers cleared at this layer; await
  A.9 formal GO/NO-GO sign-off."
- **MUESTRA** (2 cm abajo, el mismo panel): **G-DISK-1 blocked** (high) "disk usage 94.5% at/above warn
  85% (crit 95%) — 8.2 GB free" · **G-PIPE-1 blocked** (critical) "selector consumer stalled: 500
  entries behind on arbx:opps:detected (deliverable, ≥500)" · A.9 pending (critical).
- **DEBERÍA mostrar**: la siguiente acción derivada de los blockers vivos (ej. "resolver G-PIPE-1
  crítico: selector stalled"). "All known blockers cleared" es falso en la cara del propio panel.
- **Root cause** (lectura de código, `backend/api-server/src/routes/readiness-extras.ts:586-595`): el
  `nextAction` se compone SOLO del blocker A.4 (`a4_fork_real_not_executed`); si A.4 no está, el string
  cae al default "All known blockers cleared…". G-PIPE-1/G-DISK-1 jamás entran en la frase. El veredicto
  `NO_GO` y el conteo `3 PENDING` sí son honestos (`summarize(blockers)`), y `go_live` es
  estructuralmente false (:579, triple capa de NO) — el defecto es SOLO la prosa del next-action.
- **Fix**: poblar `nextActionParts` con los blockers de mayor severidad reales (y no solo A.4).
- **Dato real adicional**: el disco al 94.5% es telemetría VIVA y alarmante (crit 95%, 8.2 GB libres) —
  la retención de ARBX-RETENTION-01 (48% el 09-04) se re-llenó en 4 días. Escalar a la mesa.

### H-2 (BAJO-MODERADO — categoría b: default disfrazado de dato) — cards y statcard del home
- **MUESTRA**: cards del feed home "+0.00%" y statcard "DECOHERENCIA MEDIA (CONVERGENCE RATIO) 0.00%".
- **DEBERÍA mostrar**: "—" / "no computado" cuando `roi_pct` es null para TODOS los ítems del feed
  (R8: None ≠ 0.0; Some(0.0) = computado y cero). Hoy el feed está 100% rejected con roi no computado.
- **Root cause** (`frontend/app/page.tsx`): `:57` y `:59` renderizan `+${(opp.roi_pct ?? 0).toFixed(2)}%`
  cuando net o gross existen pero roi_pct es null → "+0.00%" fabricado. `:94-97` promedia
  `acc + (o.roi_pct ?? 0)` → con 50 ítems todos-null muestra 0.00% en lugar de "no computado".
  La corrección análoga YA existe 3 líneas abajo para confidence (`:61-67`, AUDIT-2026-08-29 R8:
  "null propagates as null — never a fabricated 0%") — el mismo patrón falta aplicarse a roi.
- **Fix**: propagar null como null en ambos sitios; StatCard ya sabe renderear "—".

### H-3 (BAJO — categoría c: framing verde sobre expectativas rechazadas) — hero del home
- **MUESTRA** (00:11Z): statcard verde (variant success) "Mejor Topological Yield · neto $4.2279 ·
  neto · USD (spine/sim)".
- **DEBERÍA**: o bien etiquetar el denominador ("mejor neto ESPERADO de candidatas — 0 viables"), o
  bajar la variante visual. El $4.2279 es el `Math.max(net_expected_profit_usd ?? simulated_net_profit_usd)`
  (`page.tsx:91-98`) sobre la ventana live de 50 ítems — y el feed es 100% rejected
  (verificado por fetch directo: statuses=["rejected"], reasons spot_product_le_one/v3_quote_unavailable).
  Es decir: el hero verde exhibe la expectativa de oportunidades que el propio sistema rechazó.
- **Exculpatorio (verificado)**: la métrica es REAL y viva — a las 00:2x, con ventana sin nets, la
  recarga muestra honestamente "— sin datos — feed vacío" (falsificación ejecutada, §2 fila 7). NO hay
  fabricación ni caché: es sobre-afirmación de framing, no mock.

### H-4 (BAJO — etiqueta imprecisa) — statcard "Asimetrías detectadas"
- **MUESTRA**: "50 · stream arbx:opps:detected".
- **DEBERÍA**: el 50 es `opportunities.length` de un fetch `limit=50` (`page.tsx:27,90`) — es el tamaño
  de ventana, no un conteo del stream. Si el stream tuviera 10K, seguiría diciendo 50. Subtext debería
  decir "ventana live (limit=50)" o el endpoint debería exponer el total real.

### 3.1 (a) Estados vacíos ocultos — NO ENCONTRADOS (positivo)
- Home: "ejecuciones: feed vacío" declarado. `/opportunities`: "0 viable / 200 total (200 rejected)"
  a la vista, no oculto tras spinner ni skeleton eterno. `/audit-logs`: gate explícito. Ningún panel
  parece lleno sin datos.

### 3.2 (d) /audit-logs y admin — honesto pero inaccesible sin sesión
- `/audit-logs` MUESTRA: alert "Admin session required — Audit logs are admin-gated" + link a
  /admin/signin. **Correcto según R8**: mejor un gate declarado que registros fabricados. No pude
  verificar timestamps de registros (sin token; §32/§33 me prohíben intentar bypasear). La mesa ya tiene
  el issue de ledgers (CB-02-API-VERIFY G2: `audit_log` mig 011 vs `audit_logs` mig 121 inexistente).
- `/deploy-pipeline` (visible sin sesión) MUESTRA: evidencia con fecha explícita "evidencia (2026-09-04)"
  y la línea estrella: *"nivel-(b): sin endpoint runtime de registry/CI — los estados por razón son
  evidencia estática fechada (verificable contra git/API GitHub/probes), NO estado vivo ni fabricado
  (RULE 00)"* + "Autorización operador del gate final: PENDIENTE OPERADOR". Timestamps internos
  coherentes (A.4 08-20, A.8 08-29, A.5 08-29, A.6/A.7 09-07 — coincide con memoria de mesa). Nota
  menor: la evidencia L4 cita "VPS HEAD == 5074bb4e" (foto del 09-04; hoy VPS = e65040f1) — está fechada
  y declarada estática, no miente, pero un lector apurado puede confundir la foto con el presente.

### 3.3 WebSocket en vivo (señales en DOM/console, no curl)
- `/opportunities` (00:24Z): badges **socket DISCONNECTED · routes DEGRADED · runtime_ack DISCONNECTED ·
  pairs STALE · quote_anchor STALE** + banner "Fallback: polling edge every 4s · Last refresh: 7:24:21 p. m."
  → degradación declarada honestamente; el feed seguía entregando cards vía REST.
- Home (00:11Z y recarga): socket CONNECTING, routes LIVE, runtime_ack LIVE, pairs/quote_anchor
  CONNECTING; header "Live opportunity stream status: IDLE". Estados vivos, cambiantes entre cargas —
  consistente con socket.io real reconectando, no con un badge pintado fijo.
- Console: 0 errores/warnings en `/control`; sin errores graves observados en el journey.

### 3.4 (e) Headers/CSP de la home — SANOS
- `content-security-policy-report-only`: default-src 'self'; frame-ancestors 'none'; base-uri 'self';
  object-src 'none'; connect-src acotado. **Report-only** = coherente con el censo CB-01
  (`ARBX_CSP_ENFORCE` ausente en VPS, built-not-wired capa deploy). Contiene `unsafe-inline`/`unsafe-eval`
  en script-src (nota de endurecimiento pendiente, ya conocida).
- `strict-transport-security: max-age=31536000; includeSubDomains` · `x-frame-options: DENY` ·
  `x-content-type-options: nosniff` · `referrer-policy: no-referrer` ·
  `permissions-policy: camera=(), microphone=(), geolocation=()` · cache-control no-store.
- Sin leak de localhost en CSP (R2/RULE 02 respetadas desde afuera).

## 4. Confirmaciones a pares (nada refutado)

1. **CB-VERIFY-FRONTEND §5** — `/control` 404: confirmado con navegación propia + DOM ("Route not
   found", alert "404 — no such page … not because we're hiding it"). NO-GIT sigue intacto: el board
   CB-03 NO está desplegado.
2. **CB-01-CROSS-EXAM §1e** — killswitch real `enabled:false` ↔ DApp "KILL SWITCH OFF" + "Paper: ON ·
   EXPLICIT" + "Capital $0": alineado punta a punta.
3. **CB-VERIFY-FRONTEND F-4** — sigue NO-observable desde el dominio público (el banner no existe en
   producción). Reitero el hand-off: debe resolverse ANTES del deploy de CB-03/CB-04, pero hoy no es
   una mentira visible para el operador.
4. **GOAL-WORKORDERS regla "RULE 00: estado desconocido se muestra como DESCONOCIDO"** — la app real
   ya lo hace en posture (badges DISCONNECTED/STALE/CONNECTING por canal); mis hallazgos H-2 son los
   últimos reductos donde null se coacciona a 0.

## 5. Gaps y clasificación

| # | Gap | Clase |
|---|---|---|
| 1 | H-1: next-action "All known blockers cleared" ignora G-PIPE-1/G-DISK-1 (readiness-extras.ts:586-595) | agent-fixable |
| 2 | H-2: roi_pct null → "+0.00%" (page.tsx:57,59) y promedio 0.00% (page.tsx:94-97) | agent-fixable |
| 3 | H-3: hero verde "Mejor Topological Yield" sobre max-net de ítems rejected — re-etiquetar denominador | agent-fixable |
| 4 | H-4: "Asimetrías detectadas" muestra tamaño de ventana (limit=50) con subtext de conteo de stream | agent-fixable |
| 5 | Disco VPS 94.5% (crit 95%, 8.2 GB) — telemetría viva del readiness; re-llenado post-RETENTION-01 | operator-gated (acción VPS) |
| 6 | /audit-logs no verificable sin admin-token (por diseño) — verificación de timestamps queda para el operador o sesión autorizada | operator-gated |

## 6. Nota operativa para la mesa: browser MCP COMPARTIDO

El Playwright MCP corre UN solo browser para todos los agentes paralelos: encontré páginas cargadas
antes de mi primer navigate, mi pestaña fue navegada a `/opportunities` por otro agente, un snapshot mío
capturó el `/control` 404 que había navegado EL PAR (por eso lo re-verifiqué yo mismo), el servidor
quedó colgado 30 min (timeout 1817s en console_messages) y el par abrió un contexto aislado
(`ws-qa-socket-feed`) que des-seleccionó mi página. **Recomendación**: cada agente de browser debe usar
`isolatedContext` propio (el par ya lo hizo al final) y/o la mesa debe serializar los journeys de
browser. Mis evidencias están re-validadas post-contaminación (H-3 falsificado con recarga; /control
re-navegado por mí). Capturas de archivo del journey: `.playwright-mcp/page-2026-09-09T00-1*.yml`.

## 7. Presupuesto

~9 navigaciones / ~7 requests efectivos al dominio (2 extra por contaminación del browser compartido,
declarados). 0 requests a API admin. 0 ssh (los hechos VPS que citó mi análisis provienen de
CB-01-CROSS-EXAM §1e, doblemente verificado por la mesa — no re-derivo). 0 git. 0 escrituras.
