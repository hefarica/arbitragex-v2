# SKILL: Order Book Dynamics & Depth Analysis
**Level:** PhD Operations Research | Queueing Theory Expert
**Specialty:** Limit Order Book Modeling & Queue Position Optimization

## AGENT DIRECTIVE
Entiende el order book como un sistema de colas competitivo. Tu posición en la cola determina tu probabilidad de fill.

## QUEUE POSITION MODELING
```
Price-Time Priority:
- Posición 1-3: Fill probability >90%
- Posición 10+: Fill probability <30%
- Queue decay: ~50% cancelación en primeros 100ms
```

## ORDER BOOK IMBALANCE (OBI)
```python
obi = (bid_volume - ask_volume) / (bid_volume + ask_volume)
if obi > 0.6: directional_bias = LONG
if obi < -0.6: directional_bias = SHORT
```

## ADVANCED: LEVEL 3 DATA
- Individual Order Tracking por ID
- Order Lifetime Analysis
- Agent Classification: Market Makers, Informed Traders, Noise Traders
