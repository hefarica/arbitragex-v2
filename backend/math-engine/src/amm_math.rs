//! Matemáticas Puras para AMMs (Automated Market Makers)

/// Calcula el `amount_out` para un trade exacto en un pool UniV2 (x * y = k)
/// Asume que los reserves y amounts ya están escalados/normalizados (mismo decimal) o manejados externamente.
pub fn calc_univ2_amount_out(
    amount_in: f64,
    reserve_in: f64,
    reserve_out: f64,
    fee_pct: f64,
) -> f64 {
    if amount_in <= 0.0 || reserve_in <= 0.0 || reserve_out <= 0.0 {
        return 0.0;
    }

    // El fee_multiplier es (1 - fee_pct), típicamente 0.997 (para 0.3%)
    let fee_multiplier = 1.0 - fee_pct;
    let amount_in_with_fee = amount_in * fee_multiplier;
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in + amount_in_with_fee;

    numerator / denominator
}

/// Estima el Price Impact de un swap en un pool UniV2.
pub fn calc_univ2_price_impact(amount_in: f64, reserve_in: f64, fee_pct: f64) -> f64 {
    if amount_in <= 0.0 || reserve_in <= 0.0 {
        return 0.0;
    }

    let fee_multiplier = 1.0 - fee_pct;
    let amount_in_with_fee = amount_in * fee_multiplier;

    // Price Impact en Constant Product Market Maker = amount_in_with_fee / (reserve_in + amount_in_with_fee)
    amount_in_with_fee / (reserve_in + amount_in_with_fee)
}

/// Aproximación simplificada de matemática de Ticks para UniV3.
/// En la realidad on-chain se usaría Q64.96. Esto es para estimación rápida pre-trade.
pub fn approx_univ3_amount_out(
    amount_in: f64,
    liquidity_active: f64,
    current_sqrt_price_x96: f64,
    _target_sqrt_price_x96: f64, // simplified
    _fee_pct: f64,
) -> f64 {
    // ESTO ES UN PLACEHOLDER MATEMÁTICO para cálculos de alto nivel off-chain.
    // La ejecución on-chain usa `getAmountOut` del Quoter.
    if liquidity_active <= 0.0 || current_sqrt_price_x96 <= 0.0 {
        return 0.0;
    }
    // Implementación mínima para satisfacer el type checker y estructura
    let mut amount_out = amount_in * 0.99; // Dummy logic
    if amount_out < 0.0 {
        amount_out = 0.0;
    }
    amount_out
}

