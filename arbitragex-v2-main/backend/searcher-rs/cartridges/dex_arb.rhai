// ═══════════════════════════════════════════════════════════════════════════════
// ArbitrageX v2 — FASE OMEGA
// Cartucho Maestro: DEX Arbitrage (Multi-Chain Universal)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Este cartucho implementa detección de arbitraje entre pools DEX (V2/V3)
// de forma completamente agnóstica a la cadena. Funciona en cualquier chain
// que el sistema soporte: Ethereum, Arbitrum, Base, Polygon, BSC, Avalanche,
// Optimism, o cualquier EVM-compatible que se agregue en el futuro.
//
// El chain_id se inyecta automáticamente por el host runtime — el cartucho
// NUNCA hardcodea direcciones ni parámetros específicos de una cadena.
//
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// CONTRATO UNIVERSAL: init_strategy()
// ─────────────────────────────────────────────────────────────────────────────

/// Retorna metadata del cartucho. Llamado una vez al cargar.
/// target_chains vacío = compatible con TODAS las chains.
fn init_strategy() {
    #{
        name: "DEX Arbitrage Universal",
        version: "2.0.0",
        author: "ArbitrageX Core Team",
        description: "Multi-chain DEX arbitrage detector. Identifies price discrepancies between V2/V3 pools across any supported chain. Supports 2-leg and 3-leg routes with dynamic sizing via constant-product and concentrated-liquidity math.",
        category: "dex_arb",
        target_chains: [],  // Empty = ALL chains supported
        min_eval_interval_ms: 50,
        supported_protocols: ["uniswap_v2", "uniswap_v3", "sushiswap", "pancakeswap", "camelot", "aerodrome", "velodrome", "quickswap", "trader_joe"],
        risk_profile: "low",
        capital_requirement: "variable"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CONTRATO UNIVERSAL: evaluate_opportunity(pool_data)
// ─────────────────────────────────────────────────────────────────────────────

/// Evalúa si existe una oportunidad de arbitraje en los datos de pool recibidos.
///
/// pool_data es un Map inyectado por el host con la siguiente estructura:
/// #{
///     chain_id: 1,                    // Inyectado por host
///     source_pool: "0x...",           // Pool donde se detectó el swap
///     token_in: "0x...",              // Token de entrada del swap observado
///     token_out: "0x...",             // Token de salida del swap observado
///     amount_in: "1000000000",        // Amount del swap observado (wei string)
///     reserves_source: #{             // Reservas del pool fuente
///         r0: "...", r1: "...", block: N, ts: N
///     },
///     protocol_type: "v2" | "v3",     // Protocolo del pool fuente
///     gas_price_gwei: 30.0,           // Gas price actual
///     block_number: 12345678          // Bloque actual
/// }
fn evaluate_opportunity(pool_data) {
    // ── Extraer datos del contexto (chain-agnostic) ─────────────────────
    let chain_id = get_chain_id();
    let token_in = pool_data.token_in;
    let token_out = pool_data.token_out;
    let source_pool = pool_data.source_pool;
    let amount_in_str = pool_data.amount_in;
    let protocol_type = pool_data.protocol_type;

    // ── Obtener metadata de tokens (resuelto por host según chain_id) ───
    let meta_in = get_token_meta(token_in);
    let meta_out = get_token_meta(token_out);

    if meta_in == () || meta_out == () {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "token_meta_unavailable"
        };
    }

    let decimals_in = meta_in.decimals;
    let decimals_out = meta_out.decimals;

    // ── BRANCH V3 (ANTES del gate insufficient_pools) ───────────────────
    // V3 no usa el índice de pools V2 ni reservas r0/r1: su precio vive en slot0
    // (sqrtPriceX96). Se evalúa aquí para emitir telemetría shadow honesta del precio
    // V3 SIN pasar por el gate `insufficient_pools` (que es para arb V2 cross-DEX y
    // bloqueaba esta rama). NO ejecuta sizing V2, NO fabrica profit, NO emite oportunidad.
    if protocol_type == "v3" {
        let slot0 = get_v3_slot0(source_pool);
        if slot0 == () {
            return #{
                is_opportunity: false,
                estimated_profit: 0.0,
                confidence: 0.0,
                urgency: "none",
                reason: "v3_slot0_unavailable"
            };
        }
        let source_price = calc_v3_spot_price(slot0.sqrt_price_x96, decimals_in, decimals_out);
        // Telemetría shadow REAL → arbx:cartridge:telemetry (log_quantum) → api-server WS → UI.
        // El sizing V3 (amount_out vía QuoterV2 / liquidez por tick) es la siguiente capa.
        log_quantum("info", `v3_source_priced cartridge=dex_arb chain=${chain_id} source_pool=${source_pool} sqrt_price_x96=${slot0.sqrt_price_x96} liquidity=${slot0.liquidity} source_price=${source_price} mode=shadow`);
        if pool_data.fee_bps == () {
            return #{
                is_opportunity: false,
                estimated_profit: 0.0,
                confidence: 0.0,
                reason: "v3_no_fee_tier",
                source_price: source_price,
                source_pool: source_pool,
                protocol_type: "v3",
                mode: "shadow"
            };
        }
        if slot0.liquidity == "" || slot0.liquidity == "0" {
            return #{
                is_opportunity: false,
                estimated_profit: 0.0,
                confidence: 0.0,
                reason: "v3_no_liquidity",
                source_price: source_price,
                source_pool: source_pool,
                protocol_type: "v3",
                mode: "shadow"
            };
        }
        // hex address order = token0 is the smaller address (Uniswap convention)
        let zero_for_one = token_in < token_out;
        let est_out = v3_amount_out_single_tick(amount_in_str, slot0.sqrt_price_x96, slot0.liquidity, pool_data.fee_bps, zero_for_one);
        if est_out == () {
            return #{
                is_opportunity: false,
                estimated_profit: 0.0,
                confidence: 0.0,
                reason: "v3_single_tick_degenerate",
                source_price: source_price,
                source_pool: source_pool,
                protocol_type: "v3",
                mode: "shadow"
            };
        }
        // The honest is_opportunity:false single-tick candidate (emitted when the cross-pool arb
        // path is gated OFF, or when it finds no profitable round-trip). single-tick is an UPPER
        // BOUND on real V3 output -> confidence capped 0.3, single_tick_unconfirmed:true.
        let single_tick_candidate = #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.3,
            reason: "v3_single_tick_candidate",
            estimated_amount_out: est_out,
            v3_sizing_method: "single_tick_upper_bound",
            single_tick_unconfirmed: true,
            source_price: source_price,
            source_pool: source_pool,
            protocol_type: "v3",
            mode: "shadow"
        };

        // ── GATE: cross-pool round-trip arb emission (is_opportunity:true path) ──────────
        // Default OFF. Until an operator sets ARBX_V3_ARB_MODE=on, emit ONLY the honest
        // is_opportunity:false candidate above (unchanged behaviour). Shadow/paper, capital=0.
        if !v3_arb_enabled() {
            return single_tick_candidate;
        }

        // ── CROSS-POOL ROUND-TRIP ARB (V3 source pool × V2 alt pools, same token pair) ──
        // Council-mandated 2-leg round-trip in ONE token (token_in), per protocol-correct curve:
        //   V3 leg -> v3_amount_out_single_tick on the source slot0 (UPPER BOUND, hence conf 0.3);
        //   V2 leg -> v2_amount_out_str on the alt pool's reserves.
        // NEVER the V2 path's buggy `out_alt - out_source` parallel-forward subtraction.
        // get_pool_index returns V2 pools (the index is built UNISWAP_V2-only), so each alt leg is
        // priced with V2 math and the source leg with V3 math — partitioned by protocol (council C5).
        let alt_pools = get_pool_index(meta_in.symbol, meta_out.symbol);

        // USD price of token_in — REQUIRED to express net profit. RULE 00: fail honestly, never
        // fabricate a price, if the oracle has no fresh value.
        let price_in_usd = get_token_price_usd(meta_in.symbol);
        if price_in_usd == () || price_in_usd <= 0.0 {
            return #{
                is_opportunity: false,
                estimated_profit: 0.0,
                confidence: 0.0,
                reason: "v3_arb_no_price",
                source_price: source_price,
                source_pool: source_pool,
                protocol_type: "v3",
                mode: "shadow"
            };
        }

        // V2 alt-leg fee = 0.30% (30 bps) — the standard constant-product fee the index pools use.
        let v2_fee_bps = 30;
        // zero_for_one for the V3 source pool, per leg's own token order (council derivation):
        let zfo_in_to_out = token_in < token_out;   // swapping token_in -> token_out on V3 source
        let zfo_out_to_in = token_out < token_in;   // swapping token_out -> token_in on V3 source

        let best_net_usd = ();          // () = no round-trip computed yet
        let best_final_in = "";         // final token_in wei of the winning round-trip
        let best_alt_pool = "";
        let best_direction = "";

        // gas_usd: gas is paid in the chain's NATIVE token (ETH/MATIC/BNB), NOT token_in.
        // Council CRITICAL fix: price gas-in-native by the NATIVE gas token's USD price, not
        // token_in's — multiplying ETH-cost by token_in's price is only correct when token_in==WETH
        // (under-charges gas ~3000x for stables → false-positive viable). RULE 00: fail honestly
        // (v3_arb_no_gas_price) if the native price is missing; never substitute a wrong price.
        let base_fee = get_base_fee();
        let gas_units = estimate_gas_cost("v3", chain_id);
        let gas_cost_native = gas_units * base_fee / 1000000000.0; // gas × gwei / 1e9 = native token (ETH)
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
                reason: "v3_arb_no_gas_price",
                source_price: source_price,
                source_pool: source_pool,
                protocol_type: "v3",
                mode: "shadow"
            };
        }
        let gas_usd = gas_cost_native * native_usd;

        let amount_in_f = from_wei(amount_in_str, decimals_in);
        let block_number = get_block_number();
        for alt_pool in alt_pools {
            if alt_pool == source_pool {
                continue;
            }
            // v3_roundtrip_for_alt does all per-alt work (reserves, freshness, both directions) and
            // returns either () (uncomputable) or #{ net_usd, final_in, direction }. Extracted to a
            // helper to keep evaluate_opportunity's expression-nesting under the Rhai depth limit.
            let rt = v3_roundtrip_for_alt(alt_pool, source_pool, token_in, token_out,
                amount_in_str, amount_in_f, slot0, pool_data.fee_bps,
                zfo_in_to_out, zfo_out_to_in, v2_fee_bps, price_in_usd, gas_usd,
                decimals_in, block_number);
            if rt == () {
                continue;
            }
            if best_net_usd == () || rt.net_usd > best_net_usd {
                best_net_usd = rt.net_usd;
                best_final_in = rt.final_in;
                best_alt_pool = alt_pool;
                best_direction = rt.direction;
            }
        }

        // No alt-pool yielded a computable round-trip -> keep the honest single-tick candidate.
        if best_net_usd == () {
            return single_tick_candidate;
        }

        // A real arb at this probe amount_in: net_usd > 0 (computed from slot0/reserves, RULE 00).
        if best_net_usd > 0.0 {
            log_quantum("info", `v3_arb_viable cartridge=dex_arb chain=${chain_id} source_pool=${source_pool} alt_pool=${best_alt_pool} dir=${best_direction} net_usd=${best_net_usd} mode=shadow`);
            return #{
                is_opportunity: true,
                estimated_profit: best_net_usd,
                estimated_amount_out: best_final_in,
                confidence: 0.3,
                single_tick_unconfirmed: true,
                v3_sizing_method: "single_tick_upper_bound",
                reason: "v3_arb_viable",
                source_pool: source_pool,
                alt_pool: best_alt_pool,
                direction: best_direction,
                source_price: source_price,
                protocol_type: "v3",
                mode: "shadow"
            };
        }

        // Round-trips computed but none profitable -> honest negative (the REAL <=0 number).
        return #{
            is_opportunity: false,
            estimated_profit: best_net_usd,
            estimated_amount_out: best_final_in,
            confidence: 0.0,
            single_tick_unconfirmed: true,
            v3_sizing_method: "single_tick_upper_bound",
            reason: "v3_arb_unprofitable",
            source_pool: source_pool,
            alt_pool: best_alt_pool,
            protocol_type: "v3",
            mode: "shadow"
        };
    }

    // ── Buscar pools alternativos para el mismo par (path V2 cross-DEX) ──
    // get_pool_index resuelve automáticamente por chain_id del host
    let alt_pools = get_pool_index(meta_in.symbol, meta_out.symbol);

    if alt_pools.len() < 2 {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "insufficient_pools"
        };
    }

    // ── Precio del pool fuente — V2 constant-product (V3 ya retornó arriba) ──
    let source_reserves = pool_data.reserves_source;
    if source_reserves == () {
        return #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none",
            reason: "source_reserves_unavailable"
        };
    }
    let source_price = calculate_price_v2(
        source_reserves.r0,
        source_reserves.r1,
        decimals_in,
        decimals_out
    );

    // ── Escanear pools alternativos buscando discrepancia ───────────────
    let best_profit = 0.0;
    let best_pool = "";
    let best_price = 0.0;
    let best_reserves = ();

    for pool_addr in alt_pools {
        // Saltar el pool fuente
        if pool_addr == source_pool {
            continue;
        }

        // Obtener reservas del pool alternativo (host resuelve por chain)
        let reserves = get_reserves(pool_addr);
        if reserves == () {
            continue;
        }

        // Verificar frescura de las reservas (máx 3 bloques de antigüedad)
        let block_number = get_block_number();
        if reserves.block < block_number - 3 {
            log_quantum("debug", "stale reserves for pool " + pool_addr);
            continue;
        }

        // Calcular precio en pool alternativo
        let alt_price = calculate_price_v2(
            reserves.r0,
            reserves.r1,
            decimals_in,
            decimals_out
        );

        if alt_price <= 0.0 || source_price <= 0.0 {
            continue;
        }

        // ── Calcular profit potencial ───────────────────────────────────
        let price_diff_pct = math_abs((alt_price - source_price) / source_price) * 100.0;

        // Calcular amount_out óptimo usando constant-product math
        let amount_in_f = from_wei(amount_in_str, decimals_in);
        let estimated_out_source = calculate_amount_out_v2(
            amount_in_str,
            source_reserves.r0,
            source_reserves.r1
        );
        let estimated_out_alt = calculate_amount_out_v2(
            amount_in_str,
            reserves.r0,
            reserves.r1
        );

        let profit_raw = 0.0;
        if alt_price > source_price {
            // Comprar en source (más barato), vender en alt (más caro)
            profit_raw = estimated_out_alt - estimated_out_source;
        } else {
            // Comprar en alt (más barato), vender en source (más caro)
            profit_raw = estimated_out_source - estimated_out_alt;
        }

        let profit_human = from_wei(profit_raw.to_string(), decimals_out);

        if profit_human > best_profit {
            best_profit = profit_human;
            best_pool = pool_addr;
            best_price = alt_price;
            best_reserves = reserves;
        }
    }

    // ── Evaluar viabilidad económica (gas-aware, chain-agnostic) ────────
    let base_fee = get_base_fee();
    let estimated_gas = estimate_gas_cost(protocol_type, chain_id);
    let gas_cost_token = estimated_gas * base_fee / 1000000000.0; // gwei to token

    // Profit mínimo debe cubrir gas + margen de seguridad (20%)
    let min_profit_threshold = gas_cost_token * 1.2;

    if best_profit <= min_profit_threshold {
        return #{
            is_opportunity: false,
            estimated_profit: best_profit,
            confidence: 0.0,
            urgency: "none",
            reason: "below_gas_threshold",
            gas_cost: gas_cost_token,
            best_pool: best_pool
        };
    }

    // ── Calcular confianza basada en múltiples factores ──────────────────
    let confidence = calculate_confidence(
        best_profit,
        min_profit_threshold,
        source_reserves,
        best_reserves,
        block_number
    );

    // ── Determinar urgencia ─────────────────────────────────────────────
    let urgency = "monitor";
    if confidence > 0.85 && best_profit > min_profit_threshold * 3.0 {
        urgency = "immediate";
    } else if confidence > 0.6 && best_profit > min_profit_threshold * 1.5 {
        urgency = "next_block";
    }

    // ── Emitir señal de oportunidad detectada ───────────────────────────
    if urgency == "immediate" || urgency == "next_block" {
        emit_signal("opportunity_detected", #{
            chain_id: chain_id,
            source_pool: source_pool,
            target_pool: best_pool,
            profit_estimate: best_profit,
            confidence: confidence
        });
    }

    log_quantum("info", `[chain:${chain_id}] DEX arb found: profit=${best_profit}, conf=${confidence}, urgency=${urgency}`);

    #{
        is_opportunity: true,
        estimated_profit: best_profit,
        confidence: confidence,
        urgency: urgency,
        source_pool: source_pool,
        target_pool: best_pool,
        source_price: source_price,
        target_price: best_price,
        token_in: token_in,
        token_out: token_out,
        amount_in: amount_in_str,
        gas_cost_estimate: gas_cost_token,
        net_profit: best_profit - gas_cost_token,
        protocol_type: protocol_type,
        chain_id: chain_id,
        block_number: block_number,
        route_type: "two_leg_arb"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CONTRATO UNIVERSAL: build_payload(opportunity)
