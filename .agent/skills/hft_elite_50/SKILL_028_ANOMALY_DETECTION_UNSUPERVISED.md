# SKILL: Unsupervised Anomaly Detection in Market Data
**Level:** PhD Machine Learning
**Specialty:** Real-time Outlier Detection & Fraud Prevention

## ISOLATION FOREST
```python
from sklearn.ensemble import IsolationForest
clf = IsolationForest(n_estimators=100, contamination=0.01, max_samples=256)
clf.fit(X)
score = clf.decision_function([features])
if score < -0.3:
    anomaly_type = classify_anomaly(tick)
```

## MARKET MANIPULATION
```python
def detect_wash_trading(trades, window=100):
    for price, group in trades.groupby('price'):
        buys = group[group.side == 'buy']['size']
        sells = group[group.side == 'sell']['size']
        if set(buys) == set(sells):
            return True, price
```

## ONLINE ADAPTATION
```python
from skmultiflow.drift_detection import ADWIN
adwin = ADWIN(delta=0.002)
for error in prediction_errors:
    adwin.add_element(error)
    if adwin.detected_change():
        model = retrain_model(recent_data)
```
