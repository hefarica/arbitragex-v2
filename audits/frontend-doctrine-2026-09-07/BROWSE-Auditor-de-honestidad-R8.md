# BROWSE · Auditor de honestidad R8 — la DApp bajo juicio adversarial (Gang Omniscience 2026-09-07)

> **WO:** FE-05 (verificación browser —贡献 de fe honesta) · **Agente:** Auditor de honestidad R8
> **Persona:** HUMANO usuario de la DApp, criterio adversarial, sin acceso a código en el momento de mirar.
> **Dominio:** https://arbx.ape-tv.net · **Fecha de fe:** 2026-09-08/09 (00:12–02:15 UTC aprox.)
> **Presupuesto:** ~8 navegaciones (límite 15) · 0 requests HTTP manuales extra (toda la evidencia via
> journey del navegador + panel de red del browser) · VPS SOLO LECTURA (`ssh arbx`: docker ps/logs,
> redis-cli read) — §32/§33 respetados, CERO mutación, CERO executor/wallet/broadcast, NO-GIT.
> **Clasificación de fuentes:** todo lo citado del dominio/vivo = PRIMARY_SOURCE (browser-observado);
> código citado = CANONICAL_REPO; lo inferido por mí = [INFERRED]; lo no determinable = [UNKNOWN].

---

## 0. Sincronía de mesa redonda (mi PRIMERA acción fue leer el board y a los pares)

Leídos ANTES de navegar: `GOAL-WORKORDERS.md` (completo), `FE-01-DESIGN.md`, `FE-02a-DESIGN.md`,
`FE-02b-DESIGN.md`, `FE-03-DESIGN.md`, `FE-04-DESIGN.md` (completos). Este reporte es público para
los pares que vengan después.

**Confirmaciones en vivo de hallazgos de pares:**
- **FE-02a BUG-08 (feed badge IDLE donde CONNECTING sería preciso, `RuntimePostureBar.tsx:103-133`)** —
  CONFIRMADO en vivo en home y en cada página bajo corte: chip socket `IDLE`/`CONNECTING` según canal,
  nunca `LIVE` en CONNECTING. El residual LOW (precisión IDLE vs CONNECTING) persiste. Ver §2.7.
- **FE-02a BUG-09 (empty state sin razón en /opportunities, `OpportunitiesClient.tsx:415-419`)** —
  NO RE-VERIFICADO esta sesión (la pestaña /opportunities era de un peer; no contaminé su estado).
  Queda en pie el hallazgo del par; sin evidencia propia nueva ni refutación.
- **FE-04 §5 D-11 (dominio público corre POLLING; NO diagnosticar "WS muerto" como defecto FE)** —
  APLICADO como doctrina de lectura en TODO este reporte: un chip socket en CONNECTING/IDLE sobre esta
  superficie es postura honesta, no defecto del frontend.
- **FE-01 §4.1 (/control huérfana — sin link entrante)** — CONFIRMADO por navegación directa: la página
  **no existe desplegada** en el dominio público (404). El árbol local la tiene untracked (programa
  CB-03 en vuelo): coherente con "no desplegada", no con "rota". Ver §2.1.
- **FE-02b (/executions listado "PENDIENTE R8")** — esta auditoría la cubre bajo corte (§2.5); el estado
  POBLADO queda pendiente de re-fe post-recuperación.

**Refutación de NINGÚN par:** no encontré evidencia que contradiga hallazgo previo. Una precisión
adjudicada: la confusión /readiness→/opportunities reportada en vivo por otro agente fue artefacto del
browser COMPARTIDO (peers con pestañas propias), no redirect del código (`frontend/app/readiness/page.tsx`
no tiene lógica de redirect — leído en árbol local).

---

## 1. Contexto operativo que enmarca la fe (incidente REAL en vivo)

Durante el journey OCURRIÓ un incidente de producción que la propia UI fue declarando con honestidad
creciente. Cadena forense (VPS solo-lectura, ~02:0x UTC):

1. **Disco lleno**: `df -h /` → `144G/150G` uso, **0 available**. G-DISK-1 se materializó EN VIVO
   (a las 00:12 UTC /readiness ya lo mostraba honesto: "disk usage 94.5% … 8.2 GB free").
2. **PostgreSQL crash-loop**: logs `postgres` → `FATAL: could not write to file … No space left on device`.
3. **api-server en restart loop** (`docker ps` Restarting) → **edge worker responde HTTP 500**
   `{"error":"internal_error"}` a TODOS los `/api/*` (con CORS + `x-arbx-trace-id` presente — el edge
   vive y contesta; el upstream es el muerto). Trace id observado:
   `128d7b2f-ad50-44e7-b32c-a1c6969db2e8` (GET /api/readiness, 500, Wed 09 Sep 02:12:25 GMT).
