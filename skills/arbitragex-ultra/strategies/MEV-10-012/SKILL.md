# NFT-backed-loan liquidation arbitrage

## Identity
- **MEV_ID**: MEV-10-012
- **Grupo**: 10
- **Familia**: NFT, juegos y activos no fungibles
- **Surface**: NFT
- **Backend**: nft_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_08, op_11, op_21
- **Secondary**: op_16, op_22, op_23

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-10-012)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
