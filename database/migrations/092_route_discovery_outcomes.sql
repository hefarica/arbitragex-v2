-- ArbitrageX v2 — Migration 092: route_discovery_outcomes (FASE B Paso 2)
--
-- Durable sink for the shadow outcomes stream `arbx:route_discovery:outcomes`
-- (emitted by the Rust searcher in shadow mode, Paso 1). A Redis stream with
-- MAXLEN ~ 1_000_000 trims after ~6 days at ~7k/h; this table is the durable
-- ≥2-week series for the Gate C hit-rate. Copies REAL rd_outcome_v1 fields only
-- (Zero-Mocks: nothing computed/derived). stream_id UNIQUE → idempotent
-- at-least-once consume. Additive + idempotent: safe to re-run.

BEGIN;

CREATE TABLE IF NOT EXISTS route_discovery_outcomes (
    id               BIGSERIAL PRIMARY KEY,
    stream_id        TEXT NOT NULL UNIQUE,
    ts_ms            BIGINT NOT NULL,
    schema_ver       TEXT NOT NULL,
    chain_id         BIGINT NOT NULL,
    cartridge_id     TEXT NOT NULL,
    tx_hash          TEXT NOT NULL,
    source_event     TEXT,
    pool_hint        TEXT,
    token_in         TEXT,
    token_out        TEXT,
    is_opportunity   BOOLEAN NOT NULL,
    estimated_profit DOUBLE PRECISION NOT NULL,
    confidence       DOUBLE PRECISION NOT NULL,
    urgency          TEXT,
    had_reserves     BOOLEAN NOT NULL,
    mode             TEXT NOT NULL,
    inserted_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Access patterns for the Gate C hit-rate analysis:
--   hit-rate por venue/cadena en una ventana temporal.
CREATE INDEX IF NOT EXISTS idx_rdo_chain_ts
    ON route_discovery_outcomes(chain_id, ts_ms);
CREATE INDEX IF NOT EXISTS idx_rdo_opportunity
    ON route_discovery_outcomes(is_opportunity, ts_ms);
CREATE INDEX IF NOT EXISTS idx_rdo_pool_hint
    ON route_discovery_outcomes(pool_hint)
    WHERE pool_hint IS NOT NULL AND pool_hint <> '';

DO $$
BEGIN
    RAISE NOTICE 'Migration 092: route_discovery_outcomes ready for the shadow outcomes consumer.';
END $$;

COMMIT;
