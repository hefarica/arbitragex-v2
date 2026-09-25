// ═══════════════════════════════════════════════════════════════════════════════
// ArbitrageX v2 — FASE OMEGA
// Cartucho: Triangular Arbitrage (Multi-Chain Universal)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Detecta ciclos de arbitraje triangular: A → B → C → A donde el valor
// final de A es mayor que el inicial. Completamente chain-agnostic.
//
// ═══════════════════════════════════════════════════════════════════════════════

fn init_strategy() {
    #{
        name: "Triangular Arbitrage Universal",
        version: "2.0.0",
        author: "ArbitrageX Core Team",
        description: "Multi-chain triangular arbitrage detector. Finds profitable A→B→C→A cycles across DEX pools on any supported chain. Uses constant-product math with fee-aware path optimization.",
        category: "triangular_arb",
        target_chains: [],  // ALL chains
        min_eval_interval_ms: 100,
        supported_protocols: ["uniswap_v2", "uniswap_v3", "sushiswap", "pancakeswap", "camelot", "aerodrome"],
        risk_profile: "low",
        capital_requirement: "medium"
    }
}

/// Evalúa oportunidades de arbitraje triangular.
///
/// pool_data incluye:
/// #{
///     chain_id: N,
///     token_a: "0x...",       // Token base del ciclo
///     token_b: "0x...",       // Token intermedio 1
///     token_c: "0x...",       // Token intermedio 2
///     pools_ab: [...],        // Pools para par A/B
///     pools_bc: [...],        // Pools para par B/C
///     pools_ca: [...],        // Pools para par C/A
///     amount_in: "...",       // Capital inicial en token A
///     gas_price_gwei: N
/// }
fn evaluate_opportunity(pool_data) {
    let chain_id = get_chain_id();
    let token_a = pool_data.token_a;
    let token_b = pool_data.token_b;
    let token_c = pool_data.token_c;

    // Obtener metadata de tokens
    let meta_a = get_token_meta(token_a);
    let meta_b = get_token_meta(token_b);
    let meta_c = get_token_meta(token_c);

    if meta_a == () || meta_b == () || meta_c == () {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "token_meta_unavailable"
        };
    }

    // Buscar pools para cada par
    let pools_ab = get_pool_index(meta_a.symbol, meta_b.symbol);
    let pools_bc = get_pool_index(meta_b.symbol, meta_c.symbol);
    let pools_ca = get_pool_index(meta_c.symbol, meta_a.symbol);

    if pools_ab.len() == 0 || pools_bc.len() == 0 || pools_ca.len() == 0 {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "incomplete_triangle"
        };
    }

    // Evaluar el mejor ciclo triangular
    let amount_in = pool_data.amount_in;
    let best_result = find_best_triangle(
        pools_ab, pools_bc, pools_ca,
        amount_in,
        meta_a.decimals, meta_b.decimals, meta_c.decimals
    );

    if best_result.profit <= 0.0 {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "no_profitable_cycle"
        };
    }

    // Evaluar contra gas cost
    let base_fee = get_base_fee();
    let gas_cost = estimate_triangle_gas(chain_id) * base_fee / 1000000000.0;

    if best_result.profit <= gas_cost * 1.3 {
        return #{
            is_opportunity: false,
            estimated_profit: best_result.profit,
            confidence: 0.0,
            urgency: "none",
            reason: "below_gas_threshold",
            gas_cost: gas_cost
        };
    }

    let net_profit = best_result.profit - gas_cost;
    let confidence = calculate_triangle_confidence(best_result, gas_cost);
    let urgency = if confidence > 0.8 { "immediate" } else if confidence > 0.5 { "next_block" } else { "monitor" };

    log_quantum("info", `[chain:${chain_id}] Triangle arb: ${meta_a.symbol}→${meta_b.symbol}→${meta_c.symbol}→${meta_a.symbol} profit=${net_profit}`);

    #{
        is_opportunity: true,
        estimated_profit: net_profit,
        confidence: confidence,
        urgency: urgency,
        route_type: "three_leg_arb",
        chain_id: chain_id,
        token_a: token_a,
        token_b: token_b,
        token_c: token_c,
        pool_ab: best_result.pool_ab,
        pool_bc: best_result.pool_bc,
        pool_ca: best_result.pool_ca,
        amount_in: amount_in,
        amount_out_final: best_result.amount_out,
        gas_cost: gas_cost
    }
}

