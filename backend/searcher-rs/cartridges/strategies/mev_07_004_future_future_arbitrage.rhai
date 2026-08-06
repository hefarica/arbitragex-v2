// Cartucho: MEV-07-004 — Future–future arbitrage
// Operador: op_01 | Modulo: derivatives_engine

let is_opp = false;
let conf = 0.0;
let profit_est = 0.0;
let threshold = 0.001;

// Read INJECTED swap data
if reserves_source != () {
    let r0 = reserves_source.r0;
    let r1 = reserves_source.r1;
    if r0 > 0.0 && r1 > 0.0 {
        let pool_price = r1 / r0;

        // Primary: USD oracle
        let oracle_price = get_token_price_usd(token_in);
        // Fallback: swap effective price (always available from intent)
        let swap_price = 0.0;
        if amount_in != () {
            let amt = parse_float(amount_in);
            if amt > 0.0 {
                swap_price = pool_price;  // pool_price IS the swap's post-trade price
            }
        }
        let ref_price = if oracle_price > 0.0 { oracle_price } else { swap_price };

        if ref_price > 0.0 {
            let mispricing = (pool_price - ref_price).abs() / ref_price;
            if mispricing > threshold {
                is_opp = true;
                conf = if mispricing > 0.05 { 0.9 } else if mispricing > 0.01 { 0.7 } else { 0.5 };
                profit_est = mispricing * 1000.0;
            }
        }
    }
}

# {
    is_opportunity: is_opp,
    estimated_profit: profit_est,
    confidence: conf,
    urgency: if is_opp { "monitor" } else { "none" },
    mev_id: "MEV-07-004",
    operator: "op_01",
}
