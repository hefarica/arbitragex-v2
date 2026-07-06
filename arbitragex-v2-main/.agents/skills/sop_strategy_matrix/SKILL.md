---
name: sop_strategy_matrix
description: Cuando el operador quiera elegir estrategia, comparar riesgo/profit/velocidad/capital entre estrategias MEV, o evaluar cuál implementar primero. Activa con triggers "qué estrategia elijo", "comparar arbitrajes", "matriz de estrategias", "ventaja competitiva", "Pendle Temporal AMM", "MEV-Boost Block Building", "ROI por estrategia". Devuelve la matriz de 10 estrategias del SOP_ArbitrageX_2026 §3 con criterio de selección por perfil de operador.
type: arbx_strategy_reference
source_section: SOP_ArbitrageX_2026.pdf §3
---

# Matriz de Estrategias MEV — 10 Categorías

## Tabla comparativa (§3 del SOP_ArbitrageX_2026)

| # | Estrategia | Riesgo | Profit % | Velocidad | Capital | Dificultad | Ventaja Competitiva |
|---|------------|--------|----------|-----------|---------|------------|---------------------|
| 1 | DEX Arbitraje Triangular | Muy Bajo | 0.1-2% | <100ms | 0 (Flash Loan) | Media | Alta |
| 2 | Cross-DEX Price Diff | Bajo | 0.05-1.5% | <200ms | 0 (Flash Loan) | Baja | **Muy Alta** |
| 3 | Sandwich Attack | Medio | 0.5-5% | <50ms | Variable | Alta | Media — **DEFENSIVO ONLY** |
| 4 | Liquidation MEV | Bajo | 2-15% | <500ms | Variable | Media | Alta |
| 5 | JIT Liquidity | Muy Bajo | 0.3-3% | <150ms | Bajo | Alta | **Muy Alta** |
| 6 | Flashbots Bundle | Muy Bajo | Variable | <100ms | 0 (Flash Loan) | Media | Alta |
| 7 | CEX-DEX Arbitraje | Bajo | 0.1-3% | <300ms | Medio | Media | **EXTREMA** |
| 8 | Pendle/Temporal AMM | Medio | 1-10% | <1s | Medio | Alta | **EXTREMA** |
| 9 | Cross-Chain Bridge Arb | Medio | 0.2-5% | 1-30s | Medio | Alta | **Muy Alta** |
| 10 | MEV-Boost Block Build | Alto | Variable | <12s | Alto | Muy Alta | **EXTREMA** |

## Las 4 estrategias con ventaja "Extrema"
Donde el 99% de competidores no opera eficazmente:
- **CEX-DEX Arbitraje**: integración simultánea APIs CEX + nodes blockchain. Asimetría de información persistente.
- **Pendle/Temporal AMM**: AMMs con dimensión temporal (yield-trading) — comprensión matemática profunda requerida.
- **Cross-Chain Bridge Arb**: fragmentación liquidez L1↔L2s, bridges con latencia explotable.
- **MEV-Boost Block Building**: construir bloques propios, máxima ventaja pero capital alto + complejidad muy alta.

## Criterio de selección por perfil

| Perfil | Capital | Recomendación primera | Recomendación segunda |
|--------|---------|------------------------|------------------------|
| Principiante con $0 (flash loans) | 0 | DEX Triangular (#1) | Cross-DEX Diff (#2) |
| Operador con $50K | medio | Liquidation MEV (#4) | JIT Liquidity (#5) |
| Operador con $200K + APIs CEX | medio-alto | **CEX-DEX (#7)** | Cross-Chain (#9) |
| Operador institucional con $1M+ | alto | Pendle/Temporal (#8) | **MEV-Boost Block (#10)** |

## Sandwich Attack — Postura ÉTICA

§8 del SOP declara explícitamente: **ArbitrageX NO implementa sandwich attacks ofensivamente.** Solo se mantiene conocimiento del mecanismo para implementar **protecciones defensivas**:
- Flashbots Protect RPC (excluye del mempool público).
- Slippage mínimo (<0.1%).
- Atomic execution (bundle revierte si alguien se inserta).
- Private mempool alternatives (MEV Blocker, Titan Builder).

En el strategy_catalog DB, Sandwich tiene `ethical_constraint='defensive_only'` y `enabled` solo controla activación de las protecciones, NUNCA ejecución ofensiva.

## Invariantes
- Sandwich Attack: solo defensivo, jamás ofensivo. Bloqueado a nivel de schema DB y UI.
- Las 4 "Extremas" tienen badge dorado en UI para señalar prioridad estratégica.
- Para "principiantes" (operador sin experiencia previa con MEV): empezar por Liquidation MEV (#4) según recomendación final del cap §3.1.

## Cross-references
- Implementación de cada estrategia: `sop_dex_triangular`, `sop_cex_dex`, `sop_liquidations`, `sop_jit_liquidity`, `sop_sandwich_defensive`, `sop_flashbots_bundles`.
- Estrategias específicas no en este matriz pero del SOP previo: Yield Arbitrage, Liquidity Migration (cap 1 sop_body.pdf §1.1).
