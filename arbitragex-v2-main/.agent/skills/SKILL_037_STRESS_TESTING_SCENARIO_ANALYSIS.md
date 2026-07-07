# SKILL: Stress Testing & Scenario Analysis
**Level:** PhD Risk Management | Scenario Planning Expert
**Specialty:** Tail Event Simulation & Portfolio Resilience

## HISTORICAL SCENARIOS
```python
scenarios = {
    '2008_CRISIS': {'sp500_drop': -57, 'vix_peak': 80},
    '2020_COVID': {'sp500_drop': -34, 'btc_drop': -50},
    '2022_CRYPTO_WINTER': {'btc_drop': -65, 'eth_drop': -68},
    'TERRA_LUNA': {'luna_drop': -99.99, 'ust_depeg': -100},
    'FTX': {'ftt_drop': -99, 'btc_drop': -25, 'sol_drop': -60}
}
```

## MONTE CARLO
```python
df, loc, scale = t.fit(returns)
simulated_paths = t.rvs(df, loc=loc, scale=scale, size=(10000, 252))
portfolio_pnl = np.dot(simulated_paths, weights)
var_95 = np.percentile(portfolio_pnl, 5)
cvar_95 = portfolio_pnl[portfolio_pnl <= var_95].mean()
```

## REVERSE STRESS TEST
```python
def reverse_stress_test(portfolio, max_drawdown_limit=-0.5):
    low, high = 0, 1.0
    while high - low > 0.001:
        mid = (low + high) / 2
        shocked_returns = apply_uniform_shock(portfolio.returns, mid)
        dd = calculate_max_drawdown(shocked_returns)
        if dd > max_drawdown_limit: high = mid
        else: low = mid
    return mid
```

## CONTINGENCY PLANNING
```
Yellow (DD > 10%): Reduce size 20%
Orange (DD > 20%): Activate hedges, increase cash
Red (DD > 30%): Full liquidation, preserve capital
Black (DD > 40%): Only high-conviction trades, max 10% exposure
```