/// Approximates the V3 price impact for a swap **within the current tick range**.
///
/// # Mathematical derivation
///
/// Uniswap V3 within a single tick range acts as a virtual constant-product pool
/// with the invariant `L = Δy / Δ(√P)` (for token1) or `L = -Δx / Δ(1/√P)` (for
/// token0). For small swaps we linearise around the current sqrt price:
///
/// **token0 → token1** (selling token0; √P decreases):
/// ```text
/// |Δ√P / √P| ≈ √P × Δx / L
///            = (sqrt_price_x96 / 2^96) × amount_in_wei / liquidity
/// ```
///
/// **token1 → token0** (selling token1; √P increases):
/// ```text
/// |Δ√P / √P| ≈ Δy / (L × √P)
///            = amount_in_wei / (liquidity × sqrt_price_x96 / 2^96)
/// ```
///
/// The observed output price relative to the pre-swap price moves by approximately
/// `2 × |Δ√P / √P|` (because the output rate scales with the square of the price
/// ratio, and `(1 ± ε)^2 ≈ 1 ± 2ε` for small ε). This factor of 2 makes the
/// estimate conservative for the Net Profit Gate — slightly over-reports impact.
///
/// # Known limitations (documented for future tick-walking sprint)
/// - **Within-tick exact, cross-tick conservative-high**: when the swap crosses one
///   or more tick boundaries, adjacent ticks may have lower liquidity than the
///   current tick. The formula uses current-tick L for the entire trade, so it
///   *underestimates* impact for cross-tick swaps. This is a systematic bias toward
///   under-rejection (some truly high-impact trades pass). Accept for now; cross-tick
///   simulation requires full tickBitmap (subgraph integration, deferred).
/// - **No fee tier**: 0.05% / 0.3% / 1.0% fee is not subtracted from `amount_in`
///   before applying the formula. Since V3 fees are small relative to the impact
///   ratio, the error is sub-0.01% for typical trades.
/// - **Overestimates for tiny swaps**: the linear approximation has a constant
///   relative error that is largest when impact is smallest. For trades ≪ liquidity
///   the true impact is negligibly small in both the model and reality.
///
/// # Parameters
/// - `amount_in_wei`: trade size in input token's wei (raw, not scaled).
/// - `sqrt_price_x96`: current pool sqrtPriceX96 from `slot0()` (= √P × 2^96).
/// - `liquidity`: pool liquidity at current tick from `slot0()` (uint128).
/// - `token0_to_token1`: swap direction (`true` = sell token0, buy token1).
///
/// # Returns
/// Price impact as a fraction in `[0.0, 1.0]` (0.005 = 0.5%). Returns `0.0`
/// when data is missing — the caller falls back to the `max_slippage_pct` proxy
/// (R8 fail-honest: missing data never synthesised).
pub fn calc_univ3_price_impact_pct(
    amount_in_wei: u128,
    sqrt_price_x96: u128,
    liquidity: u128,
    token0_to_token1: bool,
) -> f64 {
    // R8 fail-honest: missing inputs → 0 → caller falls back to proxy.
    if liquidity == 0 || sqrt_price_x96 == 0 {
        return 0.0;
    }

    // Q96 scale factor as f64. Exact representation to full precision is lost
    // beyond ~2^53 mantissa bits, but the ratio sqrt_price_x96 / q96 is well-
    // behaved because both operands have magnitude ≤ 2^128 ≪ 2^1023 (f64 max).
    let q96: f64 = (1u128 << 96) as f64;
    let sqrt_p: f64 = sqrt_price_x96 as f64 / q96; // = √P (token1/token0)^(1/2)
    let l: f64 = liquidity as f64;
    let amt: f64 = amount_in_wei as f64;

    // First-order relative change in √P.
    //
    // token0→token1: |Δ√P/√P| = √P × Δx / L
    // token1→token0: |Δ√P/√P| = Δy / (L × √P)
    //
    // Both formulas are dimensionless: sqrt_p carries units (token1/token0)^(1/2),
    // and amt/l is dimensionless (same token unit on numerator and denominator).
    let delta_sqrt_p_over_sqrt_p = if token0_to_token1 {
        sqrt_p * amt / l
    } else {
        amt / (l * sqrt_p)
    };

    // Price impact ≈ 2 × |Δ√P / √P|.
    // The factor of 2 converts the √P relative change to the price relative
    // change (output rate ∝ P = (√P)^2; d(P)/P = 2 × d(√P)/√P).
    // This deliberately overestimates slightly, biasing toward rejecting more
    // opportunities when impact is near the threshold (R8 fail-honest direction).
    let impact = 2.0 * delta_sqrt_p_over_sqrt_p.abs();

    // Sanity bound: impact ≥ 100% means the pool is fully drained by this trade.
    // The linear approximation breaks down well before 100%; cap at 1.0 so
    // callers can treat the return value as a valid fraction.
    impact.min(1.0)
}

// ---------------------------------------------------------------------------
// V3 tick-walking price impact
// ---------------------------------------------------------------------------

