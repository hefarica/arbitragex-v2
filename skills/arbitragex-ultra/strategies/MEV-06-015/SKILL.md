# Fast-bridge arbitrage

## Identity
- **MEV_ID**: MEV-06-015
- **Grupo**: 6
- **Familia**: Arbitrajes cross-chain y cross-domain
- **Surface**: CROSS_CHAIN
- **Backend**: cross_domain_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 16

## Operators
- **Primary**: op_06, op_07, op_22, op_23
- **Secondary**: op_11, op_13, op_16

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-06-015)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
