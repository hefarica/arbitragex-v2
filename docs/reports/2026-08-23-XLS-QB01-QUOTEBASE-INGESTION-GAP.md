# XLS-QB01 — Workbook QUOTEBASE-264: ingesta, extracción canónica y gap analysis

> **Workbook #5 del programa de absorción Excel.** `ArbitrageX_Dynamic_QuoteBase_Route_Manual_264.xlsx`
> (258,262 bytes, 2026-08-23 07:42). Los 4 previos (ULTRA, MASTER_LIVE, MASTER_BA, CART_MATH —
> 47 hojas / 534K celdas en `docs/coverage_manifest.json`) están absorbidos: **441 reqs, 438 VERIFIED
> (99.32%)** tras XLS-DOCTRINE-01 #455; XLS-ENUM-01 #456 cerró enums; XLS-GRAPH-01 (branch
> `feat/xls-graph-01-hot-token`, commit `fdd1bf40`) cierra el último PARTIAL (REQ-GRAPH-ELIGIBILITY).
> Este manual es una **capa nueva** — el contrato del motor dinámico N — no cubierta por el registry actual.

## 1. Ingesta (manifiesto + dump)

Fuente: `C:\Users\HFRC\Downloads\ArbitrageX_Dynamic_QuoteBase_Route_Manual_264.xlsx` (258,262 bytes, 2026-08-23 07:42).
Scripts: `scripts/xls/ingest_quotebase_264.py` (manifiesto), `scripts/xls/extract_quotebase_data.py` (datos).

| Hoja | Dims | Celdas | Fórmulas | Contenido canónico |
|---|---|---|---|---|
| 00_MANUAL | 27×10 | 75 | 0 | pipeline canónico 5 etapas, reglas no-hardcode/identidad/invalidación |
| 01_CONFIG | 21×6 | 109 | 1 | **17 knobs** con runtime binding |
| 02_ALLOWED_SYMBOLS | 262×8 | 1,100 | 0 | allowlist dinámica (muestra 22 = "Current UI sample", count por fórmula) |
| 03_CHAIN_REGISTRY | 134×17 | 19 | 512 | registry TokenKey=(chain,address) — template |
| 04_INDEX_MATH | 31×11 | 130 | 53 | PairIndex + techo combinatorio hops 2–7 + cross-chain |
| 05_QUOTE_BASE | 25×13 | 98 | 16 | QuoteScore + normalización F_e + runtime objects |
| 06_EDGE_MATH | 47×16 | 182 | 160 | adapter contract por protocolo (CPMM…NFT) |
| 07_INEFFICIENCY | 76×22 | 752 | 448 | tabla candidato con descomposición gross/net + PASS |
| 08_HOPS_2_7 | 15×12 | 92 | 12 | política por hop (shape, bounds, pruning, prioridad) |
| 09_RUNTIME_STRUCTURES | 22×8 | 114 | 0 | 12 estructuras hot-path con complejidad objetivo |
| 10_LATENCY | 21×9 | 66 | 12 | presupuesto 7 etapas + telemetry keys `lat.*` |
| 11_STRATEGY_HOP_MAP | 267×27 | 7,156 | 0 | **264 estrategias** × 27 cols, trazabilidad a filas ULTRA |
| 12_STRATEGY_HOP_EXPANDED | 1,439×18 | 25,867 | 1,996 | **1,436 combos válidos** Strategy×Hop |
| 13_DETECTOR_POLICY | 63×14 | 855 | 0 | **60 detectores** con política exacta |
| 14_RESEARCH | 11×6 | 55 | 0 | 8 referencias (SOR, ParaSwap, RICH, CFMM convex, MPO, Roaring) |
| 15_IMPLEMENTATION_CONTRACT | 29×7 | 130 | 0 | **15 pasos** input→candidato + Definition of Done |
| 16_COVERAGE | 15×8 | 66 | 9 | auto-coverage del manual (todo PASS interno) |

