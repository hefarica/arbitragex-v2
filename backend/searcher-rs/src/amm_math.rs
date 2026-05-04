//! V2 CPMM math — pure functions for amount_out + spread + USD pricing.
//!
//! Reference: UniswapV2Library.getAmountOut.
//! https://github.com/Uniswap/v2-periphery/blob/master/contracts/libraries/UniswapV2Library.sol#L43-L50
//!
//! Doctrine: math is parametrised by `fee_bps` (basis points of 10_000). Default 30 = 0.30%
//! used by both UniswapV2 and SushiSwap; future tiers (e.g. 25 bps Pancake) plug in via the
//! same function without code changes.

use ethers::types::U256;

/// V2 constant-product market maker output amount, post-fee.
///
/// Formula (UniswapV2Library.getAmountOut):
///     amount_in_with_fee = amount_in * (10_000 - fee_bps)
///     numerator          = amount_in_with_fee * reserve_out
///     denominator        = reserve_in * 10_000 + amount_in_with_fee
///     amount_out         = numerator / denominator
///
/// Returns U256::zero() on degenerate inputs (zero reserves or zero amount_in).
pub fn v2_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256, fee_bps: u32) -> U256 {
    if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
        return U256::zero();
    }
    let fee_factor = U256::from(10_000u32 - fee_bps);
    let amount_in_with_fee = amount_in.saturating_mul(fee_factor);
    let numerator = amount_in_with_fee.saturating_mul(reserve_out);
    let denominator = reserve_in.saturating_mul(U256::from(10_000u32)).saturating_add(amount_in_with_fee);
    if denominator.is_zero() {
        return U256::zero();
    }
    numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UniswapV2Library reference: amount_in=1e18 (1 WETH), reserves=(3000e18 WETH, 6_000_000e6 USDC).
    /// Expected: roughly 1995 USDC (less than 2000 due to fee + slippage on 1/3000th of pool).
    /// Hand-computed: amount_in_with_fee = 9.97e21
    ///                numerator   = 9.97e21 * 6e12 = 5.982e34
    ///                denominator = 3e25 + 9.97e21 ≈ 3.00997e25
    ///                out         = 5.982e34 / 3.00997e25 ≈ 1.987e9 → 1987 USDC (6 decimals)
    /// We assert within 5% of 1987e6.
    #[test]
    fn weth_to_usdc_realistic_pool() {
        let amount_in = U256::from(10u128).pow(18.into());                  // 1 WETH
        let reserve_in = U256::from(3000u128) * U256::from(10u128).pow(18.into()); // 3000 WETH
        let reserve_out = U256::from(6_000_000u128) * U256::from(10u128).pow(6.into()); // 6M USDC
        let out = v2_amount_out(amount_in, reserve_in, reserve_out, 30);
        let expected = U256::from(1987u128) * U256::from(10u128).pow(6.into());
        // ±5%
        let lo = expected * U256::from(95) / U256::from(100);
        let hi = expected * U256::from(105) / U256::from(100);
        assert!(out >= lo && out <= hi, "got {} expected ~{}", out, expected);
    }

    #[test]
    fn fee_zero_matches_pure_xy() {
        // With fee_bps=0, amount_out = amount_in * reserve_out / (reserve_in + amount_in)
        let amount_in = U256::from(100u128);
        let reserve_in = U256::from(1_000u128);
        let reserve_out = U256::from(2_000u128);
        let out = v2_amount_out(amount_in, reserve_in, reserve_out, 0);
        // Manual: 100 * 2000 / (1000 + 100) = 200_000 / 1100 = 181 (truncated)
        assert_eq!(out, U256::from(181u128));
    }

    #[test]
    fn zero_amount_in_returns_zero() {
        let out = v2_amount_out(U256::zero(), U256::from(1_000u128), U256::from(2_000u128), 30);
        assert_eq!(out, U256::zero());
    }

    #[test]
    fn zero_reserve_in_returns_zero() {
        let out = v2_amount_out(U256::from(100u128), U256::zero(), U256::from(2_000u128), 30);
        assert_eq!(out, U256::zero());
    }

    #[test]
    fn zero_reserve_out_returns_zero() {
        let out = v2_amount_out(U256::from(100u128), U256::from(1_000u128), U256::zero(), 30);
        assert_eq!(out, U256::zero());
    }

    #[test]
    fn fee_30_bps_reduces_output_vs_zero_fee() {
        let amount_in = U256::from(1_000_000u128);
        let reserve_in = U256::from(10_000_000u128);
        let reserve_out = U256::from(20_000_000u128);
        let no_fee = v2_amount_out(amount_in, reserve_in, reserve_out, 0);
        let with_fee = v2_amount_out(amount_in, reserve_in, reserve_out, 30);
        assert!(with_fee < no_fee, "with_fee={} should be < no_fee={}", with_fee, no_fee);
    }
}
