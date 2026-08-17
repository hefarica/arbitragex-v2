-- G-SIM-1 item 4 (variance_benchmark) — export REAL recent opportunities
-- for the replay harness (backend/sim-core/tests/variance_benchmark.rs).
--
-- Run ON the VPS against the production DB (see scripts/gsim1_variance_benchmark.sh):
--   docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At \
--     -f scripts/gsim1_variance_export.sql > /tmp/gsim1/input.jsonl
--
-- Honesty (RULE 00): every column comes from REAL detections the live scanner
-- persisted. Filters are shape/scope filters, not value filters — no row is
-- edited, imputed or synthesized:
--   * chain 1 (the harness runs the mainnet wrapped-flash path);
--   * 2-leg routes only (the A.3.a encoder supports 2-leg round trips —
--     V3 legs reject honestly inside the harness, counted as skips);
--   * last 2 hours — the fork-state window public/full RPCs can serve at a
--     pinned block, and the freshness window the harness bisects into;
--   * non-zero amount_in.
-- LIMIT 400 raw candidates: the harness dedups identical route topologies and
-- must label >= 100 pairs (VARIANCE_MIN_SAMPLES) for a PASS.

SELECT jsonb_build_object(
         'opportunity_id',   o.id::text,
         'chain_id',         o.chain_id,
         'detected_at_unix', floor(extract(epoch from o.detected_at))::bigint,
         'token_in',         o.token_in,
         'token_out',        o.token_out,
         'dex_a',            o.dex_a,
         'pool_addresses',   o.route_metadata->'pool_addresses',
         'token_addresses',  o.route_metadata->'token_addresses',
         'dex_adapters',     o.route_metadata->'dex_adapters',
         'amount_in_wei',    o.amount_in_wei
       )
FROM opportunities o
WHERE o.chain_id = 1
  AND o.route_metadata IS NOT NULL
  AND o.route_metadata::text NOT IN ('', '{}')
  AND o.route_metadata ? 'dex_adapters'
  AND jsonb_array_length(o.route_metadata->'dex_adapters') = 2
  AND o.amount_in_wei IS NOT NULL
  AND o.amount_in_wei <> '0'
  AND o.detected_at > now() - interval '2 hours'
ORDER BY o.detected_at DESC
LIMIT 400;
