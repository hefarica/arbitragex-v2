//! Crossed-market pair classification and rating — TWO-LEG-01.
//!
//! REA: flashbots/simple-arbitrage@03cf1058b5bd8079561e2505efc85f77e357b829
//! (allowlist amendment firmada por el operador; ISC declarada en
//! package.json:5 al sha — patrones/semántica/números portados, CERO código
//! verbatim):
//!   - src/Arbitrage.ts:19-29  — TEST_VOLUMES ladder (9 sizes).
//!   - src/Arbitrage.ts:31-67  — getBestCrossedMarket: walk the ladder, on the
//!     FIRST profit decline try ONE midpoint `(size + best.volume)/2` and
//!     break; the best is overwritten on every non-declining step.
//!   - src/Arbitrage.ts:107-114 — crossing rule (strict): crossed[0]=sellTo
//!     (expensive), crossed[1]=buyFrom (cheap); the destination emits legs in
//!     TRADE order (buy first, sell closes), so this module exposes
//!     `buy`/`sell` and `crossed_pair_legs` orders them buy→sell.
//!   - src/UniswappyV2EthPair.ts:156-161 — getAmountOut fee/rounding order
//!     (fee on the input BEFORE the division, floor): reuses
//!     `amm_math::v2_amount_out` with `fee_bps` (9970/10000 ≡ 997/1000 exact
//!     integer floors).
//!
//! REA: paradigmxyz/artemis@4bb158070833ec2b789a2ece14c896c4b28ce3be
//! crates/artemis-core/src/engine.rs:75-141 — the loop invariant this slots
//! into: an event may classify to ZERO candidates (`Vec<Action>` vacío no es
//! error) and classification happens per event, before any emission.
//!
//! Declared deviation from the upstream rating (documented, not silent): the
//! upstream probes each market at a FIXED 0.01 ETH (`Arbitrage.ts:102-103`)
//! to pair markets; the scanner classifies direction with the quotes it
//! already holds at the OBSERVED trade size (argmax/argmin of `amount_out`).
//! Both are crossing tests; the observed size is a real market size, the
//! 0.01 ETH probe is a fixed heuristic, and the scanner has no reserve-free
//! reason to re-quote at a different size. The ladder + meet-halfway itself
//! IS the exact upstream port (proven by the vectors below).

use crate::route_intent::ProtocolType;
use ethers::types::{I256, U256};
use prioritization_spine::route_plan::RouteLeg;

/// Reserves of one pool, oriented for a specific swap direction:
/// `reserve_in` is the input-token side, `reserve_out` the output-token side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrientedReserves {
    pub reserve_in: U256,
    pub reserve_out: U256,
}

/// One pool's quote for a fixed input size (the observed trade size).
///
/// Carries the pool IDENTITY the legacy enrichment used to throw away:
/// address, protocol family, fee tier, and — only when the reserves entry
/// pinned `token0_addr`, i.e. orientation is a deployment fact, not a
/// magnitude heuristic — the directly-oriented reserves the V2 rating needs.
#[derive(Debug, Clone)]
pub struct PoolQuote {
    /// Lowercase 0x-prefixed pool address.
    pub pool_address: String,
    /// Protocol family of the INDEX the pool came from (V2 index ⇒ V2, V3
    /// index ⇒ V3 — `pool_sync_worker` filters the PG catalog by
    /// protocol_type, so the family is data, not a guess).
    pub family: ProtocolType,
    /// V2-family: Some(30) (0.30% doctrine, the same fee the quote used).
    /// V3: Some(raw fee pips). None when unknown.
    pub fee_bps: Option<u32>,
    /// Quoted output for the fixed input.
    pub amount_out: U256,
    /// Directly-oriented reserves (V2 with pinned token0 only; R8: the
    /// dual-orientation magnitude heuristic is NOT reserve truth).
    pub oriented_reserves: Option<OrientedReserves>,
}

/// The crossed pair — upstream `CrossedMarketDetails`
/// (Arbitrage.ts:8-14: profit, volume, tokenAddress, buyFrom, sellTo)
/// minus the sizing fields the destination's own optimizer owns.
#[derive(Debug, Clone)]
pub struct CrossedPair {
    /// buyFrom — pool that delivered the MOST token_out for the fixed input
    /// (cheapest token_out). Leg 0 of the round trip.
    pub buy: PoolQuote,
    /// sellTo — pool that delivered the LEAST (most expensive token_out).
    /// Leg 1 of the round trip (closes the cycle).
    pub sell: PoolQuote,
}