// ─────────────────────────────────────────────────────────────────────────────

/// Construye el payload de ejecución Web3 para la oportunidad detectada.
/// El payload es chain-agnostic — el executor downstream resuelve contratos
/// y parámetros específicos de la chain.
fn build_payload(opportunity) {
    let chain_id = get_chain_id();
    let block_number = get_block_number();
    let base_fee = get_base_fee();
    let timestamp = get_timestamp();

    // Deadline: 2 bloques en el futuro (chain-agnostic timing)
    let block_time = get_block_time_estimate(chain_id);
    let deadline = timestamp + (block_time * 2);

    // Priority fee: escalar según urgencia y profit
    let priority_fee = calculate_priority_fee(
        opportunity.urgency,
        opportunity.net_profit,
        base_fee
    );

    // Gas limit: estimación conservadora por tipo de ruta
    let gas_limit = estimate_gas_limit(opportunity.protocol_type, opportunity.route_type);

    #{
        // Execution parameters (chain-agnostic)
        target_contract: "EXECUTOR",  // Resolved by executor layer per chain
        calldata: encode_arb_calldata(opportunity),
        value_wei: "0",
        gas_limit: gas_limit,
        max_priority_fee_gwei: priority_fee,
        deadline_ts: deadline,

        // Route metadata for executor
        route: #{
            source_pool: opportunity.source_pool,
            target_pool: opportunity.target_pool,
            token_in: opportunity.token_in,
            token_out: opportunity.token_out,
            amount_in: opportunity.amount_in,
            direction: if opportunity.source_price < opportunity.target_price { "buy_source_sell_target" } else { "buy_target_sell_source" },
            protocol: opportunity.protocol_type
        },

        // Risk parameters
        risk: #{
            max_slippage_bps: 50,  // 0.5% max slippage
            min_profit_after_gas: opportunity.net_profit * 0.8,  // 80% of estimated
            revert_protection: true,
            sandwich_guard: true
        },

        // Flash loan config (if needed for larger arbs)
        flash_loan: if opportunity.net_profit > 100.0 {
            #{
                protocol: "auto",  // Executor picks cheapest flash loan source
                token: opportunity.token_in,
                amount: opportunity.amount_in
            }
        } else {
            ()
        },

        // Metadata
        meta: #{
            cartridge: "dex_arb_universal",
            version: "2.0.0",
            chain_id: chain_id,
            generated_at: timestamp,
            block_number: block_number
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LIFECYCLE HOOKS (Opcionales)
// ─────────────────────────────────────────────────────────────────────────────

