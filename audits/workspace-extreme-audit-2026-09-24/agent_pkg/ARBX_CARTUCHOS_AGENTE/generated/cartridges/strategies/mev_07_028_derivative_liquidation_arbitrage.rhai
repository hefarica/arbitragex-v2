// =============================================================================
// CARTUCHO AGENTE V4 — generado desde AMBOS libros, no desde el nombre del archivo.
// Las capas A/B son específicas de MEV_ID. Las curvas y operadores son nativos.
// No signer, RPC endpoint, capital default, score default or fabricated calldata.
// =============================================================================

// [CAPA A: IDENTIDAD Y MISION]
// Function-local metadata: works with the existing EMPTY Rhai Scope invocation.
fn init_strategy() {
    #{
        "name": "Derivative-liquidation arbitrage",
        "version": "4.0.0",
        "author": "ArbitrageX — workbook-derived generator",
        "description": "Explota una relación de paridad, basis, funding, vencimiento o volatilidad mediante Derivative-liquidation arbitrage.",
        "category": "derivatives_engine",
        "mev_id": "MEV-07-028",
        "contract_version": "arbx.cartridge.agent/4",
        "detector_id": "D_SETTLE",
        "execution_class": "DERIVATIVE_DATA_REQUIRED",
        "target_chains": [],
        "primary_operators": [8, 21, 23],
        "secondary_operators": [11, 13, 22],
        "min_legs": 2,
        "max_legs": 6,
        "allowed_search_hops": [2, 3, 4, 5, 6],
        "requested_discovery_cap": 7,
        "requires_native_agent_v4": true,
        "trigger_description": "Desviación de basis/funding/paridad/volatilidad",
        "required_bindings": ["agent_v4_discover", "agent_v4_quote", "agent_v4_operators", "agent_v4_economic_check", "agent_v4_seal", "agent_v4_money_compare", "agent_v4_build_payload"],
        "config_schema": #{
        "enabled": #{
        "type": "bool",
        "source": "existing strategy config",
        "default": ()
    },
        "min_profit_usd": #{
        "type": "decimal_string",
        "source": "existing strategy config",
        "default": ()
    },
        "capital_cap_usd": #{
        "type": "decimal_string",
        "source": "canonical capital provider",
        "default": ()
    },
        "operator_switches": #{
        "type": "map<bool>",
        "allowed_ids": [8, 21, 23, 11, 13, 22],
        "default": ()
    },
        "source_specific_parameters": #{
        "type": "read_only_contract_text",
        "value": "max_settlement_delay_sec; max_oracle_age_sec; min_profit_usd; min_margin_buffer."
    }
    },
        "source_references": #{
        "catalog": "ArbitrageX_Master_264x31_LIVE_First(2).xlsx!01_MEV_MATRIX_1_11!A188:AA188",
        "math": "ArbitrageX_Master_264x31_LIVE_First(2).xlsx!02_CARTRIDGE_MATH_MAP!A188:AB188",
        "master": "ArbitrageX_Master_264x31_LIVE_First(2).xlsx!07_MASTER_264x31!A5768:AJ5798"
    },
        "full_manifest_path": "generated/manifests/MEV-07-028.json"
    }
}

