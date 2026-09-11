//! Receipt-based accounting. Expected profit is never a substitute for a receipt.
//!
//! Ethereum L1 and Sepolia fees are supported (execution gas and blob gas).
//! Finalized observations, priced at the receipt block hash, feed empirical risk.

use anyhow::{anyhow, bail, Result};
use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::Utc;
use ethers::abi::{decode, ParamType, Token};
use ethers::types::{Address, H256, U256};
use ethers::utils::keccak256;
use futures_util::{stream, StreamExt};
use prioritization_spine::validated_plan::ValidatedPlan;
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_rs::contracts::Opportunity;
use shared_rs::oracle_snapshot::{configured_chain, OracleRpc, Price};
use shared_rs::settlement_risk::{history_key, RealizedObservation};
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;

const PENDING_QUEUE: &str = "arbx:accounting:pending";
const INPUT_TTL_SECS: u64 = 86_400;
const RESULT_TTL_SECS: u64 = 30 * 86_400;
const HISTORY_TTL_SECS: u64 = 7 * 86_400;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingAccounting {
    opportunity: Opportunity,
    plan: ValidatedPlan,
    tx_hash: H256,
    /// The submitter has already observed Included/Reverted when it enqueues.
    /// Preserve this timestamp through retries; finality waiting is not latency.
    first_receipt_observed_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettlementResult {
    observation: RealizedObservation,
    net_profit_usd_decimal: String,
    gas_cost_wei: String,
    blob_fee_wei: String,
    asset_decimals: u8,
    asset_price_updated_at: u64,
    native_price_updated_at: u64,
    execution_status: String,
}

#[derive(Debug, Clone)]
struct EventExpectation {
    executor: Address,
    flash_executor: Address,
    route_hash: H256,
    token_in: Address,
    token_out: Address,
    principal: U256,
}

#[derive(Debug)]
struct TxExpectation<'a> {
    hash: H256,
    chain_id: u64,
    block_hash: H256,
    block_number: u64,
    caller: Address,
    target: Address,
    calldata: &'a [u8],
}

fn quantity(value: &Value) -> Result<U256> {
    let raw = value
        .as_str()
        .ok_or_else(|| anyhow!("accounting_quantity_missing"))?;
    let hex = raw
        .strip_prefix("0x")
        .ok_or_else(|| anyhow!("accounting_quantity_not_hex"))?;
    if hex.is_empty() || (hex.len() > 1 && hex.starts_with('0')) {
        bail!("accounting_quantity_noncanonical");
    }
    U256::from_str_radix(hex, 16).map_err(|_| anyhow!("accounting_quantity_invalid"))
}

fn u64_quantity(value: &Value) -> Result<u64> {
    let value = quantity(value)?;
    if value > U256::from(u64::MAX) {
        bail!("accounting_quantity_overflow");
    }
    Ok(value.as_u64())
}

fn hash(value: &Value) -> Result<H256> {
    H256::from_str(
        value
            .as_str()
            .ok_or_else(|| anyhow!("accounting_hash_missing"))?,
    )
    .map_err(|_| anyhow!("accounting_hash_invalid"))
}

fn address(value: &Value) -> Result<Address> {
    Address::from_str(
        value
            .as_str()
            .ok_or_else(|| anyhow!("accounting_address_missing"))?,
    )
    .map_err(|_| anyhow!("accounting_address_invalid"))
}

fn bytes(value: &Value) -> Result<Vec<u8>> {
    let raw = value
        .as_str()
        .and_then(|s| s.strip_prefix("0x"))
        .ok_or_else(|| anyhow!("accounting_bytes_missing"))?;
    hex::decode(raw).map_err(|_| anyhow!("accounting_bytes_invalid"))
}

fn validate_transaction(tx: &Value, expected: &TxExpectation<'_>) -> Result<()> {
    if hash(&tx["hash"])? != expected.hash
        || u64_quantity(&tx["chainId"])? != expected.chain_id
        || hash(&tx["blockHash"])? != expected.block_hash
        || u64_quantity(&tx["blockNumber"])? != expected.block_number
        || address(&tx["from"])? != expected.caller
        || address(&tx["to"])? != expected.target
        || quantity(&tx["value"])? != U256::zero()
        || bytes(&tx["input"])? != expected.calldata
    {
        bail!("accounting_transaction_identity_mismatch");
    }
    Ok(())
}