/// Llamado cuando el cartucho se activa.
fn on_activate() {
    let chain_id = get_chain_id();
    log_quantum("info", `[chain:${chain_id}] DEX Arb Universal cartridge ACTIVATED`);
    emit_signal("cartridge_lifecycle", #{
        event: "activated",
        cartridge: "dex_arb_universal",
        chain_id: chain_id
    });
}

/// Llamado cuando el cartucho se pausa.
fn on_deactivate() {
    let chain_id = get_chain_id();
    log_quantum("info", `[chain:${chain_id}] DEX Arb Universal cartridge DEACTIVATED`);
}

/// Llamado en cada nuevo bloque — útil para actualizar estado interno.
fn on_new_block(block_number) {
    // No-op para este cartucho. Estrategias más complejas pueden
    // mantener estado entre bloques aquí.
}

// ═══════════════════════════════════════════════════════════════════════════════
// FUNCIONES AUXILIARES (Pure math, chain-agnostic)
// ═══════════════════════════════════════════════════════════════════════════════

/// Calcula precio spot en un pool V2 (constant product: x * y = k).
/// Retorna precio de token0 en términos de token1.
fn calculate_price_v2(r0_str, r1_str, decimals0, decimals1) {
    let r0 = parse_float(r0_str);
    let r1 = parse_float(r1_str);

    if r0 <= 0.0 || r1 <= 0.0 {
        return 0.0;
    }

    // Normalizar por decimales
    let adj_r0 = r0 / math_pow(10.0, decimals0.to_float());
    let adj_r1 = r1 / math_pow(10.0, decimals1.to_float());

    adj_r1 / adj_r0
}

