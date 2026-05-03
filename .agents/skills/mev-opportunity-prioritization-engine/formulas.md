# Fórmulas del Motor de Priorización

## NetExpectedProfit
`NetExpectedProfit = GrossOutput - InputAmount - GasCost - Bribe - FlashLoanFee - DEXFees - SlippageLoss - FailureCost`
- **GrossOutput**: Retorno bruto de la operación swap final.
- **Bribe**: Tip al validator (ej. `profit * 0.9`).
- **FailureCost**: Costo de gas si la tx revierte.

## Opportunity Score (General)
`Score = (NetExpectedProfit * LandingProbability * StateFreshness * LiquidityConfidence * ExecutionAtomicity) / (ComputationalCost * ReversalRisk * SlippageRisk * GasVolatilityRisk * TokenRisk)`
- **Supuestos**: Las variables están normalizadas al rango [0.1, 1.0] o [1.0, 10.0] según si son penalizaciones o multiplicadores.
