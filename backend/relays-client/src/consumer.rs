//! Redis Streams consumer: arbx:opps:simulated -> execute -> persist -> XACK.

use crate::persistence::persist_execution;
use crate::submit_engine::SubmitEngine;
use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::contracts::Opportunity;
use sqlx::postgres::PgPool;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const STREAM: &str = "arbx:opps:simulated";
const GROUP: &str = "relays-client-g0";

pub struct Consumer {
    pub redis: ConnectionManager,
    pub pool: PgPool,
    pub engine: SubmitEngine,
    pub consumer_name: String,
}

impl Consumer {
    pub async fn run(mut self) -> Result<()> {
        self.ensure_group().await.ok();
        info!(event = "relays_consumer.started", stream = STREAM, group = GROUP,
              consumer = %self.consumer_name);
        loop {
            if let Err(e) = self.read_batch().await {
                error!(event = "relays_consumer.batch_err", error = %e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    async fn ensure_group(&mut self) -> Result<()> {
        let res: redis::RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE").arg(STREAM).arg(GROUP).arg("$").arg("MKSTREAM")
            .query_async(&mut self.redis).await;
        match res {
            Ok(_) => { info!(event = "relays_consumer.group_created"); Ok(()) }
            Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn read_batch(&mut self) -> Result<()> {
        let reply: Option<Vec<redis::Value>> = redis::cmd("XREADGROUP")
            .arg("GROUP").arg(GROUP).arg(&self.consumer_name)
            .arg("COUNT").arg(4)
            .arg("BLOCK").arg(2000)
            .arg("STREAMS").arg(STREAM).arg(">")
            .query_async(&mut self.redis).await?;
        let Some(reply) = reply else { return Ok(()); };
        for stream_entry in reply {
            if let redis::Value::Bulk(v) = stream_entry {
                if v.len() != 2 { continue; }
                let entries = match &v[1] { redis::Value::Bulk(e) => e.clone(), _ => continue };
                for e in entries {
                    if let redis::Value::Bulk(parts) = e {
                        if parts.len() != 2 { continue; }
                        let id = match &parts[0] { redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(), _ => continue };
                        let fields = match &parts[1] { redis::Value::Bulk(f) => f.clone(), _ => continue };
                        self.process_one(id, fields).await.ok();
                    }
                }
            }
        }
        Ok(())
    }

    async fn process_one(&mut self, id: String, kv: Vec<redis::Value>) -> Result<()> {
        let Some(json) = extract_field(&kv, "json") else {
            warn!(event = "relays_consumer.no_json", id = %id);
            let _: () = self.redis.xack(STREAM, GROUP, &id).await?;
            return Ok(());
        };
        let opp: Opportunity = match serde_json::from_str(&json) {
            Ok(o) => o,
            Err(e) => {
                warn!(event = "relays_consumer.parse_err", id = %id, error = %e);
                let _: () = self.redis.xack(STREAM, GROUP, &id).await?;
                return Ok(());
            }
        };

        let result = self.engine.execute(&opp).await;
        debug!(event = "relays_consumer.executed", opp = %opp.id, status = ?result.status);

        if let Err(e) = persist_execution(&self.pool, &result, opp.chain_id as i64).await {
            error!(event = "relays_consumer.persist_err", opp = %opp.id, error = %e);
            // Do NOT ack → retry
            return Ok(());
        }

        let _: () = self.redis.xack(STREAM, GROUP, &id).await.context("xack")?;
        Ok(())
    }
}

fn extract_field(kv: &[redis::Value], name: &str) -> Option<String> {
    let mut i = 0;
    while i + 1 < kv.len() {
        let k = match &kv[i] { redis::Value::Data(s) => std::str::from_utf8(s).ok()?.to_string(), _ => return None };
        let v = match &kv[i+1] { redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(), _ => return None };
        if k == name { return Some(v); }
        i += 2;
    }
    None
}