fn agent_manifest() {
    #{
        "contract": "arbx.cartridge.agent/4",
        "mev_id": "MEV-07-028",
        "name": "Derivative-liquidation arbitrage",
        "category": "derivatives_engine",
        "detector_id": "D_SETTLE",
        "execution_class": "DERIVATIVE_DATA_REQUIRED",
        "equation": "Compute deterministic payoff/settlement value under venue rules and compare with executable acquisition/hedge/unwind cost; include liquidation penalty and expiry timing.",
        "specific_note": "Use family detector exactly; strategy identity constrains route shape, venue/protocol mix, trigger and dependencies from the Excel row.",
        "source_leg_bounds": #{
        "min": 2,
        "max": 6,
        "model": "Hedge/estructura"
    },
        "effective_search_hops": [2, 3, 4, 5, 6],
        "repository_hop_mask": 31,
        "weights_calibrated": false,
        "modes": ["LIVE_MAINNET", "TESTNET", "PAPER_SHADOW"],
        "conflicts": [],
        "logic": "carry_projection",
        "allowed_search_hops": [2, 3, 4, 5, 6],
        "required_data_text": "Contract payoff; mark/index/settlement oracle; expiry; position/margin state; executable hedge.",
        "source_atomicity": #{
        "possible": "Condicional",
        "kind": "Settlement multi-protocolo en una transacción si todas las patas comparten dominio",
        "non_atomic": "Carry/funding/vencimiento; hedge externo o multi-bloque"
    },
        "original_live_gate": "Sim PASS + net_profit>0 + risk_score>=70 + private route cuando aplique",
        "source_digest": "84b5d25b6ba0abe49e6b50e682d80ec71276f13b19ce8d9d370388ea835da795",
        "operator_requirements": [#{
        "id": 1,
        "role": "N/A",
        "phase": "EVIDENCE",
        "semantic_role": "SPECTRAL_STRUCTURE",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 2,
        "role": "N/A",
        "phase": "EVIDENCE",
        "semantic_role": "DIMENSION_REDUCTION",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 3,
        "role": "N/A",
        "phase": "EVIDENCE",
        "semantic_role": "EIGEN_REGIME",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 4,
        "role": "N/A",
        "phase": "EVIDENCE",
        "semantic_role": "ENTROPY_COHERENCE",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 5,
        "role": "N/A",
        "phase": "DISCOVER",
        "semantic_role": "EVENT_JUMP_MODEL",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 6,
        "role": "N/A",
        "phase": "RISK",
        "semantic_role": "STATE_TRANSITION",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 7,
        "role": "N/A",
        "phase": "RISK",
        "semantic_role": "HIDDEN_REGIME",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 8,
        "role": "PRIMARY",
        "phase": "EVIDENCE",
        "semantic_role": "FAIR_VALUE_FILTER",
        "requirement": "PRIMARY_REQUIRED_OR_EQUIVALENT",
        "weight": ()
    }, #{
        "id": 9,
        "role": "N/A",
        "phase": "RISK",
        "semantic_role": "HEAVY_TAIL",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 10,
        "role": "N/A",
        "phase": "EVIDENCE",
        "semantic_role": "ONLINE_STATISTICS",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 11,
        "role": "SECONDARY",
        "phase": "EVIDENCE",
        "semantic_role": "BAYES_POSTERIOR",
        "requirement": "OPTIONAL_SUPPORT",
        "weight": ()
    }, #{
        "id": 12,
        "role": "N/A",
        "phase": "EVIDENCE",
        "semantic_role": "PARAMETER_ESTIMATION",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 13,
        "role": "SECONDARY",
        "phase": "EVIDENCE",
        "semantic_role": "PARITY_REGRESSION",
        "requirement": "OPTIONAL_SUPPORT",
        "weight": ()
    }, #{
        "id": 14,
        "role": "N/A",
        "phase": "EVIDENCE",
        "semantic_role": "DIVERGENCE",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 15,
        "role": "N/A",
        "phase": "SIZE",
        "semantic_role": "ONE_DIM_OPTIMIZER",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 16,
        "role": "N/A",
        "phase": "SIZE",
        "semantic_role": "CAPITAL_FRACTION",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 17,
        "role": "N/A",
        "phase": "SIZE",
        "semantic_role": "CONTROL_OPTIMIZER",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 18,
        "role": "N/A",
        "phase": "SIZE",
        "semantic_role": "ACTION_FUNCTIONAL",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 19,
        "role": "N/A",
        "phase": "SIZE",
        "semantic_role": "LP_ALLOCATION",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 20,
        "role": "N/A",
        "phase": "SIZE",
        "semantic_role": "CONTINUOUS_OPTIMIZER",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 21,
        "role": "PRIMARY",
        "phase": "SIZE",
        "semantic_role": "ROOT_INVARIANT_SOLVER",
        "requirement": "PRIMARY_REQUIRED_OR_EQUIVALENT",
        "weight": ()
    }, #{
        "id": 22,
        "role": "SECONDARY",
        "phase": "RISK",
        "semantic_role": "SCENARIO_MONTE_CARLO",
        "requirement": "OPTIONAL_SUPPORT",
        "weight": ()
    }, #{
        "id": 23,
        "role": "PRIMARY",
        "phase": "RISK",
        "semantic_role": "LATENCY_QUEUE",
        "requirement": "PRIMARY_REQUIRED_OR_EQUIVALENT",
        "weight": ()
    }, #{
        "id": 24,
        "role": "N/A",
        "phase": "RANK",
        "semantic_role": "GAME_THEORY",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 25,
        "role": "N/A",
        "phase": "VALIDATE",
        "semantic_role": "BUNDLE_RECONSTRUCTION",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 26,
        "role": "N/A",
        "phase": "VALIDATE",
        "semantic_role": "FLASH_FEASIBILITY",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 27,
        "role": "N/A",
        "phase": "DISCOVER",
        "semantic_role": "PATH_ORDERING",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 28,
        "role": "N/A",
        "phase": "EVIDENCE",
        "semantic_role": "LIQUIDITY_STATE",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 29,
        "role": "N/A",
        "phase": "RANK",
        "semantic_role": "SURPLUS_ATTRIBUTION",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 30,
        "role": "N/A",
        "phase": "DISCOVER",
        "semantic_role": "GRAPH_PRUNING",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }, #{
        "id": 31,
        "role": "N/A",
        "phase": "RANK",
        "semantic_role": "LEARNED_POLICY",
        "requirement": "NOT_APPLICABLE",
        "weight": ()
    }]
    }
}

