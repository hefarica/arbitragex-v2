# Leverage-loop arbitrage

## Identity
- **MEV_ID**: MEV-08-011
- **Grupo**: 8
- **Familia**: Lending, crédito, liquidaciones y deuda
- **Surface**: LENDING
- **Backend**: credit_liquidation_engine

## Route Topology
- **Min Legs**: 1
- **Max Legs**: 4

## Operators
- **Primary**: op_17, op_20, op_21
- **Secondary**: op_13, op_16, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-08-011)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
