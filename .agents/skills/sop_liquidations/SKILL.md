---
name: sop_liquidations
description: Cuando se diseñe, implemente o evalúe MEV de liquidaciones de préstamos (Aave/Compound/MakerDAO). Activa con triggers "liquidación MEV", "health factor", "Aave liquidation", "Compound liquidate", "bonus de liquidación", "underwater position", "monitor health factor". Trae el código Alloy del SOP §6 con interface ILendingPool + threshold 1.05 + bonus 5-15%.
type: arbx_strategy_reference
source_section: SOP_ArbitrageX_2026.pdf §6 + sop_body.pdf §6
---

# Liquidaciones MEV

## Concepto (§6.1)
Cuando colateral cae bajo umbral en protocolo de préstamos (Aave/Compound/MakerDAO), cualquiera puede liquidar la posición. Liquidador compra colateral con descuento (5-15%) y cierra deuda → beneficio inmediato $1K-$50K+ por op.

## Health Factor (HF)
Métrica clave del protocolo. **HF < 1.0 → posición elegible para liquidación.**
- En Aave: HF = sum_ponderado(colateral_value × liquidation_threshold) / sum(debt_value).
- Factores ponderación dependen de tipo de activo + volatilidad.

## Implementación de referencia (§6.2)

```rust
use alloy::primitives::{address, U256};
use alloy::providers::Provider;
use alloy::sol_types::SolCall;

const LIQUIDATION_THRESHOLD: f64 = 1.05; // Margin of safety
const MIN_LIQUIDATION_PROFIT: u128 = 500_000_000_000u128; // 500 USDC

#[derive(SolCall)]
#[sol(name = "LendingPool")]
interface ILendingPool {
    function getUserAccountData(address user)
        external returns (
            uint256 totalCollateralBase,
            uint256 totalDebtBase,
            uint256 availableBorrowsBase,
            uint256 currentLiquidationThreshold,
            uint256 ltv,
            uint256 healthFactor
        );
}

async fn monitor_liquidations(provider: &impl Provider) {
    let aave_lending_pool = address!("7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9");
    loop {
        let user_accounts = get_at_risk_users(provider, aave_lending_pool).await;
        for user in user_accounts {
            let health = compute_health_factor(provider, aave_lending_pool, user).await;
            if health < LIQUIDATION_THRESHOLD {
                let profit = simulate_liquidation(provider, aave_lending_pool, user).await;
                if profit > MIN_LIQUIDATION_PROFIT {
                    execute_liquidation(provider, aave_lending_pool, user, profit).await;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
```

## Protocolos y parámetros (§6.1 sop_body)

| Protocolo | TVL | Umbral Liq | Bonus Liq | Gas Est | Complejidad |
|-----------|-----|-------------|-----------|---------|-------------|
| Aave v3 | $12B | 105-110% | 5% | 250-450K | Media — batch liquidation |
| Compound v3 | $3B | 107-112% | 8% | 200-350K | Media |
| MakerDAO | $8B | Variable por vault | 13% (auction) | 300-500K | **Alta — auction system** |
| Spark Protocol | $1.5B | 105-110% | 5% | 220-380K | Media |

## Riesgos y competencia (§6.3)
- **Colateral subacuático**: si precio cae demasiado rápido, liquidación puede dar pérdida.
- **Guerras de gas**: múltiples bots compiten por liquidar primero. Solución: **Flashbots bundles** garantizan ejecución sin guerras.
- **Fallas contrato**: verificar lending pool no actualizado recientemente.

## Estrategia óptima
1. Monitorear ALL protocolos simultáneamente (Aave + Compound + Maker + Spark).
2. Calcular en tiempo real ratio beneficio/gas para cada oportunidad.
3. Priorizar por net_profit (no por gross_bonus).
4. Ejecutar SIEMPRE vía Flashbots bundle (no mempool público).
5. Mantener múltiples RPC endpoints para redundancia.

## Invariantes
- HF < 1.05 (margen de seguridad), no < 1.0 estricto.
- Min liquidation profit: 500 USDC (configurable, no menos).
- Bundle vía Flashbots OBLIGATORIO (sin esto = guerra de gas + bot pierde).
- Simulación pre-broadcast OBLIGATORIA (revm + estado on-chain real).

## Cross-references
- Bundle construction: `sop_flashbots_bundles`.
- Risk management (gas, slippage): `sop_risk_management`.
- Por qué es estrategia "Alta" (no extrema): `sop_strategy_matrix`.
