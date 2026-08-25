# Points/pre-token arbitrage

## Identity
- **MEV_ID**: MEV-04-031
- **Grupo**: 4
- **Familia**: Arbitrajes por equivalencia, paridad o redención del activo
- **Surface**: PARITY_REDEMPTION
- **Backend**: parity_redemption_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 8

## Operators
- **Primary**: op_10, op_11
- **Secondary**: op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-04-031)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