fn validate_receipt_logs(
    receipt: &Value,
    tx_hash: H256,
    block_hash: H256,
    block: u64,
) -> Result<()> {
    let logs = receipt["logs"]
        .as_array()
        .ok_or_else(|| anyhow!("accounting_logs_missing"))?;
    let mut previous_index = None;
    for log in logs {
        if hash(&log["transactionHash"])? != tx_hash
            || hash(&log["blockHash"])? != block_hash
            || u64_quantity(&log["blockNumber"])? != block
            || log
                .get("removed")
                .is_some_and(|v| v.as_bool() != Some(false))
        {
            bail!("accounting_log_identity_mismatch");
        }
        let index = u64_quantity(&log["logIndex"])?;
        if previous_index.is_some_and(|previous| previous >= index) {
            bail!("accounting_log_order_invalid");
        }
        previous_index = Some(index);
    }
    Ok(())
}

fn event_amounts(receipt: &Value, expected: &EventExpectation) -> Result<(U256, U256)> {
    let arb_signature = H256(keccak256(
        "ArbitrageExecuted(bytes32,address,address,uint256)",
    ));
    let flash_signature = H256(keccak256("FlashLoanExecuted(address,uint256,uint256,bool)"));
    let logs = receipt["logs"]
        .as_array()
        .ok_or_else(|| anyhow!("accounting_logs_missing"))?;
    let mut gross = None;
    let mut premium = None;
    for log in logs {
        let emitter = address(&log["address"])?;
        if emitter != expected.executor && emitter != expected.flash_executor {
            continue;
        }
        let topics = log["topics"]
            .as_array()
            .ok_or_else(|| anyhow!("accounting_topics_missing"))?;
        let Some(first) = topics.first() else {
            continue;
        };
        let signature = hash(first)?;
        if signature != arb_signature && signature != flash_signature {
            continue;
        }
        if topics.len() != 2 {
            bail!("accounting_event_topics_invalid");
        }
        let data = bytes(&log["data"])?;
        if data.len() != 96 {
            bail!("accounting_event_length_invalid");
        }
        if signature == arb_signature {
            if emitter != expected.executor
                || gross.is_some()
                || premium.is_some()
                || hash(&topics[1])? != expected.route_hash
            {
                bail!("accounting_arbitrage_event_mismatch");
            }
            let decoded = decode(
                &[ParamType::Address, ParamType::Address, ParamType::Uint(256)],
                &data,
            )
            .map_err(|_| anyhow!("accounting_arbitrage_event_decode"))?;
            match decoded.as_slice() {
                [Token::Address(token_in), Token::Address(token_out), Token::Uint(amount)]
                    if *token_in == expected.token_in && *token_out == expected.token_out =>
                {
                    // Re-encoding rejects noncanonical address padding accepted
                    // by some ABI decoders.
                    if ethers::abi::encode(&decoded) != data {
                        bail!("accounting_arbitrage_event_noncanonical");
                    }
                    gross = Some(*amount);
                }
                _ => bail!("accounting_arbitrage_asset_mismatch"),
            }
        } else {
            let mut asset_topic = [0_u8; 32];
            asset_topic[12..].copy_from_slice(expected.token_in.as_bytes());
            if emitter != expected.flash_executor
                || premium.is_some()
                || gross.is_none()
                || hash(&topics[1])? != H256(asset_topic)
            {
                bail!("accounting_flash_event_mismatch");
            }
            let decoded = decode(
                &[ParamType::Uint(256), ParamType::Uint(256), ParamType::Bool],
                &data,
            )
            .map_err(|_| anyhow!("accounting_flash_event_decode"))?;
            match decoded.as_slice() {
                [Token::Uint(principal), Token::Uint(fee), Token::Bool(true)]
                    if *principal == expected.principal =>
                {
                    if ethers::abi::encode(&decoded) != data {
                        bail!("accounting_flash_event_noncanonical");
                    }
                    principal
                        .checked_add(*fee)
                        .ok_or_else(|| anyhow!("accounting_repayment_overflow"))?;
                    premium = Some(*fee);
                }
                _ => bail!("accounting_flash_principal_mismatch"),
            }
        }
    }
    match (gross, premium) {
        (Some(gross), Some(premium)) => Ok((gross, premium)),
        _ => bail!("accounting_required_events_missing"),
    }
}

