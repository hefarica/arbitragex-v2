---
name: sop_token_pool_selection
description: Cuando se evalúe selección de tokens, criterios de pools, mejores pares por cadena, o construcción del token allowlist del bot. Activa con triggers "selección tokens", "criterios pools", "mejores pares cadena", "TVL volumen pool", "ratio volumen TVL", "fee tier elección", "token allowlist". Trae los criterios cuantitativos del SOP §10 + tablas por cadena con TVL y volumen.
type: arbx_security
source_section: SOP_ArbitrageX_2026.pdf §10
---

# Selección de Tokens y Pools

## Criterios de selección (§10.1)

| Criterio | Mínimo aceptable | Cómo medir |
|----------|------------------|------------|
| **Capitalización mercado** | $10M (tokens principales), $1M (high-risk opps) | CoinGecko, CMC |
| **Volumen 24h** | $1M | DEX volume aggregators, DeFi Llama |
| **Profundidad liquidez** | impacto < 0.1% para swap $10K-$100K | eth_call simulación |
| **Volatilidad** | mayor → más oportunidades pero más riesgo | TradingView, oráculos |
| **Correlación pares** | múltiples DEXs con el mismo par | DEXScreener |

## Pares recomendados por cadena (§10.2)

| Cadena | Pares principales | TVL Total | Vol. 24h |
|--------|-------------------|-----------|----------|
| **Ethereum L1** | WETH/USDC, WBTC/ETH, USDC/USDT, WETH/USDT, LINK/ETH | $95B | $4.2B |
| **BSC** | WBNB/USDT, CAKE/BNB, BUSD/USDT, ETH/BTC | $5.8B | $1.8B |
| **Arbitrum** | WETH/USDC, GMX/ETH, ARB/ETH, RDNT/ETH | $3.2B | $0.9B |
| **Optimism** | WETH/USDC, OP/ETH, SNX/ETH, DAI/USDC | $1.5B | $0.5B |
| **Base** | WETH/USDC, BASE/ETH, AERO/ETH, USDC/USDB | $2.1B | $0.7B |
| **Polygon** | WMATIC/USDC, WETH/USDC, QUICK/ETH, DAI/USDC | $1.8B | $0.6B |

## Métricas de calidad de pools (§10.3)

Para cada pool candidato, calcular:
1. **TVL** (Total Value Locked) — profundidad.
2. **Ratio volumen/TVL** — actividad relativa. **Pools con ratio > 30% son los más prometedores** (alta rotación de capital + frecuentes desequilibrios de precio).
3. **Fee tier** — costo por operación.
4. **Spread promedio** — volatilidad del precio.
5. **Historial de exploits** — descartar pools vulnerables.

## Token allowlist inicial (top 5 blue chips para Ethereum)

| Token | Address | Risk Score | Justificación |
|-------|---------|------------|---------------|
| WETH | `0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2` | 0.10 | Estándar wrapped ETH, máxima liquidez |
| USDC | `0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48` | 0.10 | Stablecoin Circle, multi-chain |
| USDT | `0xdac17f958d2ee523a2206206994597c13d831ec7` | 0.10 | Stablecoin Tether, máximo volumen |
| DAI | `0x6b175474e89094c44da98b954eedeac495271d0f` | 0.10 | Stablecoin descentralizada (MakerDAO) |
| WBTC | `0x2260fac5e5542a773aa44fbcfedf7c193bc2c599` | 0.15 | Bitcoin wrapped, custodia BitGo |

## DEXes monitoreados por estrategia

| Estrategia | DEXes prioritarios |
|------------|---------------------|
| Triangular | Uniswap V3 (15+ chains), Curve (stables), Balancer (multi-asset), SushiSwap (L2s emergentes) |
| Stable arb | Curve >> Balancer = Uniswap V3 fee 0.01% |
| Memecoin (alto riesgo) | Uniswap V3 + nuevos AMMs (Maverick, Aerodrome) |
| L2 nativo | Aerodrome (Base), Velodrome (Optimism), TraderJoe (Arbitrum) |

## Filtros automáticos pre-trade

```rust
fn pool_passes_filters(pool: &Pool) -> bool {
    pool.tvl_usd >= 1_000_000.0          // ≥ $1M (≥ $5M para Ethereum)
        && pool.volume_24h_usd >= 500_000.0  // ≥ $500K
        && pool.last_swap_age_seconds < 600   // actividad reciente
        && !pool.has_exploit_history          // sin exploits conocidos
        && pool.token0_in_allowlist
        && pool.token1_in_allowlist
}
```

## Invariantes
- Token NO en allowlist → no se opera, jamás.
- Pool con TVL < $500K → no se opera (excepto explícita override).
- Pool sin actividad en últimos 10min → no se opera (probable sin liquidez efectiva).
- Risk score 0.0 = bloqueado total. Risk score 1.0 = max desconfianza pero permitido.
- Defaults blue chips: 0.10. Memecoin: 0.30. Token nuevo: 0.50.

## Cross-references
- Detección de scams (paso siguiente): `sop_scam_detection`.
- Por estrategia, pares óptimos: `sop_dex_triangular` §3.2 (triples), `sop_cex_dex` (CEX pairs), `sop_jit_liquidity` (V3 pools).
- Tabla 8 del sop_body con TVL/volumen/DEXs/volatilidad criterios completos.
