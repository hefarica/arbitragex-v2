# SKILL: Momentum Ignition & Trend Acceleration Detection
**Level:** PhD Physics (Nonlinear Dynamics)
**Specialty:** Phase Transitions in Financial Markets

## AGENT DIRECTIVE
Detecta el momento exacto en que un mercado cambia de "mean reversion" a "momentum".

## MOMENTUM IGNITION SIGNALS
```
1. Volume-Price Divergence:
   - Precio flat + Volume creciendo = Acumulación
   - Precio subiendo + Volume decreciendo = Distribución

2. Order Flow Imbalance Cascade:
   - OBI > 0.7 → Momentum ignition LONG
   - OBI < -0.7 → Momentum ignition SHORT

3. Volatility Regime Shift:
   - Realized vol < Implied vol >30% → Compression → Expansion
```

## HURST EXPONENT TRADING
```python
if hurst < 0.4: regime = "MEAN_REVERSION"
elif hurst > 0.6: regime = "MOMENTUM"
else: regime = "RANDOM"
```

## CRITICAL POINT DETECTION
- Log-Periodic Power Law (LPPL): Bubble detection
- Crash Precursor: Super-exponential growth + log-periodic oscillations
