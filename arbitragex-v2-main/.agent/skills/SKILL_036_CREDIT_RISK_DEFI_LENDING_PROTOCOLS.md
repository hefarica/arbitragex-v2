# SKILL: Credit Risk in DeFi Lending Protocols
**Level:** PhD Financial Risk | DeFi Credit Analyst
**Specialty:** Collateralization, Liquidation & Default Modeling

## HEALTH FACTOR
```python
health_factor = (collateral * liquidation_threshold) / debt
if health_factor < 1.0:
    profit = debt * liquidation_bonus * close_factor
    if profit > gas_cost * 2:
        queue_liquidation()
```

## INTEREST RATE MODELS
```python
# Aave piecewise linear
if U < U_optimal:
    rate = R0 + (U / U_optimal) * R_slope1
else:
    rate = R0 + R_slope1 + ((U - U_optimal) / (1 - U_optimal)) * R_slope2
# At U=95%: rate = 0% + 4% + (5%/10%)*75% = 41.5% APR
```

## PROTOCOL COMPARISON
```
Protocol    | Collateral Types | Liquidation | Insurance | Governance
Aave v3     | Multi-asset      | 5-10%       | Yes       | Decentralized
Compound v3 | Isolated markets | 5-8%        | No        | Decentralized
MakerDAO    | ETH, stables     | 13%         | No        | DAO
Euler       | Permissionless   | Variable    | No        | Decentralized
```
