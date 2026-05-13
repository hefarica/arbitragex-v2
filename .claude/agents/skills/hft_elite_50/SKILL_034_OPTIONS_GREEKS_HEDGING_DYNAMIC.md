# SKILL: Options Greeks & Dynamic Hedging
**Level:** PhD Mathematical Finance
**Specialty:** Greeks Sensitivity & Continuous Hedging

## AGENT DIRECTIVE
Los griegos son tus sensores de riesgo. Hedge continuamente.

## BLACK-SCHOLES GREEKS
```python
def black_scholes_greeks(S, K, T, r, sigma, option_type='call'):
    d1 = (np.log(S/K) + (r + 0.5*sigma**2)*T) / (sigma*np.sqrt(T))
    d2 = d1 - sigma*np.sqrt(T)
    if option_type == 'call':
        delta = norm.cdf(d1)
        theta = (-S*norm.pdf(d1)*sigma/(2*np.sqrt(T)) - r*K*np.exp(-r*T)*norm.cdf(d2)) / 365
    else:
        delta = norm.cdf(d1) - 1
    gamma = norm.pdf(d1) / (S * sigma * np.sqrt(T))
    vega = S * norm.pdf(d1) * np.sqrt(T) / 100
    vanna = -norm.pdf(d1) * d2 / sigma
    volga = S * norm.pdf(d1) * np.sqrt(T) * d1 * d2 / sigma
    return {'delta': delta, 'gamma': gamma, 'theta': theta, 'vega': vega}
```

## GAMMA SCALING
```python
# P&L ≈ 0.5 * Γ * (ΔS)² + Θ * Δt + ν * Δσ
# Breakeven vol: σ_breakeven = √(2 * |Θ| / (Γ * S²))
```

## CRYPTO OPTIONS
```
- Deribit: BTC/ETH options, European
- Delta Exchange: Alt coin options
- Lyra: AMM options (on-chain)
- Ribbon: Structured products
```
