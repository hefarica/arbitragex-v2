# AUDIT — Legs/Hops Tiers vs 264 estrategias vs repo (2026-09-20)

Orden del operador: auditar `C:\Users\HFRC\Downloads\legs . hops.xlsx` (política de tiers por
legs) y asociarla con las 264 estrategias del workbook canónico, verificando cobertura de
rutas exóticas (pools, dexes, N° tokens). Clasificación: CANONICAL_WORKBOOK / CANONICAL_REPO /
GAP.

## 1. Contenido del Excel `legs . hops.xlsx` (CANONICAL_WORKBOOK, hoja 06_LEGS_SCALING)

| Legs | Tier | Casos | Riesgo | Política |
|---|---|---|---|---|
| 2 | LIVE CORE | Two-point, cross-pool, parity | Baja | Default inicial |
| 3 | LIVE CORE | Triangular y paridades compuestas | Baja-media | Soporte completo |
| 4–6 | LIVE ADVANCED | Ciclos, wrappers, liquidations+unwind, derivatives | Media | Beam search + pruning |
| 7–8 | LIVE CONTROLADO | Multi-DEX complejo | Alta | Requiere sim exacta, gas y builder path |
| 9–12 | SHADOW | Descubrimiento long-tail y multi-domain | Muy alta | No LIVE hasta evidencia estadística |
| 13–16 | RESEARCH/PAPER | Ciclos extensos y settlement multiorden | Extrema | Hard cap configurable |
| >16 | NO RECOMENDADO | Teóricamente posible | Explosión combinatoria | Dividir, netear o demostrar necesidad |

- **Regla técnica**: el máximo NO debe estar cableado a 3; debe ser N configurable con hard
  cap; cada leg con type, domain, venue, asset_in/out, quote, calldata, pre/postconditions,
  timeout, compensation/unwind y risk delta.
- **Algoritmos**: Bellman-Ford (ciclos candidatos); DFS/beam search + pruning; Yen/K-shortest;
  split routing; sizing continuo/discreto; dominance pruning; gas-aware objective.
- **Objetivo**: `net = proceeds − input − gas − builder_bid − venue_fees − bridge/withdrawal
  fees − hedge_cost − expected_failure_loss − capital_cost`.

## 2. Asociación con las 264 estrategias (workbook canónico, `docs/excel_strategies_extracted.json`)

El workbook completo ya tiene `Min_Legs` / `Max_Legs` / `Legs_Model` POR estrategia — la tabla
anterior es la política de esos rangos. Distribución exacta:

| Rango legs | # estrategias | Tier según el Excel |
|---|---|---|
| 2..2 | 1 | LIVE CORE (two-point) |
| 3..3 | 3 | LIVE CORE (triangular puro) |
| 1..4 | 25 | LIVE CORE→ADVANCED (cross-domain/bridge) |
| 2..6 | 30 | LIVE ADVANCED |
| 3..4 / 4..4 | 2 | LIVE ADVANCED |
| 2..8 | **132** | LIVE CONTROLADO (el grueso: 50%) |
| 2..12 | 30 | SHADOW (long-tail, multi-domain) |
| 2..16 + 3..16 | 27 + 14 = 41 | RESEARCH/PAPER (settlement multiorden) |

Por `Max_Legs`: 4 en tier ≤3 · 57 en 4–6 · **132 en 7–8** · 30 en 9–12 · 41 en 13–16 · 0 en >16
(consistente con NO RECOMENDADO). Min_Legs: 245 arrancan en 2, 18 en 3, 1 en 4.

`Legs_Model`: 132 Ruta dinámica · 30 Hedge/estructura · 30 Settlement multiorden · 27 N-domain
dinámico · 25 Acción+unwind · 14 N-leg dinámico · 5 Fijo · 1 Paridad compuesta.

## 3. Cobertura del repo (CANONICAL_REPO, verificado 2026-09-20)

- **Motor de discovery** (`backend/searcher-rs/src/route_discovery/multi_hop_search.rs`): DFS
  acotado, ciclos 2..=`max_hops` con `Σ log_weight < 0` (Bellman-Ford equivalente por grafo
  log-ponderado, NaN-safe). `max_hops.min(7)` (línea 158) + `clamp(2,7)` en
  route_discovery_worker.rs:399 → **hard cap efectivo = 7**.
- **Knob**: `ARBX_KNOB_MAX_HOPS` (XLS-CANON-01, workbook 01_CONFIG `Max_Hops` = 7) — N
  configurable DENTRO de 2..7. El cap 7 en sí está cableado en código.
- **Hop mask**: `SearchLimits.hop_mask` permite habilitar/deshabilitar tiers individuales
  (bit por hop 2..7) — los canales de toggle por tier YA existen.
