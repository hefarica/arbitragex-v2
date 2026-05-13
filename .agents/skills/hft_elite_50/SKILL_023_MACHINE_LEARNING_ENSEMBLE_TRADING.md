# SKILL: Machine Learning Ensemble for Trading
**Level:** PhD Machine Learning | Kaggle Grandmaster
**Specialty:** Ensemble Methods & Feature Engineering

## AGENT DIRECTIVE
Construye un **ejército de modelos** que voten.

## ENSEMBLE ARCHITECTURE
```python
models = {
    'xgboost': XGBClassifier(n_estimators=500, max_depth=5),
    'lightgbm': LGBMClassifier(n_estimators=500, num_leaves=31),
    'catboost': CatBoostClassifier(iterations=500, depth=6),
    'random_forest': RandomForestClassifier(n_estimators=200),
    'neural_net': MLPClassifier(hidden_layer_sizes=(128,64,32))
}
meta_model = LogisticRegression(C=1.0)
weights = softmax(recent_accuracy / temperature)
ensemble_prediction = sum(w * pred for w, pred in zip(weights, predictions))
```

## FEATURE ENGINEERING
```python
features = {
    'returns': log(close / close.shift(1)),
    'rsi': RSI(close, 14),
    'macd': MACD(close, 12, 26, 9),
    'obi': (bid_volume - ask_volume) / (bid_volume + ask_volume),
    'vpin': calculate_vpin(trades, window=50),
    'lead_lag': cross_correlation(exchange_a, exchange_b, lag=5),
    'hurst': estimate_hurst(returns, window=100)
}
```

## WALK-FORWARD CV
```python
for i in range(train_size, len(X) - test_size, test_size):
    X_train = X[i-train_size:i]; y_train = y[i-train_size:i]
    X_test = X[i:i+test_size]; y_test = y[i:i+test_size]
    model.fit(X_train, y_train)
    pred = model.predict(X_test)
```
