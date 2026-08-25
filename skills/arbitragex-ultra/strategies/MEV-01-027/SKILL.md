# Basket arbitrage

## Identity
- **MEV_ID**: MEV-01-027
- **Grupo**: 1
- **Familia**: Arbitrajes spot DEX dentro de una misma cadena
- **Surface**: DEX_AMM
- **Backend**: route_graph_engine

## Route Topology
- **Min Legs**: 3
- **Max Legs**: 16

## Operators
- **Primary**: op_13, op_19, op_21
- **Secondary**: op_08, op_11, op_16, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-01-027)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