- **Filtro hops frontend** (PR #620 WO-1): select data-driven con los hop counts distinct del
  feed vivo — sin hardcode.
- **Net formula** (`backend/math-engine/src/roi_engine.rs`, Sprint A+B+C+C2): gas, flashloan
  fee, LP fees, slippage, **failure_cost** (= expected_failure_loss), **capital_cost**,
  ops_overhead, copied_buffer, **estimated_relay_fee** (Flashbots coinbaseDiff EWMA ≈
  builder_bid). `bridge_fee_usd` existe en el esquema de opportunities.
- **Roadmap** (route_discovery/README.md): Phase 2 MMBF line-graph (arXiv:2406.16573) para
  rutas 7–11 hop — NO implementado; Phase 3 sizing marginal; Phase 4 cross-rollup; Phase 5 GNN.

## 4. GAPS (carga para Hermes / BOARD)

> **CORRECCIÓN 2026-09-20 (post-workbooks completos)**: tras ingerir
> `ArbitrageX_Dynamic_QuoteBase_Route_Manual_264.xlsx` (hoja `08_HOPS_2_7`) y
> `ArbitrageX_MEV_Universe_Control_Matrix.xlsx` (hojas `06_LEGS_SCALING` + `09_IMPLEMENTATION_ROADMAP`),
> el GAP-LH-1 original queda REFORMULADO: el cap 7 NO es un accidente de código — es la regla
> de admisibilidad canónica del workbook. Ver §6.

- **GAP-LH-1 (reformulado) — cap 7 canónico, no gap de drift**: la regla de admisibilidad
  del workbook (`08_HOPS_2_7` fila ADMISSIBILITY) es
  `StrategyHopAllowed(s,h) = h ∈ [max(2,Min_Legs_s), min(7,Max_Legs_s)]` — el `min(7,…)` del
  código Rust ES la materialización fiel de esa regla. El Excel `legs . hops.xlsx` es el
  extracto de `06_LEGS_SCALING` (política a largo plazo). Lo que queda como gap real: el
  hard cap superior (7) está cableado como constante; la regla técnica del operador pide que
  el máximo sea N configurable con hard cap — levantar 8..16 es la Fase 1 del roadmap
  canónico ("2–8 LIVE-ready, 12 SHADOW, 16 hard cap").
- **GAP-LH-2 — 71/264 (27%) sin vía de descubrimiento HOY**: los tiers SHADOW (9–12) y
  RESEARCH (13–16) del roadmap canónico Fase 1 requieren cap >7; hoy solo existen 2..7.
  MMBF Phase 2 cubriría 7–11.
- **GAP-LH-3 — metadatos por leg incompletos**: RouteEdge tiene protocol/fee/liquidity/
  direction/freshness; faltan calldata, pre/postconditions, timeout, compensation/unwind y
  risk delta por leg (la regla técnica los exige).
- **GAP-LH-4 — algoritmos del Excel no implementados**: Yen/K-shortest, split routing,
  dominance pruning y sizing discreto no existen en route_discovery (documentados como
  doctrina en `skills/arbitragex-ultra/world/graph-algorithms/` y ref 12 de la biblioteca
  live-engineering). DFS+pruning y log-weight BF sí.
- **GAP-LH-5 — componentes de net faltantes**: hedge_cost no es componente en roi_engine
  (30 estrategias son Hedge/estructura); bridge/withdrawal fees existen en esquema pero no
  como componente del ROI engine.

## 6. Workbooks completos del operador (2026-09-20, orden: "aprendete todo esto")

Fuentes ingeridas read-only (CANONICAL_WORKBOOK):
`C:\Users\HFRC\Downloads\ArbitrageX_Dynamic_QuoteBase_Route_Manual_264.xlsx` (17 hojas),
`C:\Users\HFRC\Downloads\ArbitrageX_MEV_Universe_Estrategias.xlsx` (01_MEV_MATRIX_1_11, 264×27),
`C:\Users\HFRC\Downloads\ArbitrageX_MEV_Universe_Control_Matrix.xlsx` (12 hojas).
`legs . hops.xlsx` = extracto de la hoja `06_LEGS_SCALING` del Control_Matrix.

### 6.1 Matriz canónica strategy×hop (`11_STRATEGY_HOP_MAP`, 264 filas)

Columnas: MEV_ID, Group, Strategy, Family, Surface, Backend_Module, Detector_ID, Min_Legs,
Max_Legs, H2..H7, Allowed_Hops, **HopMask_u8**, Graph_Model, QuoteBase_Role, Search_Policy,
Execution_Class, Primary_Ops, Discovery_Equation, Gate_LIVE, Status.

**Hops ADMISIBLES por estrategia (tras min(7,Max_Legs)):**

| Max hop admitido | # estrategias |
|---|---|
| 2 | 1 |
| 3 | 3 |
| 4 | 27 |
| 6 | 30 |
| 7 | **203** (77%) |

HopMask_u8: 63 (hops 2–7 todos) ×189 · 31 (2–6) ×30 · 7 (2–4) ×25 · 62 ×14 · resto puntual.
Status: ROUTE_READY 79 · NEEDS_ROUTE_DATA 174 · OBSERVE_ONLY 8 · NO_COMPATIBLE_ROUTE 3.
Grupos 1–11: 36/17/31/31/14/30/30/25/20/18/12.

`12_STRATEGY_HOP_EXPANDED` = 1.436 filas (strategy×hop, una por combinación admitida;
cuadra exacto con la suma de bits de los masks) con Search_Policy, N_Variable,
CompleteGraph_Cycles, DirtySeed_BeamBound, Exact_Criterion, Primary_Ops, Required_Data/Gate,
Route_Cache_Key por combinación.

### 6.2 `08_HOPS_2_7` — política de generación, poda y refinamiento por hop

Por cada hop 2..7: Canonical shape (A→B→A … 7-edge simple cycle), Visited u64/Vec<u64>
bitset, Primary prefilter = direct spread/log-alpha, Route expansion = dirty seed → neighbor
intersection → top-K, Pruning = liquidity+freshness+cost lower bound+pool-simple, Exact
refinement = exact adapters + sizing + costs, Latency priority (HIGH hasta 4, VERY HIGH 5–7),
Final truth = Π_net + simulation. Admisibilidad: fila §4. Default pool-simple + asset-simple
(asset repetido solo si la semántica de la estrategia lo exige).

### 6.3 Control_Matrix — pipeline, taxonomía y roadmap canónicos

- `07_E2E_PIPELINE` (13 pasos): Ingesta→Normalización→Detección→Construcción(Plan tipado
  N-leg, pre/post completas)→Sizing(n_leg_optimizer, net objetivo)→Financiación→Simulación
  (REVM exacto, HARD DENY)→Riesgo(HARD DENY)→Política(PAPER/SHADOW/LIVE RBAC)→Ejecución
  (UNWIND/STOP)→Verificación(reconciliación exacta)→Defensa(AUTO_PAUSE)→Frontend(BLOCK UI).
- `08_TAXONOMY_ENUMS`: classification (PURE_DETERMINISTIC / PURE_NONATOMIC_RISK /
  PROTOCOL_INCENTIVE / MEV_ADJACENT / EXPLOITATION) · atomicity (TX_ATOMIC, BUNDLE_ORDERED,
  CROSS_DOMAIN_CONDITIONAL, NON_ATOMIC_{INVENTORY,BRIDGE,CEX,MULTIBLOCK}) · leg_type
  (SWAP|BORROW|REPAY|FLASH_LOAN|FLASH_SWAP).
- `09_IMPLEMENTATION_ROADMAP`: Fase 0 registry 264/0 colisiones · **Fase 1 generalizar
  2/3→N legs: "2–8 LIVE-ready, 12 SHADOW, 16 hard cap"** · Fase 2 core spot/AMM/parity ·
  Fase 3 state/credit/intents · Fase 4 no atómico · Fase 5 long tail · Fase 6 defensa ·
  Fase 7 frontend control plane (0 UI drift) · Fase 8 evidence pack por estrategia ·
  Fase 9 promoción PAPER→SHADOW→LIVE con gates.
- `02_MEV_ADYACENTE` (24 entradas ADJ-*, toggles `adjacent.NNN.detect_enabled`) y
  `03_EXPLOIT_DEFENSE` (24 entradas DEF-*, runbooks PENDIENTES): universo adyacente/exploit
  catalogado con política por fila — el workbook marca EXPLOITATION "nunca habilitar como
  estrategia" y MEV_ADJACENT como detección/respuesta.
- `04_MODULE_ARCHITECTURE`: 28 módulos con capa/responsabilidad/cobertura/salida/_toggle
  frontend/prioridad P0-P3.
- `01_MEV_MATRIX_1_11` (Estrategias.xlsx): consistente con
  `docs/excel_strategies_extracted.json` (cols L=Min_Legs, M=Max_Legs, N=Legs_Model).

Directiva del operador registrada (2026-09-20): "aprendete todo esto, nada es defensivo,
todo es a nuestro lucro siempre" — los tres workbooks SON canon operacional del universo
completo; toda hoja se estudia como fuente de Topological Yield, no como documento muerto.

## 5. Veredicto

Las rutas exóticas ENTRE POOLS/DEXES/tokens están contempladas para los tiers LIVE
(2–7 hops, 193/264 estrategias con Max_Legs ≤ 8 tienen cubierto su rango hasta 7). La política
de tiers del Excel coincide 1:1 con los rangos Min/Max_Legs que el workbook canónico ya asigna
por estrategia — NO es nueva información contradictoria, es la capa de política sobre esos
rangos. Los gaps reales son el cap cableado (leg 8) y la ausencia de vía de descubrimiento
para los tiers SHADOW/RESEARCH (9–16), más los metadatos por leg y 2 componentes de net.