/// Ethereum L1 execution gas plus EIP-4844 blob gas. Other fee models need an
/// explicit adapter; omitting an L2 data fee would overstate realized profit.
fn receipt_gas_cost(chain_id: u64, receipt: &Value, tx: &Value) -> Result<(U256, U256)> {
    if !matches!(chain_id, 1 | 11_155_111) {
        bail!("accounting_fee_model_unavailable");
    }
    let execution = quantity(&receipt["gasUsed"])?
        .checked_mul(quantity(&receipt["effectiveGasPrice"])?)
        .ok_or_else(|| anyhow!("accounting_execution_fee_overflow"))?;
    let tx_type = tx.get("type").map(u64_quantity).transpose()?.unwrap_or(0);
    if !matches!(tx_type, 0..=4) {
        bail!("accounting_transaction_type_unsupported");
    }
    let used = receipt.get("blobGasUsed").filter(|v| !v.is_null());
    let price = receipt.get("blobGasPrice").filter(|v| !v.is_null());
    let blob = match (used, price) {
        (Some(used), Some(price)) => {
            let used = quantity(used)?;
            if tx_type != 3 && !used.is_zero() {
                bail!("accounting_unexpected_blob_fee");
            }
            used.checked_mul(quantity(price)?)
                .ok_or_else(|| anyhow!("accounting_blob_fee_overflow"))?
        }
        (None, None) if tx_type != 3 => U256::zero(),
        _ => bail!("accounting_blob_fee_missing"),
    };
    if receipt
        .get("l1Fee")
        .filter(|v| !v.is_null())
        .map(quantity)
        .transpose()?
        .is_some_and(|f| !f.is_zero())
    {
        bail!("accounting_unexpected_l2_fee");
    }
    let total = execution
        .checked_add(blob)
        .ok_or_else(|| anyhow!("accounting_total_fee_overflow"))?;
    Ok((total, blob))
}

fn units(amount: U256, decimals: u8) -> Result<BigDecimal> {
    if decimals > 77 {
        bail!("accounting_decimals_out_of_range");
    }
    BigDecimal::from_str(&format!("{amount}e-{decimals}"))
        .map_err(|_| anyhow!("accounting_decimal_conversion_failed"))
}

fn net_usd(
    gross: U256,
    premium: U256,
    gas_wei: U256,
    decimals: u8,
    asset: &Price,
    native: &Price,
) -> Result<BigDecimal> {
    Ok(
        (units(gross, decimals)? - units(premium, decimals)?)
            * units(asset.answer, asset.decimals)?
            - units(gas_wei, 18)? * units(native.answer, native.decimals)?,
    )
}

fn finite_decimal(value: &BigDecimal) -> Result<f64> {
    value
        .to_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| anyhow!("accounting_display_value_unrepresentable"))
}

