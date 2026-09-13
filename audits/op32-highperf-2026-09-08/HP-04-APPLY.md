# HP-04 — APPLY: matriz extendida estrategia×operador con op_32 (NSGA-II)

**WO**: HP-04 · kind: apply · agente: strategy-architect (PhD) · fecha: 2026-09-08
**Estado**: ✅ COMPLETE — 182 edges op_32 añadidos, validador 17/17 PASS, diff quirúrgico verificado.

> **⚠️ Provenancia de insumo (fail-honest)**: la dependencia dura `HP-01-CENSO.md`
> **NO aterrizó** cuando HP-04 arrancó (dir solo tenía GOAL-WORKORDERS.md + SEED v1;
> el SEED v2 aterrizó a mitad de mi sesión — leído y usado). Como HP-01-CENSO.md está
> bajo el claim de HP-04, ejecuté yo mismo el censo de las secciones (b) y (c) con
> evidencia file:line y lo escribí en `HP-01-CENSO.md` como fallback documentado
> (secciones (a)/(e) compactas, (d) NOT-COVERED declarado). Los edges de abajo siguen
> ese censo propio contra `knowledge_graph.jsonl` REAL — no el marketing del SEED.

## 1. N edges añadidos: 182 (todas USES_SECONDARY)

Total del grafo: **2,511 → 2,693** líneas (+182, 0 borradas — `git diff --numstat`:
`182 0 skills/arbitragex-ultra/knowledge_graph.jsonl`).

**Criterio de diseño (rubric strategy-architect, una línea económica por familia)** —
op_32 captura valor SOLO donde el pipeline enumera >1 plan de ejecución Y ≥2 de los
objetivos [rentabilidad neta, riesgo CVaR, latencia] son material de la familia y
conflictúan entre candidatos:

| Familia | N | Estrategias (rangos exactos) | Justificación económica (qué captura el frente en ESA familia) |
|---|---:|---|---|
| MEV-01 | 36 | MEV-01-001..036 | Ciclos multihop (R_CLOSED_CYCLE=25, R_ORDERBOOK=4, R_DIRECT_INDIRECT/R_SPLIT/R_BASKET_NAV=2, R_COW=1): cada hop suma fee on-chain, decoherencia de estado y latencia de inclusión — el frente elige cuántos hops pagar (staleness medido 1.29–1.78 bps/bloque, `world/graph-algorithms/BETTER_THAN_EXCEL.md` §5; ejemplo del GOAL: MEV-01-016 triangular, min/max legs 3 verificado en `strategies/MEV-01-016/SKILL.md`). |
| MEV-03 | 29 | MEV-03-001..028, MEV-03-031 | Backruns/subastas por evento (E_STATE=15, E_POST=6, E_LATENCY=4, E_ORACLE=2, E_AUCTION=2): rentabilidad neta vs prob. de inclusión (EV·P(win|bid), canon `BETTER_THAN_EXCEL.md` §8 — "Our Kelly op sizes in isolation from competition") vs ventana temporal; el bid de gas ES una posición en el frente. Excluidas MEV-03-029/030 (OBSERVE). |
| MEV-05 | 14 | MEV-05-001..014 | Split CEX/DEX: rentabilidad neta vs CVaR de inventario/contraparte del leg CEX vs latencia de transferencia cross-venue — única familia con op_16 (Kelly) y op_23 (queueing) presentes 14/14 (censo §(b)). |
| MEV-06 | 30 | MEV-06-001..030 | Selección de puente/ruta cross-chain (X_BRIDGE=16, X_PREPOS=12, X_ORACLE=2): spread vs CVaR de puente (riesgo de finalidad/fallo) vs latencia de finalidad; la elección de puente es una posición en el frente (ejemplo explícito del GOAL: "cross-chain MEV-06 con riesgo de bridge"). |
| MEV-07 | 30 | MEV-07-001..030 | Construcción de hedge en derivados (D_BASIS=13, D_OPTIONS_SURFACE=6, D_SETTLE=5, D_OPTIONS_PARITY=4, D_FUNDING=2): captura de basis/funding vs CVaR de margen/liquidación vs latencia de roll/settlement (D_SETTLE = latencia de settlement como detector propio). |
| MEV-08 | 25 | MEV-08-001..025 | Liquidaciones/subastas (L_LIQ=7, L_AUCTION=4, L_COLLATERAL=5, L_RATE=8, L_LOOP=1): bono de liquidación vs ventana de competencia vs riesgo de rebote de health-factor ("health-factor vs profit" del operador; MEV-08-012 Liquidation discount arbitrage). |
| MEV-09 | 18 | MEV-09-001..018 | Bids de solver (I_ROUTE=9, I_BATCH=4, I_ORDERFLOW=4, I_DUTCH=1): margen del bid vs prob. de adjudicación (Nash — op_24 presente en 18/20) vs deadline de inclusión. Excluidas MEV-09-019/020 (OBSERVE). |

