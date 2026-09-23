-- Optional, bounded, explicit operator execution. No schema/config mutation.
-- Counts are diagnostics, NOT proof of delivery. Export same-event receipts separately.
BEGIN TRANSACTION READ ONLY;
SET LOCAL statement_timeout = '5s';
SELECT version() AS database_version, current_timestamp AS observed_at;
SELECT name, setting FROM pg_settings
 WHERE name IN ('fsync','synchronous_commit','full_page_writes');
SELECT id, chain_id, strategy_kind, detected_at, amount_in_wei,
       expected_profit_usd, net_expected_profit_usd, roi_pct,
       risk_score, block_number, rejection_reason
  FROM opportunities
 WHERE detected_at > current_timestamp - INTERVAL '60 seconds'
 ORDER BY detected_at DESC LIMIT 50;
COMMIT;
