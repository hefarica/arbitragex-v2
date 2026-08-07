// ═══════════════════════════════════════════════════════════════════════════
// MEV-01-016 — Triangular Arbitrage (R_CLOSED_CYCLE, 3-leg fixed)
// Detector: depth-aware route composition Q_R(x) = q3(q2(q1(x))).
// DETECTION ONLY — reads reserves via get_reserves (read-only), computes V2 CPMM
// quotes per leg, checks gross > 0. No signer, no broadcast, no ETH needed.
// ═══════════════════════════════════════════════════════════════════════════

fn init_strategy() {
    #{
        mev_id: "MEV-01-016",
        name: "Triangular arbitrage",
        version: "1.0.0",
        author: "ArbitrageX",
        description: "Triangular arbitrage (R_CLOSED_CYCLE, 3-leg, depth-aware V2 CPMM)",
        detector_id: "R_CLOSED_CYCLE",
        min_legs: 3,
        max_legs: 3,
        applicable_operators: [27, 21, 15, 16],
        secondary_operators: [1, 22, 26, 30],
        mode_invariant: true,
        config_schema: #{ min_profit_usd: 0.0, max_slippage_pct: 1.0 }
    }
}

fn evaluate_opportunity(ctx) {
    let legs = ctx.route;
    if legs == () || legs.len() < 3 {
        return #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0, urgency: "none", reason: "route_too_short", mev_id: "MEV-01-016" };
    }
    if !ctx.route_closed {
        return #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0, urgency: "none", reason: "route_not_closed", mev_id: "MEV-01-016" };
    }

    let amount_in = 1000.0;
    let x = amount_in;
    let i = 0;

    while i < legs.len() {
        let leg = legs[i];
        let pool_addr = leg.pool;
        if pool_addr == "" {
            return #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0, urgency: "none", reason: "missing_pool", mev_id: "MEV-01-016", leg: i };
        }

        let rd = get_reserves(pool_addr);
        if rd == () {
            return #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0, urgency: "none", reason: "no_reserves", mev_id: "MEV-01-016", pool: pool_addr };
        }

        let r0 = to_float("" + rd.r0);
        let r1 = to_float("" + rd.r1);
        if r0 <= 0.0 || r1 <= 0.0 {
            return #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0, urgency: "none", reason: "zero_reserves", mev_id: "MEV-01-016", pool: pool_addr };
        }

        let r_in = r0;
        let r_out = r1;
        let t0 = "" + rd.token0_addr;
        if t0 != "" && leg.token_in != t0 {
            r_in = r1;
            r_out = r0;
        }

        let fee_factor = 0.997;
        let fb = leg.fee_bps;
        if fb != () {
            fee_factor = (10000.0 - to_float("" + fb)) / 10000.0;
        }

        let awf = x * fee_factor;
        x = (awf * r_out) / (r_in + awf);
        i += 1;
    }

    let gross = x - amount_in;
    let gross_pct = (gross / amount_in) * 100.0;

    if gross > 0.0 {
        return #{
            is_opportunity: true,
            estimated_profit: gross,
            confidence: 0.0,
            urgency: "monitor",
            reason: "closed_cycle_depth_positive",
            detector_id: "R_CLOSED_CYCLE",
            mev_id: "MEV-01-016",
            initial_amount: amount_in,
            final_amount: x,
            gross_profit: gross,
            gross_pct: gross_pct
        };
    }
    return #{
        is_opportunity: false,
        estimated_profit: 0.0,
        confidence: 0.0,
        urgency: "none",
        reason: "non_positive_gross",
        mev_id: "MEV-01-016",
        gross_pct: gross_pct
    };
}

fn build_payload(opp) {
    #{
        target: "ArbitrageExecutor",
        mev_id: opp.mev_id,
        detector_id: opp.detector_id,
        final_amount: opp.final_amount,
        gross_profit: opp.gross_profit
    }
}
