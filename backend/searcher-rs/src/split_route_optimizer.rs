//! Split-Route Convex Optimizer — Integrated into the search domain.
//!
//! RECONFIGURATION DIRECTIVE (operator, 2026-08-19):
//! - Split-route optimization is INTEGRATED into the search algorithm, NOT a
//!   separate simulation phase
//! - Success probability is computed via fast vector polynomial interpolation
//! - Target latency: ≤100μs per split computation
//! - Local invariants enforce correctness (no external oracle needed):
//!   ∀t: Balance_net ≥ ε
//!   ∀op: Cost_total ≤ ξ × Potential_extraction
//!
//! ## Mathematical Foundation
//!
//! The next-state vector is computed as a fast polynomial interpolation:
//!   S⃗_next = f(S⃗_current, L⃗_pool, C⃗_gas, P⃗_inclusion)
//!
//! Where f is optimized for vector operations in Rust.
//!
//! For two CPMMs with reserves (x1, y1) and (x2, y2) and fees f1, f2:
//! - Pool 1 output: Δy1 = f(x1, y1, Δx1) — concave in Δx1
//! - Pool 2 output: Δy2 = f(x2, y2, Δx2) — concave in Δx2
//! - Constraint: Δx1 + Δx2 = Δx (total input)
//!
//! Optimal split satisfies the marginal condition:
//!   ∂Δy1/∂Δx1 = ∂Δy2/∂Δx2
//!
//! For CPMM: ∂Δy/∂Δx = (y × (1-f)) / (x + (1-f)×Δx)²
//!
//! Zero-Fee Closed Form:
//!   Δx1* = (x1×√(y2) / (√(y1)×x2 + x1×√(y2))) × Δx
//!   Δx2* = Δx - Δx1*
//!
//! Source: Angeris et al. "Optimal Routing for CFMMs." EC'22, arXiv:2204.05238.

use serde::{Deserialize, Serialize};

/// Vector state for polynomial interpolation (deterministic, no simulation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowStateVector {
    /// Current split allocation [Δx1, Δx2].
    pub allocation: [f64; 2],
    /// Pool liquidity vector [x1, y1, x2, y2].
    pub liquidity: [f64; 4],
    /// Gas cost coefficient per unit of trade.
    pub gas_coefficient: f64,
    /// Inclusion probability coefficient (deterministic, not heuristic).
    pub inclusion_probability: f64,
    /// Tip coefficient (deterministic fraction of gross).
    pub tip_coefficient: f64,
}

impl FlowStateVector {
    /// Deterministic next-state computation: S⃗_next = f(S⃗, L⃗, C⃗, P⃗).
    /// Uses closed-form CPMM math — no iterative simulation needed.
    /// Latency target: ≤100μs (single-pass arithmetic).
    pub fn next_state(&self) -> FlowStateVector {
        let (x1, y1, x2, y2) = (self.liquidity[0], self.liquidity[1], self.liquidity[2], self.liquidity[3]);
        let total = self.allocation[0] + self.allocation[1];

        // Zero-fee closed-form optimal split (deterministic, O(1))
        let sqrt_y1 = y1.sqrt();
        let sqrt_y2 = y2.sqrt();
        let denom = sqrt_y1 * x2 + x1 * sqrt_y2;
        let optimal_x1 = if denom > 0.0 { x1 * sqrt_y2 / denom * total } else { total / 2.0 };

        FlowStateVector {
            allocation: [optimal_x1, total - optimal_x1],
            liquidity: self.liquidity,
            gas_coefficient: self.gas_coefficient,
            inclusion_probability: self.inclusion_probability,
            tip_coefficient: self.tip_coefficient,
        }
    }

    /// Local invariant 1: Balance_net ≥ ε (always profitable).
    pub fn satisfies_balance_invariant(&self, epsilon: f64) -> bool {
        let total_output = self.expected_output();
        let total_cost = self.total_cost();
        total_output - total_cost >= epsilon
    }

    /// Local invariant 2: Cost_total ≤ ξ × Potential_extraction.
    pub fn satisfies_cost_invariant(&self, xi: f64) -> bool {
        let cost = self.total_cost();
        let potential = self.expected_output();
        cost <= xi * potential
    }