4. **Redis + searcher-rs VIVOS**: stream `arbx:opps:detected` fluyendo; consumer `enricher` con
   **lag=18,022 entries** (XINFO GROUPS) y last-delivered-id fresco — el blocker **G-PIPE-1 es
   LIVE-REAL**, no rancio: el consumidor queda ≥500 detrás (umbral del deliverable) desde la caída.

Este incidente fue, perversamente, el MEJOR escenario de prueba R8 que se podía pedir: el backend murió
a mitad del journey y cada página tuvo que declarar SU razón de vacío. Ninguna fabricó.

---

## 2. Journey página por página (URL + qué se ve + por qué correcto/incorrecto)

### 2.1 `/control` — 404 (CB-03 no desplegado) · CORRECTO-HONESTO
**Qué se ve:** página 404 estándar de Next. **Por qué correcto:** el programa CB-03 existe como árbol
local untracked (`frontend/app/control/`, `ControlBoardLed.tsx` — verificado en repo por FE-04 §0.1);
NO desplegarlo produce 404 en vez de un panel fantasma. Un 404 es "no existe" dicho con la boca llena —
lo incorrecto sería servir una versión stale. **Gap (operador/programa CB-03):** desplegar + P0-5
(des-huérfana del sidebar, diff de 1 línea ya diseñado en FRONTEND-DOCTRINE.md §2).
Shot: `screenshots/r8-control-404-cb03-no-desplegado-2026-09-08.png`.

