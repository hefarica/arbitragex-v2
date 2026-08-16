//! `ValidatedPlan` — M2 carrier-B record.
//!
//! M2 makes `relays-client` broadcast the SAME `executeArbitrage` transaction
//! that `sim-ctl` validated. To reproduce byte-identical calldata, the broadcast
//! path must encode from the EXACT inputs the sim path encoded from. The encoder
//! [`crate::execute_arbitrage_encoder::build_execute_arbitrage_calldata`] takes
//! `(ctx, route_hash, min_profit_wei, executor_address)` — so carrier-B persists
//! precisely those four inputs keyed by `opp.id` at sim-validate time, and the
//! broadcast path reads them back to call the SAME encoder.
//!
//! This module defines ONLY the serializable carrier type. It does NOT touch the
//! fund path, the broadcast retarget, or any persistence wiring — those land in
//! later M2 increments.

use crate::round_trip_executor::RoundTripContext;
use ethers::types::{Address, U256};
use serde::{Deserialize, Serialize};

/// Carrier-B record: the exact validated inputs to
/// [`crate::execute_arbitrage_encoder::build_execute_arbitrage_calldata`],
/// persisted by `opp.id` at sim-validate time and read by the broadcast path so
/// it reproduces byte-identical `executeArbitrage` calldata (sim↔exec parity).
///
/// `route_hash` is a fixed 32-byte array. serde's const-generic array impls
/// (available since serde 1.0.123) cover `[u8; 32]` directly, so the plain
/// derive round-trips it with no custom adapter — it serializes as a JSON array
/// of 32 byte values and deserializes back to the identical bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedPlan {
    pub ctx: RoundTripContext,
    pub route_hash: [u8; 32],
    pub min_profit_wei: U256,
    pub executor_address: Address,
    /// The EXACT sim-validated wrapped-flash calldata
    /// (`build_flash_funded_broadcast_calldata_with_intermediate` output: outer
    /// `requestFlashLoan` 0x5107d61e wrapping inner `executeArbitrageFlashFunded`
    /// 0xdde0bf51). Produced by `sim_multistep::execute_multistep_revm` and filled
    /// from `SimulationOutcome.wrapped_calldata` ONLY on SIM_SUCCESS. The broadcast
    /// path sends these bytes VERBATIM — real byte-parity, not a re-encode. serde
    /// round-trips `Vec<u8>` as a JSON array of byte values.
    pub wrapped_calldata: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_rs::chains::{USDC_MAINNET, WETH_MAINNET};
    use std::str::FromStr;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).expect("valid hex address")
    }

    fn weth() -> Address {
        addr(WETH_MAINNET)
    }
    fn usdc() -> Address {
        addr(USDC_MAINNET)
    }

    fn fixed_plan() -> ValidatedPlan {
        let ctx = RoundTripContext {
            caller: addr("0x1111111111111111111111111111111111111111"),
            token_in: weth(),
            token_out: usdc(),
            amount_in: U256::from(10_u64).pow(U256::from(18)),
            forward_router: addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            forward_path: vec![weth(), usdc()],
            backward_router: addr("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
            backward_path: vec![usdc(), weth()],
            deadline: U256::from(1_700_000_000u64),
        };
        ValidatedPlan {
            ctx,
            route_hash: [0x11u8; 32],
            min_profit_wei: U256::from(1u64),
            executor_address: addr("0x2222222222222222222222222222222222222222"),
            // Non-trivial, non-uniform bytes (a plausible wrapped-flash calldata
            // prefix: requestFlashLoan 0x5107d61e) so the round-trip test catches
            // truncation / reordering in the byte-array serde path.
            wrapped_calldata: vec![0x51, 0x07, 0xd6, 0x1e, 0xde, 0xad, 0xbe, 0xef],
        }
    }

    /// Full JSON round trip: serialize → deserialize → compare field-by-field.
    ///
    /// We compare fields explicitly (rather than deriving `PartialEq` on
    /// `RoundTripContext`) to keep `RoundTripContext`'s derive set minimal — the
    /// only derive this increment adds to it is serde, which is purely additive
    /// to the sim path.
    #[test]
    fn validated_plan_json_round_trips() {
        let original = fixed_plan();

        let json = serde_json::to_string(&original).expect("serialize ValidatedPlan");
        let decoded: ValidatedPlan =
            serde_json::from_str(&json).expect("deserialize ValidatedPlan");

        // Load-bearing fields: these are what the broadcast path feeds back into
        // the encoder, so they MUST survive the round trip byte-for-byte.
        assert_eq!(
            decoded.route_hash, original.route_hash,
            "route_hash must survive the JSON round trip"
        );
        assert_eq!(
            decoded.executor_address, original.executor_address,
            "executor_address must survive the JSON round trip"
        );
        assert_eq!(decoded.min_profit_wei, original.min_profit_wei);

        // wrapped_calldata is broadcast VERBATIM, so it MUST survive byte-for-byte.
        assert_eq!(
            decoded.wrapped_calldata, original.wrapped_calldata,
            "wrapped_calldata (broadcast verbatim) must survive the JSON round trip"
        );

        // RoundTripContext (field-by-field — no PartialEq derive on it).
        assert_eq!(decoded.ctx.caller, original.ctx.caller);
        assert_eq!(decoded.ctx.token_in, original.ctx.token_in);
        assert_eq!(decoded.ctx.token_out, original.ctx.token_out);
        assert_eq!(decoded.ctx.amount_in, original.ctx.amount_in);
        assert_eq!(decoded.ctx.forward_router, original.ctx.forward_router);
        assert_eq!(decoded.ctx.forward_path, original.ctx.forward_path);
        assert_eq!(decoded.ctx.backward_router, original.ctx.backward_router);
        assert_eq!(decoded.ctx.backward_path, original.ctx.backward_path);
        assert_eq!(decoded.ctx.deadline, original.ctx.deadline);
    }

    /// Explicit guard that the full 32-byte `route_hash` (not just a prefix)
    /// is preserved — a distinct, non-uniform pattern catches truncation or
    /// element-reordering bugs in any future custom serde adapter.
    #[test]
    fn route_hash_full_32_bytes_preserved() {
        let mut plan = fixed_plan();
        let mut hash = [0u8; 32];
        for (i, b) in hash.iter_mut().enumerate() {
            *b = i as u8;
        }
        plan.route_hash = hash;

        let json = serde_json::to_string(&plan).expect("serialize");
        let decoded: ValidatedPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.route_hash, hash);
    }
}