/// Calcula amount_out para un swap V2 con fee estándar (0.3%).
/// Fórmula: amount_out = (amount_in * 997 * r1) / (r0 * 1000 + amount_in * 997)
fn calculate_amount_out_v2(amount_in_str, r0_str, r1_str) {
    let amount_in = parse_float(amount_in_str);
    let r0 = parse_float(r0_str);
    let r1 = parse_float(r1_str);

    if r0 <= 0.0 || r1 <= 0.0 || amount_in <= 0.0 {
        return 0.0;
    }

    let amount_in_with_fee = amount_in * 997.0;
    let numerator = amount_in_with_fee * r1;
    let denominator = r0 * 1000.0 + amount_in_with_fee;

    if denominator <= 0.0 {
        return 0.0;
    }

    numerator / denominator
}

/// Cross-pool round-trip for ONE alt (V2) pool against the V3 source pool.
/// Returns () if uncomputable (missing reserves, stale, missing token0, degenerate legs),
/// else #{ net_usd, final_in, direction } for the better of the two directions.
/// Math is per-protocol-partitioned: V3 source leg -> v3_amount_out_single_tick (slot0),
/// V2 alt leg -> v2_amount_out_str (reserves). gross_profit_wei = final_in - amount_in.
fn v3_roundtrip_for_alt(alt_pool, source_pool, token_in, token_out, amount_in_str, amount_in_f,
    slot0, v3_fee_bps, zfo_in_to_out, zfo_out_to_in, v2_fee_bps, price_in_usd, gas_usd,
    decimals_in, block_number) {

    let alt_res = get_reserves(alt_pool);
    if alt_res == () {
        return ();
    }
    if alt_res.block != () && alt_res.block < block_number - 3 {
        return ();
    }
    let token0 = alt_res.token0_addr;
    if token0 == () {
        return ();
    }
    // Order reserves by token0 so reserve_in/out match the swap direction.
    let rin_io = alt_res.r1;
    let rout_io = alt_res.r0;
    if token0 == token_in {
        rin_io = alt_res.r0;
        rout_io = alt_res.r1;
    }
    // Reversed (token_out -> token_in).
    let rin_oi = rout_io;
    let rout_oi = rin_io;

    let best = ();

    // Direction A: leg1 = V3 source (in->out), leg2 = V2 alt (out->in).
    let a_leg1 = v3_amount_out_single_tick(amount_in_str, slot0.sqrt_price_x96, slot0.liquidity, v3_fee_bps, zfo_in_to_out);
    if a_leg1 != () {
        let a_leg2 = v2_amount_out_str(a_leg1, rin_oi, rout_oi, v2_fee_bps);
        if a_leg2 != () {
            let net_usd = (from_wei(a_leg2, decimals_in) - amount_in_f) * price_in_usd - gas_usd;
            best = #{ net_usd: net_usd, final_in: a_leg2, direction: "v3_source_then_v2_alt" };
        }
    }

    // Direction B: leg1 = V2 alt (in->out), leg2 = V3 source (out->in).
    let b_leg1 = v2_amount_out_str(amount_in_str, rin_io, rout_io, v2_fee_bps);
    if b_leg1 != () {
        let b_leg2 = v3_amount_out_single_tick(b_leg1, slot0.sqrt_price_x96, slot0.liquidity, v3_fee_bps, zfo_out_to_in);
        if b_leg2 != () {
            let net_usd = (from_wei(b_leg2, decimals_in) - amount_in_f) * price_in_usd - gas_usd;
            if best == () || net_usd > best.net_usd {
                best = #{ net_usd: net_usd, final_in: b_leg2, direction: "v2_alt_then_v3_source" };
            }
        }
    }

    best
}

