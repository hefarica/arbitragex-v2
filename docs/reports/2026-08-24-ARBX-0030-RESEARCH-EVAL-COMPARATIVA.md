# ARBX-0030 — Eval comparativa 14_RESEARCH (8 refs) vs implementación actual

**Task:** ARBX-0030 (WP-TW / RESEARCH) · **AC:** "cada técnica (SOR/ParaSwap/RICH/CFMM-convex/MPO/Roaring)
evaluada; integrar solo si supera (el Excel es el 5%)". · **Fecha:** 2026-08-24 · **Owner:** d9
**Input canónico:** `docs/quotebase_research.json` (extracción QB-01, 8 refs, chequeadas 2026-08).
**Baseline evaluado:** árbol `feat/xls-qb-07-latency-budget` con QB-01..07 + EMIT-09 + ARBX-0007/0009
+ QB-05a/b entregados. Cada veredicto cita file:line del repo (verificable por lectura) y clasificación
anti-alucinación (CANONICAL_REPO / PRIMARY_SOURCE / INFERRED).

---

## 0. Baseline actual (contra el que se mide "supera")

| Capacidad | Implementación hoy | Ancla |
|---|---|---|
| Pruning de candidatos | DFS acotado + `rank_parallel_pools` top-k por par + semillas `base_tokens` + boost `hot_token` | `unique_route_finder.rs:145` (fn), aplicado en el DFS del mismo módulo |
| Dispatch hop-aware | `admissible_hop_bounds` (mask O(1) antes de expandir, QB-03) | `strategy_hop_mask.rs:325`, espejo runtime `route_discovery_worker.rs:407` |
| Estado en RAM | `TokenGraph` adjacency (HashMap; `dense: None` en producción) | `graph_builder.rs` |
| Propagación dirty (event-driven) | `DirtyPairSet` bitset + `PoolToPair` fan-out + `HotSeedQueue` + `DirtyPairEngine` (QB-05) + consumidor 1-drain/tick (QB-05b) + señal cross-worker | `dirty_pairs.rs:37` (struct), `dirty_consumer.rs`, `dirty_signal.rs` |
| Ciclos negativos multihop | DFS exhaustivo observe-only con cap de emisión (sin pruning por costo) | `multi_hop_search.rs:106` (fn `find_profitable_cycles`) |
| Sizing | Golden-section 2-leg exacto + `bucket_sweep` N-buckets (ARBX-0012) per-path | `size_optimizer.rs:1484` + `size_optimizer.rs:1589`, `amount_buckets.rs:79` |
| Costo/gas en ranking | net = gross − gas − flash − other; orden determinista (ARBX-0009) | `net_bps_ranking.rs` |
| Verdad de PASS | Simulación exacta amount-aware (REVM multistep) — la señal es prefiltro | `sim-core`, doctrina G-ECON |
| Sets densos | Bitset u64 sobre C(N,2) con ids triangulares O(1) (QB-04) | `pair_index.rs` |
| SLA | Presupuesto declarado 30ms p95 + gate p95-medido (QB-07/07-007); bench matriz OAT (07-006) | `latency_budget.rs`, `discovery_workload.rs`, `benches/discovery_matrix.rs` |

**Criterio de superación (el mismo para las 8):** una técnica se integra SOLO si (a) mejora una etapa
medible en el harness 07-006 (stages_ms) o el gate 07-007 SIN (b) romper la verdad exacta de PASS
(prefiltro ≠ prueba) NI (c) violar FUSILE_SOURCE_POLICY (port-with-validation, no copy).

---

## 1. Veredictos por técnica

### 1.1 Uniswap Smart Order Router (SOR) — `github.com/Uniswap/smart-order-router` · CANONICAL_REPO/world
- **Técnica:** heurísticas de candidate-pool (top-N/direct/base-token/second-hop); splits como
  trade-off latencia↔calidad; caché de rutas.
