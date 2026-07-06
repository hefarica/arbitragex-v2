---
name: sop_dex_triangular
description: Cuando se diseñe, implemente, debugee o evalúe arbitraje triangular DEX (ciclo de 3 swaps en pools distintos). Activa con triggers "arbitraje triangular", "ciclo WETH-USDC-UNI", "Bellman-Ford triangular", "quoter Uniswap V3", "Alloy sol! macro", "flash loan triangular". Trae el código Alloy del SOP §4 para query precios on-chain, condiciones óptimas y selección de pares.
type: arbx_strategy_reference
source_section: SOP_ArbitrageX_2026.pdf §4 + sop_body.pdf §3
---

# Arbitraje Triangular DEX

## Concepto
Ciclo de 3 swaps a través de 3 tokens distintos en pools diferentes, donde el monto final supera el inicial. Ciclo clásico: **WETH → USDC → UNI → WETH**.

## Modelo matemático (§4.1)
```
profit = amount × (1 - f₁) × (1 - f₂) × (1 - f₃) - amount
```
Donde f₁, f₂, f₃ son las fees de cada pool (0.3% Uniswap V2, variable V3). Rentable cuando producto de tasas netas > 1, es decir, discrepancia precios > suma de fees.

## Condiciones óptimas
- **Alta volatilidad**: noticias, listings, grandes movimientos.
- **Baja liquidez relativa**: pools con TVL <$1M más susceptibles a desequilibrios.
- **Listings frescos**: pools recién creados con precios desequilibrados.
- **Pares correlacionados**: WETH/USDC, WETH/WBTC, WBTC/USDC.

## Implementación de referencia con Alloy (§4.3)

```rust
use alloy::primitives::{address, U256};
use alloy::sol_types::SolCall;
use alloy::providers::Provider;

#[derive(SolCall)]
#[sol(name = "Quoter")]
interface IQuoter {
    function quoteExactInputSingle(
        address tokenIn,
        address tokenOut,
        uint24 fee,
        uint256 amountIn,
        uint160 sqrtPriceLimitX96
    ) external returns (uint256 amountOut);
}

async fn check_triangular_arb(
    provider: &impl Provider,
    token_a: Address, token_b: Address, token_c: Address,
    amount: U256,
) -> Option<U256> {
    let out_ab = quoter_quote(provider, token_a, token_b, amount).await?;
    let out_bc = quoter_quote(provider, token_b, token_c, out_ab).await?;
    let out_ca = quoter_quote(provider, token_c, token_a, out_bc).await?;

    if out_ca > amount {
        return Some(out_ca - amount);
    }
    None
}

async fn quoter_quote(
    provider: &impl Provider,
    token_in: Address,
    token_out: Address,
    amount_in: U256,
) -> Option<U256> {
    let quoter = address!("b27308f9F90D607463bb33eA1BeBb41C27CE5AB6");
    let call = IQuoter::quoteExactInputSingleCall {
        tokenIn: token_in,
        tokenOut: token_out,
        fee: 3000, // 0.3% fee tier
        amountIn: amount_in,
        sqrtPriceLimitX96: U256::ZERO,
    };
    let result = provider.call(&call).into_transaction_request()
        .to(quoter).call().await.ok()?;
    Some(result._0)
}
```

## DEXes principales monitoreados

| DEX | Cadenas | Fee típico | TVL aprox |
|-----|---------|------------|-----------|
| Uniswap V3/V4 | ETH, ARB, OP, BASE, MATIC | 0.01-1% | $6.5B |
| SushiSwap | ETH, BSC, ARB, MATIC | 0.3% | $1.2B |
| Curve Finance | ETH, ARB, MATIC | 0.04% | $3.8B |
| Balancer V2 | ETH, ARB, MATIC | 0.01-1% | $2.1B |
| 1inch (aggregator) | Multi-chain | Variable | aggregator |
| PancakeSwap | BSC, ETH, ARB | 0.01-0.25% | $2.5B |
| TraderJoe | ARB, AVAX | 0.3% | $0.8B |

## Mejores triples (§3.2 sop_body) para arbitraje triangular
- **WETH-USDC-LINK**: LINK con liquidez asimétrica entre Uniswap V3 vs SushiSwap.
- **USDC-USDT-DAI**: alta frecuencia, bajo riesgo, oportunidades por diferencias AMM.
- **WETH-WBTC-ARB**, **WETH-AAVE-DAI**, **WETH-OP-USDC**.

## Mitigación de riesgos (§4.5)
- **Slippage guard**: máx 0.5% desviación entre precio esperado y ejecutado.
- **Deadline**: cada swap incluye deadline 30s.
- **Gas threshold**: solo ejecutar si profit neto ≥ 3× costo gas estimado.
- **Pre-simulation**: SIEMPRE simular con revm antes del envío.

## Invariantes
- NUNCA ejecutar sin simulación revm previa.
- Profit estimado ≥ 3× gas (no ≥ gas, no 2×).
- Slippage máx 0.5% por swap.
- Deadline 30s no extensible.

## Cross-references
- Provider Alloy + flash loan: ver `sop_csa_architecture` §2.3.
- Bundle atómico para ejecutar el ciclo: ver `sop_flashbots_bundles`.
- Bellman-Ford generalizado para detectar ciclos: ver `sop_atomic_route_construction`.
