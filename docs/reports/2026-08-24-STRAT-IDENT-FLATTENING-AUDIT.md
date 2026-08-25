# STRAT-IDENT — Auditoría: dónde el pipeline de scoring/evaluación aplana identidad de estrategia por clase

Fecha: 2026-08-24 · Sesión 7b · Trigger: directiva operador `/arbitragex-omniscience`
("auditar dónde el pipeline de scoring/evaluación aplana identidad de estrategia por clase
(strategy_kind/pair_symbol) en vez de evaluar cada una de las 264 estrategias
individualmente, cada una declarando sus estructuras (operadores) aplicables").

Clasificación de evidencia (regla anti-hallucination): todo lo citado abajo es
CANONICAL_REPO (file:line verificado esta sesión) salvo donde se marca INFERRED.
Contexto canon: knowledge_graph.jsonl (2.511 edges Strategy↔Operator↔Detector),
capability_matrix.json (265), sheets 05/07/11/13.

---

## Resumen ejecutivo

El sistema tiene DOS pipelines de detección/evaluación con asimetría INVERTIDA de
identidad, más dos sitios de aplanamiento por categoría en el camino cartridge:

- **Pipeline cartridge (route_discovery → cartridge active)**: identidad individual
  FUERTE en detección y evidencia §IV (STRAT-IDENT-01, 2026-08-23) — pero se
  aplana a CATEGORÍA (11 buckets) en el `RoutePlan` que entrega al spine evaluator,
  y el filtro de pertinencia es por categoría.
- **Pipeline core engines (dex/triangular/flashloan/…)**: identidad GRANULAR
  (label `dex_arb_v2v3`…) en el RoutePlan que entrega al spine — pero se APLANA a
  la clase contract (`dex_arb`, ~5 buckets) en `Opportunity.strategy_kind`, que es
  la llave del bucket Bayesian y de la evidencia §IV → para engines la evidencia
  per-estrategia NO existe (nadie publica esa llave).

Es decir: cada uno de los dos pipelines preserva identidad donde el otro la aplana.
Ninguno de los dos entrega identidad individual de punta a punta.

---

## A. Donde la identidad individual YA se preserva (post STRAT-IDENT-01)

| # | Sitio | Evidencia |
|---|-------|-----------|
| A1 | Loop de evaluación per-cartridge | `cartridge_boot.rs:1082` itera `(cartridge_id, category, declared_primary_ops, declared_secondary_ops)`; cada cartridge se evalúa individual (`runner.evaluate(&cartridge_id, …)` :1083) |
| A2 | Opportunity con identidad | `cartridge_boot.rs:1135` `StrategyKind::cartridge(cartridge_id)` + `:1159` `cartridge_id: Some(…)` + fingerprint `cartridge_id:tx:idx` :1212-1215 |
| A3 | Evidencia §IV declarada por estrategia | `math_evidence.rs:162-178` `publish_declared_combo_evidence` evalúa SOLO los operadores que la estrategia DECLARA (primary/secondary de `CartridgeMetadata`, espejo de STRATEGY.json); llave `arbx:math_evidence:{chain}:{cartridge_id}` :228; cita textual de la directiva del operador en :163-165 |
| A4 | Bucket Bayesian por estrategia | `opportunity_emitter.rs:431-439` strategy_key = `cartridge_id` (el par queda como contexto, nunca bucket) |
| A5 | Tier de emisión por estrategia | `signal_tier.rs:191-204` resuelve stem → MEV id → SU execution_class de la fila 11/13; fail-closed por token fuera del vocabulario |
| A6 | Telemetría shadow | `cartridge_boot.rs:615-616` rd_outcome_v2 lleva `strategy_kind = cartridge_id` y `cartridge_id` |
| A7 | Frontend | Catálogo/matriz por `mev_id` (`WorkbookStrategiesPanel.tsx:245`, `StrategyHopMatrix.tsx:88`), by-strategy agrupa por `cartridge_id` (`features/opportunities/by-strategy-grouping.ts`; `useRouteDiscoveryOutcomes.ts:48` "cartridge_id is the strategy key"). Verificado con Playwright 2026-08-24: 5 superficies 200/0 pageerror, honest-empty sin backend local (RULE 01) — `scratchpad/strat_ident_fe_report.json` |

## B. Sitios de APLANAMIENTO (el hallazgo)

### B1 — CRÍTICO: cartridges → 11 categorías en el spine evaluator
`cartridge_boot.rs:1247` construye el `RoutePlan` para `ConfigAwareEvaluator` con
`strategy_kind = format!("cartridge_{}", category)` (categoría, no stem).
`config_aware.rs:545` documenta que ese campo es "the candidate's class" y TODOS
los gates del spine lo usan como llave:
- enabled-list match `config_aware.rs:675`
- capital efectivo `:747` (`effective_capital_for(token, strategy_kind)`)
- fee EWMA `arbx:relay_fee_ewma:{chain}:{strategy_kind}` `:284/:487/:995`
- sniff flashloan por substring `contains("flash")` `:925`
- caps de simulación per-strategy `:1882`
Consecuencia: los gates económicos del spine y la estadística de fees corren en
11 buckets `cartridge_{category}` aunque la Opportunity individual conserve su stem.
La identidad granular SOBREVIVE en `Opportunity.strategy_kind` (:1135) — pero el
evaluador lee el string del RoutePlan, no la Opportunity.

### B2 — CRÍTICO: core engines → clase contract en el bucket Bayesian/evidencia
Los engines setean `Opportunity.strategy_kind = label.to_contract_strategy_kind()`
que COLAPSA los labels granulares al enum contract — test nominal:
`dex_engine.rs:1336-1351` "contract_strategy_kind_collapses_to_dex_arb"
(DexArbV2V2/V2V3/V3V2/V3V3 → un solo DexArb; ídem triangular :1374, flashloan :810).
Consecuencias en cadena:
1. Fallback de bucket `opportunity_emitter.rs:436-439` usa `opp.strategy_kind.as_str()`
   = `dex_arb` → TODAS las ~16 variantes dex comparten UN bucket Bayesian.
2. Llave de evidencia `arbx:math_evidence:{chain}:dex_arb` — la escribe NADIE
   (publish :1190 sólo por cartridge_id) → `evidence_vector` estructuralmente null
   para toda opp de engine (honesto R8, pero §IV per-estrategia jamás computado
   en ese camino).
3. Asimetría invertida vs B1: el RoutePlan de engines SÍ lleva el label granular
   (`dex_engine.rs:790` `label.as_str()`) → granular en el spine, colapsado en
   Bayesian/§IV. Los cartridges son exactamente al revés (B1).

### B3 — MEDIO: pertinencia por categoría, no por declaración propia
`cartridge_boot.rs:1009-1011` (active) y `:843-847` (shadow): el filtro de qué
cartridges se evalúan es `cartridge_matches_intent(category, &intent)` — llave
CATEGORÍA. Un cartridge cuyas estructuras declaradas difieran de la forma de su
familia jamás se vuelve pertinente. STRAT-IDENT-01 lo declara en el comentario
(:977-980 "no class-level flattening") pero el filtro previo SÍ es class-level.

### B4 — MEDIO-ALTO: aplicabilidad de ruta declarada por FAMILIA
`route_discovery/strategy_applicability.rs:15-22`: los perfiles de qué RouteKind
acepta / target_engines / gates extra existen para 5 estrategias coarse (:33-39)
y 11 FAMILIAS (:44-56) — "Each gets a FAMILY profile". La declaración per-estrategia
existe HOY sólo para OPERADORES (A3); las ESTRUCTURAS DE RUTA (acepta/gates) NO se
declaran por cartridge. La pregunta del operador ("cada una declarando sus
estructuras aplicables") está resuelta para operadores y pendiente para rutas.

### B5 — BAJO (documentado, by-design): dispatch key de shape dentro de Rhai
`cartridge_boot.rs:316-330`: `pool_data["strategy_kind"]` = clase de FORMA de ruta
(classify_route_legs), clonado a todos los cartridges (:1083) — WIRE-1/PR-ROUTE-03,
documentado. El pack se despacha por forma; cada cartridge sigue auto-identificándose.

### B6 — VERIFICADO-NEGATIVO: pair_symbol NO es llave de scoring
Busqué mapas/dedupe/agrupación por `pair_symbol` en scanner/orchestrator/workers:
0 hits. El par es contexto de registro (`opportunity_emitter.rs:487` "token_pair")
y agrupación display FE. Post-STRAT-IDENT-01 no queda bucket por par en el camino
de emisión. (INFERRED-NEGATIVO de los greps efectuados, no prueba universal.)

### B7 — Contexto: sizing soporta per-estrategia
`SizeOptimizer` tiene `strategy_configs: HashMap` y `simulation_per_strategy_caps_usd`
(sólo referenciados en tests :1750/:1766 — vacíos allí); la resolución real pasa por
`effective_capital_for(token, strategy_kind)` del spine (:747) → la granularidad de
sizing ES la granularidad del string que reciba (label granular para engines = OK;
`cartridge_{category}` para cartridges = B1 aplica también a caps de capital).

## C. Recomendaciones (orden por impacto, NO ejecutadas — audit read-only)

1. **B1**: construir el RoutePlan del cartridge con `strategy_kind = cartridge_id`
   (o añadir campo granular que config_aware use para gates/caps/EWMA) — un cambio
   de una línea en :1247 rekeya 5 llaves del spine; requiere migrar
   `enabled_strategies` y `relay_fee_ewma` existentes (warm-start por categoría →
   fan-out por stem).
2. **B2**: publicar declared-combo evidence también para los labels de engines
   (operator declaration source: workbook sheet de la familia → los labels no tienen
   STRATEGY.json propio; decisión de diseño: ¿los 16 labels son "estrategias" o
   variantes de una?) y/o usar `route_plan.strategy_kind` (granular) como bucket
   Bayesian en el fallback del emitter.
3. **B3/B4**: mover acepta/gates de familia → per-cartridge en el mismo sitio donde
   ya viven los operadores declarados (`CartridgeMetadata`), y filtrar pertinencia
   por la declaración propia.

## D. Artefactos de esta auditoría
- FE pass Playwright: `scratchpad/strat_ident_fe_report.json` + screenshots
  `scratchpad/strat_ident_shots/` (5 superficies, 0 pageerror).
- Diagrama del mecanismo: entregado como artifact HTML (ver chat de sesión).
