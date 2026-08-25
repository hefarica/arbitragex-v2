# Weighted-pool arbitrage

## Identity
- **MEV_ID**: MEV-02-004
- **Grupo**: 2
- **Familia**: Arbitrajes según la curva del AMM
- **Surface**: DEX_AMM
- **Backend**: amm_curve_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_15, op_19, op_21
- **Secondary**: op_16, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-02-004)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
