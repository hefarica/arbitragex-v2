//! Bellman-Ford negative-cycle detection on a token-pool graph.
//!
//! Reference: `.agents/skills/sop_atomic_route_construction/SKILL.md` §14.
//!
//! Nodes are token addresses (20-byte). Edges carry an exchange rate `r`
//! (post-fee). A cycle whose product of rates is > 1 corresponds to a
//! negative cycle in the `-ln(r)` weighted graph — that's an arbitrage
//! opportunity. Detection is O(V·E) per starting node.
//!
//! `alloy-primitives::Address` is the address type; for the SOP skill the
//! original source uses the same type so we copy literally.

use alloy_primitives::Address;
use std::collections::HashMap;

/// Arbitrage graph: nodes are tokens, edges are pools with exchange rates.
pub struct ArbitrageGraph {
    pub nodes: HashMap<Address, Vec<(Address, f64, Address)>>,
    // token -> [(other_token, exchange_rate, pool_addr)]
}

impl ArbitrageGraph {
    pub fn new() -> Self {
        ArbitrageGraph {
            nodes: HashMap::new(),
        }
    }

    pub fn add_edge(&mut self, from: Address, to: Address, rate: f64, pool: Address) {
        self.nodes.entry(from).or_default().push((to, rate, pool));
        // Reverse edge (1/rate)
        self.nodes
            .entry(to)
            .or_default()
            .push((from, 1.0 / rate, pool));
    }

    /// Find a negative-weight cycle reachable from `start` within `max_depth`
    /// hops. A negative cycle = arbitrage opportunity.
    pub fn find_arbitrage_cycle(&self, start: Address, max_depth: usize) -> Option<Vec<Address>> {
        let mut dist: HashMap<Address, f64> = HashMap::new();
        let mut pred: HashMap<Address, Address> = HashMap::new();

        for node in self.nodes.keys() {
            dist.insert(*node, f64::INFINITY);
        }
        dist.insert(start, 0.0);

        // Relax edges up to max_depth times.
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

        // Detect a negative cycle (one more relaxation that improves a node).
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

impl Default for ArbitrageGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(byte: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[19] = byte;
        Address::from(bytes)
    }

    #[test]
    fn detects_known_negative_cycle() {
        // A→B→C→A with each leg yielding 1.01 → 1.030301 round-trip → arb.
        let mut g = ArbitrageGraph::new();
        let a = addr(1);
        let b = addr(2);
        let c = addr(3);
        let pool = addr(0xff);
        g.add_edge(a, b, 1.01, pool);
        g.add_edge(b, c, 1.01, pool);
        g.add_edge(c, a, 1.01, pool);

        let cycle = g.find_arbitrage_cycle(a, 4);
        assert!(cycle.is_some(), "expected negative cycle to be detected");
        let path = cycle.unwrap();
        assert!(
            path.len() >= 2,
            "cycle path should have at least 2 nodes, got {}",
            path.len()
        );
    }

    #[test]
    fn no_cycle_when_rates_break_even() {
        // A→B at 1.0, B→A inverse 1.0 ⇒ no profit cycle.
        let mut g = ArbitrageGraph::new();
        let a = addr(1);
        let b = addr(2);
        let pool = addr(0xff);
        g.add_edge(a, b, 1.0, pool);

        let cycle = g.find_arbitrage_cycle(a, 4);
        assert!(cycle.is_none(), "no arb expected for break-even rates");
    }
}
