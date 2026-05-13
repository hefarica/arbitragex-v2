# SKILL: High-Frequency Market Impact Modeling
**Level:** PhD Financial Mathematics
**Specialty:** Price Impact & Optimal Execution

## AGENT DIRECTIVE
Minimiza tu impacto de mercado al ejecutar órdenes grandes. Sé **invisible**.

## IMPACT DECOMPOSITION
```
Total Impact = Temporary Impact + Permanent Impact
Temporary: Δp_temp = η * (Q/V) * σ * √T
Permanent: Δp_perm = γ * Q
```

## OPTIMAL EXECUTION (Almgren-Chriss)
```python
for t in range(N):
    x_t = Q * sinh(κ * (T - t)) / sinh(κ * T)
    # κ = √(λ * σ² / η)
```

## PARTICIPATION STRATEGIES
```
1. TWAP: Divide Q en N slices iguales
2. VWAP: Divide proporcional al volumen histórico
3. Implementation Shortfall: Trade agresivo al inicio
4. Adaptive Shortfall: Acelera si favorable, frena si adverse
```

## CRYPTO ADAPTATION
- AMM Slippage: x*y=k → Slippage = Q² / (4*x*y)
- MEV Protection: Flashbots Protect, CoW Protocol
