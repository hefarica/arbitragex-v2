# NSGA-II Multi-Objective (op_32)

## Identity
- **ID**: 32 / op_32
- **Canonical Role**: Pareto ranking/selection over [rentabilidad_neta, riesgo_CVaR,
  latencia] of already-evaluated candidates; empty front = None (fail-honest R8).
- **Enabled**: YES | **Engine**: NO (PENDIENTE HP-03)
- **Calibration**: NOT_IMPLEMENTED

## Mathematical Definition
```
min F(x) = (-rentabilidad_neta(x), CVaR(x), latencia(x))  s.t. x in Omega (candidates)
```
NSGA-II: fast non-dominated sort + crowding distance + elitism; selection by
normalized preference vector [rentabilidad, riesgo, latencia].

## Pipeline Phase
RANK - selection among evaluated candidates (USES_SECONDARY en el knowledge graph,
clase evaluacion/seleccion como op_22/op_30). Nunca descubre ni dimensiona.

## Implementation
- File (PENDIENTE HP-03): `backend/math-engine/src/operators/op_32_nsga2.rs`
- Register (PENDIENTE HP-03): registry de operators (mod.rs:6 instruction) +
  `matrix/topology_map.rs` COLS 31->32 (264x31 -> 264x32).
- Tests (PENDIENTE HP-03): non-dominance of front, crowding distance, preference
  selects from front, 1-objective degeneration, seed determinism, bounded generations.

## Excel Traceability
- Source: PENDIENTE OPERADOR - 12_OPERATOR_CONTROL no tiene fila op_32; anadir fila 35
  y columna op_32 en 13_STRAT_OP_MATRIX para cerrar el PARTIAL honesto de
  scripts/excel_canon/build_canonical_artifacts.py (ops_match).
- Matrix: 13_STRAT_OP_MATRIX column `op_32` (PENDIENTE OPERADOR).

## Calibration
NOT_IMPLEMENTED - el motor no existe aun (engine_present=false hasta HP-03). Frente de
Pareto vacio = None honesto, jamas solucion fabricada (R8).

## Edges (HP-04 2026-09-08)
182 edges USES_SECONDARY en 7 familias justificadas (MEV-01/03/05/06/07/08/09, sin
OBSERVE); MEV-02/04/10/11 SIN edge por R8 (justificaciones economicas por familia en
audits/op32-highperf-2026-09-08/HP-04-APPLY.md). // HP-04 (2026-09-08)