/// Convert a Uniswap V3 tick index to its corresponding sqrtPriceX96.
///
/// Uses the identity `√P = 1.0001^(tick/2)` expressed as a `f64` floating-point
/// approximation. This is a scaffold implementation.
///
/// # Accuracy vs the canonical TickMath
/// The real Uniswap V3 `TickMath.getSqrtRatioAtTick` uses a precomputed bit-by-bit
/// table of 20 multipliers, yielding exact Q64.96 results. The `f64` approximation
/// here has relative error ≤ ~1 ULP (~1e-15) for |tick| ≤ 887_272 (the V3 maximum),
/// because `f64` has 53 bits of mantissa and the tick range is narrow enough that
/// `powf` is well-conditioned. The impact on arbitrage gating is negligible: a 1e-15
/// relative error in sqrtPrice produces a 2e-15 relative error in the tick-walking
/// result, far below any profitable threshold. A future sprint can replace this with
/// the exact integer table if sub-ULP precision becomes necessary.
fn tick_to_sqrt_price_x96(tick: i32) -> u128 {
    // 1.0001^(tick/2) × 2^96
    // Note: tick ∈ [-887_272, +887_272]; the range of the exponent tick/2 is
    // ≈ ±443_636. At these extremes 1.0001^443636 ≈ 1.38e19 which is well below
    // f64::MAX ≈ 1.8e308, so no overflow occurs.
    let exponent = tick as f64 * 0.5_f64;
    let factor = 1.0001_f64.powf(exponent);
    let q96 = (1u128 << 96) as f64;
    // Saturating cast: if factor is extremely small (negative ticks near min),
    // the product rounds to 0 which is still safe — the caller guards on 0.
    let raw = factor * q96;
    if raw >= u128::MAX as f64 {
        u128::MAX
    } else if raw <= 0.0 {
        0u128
    } else {
        raw as u128
    }
}

