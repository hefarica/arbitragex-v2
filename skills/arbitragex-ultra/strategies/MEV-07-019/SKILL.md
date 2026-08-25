# Box-spread arbitrage

## Identity
- **MEV_ID**: MEV-07-019
- **Grupo**: 7
- **Familia**: Derivados y volatilidad
- **Surface**: DERIVATIVES
- **Backend**: derivatives_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 6

## Operators
- **Primary**: op_13, op_19, op_21
- **Secondary**: op_08, op_11, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-07-019)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