- **Eval:** el núcleo (evitar explorar todo pool/todo path) YA está: top-k por par (`max_pools_per_pair`),
  semillas hot, máscara hop. Los splits multicamino NO aplican al funnel del workbook (single-path por
  candidato con preservación de pools paralelos como aristas — la dimensión split es W4, ver 1.6).
  El GAP real es la **caché de rutas entre ticks**: hoy `find_routes` reconstruye por tick; con
  dirty-propagation una ruta sigue válida hasta que un par que la toca ensucia (re-validación por
  evento, no por scan).
- **Veredicto: PARCIAL — SIN integración hot-path nueva.** La caché-ingresada-por-dirty ES la capa 2
  del registro world (W2: índice invertido par→ciclos). Se integra como **QB-05c (CycleIndex)**,
  no como port de SOR (su licencia/arquitectura Node no cabe; port-with-validation innecesario:
  la pieza faltante es un índice nuestro de 60 líneas sobre `DirtyPairEngine`).
- **Supera?** La caché sí (elimina re-scan del 95% de pares limpios — W1 cuantifica el premio:
  staleness 1.29–1.78 bps/bloque). El resto ya está.

### 1.2 Velora/ParaSwap DexLib — `github.com/VeloraDEX/paraswap-dex-lib` · CANONICAL_REPO/world
- **Técnica:** pricing event-based: Multicall inicial, suscripción a eventos, estado en RAM, quote
  sin RPC.
- **Eval:** es la ARQUITECTURA hacia la que el repo ya converge: TokenGraph en RAM + dirty signal +
  drain por tick. El riesgo residual es **RPC en el camino de pricing**: `v3_quote_provider` hace
  lecturas QuoterV2 on-demand para pares mixtos V3 (fix J-5) — eso es RPC-in-path, el anti-patrón
  DexLib. El SLA <30ms (QB-07) no se sostiene con QuoterV2 por pierna.
- **Veredicto: CONFIRMA DIRECCIÓN — integración = RETIRAR el RPC del tick.** Follow-up existente:
  QB-07b (wiring lat.*) + uno nuevo declarado: **proyección local V3 (tick-data math con estado
  simulado vía `state_projector`)** como señal, QuoterV2 relegado a calibración fría. NO se porta
  DexLib (TypeScript, licencia y superficie ajenas — FUSILE_SOURCE_POLICY tier); se adopta el
  PATRÓN (inicialización + eventos + quote sin RPC), que ya es doctrina repo.
- **Supera?** Como patrón, sí — medible en stages_ms.find del bench cuando el pricing V3 pase a
  local. Como código portado, no se evalúa (prohibido copiar ciego).

### 1.3 RICH (PVLDB 2025) — `vldb.org/pvldb/vol18/p4081-luo.pdf` · PRIMARY_SOURCE (W3)
- **Técnica:** color-coding + Held-Karp DP con color-sets bit a bit para el ciclo más negativo
  acotado en k hops; O(2^k·|V|·|E|); 32.69× vs competidor; error 0.02–3.9%.
- **Eval:** el DFS exhaustivo actual es EXACTO y barato en k≤3 (el bench 07-006 lo mide — base
  hop=3). En k≥4 el espacio explota (W3: DFS 330–2194× más lento en k=6). RICH es aproximado
  (error declarado) ⇒ como TODO detector no califica (verdad exacta), pero como GENERADOR DE
  CANDIDATOS priorizados para el exact-net gate posterior sí: el error de RICH se elimina con la
  simulación final (señal≠prueba).
- **Veredicto: INTEGRAR para k≥4 como política de expansión del hot-seed (knob `enable_rich` ya
  canónico).** DFS/beam permanece k≤3. Follow-up: QB-05b/07b (registro W3). El bench 07-006 ya
  trae el eje hop {2..7} — la comparación RICH-vs-DFS en k∈{4,5,6} es medible AHÍ cuando el
  port exista.
- **Supera?** En k≥4, sí (32.69× PRIMARY_SOURCE + medible en harness). En k≤3, no (exactitud +
  simplicidad del DFS ganan).

