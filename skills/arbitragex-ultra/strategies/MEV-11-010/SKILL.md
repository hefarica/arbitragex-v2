# Oracle-resolution timing arbitrage

## Identity
- **MEV_ID**: MEV-11-010
- **Grupo**: 11
- **Familia**: Prediction markets y mercados condicionales
- **Surface**: PREDICTION
- **Backend**: prediction_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 12

## Operators
- **Primary**: op_10, op_11
- **Secondary**: op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-11-010)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
