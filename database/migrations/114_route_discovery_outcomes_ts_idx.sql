-- 114_route_discovery_outcomes_ts_idx.sql
--
-- RDO-SUMMARY-HANG (2026-08-31): the /api/v1/route-discovery-outcomes/summary
-- aggregations all filter on ts_ms, but no index LED with ts_ms — the existing
-- idx_rdo_chain_ts (chain_id, ts_ms) and idx_rdo_opportunity (is_opportunity,
-- ts_ms) cannot serve a bare `ts_ms >= $1` range. Every poll therefore
-- seq-scanned the whole table (26M rows / 12 GB at diagnosis, ~1.36M rows/h).
--
-- Built CONCURRENTLY on the live VPS at 2026-08-31 (17.8s, INSERTs never
-- blocked) — so on prod this file is a NO-OP (IF NOT EXISTS); its job is to
-- keep fresh environments and rebuilds in sync, in the doctrine form
-- (lint-migration-index-locks.sh: CONCURRENTLY on an already-populated table,
-- precedent 105). Recovery if a CONCURRENT build ever fails midway (leaves an
-- INVALID index): DROP INDEX CONCURRENTLY IF EXISTS idx_rdo_ts; then re-run
-- this file.
--
-- statement_timeout raised past the runner default: a CONCURRENT build only
-- takes SHARE locks that don't conflict with INSERTs (FREEZE-01 doctrine).
SET statement_timeout = '15min';

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_rdo_ts
  ON route_discovery_outcomes (ts_ms);
