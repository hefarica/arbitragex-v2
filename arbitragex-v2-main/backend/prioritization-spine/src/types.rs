use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityCandidate {
    pub route_fingerprint: String,
    pub pool_addresses: Vec<String>,
    pub token_addresses: Vec<String>,
    pub dex_adapters: Vec<String>,
    pub amount_in: f64,
    pub expected_amount_out: f64,
    pub gross_profit: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityScore {
    pub net_expected_profit: f64,
    pub landing_probability: f64,
    pub state_freshness: f64,
    pub liquidity_confidence: f64,
    pub execution_atomicity: f64,
    pub computational_cost: f64,
    pub reversal_risk: f64,
    pub slippage_risk: f64,
    pub gas_volatility_risk: f64,
    pub token_risk: f64,
    pub final_score: f64,
}