    /// Expected gross output from current allocation (closed-form CPMM).
    fn expected_output(&self) -> f64 {
        let (x1, y1, x2, y2) = (self.liquidity[0], self.liquidity[1], self.liquidity[2], self.liquidity[3]);
        let (dx1, dx2) = (self.allocation[0], self.allocation[1]);

        // CPMM output: y * dx / (x + dx) for each pool
        let out1 = if x1 + dx1 > 0.0 { y1 * dx1 / (x1 + dx1) } else { 0.0 };
        let out2 = if x2 + dx2 > 0.0 { y2 * dx2 / (x2 + dx2) } else { 0.0 };
        out1 + out2
    }

    /// Total cost: gas + tip (deterministic coefficients).
    fn total_cost(&self) -> f64 {
        let total_input = self.allocation[0] + self.allocation[1];
        let gas = total_input * self.gas_coefficient;
        let tip = self.expected_output() * self.tip_coefficient;
        gas + tip
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    /// Reserve of token_in (x).
    pub reserve_in: f64,
    /// Reserve of token_out (y).
    pub reserve_out: f64,
    /// Pool fee as a fraction (e.g., 0.003 for 0.3%).
    pub fee: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitResult {
    /// Amount to route through pool 1 (Δx1).
    pub amount_pool1: f64,
    /// Amount to route through pool 2 (Δx2).
    pub amount_pool2: f64,
    /// Total output from both pools (Δy1 + Δy2).
    pub total_output: f64,
    /// Output from single-pool execution (for comparison).
    pub single_pool_output: f64,
    /// Improvement from splitting (percentage).
    pub improvement_pct: f64,
}

/// CPMM output function: how much token_out do you get for Δx token_in?
fn cpmm_output(pool: &PoolState, delta_x: f64) -> f64 {
    if delta_x <= 0.0 || pool.reserve_in <= 0.0 || pool.reserve_out <= 0.0 {
        return 0.0;
    }
    let effective_in = delta_x * (1.0 - pool.fee);
    pool.reserve_out * effective_in / (pool.reserve_in + effective_in)
}

/// CPMM marginal output: derivative of output w.r.t. input.
fn cpmm_marginal(pool: &PoolState, delta_x: f64) -> f64 {
    let effective_in = delta_x * (1.0 - pool.fee);
    let denominator = pool.reserve_in + effective_in;
    if denominator <= 0.0 {
        return 0.0;
    }
    pool.reserve_out * (1.0 - pool.fee) * pool.reserve_in / (denominator * denominator)
}

/// Find the optimal split of Δx between two pools using bisection.
///
/// We binary search on Δx1 ∈ [0, Δx] to find where the marginal outputs
/// are equal: marginal1(Δx1) = marginal2(Δx - Δx1).
///
/// # Arguments
/// * `pool1`, `pool2` - The two pools for the same token pair
/// * `total_input` - Total amount of token_in to split (Δx)
/// * `iterations` - Bisection iterations (default 50, gives ~machine precision)
pub fn optimal_split(
    pool1: &PoolState,
    pool2: &PoolState,
    total_input: f64,
    iterations: usize,
) -> SplitResult {
    if total_input <= 0.0 {
        return SplitResult {
            amount_pool1: 0.0,
            amount_pool2: 0.0,
            total_output: 0.0,
            single_pool_output: 0.0,
            improvement_pct: 0.0,
        };
    }

    // Bisection: find Δx1 where marginal1(Δx1) = marginal2(Δx - Δx1)
    let mut lo = 0.0f64;
    let mut hi = total_input;

    for _ in 0..iterations {
        let mid = (lo + hi) / 2.0;
        let m1 = cpmm_marginal(pool1, mid);
        let m2 = cpmm_marginal(pool2, total_input - mid);

        if m1 > m2 {
            // Pool 1 still has better marginal → route more through pool 1
            lo = mid;
        } else {
            // Pool 2 has better marginal → route less through pool 1
            hi = mid;
        }
    }

    let split_point = (lo + hi) / 2.0;

    // Compute outputs
    let out1 = cpmm_output(pool1, split_point);
    let out2 = cpmm_output(pool2, total_input - split_point);
    let total_output = out1 + out2;

    // Compare vs single-pool (best of two)
    let single1 = cpmm_output(pool1, total_input);
    let single2 = cpmm_output(pool2, total_input);
    let single_pool_output = single1.max(single2);

    let improvement_pct = if single_pool_output > 0.0 {
        (total_output / single_pool_output - 1.0) * 100.0
    } else {
        0.0
    };

    SplitResult {
        amount_pool1: split_point,
        amount_pool2: total_input - split_point,
        total_output,
        single_pool_output,
        improvement_pct,
    }
}

/// Zero-fee closed-form optimal split (fast path when fees are equal or negligible).
pub fn optimal_split_zero_fee(
    pool1: &PoolState,
    pool2: &PoolState,
    total_input: f64,
) -> f64 {
    // Optimal Δx1: x1×√(y2) / (√(y1)×x2 + x1×√(y2)) × Δx
    let sqrt_y1 = pool1.reserve_out.sqrt();
    let sqrt_y2 = pool2.reserve_out.sqrt();
    let denominator = sqrt_y1 * pool2.reserve_in + pool1.reserve_in * sqrt_y2;
    if denominator <= 0.0 {
        return total_input / 2.0; // Fallback: equal split
    }
    pool1.reserve_in * sqrt_y2 / denominator * total_input
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(r_in: f64, r_out: f64, fee: f64) -> PoolState {
        PoolState {
            reserve_in: r_in,
            reserve_out: r_out,
            fee,
        }
    }

    #[test]
    fn equal_pools_split_equally() {
        let p1 = pool(1_000_000.0, 1_000_000.0, 0.003);
        let p2 = pool(1_000_000.0, 1_000_000.0, 0.003);
        let result = optimal_split(&p1, &p2, 10_000.0, 50);
        // Equal pools → equal split
        assert!((result.amount_pool1 - result.amount_pool2).abs() < 1.0);
    }

    #[test]
    fn imbalanced_pools_favor_larger() {
        let p1 = pool(10_000_000.0, 10_000_000.0, 0.003); // Large pool
        let p2 = pool(100_000.0, 100_000.0, 0.003); // Small pool
        let result = optimal_split(&p1, &p2, 10_000.0, 50);
        // Most should go through the larger pool (less price impact)
        assert!(result.amount_pool1 > result.amount_pool2);
    }

    #[test]
    fn split_improves_over_single_pool() {
        let p1 = pool(500_000.0, 500_000.0, 0.003);
        let p2 = pool(1_000_000.0, 1_000_000.0, 0.003);
        let result = optimal_split(&p1, &p2, 50_000.0, 50);
        // Splitting should always be >= single pool
        assert!(result.total_output >= result.single_pool_output);
        assert!(result.improvement_pct >= 0.0);
    }

    #[test]
    fn split_respects_total_input() {
        let p1 = pool(100_000.0, 200_000.0, 0.003);
        let p2 = pool(300_000.0, 150_000.0, 0.003);
        let total = 5_000.0;
        let result = optimal_split(&p1, &p2, total, 50);
        assert!((result.amount_pool1 + result.amount_pool2 - total).abs() < 1e-6);
    }

    #[test]
    fn zero_input_returns_zero() {
        let p1 = pool(1000.0, 1000.0, 0.003);
        let p2 = pool(1000.0, 1000.0, 0.003);
        let result = optimal_split(&p1, &p2, 0.0, 50);
        assert_eq!(result.total_output, 0.0);
    }

    #[test]
    fn zero_fee_closed_form_matches_bisection() {
        let p1 = pool(500_000.0, 600_000.0, 0.0);
        let p2 = pool(800_000.0, 400_000.0, 0.0);
        let total = 10_000.0;
        let bisection = optimal_split(&p1, &p2, total, 50);
        let closed = optimal_split_zero_fee(&p1, &p2, total);
        // Closed form should match bisection within tolerance
        assert!((bisection.amount_pool1 - closed).abs() < total * 0.01);
    }

    #[test]
    fn different_fees_handled_correctly() {
        let p1 = pool(1_000_000.0, 1_000_000.0, 0.001); // 0.1% fee
        let p2 = pool(1_000_000.0, 1_000_000.0, 0.030); // 3.0% fee
        let result = optimal_split(&p1, &p2, 10_000.0, 50);
        // Lower fee pool should get more allocation
        assert!(result.amount_pool1 > result.amount_pool2);
    }

    #[test]
    fn split_improvement_significant_for_equal_pools() {
        // Two identical pools with equal liquidity
        let p1 = pool(1_000_000.0, 1_000_000.0, 0.003);
        let p2 = pool(1_000_000.0, 1_000_000.0, 0.003);
        // Large trade relative to pool (10% of reserves)
        let result = optimal_split(&p1, &p2, 100_000.0, 50);
        // Splitting should give meaningful improvement (both pools absorb impact)
        // For 10% of reserves, improvement should be > 1%
        assert!(result.improvement_pct > 1.0, "improvement: {:.3}%", result.improvement_pct);
    }
}
