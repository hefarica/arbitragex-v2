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

    // ──────────────────────────────────────────────────────────────────────
    // OMEGA-8/M4 Fase 6: blinda recon variance checker
    // ──────────────────────────────────────────────────────────────────────

    /// Negative variance (actual_out < expected_out) is just as actionable
    /// as positive — the threshold is on |variance_pct|. -25% at threshold
    /// 20% must surface as a warning.
    #[test]
    fn negative_variance_uses_abs_value() {
        let e = check(&mk_report(Some(-25.0)), 20.0).expect("must emit");
        assert_eq!(e.severity, "warning");
        let payload = e.payload.to_string();
        assert!(payload.contains("variance_exceeded"));
        assert!(payload.contains("-25"));
    }

    /// Variance exactly at threshold is NOT flagged. Boundary inclusive
    /// on the safe side.
    #[test]
    fn variance_exactly_at_threshold_is_not_flagged() {
        assert!(check(&mk_report(Some(20.0)), 20.0).is_none());
        assert!(check(&mk_report(Some(-20.0)), 20.0).is_none());
    }

    /// Trace_id and opportunity_id must propagate from the ReconReport
    /// into the RiskEventOut so downstream alerting can correlate.
    #[test]
    fn risk_event_propagates_correlation_ids() {
        let opp = Uuid::new_v4();
        let trace = Uuid::new_v4();
        let mut r = mk_report(Some(99.0));
        r.opportunity_id = opp;
        r.trace_id = trace;
        let e = check(&r, 20.0).expect("flagged");
        assert_eq!(e.trace_id, trace);
        assert_eq!(e.opportunity_id, Some(opp));
        assert_eq!(e.event_type, "degradation");
    }

    /// Threshold of 0 still uses |variance_pct|>threshold semantics (strict).
    /// At threshold=0 with variance=0.0001, must flag — confirms no
    /// divide-by-zero or off-by-one.
    #[test]
    fn tiny_variance_with_zero_threshold_is_flagged() {
        let e = check(&mk_report(Some(0.0001)), 0.0).expect("must flag");
        assert_eq!(e.severity, "warning");
    }
}
