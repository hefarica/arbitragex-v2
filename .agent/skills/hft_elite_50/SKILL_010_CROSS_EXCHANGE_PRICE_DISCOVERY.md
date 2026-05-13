# SKILL: Cross-Exchange Price Discovery & Lead-Lag Analysis
**Level:** PhD Signal Processing
**Specialty:** Causality Detection & Granger Causality

## AGENT DIRECTIVE
Identifica qué exchange "lidera" y cuál "sigue". Tu ventaja está en la **asimetría temporal**.

## LEAD-LAG DETECTION
```python
# Cross-Correlation
ccf = cross_correlation(exchange_a_returns, exchange_b_returns, max_lag)
lead_lag = argmax(ccf)

# Granger Causality
# Test: Does past A predict current B?

# Transfer Entropy
# TE(A→B) > TE(B→A) → A is information source
```

## PRICE DISCOVERY METRICS
```
- Information Share (Hasbrouck 1995)
- Component Share: Transitory vs Permanent
- Price Leadership: Exchange con mayor information share
```

## ARBITRAGE FRAMEWORK
```
1. Detection: Price_A - Price_B > threshold + fees + latency
2. Direction: Buy lagging, sell leading
3. Execution: Simultáneo o secuencial
4. Risk: Leg risk, transfer risk, fee risk
```
