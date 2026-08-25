# CEX–DEX triangular arbitrage

## Identity
- **MEV_ID**: MEV-05-003
- **Grupo**: 5
- **Familia**: Arbitraje CEX–DEX y mercados externos
- **Surface**: CEX_DEX
- **Backend**: cex_external_engine

## Route Topology
- **Min Legs**: 3
- **Max Legs**: 3

## Operators
- **Primary**: op_08, op_10, op_21, op_23
- **Secondary**: op_11, op_13, op_16, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-05-003)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
