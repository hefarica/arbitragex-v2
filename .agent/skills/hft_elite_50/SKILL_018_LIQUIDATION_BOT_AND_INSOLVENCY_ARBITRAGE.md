# SKILL: Liquidation Bot & Insolvency Arbitrage
**Level:** PhD Financial Risk | Credit Derivatives Expert
**Specialty:** DeFi Liquidation Mechanics & Auction Theory

## AGENT DIRECTIVE
Sé el primer bot en detectar cuentas insolventes.

## HEALTH FACTOR
```python
health_factor = (collateral * liquidation_threshold) / debt
if health_factor < 1.0:
    profit = debt * liquidation_bonus * close_factor
    gas_cost = estimate_gas()
    if profit > gas_cost * 2:
        queue_liquidation()
```

## FLASH LOAN LIQUIDATION
```solidity
// 1. Flash loan del debt asset
// 2. Liquidar cuenta insolvente
// 3. Recibir collateral + bonus
// 4. Swap collateral → debt asset
// 5. Repay flash loan
// 6. Profit = (ETH_received * price - DAI_borrowed) - gas - fees
```

## COMPETITIVE DYNAMICS
```
- PGA: Múltiples bots compiten
- Gas bidding: Outbid por 1-2 gwei
- Bundle: Incluir en Flashbots
- Pre-approve tokens, multicall para batch
```
