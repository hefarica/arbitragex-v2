# SKILL: Dynamic Portfolio Optimization & Robust Methods
**Level:** PhD Operations Research | Robust Optimization Expert
**Specialty:** Multi-Period Stochastic Programming

## AGENT DIRECTIVE
El mercado no es estático. Tu portfolio tampoco.

## ROBUST MEAN-VARIANCE
```python
import cvxpy as cp
w = cp.Variable(n)
epsilon = 0.1  # Uncertainty radius
objective = cp.Minimize(cp.quad_form(w, Sigma) - mu @ w + epsilon * cp.norm(w, 2))
constraints = [cp.sum(w) == 1, w >= 0]
prob = cp.Problem(objective, constraints)
prob.solve()
```

## BLACK-LITTERMAN
```python
Pi = delta * Sigma @ w_mkt
P = np.array([[1,0,0], [0,1,-1]])  # Views
Q = np.array([0.20, 0.05])
Omega = np.diag([0.05**2, 0.03**2])
tau = 0.025
M = np.linalg.inv(np.linalg.inv(tau * Sigma) + P.T @ np.linalg.inv(Omega) @ P)
mu_bl = M @ (np.linalg.inv(tau * Sigma) @ Pi + P.T @ np.linalg.inv(Omega) @ Q)
```

## TRANSACTION COSTS
```python
def transaction_cost(w_new, w_old, prices, volumes):
    delta = w_new - w_old
    notional = np.abs(delta) * portfolio_value
    fixed = 1.0 * (np.abs(delta) > 0.001).sum()
    linear = 0.001 * notional
    quadratic = 0.5 * eta * (notional / volumes) ** 0.5
    return fixed + linear + quadratic
```
