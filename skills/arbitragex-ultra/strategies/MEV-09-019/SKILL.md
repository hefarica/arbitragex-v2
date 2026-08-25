# Builder-integrated arbitrage

## Identity
- **MEV_ID**: MEV-09-019
- **Grupo**: 9
- **Familia**: Intents, solvers, subastas y order flow
- **Surface**: INTENT_AUCTION
- **Backend**: intents_solver_engine

## Route Topology
- **Min Legs**: 2
- **Max Legs**: 12

## Operators
- **Primary**: op_10, op_11
- **Secondary**: op_22

## Cartridge
- Rhai: backend/searcher-rs/cartridges/strategies/ (search for mev-09-019)

## Doctrine
Two-layer separation: Layer 1 DISCOVERY (enumerate topology), Layer 2 EVALUATION (gates G1-G5).
