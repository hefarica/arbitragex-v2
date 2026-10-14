# INTEGRATION MANIFEST — F0 maestro (t2)

> Equipo `prices-v4-integration` · auditor · 2026-10-13 · rama `feat/exp-20261013` @ 8ad03ab35 (árbol restaurado por t1: D=0, M=4092 declaradas en `docs/integration-v4/t1-evidence/T1_modified.txt`).
> Fuente autoritativa: `.tmp-inbox/integrado-supremo/` (**INTEGRADO manda**; ZIPs previos = REFERENCIA). Hashes de attachments: `docs/integration-v4/t1-evidence/T1_inventario_sha256.txt`.
> Fail-honest: toda afirmación cita ruta:línea o comando con salida.

## (a) Mapa artefacto → repo

| Artefacto (integrado-supremo/) | Rol | Destino repo | Estado F0 |
| --- | --- | --- | --- |
| `MANIFEST.txt` (A1) | fuente ÚNICA de g1-g6 (§7 anti-circularidad) | regenerar en `docs/integration-v4/` (t6, regla de hash §11-bis) | insumo verificado, sha256 `45b3bb2e…35ed575` |
| `00_PLAN/PLAN_T1_T8.md` | semilla A4 | `docs/integration-v4/PLAN_T1_T8.md` | ✅ commiteado 8ad03ab35 (sha256 `f27d869c…44f2cd`) |
| `00_PLAN/CRITERIOS_T6.md` (A3) | 7 criterios de aceptación | `docs/integration-v4/` | pendiente t6 |
| `01_SSOT/strategy_hop_map.json`+`.csv`, `detector_policy.csv` | SSOT a congelar | actualiza/sustituye `docs/quotebase_strategy_hop_map.json` y `backend/searcher-rs/src/strategy_hop_map.fixture.json` | pendiente t9 (hashes en inventario t1, líneas SUP) |
| `02_ALGORITMOS/ALGORITMOS.md` (A01–A20) | mapa algorítmico | referencias verificadas en §b | PARCIAL: 4 referencias no existen (§e.5) |
| `03_FORMULAS/FORMULAS_TARJETAS.md` (A5) | CARDS-NOTIONAL-01, ε=0.005, cota 5×, basis, 9 cost labels, Gate D SQL | cablear en t3 | insumo |
| `03_FORMULAS/FIELD_REGISTRY.md` (A6) | 14 campos del ledger | harness t3 + Gate D | insumo — GAP de schema en §d |
| `04_DIFFS/` ×5 | stubs semánticos de 1 línea (`+ // …` / `+ const …`) | N/A — no son parches aplicables | ❌ ver §c |
| `05_HARNESS/t1…t5.test.ts` ×5 | harness esquema/mask/ledger/deriv/gated | montar en rutas reales (t3) | insumo |
| `06_CLASIFICACION/clasificacion_264.csv` (A2) | 264 estrategias | validación exacta (t8) | insumo, sha256 `38f038a8…0a7786` |
| `ArbitrageX_v4_264_60hojas.xlsx` (A8) | 59 hojas / TESTS_ATOMICOS 264.000 filas | ingesta t8; +REGISTRO_INCLUSIONES ⇒ \|hojas\|≥60 | verificado sha256 `eb5b803c…e09cc3` == MANIFEST:14 == miembro del zip INTEGRADO (t1) |
| `08_AUDITORIA/RASPADO.md` | raspado live @5f8ea3d6: PAPER ON, 0 viable / 32 rejected | contexto | citado en §c/§d |

**ZIPs previos (REFERENCIA, no mandan)** — hashes en inventario t1: ATT-1 `arbitragex-v4-diffs.zip` `50d6351d…`; ATT-2 `arbitragex-prompt-supremo.zip` `e4de1832…`; ATT-3 `arbitragex-prompt-supremo-INTEGRADO.zip` `7b1b93bc…` (contiene exactamente el árbol integrado-supremo + xlsx); ATT-4 `arbitragex-v4-INTEGRADO.zip` `4c2396b5…` (igual sin xlsx/json). ATT-5 xlsx CORREGIDO y ATT-6 MANIFEST = artefactos A8/A1 ya mapeados.

## (b) Ubicaciones reales en el árbol (ruta:línea)

