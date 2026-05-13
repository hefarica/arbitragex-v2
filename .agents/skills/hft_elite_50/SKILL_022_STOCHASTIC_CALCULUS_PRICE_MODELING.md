# SKILL: Stochastic Calculus & Advanced Price Modeling
**Level:** PhD Mathematical Finance | Stochastic Analysis Expert
**Specialty:** SDEs, Jump Processes & Exotic Derivatives

## AGENT DIRECTIVE
Modela el precio como proceso estocástico multidimensional.

## MODELS HIERARCHY
```
Level 1: Black-Scholes (GBM)
Level 2: Merton Jump-Diffusion
Level 3: Heston Stochastic Volatility
Level 4: Bates (Heston + Jumps)
Level 5: Rough Volatility (H < 0.5)
```

## HESTON SIMULATION
```python
def simulate_heston(S0, v0, r, kappa, theta, xi, rho, T, N, M):
    dt = T / N
    S = np.zeros((M, N+1)); v = np.zeros((M, N+1))
    S[:,0] = S0; v[:,0] = v0
    for t in range(N):
        dW1 = np.random.normal(0, np.sqrt(dt), M)
        dW2 = rho * dW1 + np.sqrt(1-rho**2) * np.random.normal(0, np.sqrt(dt), M)
        v[:,t+1] = np.maximum(v[:,t] + kappa*(theta-v[:,t])*dt + xi*np.sqrt(v[:,t])*dW2, 0)
        S[:,t+1] = S[:,t] * np.exp((r - 0.5*v[:,t])*dt + np.sqrt(v[:,t])*dW1)
    return S, v
```

## ROUGH VOLATILITY
```
H ≈ 0.1 (mucho más rough que Browniano H=0.5)
Implicación: OTM options subvaloradas por modelos tradicionales
Edge: Comprar wings en volatilidad rough
```
