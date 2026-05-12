//! Variance checker — flags a risk_event if |variance_pct| exceeds threshold.
//!
//! Pure function. Caller persists both the ReconReport AND the RiskEvent.

use shared_rs::contracts::ReconReport;
use uuid::Uuid;

pub struct RiskEventOut {
    pub event_type: String,
    pub severity: String,
    pub payload: serde_json::Value,
    pub trace_id: Uuid,
    pub opportunity_id: Option<Uuid>,
}

pub fn check(report: &ReconReport, threshold_pct: f64) -> Option<RiskEventOut> {
    let v = report.variance_pct?;
    if v.abs() <= threshold_pct {
        return None;
    }
    Some(RiskEventOut {
        event_type: "degradation".to_string(),
        severity: if v.abs() > threshold_pct * 2.0 {
            "critical"
        } else {
            "warning"
        }
        .to_string(),
        payload: serde_json::json!({
            "reason": "variance_exceeded",
            "variance_pct": v,
            "threshold_pct": threshold_pct,
            "expected_amount_out_wei": report.expected_amount_out_wei,
            "actual_amount_out_wei": report.actual_amount_out_wei,
        }),
        trace_id: report.trace_id,
        opportunity_id: Some(report.opportunity_id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    fn mk_report(v: Option<f64>) -> ReconReport {
        ReconReport {
            opportunity_id: Uuid::nil(),
            execution_id: None,
            tx_hash: None,
            chain_id: 1,
            expected_amount_out_wei: None,
            actual_amount_out_wei: None,
            variance_native_units: None,
            variance_pct: v,
            expected_profit_usd: 0.0,
            actual_profit_usd: 0.0,
            pnl_source: "native_only".into(),
            actual_gas_used_wei: None,
            actual_gas_price_wei: None,
            fail_reason: None,
            notes: None,
            created_at: Utc::now(),
            trace_id: Uuid::nil(),
        }
    }
    #[test]
    fn variance_below_threshold_returns_none() {
        assert!(check(&mk_report(Some(15.0)), 20.0).is_none());
    }
    #[test]
    fn variance_above_threshold_returns_warning() {
        let e = check(&mk_report(Some(25.0)), 20.0).unwrap();
        assert_eq!(e.severity, "warning");
    }
    #[test]
    fn variance_2x_above_returns_critical() {
        let e = check(&mk_report(Some(50.0)), 20.0).unwrap();
        assert_eq!(e.severity, "critical");
    }
    #[test]
    fn variance_null_returns_none() {
        assert!(check(&mk_report(None), 20.0).is_none());
    }
}
