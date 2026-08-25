# Top-of-block arbitrage

## Identity
- **MEV_ID**: MEV-03-004
- **Grupo**: 3
- **Familia**: Arbitrajes disparados por transacciones o cambios de estado
- **Surface**: DEX_STATE
- **Backend**: state_event_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_05, op_21, op_25, op_27
- **Secondary**: op_08, op_10, op_11, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-03-004)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