**Total: 17 hojas, 36,806 celdas con valor (43,487 escaneadas), 3,219 fórmulas.** Manifiesto: `docs/quotebase_ingestion_manifest.json`.

## 2. Extracción canónica + validación diferencial (17/18 PASS)

Artefactos (canónicos en `docs/`): `quotebase_config.json` (17 knobs),
`quotebase_strategy_hop_map.json` (264), `quotebase_strategy_hop_expanded.json` (1,436),
`quotebase_detector_policy.json` (60), `quotebase_research.json` (8),
`quotebase_extraction_checks.json` (18 checks). Generadores: `scripts/xls/`.

| Check diferencial (Excel ↔ código) | Resultado |
|---|---|
| Estrategias == 264 | ✅ |
| `HopMask_u8` recomputado (bit h−2 ⇔ H{h}=True) vs columna | ✅ **264/264 exacto** |
| Distribución por hop == 16_COVERAGE (245/262/260/233/233/203) | ✅ 6/6 exacto |
| Combos expandidos == 1,436 == Σ distribución | ✅ |
| Biyección mapa(True) ↔ expandido(MEV_ID,hop) | ✅ 0 diferencias en ambos sentidos |
| Detectores == 60; Detector_IDs del mapa ⊆ política | ✅ |
| `PairIndex(3,17,22) == 73` (ejemplo trabajado 04_INDEX_MATH) | ✅ |
| Inyectividad PairIndex N=22: 231 pares, 0 colisiones, rango [0,230] | ✅ |
| Referencias == 8 | ✅ |
| Knobs == 18 | ❌ → **17 reales** (mi supuesto erróneo; extracción completa) |

Encoding confirmado del `HopMask_u8`: bit `h−2` para `h∈[2,7]`; mask 63 = todos los hops.

### Metadatos descubiertos (distribuciones exactas, fuente de verdad para runtime)

- **Surfaces (10)**: DEX_AMM 53 · DEX_STATE 31 · PARITY_REDEMPTION 31 · CEX_DEX 14 · CROSS_CHAIN 30 ·
  DERIVATIVES 30 · LENDING 25 · INTENT_AUCTION 20 · NFT 18 · PREDICTION 12.
- **Graph_Models (10, 1:1 con surfaces)**: TOKEN_MULTIGRAPH · TOKEN_MULTIGRAPH_EVENT ·
  TOKEN_ACTION_GRAPH · HYBRID_MARKET_GRAPH · DOMAIN_MULTIGRAPH · INSTRUMENT_ACTION_GRAPH ·
  POSITION_ACTION_GRAPH · ORDER_HYPERGRAPH · ASSET_ACTION_GRAPH · CLAIM_ACTION_GRAPH.
- **Status (honestidad del manual)**: ROUTE_READY **79** · NEEDS_ROUTE_DATA **174** ·
  OBSERVE_ONLY 8 · NO_COMPATIBLE_ROUTE 3.
- **Execution_Class: 29 clases** (DETERMINISTIC_EXECUTABLE 37, DERIVATIVE_DATA_REQUIRED 30, …).

## 3. Gap analysis — contrato de 15 pasos (hoja 15) vs repo

Leyenda: ✅ PRESERVED (existe, evidencia) · 🟡 PARTIAL · ❌ GAP.