/// TEST_VOLUMES (Arbitrage.ts:19-29) — the upstream's 9-size ladder in wei:
/// ETHER/100, ETHER/10, ETHER/6, ETHER/4, ETHER/2, ×1, ×2, ×5, ×10 (BigInt
/// `div` = floor, so 1/6 ETH = 166666666666666666 exactly). Numeric facts of
/// the source (FUSILE binding rule 3): the golden vector V2's optimal size
/// 208333333333333333 is the meet-halfway midpoint (1/6 + 1/4)/2 — a port
/// that does not exercise the halfway step cannot reproduce it.
pub const TEST_VOLUMES_WEI: [u128; 9] = [
    10_000_000_000_000_000,
    100_000_000_000_000_000,
    166_666_666_666_666_666,
    250_000_000_000_000_000,
    500_000_000_000_000_000,
    1_000_000_000_000_000_000,
    2_000_000_000_000_000_000,
    5_000_000_000_000_000_000,
    10_000_000_000_000_000_000,
];

/// Crossing rule (Arbitrage.ts:107-114, strict inequality): a pair exists iff
/// two DISTINCT pools quoted the same fixed input and the best quote
/// strictly exceeds the worst — buy where token_out is cheap, sell where it
/// is expensive. Equal quotes ⇒ NOT crossed (upstream V3: equal markets never
/// push a candidate). Zero quotes are ignored (degenerate pool, R8).
pub fn classify_crossed_pair(quotes: &[PoolQuote]) -> Option<CrossedPair> {
    if quotes.len() < 2 {
        return None;
    }
    let mut best: Option<&PoolQuote> = None;
    let mut worst: Option<&PoolQuote> = None;
    for q in quotes {
        if q.amount_out.is_zero() {
            continue;
        }
        best = match best {
            Some(b) if b.amount_out >= q.amount_out => Some(b),
            _ => Some(q),
        };
        worst = match worst {
            Some(w) if w.amount_out <= q.amount_out => Some(w),
            _ => Some(q),
        };
    }
    let buy = best?;
    let sell = worst?;
    if buy.pool_address == sell.pool_address {
        return None; // one pool cannot be its own counter-venue
    }
    if buy.amount_out <= sell.amount_out {
        return None; // not strictly crossed
    }
    Some(CrossedPair {
        buy: buy.clone(),
        sell: sell.clone(),
    })
}

/// The rating's result at its best ladder size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossedMarketRating {
    /// Optimal input size found by the ladder (wei of token_in).
    pub volume: U256,
    /// Leg-0 output at `volume` (wei of token_out) == leg-1 input.
    pub mid: U256,
    /// Leg-1 output at `mid` (wei of token_in).
    pub proceeds: U256,
    /// `proceeds − volume`, SIGNED. The upstream classifier returns the best
    /// pair even when its profit is negative (V3 control); the ADMISSION
    /// threshold (Arbitrage.ts:117, operator config in this repo) is what
    /// excludes it — never this function.
    pub profit: I256,
}

/// U256 → I256 for amounts that fit in the signed half; None (rating not
/// computable, R8) on overflow rather than a fabricated wrap.
fn u256_to_i256(v: U256) -> Option<I256> {
    I256::try_from(v).ok()
}

