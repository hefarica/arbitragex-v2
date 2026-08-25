# ARBITRAGEX DAPP OMNISCIENCE — SUPER-SKILL

> La habilidad suprema que integra TODO el conocimiento de ArbitrageX + estado del arte mundial.
> El Excel es el 5%. El mundo es el 95%.

## Cómo usar esta skill

Cuando el operador pregunta sobre estrategias, rutas, operadores, financiamiento, sizing,
MEV, DEX, o cualquier aspecto de la dapp, esta skill es el punto de entrada.

## Arquitectura interna

```
ARBITRAGEX_DAPP_OMNISCIENCE
│
├── CANON (5%) — from Excel + repo
│   ├── 264 Strategy Cartridges → skills/arbitragex-ultra/strategies/
│   ├── 31 Operator Skills      → skills/arbitragex-ultra/operators/
│   ├── Knowledge Graph         → knowledge_graph.jsonl (2,511 edges)
│   ├── Capability Matrix       → capability_matrix.json (265 strategies)
│   └── Doctrina Rutas          → docs/ROUTES_CROWN_JEWEL_DOCTRINE.md
│
├── IMPLEMENTACIÓN — from codebase
│   ├── Math Engine             → backend/math-engine/src/operators/
│   ├── Searcher (264 cartuchos) → backend/searcher-rs/cartridges/
│   ├── Route Discovery         → backend/searcher-rs/src/route_discovery/
│   ├── Financing Module        → backend/searcher-rs/src/route_discovery/financing.rs
│   └── Simulator              → backend/simulator-v2/
│
└── WORLD (95%) — from research
    ├── Graph Science: RICH VLDB'25, Bellman-Ford, Johnson, color-coding
    ├── CFMM Optimization: Angeris arXiv:2204.05238, marginal-price Bancor
    ├── MEV: Flashbots docs, searcher practice, bundle economics
    ├── Financing: fees verificados on-chain (Aave/Balancer/Morpho/Maker)
    └── Market Microstructure: adverse selection, inclusion probability, bribe γ
```

## Capacidades de razonamiento

### 1. Análisis de estrategia
Dado un MEV_ID, la skill puede responder:
- Qué familia, superficie, topología de ruta
- Qué operadores participan (primary/secondary)
- Qué detector la descubre
- Qué método de sizing corresponde
- Qué modos de financiamiento son compatibles

### 2. Rutas y descubrimiento
- Enumeración exhaustiva DeferNeverDrop (SHADOW-NO-ROUTE-CAPS)
- Dos capas: DISCOVERY pura ≠ EVALUATION con funnel
- 2 a N-hop cycles, iterative deepening
- Rotación de pools paralelos
- Grafo -ln(rate), negative cycles

### 3. Financiamiento como dimensión de ruta
- Cada ruta evaluada en paralelo: OWN_CAPITAL, AAVE_FL, BALANCER_FL, V2_FLASH_SWAP
- Born/died/resized visible al togglear
- Fees on-chain verificados (leer, no hardcodear)

### 4. Matemática por topología
- 2-pool same-pair: raíz cuadrática cerrada
- Ciclos ≥3: convexo o root-finding
- Ley √: tamaño óptimo ∝ √(discrepancia) × √(liquidez)
- Gas (fijo) no mueve el argmax; fee flash (lineal) lo baja

### 5. Sizing y EV
- Waterfall: EV = P(inclusión) × [gross(Δ) − f·Δ − Σγ^n − gas − tip]
- Break-even: min_amount_in > (gas_price × gas) / spread
- Kelly: advisory hasta calibración real del posterior

### 6. Gates y readiness
- G-SIM-1: 7/7 evidenced
- A.4 fork validation: PASS
- A.5 crucible: 72h
- A.9: operator sign-off

## Consultas que esta skill puede responder

1. "¿Qué operadores usa MEV-05-042?" → buscar en knowledge_graph.jsonl
2. "¿Qué estrategias usan op_16 (Kelly)?" → reverse lookup en matriz
3. "¿Qué rutas sobreviven si deshabilito Aave?" → funnel:born/died por mode
4. "¿Cuál es el sizing óptimo para un 2-pool WETH/USDC?" → fórmula cuadrática cerrada
5. "¿Qué detector descubre triangular?" → detector families lookup
6. "¿Qué estrategia no está en el Excel?" → gap analysis vs estado del arte

## Archivos clave

| Archivo | Contenido |
|---|---|
| `knowledge_graph.jsonl` | 2,511 edges (Strategy→Operator, →Detector, →Family) |
| `capability_matrix.json` | 265 estrategias con estado de implementación |
| `strategies/` | 264 directorios con SKILL.md + STRATEGY.json |
| `operators/` | 31 directorios con SKILL.md + OPERATOR.json |
| `../docs/ROUTES_CROWN_JEWEL_DOCTRINE.md` | Doctrina completa de rutas |

## Fuentes de verdad

1. **Excel ULTRA** (534K celdas, 30K fórmulas) → ontología canónica
2. **Repo codebase** → implementación real (vs documentación)
3. **Doctrina Crown Jewel** → estado del arte mundial investigado
4. **On-chain fees** → verificados por RPC (2026-08-18)

## Reglas innegociables

1. **El Excel es el 5%** — nunca el límite del conocimiento
2. **Dos capas** — DISCOVERY pura ≠ EVALUATION con gates
3. **Financing = dimensión de ruta** — nunca tesorería
4. **Nada muere en silencio** — cada rechazo tiene (hop_tier, gate, razón, modo)
5. **Fees on-chain** — leer, nunca hardcodear
6. **Fail-honest** — "—" = no computado, nunca fabricado
7. **Anti-hallucination** — clasificar cada claim con su fuente

## Evolución

Esta skill crece. Cuando encuentres:
- Un algoritmo mejor que el actual → regístralo como BETTER_CANDIDATE
- Una nueva Opportunity Surface → extiende la ontología
- Un fee que cambió → actualiza el valor on-chain
- Una estrategia que no existe → derívala y clasifícala

El conocimiento debe evolucionar. El Excel es el punto de partida, no el destino.
