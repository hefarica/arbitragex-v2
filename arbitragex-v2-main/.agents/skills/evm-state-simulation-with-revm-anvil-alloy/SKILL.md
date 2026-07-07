# EVM State Simulation with REVM/Anvil

## Propósito
Validar determinísticamente la rentabilidad de una oportunidad ejecutando los opcodes de la EVM en memoria sin transmitir la transacción.

## Conocimiento esencial
Usar `revm` embebido en Rust es órdenes de magnitud más rápido que llamar a `eth_call` en un nodo local Erigon/Geth. Permite simular miles de bundles por bloque.
