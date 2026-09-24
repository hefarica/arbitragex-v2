// ═══════════════════════════════════════════════════════════════════════════════
// ArbitrageX v2 — FASE OMEGA
// Cartucho Madre: omega_strategy_pack (Polymorphic Alpha Strategy Engine)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Cartucho "Módulo A — Alpha Strategy Engine". Decide QUÉ tipo de oportunidad
// existe. NO arma rutas, NO descubre pools, NO ejecuta. Recibe una ruta candidata
// (pool_data / route_data) que `route_discovery` ya construyó y responde si tiene
// edge neto, despachando al evaluador correcto por `strategy_kind`.
//
// Shadow-safe por construcción: build_payload() devuelve siempre observe_only,
// can_execute=false, capital_exposed=0. Donde falta binding real (Curve exact
// stableswap, Balancer exact math, mempool) el evaluador es fail-honest: si no
// tiene datos suficientes, RECHAZA con `reason` — nunca inventa (RULE 00).
//
// Estrategias PROHIBIDAS por arbx-mev-ethics-gate (sandwich, frontrun, oracle
// manipulation, time-bandit, JIT-displacement, exploit-driven) NO existen aquí:
// no hay rama de despacho para ellas. El backrun es POST-BLOCK (estado confirmado),
// jamás pending-tx → no extractivo.
// ═══════════════════════════════════════════════════════════════════════════════

fn init_strategy() {
    #{
        name: "omega_strategy_pack",
        version: "0.1.0",
        author: "ARBITRAGEX_SUPREME",
        description: "Shadow-safe polymorphic strategy cartridge pack for DEX arbitrage evaluation. Dispatches by strategy_kind over route_discovery pool_data; never builds routes or executes.",
        category: "polymorphic_pack",
        mode: "shadow",
        ethics: "non_predatory",
        capital_exposed: 0,
        target_chains: [],
        supported_protocols: ["uniswap_v2", "uniswap_v3", "sushiswap", "curve", "balancer", "pancakeswap"],
        risk_profile: "low",
        capital_requirement: "variable"
    }
}

// ─── Helpers compartidos ───────────────────────────────────────────────────────

fn reject(reason) {
    #{
        is_opportunity: false,
        estimated_profit: 0.0,
        confidence: 0.0,
        urgency: "none",
        reason: reason
    }
}

fn accept(strategy, net_profit, confidence, urgency, reason) {
    #{
        is_opportunity: true,
        strategy_kind: strategy,
        estimated_profit: net_profit,
        confidence: confidence,
        urgency: urgency,
        reason: reason
    }
}

fn has(m, k) {
    m.contains(k)
}

fn get_num(m, k, fallback) {
    if m.contains(k) {
        m[k]
    } else {
        fallback
    }
}

fn cost_adjusted_profit(gross, gas, fees, slippage, risk_penalty) {
    gross - gas - fees - slippage - risk_penalty
}

fn min_required_profit(pool_data) {
    get_num(pool_data, "min_net_profit_usd", 0.50)
}

fn base_confidence(pool_data, net_profit) {
    let liq = get_num(pool_data, "liquidity_score", 0.50);
    let freshness = get_num(pool_data, "freshness_score", 0.50);
    let sim = get_num(pool_data, "simulation_score", 0.0);

    let profit_score = if net_profit > 10.0 {
        1.0
    } else if net_profit > 3.0 {
        0.75
    } else if net_profit > 1.0 {
        0.55
    } else {
        0.35
    };

    (profit_score * 0.40) + (liq * 0.25) + (freshness * 0.20) + (sim * 0.15)
}

// ─── CONTRATO UNIVERSAL ──────────────────────────────────────────────────────

