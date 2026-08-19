-- 107_pool_cycles.sql
--
-- RU-1 (cartridge-math-264, Task 1): durable home for the DYNAMIC route
-- universe. `route_discovery::cycle_enumerator` (searcher-rs) enumerates
-- closed 2-3 hop cycles from the live `pools` graph and upserts them here
-- (ON CONFLICT DO NOTHING — idempotent). `ImpactIndex` boots `pool_to_cycles`
-- from this table; the `MVP_CYCLES` constant remains only as the cold-boot
-- seed while the table is empty.
--
-- Representation: token_path/pool_path are OPEN cycles (no closing repeat) in
-- canonical rotation (starting at the lexicographically smallest token
-- address), lowercase 0x-hex. Both traversal directions of a cycle are
-- distinct rows. Additive + idempotent: safe to re-run.

CREATE TABLE IF NOT EXISTS pool_cycles (
    cycle_id     BIGSERIAL PRIMARY KEY,
    chain_id     INT NOT NULL,
    token_path   TEXT[] NOT NULL,
    pool_path    TEXT[] NOT NULL,
    direction    SMALLINT NOT NULL DEFAULT 1,
    active       BOOLEAN NOT NULL DEFAULT TRUE,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (chain_id, token_path, pool_path, direction)
);