async fn reconcile(item: &PendingAccounting) -> Result<SettlementResult> {
    let opp = &item.opportunity;
    let plan = &item.plan;
    let binding = plan
        .binding
        .as_ref()
        .ok_or_else(|| anyhow!("accounting_binding_missing"))?;
    if binding.schema_version != 1
        || binding.opportunity_id != opp.id
        || binding.chain_id != opp.chain_id
        || binding.strategy_kind != opp.strategy_kind.as_str()
        || binding.caller != plan.ctx.caller
        || binding.flash_loan_executor.is_zero()
        || binding.calldata_hash != H256(keccak256(&plan.wrapped_calldata))
        || binding.state_overrides_used
        || Address::from_str(&opp.token_in).ok() != Some(plan.ctx.token_in)
        || U256::from_dec_str(&opp.amount_in_wei).ok() != Some(plan.ctx.amount_in)
    {
        bail!("accounting_plan_identity_mismatch");
    }
    if !matches!(opp.chain_id, 1 | 11_155_111) {
        bail!("accounting_fee_model_unavailable");
    }
    let rpc = OracleRpc::from_env(opp.chain_id)?;
    let (receipt, tx, chain) = tokio::try_join!(
        rpc.call(
            "eth_getTransactionReceipt",
            json!([format!("{:#x}", item.tx_hash)])
        ),
        rpc.call(
            "eth_getTransactionByHash",
            json!([format!("{:#x}", item.tx_hash)])
        ),
        rpc.call("eth_chainId", json!([])),
    )?;
    if u64_quantity(&chain)? != opp.chain_id || hash(&receipt["transactionHash"])? != item.tx_hash {
        bail!("accounting_receipt_identity_mismatch");
    }
    let block_hash = hash(&receipt["blockHash"])?;
    let block_number = u64_quantity(&receipt["blockNumber"])?;
    if block_hash.is_zero() || block_number == 0 || block_number < binding.block_number {
        bail!("accounting_receipt_block_invalid");
    }
    validate_transaction(
        &tx,
        &TxExpectation {
            hash: item.tx_hash,
            chain_id: opp.chain_id,
            block_hash,
            block_number,
            caller: plan.ctx.caller,
            target: binding.flash_loan_executor,
            calldata: &plan.wrapped_calldata,
        },
    )?;
    validate_receipt_logs(&receipt, item.tx_hash, block_hash, block_number)?;
    let status = u64_quantity(&receipt["status"])?;
    let (gross, premium) = match status {
        1 => event_amounts(
            &receipt,
            &EventExpectation {
                executor: plan.executor_address,
                flash_executor: binding.flash_loan_executor,
                route_hash: H256(plan.route_hash),
                token_in: plan.ctx.token_in,
                token_out: plan.ctx.token_out,
                principal: plan.ctx.amount_in,
            },
        )?,
        0 if receipt["logs"].as_array().is_some_and(Vec::is_empty) => (U256::zero(), U256::zero()),
        _ => bail!("accounting_receipt_status_invalid"),
    };
    let (gas_cost, blob_fee) = receipt_gas_cost(opp.chain_id, &receipt, &tx)?;
    let block = rpc
        .call(
            "eth_getBlockByNumber",
            json!([format!("0x{block_number:x}"), false]),
        )
        .await?;
    if hash(&block["hash"])? != block_hash || u64_quantity(&block["number"])? != block_number {
        bail!("accounting_receipt_not_canonical");
    }
    let block_timestamp = u64_quantity(&block["timestamp"])?;
    let feeds = configured_chain(opp.chain_id)?;
    let asset_key = format!("{:#x}", plan.ctx.token_in);
    let asset_feed = feeds
        .assets_usd
        .get(&asset_key)
        .ok_or_else(|| anyhow!("accounting_asset_usd_feed_missing"))?;
    let (asset_price, native_price, decimals) = tokio::try_join!(
        rpc.price(asset_feed, block_hash, block_timestamp),
        rpc.price(&feeds.native_usd, block_hash, block_timestamp),
        rpc.token_decimals(plan.ctx.token_in, block_hash),
    )?;
    let exact_net = net_usd(
        gross,
        premium,
        gas_cost,
        decimals,
        &asset_price,
        &native_price,
    )?;
    let principal =
        units(plan.ctx.amount_in, decimals)? * units(asset_price.answer, asset_price.decimals)?;
    let principal_usd = finite_decimal(&principal)?;
    if principal_usd <= 0.0 {
        bail!("accounting_principal_invalid");
    }
    let latency = item
        .first_receipt_observed_at_ms
        .checked_sub(opp.detected_at.timestamp_millis())
        .filter(|latency| *latency >= 0)
        .ok_or_else(|| anyhow!("accounting_observation_time_invalid"))?;
    // Check finality and then re-read the canonical receipt height. A reorg
    // between initial receipt fetch and feed reads never enters the history.
    let finalized = rpc
        .call("eth_getBlockByNumber", json!(["finalized", false]))
        .await?;
    let finalized_number = u64_quantity(&finalized["number"])?;
    if hash(&finalized["hash"])?.is_zero() {
        bail!("accounting_finality_unavailable");
    }
    let canonical = rpc
        .call(
            "eth_getBlockByNumber",
            json!([format!("0x{block_number:x}"), false]),
        )
        .await?;
    if hash(&canonical["hash"])? != block_hash
        || u64_quantity(&canonical["number"])? != block_number
    {
        bail!("accounting_receipt_reorganized");
    }
    let source = if finalized_number >= block_number {
        "finalized_receipt"
    } else {
        "included_receipt"
    };
    Ok(SettlementResult {
        observation: RealizedObservation {
            schema_version: 1,
            source: source.into(),
            chain_id: opp.chain_id,
            strategy_kind: opp.strategy_kind.as_str().into(),
            token_in: asset_key,
            tx_hash: format!("{:#x}", item.tx_hash),
            block_hash: format!("{block_hash:#x}"),
            block_number,
            observed_at_ms: item.first_receipt_observed_at_ms,
            principal_usd,
            net_profit_usd: finite_decimal(&exact_net)?,
            latency_ms: latency as f64,
        },
        net_profit_usd_decimal: exact_net.normalized().to_string(),
        gas_cost_wei: gas_cost.to_string(),
        blob_fee_wei: blob_fee.to_string(),
        asset_decimals: decimals,
        asset_price_updated_at: asset_price.updated_at,
        native_price_updated_at: native_price.updated_at,
        execution_status: if status == 1 { "included" } else { "reverted" }.into(),
    })
}

