// ═══════════════════════════════════════════════════════════════════════════════
// ArbitrageX v2 — FASE OMEGA
// Cartucho: Liquidation Monitor (Multi-Chain Universal)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Monitorea posiciones de lending protocols (Aave V3, Compound V2/V3) y
// detecta oportunidades de liquidación rentables. Chain-agnostic.
//
// ═══════════════════════════════════════════════════════════════════════════════

fn init_strategy() {
    #{
        name: "Liquidation Monitor Universal",
        version: "2.0.0",
        author: "ArbitrageX Core Team",
        description: "Multi-chain liquidation opportunity detector for Aave V3 and Compound V2/V3. Monitors health factors and identifies profitable liquidation targets with flash-loan capital sourcing.",
        category: "liquidation",
        target_chains: [],  // ALL chains with lending protocols
        min_eval_interval_ms: 200,
        supported_protocols: ["aave_v3", "compound_v2", "compound_v3"],
        risk_profile: "medium",
        capital_requirement: "high_flash_loan"
    }
}

/// Evalúa si una posición es liquidable y rentable.
///
/// pool_data (en este contexto, position_data):
/// #{
///     chain_id: N,
///     protocol: "aave_v3" | "compound_v2" | "compound_v3",
///     borrower: "0x...",
///     health_factor: 0.95,        // < 1.0 = liquidable
///     debt_token: "0x...",
///     debt_amount: "...",
///     collateral_token: "0x...",
///     collateral_amount: "...",
///     liquidation_bonus_bps: 500, // 5% bonus
///     close_factor_bps: 5000      // 50% max liquidation
/// }
fn evaluate_opportunity(pool_data) {
    let chain_id = get_chain_id();
    let health_factor = pool_data.health_factor;

    // Solo liquidable si health factor < 1.0
    if health_factor >= 1.0 {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "position_healthy"
        };
    }

    let debt_token = pool_data.debt_token;
    let collateral_token = pool_data.collateral_token;
    let debt_amount_str = pool_data.debt_amount;
    let collateral_amount_str = pool_data.collateral_amount;
    let liquidation_bonus_bps = pool_data.liquidation_bonus_bps;
    let close_factor_bps = pool_data.close_factor_bps;

    // Obtener metadata de tokens
    let debt_meta = get_token_meta(debt_token);
    let collateral_meta = get_token_meta(collateral_token);

    if debt_meta == () || collateral_meta == () {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "token_meta_unavailable"
        };
    }

    // Calcular monto máximo liquidable (unidades humanas del debt token)
    let debt_amount = from_wei(debt_amount_str, debt_meta.decimals);
    let max_liquidatable = debt_amount * (close_factor_bps.to_float() / 10000.0);

    // ── RHAI-17 fix (2026-09-24): el bonus se recibe en COLLATERAL por el
    // valor repagado. Antes: gross = max_liquidatable(debt units) × bonus —
    // mezclaba deuda/colateral/nativo. Ahora todo se valora en USD con el
    // precio de CADA token (el binding resuelve addr→símbolo canónico).
    let debt_usd_px = get_token_price_usd(debt_token);
    let coll_usd_px = get_token_price_usd(collateral_token);
    if debt_usd_px == () || debt_usd_px <= 0.0 || coll_usd_px == () || coll_usd_px <= 0.0 {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "liquidation_token_price_unavailable"
        };
    }
    // Valor repagado (debt) y collateral recibido por ese repago (con bonus).
    let repaid_usd = max_liquidatable * debt_usd_px;
    let bonus_pct = liquidation_bonus_bps.to_float() / 10000.0;
    let collateral_received_usd = repaid_usd * (1.0 + bonus_pct);

    // Costes en USD: gas en NATIVO × precio nativo (no en debt units).
    let base_fee = get_base_fee();
    let gas_native = estimate_liquidation_gas(pool_data.protocol, chain_id) * base_fee / 1000000000.0;
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
            reason: "liquidation_no_gas_price"
        };
    }
    let gas_usd = gas_native * native_usd;

    // Swap del collateral recibido a debt token para repagar el flash loan:
    // fee aproximado sobre el VALOR USD del collateral vendido.
    let swap_cost_bps = 30.0; // 0.3% swap fee
    let swap_cost_usd = collateral_received_usd * (swap_cost_bps / 10000.0);

    // Neto en USD (única frontera económica): colateral recibido − deuda
    // repagada − gas − swap. Gross profit = solo el bonus (lo que excede el
    // repago) menos el costo de conversión.
    let gross_profit_usd = collateral_received_usd - repaid_usd;
    let net_profit_usd = gross_profit_usd - gas_usd - swap_cost_usd;

    if net_profit_usd <= 0.0 {
        return #{
            is_opportunity: false,
            estimated_profit: net_profit_usd,
            confidence: 0.0,
            urgency: "none",
            reason: "unprofitable_after_costs",
            gas_cost_usd: gas_usd,
            swap_cost_usd: swap_cost_usd
        };
    }

    // Confianza basada en health factor y profit (USD, dimensionalmente consistente)
    let confidence = calculate_liq_confidence(health_factor, net_profit_usd, gas_usd);
    let urgency = if health_factor < 0.8 { "immediate" } else if health_factor < 0.95 { "next_block" } else { "monitor" };

    log_quantum("info", `[chain:${chain_id}] Liquidation: borrower=${pool_data.borrower}, HF=${health_factor}, net_usd=${net_profit_usd}`);

    #{
        is_opportunity: true,
        estimated_profit: net_profit_usd,
        profit_usd_hint: net_profit_usd,
        confidence: confidence,
        urgency: urgency,
        chain_id: chain_id,
        protocol: pool_data.protocol,
        borrower: pool_data.borrower,
        health_factor: health_factor,
        debt_token: debt_token,
        collateral_token: collateral_token,
        max_liquidatable: max_liquidatable,
        debt_decimals: debt_meta.decimals,
        collateral_decimals: collateral_meta.decimals,
        liquidation_bonus_pct: bonus_pct,
        gas_cost_usd: gas_usd,
        swap_cost_usd: swap_cost_usd,
        route_type: "liquidation"
    }
}

