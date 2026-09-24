// ═══════════════════════════════════════════════════════════════════════════════
// ArbitrageX v2 — FASE OMEGA
// Cartucho: Backrun No-Extractivo (Zero Victim)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Estrategia de backrunning ÉTICO. A diferencia del sandwich (PROHIBIDO por
// arbx-mev-ethics-gate), el backrun no-extractivo NO toca la transacción del
// usuario ni la reordena: actúa ESTRICTAMENTE sobre el estado YA CONFIRMADO,
// después de que el swap del usuario ha sido liquidado on-chain. Captura el
// arbitraje residual (el desequilibrio que el swap dejó en el pool) y re-nivela
// el pool hacia su precio justo. El usuario ya recibió su precio; cero víctima.
//
// GUARDRAIL ÉTICO (Decisión #1 del gate): si el evento NO es "confirmed_block"
// (p.ej. una tx pendiente en mempool), el cartucho SE NIEGA — leer una tx
// pendiente específica para anticiparse es frontrun/sandwich, PROHIBIDO.
//
// Chain-agnostic: el host inyecta chain_id, decimales y precio de referencia.
// El cartucho NUNCA hardcodea direcciones ni parámetros de cadena.
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// CONTRATO UNIVERSAL: init_strategy()
// ─────────────────────────────────────────────────────────────────────────────
fn init_strategy() {
    #{
        name: "Backrun No-Extractive (Zero Victim)",
        version: "1.0.0",
        author: "ArbitrageX Core Team",
        description: "Non-extractive residual backrun. Acts ONLY on confirmed post-state imbalance after a user's swap has settled on-chain; never reads a pending tx, never reorders or harms the originator. Re-levels the pool toward fair price and captures the residual spread. PERMITTED by arbx-mev-ethics-gate (#2 backrun, zero-victim).",
        category: "backrun",
        target_chains: [],            // Empty = ALL chains supported
        min_eval_interval_ms: 50,
        supported_protocols: ["uniswap_v2", "sushiswap", "pancakeswap", "camelot", "aerodrome"],
        risk_profile: "low",
        capital_requirement: "variable",
        ethics: "non_extractive_zero_victim"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CONTRATO UNIVERSAL: evaluate_opportunity(pool_data)
// ─────────────────────────────────────────────────────────────────────────────
//
// pool_data (inyectado por el host):
// #{
//     chain_id, source_pool, token_in, token_out,
//     amount_in,             // tamaño del swap CONFIRMADO del usuario (wei string)
//     reserves_post: #{ r0, r1, block, ts },  // reservas DESPUÉS del swap confirmado
//     fair_price,            // precio de referencia justo (pre-swap / venue externo)
//     confirmed_tx_hash,     // hash de la tx YA CONFIRMADA tras la cual hacemos backrun
//     source_event,          // DEBE ser "confirmed_block" — guardrail anti-frontrun
//     protocol_type, gas_price_gwei, block_number
// }
fn evaluate_opportunity(pool_data) {
    let chain_id = get_chain_id();

    // ── GUARDRAIL ÉTICO: solo post-estado CONFIRMADO (zero-victim) ──────────
    // Si el evento no es un bloque confirmado, NOS NEGAMOS: anticiparse a una
    // tx pendiente sería frontrun/sandwich (PROHIBIDO por arbx-mev-ethics-gate).
    let source_event = pool_data.source_event;
    if source_event != "confirmed_block" {
        return reject("not_confirmed_zero_victim_guard");
    }

    let post = pool_data.reserves_post;
    if post == () {
        return reject("post_reserves_unavailable");
    }

    let meta_in = get_token_meta(pool_data.token_in);
    let meta_out = get_token_meta(pool_data.token_out);
    if meta_in == () || meta_out == () {
        return reject("token_meta_unavailable");
    }
    let d_in = meta_in.decimals;
    let d_out = meta_out.decimals;

    // ── Precio del pool DESPUÉS del swap confirmado vs precio justo ─────────
    let price_post = calc_price(post.r0, post.r1, d_in, d_out);
    let fair_price = pool_data.fair_price;
    if price_post <= 0.0 || fair_price <= 0.0 {
        return reject("price_unavailable");
    }

    // Desequilibrio residual dejado por el swap confirmado.
    let imbalance_pct = math_abs((price_post - fair_price) / fair_price) * 100.0;

    // ── Capturable: acotado por el TAMAÑO del swap que causó el desequilibrio ──
    // No se captura sobre todo el pool (irreal): el backrun re-nivela en proporción
    // al swap observado. capture_eff < 1 cubre fees + decoherencia de estado.
    let swap_size = from_wei(pool_data.amount_in, d_in);
    let capture_eff = 0.5;
    let gross = swap_size * (imbalance_pct / 100.0) * capture_eff;

    // ── Net profit gas-aware (arbx-net-profit-gate, shadow) ─────────────────
    let base_fee = get_base_fee();
    let gas_units = 180000.0;                       // backrun single-leg rebalance
    let gas_cost = gas_units * base_fee / 1000000000.0;
    let net = gross - gas_cost;
    let min_net = gas_cost * 0.2;                    // 20% margen sobre gas

    if net <= min_net {
        return #{
            is_opportunity: false,
            estimated_profit: gross,
            net_profit: net,
            confidence: 0.0,
            urgency: "none",
            reason: "below_gas_threshold",
            gas_cost: gas_cost,
            imbalance_pct: imbalance_pct
        };
    }

    let confidence = calc_conf(imbalance_pct, net, gas_cost);
    let urgency = if confidence > 0.8 { "immediate" } else { "next_block" };

    log_quantum("info", `[chain:${chain_id}] backrun residual imbalance=${imbalance_pct}% net=${net} conf=${confidence} mode=shadow`);

    #{
        is_opportunity: true,
        estimated_profit: gross,
        net_profit: net,
        confidence: confidence,
        urgency: urgency,
        reason: "post_state_rebalance",
        imbalance_pct: imbalance_pct,
        source_pool: pool_data.source_pool,
        token_in: pool_data.token_in,
        token_out: pool_data.token_out,
        target_tx_hash: pool_data.confirmed_tx_hash,
        gas_cost_estimate: gas_cost,
        chain_id: chain_id,
        protocol_type: pool_data.protocol_type,
        route_type: "backrun_rebalance"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CONTRATO UNIVERSAL: build_payload(opportunity)
// ─────────────────────────────────────────────────────────────────────────────
//
// El payload pide al ensamblador colocar NUESTRA tx INMEDIATAMENTE DESPUÉS de la
// tx ya confirmada (bundle_type "backrun", ordering "after_target"). Lleva una
// atestación zero-victim: el executor downstream debe descartar el bundle si la
// simulación detecta cualquier impacto sobre el originador.
fn build_payload(opportunity) {
    let chain_id = get_chain_id();
    let timestamp = get_timestamp();

    #{
        target_contract: "EXECUTOR",     // resuelto por el executor layer por chain
        calldata: "0x",                  // el executor ensambla el calldata real
        value_wei: "0",
        gas_limit: 220000,

        // Instrucción de empaquetado para el ensamblador
        bundle: #{
            bundle_type: "backrun",
            target_tx_hash: opportunity.target_tx_hash,
            ordering: "after_target"     // nuestra tx va DESPUÉS de la confirmada
        },

        // Parámetros de riesgo
        risk: #{
            max_slippage_bps: 30,
            min_profit_after_gas: opportunity.net_profit * 0.8,
            revert_protection: true,
            zero_victim_attestation: true   // no-extractivo: solo estado liquidado
        },

        meta: #{
            cartridge: "backrun_no_extractive",
            version: "1.0.0",
            chain_id: chain_id,
            generated_at: timestamp,
            route_type: opportunity.route_type
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LIFECYCLE HOOKS (Opcionales)
// ─────────────────────────────────────────────────────────────────────────────
fn on_activate() {
    let chain_id = get_chain_id();
    log_quantum("info", `[chain:${chain_id}] Backrun No-Extractive cartridge ACTIVATED`);
    emit_signal("cartridge_lifecycle", #{
        event: "activated",
        cartridge: "backrun_no_extractive",
        chain_id: chain_id
    });
}

