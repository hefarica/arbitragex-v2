//! Analysis remains eligible. Only a fresh execution against real permissions
//! upgrades an analysis carrier to a broadcast-authorizing plan.
use anyhow::{anyhow, ensure, Result};
use bigdecimal::{BigDecimal, ToPrimitive};
use ethers::types::{Address, H256, U256};
use prioritization_spine::ValidatedPlan;
use shared_rs::{
    contracts::Opportunity,
    oracle_snapshot::{configured_chain, OracleRpc, Price},
};
use std::{
    str::FromStr,
    sync::{Arc, OnceLock},
    time::Duration,
};

pub async fn refresh(
    opp: &Opportunity,
    original: &ValidatedPlan,
    caller: Address,
) -> Result<ValidatedPlan> {
    ensure!(
        crate::plan_validation::supports_strategy(&opp.strategy_kind),
        "execution_surface_adapter_required"
    );
    ensure!(
        Address::from_str(&opp.token_in)? == original.ctx.token_in
            && Address::from_str(&opp.token_out)? == original.ctx.token_out
            && U256::from_dec_str(&opp.amount_in_wei)? == original.ctx.amount_in,
        "analysis_identity_mismatch"
    );
    if let Some(b) = &original.binding {
        ensure!(
            b.opportunity_id == opp.id
                && b.chain_id == opp.chain_id
                && b.strategy_kind == opp.strategy_kind.as_str()
                && b.calldata_hash == H256(ethers::utils::keccak256(&original.wrapped_calldata)),
            "analysis_binding_mismatch"
        );
    }
    let rpc = OracleRpc::from_env(opp.chain_id)?;
    let head = rpc
        .call("eth_getBlockByNumber", serde_json::json!(["latest", false]))
        .await?;
    let hash = H256::from_str(
        head["hash"]
            .as_str()
            .ok_or_else(|| anyhow!("head_missing"))?,
    )?;
    let now = chrono::Utc::now().timestamp_millis();
    if original.binding.as_ref().is_some_and(|b| {
        !b.state_overrides_used
            && b.block_hash == hash
            && b.caller == caller
            && b.simulated_at_ms <= now
            && now - b.simulated_at_ms < 30_000
    }) {
        crate::plan_validation::validate_calldata(original).map_err(|e| anyhow!(e))?;
        return Ok(original.clone());
    }
    static SLOTS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let permit = SLOTS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| anyhow!("execution_validation_capacity"))?;
    let block = u64::from_str_radix(
        head["number"]
            .as_str()
            .and_then(|s| s.strip_prefix("0x"))
            .ok_or_else(|| anyhow!("head_number_missing"))?,
        16,
    )?;
    let timestamp = u64::from_str_radix(
        head["timestamp"]
            .as_str()
            .and_then(|s| s.strip_prefix("0x"))
            .ok_or_else(|| anyhow!("head_timestamp_missing"))?,
        16,
    )?;
    let rpc_url = std::env::var(format!("RPC_HTTP_{}", opp.chain_id))
        .map_err(|_| anyhow!("execution_rpc_missing"))?;
    let mut ctx = original.ctx.clone();
    ctx.caller = caller;
    ctx.deadline = U256::from(
        timestamp
            .checked_add(120)
            .ok_or_else(|| anyhow!("deadline_overflow"))?,
    );
    let config = sim_core::sim_multistep::MultiStepExecutionConfig {
        chain_id: opp.chain_id,
        executor_address: original.executor_address,
        route_hash: original.route_hash,
        min_profit_wei: original.min_profit_wei,
        gas_price_wei: U256::zero(),
        gas_limit_per_step: 16_000_000,
        paper_mode: false,
        enable_storage_cheats: false,
        require_trace_hash: true,
        require_positive_net_profit: true,
        max_steps: 8,
    };
    let sim_ctx = ctx.clone();
    let outcome = tokio::time::timeout(
        Duration::from_secs(20),
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            sim_core::verified_simulation::execute(
                &sim_ctx,
                Arc::new(simulator_v2::SimulatorV2::new(rpc_url).with_block(block)),
                &config,
            )
        }),
    )
    .await
    .map_err(|_| anyhow!("execution_validation_timeout"))??;
    ensure!(
        outcome.passed,
        "execution_validation_failed:{}",
        outcome.fail_reason.as_deref().unwrap_or("unknown")
    );
    let evidence = outcome
        .evidence
        .as_ref()
        .ok_or_else(|| anyhow!("execution_evidence_missing"))?;
    ensure!(evidence.block_hash == hash, "execution_snapshot_changed");
    ValidatedPlan::from_simulation(
        ctx,
        opp.id,
        opp.strategy_kind.as_str(),
        original.route_hash,
        evidence.min_profit_wei,
        original.executor_address,
        &outcome,
    )
    .map_err(|e| anyhow!(e))
}

