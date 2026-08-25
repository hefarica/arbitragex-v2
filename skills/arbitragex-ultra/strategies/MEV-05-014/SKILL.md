# Fiat–stablecoin–DEX arbitrage

## Identity
- **MEV_ID**: MEV-05-014
- **Grupo**: 5
- **Familia**: Arbitraje CEX–DEX y mercados externos
- **Surface**: CEX_DEX
- **Backend**: cex_external_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_08, op_10, op_21, op_23
- **Secondary**: op_11, op_13, op_16, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-05-014)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
