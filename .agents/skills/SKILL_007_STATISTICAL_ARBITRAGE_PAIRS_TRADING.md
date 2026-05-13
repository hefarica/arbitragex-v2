# SKILL: Statistical Arbitrage & Pairs Trading HFT
**Level:** PhD Econometrics | Cointegration Master
**Specialty:** Mean Reversion & Factor Models

## AGENT DIRECTIVE
Encuentra relaciones estadísticas falsamente rotas y apuesta a su restauración.

## PAIR SELECTION
```python
# Cointegration Test (Engle-Granger)
# half_life = -ln(2) / θ
# Target: 1 hour < half_life < 2 days

# Hurst Exponent
# H < 0.5: Mean reverting (ideal)
# H = 0.5: Random walk (avoid)
# H > 0.5: Trending (avoid)
```

## ENTRY/EXIT RULES
```
Entry: Z-score > 2.0σ or < -2.0σ
Exit: Z-score crosses 0
Stop: Z-score > 3.5σ (relationship broken)
Sizing: Kelly Criterion fractional 0.3x
```

## MULTI-LEG STATARB
- Triangular Arbitrage: A/B, B/C, C/A
- Basket Trading: ETF vs components
- Cross-Asset: Gold miners vs Gold spot
