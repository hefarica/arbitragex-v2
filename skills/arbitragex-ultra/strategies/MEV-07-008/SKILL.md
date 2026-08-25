# Funding-rate arbitrage

## Identity
- **MEV_ID**: MEV-07-008
- **Grupo**: 7
- **Familia**: Derivados y volatilidad
- **Surface**: DERIVATIVES
- **Backend**: derivatives_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 6

## Operators
- **Primary**: op_08, op_10, op_13
- **Secondary**: op_11, op_16, op_22, op_23

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-07-008)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