// [CAPA B: DESCUBRIMIENTO Y SEMANTICA DE PROTOCOLO]
// Required native bindings are shipped in runtime/rhai_agent_bridge.rs.
// Discovery is bounded and shared by immutable snapshot; it is NOT Dijkstra
// applied to size-dependent/nonlinear edge weights. Effective hops preserve
// the exact strategy mask; wider cycles remain visible as handoff candidates.
fn find_optimal_paths(ctx) {
    let spec = agent_manifest();
    agent_v4_discover(ctx, spec)
}

// This list is an executable input contract, NOT a claim that the checks ran.
// Each check needs a snapshot-bound native receipt, not a frontend boolean.
fn detector_requirements() {
    ["hedged_position", "matched_horizon", "margin_sufficient", "funding_schedule", "finite_horizon", "settlement_executable", "strategy_specific_note_verified", "native_risk_and_impact_policy"]
}

// [CAPA C: DECISION ECONOMICA Y EVIDENCIA]
fn evaluate_opportunity_internal(ctx, candidate) {
    let spec = agent_manifest();
    // Exact protocol quote at candidate amount; output i is input i+1.
    // V3/Curve/bins/hook adapters may not fall back to constant-product math.
    let quote = agent_v4_quote(ctx, spec, candidate);
    // Run all applicable enabled operators even when gross/net is negative.
    // DISABLED is not zero; missing applicable output is an observable DATA_GAP.
    let evidence = agent_v4_operators(ctx, spec, candidate);
    // Applies the detector-specific settlement constraint receipts and computes
    // common-numeraire cashflows. Embedded fees/impact are NOT subtracted twice.
    // Carry/bridge expectations remain projections, never realized gains.
    agent_v4_economic_check(ctx, spec, candidate, quote, evidence, detector_requirements())
}

// [CAPA D: SELECCION, PLAN INMUTABLE Y ENTREGA]
// Signature matches the Universal Cartridge Contract: ONE context argument.
fn evaluate_opportunity(ctx) {
    let spec = agent_manifest();
    let discovery = find_optimal_paths(ctx);
    if discovery.status != "READY" {
        return agent_v4_seal(ctx, spec, (), [], discovery);
    }
    let observations = [];
    let best_candidate = ();
    let best_diagnostic = ();
    for candidate in discovery.candidates {
        let result = evaluate_opportunity_internal(ctx, candidate);
        observations.push(result);
        // Preserve a computed loss and exactly zero; () means absent, not zero.
        if best_diagnostic == () {
            best_diagnostic = result;
        } else if result.net_profit_usd != () {
            if best_diagnostic.net_profit_usd == () {
                best_diagnostic = result;
            } else if agent_v4_money_compare(result.net_profit_usd, best_diagnostic.net_profit_usd) > 0 {
                best_diagnostic = result;
            }
        }
        if result.candidate_eligible == true {
            if best_candidate == () {
                best_candidate = result;
            } else if agent_v4_money_compare(result.net_profit_usd, best_candidate.net_profit_usd) > 0 {
                best_candidate = result;
            }
        }
    }
    // A positive bounded-search proposal is NOT a simulation PASS or broadcast
    // authorization. The canonical SizeOptimizer/simulator/risk gates still own
    // final acceptance. Observations include every failed applicable producer.
    let selected = if best_candidate != () { best_candidate } else { best_diagnostic };
    agent_v4_seal(ctx, spec, selected, observations, discovery)
}

fn build_payload(opportunity) {
    // The backend encoder must bind the EXACT selected plan + amount + revision
    // to its simulation trace. This binding returns real encoded bytes or an
    // explicit NEEDS_CANONICAL_ENCODER result — never 0x/EXECUTOR placeholders.
    agent_v4_build_payload(opportunity, agent_manifest())
}