**Rol USES_SECONDARY (todos)**: op_32 es selección/ranking sobre candidatos ya
evaluados (f=[−rentabilidad, CVaR, latencia], SEED v2 líneas 367-377) — la clase
evaluación/selección del grafo vive en USES_SECONDARY (op_22 Monte Carlo 248/264
secondary, op_30 GNN 27/0; censo §(c)). Nunca descubre ni dimensiona → nunca primary.

## 2. R8 — sin edge donde no hay justificación (no se rellena simetría)

| Familia | N sin edge | Razón económica |
|---|---:|---|
| MEV-02 | 17 | Routing CFMM resoluble convexo a óptimo global neto-de-fees (Angeris/Diamandis, `BETTER_THAN_EXCEL.md` §1); riesgo/latencia ~constantes intra-bloque atómico → frente degenerado a un punto. |
| MEV-04 | 31 | Set de candidatos pequeño (venta inmediata vs redención); comparación escalar NPV-por-día-de-lockup suficiente; sin población para un orden de no-dominancia. |
| MEV-10 | 18 | Sweeps discretos pequeños; decoherencia ya capturada por sizing (op_21/op_15); sin conflicto multi-objetivo medible in-stream. |
| MEV-11 | 12 | Pares de plataformas pequeños; lockup estático por mercado; odds-netas escalares suficientes. |
| OBSERVE (todas las familias) | 8 | MEV-03-029/030, MEV-04-031, MEV-09-019/020, MEV-11-009/010/011: sin decisión de ejecución → un selector Pareto no tiene decisión que tomar. |

El propio canon respalda la disciplina: solo 23/31 operadores tienen edges hoy
(op_02/03/04/09/12/18/28/31 = 0 edges) — un operador sin justificación se queda a
cero (HP-01-CENSO.md §(c)). El claim del SEED "op_32 → TODAS LAS ESTRATEGIAS"
(264 edges) se RECHAZA por marketing; 182/264 = 69% es el corte defendido.

## 3. Dónde vive el dato (4 almacenes, 1 transacción coherente)

- `skills/arbitragex-ultra/knowledge_graph.jsonl` — +182 edges tras el último
  op-edge de cada estrategia objetivo (orden ascendente de op preservado, formato
  byte-idéntico a vecinos, ejemplo diff:
  `{"from": "MEV-01-016", "rel": "USES_SECONDARY", "to": "op_30", ...}` →
  `+{"from": "MEV-01-016", "rel": "USES_SECONDARY", "to": "op_32", ...}`).
- `skills/arbitragex-ultra/capability_matrix.json` — `operators[]` += `op_32` en las
  182 entradas (listas siguen sorted: op_32 > op_31). Diff: +364/−182 (182 comas de
  último-ítem + 182 inserciones).
- **EXTENSIÓN DE SCOPE (descubrimiento post-charter, documentada para ratificación
  del BOARD)**: `skills/arbitragex-ultra/strategies/*/STRATEGY.json` (182×
  `secondary_operators` += op_32) y `strategies/*/SKILL.md` (182× línea
  `**Secondary**` += op_32). No estaban en la lista textual de mi claim, pero el
  censo demostró el invariante kg ≡ cm ≡ STRATEGY.json 264/264 (HP-01-CENSO.md
  "Invariante de 3 almacenes"): tocar solo 2 de 3 dejaría la matriz canónica
  contradictoria consigo misma (RULE 00). Sin conflicto con otros WO (HP-01
  read-only, HP-03 Rust, HP-05 frontend, HP-06/07 docs). Revert = 1 script.
- `skills/arbitragex-ultra/operators/op_32/` — CREADO espejo del patrón op_01..op_31:
  `OPERATOR.json` (9 keys idénticas al patrón verificado 31/31) + `SKILL.md`
  (Identity/Math/Pipeline Phase RANK/Implementation/Excel Traceability/Calibration).
  **Honesto**: `engine_present: false`, `calibration_state: "NOT_IMPLEMENTED"`
  (HP-03 no ha aterrizado; el enum de estados actuales es UNCALIBRATED=31, nuevo
  valor introducido DELIBERADAMENTE para no mentir). Excel Traceability declara
  PENDIENTE OPERADOR (workbook sin fila op_32).

## 4. Output del validador (`py audits/op32-highperf-2026-09-08/hp04_apply_op32.py` — idempotente)

