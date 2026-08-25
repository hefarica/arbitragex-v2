# Coincidence-of-Wants arbitrage

## Identity
- **MEV_ID**: MEV-01-030
- **Grupo**: 1
- **Familia**: Arbitrajes spot DEX dentro de una misma cadena
- **Surface**: DEX_AMM
- **Backend**: route_graph_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_19, op_24, op_27, op_29
- **Secondary**: op_20, op_22, op_23

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-01-030)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
