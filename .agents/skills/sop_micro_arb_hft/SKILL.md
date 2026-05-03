---
name: sop_micro_arb_hft
description: Cuando se diseñe sistema de micro-arbitrajes de alta frecuencia, beneficios pequeños 0.01-0.1% de alta repetición, o estrategia HFT en L2s con bajo gas. Activa con triggers "micro arbitraje", "high frequency", "L2 HFT", "Arbitrum micro arb", "Base micro arb", "100-1000 ops/día", "scan_micro_arbs", "volume sobre home runs". Trae código + matemática del SOP §13.
type: arbx_strategy_reference
source_section: SOP_ArbitrageX_2026.pdf §13 + sop_body.pdf §7
---

# Sistema de Micro-Beneficios de Alta Frecuencia

## Filosofía: Volume sobre Home Runs (§13.1)

En lugar de buscar profits extraordinarios (0.5-5%) que ocurren raramente, apuntar a profits pequeños (0.01-0.1%) con **alta frecuencia** (100-1000 ops/día) en L2s con gas marginal.

### Matemática del modelo
```
profit_diario = ops_exitosas × profit_promedio_por_op
              = 500 × 0.05% del capital rotado
              = 25% del capital rotado por día
```

Con capital de $100K rotado vía flash loans (capital cero propio):
- Profit diario: $25K
- Profit mensual: $750K
- **Riesgo por op: virtualmente cero** (cada op simulada antes).

### Por qué supera "home runs"
1. Oportunidades de micro-beneficio son **mucho más frecuentes**.
2. **Competencia menor** — los bots grandes ignoran ops pequeñas.
3. **Efecto compuesto** de cientos de ops diarias → rendimientos exponenciales.
4. **Varianza menor** → planificación financiera predecible.

## Implementación de referencia (§13.2)

```rust
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;

const GAS_PRICE_MULTIPLIER: u128 = 3;
const MAX_OPPORTUNITIES_PER_BLOCK: usize = 50;

#[derive(Debug)]
struct MicroArb {
    route: Vec<Address>,
    expected_profit: U256,
    gas_cost: U256,
    net_profit: U256,
    confidence: f64,
}

async fn scan_micro_arbs(provider: &impl Provider) -> Vec<MicroArb> {
    let pairs = get_watched_pairs();
    let mut opportunities = vec![];

    for pair in &pairs {
        let routes = compute_all_routes(provider, pair).await;
        for route in routes {
            let profit = simulate_route(provider, &route).await;
            let gas = estimate_route_gas(provider, &route).await;

            // Solo incluir si profit > 3× gas
            if profit > gas * GAS_PRICE_MULTIPLIER {
                opportunities.push(MicroArb {
                    route: route.tokens.clone(),
                    expected_profit: profit,
                    gas_cost: gas,
                    net_profit: profit - gas,
                    confidence: route.confidence,
                });
            }
        }
    }

    opportunities.sort_by(|a, b| b.net_profit.cmp(&a.net_profit));
    opportunities.truncate(MAX_OPPORTUNITIES_PER_BLOCK);
    opportunities
}

async fn execute_micro_arbs(provider: &impl Provider, arbs: Vec<MicroArb>) {
    let mut total_profit = U256::ZERO;
    for arb in &arbs {
        match execute_single_arb(provider, arb).await {
            Ok(profit) => {
                total_profit += profit;
                tracing::info!("Micro-arb exitoso: profit={:?} confidence={:.2}", profit, arb.confidence);
            }
            Err(e) => {
                tracing::debug!("Micro-arb fallido: {} (route={:?})", e, arb.route);
            }
        }
    }
    tracing::info!("Batch completado: {} arbs, profit total={:?}", arbs.len(), total_profit);
}
```

## Configuración óptima por red (§7.2 sop_body)

| Red | Gas Cost | Profit Min | Ops/Hora | Volumen Ops | Beneficio/Hora | Estrategia ideal |
|-----|----------|-------------|----------|-------------|-----------------|-------------------|
| **Arbitrum** | $0.001-0.01 | $0.01 | 100-500 | $0.10-2.00 | $50-200 | Stable pools, USDC/USDT |
| **Base** | $0.0005-0.005 | $0.005 | 200-800 | $0.05-1.00 | $40-300 | Aerodrome pools |
| **Optimism** | $0.001-0.005 | $0.01 | 80-300 | $0.10-3.00 | $30-150 | Velodrome, wETH pairs |
| **zkSync Era** | $0.001-0.008 | $0.01 | 50-200 | $0.10-2.50 | $20-100 | Emergente, bajo gas |

## Criterios de selección de micro-trades

1. **Costo gas absoluto**: profit_min ≥ 2× costo_gas (margen seguridad).
2. **Tasa éxito estimada**: ≥ 90% (medida vs slippage estimado, mín 3× spread > slippage).
3. **Velocidad de ejecución**: ciclo deteccion-ejec < 100ms (en L2 generalmente fácil).
4. **Monitoreo de densidad**: ajustar dinámicamente umbrales para maximizar throughput sin comprometer success rate.

## Pares ideales para micro-arb por L2

- **Arbitrum**: USDC/USDT (stable arb), WETH/USDC (high vol).
- **Base**: AERO/USDC, BASE/ETH, USDC/USDB.
- **Optimism**: VELO pairs, OP/USDC.

## Invariantes
- Profit_min ≥ 3× gas (no 2×, no 1×).
- Success rate ≥ 90% medido en histórico (sino ajustar profit threshold up).
- Max 50 ops por bloque (evita saturar relay).
- Solo en L2s con gas < $0.01 (en Ethereum L1 micro-arb es inviable).
- Sort by net_profit descending para priorizar las más rentables.

## Cross-references
- Liquidity aggregation para encontrar best routes: `sop_liquidity_aggregation`.
- Risk management con position sizing: `sop_risk_management`.
- Bundle execution rapid: `sop_flashbots_bundles`.
