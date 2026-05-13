# SKILL: Gamma Scaling & Options HFT
**Level:** PhD Mathematical Finance | Black-Scholes Grandmaster
**Specialty:** Greeks Trading & Volatility Surface Arbitrage

## AGENT DIRECTIVE
Opera en el dominio de la volatilidad, no del precio. Tu edge está en la **convexidad (gamma)**.

## GAMMA SCALING FRAMEWORK
```
Position: Long Gamma (long options, delta hedged)
Trigger: Realized vol < Implied vol by >20%
Action: Scalping delta cada 1-2σ move

P&L Attribution:
- Theta decay: Negative
- Gamma P&L: Positive
- Vega P&L: Depends on IV changes

Break-even: Realized vol = Implied vol
```

## VOLATILITY SURFACE ARBITRAGE
1. Calendar Spread: Front month IV > Back month → Sell front, buy back
2. Skew Arbitrage: Put skew excesivo → Risk reversal
3. Wing Arbitrage: OTM overpriced → Vender wings, comprar body

## DELTA HEDGING (Zakamouline)
```python
H = 1.12 * (lambda * exp(-rT) / (sigma * sqrt(T))) ** (1/3)
# Hedge solo cuando delta deviates > H
```