/// Calcula la confianza de la oportunidad basada en múltiples factores.
fn calculate_confidence(profit, min_threshold, source_reserves, target_reserves, block_number) {
    let confidence = 0.0;

    // Factor 1: Ratio profit/threshold (más profit = más confianza)
    let profit_ratio = profit / min_threshold;
    if profit_ratio > 5.0 {
        confidence += 0.4;
    } else if profit_ratio > 3.0 {
        confidence += 0.3;
    } else if profit_ratio > 1.5 {
        confidence += 0.2;
    } else {
        confidence += 0.1;
    }

    // Factor 2: Liquidez del pool fuente (más liquidez = menos slippage)
    if source_reserves != () {
        let source_liq = parse_float(source_reserves.r0) + parse_float(source_reserves.r1);
        if source_liq > 1000000000000000000000.0 {  // > 1000 ETH equivalent
            confidence += 0.3;
        } else if source_liq > 100000000000000000000.0 {  // > 100 ETH
            confidence += 0.2;
        } else {
            confidence += 0.1;
        }
    }

    // Factor 3: Frescura de datos (reservas del mismo bloque = máxima confianza)
    if source_reserves != () && source_reserves.block >= block_number {
        confidence += 0.2;
    } else if source_reserves != () && source_reserves.block >= block_number - 1 {
        confidence += 0.15;
    } else {
        confidence += 0.05;
    }

    // Factor 4: Target pool liquidity
    if target_reserves != () {
        let target_liq = parse_float(target_reserves.r0) + parse_float(target_reserves.r1);
        if target_liq > 100000000000000000000.0 {
            confidence += 0.1;
        } else {
            confidence += 0.05;
        }
    }

    // Clamp entre 0 y 1
    if confidence > 1.0 { confidence = 1.0; }
    if confidence < 0.0 { confidence = 0.0; }

    confidence
}

