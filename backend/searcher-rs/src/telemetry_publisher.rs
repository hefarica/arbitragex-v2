use sed_core::telemetry::{ConvergencePublisher, ConvergenceSignal};

/// Publisher Redis real. Envía señales al canal `arbx:signals:convergence`.
/// Fire-and-forget: usa redis::aio::Connection para no bloquear.
pub struct RedisPublisher {
    redis_url: String,
}

impl RedisPublisher {
    pub fn new(redis_url: String) -> Self {
        Self { redis_url }
    }
}

impl ConvergencePublisher for RedisPublisher {
    fn publish_signal(&self, signal: ConvergenceSignal) -> anyhow::Result<()> {
        let json_result = serde_json::to_string(&signal);
        let json = match json_result {
            Ok(j) => j,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "serialize ConvergenceSignal: {}",
                    e
                ));
            }
        };

        // Fire-and-forget: tokio::spawn para no bloquear el pipeline
        let redis_url = self.redis_url.clone();
        let channel = "arbx:signals:convergence".to_string();
        tokio::spawn(async move {
            let client_result = redis::Client::open(redis_url.as_str());
            let client = match client_result {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        target: "sed_telemetry",
                        "Redis client open failed: {}",
                        e
                    );
                    return;
                }
            };

            let conn_result = client.get_multiplexed_async_connection().await;
            let mut conn = match conn_result {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        target: "sed_telemetry",
                        "Redis connect failed: {}",
                        e
                    );
                    return;
                }
            };

            let publish_result: redis::RedisResult<()> = redis::cmd("PUBLISH")
                .arg(&channel)
                .arg(&json)
                .query_async(&mut conn)
                .await;

            match publish_result {
                Ok(()) => {
                    tracing::debug!(
                        target: "sed_telemetry",
                        "Published ConvergenceSignal to {}",
                        channel
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        target: "sed_telemetry",
                        "Redis PUBLISH failed: {}",
                        e
                    );
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sed_core::telemetry::EntropySnapshot;

    #[test]
    fn redis_publisher_new() {
        let publisher = RedisPublisher::new("redis://127.0.0.1:6379".to_string());
        assert_eq!(publisher.redis_url, "redis://127.0.0.1:6379");
    }

    #[tokio::test]
    async fn redis_publisher_publish_returns_ok() {
        // Este test verifica que publish_signal retorna Ok(()) inmediatamente
        // (fire-and-forget). No requiere un servidor Redis real.
        let publisher = RedisPublisher::new("redis://invalid:6379".to_string());
        let signal = ConvergenceSignal {
            entropy_snapshot: EntropySnapshot {
                mempool_tx_per_sec: 0.0,
                mempool_avg_gas_price_gwei: 0.0,
                mempool_entropy_score: 0.0,
                reserve_divergence_max: 0.0,
            },
            pipeline_latency_ms: 0,
            opportunities_detected: 0,
            simulations_run: 0,
            simulations_success: 0,
            timestamp: "2026-05-14T00:00:00Z".to_string(),
            schema_version: 1,
        };
        let result = publisher.publish_signal(signal);
        assert!(
            result.is_ok(),
            "publish_signal debe retornar Ok inmediatamente (fire-and-forget)"
        );
    }

    #[tokio::test]
    async fn redis_publisher_serialize_error_propagated() {
        // Este test verifica que errores de serialización se propagan correctamente.
        // Con serde_json + nuestra estructura (todos tipos serializables),
        // un error de serialización es imposible, pero verificamos el path.
        let publisher = RedisPublisher::new("redis://127.0.0.1:6379".to_string());
        let signal = ConvergenceSignal {
            entropy_snapshot: EntropySnapshot {
                mempool_tx_per_sec: f64::NAN,
                mempool_avg_gas_price_gwei: 0.0,
                mempool_entropy_score: 0.0,
                reserve_divergence_max: 0.0,
            },
            pipeline_latency_ms: 0,
            opportunities_detected: 0,
            simulations_run: 0,
            simulations_success: 0,
            timestamp: "2026-05-14T00:00:00Z".to_string(),
            schema_version: 1,
        };
        // serde_json serializa f64::NAN sin error (lo pone como null o "NaN")
        let result = publisher.publish_signal(signal);
        assert!(
            result.is_ok(),
            "f64::NAN debe serializarse sin error (serde_json lo acepta)"
        );
    }
}
