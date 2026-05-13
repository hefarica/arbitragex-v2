# Gas-Aware Profitability Model

## Propósito
Estimar el costo exacto de ejecución en L1 Ethereum (Base Fee + Priority Fee) para descontarlo del gross profit antes de la simulación pesada.

## Conocimiento esencial
EIP-1559 hace que el base fee sea predecible para el bloque `N+1`. Sin embargo, el priority fee (o bribe al builder de Flashbots) es competitivo. El modelo calcula el umbral de gas mínimo para romper el punto de equilibrio (break-even).
