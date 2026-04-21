//! Real publisher: XADD to Redis Stream `arbx:opps:detected` with MAXLEN trim.

use anyhow::Context;
use redis::AsyncCommands;
use shared_rs::contracts::Opportunity;

pub const STREAM_KEY: &str = "arbx:opps:detected";
pub const STREAM_MAXLEN: usize = 10_000;

pub async fn publish(
    redis: &mut redis::aio::ConnectionManager,
    opp: &Opportunity,
) -> anyhow::Result<()> {
    let json = serde_json::to_string(opp).context("serialize opportunity")?;
    // XADD arbx:opps:detected MAXLEN ~ 10000 * json <payload>
    // Using a raw command because redis-rs high-level doesn't expose MAXLEN on xadd yet.
    let _: String = redis::cmd("XADD")
        .arg(STREAM_KEY)
        .arg("MAXLEN")
        .arg("~")
        .arg(STREAM_MAXLEN)
        .arg("*")
        .arg("json")
        .arg(&json)
        .query_async(redis)
        .await
        .context("XADD opps.detected")?;
    Ok(())
}
