-- Migration 092: route_discovery_outcomes — shadow eval outcome sink (Fase B Paso 2)
--
-- Durable Postgres table for the shadow outcomes the searcher emits to the Redis
-- stream `arbx:route_discovery:outcomes` (Fase B Paso 1, commit e977ab0). The Redis
-- stream is capped (XADD MAXLEN ~1_000_000 → trims oldest after ~6 days at the
-- current rate); this table preserves the FULL >=2-week series required to measure
-- hit-rate (Gate C). Consumer: api-server `route-discovery-outcomes-archiver`
-- (XREADGROUP/XACK, idempotent via the Redis stream entry id in `stream_id`).
--
-- NO-ACTIVE: this table records observe-only shadow evaluations. No FK to capital
-- tables, no executor, no opps:detected. arbx-no-hardcode: no operator literals.

BEGIN;

CREATE TABLE IF NOT EXISTS route_discovery_outcomes (
    id                BIGSERIAL    PRIMARY KEY,
    stream_id         TEXT         NOT NULL UNIQUE,        -- Redis entry id → idempotent ingest
    schema_ver        TEXT         NOT NULL,
    ts_ms             BIGINT       NOT NULL,
    chain_id          INTEGER      NOT NULL,
    cartridge_id      TEXT         NOT NULL,
    tx_hash           TEXT,
    source_event      TEXT,
    pool_hint         TEXT,
    token_in          TEXT,
    token_out         TEXT,
    is_opportunity    BOOLEAN      NOT NULL,
    estimated_profit  NUMERIC(38, 18),
    confidence        NUMERIC(18, 9),
    urgency           TEXT,
    had_reserves      BOOLEAN      NOT NULL,
    mode              TEXT         NOT NULL,
    inserted_at       TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- Time-series scans (latest first).
CREATE INDEX IF NOT EXISTS idx_rd_outcomes_ts
    ON route_discovery_outcomes(ts_ms DESC);
-- Hit-rate aggregation by chain + cartridge + opportunity flag.
CREATE INDEX IF NOT EXISTS idx_rd_outcomes_opp
    ON route_discovery_outcomes(chain_id, cartridge_id, is_opportunity, ts_ms DESC);
-- Reserve-coverage analysis (had_reserves=false is the data-starvation tail).
CREATE INDEX IF NOT EXISTS idx_rd_outcomes_reserves
    ON route_discovery_outcomes(had_reserves, ts_ms DESC);

DO $$
BEGIN
    RAISE NOTICE 'Migration 092: route_discovery_outcomes created (Fase B Paso 2 shadow outcomes sink).';
END $$;

COMMIT;