| Paso | Requisito del manual | Estado | Evidencia repo |
|---|---|---|---|
| 1 | AllowedListParser (UI→symbols, config version) | ✅ | `TokenAllowlistTab.tsx` + PUT `/admin/trading-config/:chain_id` (zod, dedupe, ≤64) |
| 2 | ChainResolver (symbol→address por chain, rechazar binding symbol-only) | 🟡 | `tokens` table (chain_id,address,symbol) + token-enricher; **runtime config sigue SYMBOL-keyed** (`allowed_token_symbols`) — la clase de bug de CARTRIDGE-GATE-ADDR-01 (#449 sin merge) |
| 3 | DenseIdBuilder (TokenId 0..N−1 + allowed masks) | ❌ | No hay IDs densos; `TokenGraph.adjacency: HashMap<Address, Vec<usize>>` |
| 4 | PairIndexBuilder (C(N,2) buckets, índice triangular O(1)) | ❌ | Inexistente; fórmula ya validada diferencialmente (§2) |
| 5 | Ingesta pool/action con pools paralelos preservados | ✅ | `graph_builder.rs` — `RouteEdge` por pool (`XLS-GRAPH-01` añadió `hot_token` por edge) |
| 6 | Adjacency dense bitsets/CSR | 🟡 | Semántica correcta (HashMap<Address,Vec>), no densa; OK para N actual |
| 7 | EventUpdate → DirtyQueue (par/edge exacto) | 🟡 | Estado event-driven existe (reserves, price stream G-PRICE-1); **no hay dirty-pair queue** |
| 8 | PairPrefilter (rates ejecutables por bucket, log-alpha) | 🟡 | SizeOptimizer sondea amounts; sin F_e normalizado ni buckets por estrategia |
| 9 | StrategyMask (detector∩surface∩hop) | 🟡 | detector/surface gating existe (prioritization-spine, cartridges); **hop mask ❌** |
| 10 | RouteExpand (DFS/beam/RICH/BF/k-shortest por detector) | 🟡 | multi_hop_search, cycle_enumerator, unique_route_finder, knobs enable_rich/bfm/mmbf/johnson (#456); **beam_k ❌** |
| 11 | ExactQuote (matemática entera por protocolo) | ✅ | math-engine + QuoterV2 V3 + simulator-v2 revm (G-SIM-1 7/7) |
| 12 | Sizing (Newton/Golden/convex/MPO) | ✅ | SizeOptimizer (motor económico vivo) |
| 13 | NetGate (gas+flash+tip+fees+risk+freshness) | 🟡 | net gate existe (G-ECON, min_profit_usd, gas floor honesto); **Min_Net_bps=5 ❌** |
| 14 | Simulation (fork/relay, FUERA del <30ms) | ✅ | simulator-v2 + A.4 fork validation PASS 2026-08-20 |
| 15 | Telemetry p50/p95/p99 por etapa (`lat.*`) | ❌ | Solo `execution_time_ms` ledger (LATLED-01 #451 desde hoy); sin `lat.decode/state/reprice/pair/expand/refine/gates/emit` |

### Gaps transversales

| Requisito | Estado | Nota |
|---|---|---|
| Knobs del manual (17) | 🟡 | 3 ya existen (`max_hops`, `min_hops`, `min_pool_liquidity_usd` en `canonical_knobs.rs`); `max_freshness_s` ≈ `Max_State_Age_Blocks` (unidad s vs blocks); **~8 nuevos**: Beam_K, Dirty_Seeds, Min_Net_bps, 5×quote weights, Discovery_SLA_ms (+2 observacionales) |
| Enums GRAPH_MODELS ×10 + SURFACES ×10 | ❌ | `canonical_enums.rs` tiene ALGORITHMS ×7 + DEXES ×6 (#456); falta esta pareja 1:1 |
| StrategyHopMask runtime (264×u8, test O(1)) | ❌ | Datos ya extraídos canónicos; falta tabla estática + admissibility `h∈[max(2,min),min(7,max)]` |
| QuoteScore dinámico + quote_version | ❌ | Fórmula 0.3P+0.3L+0.2V+0.1S+0.1C del manual; hoy quote fijo implícito |
| Presupuesto <30ms con p95 demostrado | ❌ | SLA definido (29ms sobre 7 etapas); sin harness de benchmark |
| Status/Execution_Class en dispatch | ❌ | Solo 79/264 ROUTE_READY — el dispatch runtime debe respetar Status honesto (174 NEEDS_ROUTE_DATA no generan rutas) |

## 4. Plan incremental (PR-sized, P-∅: un PR = un ID)

| ID | Entregable | Scope mínimo verificable |
|---|---|---|
| **XLS-QB-01** (este) | Ingesta + extracción + gap analysis + JSONs canónicos a `docs/` | 17/18 checks diferenciales PASS |
| XLS-QB-02 | Enums `GRAPH_MODELS` ×10 + `SURFACES` ×10 en `canonical_enums.rs` + `StrategyHopMask` tabla estática 264×u8 generada del JSON + tests (biyección con extracción, admissibility min/max legs) | cargo check + tests CI (local AppControl: clippy only) |
| XLS-QB-03 | Dispatch hop-aware en route_discovery (mask test O(1) antes de expandir) + knob `Min_Net_bps` + `Beam_K` en `canonical_knobs.rs` | unit tests mask-prune; sin cambio de matemática |
| XLS-QB-04 | `DenseIdBuilder` + `PairIndex` + `PairBuckets` (capa densa PRESERVANDO el path HashMap actual) + property tests (inyectividad, rango, biyección edges↔buckets) | differential vs extracción §2 |
| XLS-QB-05 | DirtyPair propagation (PoolToPair map + bitset + ring queue) + hot-seed queue | replay fixtures |
| XLS-QB-06 | QuoteScore dinámico (5 weights como knobs) + normalización F_e como prefilter (NO proof) | fixtures de spreads conocidos |
| XLS-QB-07 | Telemetry `lat.*` 8 stages + harness benchmark (matriz N=8..128) + KPI p50/p95 al dashboard | SLA p95 medido, no prometido |

Orden razona dependencia: QB-02 (datos/enum) → QB-03 (dispatch usa mask) → QB-04/05 (estructuras) → QB-06 (señal) → QB-07 (evidencia). QB-07 alimenta además la deuda de evidencia A.5 (pata latencia por etapa).

## 5. Definition of Done de este engine (hoja 15, filas 22–29) — estado inicial

| DoD | Estado |
|---|---|
| Coverage: 264 mapeadas, 1.436 generadas, 60 detectores | Datos ✅ extraídos y validados; runtime ⏳ (QB-02+) |
| Dynamicity: sin constantes 22/231/462 en lógica runtime | ✅ ya cierto en repo (allowlist es data, hoy 5 tokens por decisión operador) |
| Correctness: symbol nunca runtime key; pools paralelos preservados | 🟡 edges key por Address ✅; **config gate aún symbol** (paso 2) |
| Performance: sin RPC/rebuild global en hot path | 🟡 parcial (dirty propagation = QB-05) |
| Profit truth: señales son prefiltro; PASS = net exacto amount-aware | ✅ doctrina G-ECON/SizeOptimizer ya así |
| Strategy truth: cada MEV_ID conserva equation/ops/surface/gate/hops | 🟡 capability_matrix + STRATEGY.json existentes; hops ⏳ QB-02 |
| Benchmark: <30ms PASS solo con p95 medido | ❌ pendiente QB-07 |
| Safety: LIVE gated; el manual no cambia modos | ✅ §34 intacto |

## 6. Trazabilidad

- Requirements nuevos propuestos: **REQ-QB-001..015** (los 15 pasos), **REQ-QB-KNOBS** (17),
  **REQ-QB-ENUMS** (2 pares), **REQ-QB-HOPMAP** (264), **REQ-QB-COMBOS** (1,436),
  **REQ-QB-DETECTORS** (60) — a registrar en el registry del programa (441 reqs ULTRA) al mergear QB-01.
- Cada fila de 11_STRATEGY_HOP_MAP trae `Source_Row_11`/`Source_Row_14` apuntando a
  `11_STRATEGY_CATALOG`/`14_STRATEGY_TEMPLATES` del ULTRA — cadena Excel→Excel→repo intacta.