fn evaluate_opportunity(pool_data) {
    if !has(pool_data, "strategy_kind") {
        return reject("missing_strategy_kind");
    }

    let s = pool_data["strategy_kind"];

    if s == "spatial_cross_dex" { return eval_spatial_cross_dex(pool_data); }
    if s == "triangular_same_dex" { return eval_triangular_same_dex(pool_data); }
    if s == "triangular_cross_dex" { return eval_triangular_cross_dex(pool_data); }
    if s == "stable_depeg" { return eval_stable_depeg(pool_data); }
    if s == "curve_stableswap" { return eval_curve_stableswap(pool_data); }
    if s == "balancer_weighted" { return eval_balancer_weighted(pool_data); }
    if s == "v3_fee_tier" { return eval_v3_fee_tier(pool_data); }
    if s == "v2_v3_cross_invariant" { return eval_v2_v3_cross_invariant(pool_data); }
    if s == "hub_spoke_cycle" { return eval_hub_spoke_cycle(pool_data); }
    if s == "multi_hop_cycle" { return eval_multi_hop_cycle(pool_data); }
    if s == "negative_cycle_bellman_ford" { return eval_negative_cycle(pool_data); }
    if s == "flashloan_atomic" { return eval_flashloan_atomic(pool_data); }
    if s == "flashmint_atomic" { return eval_flashmint_atomic(pool_data); }
    if s == "liquidation_protocol" { return eval_liquidation_protocol(pool_data); }
    if s == "lending_rate_spread" { return eval_lending_rate_spread(pool_data); }
    if s == "oracle_divergence_signal" { return eval_oracle_divergence_signal(pool_data); }
    if s == "cex_dex_convergence" { return eval_cex_dex_convergence(pool_data); }
    if s == "post_block_residual_backrun" { return eval_post_block_residual_backrun(pool_data); }

    reject("unsupported_strategy_kind")
}

fn build_payload(opportunity) {
    #{
        mode: "shadow",
        action: "observe_only",
        can_execute: false,
        capital_exposed: 0,
        opportunity: opportunity
    }
}

// ─── GOLD ────────────────────────────────────────────────────────────────────

fn eval_spatial_cross_dex(d) {
    if !has(d, "buy_quote_usd") || !has(d, "sell_quote_usd") {
        return reject("spatial_missing_quotes");
    }
    let gross = d["sell_quote_usd"] - d["buy_quote_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("spatial_net_profit_below_threshold");
    }
    accept("spatial_cross_dex", net, base_confidence(d, net), "medium", "spatial_cross_dex_net_positive")
}

fn eval_triangular_same_dex(d) {
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("triangular_same_dex_missing_amounts");
    }
    if has(d, "dex_count") && d["dex_count"] != 1 {
        return reject("triangular_same_dex_requires_one_dex");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("triangular_same_dex_not_profitable");
    }
    accept("triangular_same_dex", net, base_confidence(d, net), "medium", "triangular_same_dex_cycle_positive")
}

