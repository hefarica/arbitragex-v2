# Parallel-route arbitrage

## Identity
- **MEV_ID**: MEV-01-026
- **Grupo**: 1
- **Familia**: Arbitrajes spot DEX dentro de una misma cadena
- **Surface**: DEX_AMM
- **Backend**: route_graph_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_15, op_19, op_20
- **Secondary**: op_21, op_22, op_27

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-01-026)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