fn job_id(item: &PendingAccounting) -> String {
    format!("{}:{:#x}", item.opportunity.chain_id, item.tx_hash)
}

async fn publish(
    result: &SettlementResult,
    opportunity: &Opportunity,
    redis: &ConnectionManager,
) -> Result<()> {
    let row = &result.observation;
    let mut conn = redis.clone();
    let encoded = serde_json::to_string(result)?;
    let result_key = format!("arbx:accounting:result:{}", opportunity.id);
    let receipt_key = format!("arbx:accounting:receipt:{}:{}", row.chain_id, row.tx_hash);
    if row.source != "finalized_receipt" {
        let _: () = redis::pipe()
            .atomic()
            .cmd("SET")
            .arg(&result_key)
            .arg(&encoded)
            .arg("EX")
            .arg(RESULT_TTL_SECS)
            .ignore()
            .cmd("SET")
            .arg(&receipt_key)
            .arg(&encoded)
            .arg("EX")
            .arg(RESULT_TTL_SECS)
            .ignore()
            .query_async(&mut conn)
            .await?;
        return Ok(());
    }
    let script = redis::Script::new(
        r#"
        local previous = redis.call('GET', KEYS[1])
        if previous and previous ~= ARGV[1] then return -1 end
        if not previous then
            redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[3])
            redis.call('LPUSH', KEYS[2], ARGV[1])
            redis.call('LTRIM', KEYS[2], 0, 127)
            redis.call('EXPIRE', KEYS[2], ARGV[3])
        end
        redis.call('SET', KEYS[3], ARGV[2], 'EX', ARGV[4])
        redis.call('SET', KEYS[4], ARGV[2], 'EX', ARGV[4])
        return 1
    "#,
    );
    let result: i64 = script
        .key(format!(
            "arbx:accounting:seen:{}:{}",
            row.chain_id, row.tx_hash
        ))
        .key(history_key(row.chain_id, &row.strategy_kind, &row.token_in))
        .key(result_key)
        .key(receipt_key)
        .arg(serde_json::to_string(row)?)
        .arg(encoded)
        .arg(HISTORY_TTL_SECS)
        .arg(RESULT_TTL_SECS)
        .invoke_async(&mut conn)
        .await?;
    if result != 1 {
        bail!("accounting_finalized_observation_conflict");
    }
    Ok(())
}

/// Called only after the tracker observes Included/Reverted. Missing RPC,
/// prices, finality, events or identity yields None and a durable retry job.
pub async fn reconcile_and_publish(
    opp: &Opportunity,
    plan: &ValidatedPlan,
    tx_hash: H256,
    redis: &ConnectionManager,
) -> Option<f64> {
    let item = PendingAccounting {
        opportunity: opp.clone(),
        plan: plan.clone(),
        tx_hash,
        first_receipt_observed_at_ms: Utc::now().timestamp_millis(),
    };
    let encoded = serde_json::to_string(&item).ok()?;
    let job = job_id(&item);
    let mut conn = redis.clone();
    // NX preserves the first observation time across repeated inclusion calls.
    let queued: redis::RedisResult<()> = redis::pipe()
        .atomic()
        .cmd("SET")
        .arg(format!("arbx:accounting:input:{job}"))
        .arg(encoded)
        .arg("NX")
        .arg("EX")
        .arg(INPUT_TTL_SECS)
        .ignore()
        .cmd("ZADD")
        .arg(PENDING_QUEUE)
        .arg(Utc::now().timestamp_millis())
        .arg(&job)
        .ignore()
        .query_async(&mut conn)
        .await;
    if let Err(error) = queued {
        tracing::warn!(opportunity_id = %opp.id, error = %error, "receipt accounting enqueue failed");
        return None;
    }
    let stored: Option<String> = redis::cmd("GET")
        .arg(format!("arbx:accounting:input:{job}"))
        .query_async(&mut conn)
        .await
        .ok()?;
    let item: PendingAccounting = serde_json::from_str(stored.as_deref()?).ok()?;
    match tokio::time::timeout(Duration::from_secs(20), reconcile(&item)).await {
        Ok(Ok(result)) => {
            if let Err(error) = publish(&result, opp, redis).await {
                tracing::warn!(opportunity_id = %opp.id, error = %error, "receipt accounting publish failed");
                return None;
            }
            Some(result.observation.net_profit_usd)
        }
        Ok(Err(error)) => {
            tracing::debug!(opportunity_id = %opp.id, reason = %error, "receipt accounting pending");
            None
        }
        Err(_) => {
            tracing::debug!(opportunity_id = %opp.id, "receipt accounting timeout; queued for retry");
            None
        }
    }
}

