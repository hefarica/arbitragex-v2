# SKILL: Concentrated Liquidity & Uniswap v3 Mathematics
**Level:** PhD Applied Mathematics
**Specialty:** Concentrated Liquidity Positioning & IL Optimization

## AGENT DIRECTIVE
Uniswap v3 es el order book disfrazado de AMM.

## LIQUIDITY POSITIONING
```python
L = C / ((1/√P - 1/√P_b) * P + (√P - √P_a))
x = L * (1/√P - 1/√P_b)  # token0
y = L * (√P - √P_a)      # token1
```

## FEE TIERS
```
0.05%: Stable pairs (USDC/USDT)
0.3%: Standard pairs (ETH/USDC)
1.0%: Exotic pairs (low volume)
APY_fee = (volume_24h * fee_tier * share * 365) / capital
```

## IMPERMANENT LOSS
```python
def impermanent_loss(price_ratio):
    return 2 * np.sqrt(price_ratio) / (1 + price_ratio) - 1
# ETH $2000 → $3000 (ratio=1.5): IL = -2.0%
```

## DELTA HEDGING FOR LPs
```python
if P < P_a: delta = 0  # 100% token0
elif P > P_b: delta = 1  # 100% token1
else:
    delta = (√P - √P_a) / (2 * √P - √P_a - P/√P_b)
# Hedge con perps si delta > 0.5
```
