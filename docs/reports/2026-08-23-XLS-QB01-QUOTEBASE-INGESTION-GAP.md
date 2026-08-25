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

> **Snapshot del estado EN INGESTIÓN (2026-08-23 mañana).** El estado actual
> por entregable vive en §4 — varias ❌/🟡 de esta tabla ya tienen módulo
> entregado (PairIndex, StrategyHopMask, dirty-pairs, QuoteScore, lat.*).

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

## 4. Plan incremental (PR-sized, P-∅: un PR = un ID) — ESTADO 2026-08-23

> **⚠️ SECCIÓN ANULADA (operador, 2026-08-23 — corrección obligatoria de proceso).**
> El enfoque "PR-sized / un PR por ID / branches-stack / merge en orden" de esta sección QUEDA ANULADO.
> **XLS-QB-01..07 NO son PRs, NO son branches, NO son commits, NO son deploys**: son exclusivamente
> **LOCAL WORK PACKAGES** dentro del único MASTER CHECKLIST (`.ai-work/TASK_REGISTRY.json`, 88 tareas
> atómicas). Mientras exista UNA tarea implementable pendiente: COMMIT=FALSE, PUSH=FALSE, PR=FALSE,
> MERGE=FALSE, DEPLOY=FALSE. Git aparece UNA sola vez, al final (WP-F), tras la validación integral.
> El contenido de las tablas siguientes se conserva como **registro histórico de evidencia**
> (qué módulos existen y con qué verificación), no como plan de entrega.

| ID | Entregable | Scope mínimo verificable | Estado |
|---|---|---|---|
| **XLS-QB-01** (este) | Ingesta + extracción + gap analysis + JSONs canónicos a `docs/` | 17/18 checks diferenciales PASS | ✅ db031e43 |
| XLS-QB-02 | Enums `GRAPH_MODELS` ×10 + `SURFACES` ×10 en `canonical_enums.rs` + `StrategyHopMask` tabla estática 264×u8 generada del JSON + tests (biyección con extracción, admissibility min/max legs) | cargo check + tests CI (local AppControl: clippy only) | ✅ 908ccc34 — enums 1:1 + `strategy_hop_mask.rs` (generator `scripts/xls/gen_hopmask_rs.py`, 8 tests, fixture in-crate) |
| XLS-QB-03 | Dispatch hop-aware en route_discovery (mask test O(1) antes de expandir) + knob `Min_Net_bps` + `Beam_K` en `canonical_knobs.rs` | unit tests mask-prune; sin cambio de matemática | ✅ 094589ec + fix-forward 3cf0e691 — `admissible_hop_bounds` en `route_discovery_worker` (observe-only, default MEV-01-001 mask 63 = comportamiento desplegado idéntico); knobs declarativos con roadmap de consumo; VPS `cargo check -p searcher-rs --locked` EXIT=0 |
| XLS-QB-04 | `DenseIdBuilder` + `PairIndex` + `PairBuckets` (capa densa PRESERVANDO el path HashMap actual) + property tests (inyectividad, rango, biyección edges↔buckets) | differential vs extracción §2 | ✅ 2f0604a0 — `pair_index.rs` (índice triangular O(1), decode binario, techo N!/[h(N−h)!] derivado; 7/7 tests incl. inyectividad exhaustiva N≤24). DenseIdBuilder/PairBuckets quedan como follow-up del consumidor |
| XLS-QB-05 | DirtyPair propagation (PoolToPair map + bitset + ring queue) + hot-seed queue | replay fixtures | ✅ 74ab55cf — `dirty_pairs.rs`: DirtyPairSet (bitset ceil(C(N,2)/64), version-scoped dedupe) + PoolToPair (fan-out exacto, pools paralelos colapsan) + HotSeedQueue (ring bounded, evicción observable) + DirtyPairEngine (PoolEventOutcome honesto); 16/16. Wiring al hot path de reserves = follow-up declarado |
| XLS-QB-06 | QuoteScore dinámico (5 weights como knobs) + normalización F_e como prefilter (NO proof) | fixtures de spreads conocidos | ✅ 03a9345f — `quote_score.rs` (forma lineal exacta 05 r11; fixtures USDC 96.0 / WETH 91.0 / WBTC 75.0 / LINK 55.5; sin umbral inventado — el workbook no define QuoteEligible threshold) + 5 knobs quote_w_* validados sum==1.0; 10/10. F_e normalization = follow-up (requiere QuoteState anchor) |
| XLS-QB-07 | Telemetry `lat.*` 8 stages + harness benchmark (matriz N=8..128) + KPI p50/p95 al dashboard | SLA p95 medido, no prometido | 🟡 e07d0c5f — `latency_budget.rs` (tabla 8 stages lat.* 2/3/4/3/7/5/3/2, total 29 DERIVADO, recorder p50/p95 nearest-rank, Headroom_p95 firmado, PASS_p95 vs knob `discovery_sla_ms`=30) + knob; 7/7. **Pendiente honesto**: wiring Instant en el hot path + benchmark matrix + KPI dashboard — el <30ms NO se afirma hasta medirse (fila 21 del workbook) |

**Knobs QUOTEBASE-264 absorbidos: 8/8** (`min_net_bps`, `beam_k`, 5×`quote_w_*`, `discovery_sla_ms`) — `canonical_knobs.rs` ahora 50 knobs (42 ULTRA + 8 QUOTEBASE), to_json 51 keys. Los 2 observacionales (Dirty_Seeds metric, Avg_Parallel_Pools) son telemetría de runtime, no knobs.