async fn remove_job(redis: &ConnectionManager, job: &str) {
    let mut conn = redis.clone();
    let _: redis::RedisResult<()> = redis::pipe()
        .atomic()
        .cmd("ZREM")
        .arg(PENDING_QUEUE)
        .arg(job)
        .ignore()
        .cmd("DEL")
        .arg(format!("arbx:accounting:input:{job}"))
        .ignore()
        .query_async(&mut conn)
        .await;
}

async fn retry_job(redis: ConnectionManager, pg: PgPool, job: String) {
    let mut conn = redis.clone();
    let stored: redis::RedisResult<Option<String>> = redis::cmd("GET")
        .arg(format!("arbx:accounting:input:{job}"))
        .query_async(&mut conn)
        .await;
    let item: PendingAccounting = match stored {
        Ok(Some(raw)) => match serde_json::from_str(&raw) {
            Ok(item) => item,
            Err(_) => {
                remove_job(&redis, &job).await;
                return;
            }
        },
        Ok(None) => {
            remove_job(&redis, &job).await;
            return;
        }
        Err(_) => return,
    };
    if job_id(&item) != job {
        remove_job(&redis, &job).await;
        return;
    }
    if let Ok(Ok(result)) = tokio::time::timeout(Duration::from_secs(20), reconcile(&item)).await {
        if publish(&result, &item.opportunity, &redis).await.is_ok()
            && result.observation.source == "finalized_receipt"
        {
            // Numeric binds as text to avoid a binary float or mixed-version
            // BigDecimal codec on the durable monetary column.
            let updated = sqlx::query(
                "UPDATE executions e SET actual_profit_usd = CAST($1 AS NUMERIC) \
                 FROM opportunities o WHERE e.opportunity_id = o.id \
                 AND e.opportunity_id = $2 AND lower(e.tx_hash) = $3 \
                 AND e.status = $4 AND o.chain_id = $5",
            )
            .bind(&result.net_profit_usd_decimal)
            .bind(item.opportunity.id)
            .bind(format!("{:#x}", item.tx_hash))
            .bind(&result.execution_status)
            .bind(item.opportunity.chain_id as i64)
            .execute(&pg)
            .await;
            if updated.is_ok_and(|done| done.rows_affected() > 0) {
                remove_job(&redis, &job).await;
                return;
            }
        }
    }
    let _: redis::RedisResult<()> = redis::cmd("ZADD")
        .arg(PENDING_QUEUE)
        .arg(Utc::now().timestamp_millis() + 30_000)
        .arg(job)
        .query_async(&mut conn)
        .await;
}

