---
name: sop_cex_dex
description: Cuando se diseñe, implemente o evalúe arbitraje CEX-DEX (Binance/OKX/Bybit vs Uniswap/Curve). Activa con triggers "CEX-DEX", "spread Binance Uniswap", "ventaja extrema MEV", "WebSocket Binance", "asimetría order book vs AMM", "API exchange centralizado". Esta es UNA DE LAS 4 ESTRATEGIAS CON VENTAJA EXTREMA — el 99% de competidores no opera aquí. Trae código Alloy + Binance WS del SOP §5.
type: arbx_strategy_reference
source_section: SOP_ArbitrageX_2026.pdf §5
competitive_advantage: extrema
---

# CEX-DEX Arbitraje — VENTAJA COMPETITIVA EXTREMA

## Por qué es ventaja extrema (§5.1)
Asimetría estructural permanente entre dos mundos:
- **CEX** (Binance, OKX, Bybit): order book centralizado, latencia microsegundos.
- **DEX** (Uniswap, Curve): precios = razón de reservas (x·y=k V2), latencia 100ms-segundos on-chain.

**Esta asimetría no puede ser eliminada por la competencia.** Mientras más participantes intentan cerrar el spread, más liquidez fluye, pero la latencia inherente cross-system garantiza que los spreads reaparezcan constantemente.

## Mecánica de detección (§5.2)
```
spread = |precio_cex - precio_dex| / min(precio_cex, precio_dex)
```
Cuando `spread > MIN_SPREAD_THRESHOLD` (típicamente 0.15%), determinar dirección:
- Si `precio_cex > precio_dex` → **comprar DEX, vender CEX**
- Si `precio_cex < precio_dex` → **comprar CEX, vender DEX**

## Implementación de referencia (§5.3)

```rust
use alloy::providers::Provider;
use tokio::sync::mpsc;

const MIN_SPREAD_THRESHOLD: f64 = 0.0015; // 0.15%

#[derive(Debug, Clone)]
enum TradeDirection { BuyDexSellCex, BuyCexSellDex }

async fn cex_dex_arb_loop(binance_ws: &str, provider: &impl Provider) {
    let (tx, mut rx) = mpsc::channel::<(String, f64)>(1024);
    spawn_binance_price_feed(binance_ws, tx).await;
    while let Some((symbol, cex_price)) = rx.recv().await {
        let dex_price = get_on_chain_price(provider, &symbol).await;
        let spread = (cex_price - dex_price).abs() / dex_price.min(cex_price);
        if spread > MIN_SPREAD_THRESHOLD {
            let direction = if cex_price > dex_price {
                TradeDirection::BuyDexSellCex
            } else { TradeDirection::BuyCexSellDex };
            execute_cex_dex_trade(direction, spread).await;
        }
    }
}
```

## Por qué el 95% fracasa (§5.4) — los 3 errores
1. **Latencia excesiva**: REST APIs (50-200ms) en lugar de WebSockets persistentes. Solución: WS con thread dedicado.
2. **Desajuste profundidad order book**: comprar $1M a precio top-of-book ignora slippage real. Solución: calcular impacto real con depth.
3. **Timing retiros/depósitos**: mover fondos entre CEX↔wallet toma minutos. Solución: capital pre-posicionado en ambos lados.

## Configuración recomendada por exchange

| Exchange | Latencia WS | Rate limit REST | Fee maker/taker |
|----------|-------------|------------------|------------------|
| Binance | 10-30ms | 1200 req/min VIP | 0.1%/0.1% (lower vols) |
| Coinbase Pro | 10-30ms | 10 req/s | 0%/0.05-0.08% |
| OKX | 10-50ms | varía | similar Binance |
| Bybit | 10-50ms | varía | similar OKX |

## Configuración óptima
- WS persistente con auto-reconnect.
- Order book local en memoria (cálculo instant de spreads).
- Fallback REST si WS cae.
- Órdenes **limit** (no market) para garantizar precio.
- Nonce management robusto.
- Capital pre-posicionado en ambos lados (CEX + wallet on-chain).
- VPS en región del exchange (Tokio para Binance APAC, Londres/Frankfurt para OKX/Bybit).

## Invariantes
- NUNCA REST API para feed de precios (solo WS).
- NUNCA órdenes market en CEX para arb (riesgo slippage adversa).
- NUNCA mover fondos entre CEX↔wallet en hot-path (capital pre-posicionado).
- Spread mínimo configurable, default 0.15%.

## Cross-references
- Tabla de pares por cadena: `sop_token_pool_selection` §10.2.
- Risk management aplicable: `sop_risk_management`.
- Por qué es estrategia "Extrema": ver `sop_strategy_matrix`.
