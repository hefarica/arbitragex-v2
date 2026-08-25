# CLOB–CLOB arbitrage

## Identity
- **MEV_ID**: MEV-01-010
- **Grupo**: 1
- **Familia**: Arbitrajes spot DEX dentro de una misma cadena
- **Surface**: DEX_AMM
- **Backend**: route_graph_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_08, op_10, op_11, op_23
- **Secondary**: op_13, op_16, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-01-010)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