pub struct Economics {
    pub net: f64,
    pub gas: f64,
    pub slippage_pct: f64,
}
fn units(v: U256, d: u8) -> Result<BigDecimal> {
    ensure!(d <= 77, "decimals_invalid");
    Ok(BigDecimal::from_str(&format!("{v}e-{d}"))?)
}
fn usd(v: U256, d: u8, p: &Price) -> Result<BigDecimal> {
    Ok(units(v, d)? * units(p.answer, p.decimals)?)
}

pub async fn economics(
    opp: &Opportunity,
    plan: &ValidatedPlan,
    redis: &redis::aio::ConnectionManager,
) -> Result<Economics> {
    let b = plan
        .binding
        .as_ref()
        .ok_or_else(|| anyhow!("execution_binding_missing"))?;
    let rpc = OracleRpc::from_env(opp.chain_id)?;
    let feeds = configured_chain(opp.chain_id)?;
    let asset = feeds
        .assets_usd
        .get(&format!("{:#x}", plan.ctx.token_in))
        .ok_or_else(|| anyhow!("execution_asset_feed_missing"))?;
    let asset = rpc.price(asset, b.block_hash, b.block_timestamp).await?;
    let native = rpc
        .price(&feeds.native_usd, b.block_hash, b.block_timestamp)
        .await?;
    let decimals = rpc.token_decimals(plan.ctx.token_in, b.block_hash).await?;
    let gas = usd(
        b.gas_price_wei
            .checked_mul(U256::from(b.gas_used))
            .ok_or_else(|| anyhow!("gas_overflow"))?,
        18,
        &native,
    )?;
    let net = usd(b.retained_profit_wei, decimals, &asset)? - &gas;
    let claimed = opp
        .net_expected_profit_usd
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| anyhow!("net_profit_missing"))?;
    let admitted = net.min(BigDecimal::from_str(&claimed.to_string())?);
    ensure!(
        admitted > 0 && admitted >= &gas * BigDecimal::from(3),
        "execution_three_gas_rule"
    );
    let config = shared_rs::trading_config::TradingConfigClient::from_manager(redis.clone())
        .state(opp.chain_id)
        .await?
        .ok_or_else(|| anyhow!("execution_trading_config_missing"))?;
    ensure!(
        config.capital_usd.is_finite() && config.capital_usd > 0.0,
        "execution_capital_missing"
    );
    let cap = BigDecimal::from_str(&config.capital_usd.to_string())? / BigDecimal::from(50);
    ensure!(
        usd(plan.ctx.amount_in, decimals, &asset)? <= cap,
        "execution_two_percent_capital_limit"
    );
    let display = |v: &BigDecimal| {
        v.to_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| anyhow!("execution_display_overflow"))
    };
    Ok(Economics {
        net: display(&admitted)?,
        gas: display(&gas)?,
        slippage_pct: f64::from(b.total_slippage_bps) / 100.0,
    })
}
