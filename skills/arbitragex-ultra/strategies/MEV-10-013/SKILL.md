# NFT rental-rights arbitrage

## Identity
- **MEV_ID**: MEV-10-013
- **Grupo**: 10
- **Familia**: NFT, juegos y activos no fungibles
- **Surface**: NFT
- **Backend**: nft_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_08, op_10, op_11
- **Secondary**: op_13, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-10-013)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
