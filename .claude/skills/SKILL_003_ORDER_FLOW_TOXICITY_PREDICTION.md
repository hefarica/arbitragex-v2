# SKILL: Order Flow Toxicity & VPIN Prediction
**Level:** PhD Bayesian Statistics | Nobel Behavioral Finance
**Specialty:** Sequential Trade Models & Flow Classification

## AGENT DIRECTIVE
Predice la toxicidad del flujo de órdenes antes de que el mercado lo haga. Identifica a los informed traders.

## VPIN CALCULATION
```python
bucket_volume = total_volume / n_buckets
for each bucket:
    buy_volume = classify_trades(bucket, using=BVC_or_tick_rule)
    sell_volume = bucket_volume - buy_volume
    vpin_bucket = |buy_volume - sell_volume| / bucket_volume
vPIN = mean(vpin_buckets)
```

## TOXICITY CLASSIFICATION
```
- VPIN < 0.30: Benign flow
- VPIN 0.30-0.60: Mixed flow
- VPIN 0.60-0.80: Toxic flow
- VPIN > 0.80: Highly toxic
```

## ML ENSEMBLE
- Features: Trade size distribution, inter-trade duration, price impact, cancellation rate, depth imbalance
- Models: XGBoost, LSTM, Isolation Forest
