# NFT fractionalization arbitrage

## Identity
- **MEV_ID**: MEV-10-010
- **Grupo**: 10
- **Familia**: NFT, juegos y activos no fungibles
- **Surface**: NFT
- **Backend**: nft_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_13, op_19, op_21
- **Secondary**: op_08, op_11, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-10-010)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
