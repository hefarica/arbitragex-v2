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

    // Evaluar el mejor ciclo triangular (orientado por direcciones reales)
    let amount_in = pool_data.amount_in;
    let best_result = find_best_triangle(
        pools_ab, pools_bc, pools_ca,
        amount_in,
        meta_a.decimals, meta_b.decimals, meta_c.decimals,
        token_a, token_b, token_c
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

    // ── Gas en USD (RHAI-08 fix): el profit ya está en token humano; el gas se
    // paga en el token NATIVO — se pricing con su precio USD, nunca en wei ni
    // en ETH crudo contra token units.
    let base_fee = get_base_fee();
    let gas_native = estimate_triangle_gas(chain_id) * base_fee / 1000000000.0;
    let native_gas_symbol = if chain_id == 1 || chain_id == 10 || chain_id == 8453 || chain_id == 42161 {
        "WETH"
    } else if chain_id == 137 {
        "WMATIC"
    } else if chain_id == 56 {
        "WBNB"
    } else {
        ""
    };
    let native_usd = if native_gas_symbol == "" { () } else { get_token_price_usd(native_gas_symbol) };
    if native_usd == () || native_usd <= 0.0 {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "triangle_no_gas_price"
        };
    }
    let gas_usd = gas_native * native_usd;

    // Precio USD del token base del ciclo (el binding resuelve addr→símbolo).
    let base_usd = get_token_price_usd(token_a);
    if base_usd == () || base_usd <= 0.0 {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "triangle_no_base_price"
        };
    }
    let profit_usd = best_result.profit * base_usd;

    if profit_usd <= gas_usd * 1.3 {
        return #{
            is_opportunity: false,
            estimated_profit: best_result.profit,
            profit_usd_hint: profit_usd,
            confidence: 0.0,
            urgency: "none",
            reason: "below_gas_threshold",
            gas_cost_usd: gas_usd
        };
    }

    let net_usd = profit_usd - gas_usd;
    let confidence = calculate_triangle_confidence(net_usd, gas_usd);
    let urgency = if confidence > 0.8 { "immediate" } else if confidence > 0.5 { "next_block" } else { "monitor" };

    log_quantum("info", `[chain:${chain_id}] Triangle arb: ${meta_a.symbol}→${meta_b.symbol}→${meta_c.symbol}→${meta_a.symbol} net_usd=${net_usd}`);

    #{
        is_opportunity: true,
        estimated_profit: best_result.profit,
        profit_usd_hint: net_usd,
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
        gas_cost_usd: gas_usd,
        net_profit_usd: net_usd,
        quote_method: "v2_chained_roundtrip_oriented"
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

// find_best_triangle (RHAI-08/16 fix 2026-09-24):
//  · Reservas ORIENTADAS por token0_addr de cada pool con las DIRECCIONES
//    reales A/B/C del ciclo (antes r0 era SIEMPRE reserve_in — el 50% de las
//    orientaciones cotizaba el pool invertido).
//  · Profit normalizado a token humano con los decimales del token base
//    (antes quedaba en wei y el gate comparaba contra gas en ETH).
fn find_best_triangle(pools_ab, pools_bc, pools_ca, amount_in_str, dec_a, dec_b, dec_c, token_a, token_b, token_c) {
    let best = #{ profit: 0.0, pool_ab: "", pool_bc: "", pool_ca: "", amount_out: "0" };

    let max_per_pair = 3;
    let ab_limit = math_min(pools_ab.len().to_float(), max_per_pair.to_float()).to_int();
    let bc_limit = math_min(pools_bc.len().to_float(), max_per_pair.to_float()).to_int();
    let ca_limit = math_min(pools_ca.len().to_float(), max_per_pair.to_float()).to_int();
    let base_unit_a = math_pow(10.0, dec_a.to_float());

    for i in 0..ab_limit {
        let res_ab = get_reserves(pools_ab[i]);
        if res_ab == () { continue; }

        // Hop 1: A→B (token_in = token_a)
        let or_ab = oriented_reserves(res_ab, token_a);
        let ri_ab = or_ab[0];
        let ro_ab = or_ab[1];
        let out_b_raw = calculate_amount_out(amount_in_str, ri_ab.to_string(), ro_ab.to_string());
        if out_b_raw <= 0.0 { continue; }

        for j in 0..bc_limit {
            let res_bc = get_reserves(pools_bc[j]);
            if res_bc == () { continue; }

            // Hop 2: B→C (token_in = token_b)
            let or_bc = oriented_reserves(res_bc, token_b);
            let ri_bc = or_bc[0];
            let ro_bc = or_bc[1];
            let out_c_raw = calculate_amount_out(out_b_raw.to_string(), ri_bc.to_string(), ro_bc.to_string());
            if out_c_raw <= 0.0 { continue; }

            for k in 0..ca_limit {
                let res_ca = get_reserves(pools_ca[k]);
                if res_ca == () { continue; }

                // Hop 3: C→A (token_in = token_c)
                let or_ca = oriented_reserves(res_ca, token_c);
                let ri_ca = or_ca[0];
                let ro_ca = or_ca[1];
                let out_a_raw = calculate_amount_out(out_c_raw.to_string(), ri_ca.to_string(), ro_ca.to_string());
                if out_a_raw <= 0.0 { continue; }

                // Profit en token humano (unidades del token base A).
                let profit_human = (out_a_raw - parse_float(amount_in_str)) / base_unit_a;

                if profit_human > best.profit {
                    best.profit = profit_human;
                    best.pool_ab = pools_ab[i];
                    best.pool_bc = pools_bc[j];
                    best.pool_ca = pools_ca[k];
                    best.amount_out = out_a_raw.to_string();
                }
            }
        }
    }

    best
}

/// (reserve_in, reserve_out) para un swap cuyo token_in es `swap_token_in`,
/// orientado por el token0_addr real del pool. Sin token0_addr se asume que
/// token_in es token0 (convención del pool_index — best-effort documentado).
fn oriented_reserves(res, swap_token_in) {
    if res.token0_addr != () && swap_token_in != res.token0_addr {
        return (res.r1, res.r0);
    }
    (res.r0, res.r1)
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

fn calculate_triangle_confidence(net_usd, gas_usd) {
    let ratio = net_usd / gas_usd;
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