/// Start once after both durable stores are available. Bounded batches and
/// concurrency prevent a slow RPC from spawning an unbounded reconciliation fanout.
pub fn start_worker(redis: ConnectionManager, pg: PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let mut conn = redis.clone();
            let jobs: redis::RedisResult<Vec<String>> = redis::cmd("ZRANGEBYSCORE")
                .arg(PENDING_QUEUE)
                .arg("-inf")
                .arg(Utc::now().timestamp_millis())
                .arg("LIMIT")
                .arg(0)
                .arg(16)
                .query_async(&mut conn)
                .await;
            if let Ok(jobs) = jobs {
                stream::iter(jobs)
                    .for_each_concurrent(4, |job| retry_job(redis.clone(), pg.clone(), job))
                    .await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use ethers::abi::encode;

    fn expectation() -> EventExpectation {
        EventExpectation {
            executor: Address::from_low_u64_be(1),
            flash_executor: Address::from_low_u64_be(2),
            route_hash: H256::from_low_u64_be(3),
            token_in: Address::from_low_u64_be(4),
            token_out: Address::from_low_u64_be(5),
            principal: U256::from(1_000_000_u64),
        }
    }

    fn receipt(expected: &EventExpectation) -> Value {
        let mut asset_topic = [0_u8; 32];
        asset_topic[12..].copy_from_slice(expected.token_in.as_bytes());
        json!({"gasUsed":"0x5208", "effectiveGasPrice":"0x3b9aca00", "status":"0x1", "logs":[
            {"address":expected.executor, "topics":[H256(keccak256("ArbitrageExecuted(bytes32,address,address,uint256)")),expected.route_hash],
             "data":format!("0x{}",hex::encode(encode(&[Token::Address(expected.token_in), Token::Address(expected.token_out),Token::Uint(U256::from(100_u64))])))},
            {"address":expected.flash_executor,"topics":[H256(keccak256("FlashLoanExecuted(address,uint256,uint256,bool)")),H256(asset_topic)],
             "data":format!("0x{}",hex::encode(encode(&[Token::Uint(expected.principal),Token::Uint(U256::from(9_u64)),Token::Bool(true)])))}
        ]})
    }

    fn price(answer: u64, decimals: u8) -> Price {
        Price {
            answer: U256::from(answer),
            decimals,
            updated_at: 1_800_000_000,
        }
    }

    #[test]
    fn exact_contract_events_produce_gross_and_premium() -> Result<()> {
        let expected = expectation();
        assert_eq!(
            event_amounts(&receipt(&expected), &expected)?,
            (U256::from(100), U256::from(9))
        );
        Ok(())
    }

    #[test]
    fn spoofed_emitter_does_not_supply_missing_profit() {
        let expected = expectation();
        let mut receipt = receipt(&expected);
        receipt["logs"][0]["address"] = json!(Address::from_low_u64_be(99));
        assert!(event_amounts(&receipt, &expected).is_err());
    }

    #[test]
    fn duplicate_and_reordered_events_are_rejected() -> Result<()> {
        let expected = expectation();
        let mut receipt = receipt(&expected);
        let duplicate = receipt["logs"][0].clone();
        receipt["logs"]
            .as_array_mut()
            .context("logs")?
            .insert(1, duplicate);
        assert!(event_amounts(&receipt, &expected).is_err());
        receipt["logs"].as_array_mut().context("logs")?.remove(1);
        receipt["logs"].as_array_mut().context("logs")?.reverse();
        assert!(event_amounts(&receipt, &expected).is_err());
        Ok(())
    }

    #[test]
    fn wrong_route_asset_principal_and_success_are_rejected() {
        let expected = expectation();
        for mutation in 0..5 {
            let mut receipt = receipt(&expected);
            match mutation {
                0 => receipt["logs"][0]["topics"][1] = json!(H256::zero()),
                1 => receipt["logs"][1]["topics"][1] = json!(H256::zero()),
                2 => {
                    receipt["logs"][0]["data"] = json!(format!(
                        "0x{}",
                        hex::encode(encode(&[
                            Token::Address(expected.token_out),
                            Token::Address(expected.token_in),
                            Token::Uint(U256::from(100))
                        ]))
                    ))
                }
                3 => {
                    receipt["logs"][1]["data"] = json!(format!(
                        "0x{}",
                        hex::encode(encode(&[
                            Token::Uint(expected.principal + 1),
                            Token::Uint(U256::from(9)),
                            Token::Bool(true)
                        ]))
                    ))
                }
                _ => {
                    receipt["logs"][1]["data"] = json!(format!(
                        "0x{}",
                        hex::encode(encode(&[
                            Token::Uint(expected.principal),
                            Token::Uint(U256::from(9)),
                            Token::Bool(false)
                        ]))
                    ))
                }
            }
            assert!(
                event_amounts(&receipt, &expected).is_err(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn malformed_or_noncanonical_abi_is_rejected() -> Result<()> {
        let expected = expectation();
        let mut receipt = receipt(&expected);
        receipt["logs"][0]["data"] = json!("0x00");
        assert!(event_amounts(&receipt, &expected).is_err());
        let mut receipt = super::tests::receipt(&expected);
        let mut data = bytes(&receipt["logs"][0]["data"])?;
        data[0] = 1;
        receipt["logs"][0]["data"] = json!(format!("0x{}", hex::encode(data)));
        assert!(event_amounts(&receipt, &expected).is_err());
        Ok(())
    }

    #[test]
    fn premium_and_paid_gas_can_turn_gross_into_loss() -> Result<()> {
        let result = net_usd(
            U256::from(100),
            U256::from(9),
            U256::exp10(15),
            6,
            &price(1, 0),
            &price(2000, 0),
        )?;
        assert_eq!(result, BigDecimal::from_str("-1.999909")?);
        let reverted = net_usd(
            U256::zero(),
            U256::zero(),
            U256::exp10(15),
            6,
            &price(1, 0),
            &price(2000, 0),
        )?;
        assert_eq!(reverted, BigDecimal::from(-2));
        Ok(())
    }

    #[test]
    fn uint256_above_float_precision_keeps_one_unit_difference() -> Result<()> {
        let raw = U256::one() << 200;
        let result = net_usd(raw + 1, raw, U256::zero(), 6, &price(1, 0), &price(2000, 0))?;
        assert_eq!(result, BigDecimal::from_str("0.000001")?);
        Ok(())
    }

    #[test]
    fn blob_fee_is_included_and_unsupported_fee_models_are_unknown() -> Result<()> {
        let mut receipt = receipt(&expectation());
        receipt["blobGasUsed"] = json!("0x20000");
        receipt["blobGasPrice"] = json!("0xa");
        let (total, blob) = receipt_gas_cost(1, &receipt, &json!({"type":"0x3"}))?;
        assert_eq!(blob, U256::from(1_310_720_u64));
        assert_eq!(total, U256::from(21_000_000_000_000_u64) + blob);
        assert!(receipt_gas_cost(10, &receipt, &json!({"type":"0x3"})).is_err());
        Ok(())
    }

    #[test]
    fn missing_blob_component_and_fee_overflow_are_rejected() {
        let mut receipt = receipt(&expectation());
        assert!(receipt_gas_cost(1, &receipt, &json!({"type":"0x3"})).is_err());
        receipt["blobGasUsed"] = json!("0x20000");
        assert!(receipt_gas_cost(1, &receipt, &json!({"type":"0x3"})).is_err());
        receipt["blobGasPrice"] = json!("0x1");
        assert!(receipt_gas_cost(1, &receipt, &json!({"type":"0x2"})).is_err());
        receipt["gasUsed"] = json!(format!("{:#x}", U256::MAX));
        assert!(receipt_gas_cost(1, &receipt, &json!({"type":"0x3"})).is_err());
    }

    #[test]
    fn full_transaction_identity_binds_receipt_caller_chain_target_and_payload() -> Result<()> {
        let expected = TxExpectation {
            hash: H256::from_low_u64_be(1),
            chain_id: 1,
            block_hash: H256::from_low_u64_be(2),
            block_number: 3,
            caller: Address::from_low_u64_be(4),
            target: Address::from_low_u64_be(5),
            calldata: &[6, 7],
        };
        let tx = json!({"hash":expected.hash,"chainId":"0x1","blockHash":expected.block_hash,"blockNumber":"0x3",
            "from":expected.caller,"to":expected.target,"value":"0x0","input":"0x0607"});
        validate_transaction(&tx, &expected)?;
        for (field, replacement) in [
            ("hash", json!(H256::zero())),
            ("chainId", json!("0xa")),
            ("blockHash", json!(H256::zero())),
            ("blockNumber", json!("0x4")),
            ("from", json!(Address::zero())),
            ("to", json!(Address::zero())),
            ("value", json!("0x1")),
            ("input", json!("0x0608")),
        ] {
            let mut changed = tx.clone();
            changed[field] = replacement;
            assert!(
                validate_transaction(&changed, &expected).is_err(),
                "field {field}"
            );
        }
        Ok(())
    }

    #[test]
    fn log_identity_and_order_reject_removed_or_foreign_receipts() -> Result<()> {
        let tx = H256::from_low_u64_be(1);
        let block = H256::from_low_u64_be(2);
        let base = json!({"logs":[{"transactionHash":tx,"blockHash":block,"blockNumber":"0x3","logIndex":"0x0","removed":false}]});
        validate_receipt_logs(&base, tx, block, 3)?;
        for (field, value) in [
            ("removed", json!(true)),
            ("transactionHash", json!(H256::zero())),
            ("blockHash", json!(H256::zero())),
        ] {
            let mut changed = base.clone();
            changed["logs"][0][field] = value;
            assert!(validate_receipt_logs(&changed, tx, block, 3).is_err());
        }
        let mut repeated = base.clone();
        repeated["logs"]
            .as_array_mut()
            .context("logs")?
            .push(base["logs"][0].clone());
        assert!(validate_receipt_logs(&repeated, tx, block, 3).is_err());
        Ok(())
    }
}