/// getBestCrossedMarket (Arbitrage.ts:31-67) — exact port over
/// `amm_math::v2_amount_out`. Line-by-line order preserved:
///   1. for each ladder size: `mid = buy.getAmountOut(size)`,
///      `proceeds = sell.getAmountOut(mid)`, `profit = proceeds − size`;
///   2. on the FIRST profit decline vs the running best: try ONE midpoint
///      `(size + best.volume)/2`, keep it only if it beats the best, BREAK;
///   3. otherwise the best is OVERWRITTEN (upstream `best = {volume: size,
///      profit}` runs on every non-declining step, equal-profit included).
///
/// Each leg is priced with its own pool's fee (`buy_fee` for leg 0,
/// `sell_fee` for leg 1); the upstream hardcodes 997/1000 for both — V2
/// family 30 bps reproduces it exactly (9970/10000).
pub fn best_crossed_market_v2(
    buy: &OrientedReserves,
    buy_fee_bps: u32,
    sell: &OrientedReserves,
    sell_fee_bps: u32,
) -> Option<CrossedMarketRating> {
    let price = |reserves: &OrientedReserves, fee: u32, amount_in: U256| {
        crate::amm_math::v2_amount_out(amount_in, reserves.reserve_in, reserves.reserve_out, fee)
    };
    let mut best: Option<CrossedMarketRating> = None;
    for &size_raw in TEST_VOLUMES_WEI.iter() {
        let size = U256::from(size_raw);
        let mid = price(buy, buy_fee_bps, size);
        let proceeds = price(sell, sell_fee_bps, mid);
        let profit = u256_to_i256(proceeds)? - u256_to_i256(size)?;
        if let Some(b) = best.as_ref() {
            if profit < b.profit {
                // Meet halfway (Arbitrage.ts:42) — ONE midpoint, then break
                // regardless of outcome (Arbitrage.ts:55): the scan is not
                // exhaustive by design.
                let try_size = (size + b.volume) / 2;
                let try_mid = price(buy, buy_fee_bps, try_size);
                let try_proceeds = price(sell, sell_fee_bps, try_mid);
                let try_profit = u256_to_i256(try_proceeds)? - u256_to_i256(try_size)?;
                if try_profit > b.profit {
                    best = Some(CrossedMarketRating {
                        volume: try_size,
                        mid: try_mid,
                        proceeds: try_proceeds,
                        profit: try_profit,
                    });
                }
                break;
            }
        }
        best = Some(CrossedMarketRating {
            volume: size,
            mid,
            proceeds,
            profit,
        });
    }
    best
}

/// Rate a crossed pair with the ported ladder when BOTH legs are V2-family
/// pools whose reserves were directly oriented (pinned token0 — deployment
/// fact). R8 fail-honest: V3 legs (no local reserves) or heuristic-oriented
/// V2 legs attach no rating — "not computed", never a guessed figure.
pub fn rate_crossed_pair_v2(pair: &CrossedPair) -> Option<CrossedMarketRating> {
    if pair.buy.family != ProtocolType::V2 || pair.sell.family != ProtocolType::V2 {
        return None;
    }
    let buy_reserves = pair.buy.oriented_reserves.as_ref()?;
    let sell_reserves = pair.sell.oriented_reserves.as_ref()?;
    let buy_fee = pair.buy.fee_bps?;
    let sell_fee = pair.sell.fee_bps?;
    best_crossed_market_v2(buy_reserves, buy_fee, sell_reserves, sell_fee)
}

/// Flat-column venue labels for the persisted row. Destination convention
/// (`dex_engine.rs:1306-1307` fills dex_a/dex_b with the two pools' venue
/// labels; the venue-unresolved label is the protocol family —
/// `protocol_type_to_str`, the same mapping `hop_cycle_bridge` uses for
/// venue-less legs): dex_b non-null is the TWO-LEG-01 acceptance.
pub fn crossed_pair_dex_labels(pair: &CrossedPair) -> (String, Option<String>) {
    (
        crate::engines::dex_engine::protocol_type_to_str(pair.buy.family),
        Some(crate::engines::dex_engine::protocol_type_to_str(
            pair.sell.family,
        )),
    )
}

