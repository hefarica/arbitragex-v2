# SKILL: Genetic Algorithms & Evolutionary Portfolio Optimization
**Level:** PhD Evolutionary Computation
**Specialty:** Multi-Objective Optimization & Strategy Evolution

## AGENT DIRECTIVE
Evoluciona estrategias de trading como especies biológicas.

## GA PORTFOLIO
```python
from deap import base, creator, tools, algorithms

def evaluate_portfolio(individual, returns, cov_matrix):
    weights = np.array(individual) / np.array(individual).sum()
    portfolio_return = np.dot(weights, returns.mean()) * 252
    portfolio_vol = np.sqrt(np.dot(weights.T, np.dot(cov_matrix, weights))) * np.sqrt(252)
    sharpe = portfolio_return / portfolio_vol
    max_dd = calculate_max_drawdown(weights, returns)
    return sharpe, -max_dd

toolbox.register("mate", tools.cxBlend, alpha=0.5)
toolbox.register("mutate", tools.mutPolynomialBounded, low=0, up=1, eta=20, indpb=0.1)
toolbox.register("select", tools.selNSGA2)
result = algorithms.eaMuPlusLambda(pop, toolbox, mu=100, lambda_=200, cxpb=0.7, mutpb=0.3, ngen=100)
```

## COEVOLUTION
```python
# Population A: Market Makers
# Population B: Informed Traders
# Population C: Noise Traders
# Evolucionar cada población contra las otras
```

## HYPERPARAMETER OPTIMIZATION
```python
import optuna
def objective(trial):
    fast_ma = trial.suggest_int('fast_ma', 5, 50)
    slow_ma = trial.suggest_int('slow_ma', 20, 200)
    strategy = MACD_Strategy(fast_ma, slow_ma)
    returns = backtest(strategy, data)
    return returns.mean() / returns.std() * np.sqrt(252)
study = optuna.create_study(direction='maximize')
study.optimize(objective, n_trials=1000)
```
