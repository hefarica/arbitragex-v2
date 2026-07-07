---
name: sop_atomic_route_construction
description: Cuando se diseñe el motor de detección de oportunidades multi-hop, modelo de grafo de tokens, o algoritmo Bellman-Ford para arbitraje. Activa con triggers "Bellman-Ford arbitraje", "ciclos peso negativo", "grafo tokens arbitraje", "ArbitrageGraph", "find_arbitrage_cycle", "reconstruct_cycle", "ruta multi-hop". CONTIENE LA IMPLEMENTACIÓN COMPLETA del SOP §14 — referencia primaria para Sprint 4.
type: arbx_architecture
source_section: SOP_ArbitrageX_2026.pdf §14
---

# Construcción Atómica de Rutas de Arbitraje (Bellman-Ford)

## Modelo (§14.1)
- **Nodos**: tokens (Address ERC-20).
- **Aristas**: pools de liquidez con peso `-ln(rate × (1 - fee))`.
- **Ciclo de peso negativo** = oportunidad de arbitraje.

Bellman-Ford maneja pesos negativos (Dijkstra no), por eso es el algoritmo correcto.
Complejidad: **O(V × E)** donde V=tokens, E=pools activos.

## Implementación COMPLETA (§14.2)

```rust
use std::collections::HashMap;
use alloy::primitives::Address;

/// Arbitrage graph: nodes are tokens, edges are pools with exchange rates.
struct ArbitrageGraph {
    nodes: HashMap<Address, Vec<(Address, f64, Address)>>,
    // token -> [(other_token, exchange_rate, pool_addr)]
}

impl ArbitrageGraph {
    fn new() -> Self {
        ArbitrageGraph { nodes: HashMap::new() }
    }

    fn add_edge(
        &mut self,
        from: Address,
        to: Address,
        rate: f64,    // e.g. 1.002 for 0.2% gain
        pool: Address,
    ) {
        self.nodes.entry(from).or_default().push((to, rate, pool));
        // Reverse edge (1/rate)
        self.nodes.entry(to).or_default().push((from, 1.0 / rate, pool));
    }

    /// Find negative-weight cycles via Bellman-Ford variant.
    /// A negative cycle = arbitrage opportunity.
    fn find_arbitrage_cycle(
        &self,
        start: Address,
        max_depth: usize,
    ) -> Option<Vec<Address>> {
        let mut dist: HashMap<Address, f64> = HashMap::new();
        let mut pred: HashMap<Address, Address> = HashMap::new();

        // Init distances
        for node in self.nodes.keys() {
            dist.insert(*node, f64::INFINITY);
        }
        dist.insert(start, 0.0);

        // Relax edges up to max_depth times
        for _ in 0..max_depth {
            for (u, edges) in &self.nodes {
                for (v, rate, _pool) in edges {
                    let weight = -(rate.ln());
                    if let Some(&d_u) = dist.get(u) {
                        if d_u + weight < *dist.get(v).unwrap_or(&f64::INFINITY) {
                            dist.insert(*v, d_u + weight);
                            pred.insert(*v, *u);
                        }
                    }
                }
            }
        }

        // Detect negative cycle (arbitrage)
        for (u, edges) in &self.nodes {
            for (v, rate, _pool) in edges {
                let weight = -(rate.ln());
                if let (Some(&d_u), Some(&d_v)) = (dist.get(u), dist.get(v)) {
                    if d_u + weight < d_v {
                        return self.reconstruct_cycle(*v, &pred, start);
                    }
                }
            }
        }
        None
    }

    fn reconstruct_cycle(
        &self,
        node: Address,
        pred: &HashMap<Address, Address>,
        start: Address,
    ) -> Option<Vec<Address>> {
        let mut path = vec![node];
        let mut current = node;
        let max_steps = pred.len();

        for _ in 0..max_steps {
            if let Some(&prev) = pred.get(&current) {
                path.push(prev);
                current = prev;
                if current == start {
                    path.reverse();
                    return Some(path);
                }
            } else {
                break;
            }
        }
        None
    }
}
```

## Garantía de ejecución atómica (§14.3)

Una vez identificada la ruta, se traduce en bundle atómico vía Flashbots:
- Si cualquier paso falla (precio cambió entre detección y ejecución) → todo el bundle se descarta.
- Profit potencial se pierde, pero **NUNCA se incurre en pérdidas**.

## Profundidad recomendada
- **max_hops 3-4**: rutas más rentables típicamente.
- **max_hops 5**: solo si volatilidad alta (spreads amplios compensan fees acumuladas).
- **Más de 5 hops**: gas + fees > profit, raramente rentable.

**Configuración dinámica**: ajustar `max_hops` según volatilidad reciente.
- Volatilidad baja (varianza precios < 1% en 1h) → max_hops=3.
- Volatilidad media → max_hops=4.
- Volatilidad alta (>5% en 1h) → max_hops=5.

## Optimización con Rayon (paralelo)
```rust
use rayon::prelude::*;

fn find_all_arbs(graph: &ArbitrageGraph, base_tokens: &[Address]) -> Vec<Vec<Address>> {
    base_tokens.par_iter()
        .filter_map(|base| graph.find_arbitrage_cycle(*base, 4))
        .collect()
}
```

Cada base token se procesa en thread independiente → paralelismo a nivel de token base.

## Cache de rutas calientes
Mantener cache LRU con las 50 rutas más frecuentes. Cuando un precio cambia, primero re-evaluar rutas en cache (microsegundos) antes de ejecutar Bellman-Ford completo (milisegundos).

## Invariantes
- Pesos siempre `-ln(rate × (1 - fee))` (no `-ln(rate)` — debe incluir fee).
- Reverse edge siempre añadida con `1.0 / rate`.
- max_hops configurable, default 4.
- Detección + simulación + ejecución en ≤ 200ms (target SOP §1).
- Cache LRU con TTL = 1 block (12s en Ethereum, 250ms en Arbitrum).

## Cross-references
- Liquidity aggregation antes de Bellman-Ford: `sop_liquidity_aggregation`.
- Después de detect: bundle construction: `sop_flashbots_bundles`.
- Esta es la implementación de referencia para **Sprint 4** del plan vigente.
