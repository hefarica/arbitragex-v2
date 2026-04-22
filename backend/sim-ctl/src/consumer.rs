//! Redis Streams consumer for sim-ctl.
//!
//! Reads `arbx:opps:validated` published by selector-api (S3). For each opp:
//!   1. simulate via sim_engine
//!   2. persist simulation + update opp status
//!   3. if passed → XADD arbx:opps:simulated (downstream for S5)
//!   4. XACK only after persist.

use crate::sim_engine::SimEngine;
use crate::persistence::insert_simulation;
use anyhow::{Context, Result};
use futures_util::StreamExt;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::contracts::Opportunity;
use shared_rs::killswitch::KillSwitchClient;
use sqlx::postgres::PgPool;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const STREAM_IN: &str = "arbx:opps:validated";
const STREAM_OUT: &str = "arbx:opps:simulated";
const GROUP: &str = "sim-ctl-g0";
const STREAM_MAXLEN: usize = 10_000;

pub struct Consumer {
    pub redis: ConnectionManager,
    pub pool: PgPool,
    pub engine: SimEngine,
    pub killswitch: KillSwitchClient,
    pub consumer_name: String,
}

impl Consumer {
    pub async fn run(mut self) -> Result<()> {
        self.ensure_group().await.ok();
        info!(event = "sim_consumer.started", stream = STREAM_IN, group = GROUP, consumer = %self.consumer_name);
        loop {
            if self.killswitch.is_enabled().await {
                debug!(event = "sim_consumer.paused_kill_switch");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            if let Err(e) = self.read_batch().await {
                error!(event = "sim_consumer.read_batch_err", error = %e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    async fn ensure_group(&mut self) -> Result<()> {
        let res: redis::RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE").arg(STREAM_IN).arg(GROUP).arg("$").arg("MKSTREAM")
            .query_async(&mut self.redis).await;
        match res {
            Ok(_) => { info!(event = "sim_consumer.group_created"); Ok(()) }
            Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn read_batch(&mut self) -> Result<()> {
        let res: Option<Vec<redis::Value>> = redis::cmd("XREADGROUP")
            .arg("GROUP").arg(GROUP).arg(&self.consumer_name)
            .arg("COUNT").arg(8)
            .arg("BLOCK").arg(2000)
            .arg("STREAMS").arg(STREAM_IN).arg(">")
            .query_async(&mut self.redis).await?;

        let Some(reply) = res else { return Ok(()); };
        for stream_entry in reply {
            if let redis::Value::Bulk(v) = stream_entry {
                // [stream_name, [[id, [k, v, k, v, ...]], ...]]
                if v.len() != 2 { continue; }
                let entries = match &v[1] {
                    redis::Value::Bulk(e) => e.clone(),
                    _ => continue,
                };
                for e in entries {
                    if let redis::Value::Bulk(parts) = e {
                        if parts.len() != 2 { continue; }
                        let id = match &parts[0] { redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(), _ => continue };
                        let fields = match &parts[1] { redis::Value::Bulk(f) => f.clone(), _ => continue };
                        self.process_message(id, fields).await.ok();
                    }
                }
            }
        }
        Ok(())
    }

    async fn process_message(&mut self, id: String, kv: Vec<redis::Value>) -> Result<()> {
        let json = extract_field(&kv, "json");
        let Some(json) = json else {
            warn!(event = "sim_consumer.invalid_msg_no_json", id=%id);
            let _: () = self.redis.xack::<_,_,&[&str],_>(STREAM_IN, GROUP, &[id.as_str()]).await?;
            return Ok(());
        };
        let opportunity: Opportunity = match serde_json::from_str(&json) {
            Ok(o) => o,
            Err(e) => {
                warn!(event = "sim_consumer.invalid_msg_parse", id=%id, error=%e);
                let _: () = self.redis.xack::<_,_,&[&str],_>(STREAM_IN, GROUP, &[id.as_str()]).await?;
                return Ok(());
            }
        };

        let sim = self.engine.simulate(&opportunity).await;

        // Persist; if it fails, do NOT ack — retry on next iteration.
        if let Err(e) = insert_simulation(&self.pool, &sim).await {
            error!(event = "sim_consumer.persist_err", id=%id, error=%e);
            return Ok(());
        }

        if sim.passed {
            let payload = serde_json::to_string(&opportunity).unwrap_or_default();
            let _: redis::RedisResult<String> = redis::cmd("XADD")
                .arg(STREAM_OUT).arg("MAXLEN").arg("~").arg(STREAM_MAXLEN)
                .arg("*").arg("json").arg(payload)
                .query_async(&mut self.redis).await;
        }

        let _: () = self.redis.xack::<_,_,&[&str],_>(STREAM_IN, GROUP, &[id.as_str()]).await.context("xack")?;
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
