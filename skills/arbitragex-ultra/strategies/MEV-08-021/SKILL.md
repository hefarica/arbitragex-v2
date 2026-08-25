# Recapitalization auction arbitrage

## Identity
- **MEV_ID**: MEV-08-021
- **Grupo**: 8
- **Familia**: Lending, crédito, liquidaciones y deuda
- **Surface**: LENDING
- **Backend**: credit_liquidation_engine

## Route Topology
- **Min Legs**: 1
- **Max Legs**: 4

## Operators
- **Primary**: op_19, op_21, op_23
- **Secondary**: op_08, op_11, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-08-021)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
