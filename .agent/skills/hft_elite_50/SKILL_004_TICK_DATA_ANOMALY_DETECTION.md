# SKILL: Tick Data Anomaly Detection & Fat Finger Prevention
**Level:** PhD Applied Mathematics | Chaos Theory Specialist
**Specialty:** Real-time Outlier Detection in High-Frequency Streams

## DETECTION LAYERS
```
Layer 1: Price Sanity
  - |price_change| > 5σ → FLAG
  - Price = 0 → REJECT
  - Price deviation > 20% from VWAP → HALT

Layer 2: Volume Sanity
  - Volume = 0 → FLAG
  - Volume > 10x average → INVESTIGATE

Layer 3: Timestamp Sanity
  - Out-of-sequence → REORDER
  - Future timestamps → REJECT

Layer 4: Cross-Market Consistency
  - Deviation vs primary > 3σ → ARBITRAGE or ERROR
```

## KALMAN FILTER
```python
x = [mid_price, volume, 0]  # State
P = identity(3) * 1000      # Covariance
Q = diag([0.1, 100, 0.01])  # Process noise
R = diag([sigma_price^2, sigma_volume^2])  # Measurement noise

for each tick:
    x = F @ x
    P = F @ P @ F.T + Q
    y = z - H @ x
    K = P @ H.T @ inv(H @ P @ H.T + R)
    x = x + K @ y
    P = (I - K @ H) @ P
```
