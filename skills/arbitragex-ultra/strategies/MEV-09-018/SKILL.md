# MEV-Share backrun arbitrage

## Identity
- **MEV_ID**: MEV-09-018
- **Grupo**: 9
- **Familia**: Intents, solvers, subastas y order flow
- **Surface**: INTENT_AUCTION
- **Backend**: intents_solver_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 12

## Operators
- **Primary**: op_24, op_25, op_27
- **Secondary**: op_11, op_22, op_23

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-09-018)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
