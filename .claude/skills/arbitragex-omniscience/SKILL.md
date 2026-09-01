---
name: arbitragex-omniscience
description: ARBITRAGEX DAPP OMNISCIENCE — La super-skill que integra 264 estrategias, 31 operadores, 60 detectores, knowledge graph (2,511 edges), doctrina de rutas, y estado del arte mundial DApp/DeFi/MEV. Úsala para CUALQUIER pregunta sobre estrategias, rutas, sizing, financiamiento, MEV, DEX, optimización, o ejecución.
---

# ARBITRAGEX DAPP OMNISCIENCE

## Cómo activar el conocimiento completo

Cuando el operador hace cualquier pregunta relacionada con estrategias, rutas, operadores,
financiamiento, sizing, MEV, DEX, arbitraje, o ejecución, cargar TODOS estos recursos:

### 1. Canon (el 5% — Excel + repo)
```
skills/arbitragex-ultra/SUPER_SKILL.md                     ← Arquitectura + reglas
skills/arbitragex-ultra/knowledge_graph.jsonl              ← 2,511 edges Strategy↔Operator↔Detector
skills/arbitragex-ultra/capability_matrix.json             ← 265 estrategias con estado
skills/arbitragex-ultra/operators/op_XX/SKILL.md           ← 31 operator skills
skills/arbitragex-ultra/operators/op_XX/OPERATOR.json       ← 31 operator data
skills/arbitragex-ultra/strategies/MEV-XX-XXX/SKILL.md     ← 264 strategy cartridges
skills/arbitragex-ultra/strategies/MEV-XX-XXX/STRATEGY.json ← 264 strategy data
```

### 2. Doctrina (investigación mundial)
```
docs/ROUTES_CROWN_JEWEL_DOCTRINE.md                        ← RICH, CFMM convex, fees on-chain
docs/superpowers/plans/2026-08-19-GPRICE-SPEED-LIGHT.md   ← Precios velocidad de la luz
docs/superpowers/plans/2026-08-19-GSIM1-THREE-BUGS-PLAN-SUPREMO.md ← Bugs G-SIM-1
```

### 3. Implementación (código real)
```
backend/math-engine/src/operators/                         ← 31 operadores Rust
backend/searcher-rs/cartridges/strategies/                 ← 264 cartridges Rhai
backend/searcher-rs/src/route_discovery/                   ← Discovery + financing
backend/searcher-rs/src/workers/chainlink_subscriber.rs    ← Event-driven prices
backend/simulator-v2/src/                                  ← LazyDb + REVM runner
```

### 4. Mundo (el 95% — research)
```
skills/arbitragex-ultra/world/graph-algorithms/            ← RICH, BF variants, convex routing
skills/arbitragex-ultra/world/mev-practice/                ← Searcher real, bundles, margins
skills/arbitragex-ultra/world/defi-protocols/              ← UniV4, Morpho, Hyperliquid, intents
skills/arbitragex-ultra/world/security-simulation/         ← REVM, formal verify, attack surface
skills/arbitragex-ultra/world/quant-math/                  ← Kyle, HJB, Kelly multi-armed, VPIN
```

### 5. Datos extraídos del Excel
```
docs/excel_ingestion_manifest.json                          ← 47 hojas, 534K celdas
docs/excel_strategies_extracted.json                        ← 267 estrategias
docs/excel_operators_extracted.json                         ← 33 operadores
docs/excel_matrix_extracted.json                            ← 1,716 asociaciones
docs/excel_detectors_extracted.json                         ← 60 detectores
docs/coverage_manifest.json                                 ← Coverage verificada
```

## Reglas de razonamiento

1. **El Excel es el 5%** — nunca el límite. Cuando encuentres algo mejor, regístralo.
2. **Dos capas** — DISCOVERY (enumerar topología) ≠ EVALUATION (gates + sizing + EV)
3. **Financing = dimensión de ruta** — cambia qué rutas son viables, no cuántas se descubren
4. **Nada muere en silencio** — cada rechazo: (hop_tier, gate, razón, financing_mode)
5. **Fees on-chain** — leer de la cadena, nunca hardcodear (Aave = 5bps HOY, gobernable)
6. **Fail-honest** — "—" = no computado, 0 = exactamente cero, nunca fabricar
7. **Anti-hallucination** — clasificar: CANONICAL_WORKBOOK / CANONICAL_REPO / PRIMARY_SOURCE / INFERRED / HYPOTHESIS / UNKNOWN

## Consultas que puedes responder

- "¿Qué operadores usa MEV-06-018?" → knowledge_graph.jsonl
- "¿Qué estrategias usan Kelly?" → reverse lookup en matriz
- "¿Qué rutas sobreviven sin flash loans?" → funnel born/died por mode
- "¿Sizing óptimo para 2-pool WETH/USDC?" → fórmula cuadrática cerrada
- "¿Qué detector descubre triangular?" → detector families
- "¿Qué estrategia NO está en el Excel?" → gap analysis vs estado del arte
- "¿Hay un algoritmo mejor para esta ruta?" → comparar con world/

## Invocación

El operador simplemente pregunta. Esta skill se activa automáticamente para cualquier
consulta relacionada con estrategias, rutas, operators, financiamiento, sizing, MEV,
DEX, arbitraje, optimización, ejecución, o cualquier aspecto de la dapp ArbitrageX.

No necesitas un comando especial — solo pregunta.
