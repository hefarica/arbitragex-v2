# Partial-liquidation arbitrage

## Identity
- **MEV_ID**: MEV-08-013
- **Grupo**: 8
- **Familia**: Lending, crédito, liquidaciones y deuda
- **Surface**: LENDING
- **Backend**: credit_liquidation_engine

## Route Topology
- **Min Legs**: 1
- **Max Legs**: 4

## Operators
- **Primary**: op_08, op_16, op_21, op_26
- **Secondary**: op_11, op_13, op_22, op_23

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-08-013)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
