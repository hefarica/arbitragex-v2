-- Migration 096: pool-enumeration discovery metadata (idempotent, non-destructive).
--
-- Stamped by the multi-source PoolEnumerationWorker after a pool is persisted:
-- the source's TVL / 24h volume (ranking metadata only, never profit math) and
-- the discovery timestamp. Builds on 095 (enum_source). Safe to run twice.

ALTER TABLE pools ADD COLUMN IF NOT EXISTS tvl_usd            NUMERIC;
ALTER TABLE pools ADD COLUMN IF NOT EXISTS volume_usd_24h     NUMERIC;
ALTER TABLE pools ADD COLUMN IF NOT EXISTS last_enumerated_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_pools_last_enumerated
    ON pools (last_enumerated_at DESC);
