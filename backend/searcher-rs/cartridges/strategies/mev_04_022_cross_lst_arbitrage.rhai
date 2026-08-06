// Cartucho: MEV-04-022 — Cross-LST arbitrage
// Operador: op_01 | Modulo: parity_redemption_engine | Modo: SHADOW
// FIX v2: uses INJECTED reserves_source (r0/r1 from detected swap) instead of empty pool_a.
// Compares pool_price vs USD oracle → detects real mispricing.

let is_opp = false;
let conf = 0.0;
let profit_est = 0.0;
let threshold = 0.001;  // 10 bps (lowered from 30 to catch more)

    // op_01: reads the INJECTED swap data (reserves_source) + compares pool price vs USD oracle
    let asymmetry = 0.0;
    let conf = 0.0;
    let profit_est = 0.0;

    // reserves_source is INJECTED by the orchestrator (from the detected swap)
    if reserves_source != () {
        let r0 = reserves_source.r0;
        let r1 = reserves_source.r1;
        if r0 > 0.0 && r1 > 0.0 {
            let pool_price = r1 / r0;
            // Compare with USD oracle price
            let oracle_price = get_token_price_usd(token_in);
            if oracle_price > 0.0 && pool_price > 0.0 {
                let price_diff = (pool_price - oracle_price).abs() / oracle_price;
                if price_diff > threshold {
                    is_opp = true;
                    let evidence = get_math_evidence("04_022");
                    let kelly = 0.5;
                    if evidence != () && evidence.kelly_fraction > 0.0 { kelly = evidence.kelly_fraction; }
                    conf = kelly * (1.0 - math_exp(-price_diff * 50.0));
                    if conf > 1.0 { conf = 1.0; }
                    profit_est = price_diff * amount_in * conf;
                }
            }
        }
    }

# {
    is_opportunity: is_opp,
    estimated_profit: profit_est,
    confidence: conf,
    urgency: if is_opp { "monitor" } else { "none" },
    mev_id: "MEV-04-022",
    operator: "op_01",
}
