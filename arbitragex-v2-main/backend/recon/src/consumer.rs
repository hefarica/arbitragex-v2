//! Redis Streams consumer: arbx:opps:executed → PnlEngine → persist → XACK.

use crate::persistence::{insert_risk_event, persist_recon_report};
use crate::pnl_engine;
use crate::variance;
use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::config::ReconCfg;
use shared_rs::contracts::{ExecutionResult, Opportunity};
use shared_rs::rpc_failover::AlloyHttpProvider;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const STREAM: &str = "arbx:opps:executed";
const GROUP: &str = "recon-g0";

pub struct Consumer {
    pub redis: ConnectionManager,
    pub pool: PgPool,
    pub provider: Arc<AlloyHttpProvider>,
    pub cfg: ReconCfg,
    pub consumer_name: String,
}

impl Consumer {
    pub async fn run(mut self) -> Result<()> {
        self.ensure_group().await.ok();
        info!(event = "recon_consumer.started", stream = STREAM, group = GROUP, consumer = %self.consumer_name);
        loop {
            if let Err(e) = self.read_batch().await {
                error!(event = "recon_consumer.batch_err", error = %e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    async fn ensure_group(&mut self) -> Result<()> {
        let res: redis::RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(STREAM)
            .arg(GROUP)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut self.redis)
            .await;
        match res {
            Ok(_) => {
                info!(event = "recon_consumer.group_created");
                Ok(())
            }
            Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn read_batch(&mut self) -> Result<()> {
        let reply: Option<Vec<redis::Value>> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(GROUP)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(4)
            .arg("BLOCK")
            .arg(2000)
            .arg("STREAMS")
            .arg(STREAM)
            .arg(">")
            .query_async(&mut self.redis)
            .await?;
        let Some(reply) = reply else {
            return Ok(());
        };
        for stream_entry in reply {
            if let redis::Value::Bulk(v) = stream_entry {
                if v.len() != 2 {
                    continue;
                }
                let entries = match &v[1] {
                    redis::Value::Bulk(e) => e.clone(),
                    _ => continue,
                };
                for e in entries {
                    if let redis::Value::Bulk(parts) = e {
                        if parts.len() != 2 {
                            continue;
                        }
                        let id = match &parts[0] {
                            redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(),
                            _ => continue,
                        };
                        let fields = match &parts[1] {
                            redis::Value::Bulk(f) => f.clone(),
                            _ => continue,
                        };
                        self.process_one(id, fields).await.ok();
                    }
                }
            }
        }
        Ok(())
    }

    async fn process_one(&mut self, id: String, kv: Vec<redis::Value>) -> Result<()> {
        let Some(json) = extract_field(&kv, "json") else {
            warn!(event = "recon_consumer.no_json", id = %id);
            let _: () = self
                .redis
                .xack::<_, _, &str, ()>(STREAM, GROUP, &[id.as_str()])
                .await?;
            return Ok(());
        };
        #[derive(serde::Deserialize)]
        struct Combined {
            opportunity: Opportunity,
            execution: ExecutionResult,
        }
        let combined: Combined = match serde_json::from_str(&json) {
            Ok(c) => c,
            Err(e) => {
                warn!(event = "recon_consumer.parse_err", id = %id, error = %e);
                let _: () = self
                    .redis
                    .xack::<_, _, &str, ()>(STREAM, GROUP, &[id.as_str()])
                    .await?;
                return Ok(());
            }
        };

        let report = pnl_engine::compute(
            &combined.opportunity,
            &combined.execution,
            self.provider.as_ref(),
            self.cfg.receipt_fetch_timeout_ms,
        )
        .await;
        debug!(event = "recon_consumer.computed", opp = %combined.opportunity.id,
               variance_pct = ?report.variance_pct, fail = ?report.fail_reason);

        // Persist the report (even on fail, to record the attempt).
        if let Err(e) = persist_recon_report(&self.pool, &report).await {
            error!(event = "recon_consumer.persist_err", opp = %combined.opportunity.id, error = %e);
            return Ok(()); // don't ack; retry
        }

        // Variance check → risk_event
        if let Some(evt) = variance::check(&report, self.cfg.variance_threshold_pct) {
            // B0.3 (2026-05-13): pass chain_id from the underlying opportunity
            // so the event is traceable per-chain in risk_events table.
            let evt_chain_id = combined.opportunity.chain_id as i64;
            if let Err(e) = insert_risk_event(
                &self.pool,
                &evt.event_type,
                &evt.severity,
                evt.payload,
                evt.trace_id,
                evt.opportunity_id,
                Some(evt_chain_id),
            )
            .await
            {
                warn!(event = "recon_consumer.risk_event_err", error = %e);
            }
        }

        let _: () = self
            .redis
            .xack::<_, _, &str, ()>(STREAM, GROUP, &[id.as_str()])
            .await
            .context("xack")?;
        Ok(())
    }
}

fn extract_field(kv: &[redis::Value], name: &str) -> Option<String> {
    let mut i = 0;
    while i + 1 < kv.len() {
        let k = match &kv[i] {
            redis::Value::Data(s) => std::str::from_utf8(s).ok()?.to_string(),
            _ => return None,
        };
        let v = match &kv[i + 1] {
            redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(),
            _ => return None,
        };
        if k == name {
            return Some(v);
        }
        i += 2;
    }
    None
}
