# Binary complement arbitrage

## Identity
- **MEV_ID**: MEV-11-001
- **Grupo**: 11
- **Familia**: Prediction markets y mercados condicionales
- **Surface**: PREDICTION
- **Backend**: prediction_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 12

## Operators
- **Primary**: op_11, op_19, op_21
- **Secondary**: op_08, op_14, op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-11-001)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