| Objetivo F0 | Ubicación real | Verificación |
| --- | --- | --- |
| `hop_cycle_bridge.rs` | `backend/searcher-rs/src/route_discovery/hop_cycle_bridge.rs` | EXISTE (git ls-files) |
| `detector_policy.rs` | `backend/searcher-rs/src/detector_policy.rs` | EXISTE; vocabularios Sheet 13 (`required_data` :174–176); **sin 'DFAC'** (grep backend + worktrees = 0 matches) |
| bug `DFAC['D_']` | **NO está en el repo — vive en el GENERADOR (xlsx A8)**: `xl/worksheets/sheet1.xml` (hoja LEEME): "FIX: DFAC\[D_\]=0.80 -> DERIVATIVES con positivos"; `xl/worksheets/sheet33.xml` (hoja **F04**): "D_  Derivados (fix DFAC D_=0.80)  5  30" (5 detectores × 6 estrategias = 30) | t9 materializa el coeficiente 0.80 al regenerar `detector_policy.rs` desde SSOT (L1) |
| cota `\|gross\| ≤ 5·principal` | `backend/searcher-rs/src/engines/triangular_engine.rs:69` = `const SANITY_PROFIT_MULT_OF_CAP: f64 = 5.0;` (doc :67–68 "reject if profit/cap > this factor") | ✅ coincide con A5:5 y ALGORITMOS A08 |
| `opportunity-ledger.ts` | **NO existe en el árbol principal**; solo en worktrees `.claude/worktrees/gas-cb-fix/` y `.claude/worktrees/rpc-rec-01/` (`frontend/lib/opportunity-ledger.ts`) | crear en t3 (stub: `LEDGER_TOLERANCE_USD=0.005`) |
| websocket gateway | `backend/api-server/src/websocket.ts:347` = `export function setupWebSocketGateway(server, carnotStore?, logger?)` — firma COINCIDE con el contexto del parche v4; **SIN** `PRICES_ROOM`/`subscribe:prices` hoy (Select-String = False) | añadido nuevo por `PATCHES/websocket.ts.diff` |
| hook monitor | `frontend/app/monitor/hooks/useSocketIO.ts` | EXISTE |
| socket-lifecycle | `frontend/features/opportunities/socket-lifecycle.ts:50` = `const CONNECT_OPTS = { reconnectionAttempts: 5, timeout: 2000 }` — confirma `PATCHES/nota-reconexion.md`; copia espejo en `frontend/features_backup/opportunities/` | target del parche v4 |
| PricesStream actual | `frontend/lib/hooks/usePricesStream.ts` (+test); api-server: `backend/api-server/src/prices-stream.ts` (comentario seq :26), `backend/api-server/src/routes/prices-live.ts` (`GET /api/v1/prices/live`) | coexistencia t5 — nada se borra |
| `priceBus.ts` | NO existe (archivo nuevo; stub `MIN_INTERVAL_MS=100`) | t5 |
| SSOT actual en repo | `docs/quotebase_strategy_hop_map.json` + `backend/searcher-rs/src/strategy_hop_map.fixture.json` | congelar/comparar en t9 |
| edge twins | `edge/worker/src/index.ts:702` = `app.get("/api/prices/live", … proxyPassThrough(c, "/api/v1/prices/live"))` — línea de contexto EXACTA del parche v4 ✓; `edge/dev-local/src/index.ts` SIN rutas prices (asimetría declarada en `backend/api-server/src/edge-parity.test.ts:53` `route: "GET /api/prices/live"`) | ARBX-V-014 |
| ruta SSE `/api/v1/prices/stream` | NO existe en api-server (grep: solo `/api/v1/prices/live` en `prices-live.test.ts:2,35,40…`) | nueva — t4 (edge worker + dev-local en paridad) |

## (c) Reconciliación sha

- **Base REAL de los artefactos = `5f8ea3d67c964d152ae8fda89883952de8282dfb` (main real)**: `MANIFEST.txt:3` `repo @ 5f8ea3d67…` == `git rev-parse origin/main` == sha_live del release activo (GET `/api/status` → `deploy.sha`, id 38353373370, 2026-10-13T06:16:24Z). Confirmaciones independientes: `.tmp-inbox/v4-diffs/00-LEEME.md:3` (fuente `raw.githubusercontent.com/hefarica/arbitragex-v2/main/`) y `RASPADO.md:2` ("Repo main @ 5f8ea3d6").
- **HEAD de trabajo ≠ base**: `feat/exp-20261013` = 6719f079 (+A4 8ad03ab35), linaje `step2.5c-snipe-real-fetch`. El worktree además arrastra **4092 M con contenido REAL** (+19.676/−544 vs HEAD, medido con `git diff --ignore-cr-at-eol` en t1) — trabajo no commiteado que NO se pisa.
- **`git apply --check` — salidas literales (9/9 FALLO)**: los 5 stubs INTEGRADO (`detector_policy.rs.diff`, `hop_cycle_bridge.rs.diff`, `opportunity-ledger.ts.diff`, `priceBus.ts.diff`, `socket-lifecycle.ts.diff`) y los 4 parches v4 (`PATCHES/socket-lifecycle.ts.diff`, `websocket.ts.diff`, `edge-worker.diff`, `edge-dev-local.diff`) devuelven TODOS:
  ```
  error: No valid patches in input (allow with "--allow-empty")   [exit=128]
  ```
  Ninguno tiene hunks `@@ -l,c +l,c @@` numerados: son stubs/patches ANOTADOS. ⇒ **Integración MANUAL guiada por referencia; NINGÚN diff aplica limpio.** Los contextos citados SÍ calzan con main real (worker `index.ts:702`, `websocket.ts:347`, `socket-lifecycle.ts:50`, allowlist `edge-parity.test.ts:53`), así que los parches son especificación fiable punto-por-punto.

