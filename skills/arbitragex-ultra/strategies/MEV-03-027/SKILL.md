# Block-time arbitrage

## Identity
- **MEV_ID**: MEV-03-027
- **Grupo**: 3
- **Familia**: Arbitrajes disparados por transacciones o cambios de estado
- **Surface**: DEX_STATE
- **Backend**: state_event_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_08, op_10, op_11, op_23
- **Secondary**: op_13, op_16, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-03-027)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