fn on_deactivate() {
    let chain_id = get_chain_id();
    log_quantum("info", `[chain:${chain_id}] Backrun No-Extractive cartridge DEACTIVATED`);
}

fn on_new_block(block_number) {
    // No-op: este cartucho es event-driven (reacciona a swaps confirmados).
}

// ═══════════════════════════════════════════════════════════════════════════════
// FUNCIONES AUXILIARES (pure, chain-agnostic)
// ═══════════════════════════════════════════════════════════════════════════════

/// Map de rechazo estándar (R8 fail-honest: razón explícita, sin fabricar profit).
fn reject(reason) {
    #{
        is_opportunity: false,
        estimated_profit: 0.0,
        net_profit: 0.0,
        confidence: 0.0,
        urgency: "none",
        reason: reason
    }
}

/// Precio spot V2 normalizado por decimales: token1 por token0.
fn calc_price(r0_str, r1_str, d0, d1) {
    let a0 = from_wei(r0_str, d0);
    let a1 = from_wei(r1_str, d1);
    if a0 <= 0.0 {
        return 0.0;
    }
    a1 / a0
}

/// Confianza basada en magnitud del desequilibrio y ratio net/gas.
fn calc_conf(imbalance_pct, net, gas_cost) {
    let c = 0.0;
    if imbalance_pct > 3.0 {
        c += 0.5;
    } else if imbalance_pct > 1.0 {
        c += 0.3;
    } else {
        c += 0.1;
    }

    let ratio = if gas_cost > 0.0 { net / gas_cost } else { 0.0 };
    if ratio > 50.0 {
        c += 0.4;
    } else if ratio > 10.0 {
        c += 0.3;
    } else {
        c += 0.1;
    }

    if c > 1.0 { c = 1.0; }
    if c < 0.0 { c = 0.0; }
    c
}
