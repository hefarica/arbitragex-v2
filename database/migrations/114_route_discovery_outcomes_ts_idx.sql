-- RDO-SUMMARY-HANG (2026-08-31): the /api/v1/route-discovery-outcomes/summary
-- aggregations all filter on ts_ms, but no index LED with ts_ms — the existing
-- idx_rdo_chain_ts (chain_id, ts_ms) and idx_rdo_opportunity (is_opportunity,
-- ts_ms) cannot serve a bare `ts_ms >= $1` range. Every poll therefore
-- seq-scanned the whole table (26M rows / 12 GB at diagnosis, ~1.36M rows/h).
--
-- Created CONCURRENTLY on the live VPS at 2026-08-31 (17.8s, no write
-- blocking); this tracked form keeps fresh environments and rebuilds in sync
-- (IF NOT EXISTS makes the already-migrated VPS a no-op).
CREATE INDEX IF NOT EXISTS idx_rdo_ts ON route_discovery_outcomes (ts_ms);
