# Leverage-token rebalancing arbitrage

## Identity
- **MEV_ID**: MEV-07-030
- **Grupo**: 7
- **Familia**: Derivados y volatilidad
- **Surface**: DERIVATIVES
- **Backend**: derivatives_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 6

## Operators
- **Primary**: op_08, op_21, op_23
- **Secondary**: op_11, op_13, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-07-030)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