```
===== HP-04 VALIDATOR =====
[PASS] JSONL parsea 100% (2693 lineas, 0 errores)
[PASS] conteo total = 2511 + 182 = 2693 (real: 2693)
[PASS] kg termina en newline
[PASS] sin edges duplicados (from,rel,to) — dupes=0
[PASS] schema homogeneo (4 keys exactas + enums type/rel)
[PASS] op_32 edges = 182 (real: 182)
[PASS] todos los op_32 son USES_SECONDARY / Strategy->Operator
[PASS] op_32 solo en las 182 estrategias objetivo ([])
[PASS] ningun OBSERVE lleva op_32 ([])
[PASS] ninguna familia R8-sin-edge lleva op_32 ([])
[PASS] kg ≡ capability_matrix 264/264 (mismatches: 0)
[PASS] kg ≡ STRATEGY.json 264/264 (mismatches: 0)
[PASS] SKILL.md Secondary espeja op_32 exactamente en los 182 (mal: 0)
[PASS] operators/op_32/{OPERATOR.json,SKILL.md} existen
[PASS] OPERATOR.json op_32 schema = patron op_01..op_31 (9 keys)
[PASS] op_32 honesto: engine_present=false / NOT_IMPLEMENTED (HP-03 pendiente)
[PASS] conteo por familia {'MEV-01': 36, 'MEV-03': 29, 'MEV-05': 14, 'MEV-06': 30,
       'MEV-07': 30, 'MEV-08': 25, 'MEV-09': 18}
VALIDATOR: ALL PASS
```

Verificación de cirugía (`git diff --numstat`): kg `+182/−0`; cm `+364/−182`;
366 archivos tocados bajo skills/ = 2 canónicos + 182 STRATEGY.json + 182 SKILL.md;
`artifacts/` INTACTO (no corrí build_canonical_artifacts.py — escribiría fuera de
mi claim). Cada línea cambiada traza al op_32 (spot-check MEV-01-016 en §3).

## 5. Verificación por capa

WO de DATOS canónicos (JSONL/JSON/MD): no aplica cargo/tsc/vitest (nada que toqué
es importado por build TS o Rust — skills/ no está en tsconfig; el hot-path lee
cartridges, no el grafo). Riesgo de runtime auditado: `math_evidence.rs:87-104`
devuelve None honesto para ids no registrados → cartridges (que NO declaran op_32)
intactos. La verificación de esta capa ES el validador §4 + git numstat.

## 6. Reporte al BOARD (decisiones que me exceden — ninguna bloqueante)

1. **Espejo TS espejo ubicado, NO tocado (fuera de claim)**: no existe enum TS de
   operadores matemáticos (`OperatorIdSchema` = UUID del operador humano,
   `frontend/lib/apex/schemas/_primitives.ts:95-97`). El único espejo es GENERADO:
   `docs/quotebase_strategy_hop_map.json` (workbook 11_STRATEGY_HOP_MAP) +
   `docs/quotebase_detector_policy.json` → `scripts/gen_quotebase_catalog_ts.py` →
   `backend/api-server/src/generated/quotebase_catalog.ts` ("DO NOT EDIT", líneas
   1-9) → GET /api/strategies/catalog + /api/detectors/catalog →
   `frontend/lib/apex/schemas/{strategies,detectors}.ts` (Zod string[]). Esa cadena
   NO se alimenta del knowledge_graph → fluir op_32 al frontend exige regenerar el
   JSON fuente + correr el generador: **decisión del orquestador** (HP-05/HP-08
   deberían ver op_32 en la UI de preferencias vía otra vía, no por este archivo).
2. **Follow-up OPERADOR (workbook)**: 12_OPERATOR_CONTROL no tiene fila op_32 y
   13_STRAT_OP_MATRIX no tiene columna op_32. Hasta que el operador las añada,
   `scripts/excel_canon/build_canonical_artifacts.py:626-641` (ops_match
   STRATEGY.json↔workbook) marcará mis 182 estrategias **PARTIAL** en el próximo
   run de cobertura — divergencia honesta y visible (correcta bajo RULE 00), no un
   defecto; HP-08 debe esperarla y no "corregirla" borrando op_32.
3. **HP-03 (Rust) pendiente**: cuando aterrice `op_32_nsga2.rs` + registro, debe
   flip `operators/op_32/OPERATOR.json` → `engine_present: true`,
   `calibration_state: "UNCALIBRATED"` (o el estado diseñado), y tocar
   `matrix/topology_map.rs` COLS 31→32 (264×31 → 264×32, matriz Master). El grafo
   YA está listo para su wiring (182 edges esperándolo).
4. **HP-05 (UI preferencias)**: sin dependencia de este WO para su diseño; si
   quiere mostrar op_32 en catálogos, ver punto 1.

## 7. Doctrina cumplida

- RULE 00: cero datos fabricados — engine_present=false honesto, NOT_IMPLEMENTED,
  OBSERVE sin edge, familias sin justificación sin edge, censo propio documentado
  como fallback (HP-01 ausente declarado, no fingido).
- §32/§33/§34.3: cero executor/capital/broadcast; VPS ni tocado; read-only total
  fuera de mis archivos.
- NO-GIT: cero commit/push/PR — working tree local únicamente.
- R8: frente vacío = None (documentado en op_32/SKILL.md para HP-03).
- X10: el diseño de edges se validó contra 3 fuentes independientes (kg real, censo
  por familia de detectores/ops, canon mundial BETTER_THAN_EXCEL.md).

— HP-04 (strategy-architect), 2026-09-08. // HP-04 (2026-09-08)
