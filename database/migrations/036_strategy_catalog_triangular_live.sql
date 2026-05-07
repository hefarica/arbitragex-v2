-- ArbitrageX v2 — Migration 036: triangular strategy lifecycle scaffold → live
--
-- Promotes the `triangular` strategy from `scaffold` (StrategyKind enum + persistence
-- accept the kind, no emitter) to `live` (TriangularWorker now scans MVP cycles every
-- block and emits opportunities to `arbx:opps:detected` + the `opportunities` table).
--
-- The UI badge on /strategies auto-updates because the lifecycle_status column is
-- the single source of truth read by the frontend (no per-strategy override path).
--
-- Rollback path: if the worker misbehaves, the operator can demote with
--   UPDATE strategy_catalog SET lifecycle_status = 'scaffold' WHERE kind = 'triangular';
-- and the badge reverts without redeploying.
--
-- Idempotent — safe to re-run.

UPDATE strategy_catalog
   SET lifecycle_status = 'live'
 WHERE kind = 'triangular';

-- Sanity: confirm the row exists post-update.
DO $$
DECLARE
  promoted_count INT;
BEGIN
  SELECT COUNT(*) INTO promoted_count
    FROM strategy_catalog
   WHERE kind = 'triangular' AND lifecycle_status = 'live';
  IF promoted_count = 0 THEN
    RAISE WARNING
      'migration 036: no row for kind=triangular found OR not promoted to live; '
      'verify migration 028 + 035 ran first';
  END IF;
END $$;
