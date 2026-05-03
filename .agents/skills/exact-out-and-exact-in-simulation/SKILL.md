# Exact-Out and Exact-In Simulation

## Propósito
Diferenciar y simular transacciones donde el Searcher conoce el capital inicial exacto (Exact-In, ej. WETH) versus transacciones donde el Searcher requiere cumplir un repago exacto (Exact-Out, ej. Flashloans).

## Integración
En el motor EVM, Exact-Out requiere simulaciones inversas para pre-calcular si las reservas cubren el output demandado antes de intentar ejecutar el payload de simulación.
