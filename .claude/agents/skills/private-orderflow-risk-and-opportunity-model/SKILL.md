# Private Orderflow Risk and Opportunity Model

## Propósito
Modelar el perfil de riesgo-recompensa del orderflow privado. Los flujos privados son de alta calidad pero susceptibles a "Toxic Orderflow" (trampas de builders o pools que alteran estado inesperadamente).

## Señales
- Tasa de win-rate histórico del builder/relay emisor.
- Probabilidad de blind-backrun failed execution (costos de gas incurridos por reverts fallidos en la simulación oculta del builder).
