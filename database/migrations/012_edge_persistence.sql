-- ============================================================
-- MIGRACIÓN 012: Edge Persistence Layer
-- Proyecto OMEGA — Nodo de Borde
-- ============================================================

-- Tabla de oportunidades detectadas (fase 1-2 del SED)
CREATE TABLE IF NOT EXISTS sed_opportunities (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    detected_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    block_number    BIGINT NOT NULL,
    tx_trigger_hash TEXT NOT NULL,

    -- Tokens involucrados (normalizados)
    token_in        TEXT NOT NULL,
    token_out       TEXT NOT NULL,

    -- Métricas de entrada (f64 normalizados)
    amount_in       DOUBLE PRECISION NOT NULL,
    expected_out    DOUBLE PRECISION NOT NULL,
    price_impact    DOUBLE PRECISION NOT NULL,

    -- Estado del pipeline SED
    phase           SMALLINT NOT NULL CHECK (phase BETWEEN 1 AND 6),
    status          TEXT NOT NULL CHECK (status IN ('PENDING', 'SIMULATING', 'SELECTED', 'FUNDED', 'EXECUTED', 'RECONCILED', 'REJECTED')),

    -- Métricas de rendimiento (microsegundos)
    latency_detect_us   BIGINT,
    latency_simulate_us BIGINT,
    latency_total_us    BIGINT,

    -- Resultado de simulación (dry-run)
    simulation_gas_used BIGINT,
    simulation_success  BOOLEAN,
    simulation_revert_reason TEXT,

    -- Resultado de ejecución (paper-shadow: siempre NULL en producción hasta S8)
    execution_tx_hash   TEXT,
    execution_gas_cost  DOUBLE PRECISION,
    execution_net_pnl   DOUBLE PRECISION,

    -- Metadatos
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    edge_node_id    TEXT NOT NULL DEFAULT 'edge-001'
);

CREATE INDEX idx_sed_opportunities_status ON sed_opportunities(status);
CREATE INDEX idx_sed_opportunities_phase ON sed_opportunities(phase);
CREATE INDEX idx_sed_opportunities_detected_at ON sed_opportunities(detected_at DESC);
CREATE INDEX idx_sed_opportunities_block ON sed_opportunities(block_number DESC);

-- Tabla de métricas de entropía (series temporales)
CREATE TABLE IF NOT EXISTS sed_entropy_metrics (
    id              BIGSERIAL PRIMARY KEY,
    sampled_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    edge_node_id    TEXT NOT NULL,

    -- Métricas del mempool
    mempool_tx_per_sec      DOUBLE PRECISION,
    mempool_avg_gas_price   DOUBLE PRECISION,
    mempool_entropy_score   DOUBLE PRECISION,  -- Shannon entropy de distribución de gas

    -- Métricas de reserves
    reserve_update_freq_hz  DOUBLE PRECISION,
    reserve_divergence_max  DOUBLE PRECISION,  -- Máxima divergencia entre DEXes

    -- Métricas del SED
    sed_opportunities_per_min   DOUBLE PRECISION,
    sed_simulation_success_rate DOUBLE PRECISION,
    sed_avg_pipeline_latency_ms DOUBLE PRECISION,

    -- Estado del nodo
    cpu_usage_percent   DOUBLE PRECISION,
    memory_usage_mb     DOUBLE PRECISION,
    network_io_mbps     DOUBLE PRECISION
);

CREATE INDEX idx_entropy_sampled_at ON sed_entropy_metrics(sampled_at DESC);

-- Tabla de auditoría de kill-switch
CREATE TABLE IF NOT EXISTS kill_switch_audit (
    id              BIGSERIAL PRIMARY KEY,
    triggered_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    edge_node_id    TEXT NOT NULL,
    reason          TEXT NOT NULL,
    action_taken    TEXT NOT NULL,
    resolved_at     TIMESTAMPTZ,
    resolved_by     TEXT
);

-- Trigger para updated_at automático
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_sed_opportunities_updated_at
    BEFORE UPDATE ON sed_opportunities
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