fn build_payload(opportunity) {
    let chain_id = get_chain_id();
    let timestamp = get_timestamp();
    let base_fee = get_base_fee();
    let block_time = 3; // Generic fallback

    #{
        target_contract: "EXECUTOR",
        calldata: "0x_triangle_arb_placeholder",
        value_wei: "0",
        gas_limit: 550000,
        max_priority_fee_gwei: base_fee * 1.5,
        deadline_ts: timestamp + block_time * 2,
        route: #{
            type: "triangular",
            legs: [
                #{ pool: opportunity.pool_ab, token_in: opportunity.token_a, token_out: opportunity.token_b },
                #{ pool: opportunity.pool_bc, token_in: opportunity.token_b, token_out: opportunity.token_c },
                #{ pool: opportunity.pool_ca, token_in: opportunity.token_c, token_out: opportunity.token_a }
            ]
        },
        risk: #{
            max_slippage_bps: 30,
            min_profit_after_gas: opportunity.estimated_profit * 0.75,
            revert_protection: true
        },
        meta: #{
            cartridge: "triangular_arb_universal",
            version: "2.0.0",
            chain_id: chain_id
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn find_best_triangle(pools_ab, pools_bc, pools_ca, amount_in_str, dec_a, dec_b, dec_c) {
    let best = #{ profit: 0.0, pool_ab: "", pool_bc: "", pool_ca: "", amount_out: "0" };
    let amount_in = parse_float(amount_in_str);

    // Iterate over pool combinations (limit to top 3 per pair for performance)
    let max_per_pair = 3;
    let ab_limit = math_min(pools_ab.len().to_float(), max_per_pair.to_float()).to_int();
    let bc_limit = math_min(pools_bc.len().to_float(), max_per_pair.to_float()).to_int();
    let ca_limit = math_min(pools_ca.len().to_float(), max_per_pair.to_float()).to_int();

    for i in 0..ab_limit {
        let res_ab = get_reserves(pools_ab[i]);
        if res_ab == () { continue; }

        let out_b = calculate_amount_out(amount_in_str, res_ab.r0, res_ab.r1);
        if out_b <= 0.0 { continue; }

        for j in 0..bc_limit {
            let res_bc = get_reserves(pools_bc[j]);
            if res_bc == () { continue; }

            let out_c = calculate_amount_out(out_b.to_string(), res_bc.r0, res_bc.r1);
            if out_c <= 0.0 { continue; }

            for k in 0..ca_limit {
                let res_ca = get_reserves(pools_ca[k]);
                if res_ca == () { continue; }

                let out_a = calculate_amount_out(out_c.to_string(), res_ca.r0, res_ca.r1);
                let profit = out_a - amount_in;

                if profit > best.profit {
                    best.profit = profit;
                    best.pool_ab = pools_ab[i];
                    best.pool_bc = pools_bc[j];
                    best.pool_ca = pools_ca[k];
                    best.amount_out = out_a.to_string();
                }
            }
        }
    }

    best
}

fn calculate_amount_out(amount_in_str, r0_str, r1_str) {
    let amount_in = parse_float(amount_in_str);
    let r0 = parse_float(r0_str);
    let r1 = parse_float(r1_str);

    if r0 <= 0.0 || r1 <= 0.0 || amount_in <= 0.0 { return 0.0; }

    let amount_with_fee = amount_in * 997.0;
    let numerator = amount_with_fee * r1;
    let denominator = r0 * 1000.0 + amount_with_fee;

    if denominator <= 0.0 { return 0.0; }
    numerator / denominator
}

fn estimate_triangle_gas(chain_id) {
    // 3 swaps + overhead
    450000.0
}

fn calculate_triangle_confidence(result, gas_cost) {
    let ratio = result.profit / gas_cost;
    if ratio > 5.0 { 0.9 }
    else if ratio > 3.0 { 0.75 }
    else if ratio > 2.0 { 0.6 }
    else if ratio > 1.3 { 0.4 }
    else { 0.2 }
}

fn parse_float(s) {
    if s == () || s == "" { return 0.0; }
    let result = s.to_float();
    if result == () { 0.0 } else { result }
}

fn on_activate() {
    log_quantum("info", `[chain:${get_chain_id()}] Triangular Arb cartridge ACTIVATED`);
}

fn on_deactivate() {
    log_quantum("info", `[chain:${get_chain_id()}] Triangular Arb cartridge DEACTIVATED`);
}

fn on_new_block(block_number) {
    // No-op
}
