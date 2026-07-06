---
name: sop_liquidity_aggregation
description: Cuando se diseñe agregación de liquidez multi-DEX o split-routing para grandes operaciones. Activa con triggers "agregación liquidez", "split routing", "best execution", "multi-pool routing", "find best liquidity", "60/40 split Uniswap Curve", "compute_split_route". Trae el código Alloy del SOP §12 con LiquidityRoute + split proporcional por depth.
type: arbx_architecture
source_section: SOP_ArbitrageX_2026.pdf §12
---

# Algoritmo de Búsqueda de Liquidez Multi-Chain

## Concepto (§12.1)
**No basta con conocer un solo pool.** El sistema debe encontrar el mejor precio entre todos los DEXes y pools de todas las cadenas relevantes. Para operaciones grandes, **split routing** divide el monto entre múltiples pools para minimizar impact.

Ejemplo: swap de 100 WETH puede dividirse en 60 WETH en Uniswap V3 fee 0.05% + 40 WETH en Curve fee 0.04% → mejor precio promedio ponderado que cualquier pool individual.

## Implementación de referencia (§12.2)

```rust
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;

#[derive(Debug, Clone)]
struct LiquidityRoute {
    dex: String,
    pool: Address,
    amount_in: U256,
    amount_out: U256,
    gas_cost: U256,
    profit: U256,
}

async fn find_best_liquidity(
    provider: &impl Provider,
    token_in: Address,
    token_out: Address,
    amount: U256,
) -> Vec<LiquidityRoute> {
    let dexes = get_dex_list();
    // Uniswap, SushiSwap, Curve, Balancer, 1inch, PancakeSwap, etc.
    let mut routes = vec![];

    for dex in &dexes {
        if let Some(pool) = find_pool(provider, dex, token_in, token_out).await {
            let quote = get_quote(provider, pool, amount).await;
            let gas = estimate_gas(provider, pool, amount).await;
            if quote > U256::ZERO {
                routes.push(LiquidityRoute {
                    dex: dex.name.clone(),
                    pool,
                    amount_in: amount,
                    amount_out: quote,
                    gas_cost: gas,
                    profit: quote.saturating_sub(amount),
                });
            }
        }
    }

    routes.sort_by(|a, b| b.profit.cmp(&a.profit));
    routes
}

/// Split routing proporcional por depth
async fn compute_split_route(
    routes: &[LiquidityRoute],
    total_amount: U256,
) -> Vec<(String, U256)> {
    let total_depth: U256 = routes.iter()
        .map(|r| r.amount_out)
        .fold(U256::ZERO, |acc, x| acc + x);

    routes.iter()
        .map(|r| {
            let split = total_amount * r.amount_out / total_depth;
            (r.dex.clone(), split)
        })
        .collect()
}
```

## DEXes a agregar (cap 4.4 sop_body)

| DEX | Cadenas | Modelo | Fee | Mejor para |
|-----|---------|--------|-----|------------|
| Uniswap V3 | 15+ chains | Concentrado | 0.01-1% | Blue chips, volátiles |
| Curve | 12+ chains | StableSwap | 0.01-0.04% | Stables, pegged |
| Balancer V2 | 10+ chains | Ponderado | 0.01-1% | Multi-asset, flash |
| SushiSwap | 20+ chains | Constant Product | 0.05-0.3% | L2s, emergentes |
| 1inch | 15+ chains | Aggregator | Variable | Ruta óptima |
| Maverick | 8+ chains | Dynamic AMM | 0.01-0.3% | Gamma trading |
| Aerodrome | Base | ve(3,3) | 0.01-0.3% | Base nativa |
| PancakeSwap | BSC + 5 chains | CPMM | 0.01-0.25% | BSC nativa |

## Cuándo usar split routing
- Trade size > 10% de la liquidez del pool más profundo.
- Múltiples pools con diferencias de profit > 0.05%.
- Tokens stablecoin (Curve + Uniswap V3 fee 0.01% combinados).

## Cuándo NO usar split routing
- Trade size pequeño (gas adicional > ahorro).
- Solo un pool tiene liquidez significativa.
- Tiempo crítico (cada ms cuenta) — usar el mejor pool individual.

## Invariantes
- Sort routes by profit descending SIEMPRE.
- Split solo si profit_split > profit_single + (gas_split - gas_single).
- Para split de N pools, gas total ≈ N × gas_single (no asumir overhead amortizado).
- Min 3 DEXes consultados por swap (redundancia + best execution).

## Cross-references
- Quoter implementations: `sop_dex_triangular` §4.3 (V3 quoter).
- Bellman-Ford uses LiquidityRoute results: `sop_atomic_route_construction`.
- Cuando NO hay liquidez suficiente: skill `liquidity-aware-route-filtering` (índice MEV).
