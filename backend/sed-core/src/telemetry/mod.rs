use serde::{Deserialize, Serialize};

/// Señal de convergencia emitida por el SED engine al final de cada ciclo de evaluación.
/// Serializa a JSON y se publica en Redis en el canal `arbx:signals:convergence`.
/// El backend Node.js (api-server) retransmite por WebSocket al frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceSignal {
    /// Snapshot de entropía del mempool en el momento de evaluación
    pub entropy_snapshot: EntropySnapshot,
    /// Latencia total del pipeline SED en milisegundos
    pub pipeline_latency_ms: u64,
    /// Oportunidades detectadas en esta ventana
    pub opportunities_detected: u64,
    /// Simulaciones ejecutadas
    pub simulations_run: u64,
    /// Simulaciones exitosas (que pasaron todos los gates)
    pub simulations_success: u64,
    /// Timestamp ISO8601 UTC
    pub timestamp: String,
    /// Versión del schema para compatibilidad futura
    pub schema_version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropySnapshot {
    /// Transacciones por segundo en mempool
    pub mempool_tx_per_sec: f64,
    /// Precio promedio de gas en Gwei
    pub mempool_avg_gas_price_gwei: f64,
    /// Score de entropía del sistema (0.0 = orden, 1.0 = caos)
    pub mempool_entropy_score: f64,
    /// Máxima divergencia de reservas entre DEXes
    pub reserve_divergence_max: f64,
}

/// Trait para publishers de convergencia. Permite que sed-core defina la telemetría
/// mientras que el crate ejecutable (searcher-rs) provee la conexión Redis real.
pub trait ConvergencePublisher: Send + Sync {
    /// Publica un ConvergenceSignal en el canal Redis. Debe ser fire-and-forget:
    /// nunca bloquear el pipeline principal. Retorna Ok(()) si se encoló correctamente.
    fn publish_signal(&self, signal: ConvergenceSignal) -> anyhow::Result<()>;
}

/// Publisher "no-op" para testing y pipelines sin Redis.
/// Logs la señal con tracing::debug pero no publica.
pub struct NoOpPublisher;

impl ConvergencePublisher for NoOpPublisher {
    fn publish_signal(&self, signal: ConvergenceSignal) -> anyhow::Result<()> {
        tracing::debug!(
            target: "sed_telemetry",
            "NoOpPublisher: signal would be published: {:?}",
            signal
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn convergence_signal_serializes_to_json() {
        let signal = ConvergenceSignal {
            entropy_snapshot: EntropySnapshot {
                mempool_tx_per_sec: 12.5,
                mempool_avg_gas_price_gwei: 25.3,
                mempool_entropy_score: 0.78,
                reserve_divergence_max: 0.04,
            },
            pipeline_latency_ms: 145,
            opportunities_detected: 3,
            simulations_run: 3,
            simulations_success: 2,
            timestamp: "2026-05-14T00:00:00Z".to_string(),
            schema_version: 1,
        };
        let json_result = serde_json::to_string(&signal);
        assert!(
            json_result.is_ok(),
            "ConvergenceSignal debe serializar a JSON"
        );
        let json_str = match json_result {
            Ok(s) => s,
            Err(_) => return,
        };
        assert!(
            json_str.contains("entropy_snapshot"),
            "JSON debe contener entropy_snapshot"
        );
        assert!(
            json_str.contains("mempool_entropy_score"),
            "JSON debe contener mempool_entropy_score"
        );
        assert!(
            json_str.contains("0.78"),
            "JSON debe contener el valor 0.78"
        );
    }

    #[test]
    fn noop_publisher_does_not_panic() {
        let publisher = NoOpPublisher;
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
            timestamp: Utc::now().to_rfc3339(),
            schema_version: 1,
        };
        let result = publisher.publish_signal(signal);
        assert!(result.is_ok(), "NoOpPublisher debe retornar Ok");
    }

    #[test]
    fn entropy_snapshot_default_zeros() {
        // R8 Fail-Honest: si no hay datos de entropía, los campos son 0.0
        let snapshot = EntropySnapshot {
            mempool_tx_per_sec: 0.0,
            mempool_avg_gas_price_gwei: 0.0,
            mempool_entropy_score: 0.0,
            reserve_divergence_max: 0.0,
        };
        assert!((snapshot.mempool_tx_per_sec - 0.0).abs() < f64::EPSILON);
        assert!((snapshot.mempool_avg_gas_price_gwei - 0.0).abs() < f64::EPSILON);
        assert!((snapshot.mempool_entropy_score - 0.0).abs() < f64::EPSILON);
        assert!((snapshot.reserve_divergence_max - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn convergence_signal_schema_version_1() {
        let signal = ConvergenceSignal {
            entropy_snapshot: EntropySnapshot {
                mempool_tx_per_sec: 1.0,
                mempool_avg_gas_price_gwei: 1.0,
                mempool_entropy_score: 0.5,
                reserve_divergence_max: 0.01,
            },
            pipeline_latency_ms: 100,
            opportunities_detected: 1,
            simulations_run: 1,
            simulations_success: 1,
            timestamp: "2026-05-14T12:00:00Z".to_string(),
            schema_version: 1,
        };
        assert_eq!(signal.schema_version, 1);
        let json_result = serde_json::to_string(&signal);
        assert!(json_result.is_ok());
        let json_str = match json_result {
            Ok(s) => s,
            Err(_) => return,
        };
        assert!(
            json_str.contains("schema_version"),
            "JSON debe contener schema_version"
        );
        assert!(
            json_str.contains('"'),
            "JSON debe contener al menos un campo serializado"
        );
    }
}