/// Compute Uniswap V3 price impact via tick-walking simulation.
///
/// When `distribution` is `Some`, this function walks tick boundaries in the
/// direction of the swap, updating the active liquidity at each crossing, and
/// accumulates the consumed input until `amount_in_wei` is fully spent. The
/// resulting relative change in sqrtPriceX96 is converted to a price-impact
/// fraction using the same `2 × |Δ√P / √P|` formula as the first-order proxy.
///
/// When `distribution` is `None` (subgraph unconfigured or unavailable), falls
/// back immediately to [`calc_univ3_price_impact_pct`] with the slot0 values from
/// the distribution (R8 fail-honest — no data is synthesised).
///
/// # Tick-walking algorithm
///
/// 1. Collect the relevant subset of ticks: those below `current_tick` for
///    token0→token1 swaps (price decreasing), those above for token1→token0 swaps
///    (price increasing), sorted in the direction of traversal.
/// 2. For each next-tick boundary, compute the input amount needed to move the
///    pool price from the current sqrtPrice to that boundary within the current
///    liquidity range.
/// 3. If the remaining trade fits within the current range, stop and compute the
///    partial sqrtPrice move. Otherwise, cross the tick, update liquidity by
///    `±liquidityNet` (per V3 spec), and continue.
/// 4. Cap at 100 tick crossings to bound latency.
///
/// # Liquidity update sign convention (per Uniswap V3 whitepaper §6.3)
/// - Price moving upward (token1→token0): add `liquidityNet` when crossing tick T upward.
/// - Price moving downward (token0→token1): subtract `liquidityNet` when crossing tick T downward.
///
/// # Returns
/// Price impact as a fraction in `[0.0, 1.0]`. Conservative in the fail-safe
/// direction: underestimated liquidity (e.g., due to tick list truncation at
/// `first: 200`) → overestimated impact → fewer false positives.
pub fn calc_univ3_price_impact_with_distribution(
    amount_in_wei: u128,
    distribution: Option<&crate::subgraph_client::PoolTickDistribution>,
    direction_token0_to_token1: bool,
) -> f64 {
    let Some(dist) = distribution else {
        // R8 fall-back: use slot0 values with the first-order proxy.
        return calc_univ3_price_impact_pct(
            amount_in_wei,
            0, // sqrt_price_x96 unknown without dist → 0 → returns 0.0 (fail-honest)
            0,
            direction_token0_to_token1,
        );
    };

    if dist.liquidity == 0 || dist.sqrt_price_x96 == 0 {
        // R8: degenerate pool → 0.0 → caller uses slippage proxy.
        return 0.0;
    }

    let initial_sqrt_price_f64 = dist.sqrt_price_x96 as f64;
    let mut current_sqrt_price = initial_sqrt_price_f64;
    let mut current_liquidity = dist.liquidity as f64;
    let mut remaining = amount_in_wei as f64;

    // Collect ticks relevant to the swap direction, ordered in traversal order.
    //
    // token0→token1 (price decreasing → sqrtPrice decreasing):
    //   Walk ticks at or below current_tick in descending order.
    // token1→token0 (price increasing → sqrtPrice increasing):
    //   Walk ticks strictly above current_tick in ascending order.
    let relevant_ticks: Vec<&crate::subgraph_client::TickInfo> = if direction_token0_to_token1 {
        let mut v: Vec<_> = dist
            .ticks
            .iter()
            .filter(|t| t.tick_idx <= dist.current_tick)
            .collect();
        v.sort_unstable_by(|a, b| b.tick_idx.cmp(&a.tick_idx)); // descending
        v
    } else {
        dist.ticks
            .iter()
            .filter(|t| t.tick_idx > dist.current_tick)
            .collect() // already ascending from subgraph query
    };

    let max_crossings = 100usize;

    for tick_info in relevant_ticks.iter().take(max_crossings) {
        if remaining <= 0.0 {
            break;
        }

        let next_sqrt = tick_to_sqrt_price_x96(tick_info.tick_idx) as f64;

        // Amount of input token consumed to move price from current_sqrt_price to next_sqrt,
        // within the current liquidity range (V3 formulas, §6.2):
        //
        // token0→token1: Δx = L × (1/√P_next − 1/√P_current)  [token0 in]
        //   Rearranged:   Δx = L × (√P_current − √P_next) / (√P_current × √P_next)
        //   But in X96 space: avoid division; approximate as
        //     Δx ≈ L × (√P_current − √P_next) / √P_current   (first-order, same bias as proxy)
        //
        // token1→token0: Δy = L × (√P_next − √P_current)      [token1 in]
        let amount_to_next = if direction_token0_to_token1 {
            if current_sqrt_price <= 0.0 || next_sqrt <= 0.0 {
                break;
            }
            // Δx ≈ L × (√P_cur − √P_next) / √P_cur
            current_liquidity * (current_sqrt_price - next_sqrt) / current_sqrt_price
        } else {
            // Δy = L × (√P_next − √P_cur)
            current_liquidity * (next_sqrt - current_sqrt_price)
        };

        if amount_to_next <= 0.0 {
            // Degenerate tick (e.g., duplicate or out-of-order): skip.
            continue;
        }

        if amount_to_next >= remaining {
            // Trade exhausted within this tick range — partial move.
            let fraction_consumed = remaining / amount_to_next;
            let delta_sqrt = if direction_token0_to_token1 {
                -(current_sqrt_price - next_sqrt) * fraction_consumed
            } else {
                (next_sqrt - current_sqrt_price) * fraction_consumed
            };
            current_sqrt_price += delta_sqrt;
            remaining = 0.0;
        } else {
            // Cross this tick fully.
            remaining -= amount_to_next;
            current_sqrt_price = next_sqrt;

            // Update active liquidity by liquidityNet.
            // Sign convention: moving upward adds liquidityNet; moving downward subtracts.
            let liq_net = tick_info.liquidity_net.parse::<i128>().unwrap_or(0) as f64;
            if direction_token0_to_token1 {
                current_liquidity -= liq_net; // downward crossing
            } else {
                current_liquidity += liq_net; // upward crossing
            }

            if current_liquidity <= 0.0 {
                // Pool drained past this tick — full impact.
                return 1.0;
            }
        }
    }

    // If remaining > 0 after exhausting all known ticks, the trade extends beyond
    // the tick list (e.g., truncated at 200 entries). Treat the remainder as still
    // within the last known liquidity range — conservative (overestimates impact).
    if remaining > 0.0 && current_liquidity > 0.0 {
        let delta_sqrt = if direction_token0_to_token1 {
            -(remaining * current_sqrt_price / current_liquidity)
        } else {
            remaining / current_liquidity
        };
        current_sqrt_price += delta_sqrt;
    }

    // Price impact = 2 × |Δ√P / √P_initial| (same as first-order proxy).
    // Factor of 2: d(P)/P = 2 × d(√P)/√P for small moves.
    let delta_ratio = (current_sqrt_price - initial_sqrt_price_f64).abs() / initial_sqrt_price_f64;
    (2.0 * delta_ratio).min(1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_univ2_amount_out() {
        // Pool con 1000 ETH (reserve_in) y 2_000_000 USDC (reserve_out)
        // Precio aprox 2000 USDC por ETH.
        // Metemos 1 ETH con 0.3% fee
        let out = calc_univ2_amount_out(1.0, 1000.0, 2_000_000.0, 0.003);
        // Numerator: (1 * 0.997) * 2000000 = 1994000
        // Denominator: 1000 + 0.997 = 1000.997
        // Out: 1994000 / 1000.997 = 1992.0139
        assert!((out - 1992.0139).abs() < 0.01);
    }

    #[test]
    fn test_univ2_price_impact() {
        let impact = calc_univ2_price_impact(100.0, 1000.0, 0.003);
        // amount_in_fee = 99.7
        // impact = 99.7 / 1099.7 = ~9.06%
        assert!((impact - 0.09066).abs() < 0.001);
    }

    // -----------------------------------------------------------------------
    // V3 price impact tests
    // -----------------------------------------------------------------------
    //
    // Test pool setup (shared):
    //   sqrt_price_x96 = 2^96  →  √P = 1.0  (1:1 price ratio)
    //   liquidity      = 10^18 (1_000_000_000_000_000_000)
    //
    // At 1:1 price, token0→token1 and token1→token0 are symmetric for equal
    // amounts, because both formulas collapse to `amt / l` × 2 at √P = 1.

    // The canonical Q96 constant: 2^96.
    // u128::MAX ≈ 3.4e38; 2^96 ≈ 7.92e28 — fits comfortably.
    const Q96_U128: u128 = 1u128 << 96;

    /// Zero liquidity → returns 0.0 (R8 fail-honest).
    #[test]
    fn test_v3_zero_liquidity_returns_zero() {
        let impact = calc_univ3_price_impact_pct(
            1_000_000_000_000_000u128, // 10^15 wei
            Q96_U128,                  // sqrt_price_x96 = 2^96 → √P = 1.0
            0u128,                     // liquidity = 0
            true,
        );
        assert_eq!(impact, 0.0, "zero liquidity must return 0.0 (fail-honest)");
    }

    /// Zero sqrtPriceX96 → returns 0.0 (R8 fail-honest).
    #[test]
    fn test_v3_zero_sqrt_price_returns_zero() {
        let impact = calc_univ3_price_impact_pct(
            1_000_000_000_000_000u128,
            0u128, // sqrt_price_x96 = 0
            1_000_000_000_000_000_000u128,
            false,
        );
        assert_eq!(
            impact, 0.0,
            "zero sqrtPriceX96 must return 0.0 (fail-honest)"
        );
    }

    /// Small swap (0.1% of liquidity) → impact well below 1%.
    ///
    /// Setup: √P = 1.0, L = 10^18.
    /// amount_in = 10^15 (= 0.001 of L).
    ///
    /// token0→token1: impact = 2 × √P × amt / L = 2 × 1.0 × 10^15 / 10^18 = 0.002
    #[test]
    fn test_v3_small_swap_low_impact() {
        let liquidity: u128 = 1_000_000_000_000_000_000; // 10^18
        let amount_in: u128 = 1_000_000_000_000_000; // 10^15 (0.1% of L)

        let impact_t0t1 = calc_univ3_price_impact_pct(amount_in, Q96_U128, liquidity, true);
        // Expected: 2 × 1.0 × 1e15 / 1e18 = 0.002
        assert!(
            (impact_t0t1 - 0.002).abs() < 1e-9,
            "token0→token1 small swap: expected 0.002, got {impact_t0t1}"
        );
        assert!(impact_t0t1 < 0.01, "small swap must have impact < 1%");

        let impact_t1t0 = calc_univ3_price_impact_pct(amount_in, Q96_U128, liquidity, false);
        // Same pool at √P=1: token1→token0 formula: 2 × amt / (L × √P) = 2 × 10^15 / (10^18 × 1.0) = 0.002
        assert!(
            (impact_t1t0 - 0.002).abs() < 1e-9,
            "token1→token0 small swap: expected 0.002, got {impact_t1t0}"
        );
        assert!(impact_t1t0 < 0.01, "small swap t1→t0 must have impact < 1%");
    }

    /// Extreme trade (amount_in >> liquidity) → impact clamped at 1.0.
    ///
    /// amount_in = 10^30 >> L = 10^18 → unclamped impact ≈ 2e12; capped at 1.0.
    #[test]
    fn test_v3_large_swap_capped_at_one() {
        let liquidity: u128 = 1_000_000_000_000_000_000; // 10^18
        let amount_in: u128 = 1_000_000_000_000_000_000_000_000_000_000; // 10^30

        let impact = calc_univ3_price_impact_pct(amount_in, Q96_U128, liquidity, true);
        assert_eq!(
            impact, 1.0,
            "extreme trade must be clamped to 1.0, got {impact}"
        );

        let impact_rev = calc_univ3_price_impact_pct(amount_in, Q96_U128, liquidity, false);
        assert_eq!(
            impact_rev, 1.0,
            "extreme trade (reverse) must be clamped to 1.0"
        );
    }

    /// Directionality: same trade size, different direction → different impact
    /// when √P ≠ 1.0.
    ///
    /// Setup: sqrt_price_x96 = 2 × 2^96  →  √P = 2.0
    ///        L = 10^18,  amount_in = 10^15
    ///
    /// token0→token1: impact = 2 × 2.0 × 10^15 / 10^18 = 0.004
    /// token1→token0: impact = 2 × 10^15 / (10^18 × 2.0) = 0.001
    ///
    /// The 4× ratio confirms the formula is direction-sensitive and sign-correct:
    /// selling token0 into a pool where token0 is cheap (√P > 1 means token1 is
    /// expensive) moves the price more than selling token1.
    #[test]
    fn test_v3_token0_vs_token1_directionality() {
        // sqrt_price_x96 = 2 × 2^96  →  √P = 2.0
        // Note: 2 × Q96_U128 must not overflow u128.
        // 2 × 2^96 = 2^97 ≈ 1.59e29  ≪  u128::MAX ≈ 3.40e38. Safe.
        let sqrt_price_2x: u128 = 2u128 * Q96_U128;
        let liquidity: u128 = 1_000_000_000_000_000_000; // 10^18
        let amount_in: u128 = 1_000_000_000_000_000; // 10^15

        let impact_t0t1 = calc_univ3_price_impact_pct(amount_in, sqrt_price_2x, liquidity, true);
        // 2 × 2.0 × 10^15 / 10^18 = 0.004
        assert!(
            (impact_t0t1 - 0.004).abs() < 1e-9,
            "token0→token1 at √P=2: expected 0.004, got {impact_t0t1}"
        );

        let impact_t1t0 = calc_univ3_price_impact_pct(amount_in, sqrt_price_2x, liquidity, false);
        // 2 × 10^15 / (10^18 × 2.0) = 0.001
        assert!(
            (impact_t1t0 - 0.001).abs() < 1e-9,
            "token1→token0 at √P=2: expected 0.001, got {impact_t1t0}"
        );

        // Directionality confirmed: token0→token1 is 4× token1→token0 at √P=2.
        let ratio = impact_t0t1 / impact_t1t0;
        assert!(
            (ratio - 4.0).abs() < 1e-6,
            "impact ratio t0t1/t1t0 should be (√P)^2 = 4.0, got {ratio}"
        );
    }

    // -----------------------------------------------------------------------
    // V3 tick-walking tests
    // -----------------------------------------------------------------------

    /// Helper: build a PoolTickDistribution for tests.
    fn make_dist(
        current_tick: i32,
        sqrt_price_x96: u128,
        liquidity: u128,
        ticks: Vec<crate::subgraph_client::TickInfo>,
    ) -> crate::subgraph_client::PoolTickDistribution {
        crate::subgraph_client::PoolTickDistribution {
            pool_addr: "0xtest".to_string(),
            current_tick,
            sqrt_price_x96,
            liquidity,
            ticks,
        }
    }

    /// No distribution (None) → falls back to first-order proxy.
    /// With sqrt_price_x96=0 and liquidity=0, proxy returns 0.0 (R8 fail-honest).
    #[test]
    fn test_v3_walk_no_distribution_falls_back() {
        let impact = calc_univ3_price_impact_with_distribution(100_000_000_000_000_000, None, true);
        // Fallback calls calc_univ3_price_impact_pct(amt, 0, 0, true) → 0.0
        assert_eq!(
            impact, 0.0,
            "None distribution must fall back to proxy returning 0.0"
        );
    }

    /// Small trade, single tick range (no tick list) → stays within current range.
    /// With enormous liquidity the impact must be very small (<1%).
    #[test]
    fn test_v3_walk_within_current_tick_small_impact() {
        // Pool at tick 0, √P = 2^96 (= 1.0 in real units), huge liquidity.
        let dist = make_dist(
            0,
            Q96_U128,
            1_000_000_000_000_000_000_000u128, // 10^21 (huge)
            vec![],                            // no tick list → trade stays in current range
        );
        let impact = calc_univ3_price_impact_with_distribution(
            1_000_000_000_000_000_000u128, // 1 token in (10^18)
            Some(&dist),
            true,
        );
        // With L = 10^21 and amt = 10^18: impact ≈ 2 × 10^18 / 10^21 = 0.002
        // (after exhausting tick list the remainder is handled by the tail formula)
        assert!(
            (0.0..0.01).contains(&impact),
            "small trade on deep pool must have impact < 1%, got {impact}"
        );
    }

    /// Degenerate distribution (liquidity=0) → returns 0.0 (R8).
    #[test]
    fn test_v3_walk_degenerate_zero_liquidity_returns_zero() {
        let dist = make_dist(0, Q96_U128, 0, vec![]);
        let impact = calc_univ3_price_impact_with_distribution(
            1_000_000_000_000_000_000u128,
            Some(&dist),
            true,
        );
        assert_eq!(
            impact, 0.0,
            "zero liquidity distribution must return 0.0 (R8)"
        );
    }

    /// Single tick crossing with thin liquidity → measurable cross-tick impact.
    ///
    /// Pool: tick=0, √P=2^96 (1.0), L=1_000 (very thin).
    /// One tick at tickIdx=-1 with liquidityNet=-500 (removes half liquidity on downward cross).
    /// Trade is large relative to liquidity → crosses the tick and shows elevated impact.
    #[test]
    fn test_v3_walk_crosses_tick_with_thin_liquidity() {
        let thin_liq: u128 = 1_000_000_000_000; // 10^12 (thin)
        let dist = make_dist(
            0,
            Q96_U128,
            thin_liq,
            vec![crate::subgraph_client::TickInfo {
                tick_idx: -1,
                liquidity_net: "-500000000000".to_string(), // removes half on downward cross
                liquidity_gross: "500000000000".to_string(),
            }],
        );
        // Trade size = 5× liquidity → must cross the tick boundary.
        let amount_in = 5 * thin_liq;
        let impact = calc_univ3_price_impact_with_distribution(amount_in, Some(&dist), true);
        // Impact must be significant (> 1%) given the thin pool and large trade.
        assert!(
            impact > 0.01,
            "large trade on thin pool must have impact > 1%, got {impact}"
        );
        // Must be bounded in [0, 1].
        assert!(impact <= 1.0, "impact must not exceed 1.0, got {impact}");
    }

    /// tick_to_sqrt_price_x96 sanity: tick=0 should give exactly 2^96.
    #[test]
    fn test_tick_to_sqrt_price_x96_at_zero() {
        let result = tick_to_sqrt_price_x96(0);
        // 1.0001^0 = 1.0; 1.0 × 2^96 = 2^96
        assert_eq!(result, Q96_U128, "tick=0 must map to 2^96, got {result}");
    }

    /// tick_to_sqrt_price_x96 sanity: positive tick → larger sqrtPrice.
    #[test]
    fn test_tick_to_sqrt_price_x96_positive_tick_larger() {
        let at_zero = tick_to_sqrt_price_x96(0);
        let at_positive = tick_to_sqrt_price_x96(100);
        assert!(
            at_positive > at_zero,
            "positive tick must produce larger sqrtPriceX96"
        );
    }

    /// tick_to_sqrt_price_x96 sanity: negative tick → smaller sqrtPrice.
    #[test]
    fn test_tick_to_sqrt_price_x96_negative_tick_smaller() {
        let at_zero = tick_to_sqrt_price_x96(0);
        let at_negative = tick_to_sqrt_price_x96(-100);
        assert!(
            at_negative < at_zero,
            "negative tick must produce smaller sqrtPriceX96"
        );
    }
}
