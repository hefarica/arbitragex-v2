# Index token–basket arbitrage

## Identity
- **MEV_ID**: MEV-04-014
- **Grupo**: 4
- **Familia**: Arbitrajes por equivalencia, paridad o redención del activo
- **Surface**: PARITY_REDEMPTION
- **Backend**: parity_redemption_engine

## Route Topology
- **Min Legs**: 3
- **Max Legs**: 16

## Operators
- **Primary**: op_13, op_19, op_21
- **Secondary**: op_08, op_11, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-04-014)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