### 1.4 Optimal Routing for CFMMs (convexidad sin costos fijos) — `arxiv.org/abs/2204.05238` · PRIMARY_SOURCE
- **Técnica:** routing/arb en CFMMs = optimización convexa SIN costos fijos de ejecución.
- **Eval:** nuestro problema TIENE costo fijo (gas + flash fee entran al net, ARBX-0009/G-ECON:
  48% de net=0 eran `gas_floor_breach` honestos) ⇒ la convexidad estricta NO aplica al problema
  completo. Para el sub-problema alocaión-continua-sin-gas sí. El workbook mismo lo acota:
  "differential/offline validation; not necessarily hot path".
- **Veredicto: NO hot-path. INTEGRAR como validador diferencial OFFLINE:** un test/propiedad que
  compare la respuesta de `golden_section_search_2leg`/`bucket_sweep` contra una referencia
  convexa (mismo sub-problema sin gas) en fixtures — barandilla de regresión del sizing, no
  runtime. Follow-up declarado (nuevo, pequeño): **differential-sizing-validator**.
- **Supera?** En hot path NO (costos fijos + latencia). Como validador offline sí aporta
  (seguridad de regresión sin costo runtime).

### 1.5 Efficient CFMM Routing (descomposición heterogénea) — `arxiv.org/abs/2302.04938` · PRIMARY_SOURCE
- **Técnica:** método de descomposición para routing CFMM heterogéneo (protocolos mezclados).
- **Eval:** la heterogeneidad V2/V3 es real en el repo (pares mixtos, fix J-5). La descomposición
  opera sobre SPLIT de flujo entre pools — capa que hoy no existe (per-path solamente). Es la
  pata "heterogéneo" de W4: sin la capa de alocación vectorial (1.6) no hay dónde descomponer.
- **Veredicto: DIFERIDO-DEPENDIENTE — se evalúa DENTRO de W4/QB-06c cuando la capa de split
  exista; el workbook lo aplica a "refine selected routes / split flow across pools".** Sin
  integración inmediata.
- **Supera?** No evaluable aún (falta la capa); criterio quedará: stages_ms.refine + verdad
  exacta del net por split.

### 1.6 MPO (Marginal Price Optimization) — `arxiv.org/abs/2502.08258` · PRIMARY_SOURCE (W4)
- **Técnica:** formulación por frontera de precio marginal (root-finding); hasta 200× vs Clarabel;
  UNA variable por token (equalización λ*).
- **Eval:** el refine actual (`bucket_sweep` N-buckets por path) es grid; MPO equaliza precios
  marginales con una variable por token ⇒ split-allocation vectorial natural. Encaja EXACTO en el
  funnel como refinamiento final de los Beam_K finalistas (topología ya conocida — caveat propio
  del workbook: "use after route topology is known, with exact protocol adapters").
- **Veredicto: INTEGRAR como upgrade del refine (QB-06c, registro W4), GATEADO por medición:**
  debe superar a `bucket_sweep` en stages_ms.refine del harness 07-006 CON la misma verdad de
  net (mismo fixture, mismo exact-gate). Hasta entonces `bucket_sweep` permanece (no se promete,
  se mide — misma regla que el SLA).
- **Supera?** Candidato serio (200× vs solver genérico es contra Clarabel, NO contra grid — INFERRED:
  la comparación honesta es la del harness). Integración condicional = mecanismo 07-006.

### 1.7 Multi-Path Routing in DEX Networks — `arxiv.org/abs/2607.22540` · PRIMARY_SOURCE (2026, el ref más nuevo)
- **Técnica:** multigrafo dirigido de tokens; k-shortest marginales gas-aware; alocación continua;
  restricción pool-simple.
- **Eval:** su SEPARACIÓN descubrir-topología / alocación-sizing es la doctrina two-layer del repo
  (omniscience: DISCOVERY ≠ EVALUATION) — CONFIRMA el diseño. Su "gas-aware marginal k-shortest"
  ya vive distribuido: gas entra al ranking net (ARBX-0009) y al gate; k-shortest por margen ≈
  emisión acotada + beam. La alocación continua = misma capa que 1.5/1.6 (W4).
- **Veredicto: CONFIRMA DOCTRINA — sin integración de código.** Se registra como fuente de
  criterios para W4 (gas-awareness en la selección de finalistas).