### 2.2 `/readiness` — POBLADO (pre-corte) · CORRECTO, NO-GO sin maquillaje
**Qué se ve (00:12 UTC, snapshot nav-time):** Overall Status **NO-GO** (pill roja) + "3 PENDING".
Fase `P2_READINESS` · Capital expuesto **$0.00** · Live trading **OFF**. System guard: Paper ON ·
GO live NO-GO · Capital $0 · Readiness 4/4 · LIVE lock REVIEW · "Flip blocked". Postura: KILL SWITCH
OFF · PAPER MODE ON · socket **STALE** (ámbar) · routes LIVE · runtime_ack LIVE · paint STALE ·
quote_snapshot STALE. Blockers con razón textual exacta: **G-DISK-1** (high — "disk usage 94.5% at/above
warn 85% (crit 95%) — 8.2 GB free"), **G-PIPE-1** (critical — "selector consumer stalled: 500 entries
behind…"), **A.9** (critical — sign-off formal pendiente). Callout: "Fuente de verdad: backend" +
"Doctrina Zero-Mocks: sin datos fabricados".
**Por qué correcto:** el veredicto NO-GO se muestra SIN suavizado, con los 3 blockers nombrados y
cuantificados; los canales sin dato fresco degradan a STALE en vez de fingir LIVE; y el propio G-DISK-1
anunció (94.5%) lo que 2h después mató el disco (100%) — el sistema dijo la verdad ANTES de que doliera.
Shot: `screenshots/r8-readiness-nogo-3blockers-2026-09-08.png` (verificado visualmente).

### 2.3 `/readiness` — RECHECK bajo corte · CORRECTO, degradación honesta
**Qué se ve (02:12 UTC):** banner "Backend no disponible…" + **Overall Status UNAVAILABLE** +
"R8 fail-honest: no se fabrican gates." **Por qué correcto:** cuando no puede computar el estado,
declara "no computado" (None ≠ Some(0)) — exactamente R8. No muestra un NO-GO rancio ni un GO falso.
Shot: `screenshots/r8-readiness-recheck-outage-2026-09-08.png` · snapshot:
`snapshots/r8-readiness-recheck-snapshot-2026-09-08.txt`.

### 2.4 `/status` · CORRECTO — el patrón fail-honest de referencia
**Qué se ve:** alert "edge / upstream failure — edge HTTP 500: {\"error\":\"internal_error\"}" +
"This view displays only real edge data. There is no fallback. If the edge is down, this page shows
the error — it never synthesizes values." System guard degrada sin inventar: A.4/A.5 **UNKNOWN**,
PG/Redis **edge_error**, HB **stale/absent**, Engine **?**, Flip **?**, Readiness **0/4** honesto
(no el 4/4 rancio de cache). Feed: "Opportunity feed unavailable — edge HTTP 500 (retrying every 30s)"
— declara SU razón y SU retry. Postura: socket CONNECTING, routes LIVE, runtime_ack LIVE, pairs
CONNECTING, quote_anchor CONNECTING — LIVE solo donde el canal REST-nativo sigue vivo.
**Por qué correcto:** cada celda vacía lleva la razón del vacío. Cero spinners eternos, cero ceros
decorativos. Shot: `screenshots/r8-status-failhonest-edge500-2026-09-08.png` · snapshot:
`snapshots/r8-status-snapshot-2026-09-08.txt`.

### 2.5 `/executions` · CORRECTO bajo corte — estado POBLADO queda PENDIENTE
**Qué se ve:** mismo error edge 500 declarado con copy de contexto honesta ("Paper-mode is ON until
S9" — el terminus de ejecución se declara, §34). **Por qué correcto:** vacío con razón. **Límite
honesto:** FE-02b marcó esta página "PENDIENTE R8" en su estado POBLADO; bajo corte no puedo dar fe
de filas de ejecución reales. Re-fe post-recuperación = gap.

### 2.6 `/paper/history` · CORRECTO — la página más honesta de la DApp
**Qué se ve:** "Paper history unavailable — upstream: history HTTP 500 · GET /API/PAPER/HISTORY".
Y aunque vacío, declara método y status: "SOURCE: POSTGRES", caveats: "Rows before 2026-08-16 lack
gas/route capture…", "route_id, detector_id, quote_version, graph_version y config_version no están
persistidos en ningún contrato (gap nivel-(b), declarado — **no se muestran ni se fabrican**)",
"Net bps = roi_pct persistido ×100 (conversión de unidad de display, no recomputo §79)".
**Por qué correcto:** dice QUÉ no persiste y que no lo va a fingir — el estándar R8 aplicado hasta la
meta-comunicación. Shot: `screenshots/r8-paper-history-upstream500-declarado-2026-09-08.png` ·
snapshot: `snapshots/r8-paper-history-snapshot-2026-09-08.txt`.

### 2.7 `/` (home) + trayectoria del chip socket en TODAS las páginas · CORRECTO con 1 residual
**Qué se ve:** posture bar con socket IDLE bajo corte (nunca LIVE), ticker inferior con fila real
("2250fa…c02aaa_ · UniswapV2 → SushiSwap · +0.00%") y Worker health fresco (7s). En la pestaña del
peer sobre /opportunities (observada, no intervenida): chip **LIVE** + "0 viable / 50 total
(50 rejected)" + "Last refresh 7:12:59 p. m." fresco — LIVE solo mientras el feed entregaba filas
reales (50 Holonomic Loop Resolutions detectadas y rechazadas por economía — rechazo honesto, no
ausencia fingida).
**Residual LOW (BUG-08, FE-02a `RuntimePostureBar.tsx:103-133`):** bajo corte, el chip del feed
mostró **IDLE** donde **CONNECTING** sería más preciso (IDLE≈sin intento; el feed SÍ reintienta cada
30s — lo dice el propio copy de /status). Postura no-mentirosa (no finge LIVE), pero imprecisa.
Agente-arreglable (FIX-8 ya diseñado por el par).
**Nota [INFERRED]:** la fila "…+0.00%" del ticker — si es fila cotizada, 0.00 es "computado y
exactamente cero" (R8-legítimo: Δ precio 0 en la ventana); si es fila SIN cotización, sería el
pseudo-Topological-Yield de BUG-06. Bajo corte no pude confirmar el estado de cotización de esa fila.
BUG-06 queda como hallazgo del par FE-02a, no re-confirmado ni refutado por mí.
Shot: `screenshots/r8-home-posture-ticker-2026-09-08.png`.

### 2.8 `/agent-insights` · CORRECTO
**Qué se ve:** "Could not reach /api/agents/status" — el upstream muerto nombrado, sin panel decorativo.
Shot: `screenshots/r8-agent-insights-api-caida-honesta-2026-09-08.png` · snapshot:
`snapshots/r8-agent-insights-snapshot-2026-09-08.txt`.

**Consola y red (todas las páginas):** sin errores de render/hydration propios del FE; los errores de
console son los fetch 500 propagados (honestos); peticiones `?_rsc=` de Next devuelven 200 (el shell
SSR vive) mientras `/api/*` devuelve 500 con `x-arbx-trace-id` — separación limpia shell/datos que
permite que la UI degrade en vez de blank-white.

---

## 3. Veredicto de fe (como lo daría el operador)

**FE (frente honesto).** En cada superficie auditada la DApp hizo lo que RULE 00/R8 exigen:
el vacío declara SU razón, el LIVE aparece solo con canales entregando, el NO-GO se muestra sin
maquillaje con sus 3 blockers nombrados, y cuando el backend murió a mitad del journey la UI degradó
a UNAVAILABLE/edge_error en vez de congelar números rancios. El incidente de disco que ocurrió en vivo
fue ANUNCIADO por G-DISK-1 dos horas antes de matar PostgreSQL — el sistema dijo la verdad cuando aún
no dolía. Los gaps que quedan NO son deshonestidad del frontend: son infraestructura caída (operador),
un programa sin desplegar (CB-03) y un residual de precisión de postura (agente-arreglable).

## 4. Gaps

| # | Gap | Dueño |
|---|---|---|
| G-1 | **Incidente vivo: disco 100%** (144G/150G, 0 avail) → PG crash-loop → api-server restart loop → edge 500. Remediation pointer (NO ejecutada, solo lectura): `docker builder prune` recupera ~**21.46 GB** (Build Cache 21.51 GB, `docker system df`); volumen PG 110.6 GB (retención/particionado = programa ARBX-RETENTION). | **operator-gated** |
| G-2 | **G-PIPE-1 LIVE-REAL**: consumer `enricher` lag **18,022** entries en `arbx:opps:detected` (umbral deliverable ≥500) — el selector come tarde desde la caída; requiere recuperación de PG + drenaje del lag. | **operator-gated** |
| G-3 | **/control 404**: CB-03 existe local (untracked) pero no está desplegado; además huérfana del sidebar (P0-5 de FRONTEND-DOCTRINE.md ya diseñado). | **operator-gated** (deploy) + coordinación CB-03 |
| G-4 | **Estados POBLADOS de /executions y /paper/history sin fe** (auditados solo bajo corte); re-fe post-recuperación. | **operator-gated** (requiere backend vivo) |
| G-5 | **BUG-08 residual (LOW)**: chip feed IDLE donde CONNECTING sería preciso (RuntimePostureBar.tsx:103-133; FIX-8 de FE-02a ya diseñado). | **agent-fixable** |
| G-6 | **BUG-06 sin veredicto vivo**: ticker "…+0.00%" — fila cotizada (R8-legítimo) vs pseudo-yield no-cotizado (BUG-06); requiere backend vivo para discriminar. | **agent-fixable** (post-recuperación) |

## 5. Evidencia (screenshots + snapshots)

- `audits/frontend-doctrine-2026-09-07/screenshots/r8-readiness-nogo-3blockers-2026-09-08.png` — NO-GO + 3 PENDING + posturas STALE honestas + System guard (contenido verificado visualmente).
- `audits/frontend-doctrine-2026-09-07/screenshots/r8-readiness-recheck-outage-2026-09-08.png` — UNAVAILABLE + "no se fabrican gates".
- `audits/frontend-doctrine-2026-09-07/screenshots/r8-status-failhonest-edge500-2026-09-08.png` — patrón fail-honest de referencia.
- `audits/frontend-doctrine-2026-09-07/screenshots/r8-paper-history-upstream500-declarado-2026-09-08.png` — caveats nivel-(b) "no se muestran ni se fabrican".
- `audits/frontend-doctrine-2026-09-07/screenshots/r8-agent-insights-api-caida-honesta-2026-09-08.png`.
- `audits/frontend-doctrine-2026-09-07/screenshots/r8-executions-edge500-honesto-2026-09-08.png`.
- `audits/frontend-doctrine-2026-09-07/screenshots/r8-home-posture-ticker-2026-09-08.png` — socket IDLE (BUG-08 residual) + ticker real.
- `audits/frontend-doctrine-2026-09-07/screenshots/r8-control-404-cb03-no-desplegado-2026-09-08.png`.
- Snapshots a11y completos en `audits/frontend-doctrine-2026-09-07/snapshots/r8-*-2026-09-08.txt` (5).
- Nav-time yml `.playwright-mcp/page-2026-09-09T00-12-55-458Z.yml` (estado poblado de /readiness).
- Red: reqid=205 GET /api/readiness → 500 `{"error":"internal_error"}` + `x-arbx-trace-id:128d7b2f-…`.

*Nota de integridad: los archivos `r8-*-2026-09-09.png` del raíz y de `.playwright-mcp/` son de un peer
auditor R8 paralelo — NO los reclamo como evidencia propia.*

— Auditor de honestidad R8 · FE-05 (fe de browser) · 2026-09-08/09
