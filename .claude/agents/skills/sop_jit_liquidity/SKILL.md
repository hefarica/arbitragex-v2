---
name: sop_jit_liquidity
description: Cuando se diseñe, implemente o evalúe Just-In-Time (JIT) Liquidity — proporcionar liquidez exactamente cuando un swap pendiente la necesita. Activa con triggers "JIT liquidity", "Just-In-Time", "liquidez Uniswap V3 V4", "concentrated liquidity", "joya MEV oculta", "swap grande mempool". Estrategia con ventaja MUY ALTA, riesgo MUY BAJO (atómico). Trae mecanismo y rangos óptimos del SOP §7.
type: arbx_strategy_reference
source_section: SOP_ArbitrageX_2026.pdf §7
competitive_advantage: muy_alta
---

# JIT (Just-In-Time) Liquidity — La Joya Oculta del MEV

## Concepto (§7.1)
**Proporcionar liquidez a un pool exactamente en el momento en que un swap pendiente la necesita**, capturar fees del LP, y retirar liquidez **inmediatamente después** — todo en la misma transacción atómica.

## Mecánica detallada
1. Swap grande aparece en mempool (ej. $500K en Uniswap V3 WETH/USDC fee 0.05%).
2. Searcher analiza impacto: si swap moverá precio significativamente, se identifica oportunidad.
3. Searcher proporciona liquidez en el rango de precio afectado **justo antes** del swap (en el mismo bundle).
4. Swap ejecuta → searcher captura fees del LP.
5. **En la misma tx**, searcher retira la liquidez.
6. Profit neto = fees_capturadas - gas - impermanent_loss (mínimo dado mismo-bloque).

## Por qué pocos competidores la implementan (§7.2)
1. Requiere comprensión profunda de **concentrated liquidity** (Uniswap V3/V4 ticks, sqrtPriceX96, Q64.96).
2. Calcular **posición óptima de liquidez en milisegundos** (qué tick range, qué cantidad).
3. **Bundle perfectamente sincronizado**: mint position + victim swap + burn position, todos en mismo block, en este orden.

## Beneficios y riesgos
- **Profit por op**: 0.3% - 3%.
- **Riesgo**: virtualmente cero (atomicidad — si algún paso falla, todo el bundle se descarta sin pérdida).
- **Gas**: alto (mint + swap + burn ~ 600-800K gas) pero compensado por fees capturadas en swap grande.

## Condiciones óptimas
- Swaps pendientes ≥ $100K en pools con liquidez concentrada.
- Alta volatilidad del token subyacente.
- Mempool con tiempo de propagación suficiente para detectar y reaccionar.
- Brilla en pares como **WETH/USDC en Uniswap V3 fee tier 0.05%** (volumen diario >$500M).

## Estructura del bundle JIT
```
[1] mint(pool, tick_lower, tick_upper, liquidity)  — antes del swap victim
[2] victim_swap                                    — la tx pendiente del usuario
[3] burn(pool, position_id) + collect(fees)        — inmediatamente después
```

Si simulación muestra:
- profit_neto > umbral → enviar bundle vía Flashbots.
- profit_neto ≤ 0 → descartar (atomicidad lo cubrirá si se envía pero mejor prevenir).

## Implementación (esqueleto)
```rust
async fn try_jit_liquidity(
    provider: &impl Provider,
    victim_tx: &Transaction,
) -> Option<JitBundle> {
    // 1. Decode swap params del victim_tx
    let swap = decode_swap(victim_tx)?;

    // 2. Reconstruir estado del pool DESPUÉS del swap
    let pool_state_post = simulate_pool_after(swap).await?;

    // 3. Calcular tick range óptimo para JIT position
    let (tick_lower, tick_upper, liquidity) = optimal_jit_position(swap, pool_state_post)?;

    // 4. Estimar fees capturadas
    let fees_capt = estimate_fees_captured(swap, tick_lower, tick_upper, liquidity);

    // 5. Estimar gas total (mint + burn + collect)
    let gas_total = estimate_jit_gas();

    // 6. Profit neto
    let profit = fees_capt.saturating_sub(gas_total);
    if profit > MIN_PROFIT_USD {
        Some(build_jit_bundle(victim_tx, tick_lower, tick_upper, liquidity))
    } else {
        None
    }
}
```

## Invariantes
- SIEMPRE bundle atómico vía Flashbots (la atomicidad ES la mitigación de riesgo).
- Tick range debe contener el precio post-swap (sino no captura fees).
- Liquidez debe ser ≥ una fracción mínima de la liquidez total del rango (sino impacto despreciable).
- Simulación con revm OBLIGATORIA antes del envío.
- Si simulación muestra IL > fees, descartar (raro pero posible en swaps que cruzan muchos ticks).

## Cross-references
- Concentrated liquidity math: ver skill `uniswap-v3-concentrated-liquidity-math` (índice MEV).
- Bundle construction: `sop_flashbots_bundles`.
- Detección de swaps pendientes en mempool: `sop_csa_architecture` §2.3.