/// The two-leg closed-cycle route legs for a crossed pair, in TRADE order
/// (destination convention: leg 0 buys token_in→token_out on the cheap pool,
/// leg 1 sells token_out→token_in on the expensive pool and closes the
/// cycle). Leg 0 carries the OBSERVED amounts; leg 1's amounts are the
/// rating's business (`rate_crossed_pair_v2` + `attach_leg_ledger`), not
/// fabricated here.
pub fn crossed_pair_legs(
    pair: &CrossedPair,
    token_in_lower: &str,
    token_out_lower: &str,
    amount_in_f64: f64,
    expected_amount_out_f64: f64,
) -> Vec<RouteLeg> {
    let mk = |pool_address: &str,
              family: ProtocolType,
              fee_bps: Option<u32>,
              t_in: &str,
              t_out: &str,
              amount_in: Option<f64>,
              amount_out: Option<f64>| {
        let family_label = crate::engines::dex_engine::protocol_type_to_str(family);
        RouteLeg {
            dex_id: family_label.to_ascii_lowercase(),
            dex_name: family_label.clone(),
            protocol_type: family_label,
            // The pair index carries no factory identity — empty, never a
            // sentinel (hop_cycle_bridge convention).
            factory_address: String::new(),
            pool_id: None,
            pool_address: Some(pool_address.to_string()),
            token_in: t_in.to_string(),
            token_out: t_out.to_string(),
            fee_bps,
            amount_in,
            amount_out,
            tvl_usd: None,
            volume_24h_usd: None,
            pool_is_active: true,
        }
    };
    vec![
        mk(
            &pair.buy.pool_address,
            pair.buy.family,
            pair.buy.fee_bps,
            token_in_lower,
            token_out_lower,
            Some(amount_in_f64),
            Some(expected_amount_out_f64),
        ),
        mk(
            &pair.sell.pool_address,
            pair.sell.family,
            pair.sell.fee_bps,
            token_out_lower,
            token_in_lower,
            None,
            None,
        ),
    ]
}

