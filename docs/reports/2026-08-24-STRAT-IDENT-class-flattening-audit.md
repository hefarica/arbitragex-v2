# AUDIT — Flattening de identidad de estrategia por clase (2026-08-24)

**Pregunta del operador:** ¿dónde el pipeline de scoring/evaluación aplana identidad
de estrategia por clase (`strategy_kind`/`pair_symbol`) en vez de evaluar cada una de
las 264 estrategias individualmente, cada una declarando sus estructuras (operadores)
aplicables?

**Modo:** §32 audit/read-only. Todo lo abajo es OBSERVADO en código/canon con file:line.
Clasificación anti-hallucination: CANONICAL_WORKBOOK / OBSERVED.

---

## Contraste canon (lo que el sistema DECLARA ser)

- **CANONICAL_WORKBOOK** `skills/arbitragex-ultra/capability_matrix.json`: 265
  estrategias, cada una con `{detector, operators[], rhai_implemented,
  excel_documented}`. Ej. MEV-01-001 → detector R_CLOSED_CYCLE, operators
  [op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30].
- **CANONICAL_WORKBOOK** `skills/arbitragex-ultra/knowledge_graph.jsonl`: 2.511
  edges tipados Strategy→Detector (DETECTED_BY) y Strategy→Operator
  (USES_PRIMARY / USES_SECONDARY).

El contrato canónico es **per-strategy**: cada estrategia declara sus operadores.

## Mapa del código — ejes que PRESERVAN identidad

1. **Detección** — OBSERVED `backend/searcher-rs/src/cartridge_boot.rs`: cada uno
   de los 264 cartuchos Rhai se evalúa individualmente. `:615-616` emite
   `"strategy_kind": cartridge_id, "cartridge_id": cartridge_id` — la identidad
   completa viaja en el wire para opps de cartucho.
2. **Scoring Gate C** — OBSERVED `backend/searcher-rs/src/opportunity_emitter.rs:431-439`
   (STRAT-IDENT-01): `strategy_key = opp.cartridge_id.clone()
   .unwrap_or_else(|| opp.strategy_kind.as_str().to_string())` — score por
   ESTRATEGIA; los 5 core engines cuentan como estrategias individuales; el par
   queda como contexto del record, jamás como bucket de calibración.
3. **Evidencia matemática §IV** — OBSERVED `backend/searcher-rs/src/math_evidence.rs:178-243`:
   `publish_declared_combo_evidence(chain_id, strategy_key, primary_operator_ids,
   secondary_operator_ids)` → `evaluate_strategy_operators` corre el registry de
   31 operadores SOLO sobre los ids declarados del cartucho (llamado desde
   `cartridge_boot.rs:1190`). La declaración canónica de operadores por estrategia
   SÍ tiene estructura de runtime (los ~8.184 pares estrategia↔operador).
   Nota de contexto: priors flat hasta la ventana A.5 (paper-shadow) — wiring
   existe, calibración pendiente.
4. **Persistencia** — OBSERVED `backend/recon/src/aggregator.rs:124-157`:
   `GROUP BY o.strategy_kind, o.chain_id` sobre `strategy_scores`. Como las filas
   de cartucho persisten strategy_kind = id de cartucho, la agregación ES
   per-cartridge para esas filas (la columna se llama "kind" pero lleva
   identidad completa). El nombre invita a leerla como clase — riesgo de drift
   interpretativo, no de datos.
5. **Paper executor** — OBSERVED `backend/api-server/src/paper/executor.ts:79-94`:
   lee `strategy_kind` del KV (lleva el id de cartucho cuando el origen es
   cartucho) — preserva.

## Mapa del código — ejes que APLANAN a clase (el hallazgo)

6. **trading-config Migration 056** — OBSERVED
   `backend/api-server/src/routes/trading-config.ts:181-183`: `strategy_configs`
   JSONB documentado in-place como **"Keyed by strategy_kind"** (el universo de
   5 clases). `Record<string, StrategyRuntimeConfig>` mecánicamente aceptaría
   claves MEV-XX-XXX, pero la doctrina del archivo y el flujo operativo (editor,
   enabled_strategies) viven en el espacio de 5.
   - **Consecuencia observada** — `backend/api-server/src/simulation/computeSimulatedNet.ts:323-362`
     (`resolveTarget`): "Priority 1: per-strategy override" hace
     `k.toLowerCase() === strategy_kind.toLowerCase()` sobre ese espacio de 5
     claves. Para un opp MEV-01-001 el needle `"mev-01-001"` no matchea NINGUNA
     clave configurada → **cae siempre a Priority 2 (simulation_tab GLOBAL)**.
     El target verdict (PASS/FAIL de `meets_target_at_cap`) de las 264 estrategias
     se computa contra pisos GLOBALES, no contra pisos por estrategia.
   - **Idem** `backend/api-server/src/simulation/tradingConfigSnapshot.ts:237-257`
     (`lookupPerStrategyCap` + cap efectivo `(token_symbol, strategy_kind)`):
     el capital cap "per-strategy" es per-CLASE en la práctica.
7. **enabled_strategies** — OBSERVED `trading-config.ts:175`:
   `z.array(z.string().min(1).max(64)).max(32)` — lista chain-level con MÁXIMO 32
   entradas: el universo 264 ni cabe; es un switch de clases.
8. **Modelo de costos global** — OBSERVED `computeSimulatedNet.ts:369-377`
     (`varCostRateFromCfg`): flashloan_fee / slippage / failure buffer / lp fee /
     capital cost — knobs GLOBALES sin eje estrategia. (La fricción es estructura
     de mercado; el punto del audit es que NO existe NINGÚN parámetro
     per-estrategy además de los pisos.)

## pair_symbol como eje de evaluación

NO encontrado. STRAT-IDENT-01 eliminó el pair-bucket del scoring; `pair_symbol`
aparece como SELECT de display (`backend/api-server/src/index.ts:985`) y contexto
del record, no como eje de evaluación.

## Síntesis

El flattening real es **exactamente uno**: el espacio de claves de CONFIGURACIÓN
(`strategy_configs` + `enabled_strategies`) vive en el universo de 5 clases.
Detección, scoring Gate C, evidencia §IV y persistencia ya operan per-estrategia.
El gap práctico: las 264 se evalúan contra pisos/caps globales o de clase porque
sus claves MEV-XX-XXX no existen en el config — la declaración canónica de
OPERADORES está wired (math_evidence), la declaración de PARÁMETROS (targets,
caps) no.

— 7b, sesión arbitragex-v2-main-17-7b, read-only, cero commits.
