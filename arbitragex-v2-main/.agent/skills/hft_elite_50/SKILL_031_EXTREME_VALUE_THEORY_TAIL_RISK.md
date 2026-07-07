# SKILL: Extreme Value Theory & Tail Risk Management
**Level:** PhD Statistics | Actuarial Risk Grandmaster
**Specialty:** Tail Risk Quantification & Catastrophic Event Modeling

## AGENT DIRECTIVE
El dinero no se pierde en el cuerpo de la distribución. Se pierde en las **colas**.

## HILL ESTIMATOR
```python
def hill_estimator(returns, k=100):
    sorted_returns = np.sort(np.abs(returns))[::-1]
    tail_obs = sorted_returns[:k]
    xi = 1 / k * sum(np.log(tail_obs / tail_obs[k-1]))
    return xi
# ξ > 0: Heavy tail (crypto typical: 0.3-0.5)
```

## GPD (POT)
```python
from scipy.stats import genpareto
threshold = np.percentile(np.abs(returns), 95)
excesses = np.abs(returns)[np.abs(returns) > threshold] - threshold
params = genpareto.fit(excesses)
xi, mu, sigma = params
var = threshold + sigma/xi * ((len(returns)/len(excesses) * (1-p)) ** (-xi) - 1)
es = var / (1 - xi) + (sigma - xi*threshold) / (1 - xi)
```

## CVaR OPTIMIZATION
```python
from scipy.optimize import linprog
def minimize_cvar(returns, alpha=0.95):
    c = [0]*n_assets + [1/(n_scenarios*(1-alpha))]*n_scenarios + [1]
    # Linear programming formulation (Rockafellar & Uryasev)
    result = linprog(c, A_ub=A, b_ub=b, A_eq=A_eq, b_eq=b_eq, bounds=bounds)
    return result.x[:n_assets]
```

## TAIL RISK HEDGING
```
1. Options: OTM puts (premium ~2-5% anual)
2. VIX Futures: Long VIX como hedge
3. Crypto: Stablecoin buffers 20-30%, inverse perpetuals
4. Dynamic: Aumentar hedge cuando realized vol > implied vol
```
