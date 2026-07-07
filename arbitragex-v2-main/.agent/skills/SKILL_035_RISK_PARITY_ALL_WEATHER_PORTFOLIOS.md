# SKILL: Risk Parity & All-Weather Portfolio Construction
**Level:** PhD Portfolio Theory | Bridgewater-style Architect
**Specialty:** Equal Risk Contribution & Macro Diversification

## AGENT DIRECTIVE
No apuestes al retorno. Apuesta al **riesgo**.

## RISK PARITY
```python
def risk_parity_weights(cov_matrix):
    n = len(cov_matrix)
    def objective(w):
        portfolio_vol = np.sqrt(w @ cov_matrix @ w)
        marginal_risk = (cov_matrix @ w) / portfolio_vol
        risk_contrib = w * marginal_risk
        target = portfolio_vol / n
        return sum((risk_contrib - target)**2)
    result = opt.minimize(objective, np.ones(n)/n, constraints={'type': 'eq', 'fun': lambda w: sum(w) - 1}, bounds=[(0,1)]*n)
    return result.x
```

## ALL-WEATHER (Dalio)
```
30% Stocks | 40% Long-term Bonds | 15% Intermediate Bonds | 7.5% Gold | 7.5% Commodities
Crypto Adaptation:
30% BTC/ETH | 30% Stablecoin Yield | 20% DeFi | 10% Gold-backed | 10% Commodity tokens
```

## LEVERAGED CARRY
```
- Borrow stablecoin en Aave (3% APR)
- Buy spot en DEX
- Short perpetual con 2x leverage
- Funding income: 30% APR
- Net: 54% sobre capital propio
```
