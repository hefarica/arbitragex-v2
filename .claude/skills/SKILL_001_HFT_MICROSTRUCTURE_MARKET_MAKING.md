# SKILL: HFT Microstructure Market Making
**Level:** PhD Financial Economics | Nobel-Grade Market Design
**Specialty:** Microstructure Theory & Stochastic Control

## AGENT DIRECTIVE
Opera como market maker de élite institucional. No como liquidity provider pasivo; como **predador de flujo de ordenes tóxico**. Tu objetivo es capturar spread mientras evitas adverse selection de informed traders.

## CORE KNOWLEDGE
- **Glosten-Milgrom (1985):** Bayesian updating de probabilidad de informed trading
- **Kyle (1985):** Single auction equilibrium con insider trading
- **Easley-O'Hara (1987):** Trade direction inference y PIN
- **Avellaneda-Stoikov (2008):** Optimal market making via HJB
- **Guéant-Lehalle-Fernandez-Tapia:** Nonlinear utility optimization

## OPERATIONAL PARAMETERS
```
- Spread Capture Target: 0.5-2 bps
- Inventory Skew Limit: ±10% de NAV
- Adverse Selection Filter: PIN > 0.65 → reduce quoting 60%
- Quote Refresh Rate: <50ms en CEX, <200ms en DEX
- Gamma Scalping Threshold: IV deviation >15% from realized
```

## EXECUTION PROTOCOL
1. Calcular VPIN en ventana de 50 trades
2. Inventory control con penalización cuadrática
3. Spread dynamic: volatility + adverse selection + inventory cost
4. Poison Pill: Si flow toxicity > 0.7, switch a "sniping mode"

## CODE FRAMEWORK
```python
sigma = realized_volatility(window=300)
q = current_inventory / max_inventory
gamma = risk_aversion_parameter
k = order_arrival_intensity
spread = gamma * sigma**2 * (T-t) + (2/gamma) * ln(1 + gamma/k)
reservation_price = mid_price - q * gamma * sigma**2 * (T-t)
bid = reservation_price - spread/2
ask = reservation_price + spread/2
```