/// Estima el costo de gas en unidades de gas según protocolo y chain.
/// Chain-agnostic: usa heurísticas por tipo de protocolo.
fn estimate_gas_cost(protocol_type, chain_id) {
    // Gas base por tipo de swap
    let base_gas = if protocol_type == "v3" {
        185000.0  // V3 swaps son más costosos
    } else {
        150000.0  // V2 swaps
    };

    // Factor multiplicador por complejidad de ruta (2-leg arb)
    base_gas * 2.2
}

/// Estima el gas limit para la transacción.
fn estimate_gas_limit(protocol_type, route_type) {
    let base = if protocol_type == "v3" { 350000 } else { 280000 };

    if route_type == "three_leg_arb" {
        base * 2
    } else {
        base
    }
}

/// Estima el block time de la chain (en segundos).
/// Valores por defecto razonables para chains conocidas, con fallback genérico.
fn get_block_time_estimate(chain_id) {
    // No hardcodeamos — usamos heurísticas generales.
    // L2s tienden a tener bloques más rápidos que L1.
    // El host puede override esto con datos reales en el futuro.
    if chain_id == 1 || chain_id == 5 || chain_id == 11155111 {
        12  // Ethereum mainnet/testnets: ~12s
    } else if chain_id == 137 || chain_id == 80001 {
        2   // Polygon: ~2s
    } else if chain_id == 42161 || chain_id == 421613 {
        1   // Arbitrum: ~0.25s but we use 1s for safety
    } else if chain_id == 8453 || chain_id == 84531 {
        2   // Base: ~2s
    } else if chain_id == 10 || chain_id == 420 {
        2   // Optimism: ~2s
    } else if chain_id == 56 {
        3   // BSC: ~3s
    } else if chain_id == 43114 {
        2   // Avalanche: ~2s
    } else {
        3   // Default fallback for unknown chains
    }
}

