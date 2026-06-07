// arbx-risk-limits-enforcement: Daily loss cap + kill-switch
use shared_rs::risk_ledger::RiskLedger;

let risk_ledger = RiskLedger::from_env()
    .with_drawdown_cap_pct({{RISK_CAP_USD}})
    .with_daily_loss_cap_usd({{MAX_DAILY_LOSS_USD}});

if !risk_ledger.is_within_limits().await {
    log::error!("Route rejected: exceeds risk limits (drawdown, daily loss, or kill-switch armed)");
    return Ok(None);
}