## (d) Gate D SQL — tabla objetivo y columnas

- **Tabla objetivo ACTUAL: `opportunities`** (`database/migrations/003_opportunities.sql:4` CREATE TABLE). NO existe tabla ledger en `database/migrations/`.
- Columnas existentes: 003 (id, chain_id, strategy_kind, dex_a, dex_b, pair_symbol, token_in, token_out, amount_in_wei, expected_profit_usd, roi_pct, risk_score, block_number, status, **rejection_reason:24**, trace_id, detected_at, updated_at) + ALTERs: 033:37/44/51 (chain_id_out, bridge, bridge_fee_usd), 099:41 (route_metadata JSONB), 102:27 (cartridge_id), 121:23/30 (**detector_id**, pipeline_latency_ms).
- **vs A6 FIELD_REGISTRY (14 campos: mev_id, detector_id, family, surface, hop_mask, gross, total_cost, net_amount, basis, computation_status, rejection_reason, exec_class, seq, ts): solapan 2/14** (`detector_id`, `rejection_reason`). **FALTAN 12**: grep de `computation_status|net_amount|exec_class|hop_mask|basis|total_cost` en TODAS las migraciones = 0 definiciones (único match `045_dex_chain_metrics.sql:81` = comentario "insufficient basis", ajeno).
- ⇒ **El predicado Gate D (`FORMULAS_TARJETAS.md:8`: `rejection_reason IS NULL OR (computation_status='computed' AND net_amount>0)`) NO es ejecutable hoy**: `computation_status` y `net_amount` no existen en el schema. Requiere ALTER TABLE o tabla ledger (spec t3/t5). El "Gate D activo" de `RASPADO.md:2` es el gate fail-honest ACTUAL sobre `rejection_reason` (histograma de 32 rechazos), no el predicado v4.
- **PG vivo: NO COMPUTADO** — `sql_query`/`sql_tables` (db `arbitragex`) devolvieron data vacía 2 veces en esta sesión (ok:true, 0 filas); posible credencial/DB/SSH distinta del stack real. Verificación de schema hecha contra las migraciones del repo (fuente sólida, ruta:línea arriba).

## (e) Divergencias DECLARADAS (no reconciliadas en silencio)

1. **delta_pct**: A1 `MANIFEST.txt:10` = `+60.8%` — ÚNICA fuente de g1-g6 (§7). El valor §10 '+0.604' NO tiene artefacto en el paquete (0 apariciones de `0.604` y `0.636` en los 59 XML de hojas del xlsx y en grep de `.tmp-inbox`); llega solo por el texto del contrato del prompt maestro. `+0.636` PROHIBIDO: ausente del paquete (0 apariciones).
2. **59 hojas vs invariante ≥60**: t1 contó 59 hojas fijas (todas presentes); se cierra al añadir `REGISTRO_INCLUSIONES` (A7, t14) y ADM-n durante la ejecución.
3. **DFAC['D_']**: el bug y su fix (0.80) viven en el GENERADOR (xlsx LEEME + F04), no en código del repo; t9 debe regenerar `detector_policy.rs` desde SSOT con ese coeficiente.
4. **Formato de parches**: 04_DIFFS INTEGRADO = stubs de 1 línea; PATCHES v4 = parches anotados sin hunks numerados. Ninguno aplica vía `git apply` (9/9, §c).
5. **Refs de ALGORITMOS.md aspiracionales**: A01 `searcher-rs::marginal.rs`, A02 `dirty.rs`, A03 `negcyc.rs` NO_EXISTEN; A09 "scanner.rs:452" no corresponde (real :452 = `SizeOptimizer::new(...)`, y `slot0` tiene 0 matches en `backend/searcher-rs/src/scanner.rs`, 3597 líneas).
6. **Asimetría edge actual**: `/api/prices/live` existe SOLO en worker (`edge-parity.test.ts:53`); añadir `/api/prices/stream` en AMBOS árboles no toca el allowlist según `edge-worker.diff:14-15` — t4 debe verificarlo con `npx vitest run backend/api-server/src/edge-parity.test.ts`.

> **Regla de hash §11-bis**: el sha256 de este manifiesto debe quedar registrado dentro de A1 al regenerarlo (t6).
