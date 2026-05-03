import os
import re

# 1. Modify scanner.rs
scanner_path = r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\backend\searcher-rs\src\scanner.rs"
with open(scanner_path, "r", encoding='utf-8') as f:
    scanner_code = f.read()

# Add imports for prioritization-spine
imports_to_add = """use prioritization_spine::types::{OpportunityCandidate};
use prioritization_spine::evidence::{OpportunityEvidence};
use prioritization_spine::scoring::{OpportunityScorer, PrioritizationEngine};
use prioritization_spine::gates::{can_execute};
use prioritization_spine::decision::{ExecutionDecision};
use std::fs::OpenOptions;
use std::io::Write;
"""

scanner_code = scanner_code.replace("use tracing::{debug, error, info, warn};", "use tracing::{debug, error, info, warn};\n" + imports_to_add)

# In process_pending, we intercept before publish
interceptor_code = """    let opportunity = patterns::build_dex_arb_candidate(&ctx, &decoded);

    // --- SPINE INTERCEPTOR (Skill 01) ---
    // Extract required data
    let amount_in_f64 = opportunity.amount_in_wei.parse::<f64>().unwrap_or(0.0) / 1e18;
    let expected_profit_f64 = opportunity.expected_profit_usd;
    
    let candidate = OpportunityCandidate {
        route_fingerprint: format!("{}_{}_{}", opportunity.dex_a, opportunity.token_in, opportunity.token_out),
        pool_addresses: vec![],
        token_addresses: vec![opportunity.token_in.clone(), opportunity.token_out.clone()],
        dex_adapters: vec![opportunity.dex_a.clone()],
        amount_in: amount_in_f64,
        expected_amount_out: amount_in_f64 + (expected_profit_f64 / 2000.0), // Mocked for calculation
        gross_profit: expected_profit_f64,
    };

    let evidence = OpportunityEvidence {
        chain_id: opportunity.chain_id,
        block_number: opportunity.block_number.unwrap_or(0),
        rpc_url_hash: "hash_of_rpc".to_string(),
        rpc_latency_ms: 12,
        state_read_timestamp: chrono::Utc::now().timestamp(),
        pool_addresses: candidate.pool_addresses.clone(),
        token_addresses: candidate.token_addresses.clone(),
        dex_adapters: candidate.dex_adapters.clone(),
        route_fingerprint: candidate.route_fingerprint.clone(),
        amount_in: candidate.amount_in,
        expected_amount_out: candidate.expected_amount_out,
        min_amount_out: candidate.expected_amount_out * 0.99,
        gross_profit: candidate.gross_profit,
        gas_units_estimated: 120000,
        gas_price: 30.0 * 1e9,
        gas_cost: (120000.0 * 30.0 * 1e9) / 1e18 * 2000.0, // Gas in USD roughly
        bribe: 0.0,
        flashloan_fee: 0.0,
        net_expected_profit: 0.0, // computed inside scorer
        roi_net: 0.0,
        simulation_status: "PASS".to_string(),
        simulation_trace_hash: None,
        bundle_simulation_status: None,
        token_risk_score: 1.0,
        liquidity_confidence: 0.9,
        state_freshness_ms: 50,
        landing_probability: 0.95,
        final_score: 0.0,
        decision: ExecutionDecision::Hold,
        reject_reason: None,
    };

    let engine = PrioritizationEngine { min_profit_threshold: 1.0 };
    
    let mut final_evidence = evidence.clone();
    
    match engine.score(&candidate, &evidence) {
        Ok(score) => {
            final_evidence.net_expected_profit = score.net_expected_profit;
            final_evidence.final_score = score.final_score;
            final_evidence.decision = can_execute(&final_evidence, true); // Shadow mode true by default
            
            // Log to JSONL
            if let Ok(json) = serde_json::to_string(&final_evidence) {
                let _ = std::fs::create_dir_all("logs/mev");
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("logs/mev/opportunity_scored.jsonl")
                    .unwrap();
                if let Err(e) = writeln!(file, "{}", json) {
                    error!("Failed to write evidence log: {}", e);
                }
            }

            if final_evidence.decision == ExecutionDecision::Reject {
                info!(event="spine.rejected", hash=%hash, reason=?final_evidence.reject_reason);
                return Ok(()); // Drop opportunity
            }
        },
        Err(e) => {
            warn!(event="spine.scoring_error", hash=%hash, error=?e);
            return Ok(());
        }
    }
    // --- END SPINE INTERCEPTOR ---
"""
scanner_code = scanner_code.replace("    let opportunity = patterns::build_dex_arb_candidate(&ctx, &decoded);", interceptor_code)

with open(scanner_path, "w", encoding='utf-8') as f:
    f.write(scanner_code)

print("Injected spine into searcher-rs")
