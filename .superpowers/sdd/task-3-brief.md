# Task Brief: Nuevas Estrategias de Motor

## Context

Task 3 del Plan Maestro OMEGA. Depende de Task 2 (HotPathEmitter listo).

## Goal

Implementar 3 nuevos engines: SpanningTreeEngine, CrossChainBridgeEngine, LiquidationSnipeEngine.

## Files

- Create: `backend/searcher-rs/src/engines/spanning_tree_engine.rs`
- Create: `backend/searcher-rs/src/engines/cross_chain_bridge_engine.rs`
- Create: `backend/searcher-rs/src/engines/liquidation_snipe_engine.rs`
- Modify: `backend/searcher-rs/src/orchestrator.rs`

## Interfaces

- Consumes: RouteIntent desde route_decoder
- Produces: Vec<OpportunityCandidate> para cada estrategia

## Steps

### Step 1: SpanningTreeEngine (Bellman-Ford)

```rust
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::bellman_ford;

pub struct SpanningTreeEngine {
    graph: DiGraph<TokenNode, PoolEdge>,
    token_indices: HashMap<String, NodeIndex>,
}

impl SpanningTreeEngine {
    pub fn detect_cycles(&self, start_token: &str) -> Vec<ArbCycle> {
        // Bellman-Ford para ciclos negativos
        // -ln(rate) como peso
    }
}
```

### Step 2: CrossChainBridgeEngine

```rust
pub struct CrossChainBridgeEngine {
    bridges: Vec<BridgeConfig>,
    price_oracles: HashMap<u64, Arc<dyn PriceOracle>>,
}

impl CrossChainBridgeEngine {
    pub async fn detect_cross_chain_arbs(&self) -> Vec<CrossChainOpportunity> {
        // Comparar precios entre cadenas
        // Spread > 50bps después de fees = oportunidad
    }
}
```

### Step 3: LiquidationSnipeEngine

```rust
pub struct LiquidationSnipeEngine {
    lending_pools: HashMap<String, LendingPoolConfig>,
    min_liquidation_bonus_bps: u32,
}

impl LiquidationSnipeEngine {
    pub async fn scan_liquidatable_positions(&self) -> Vec<LiquidationCandidate> {
        // Query Aave/Compound subgraph
        // health_factor < 1.0
    }
}
```

### Step 4: Registrar en Orchestrator

Agregar campos a Orchestrator:
```rust
pub struct Orchestrator {
    // ... existentes
    pub spanning_tree_engine: Option<Arc<SpanningTreeEngine>>,
    pub cross_chain_engine: Option<Arc<CrossChainBridgeEngine>>,
    pub liquidation_engine: Option<Arc<LiquidationSnipeEngine>>,
}
```

### Step 5: Compilar

```bash
cd backend/searcher-rs && cargo check 2>&1 | tail -30
```

### Step 6: Commit

```bash
git add backend/searcher-rs/src/engines/
git commit -m "feat(searcher): add spanning tree, cross-chain, and liquidation engines"
```

## Acceptance Criteria

- [ ] SpanningTreeEngine con Bellman-Ford implementado
- [ ] CrossChainBridgeEngine con detección de spreads
- [ ] LiquidationSnipeEngine para Aave/Compound
- [ ] Todos registrados en Orchestrator
- [ ] Compila sin errores
- [ ] Commiteado

## Out of Scope

- Implementación completa de algoritmos (stubs OK)
- Tests unitarios
- Integración con subgraphs reales
