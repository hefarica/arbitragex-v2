# Reorg/time-bandit arbitrage

## Identity
- **MEV_ID**: MEV-03-030
- **Grupo**: 3
- **Familia**: Arbitrajes disparados por transacciones o cambios de estado
- **Surface**: DEX_STATE
- **Backend**: state_event_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_10, op_11
- **Secondary**: op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-03-030)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
