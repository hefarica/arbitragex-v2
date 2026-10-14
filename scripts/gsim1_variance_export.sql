-- G-SIM-1 item 4 (variance_benchmark) — export REAL recent opportunities
-- for the replay harness (backend/sim-core/tests/variance_benchmark.rs).
--
-- Run ON the VPS against the production DB (see scripts/gsim1_variance_benchmark.sh):
--   docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At \
--     < scripts/gsim1_variance_export.sql > /tmp/gsim1/input.jsonl
--
-- Honesty (RULE 00): every column comes from REAL detections the live scanner
-- persisted. Filters are shape/scope filters, not value filters — no row is
-- edited, imputed or synthesized:
--   * chain 1 (the harness runs the mainnet wrapped-flash path);
--   * 2-leg routes only (the A.3.a encoder supports 2-leg round trips);
--   * legs restricted to the adapters the A.3.a encoder can actually encode
--     (UniswapV2 / SushiSwap — the exact set of
--     `adapter_to_semantic()` in variance_benchmark.rs). V3/PancakeSwap legs
--     are honestly UNSUPPORTED by that encoder, so exporting them only
--     inflated the `unsupported_adapter` skip counter with rows the harness
--     can never label. Scope filter, not a value filter: the V3 population
--     stays fully visible in `opportunities` and is measured separately by the
--     driver (see docs/operations/SIMULATOR_V2_READINESS.md);
--   * last 2 hours — the freshness window the harness can pin: it resolves
--     block B by timestamp bisection inside `tip − 1100` blocks (≈3.7h) and
--     counts anything older as `stale_timestamp`. Widening this window does
--     NOT enlarge the labelable set; it only inflates the dedup/stale skips;
--   * non-zero amount_in.
--
-- G-SIM1-AUTOREFRESH (2026-09-26) — WHY `DISTINCT ON` REPLACED `LIMIT 400`:
-- the scanner re-detects the same handful of hot routes continuously, so an
-- unfiltered 2h window is ~96% duplicate topology (measured: 800 raw rows →
-- 779 dedup → 21 distinct). The old `ORDER BY detected_at DESC LIMIT 400`
-- therefore spent its whole budget on duplicates: the harness deduped them
-- away and labeled the survivors, so the sample size was decided by the
-- duplicate rate, not by the market. This query returns ONE row per distinct
-- route topology (the freshest detection of each), so the export IS the
-- labelable population of the window and the harness' `samples_labeled` is
-- directly comparable to the population size the driver measures.
-- LIMIT 500 is a safety valve on the DISTINCT population, not a sampling cut.

SELECT DISTINCT ON (
         o.dex_a,
         o.token_in,
         o.token_out,
         o.route_metadata->'pool_addresses'
       )
       jsonb_build_object(
         'opportunity_id',   o.id::text,
         'chain_id',         o.chain_id,
         'detected_at_unix', floor(extract(epoch from o.detected_at))::bigint,
         'token_in',         o.token_in,
         'token_out',        o.token_out,
         'dex_a',            o.dex_a,
         'pool_addresses',   o.route_metadata->'pool_addresses',
         'token_addresses',  o.route_metadata->'token_addresses',
         'dex_adapters',     o.route_metadata->'dex_adapters',
         'amount_in_wei',    o.amount_in_wei::text
       )
FROM opportunities o
WHERE o.chain_id = 1
  AND o.route_metadata IS NOT NULL
  AND o.route_metadata::text NOT IN ('', '{}')
  AND o.route_metadata ? 'dex_adapters'
  AND jsonb_array_length(o.route_metadata->'dex_adapters') = 2
  -- A.3.a encoder-supported legs only (mirror of adapter_to_semantic()).
  AND (o.route_metadata->'dex_adapters') <@ '["UniswapV2","SushiSwap"]'::jsonb
  AND o.amount_in_wei IS NOT NULL
  AND o.amount_in_wei <> '0'
  AND o.detected_at > now() - interval '2 hours'
ORDER BY o.dex_a,
         o.token_in,
         o.token_out,
         o.route_metadata->'pool_addresses',
         o.detected_at DESC
LIMIT 500;