- **Supera?** No introducen nada ausente; su valor es validación externa del diseño.

### 1.8 Roaring Bitmaps (roaring-rs) — `github.com/RoaringBitmap/roaring-rs` · CANONICAL_REPO/world
- **Técnica:** bitmaps comprimidos para sets dinámicos grandes/ralos.
- **Eval:** universo actual = workbook N=22 tokens ⇒ C(22,2)=231 pares, bitset denso u64 de
  4 palabras (`dirty_pairs.rs`; techo derivado en `pair_index.rs`). Roaring comprime cuando
  N>~4096 con ocupación rala; a N pequeño el contenedor denso gana (sin indirección). El propio
  workbook lo acota: "use only when graph universe becomes large+sparse".
- **Veredicto: NO AHORA — TRIGGER registrado:** si N_active supera ~4096 tokens o la ocupación
  del set de pares cae bajo ~10%, re-evaluar Roaring para `DirtyPairSet`/`CycleIndex` (misma
  interfaz, swap interno). Hasta entonces dense es correcto y más rápido.
- **Supera?** No en el régimen actual (INFERRED de las cotas arriba + benchmarks del propio
  roaring-rs, que muestran paridad-or-peor en N pequeño).

---

## 2. Tabla de decisión (resumen ejecutable)

| # | Técnica | Veredicto | Integración (ID existente/nuevo) | Gate de superación |
|---|---|---|---|---|
| 1.1 | SOR | PARCIAL: caché sí, resto ya está | QB-05c (CycleIndex, W2) | stages_ms.find ↓ con dirty-hit-rate |
| 1.2 | ParaSwap DexLib | CONFIRMA arquitectura; retirar RPC del tick | QB-07b + **nuevo: local-V3 projection** (state_projector como señal) | stages_ms.find sin QuoterV2 in-path |
| 1.3 | RICH | INTEGRAR k≥4 (candidatos, no prueba) | QB-05b/07b (W3, knob `enable_rich`) | eje hop 4-6 del harness 07-006 vs DFS |
| 1.4 | CFMM-convex | SOLO validador diferencial offline | **nuevo: differential-sizing-validator** | test de regresión verde, cero runtime |
| 1.5 | CFMM decomposition | DIFERIDO (requiere capa split) | dentro de W4/QB-06c | stages_ms.refine post-split |
| 1.6 | MPO | INTEGRAR condicionado a medir | QB-06c (W4) | superar bucket_sweep en stages_ms.refine, misma verdad net |
| 1.7 | Multi-Path 2026 | CONFIRMA doctrina two-layer | — (criterios para W4) | n/a |
| 1.8 | Roaring | NO (régimen denso N=22) | **trigger registrado** N>4096 u ocupación <10% | re-bench del swap interno |

**Integraciones INMEDIATAS de código: NINGUNA** — las tres que superan (1.1-caché, 1.3-RICH,
1.6-MPO) ya tienen follow-up con ID y gate de medición propio; las que confirman doctrina no
requieren cambio; las que no superan quedan con trigger/criterio explícito. Esto ES el AC:
"integrar solo si supera" — evaluado, con el mecanismo de medición ya construido (07-006/07-007).

## 3. Trazabilidad
- Workbook: `14_RESEARCH` 8/8 refs evaluadas (`docs/quotebase_research.json`, extracción QB-01).
- Registro world W1–W6: `docs/reports/2026-08-23-XLS-QB01-QUOTEBASE-INGESTION-GAP.md` §4 — este
  doc lo extiende con SOR/ParaSwap/Roaring/convex-offline explícitos y los gates de medición.
- Doctrina "señal≠prueba" (prefilter): omniscience §razonamiento-2 + G-ECON — RICH y MPO la
  respetan (candidatos/allocación; PASS sigue siendo net exacto simulado).
- FUSILE_SOURCE_POLICY: ningún port se ejecuta en este doc; los tres follow-ups de integración
  deberán pasar port-with-validation con license-check en sus propios PRs.

COMMITS = 0 · PUSHES = 0 · DEPLOYS = 0 (protocolo no-git-until-final-gate)
