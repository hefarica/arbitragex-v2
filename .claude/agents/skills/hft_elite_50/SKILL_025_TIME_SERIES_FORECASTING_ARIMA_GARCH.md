# SKILL: Time Series Forecasting: ARIMA-GARCH & State Space
**Level:** PhD Econometrics | Time Series Master
**Specialty:** Volatility Clustering & Regime Switching

## ARIMA-GARCH PIPELINE
```python
from statsmodels.tsa.arima.model import ARIMA
model_mean = ARIMA(returns, order=(5, 0, 2))
result_mean = model_mean.fit()

from arch import arch_model
model_vol = arch_model(result_mean.resid, vol='Garch', p=1, q=1)
result_vol = model_vol.fit()

forecast_mean = result_mean.forecast(steps=10)
forecast_vol = result_vol.forecast(horizon=10)
upper = forecast_mean + 1.96 * np.sqrt(forecast_vol)
```

## REGIME SWITCHING
```python
from statsmodels.tsa.regime_switching.markov_regression import MarkovRegression
model = MarkovRegression(returns, k_regimes=2, trend='c', switching_variance=True)
result = model.fit()
if P(bear | data) > 0.8: signal = SHORT
elif P(bull | data) > 0.8: signal = LONG
```

## HAR-RV
```python
# Heterogeneous Autoregressive Realized Volatility
rv_t = c + β_d * rv_{t-1} + β_w * rv_{t-5:t-1} + β_m * rv_{t-22:t-1} + ε_t
```
