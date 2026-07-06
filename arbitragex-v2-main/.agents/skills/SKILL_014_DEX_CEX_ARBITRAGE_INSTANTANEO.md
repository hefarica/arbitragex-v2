# SKILL: DEX-CEX Arbitrage Instantáneo
**Level:** PhD Quantitative Finance
**Specialty:** Cross-Venue Price Synchronization

## AGENT DIRECTIVE
Sé el puente entre centralizado y descentralizado.

## EXECUTION MATHEMATICS
```python
price_dex = getAmountOut(amount_in, reserve_in, reserve_out)
price_cex = get_best_ask()
spread = price_cex - price_dex
fees = taker_fee_cex + gas_cost_eth + swap_fee_uniswap
net_profit = spread - fees
if net_profit > min_profit_threshold:
    execute_arbitrage()
```

## SPEED OPTIMIZATION
```
1. Pre-approval de tokens
2. Private mempool (Flashbots Protect)
3. CEX: WebSocket precios, REST ejecución
4. Parallel execution: Preparar tx DEX mientras CEX ejecuta
```

## PROFITABILITY
```
- Average opportunity: $50-$500 por trade
- Frequency: 10-50 por día
- Capital: $10k-$100k por lado
- ROI diario: 0.1-0.5%
```