fn build_payload(opportunity) {
    let chain_id = get_chain_id();
    let timestamp = get_timestamp();
    let base_fee = get_base_fee();

    #{
        target_contract: "EXECUTOR",
        calldata: "0x_liquidation_placeholder",
        value_wei: "0",
        gas_limit: 650000,
        max_priority_fee_gwei: base_fee * 2.0,
        deadline_ts: timestamp + 24,  // ~2 blocks on most chains
        flash_loan: #{
            protocol: "auto",
            token: opportunity.debt_token,
            // RHAI-09 fix (2026-09-24): usar los decimales REALES del debt
            // token (antes to_wei(..., 18) fijo — para USDC sobrepréstamo 1e12×).
            amount: to_wei(opportunity.max_liquidatable, opportunity.debt_decimals)
        },
        route: #{
            type: "liquidation",
            protocol: opportunity.protocol,
            borrower: opportunity.borrower,
            debt_token: opportunity.debt_token,
            collateral_token: opportunity.collateral_token,
            amount: opportunity.max_liquidatable
        },
        risk: #{
            max_slippage_bps: 100,
            min_profit_after_gas: opportunity.estimated_profit * 0.7,
            revert_protection: true
        },
        meta: #{
            cartridge: "liquidation_universal",
            version: "2.0.0",
            chain_id: chain_id
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn estimate_liquidation_gas(protocol, chain_id) {
    // Liquidation + flash loan + swap = ~600k gas
    if protocol == "aave_v3" {
        550000.0
    } else if protocol == "compound_v3" {
        480000.0
    } else {
        600000.0
    }
}

fn calculate_liq_confidence(health_factor, net_profit, gas_cost) {
    let conf = 0.0;

    // Health factor proximity to liquidation
    if health_factor < 0.7 { conf += 0.4; }
    else if health_factor < 0.85 { conf += 0.3; }
    else if health_factor < 0.95 { conf += 0.2; }
    else { conf += 0.1; }

    // Profit ratio
    let ratio = net_profit / gas_cost;
    if ratio > 5.0 { conf += 0.4; }
    else if ratio > 3.0 { conf += 0.3; }
    else if ratio > 1.5 { conf += 0.2; }
    else { conf += 0.1; }

    // Freshness bonus
    conf += 0.2;

    if conf > 1.0 { conf = 1.0; }
    conf
}

fn on_activate() {
    log_quantum("info", `[chain:${get_chain_id()}] Liquidation Monitor cartridge ACTIVATED`);
}

fn on_deactivate() {
    log_quantum("info", `[chain:${get_chain_id()}] Liquidation Monitor cartridge DEACTIVATED`);
}

fn on_new_block(block_number) {
    // Could refresh health factors here in future versions
}