fn eval_triangular_cross_dex(d) {
    if !has(d, "route_legs") {
        return reject("triangular_cross_dex_missing_route_legs");
    }
    if has(d, "leg_count") && d["leg_count"] != 3 {
        return reject("triangular_cross_dex_requires_3_legs");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("triangular_cross_dex_missing_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("triangular_cross_dex_not_profitable");
    }
    accept("triangular_cross_dex", net, base_confidence(d, net), "high", "triangular_cross_dex_net_positive")
}

fn eval_stable_depeg(d) {
    if !has(d, "token_price_usd") {
        return reject("stable_depeg_missing_price");
    }
    let peg = get_num(d, "target_peg_usd", 1.0);
    let price = d["token_price_usd"];
    let deviation = if price > peg { price - peg } else { peg - price };
    let min_deviation = get_num(d, "min_depeg_pct", 0.002);
    if deviation < min_deviation {
        return reject("stable_depeg_too_small");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("stable_depeg_missing_simulated_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("stable_depeg_net_profit_below_threshold");
    }
    accept("stable_depeg", net, base_confidence(d, net), "high", "stable_depeg_profitable_after_costs")
}

fn eval_curve_stableswap(d) {
    if !has(d, "venue_type") || d["venue_type"] != "curve" {
        return reject("curve_requires_curve_venue");
    }
    if !has(d, "amount_in_usd") {
        return reject("curve_missing_amount_in");
    }
    if !has(d, "amount_out_usd") {
        return reject("curve_missing_stableswap_simulation");
    }
    let imbalance = get_num(d, "pool_imbalance_score", 0.0);
    if imbalance < get_num(d, "min_curve_imbalance_score", 0.05) {
        return reject("curve_imbalance_too_low");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "curve_fee_usd", get_num(d, "dex_fees_usd", 0.0)),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("curve_net_profit_below_threshold");
    }
    accept("curve_stableswap", net, base_confidence(d, net), "medium", "curve_stableswap_shadow_positive")
}

fn eval_balancer_weighted(d) {
    if !has(d, "venue_type") || d["venue_type"] != "balancer" {
        return reject("balancer_requires_balancer_venue");
    }
    if !has(d, "pool_spot_price") || !has(d, "reference_price") {
        return reject("balancer_missing_prices");
    }
    let spot = d["pool_spot_price"];
    let reference = d["reference_price"];
    let diff = if spot > reference { spot - reference } else { reference - spot };
    let min_diff = get_num(d, "min_price_diff_usd", 0.001);
    if diff < min_diff {
        return reject("balancer_price_diff_too_small");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("balancer_missing_simulated_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("balancer_net_profit_below_threshold");
    }
    accept("balancer_weighted", net, base_confidence(d, net), "medium", "balancer_weighted_positive")
}

fn eval_v3_fee_tier(d) {
    if !has(d, "same_pair") || d["same_pair"] != true {
        return reject("v3_fee_tier_requires_same_pair");
    }
    if !has(d, "fee_tier_a") || !has(d, "fee_tier_b") {
        return reject("v3_fee_tier_missing_fee_tiers");
    }
    if d["fee_tier_a"] == d["fee_tier_b"] {
        return reject("v3_fee_tier_requires_different_tiers");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("v3_fee_tier_missing_simulated_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("v3_fee_tier_not_profitable");
    }
    accept("v3_fee_tier", net, base_confidence(d, net), "high", "v3_fee_tier_spread_positive")
}

fn eval_v2_v3_cross_invariant(d) {
    if !has(d, "invariant_a") || !has(d, "invariant_b") {
        return reject("cross_invariant_missing_invariant_types");
    }
    if d["invariant_a"] == d["invariant_b"] {
        return reject("cross_invariant_requires_different_invariants");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("cross_invariant_missing_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("cross_invariant_net_profit_below_threshold");
    }
    accept("v2_v3_cross_invariant", net, base_confidence(d, net), "high", "cross_invariant_positive")
}

fn eval_hub_spoke_cycle(d) {
    if !has(d, "hub_token") {
        return reject("hub_spoke_missing_hub_token");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("hub_spoke_missing_amounts");
    }
    let hub_liq = get_num(d, "hub_liquidity_score", 0.0);
    if hub_liq < 0.60 {
        return reject("hub_spoke_low_hub_liquidity");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("hub_spoke_not_profitable");
    }
    accept("hub_spoke_cycle", net, base_confidence(d, net), "medium", "hub_spoke_positive")
}

// ─── SILVER ───────────────────────────────────────────────────────────────────

fn eval_multi_hop_cycle(d) {
    if !has(d, "leg_count") {
        return reject("multi_hop_missing_leg_count");
    }
    if d["leg_count"] < 4 {
        return reject("multi_hop_requires_at_least_4_legs");
    }
    if d["leg_count"] > get_num(d, "max_legs", 6) {
        return reject("multi_hop_too_many_legs");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("multi_hop_missing_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let gas_penalty = get_num(d, "gas_cost_usd", 0.0) * 1.20;
    let net = cost_adjusted_profit(
        gross,
        gas_penalty,
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("multi_hop_net_profit_below_threshold");
    }
    accept("multi_hop_cycle", net, base_confidence(d, net), "low", "multi_hop_positive_after_extra_gas_penalty")
}

fn eval_negative_cycle(d) {
    if !has(d, "cycle_weight") {
        return reject("negative_cycle_missing_weight");
    }
    if d["cycle_weight"] >= 0.0 {
        return reject("not_a_negative_cycle");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("negative_cycle_missing_simulated_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("negative_cycle_not_profitable_after_costs");
    }
    accept("negative_cycle_bellman_ford", net, base_confidence(d, net), "medium", "negative_cycle_confirmed_by_simulation")
}

fn eval_flashloan_atomic(d) {
    if !has(d, "flashloan_available") || d["flashloan_available"] != true {
        return reject("flashloan_not_available");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("flashloan_missing_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let flash_fee = get_num(d, "flashloan_fee_usd", 0.0);
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0) + flash_fee,
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("flashloan_not_profitable_after_fee");
    }
    accept("flashloan_atomic", net, base_confidence(d, net), "high", "flashloan_atomic_shadow_positive")
}

fn eval_flashmint_atomic(d) {
    if !has(d, "flashmint_available") || d["flashmint_available"] != true {
        return reject("flashmint_not_available");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("flashmint_missing_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "mint_burn_fee_usd", 0.0) + get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("flashmint_not_profitable_after_costs");
    }
    accept("flashmint_atomic", net, base_confidence(d, net), "high", "flashmint_atomic_shadow_positive")
}

fn eval_liquidation_protocol(d) {
    if !has(d, "health_factor") {
        return reject("liquidation_missing_health_factor");
    }
    if d["health_factor"] >= 1.0 {
        return reject("position_not_liquidatable");
    }
    if !has(d, "liquidation_bonus_usd") {
        return reject("liquidation_missing_bonus");
    }
    let gross = d["liquidation_bonus_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "swap_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("liquidation_not_profitable_after_costs");
    }
    accept("liquidation_protocol", net, base_confidence(d, net), "high", "liquidation_protocol_positive")
}

fn eval_lending_rate_spread(d) {
    if !has(d, "supply_apy") || !has(d, "borrow_apy") {
        return reject("lending_missing_rates");
    }
    let spread = d["supply_apy"] - d["borrow_apy"];
    if spread <= get_num(d, "min_apy_spread", 0.02) {
        return reject("lending_spread_too_low");
    }
    let capital = get_num(d, "notional_usd", 0.0);
    let horizon_days = get_num(d, "horizon_days", 1.0);
    if capital <= 0.0 {
        return reject("lending_missing_notional");
    }
    let gross = capital * spread * (horizon_days / 365.0);
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "protocol_fees_usd", 0.0),
        0.0,
        get_num(d, "risk_penalty_usd", 0.0)
    );
    if net <= min_required_profit(d) {
        return reject("lending_spread_not_profitable");
    }
    accept("lending_rate_spread", net, base_confidence(d, net), "low", "lending_rate_spread_positive")
}

fn eval_oracle_divergence_signal(d) {
    if !has(d, "oracle_price") || !has(d, "dex_price") {
        return reject("oracle_divergence_missing_prices");
    }
    let oracle = d["oracle_price"];
    let dex = d["dex_price"];
    let diff = if oracle > dex { oracle - dex } else { dex - oracle };
    let pct = diff / oracle;
    if pct < get_num(d, "min_oracle_divergence_pct", 0.003) {
        return reject("oracle_divergence_too_low");
    }
    // Señal pura: NUNCA ejecuta, NUNCA manipula el oráculo. Alimenta el router.
    #{
        is_opportunity: false,
        strategy_kind: "oracle_divergence_signal",
        estimated_profit: 0.0,
        confidence: 0.65,
        urgency: "medium",
        reason: "oracle_divergence_signal_only_no_execution"
    }
}

fn eval_cex_dex_convergence(d) {
    if !has(d, "cex_price") || !has(d, "dex_price") {
        return reject("cex_dex_missing_prices");
    }
    let cex = d["cex_price"];
    let dex = d["dex_price"];
    let diff = if cex > dex { cex - dex } else { dex - cex };
    let amount = get_num(d, "trade_size_units", 0.0);
    if amount <= 0.0 {
        return reject("cex_dex_missing_trade_size");
    }
    let gross = diff * amount;
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "cex_fees_usd", 0.0) + get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "settlement_risk_penalty_usd", get_num(d, "risk_penalty_usd", 0.0))
    );
    if net <= min_required_profit(d) {
        return reject("cex_dex_not_profitable_after_costs");
    }
    accept("cex_dex_convergence", net, base_confidence(d, net), "medium", "cex_dex_convergence_positive")
}

// ─── Backrun POST-BLOCK (no mempool; estado confirmado; no extractivo) ──────────

fn eval_post_block_residual_backrun(d) {
    if !has(d, "source_event") {
        return reject("post_block_missing_source_event");
    }
    let source = d["source_event"];
    if source != "new_block" && source != "confirmed_swap" && source != "block_swap_log" {
        return reject("post_block_requires_confirmed_state_not_pending_tx");
    }
    if has(d, "pending_tx") && d["pending_tx"] == true {
        return reject("pending_tx_not_supported_by_current_cartridge_surface");
    }
    if !has(d, "pre_state_price") || !has(d, "post_state_price") {
        return reject("post_block_missing_pre_post_state_prices");
    }
    let pre = d["pre_state_price"];
    let post = d["post_state_price"];
    let reference = get_num(d, "reference_price", pre);
    let imbalance = if post > reference { post - reference } else { reference - post };
    let min_imbalance = get_num(d, "min_post_block_imbalance_usd", 0.001);
    if imbalance < min_imbalance {
        return reject("post_block_imbalance_too_small");
    }
    if !has(d, "amount_in_usd") || !has(d, "amount_out_usd") {
        return reject("post_block_missing_simulated_rebalance_amounts");
    }
    let gross = d["amount_out_usd"] - d["amount_in_usd"];
    let net = cost_adjusted_profit(
        gross,
        get_num(d, "gas_cost_usd", 0.0),
        get_num(d, "dex_fees_usd", 0.0),
        get_num(d, "slippage_cost_usd", 0.0),
        get_num(d, "mev_competition_penalty_usd", get_num(d, "risk_penalty_usd", 0.0))
    );
    if net <= min_required_profit(d) {
        return reject("post_block_residual_not_profitable");
    }
    accept("post_block_residual_backrun", net, base_confidence(d, net), "high", "post_block_residual_imbalance_positive")
}