// ============================================================================
// Tests — upstream vectors (REGLA DE ORO REA: sin el número del repo fuente
// como test, esto no es un port). Fuentes de los vectores:
//   flashbots/simple-arbitrage@03cf1058
//     test/UniswappyV2EthPair.test.ts:20-27 (V1)
//     test/Arbitrage.test.ts:21-32 (V2 — EL vector del port)
//     test/Arbitrage.test.ts:33-43 (V3 — control negativo)
//   Verificación independiente previa: docs/rea/vectors_rea1_verify.mjs
//   (node, exit 0, 7/7) sobre FICHA_REA1_ARTEMIS.md §5.
// ============================================================================
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const E18: u128 = 1_000_000_000_000_000_000;

    fn u(x: u128) -> U256 {
        U256::from(x)
    }

    fn quote(
        pool: &str,
        family: ProtocolType,
        fee_bps: Option<u32>,
        amount_out: u128,
        reserves: Option<(u128, u128)>,
    ) -> PoolQuote {
        PoolQuote {
            pool_address: pool.to_string(),
            family,
            fee_bps,
            amount_out: u(amount_out),
            oriented_reserves: reserves.map(|(rin, rout)| OrientedReserves {
                reserve_in: u(rin),
                reserve_out: u(rout),
            }),
        }
    }

    // ── V1 — formula base (UniswappyV2EthPair.test.ts:20-27) ──────────────
    // Pool reserves [1 TOKEN, 2 WETH]. The upstream getAmountIn legs of V1
    // (V1a/V1b) are NOT ported — the classifier derives direction from the
    // observed-size quotes (documented deviation, module docs). V1c/V1d pin
    // the getAmountOut fee/rounding order the whole ladder rides on.
    #[test]
    fn v1c_get_tokens_out_token_to_weth_round_trip() {
        // getTokensOut(TOKEN→WETH, 52789948793749671) = ETHER/10 + 1
        let out = crate::amm_math::v2_amount_out(u(52789948793749671), u(E18), u(2 * E18), 30);
        assert_eq!(out, u(E18 / 10 + 1));
    }

    #[test]
    fn v1d_get_tokens_out_weth_to_token_round_trip() {
        // getTokensOut(WETH→TOKEN, 222890894906943052) = ETHER/10 exactly
        let out = crate::amm_math::v2_amount_out(u(222890894906943052), u(2 * E18), u(E18), 30);
        assert_eq!(out, u(E18 / 10));
    }

    // ── V2 — the port's golden vector (Arbitrage.test.ts:21-32) ───────────
    // market[0] reserves [1 TOKEN, 2 WETH] (expensive → sellTo),
    // market[1] reserves [1 TOKEN, 1 WETH] (cheap → buyFrom).
    // buyFrom = m1 oriented WETH→TOKEN: (1 WETH, 1 TOKEN);
    // sellTo  = m0 oriented TOKEN→WETH: (1 TOKEN, 2 WETH).
    #[test]
    fn v2_golden_vector_volume_and_profit() {
        let buy = OrientedReserves {
            reserve_in: u(E18),
            reserve_out: u(E18),
        };
        let sell = OrientedReserves {
            reserve_in: u(E18),
            reserve_out: u(2 * E18),
        };
        let r = best_crossed_market_v2(&buy, 30, &sell, 30).unwrap();
        assert_eq!(
            r.volume,
            u(208333333333333333),
            "V2 volume (meet-halfway of 1/6 and 1/4)"
        );
        assert_eq!(
            r.profit,
            I256::from(84409320961485006i64),
            "V2 profit = 0x012be1d487a428ce"
        );
        // The rating's ledger is internally consistent: leg1_out − leg0_in IS
        // the reported profit (the invariant the persisted row carries).
        assert_eq!(
            u256_to_i256(r.proceeds).unwrap() - u256_to_i256(r.volume).unwrap(),
            r.profit
        );
    }

    // ── V3 — negative control (Arbitrage.test.ts:33-43) ───────────────────
    // Equal reserves [1,1]/[1,1]: the classifier returns a candidate with
    // NEGATIVE profit; the admission threshold (Arbitrage.ts:117 — operator
    // config here, never a code literal) is what excludes it.
    #[test]
    fn v3_negative_control_profit_negative_and_excluded_by_threshold() {
        let eq = OrientedReserves {
            reserve_in: u(E18),
            reserve_out: u(E18),
        };
        let nc = best_crossed_market_v2(&eq, 30, &eq, 30).unwrap();
        assert!(nc.profit.is_negative(), "V3: best pair profit is negative");
        // Exact re-implementation value (FICHA_REA1 §5 V3): the ladder never
        // improves, so the best stays at the FIRST size with the double-fee
        // round-trip loss.
        assert_eq!(
            nc.volume,
            u(E18 / 100),
            "V3: best stays at the first ladder size"
        );
        assert_eq!(nc.profit, I256::from(-253954640256575i64));
        // And the CLASSIFIER never pairs equal quotes (strict crossing) —
        // there is no row with a crossed pair to threshold in the first place.
        let equal_quote = crate::amm_math::v2_amount_out(u(E18 / 100), u(E18), u(E18), 30);
        let equal_quotes = vec![
            quote(
                "0xa",
                ProtocolType::V2,
                Some(30),
                equal_quote.low_u128(),
                Some((E18, E18)),
            ),
            quote(
                "0xb",
                ProtocolType::V2,
                Some(30),
                equal_quote.low_u128(),
                Some((E18, E18)),
            ),
        ];
        assert!(
            classify_crossed_pair(&equal_quotes).is_none(),
            "V3: equal quotes are NOT crossed — excluded before any rating, never a profitable row"
        );
    }

    // ── Classifier direction semantics ─────────────────────────────────────
    #[test]
    fn classify_picks_buy_argmax_sell_argmin_across_families() {
        let quotes = vec![
            quote("0xpool_lo", ProtocolType::V3, Some(3000), 90, None),
            quote(
                "0xpool_hi",
                ProtocolType::V2,
                Some(30),
                100,
                Some((E18, E18)),
            ),
            quote("0xpool_mid", ProtocolType::V2, Some(30), 95, None),
            quote("0xpool_zero", ProtocolType::V2, Some(30), 0, None), // degenerate: ignored
        ];
        let pair = classify_crossed_pair(&quotes).unwrap();
        assert_eq!(pair.buy.pool_address, "0xpool_hi");
        assert_eq!(pair.sell.pool_address, "0xpool_lo");
        assert_eq!(pair.buy.family, ProtocolType::V2);
        assert_eq!(pair.sell.family, ProtocolType::V3);
    }

    #[test]
    fn classify_rejects_single_pool_and_same_pool_twice() {
        assert!(classify_crossed_pair(&[]).is_none());
        let one = vec![quote("0xa", ProtocolType::V2, Some(30), 100, None)];
        assert!(classify_crossed_pair(&one).is_none());
        // Same pool quoted twice (e.g. present in two indexes): not a pair.
        let twice = vec![
            quote("0xa", ProtocolType::V2, Some(30), 100, None),
            quote("0xa", ProtocolType::V3, Some(3000), 90, None),
        ];
        assert!(classify_crossed_pair(&twice).is_none());
    }

    #[test]
    fn rate_requires_v2_legs_with_oriented_reserves() {
        let mixed = CrossedPair {
            buy: quote("0xhi", ProtocolType::V2, Some(30), 100, Some((E18, E18))),
            sell: quote("0xlo", ProtocolType::V3, Some(3000), 90, None),
        };
        assert!(
            rate_crossed_pair_v2(&mixed).is_none(),
            "V3 leg: no local reserves — R8 None"
        );
        let heuristic = CrossedPair {
            buy: quote("0xhi", ProtocolType::V2, Some(30), 100, Some((E18, E18))),
            sell: quote("0xlo", ProtocolType::V2, Some(30), 90, None), // heuristic orientation
        };
        assert!(
            rate_crossed_pair_v2(&heuristic).is_none(),
            "heuristic reserves are not truth"
        );
        let both = CrossedPair {
            buy: quote("0xhi", ProtocolType::V2, Some(30), 100, Some((E18, E18))),
            sell: quote("0xlo", ProtocolType::V2, Some(30), 90, Some((E18, 2 * E18))),
        };
        assert!(rate_crossed_pair_v2(&both).is_some());
    }

    // ── The emitter's row shape (TWO-LEG-01 acceptance) ────────────────────
    // Given the V2 scenario pools, the emitted row carries BOTH legs:
    // dex_b non-null, pool_addresses[1] present, closed-cycle token path,
    // and the rating's per-leg ledger at the optimal size.
    #[test]
    fn emitter_row_carries_both_legs_and_dex_b_non_null() {
        use crate::persistence;

        // The V2 scenario: two V2 pools, [1 TOKEN, 2 WETH] vs [1 TOKEN, 1 WETH].
        // The quotes are computed exactly as the scanner computes them at
        // runtime — v2_amount_out at the observed size, oriented WETH→TOKEN
        // (m1 [1 WETH, 1 TOKEN] delivers more TOKEN per WETH ⇒ cheap ⇒ buy;
        // m0 [2 WETH, 1 TOKEN] ⇒ expensive ⇒ sell).
        let observed_size = u(5 * E18 / 1000);
        let m1_buy_quote = crate::amm_math::v2_amount_out(observed_size, u(E18), u(E18), 30);
        let m0_sell_quote = crate::amm_math::v2_amount_out(observed_size, u(2 * E18), u(E18), 30);
        assert!(
            m1_buy_quote > m0_sell_quote,
            "m1 [1,1] is the cheap side (buyFrom); m0 [1,2] the expensive side (sellTo)"
        );
        let quotes = vec![
            // sellTo m0 [1 TOKEN, 2 WETH] — oriented for the SELL leg:
            // (1 TOKEN in, 2 WETH out).
            quote(
                "0xpool_sell_m0",
                ProtocolType::V2,
                Some(30),
                m0_sell_quote.low_u128(),
                Some((E18, 2 * E18)),
            ),
            // buyFrom m1 [1 TOKEN, 1 WETH] — oriented for the BUY leg:
            // (1 WETH in, 1 TOKEN out).
            quote(
                "0xpool_buy_m1",
                ProtocolType::V2,
                Some(30),
                m1_buy_quote.low_u128(),
                Some((E18, E18)),
            ),
        ];
        let pair = classify_crossed_pair(&quotes)
            .expect("distinct pools with strictly different quotes are crossed");
        assert_eq!(pair.buy.pool_address, "0xpool_buy_m1");
        assert_eq!(pair.sell.pool_address, "0xpool_sell_m0");

        // Flat columns: the row's dex_a/dex_b are the two pools' venue labels
        // (family label when the venue is unresolved — destination convention).
        let (dex_a, dex_b) = crossed_pair_dex_labels(&pair);
        assert_eq!(dex_a, "uniswap-v2");
        let dex_b = dex_b.expect("TWO-LEG-01: dex_b non-null on a crossed pair");
        assert_eq!(dex_b, "uniswap-v2");

        // Route topology: two legs, closed cycle, both pool addresses.
        let legs = crossed_pair_legs(&pair, "0xweth", "0xtoken", 0.005, 0.00995);
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].pool_address.as_deref(), Some("0xpool_buy_m1"));
        assert_eq!(legs[1].pool_address.as_deref(), Some("0xpool_sell_m0"));
        let mut rm = persistence::build_route_metadata_from_plan(
            &prioritization_spine::route_plan::RoutePlan {
                route_id: Some("test-crossed".into()),
                strategy_kind: "dex_arb_v2v2".into(),
                chain_id: 1,
                legs,
                atomic: true,
                estimated_slippage_pct: None,
                price_impact_pct: None,
            },
        );
        assert_eq!(rm.token_addresses, vec!["0xweth", "0xtoken", "0xweth"]);
        assert_eq!(rm.dex_adapters.len(), 2);
        assert_eq!(rm.pool_addresses.len(), 2, "pool_addresses[1] presente");
        assert_eq!(rm.pool_addresses[1], "0xpool_sell_m0");
        assert!(rm.is_populated());

        // The rating at the optimal size rides the row as the per-leg ledger
        // (HOPS-LEDGER-04 all-or-nothing attach): leg0_in at 208333333333333333
        // and leg1_out − leg0_in == the golden profit.
        let rating = rate_crossed_pair_v2(&pair).unwrap();
        assert_eq!(rating.volume, u(208333333333333333));
        assert_eq!(rating.profit, I256::from(84409320961485006i64));
        let attached = rm.attach_leg_ledger(
            &[rating.volume.to_string(), rating.mid.to_string()],
            &[rating.mid.to_string(), rating.proceeds.to_string()],
        );
        assert!(attached, "ledger arrays align with the 2-hop plan");
        assert_eq!(
            rm.leg_amounts_in.as_deref(),
            Some(&[rating.volume.to_string(), rating.mid.to_string()][..])
        );
        let leg0_in: u128 = rm.leg_amounts_in.as_ref().unwrap()[0].parse().unwrap();
        let leg1_out: u128 = rm.leg_amounts_out.as_ref().unwrap()[1].parse().unwrap();
        assert_eq!(leg1_out as i128 - leg0_in as i128, 84409320961485006i128);
    }

    // ── NEGATIVE CONTROL — the pre-change row shape, same fixture family ──
    // Local, reproducible form of "ventana previa al cambio: 0 filas con
    // dex_b": with the legacy single-leg construction (the exact shape
    // scanner.rs builds when no pair is classified — 1 leg, pool_address
    // None, dex_b None from patterns.rs), the row has NO second leg and NO
    // dex_b. Production baseline measured pre-change (2026-10-23, PG VPS):
    // 493 rows in the trailing 6h window, 493/493 dex_b NULL,
    // route_metadata.pool_addresses == [""] — zero rows with dex_b.
    #[test]
    fn negative_control_legacy_single_leg_shape_has_no_dex_b() {
        use crate::persistence;

        // patterns.rs builds dex_b: None (patterns.rs:46) — unchanged legacy.
        let legacy_dex_b: Option<String> = None;
        assert!(
            legacy_dex_b.is_none(),
            "pre-change: the emitter persists one leg only"
        );

        // The legacy plan: ONE observed leg, pool_address: None (scanner.rs
        // Migration 056 shape) — route_metadata lands as pool_addresses [""].
        let legacy_leg = RouteLeg {
            dex_id: "uniswap-v2".to_string(),
            dex_name: "uniswap-v2".to_string(),
            protocol_type: "uniswap-v2".to_string(),
            factory_address: String::new(),
            pool_id: None,
            pool_address: None,
            token_in: "0xweth".to_string(),
            token_out: "0xtoken".to_string(),
            fee_bps: None,
            amount_in: Some(0.005),
            amount_out: Some(0.00995),
            tvl_usd: None,
            volume_24h_usd: None,
            pool_is_active: true,
        };
        let rm = persistence::build_route_metadata_from_plan(
            &prioritization_spine::route_plan::RoutePlan {
                route_id: Some("test-legacy".into()),
                strategy_kind: "dex_arb_v2v2".into(),
                chain_id: 1,
                legs: vec![legacy_leg],
                atomic: true,
                estimated_slippage_pct: None,
                price_impact_pct: None,
            },
        );
        assert_eq!(
            rm.pool_addresses,
            vec![String::new()],
            "pre-change: one EMPTY pool slot"
        );
        assert_eq!(rm.dex_adapters.len(), 1, "pre-change: one adapter");
        assert!(
            rm.pool_addresses.get(1).is_none(),
            "pre-change: NO pool_addresses[1]"
        );
    }
}