**Branches (stack, base = QB-04 hacia main):** `feat/xls-qb-01-quotebase-ingestion` (db031e43) ← `feat/xls-qb-02-hopmask-enums` (908ccc34) ← `feat/xls-qb-03-hop-dispatch` (3cf0e691) ← `feat/xls-qb-04-pair-index` (2f0604a0) ← `feat/xls-qb-06-quote-score` (03a9345f) ← `feat/xls-qb-05-dirty-pairs` (74ab55cf) ← `feat/xls-qb-07-latency-budget` (e07d0c5f). Todas pushed a origin; merge en orden.

Orden razona dependencia: QB-02 (datos/enum) → QB-03 (dispatch usa mask) → QB-04/05 (estructuras) → QB-06 (señal) → QB-07 (evidencia). QB-07 alimenta además la deuda de evidencia A.5 (pata latencia por etapa).

### Follow-ups declarados (consumidores runtime, cada uno su PR con ID)

1. **Wiring dirty_pairs al hot path de reserves** (QB-05b): `on_pool_event` desde el reserve-update path + `Dirty_Seeds` metric a telemetría.
2. **DenseIdBuilder + PairBuckets** (QB-04b): asignación de TokenIds densos desde TokenRegistry + buckets C(N,2) PRESERVANDO el HashMap actual.
3. **F_e normalization prefilter** (QB-06b): requiere QuoteState anchor + freshness/version (05 r19-25).
4. **lat.* wiring + benchmark matrix** (QB-07b): Instant por stage en route_discovery + matriz N=8..128 + KPIs al dashboard — PASS_p95 solo con p95 MEDIDO.
5. **Consumo de knobs declarativos**: `min_net_bps` (net gate evaluation) y `beam_k` (DFS beam con la seed queue de QB-05).
6. **Paso 2 del contrato** (ChainResolver runtime symbol→TokenKey) — sigue 🟡, clase de bug CARTRIDGE-GATE-ADDR-01.

### Registro world-layer — lo mejor del mundo integrado a los follow-ups (regla omniscience: "el Excel es el 5%")

Fuentes: `skills/arbitragex-ultra/world/{graph-algorithms,quant-math,mev-practice}/BETTER_THAN_EXCEL.md` (repo canon) + verificación web 2026-08-23. Clasificación anti-alucinación en cada ítem.

| # | Hallazgo mundial | Clase | Integración en este programa |
|---|---|---|---|
| W1 | **El shortfall medido es STALE STATE, no faltan estrategias**: 2.02 bps/pair promedio ($24M/$120B SCO vs FVO), "most of it from stale state"; staleness 1.29–1.78 bps/bloque de lag | PRIMARY_SOURCE (arXiv:2607.20762, 2502.08258; world graph-algorithms ítem 5) | **QB-05b y QB-07b son el follow-up de mayor palanca** — dirty-pair propagation + budget <30ms SON la máquina anti-staleness. El prize está cuantificado. |
| W2 | **Cycle-edge inverted indexes**: amms-rs mantiene grafo persistente con reserves actualizados in-place por evento + índice invertido edge→ciclos para re-validar SOLO los ciclos afectados (vs rebuild por scan) | CANONICAL_REPO/world (graph-algorithms ítem 7) | **QB-05c (nuevo)**: extender `DirtyPairEngine` con `CycleIndex` (pair→ciclos que lo contienen) — fan-out dirty pair→ciclos, no solo pair. El módulo actual es la capa 1 (pair-level); la capa 2 (cycle-level) es el upgrade. |
| W3 | **RICH exact-k** (color-coding + Held-Karp DP, O(2^k·|V|·|E|), 32.69× más rápido que el competidor, 0.02–3.9% error) domina DFS bounded en k≥4 (DFS 330–2194× más lento en k=6) | PRIMARY_SOURCE (VLDB 2025, vldb.org/pvldb/vol18/p4081-luo.pdf) | **QB-05b/QB-07b**: la política de expansión del hot-seed queue usa RICH (knob `enable_rich` ya canónico desde #456) para k≥4; DFS/beam solo k≤3. |
| W4 | **Split-route convex global** (MPO/G-FVO: marginal-price equalization λ*, UNA variable por token, 200× más rápido que solvers genéricos) vs path-then-size | PRIMARY_SOURCE (arXiv:2502.08258, 2607.20762) | **QB-06c (nuevo)**: los AmountBuckets del workbook (09 r11) escalan hacia split-allocation vectorial; SizeOptimizer queda como capa per-path del funnel, MPO como refinamiento final. |
| W5 | **LVR/adverse selection sin operador en los 31**: no-arbitrage band A(f;v)=(v/8)/(1+√(2λ/v)f) — el mundo modela impact como STATE STALENESS con triggers cerrados, no slippage estático | PRIMARY_SOURCE (arXiv:2505.05113, 2606.21769) | **Gate candidato post-QB**: edge computado como dislocación − banda LVR-style; conecta con W1 (stale = dentro de banda). NO se implementa en QB (fuera de scope del workbook — registrar como estrategia world nueva en NEW_STRATEGIES.md). |
| W6 | **Búsqueda web 2026-08-23**: nada más nuevo que el canon world/ para incremental cycle revalidation (RICH VLDB'25 + linaje Bender–Fineman–Gilbert–Tarjan/Bernstein = fundamento de dirty-edge propagation) | PRIMARY_SOURCE (web) | Canon world/ confirmado vigente — sin deriva. |

**Principio del operador (2026-08-23): "nada de lo desarrollado se pierde; buscar a nivel mundial lo mejor; integrar el mejor-de-ambos-mundos o el mejor mundo prevalece."** Los módulos QB-01..07 se PRESERVAN como capa Excel-canónica; W1–W6 se integran como upgrades en los follow-ups, cada uno con su PR + ID.

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