/// Calcula priority fee basado en urgencia y profit.
fn calculate_priority_fee(urgency, net_profit, base_fee) {
    if urgency == "immediate" {
        // Pagar hasta 10% del profit como priority fee
        let max_fee = net_profit * 0.1;
        let suggested = base_fee * 2.0;
        math_min(max_fee, suggested)
    } else if urgency == "next_block" {
        base_fee * 1.2
    } else {
        base_fee * 0.8
    }
}

/// Codifica el calldata para la transacción de arbitraje.
/// Retorna un placeholder — el executor real ensambla el calldata final.
fn encode_arb_calldata(opportunity) {
    // El calldata real es ensamblado por el Executor layer en Rust,
    // que conoce las direcciones de contrato específicas de cada chain.
    // Aquí retornamos los parámetros estructurados para que el executor
    // pueda construir el calldata final.
    "0x"
        + "execute_arb("
        + opportunity.source_pool + ","
        + opportunity.target_pool + ","
        + opportunity.token_in + ","
        + opportunity.token_out + ","
        + opportunity.amount_in + ")"
}

/// Parse string a float con manejo seguro.
fn parse_float(s) {
    if s == () || s == "" {
        return 0.0;
    }
    // Rhai parse_float nativo
    let result = s.to_float();
    if result == () {
        0.0
    } else {
        result
    }
}
