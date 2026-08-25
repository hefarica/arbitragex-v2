# Auction-clearing arbitrage

## Identity
- **MEV_ID**: MEV-03-026
- **Grupo**: 3
- **Familia**: Arbitrajes disparados por transacciones o cambios de estado
- **Surface**: DEX_STATE
- **Backend**: state_event_engine

## Route Topology
- **Min Legs**: 3
- **Max Legs**: 16

## Operators
- **Primary**: op_19, op_21, op_23, op_24
- **Secondary**: op_08, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-03-026)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
