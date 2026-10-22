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
--   * POBLACION-FILTER-01 (2026-10-23): token path of exactly 3 entries
--     (round-trip shape [token_in, intermediate, token_in]). The harness's own
--     per-row gate (backend/sim-core/tests/variance_benchmark.rs, `bad_shape`
--     skip) requires dex_adapters.len() == 2 AND token_addresses.len() == 3,
--     but this export only checked the first half, so rows with a malformed
--     token path were exported and then skipped by the harness (or, when the
--     token_addresses key was missing entirely, the JSONL line carried
--     `"token_addresses": null` and the harness PANICKED on deserialization —
--     one bad row killed the whole benchmark run). The export IS the labelable
--     population of the window: a row the sim itself cannot process must not
--     occupy a slot of it (nor of the LIMIT 500 / DISTINCT budget);
--   * legs restricted to the adapters the A.3.a encoder can actually encode,
--     compared as the EXACT set of `adapter_to_semantic()` in
--     variance_benchmark.rs: trim + case-sensitive match against the six
--     spellings UniswapV2 | uniswap-v2 | uniswapv2 | SushiSwap | sushi |
--     sushiswap. The pre-POBLACION-FILTER-01 mirror compared lower(leg) IN
--     (four lowercase tokens), a case-insensitive SUPERSET: spellings like
--     'UNISWAP-V2' passed the export and were then skipped by the harness as
--     unsupported_adapter — the same population-contract violation this PR
--     fixes for the token path. The six spellings keep ADAPTER-CASE-01's
--     lesson intact (kebab, CamelCase AND plain lowercase all pass; V3 and
--     PancakeSwap legs stay honestly UNSUPPORTED by that encoder, so exporting
--     them only inflated the `unsupported_adapter` skip counter with rows the
--     harness can never label — the V3 population stays fully visible in
--     `opportunities` and is measured separately by the driver, see
--     docs/operations/SIMULATOR_V2_READINESS.md);
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
  -- POBLACION-FILTER-01 (2026-10-23): the harness's own shape gate is
  --   dex_adapters.len() == 2 AND token_addresses.len() == 3
  -- (variance_benchmark.rs `bad_shape` skip). This export previously mirrored
  -- only the first half, so malformed token paths were exported and then
  -- skipped by the sim (or panicked the harness when the key was missing —
  -- jsonb_array_length(NULL) is NULL, so this predicate also excludes rows
  -- whose route_metadata has NO token_addresses key at all).
  AND jsonb_array_length(o.route_metadata->'token_addresses') = 3
  -- A.3.a encoder-supported legs only (mirror of adapter_to_semantic()).
  -- ADAPTER-CASE-01 (2026-10-20): the previous filter was
  --   (route_metadata->'dex_adapters') <@ '["UniswapV2","SushiSwap"]'::jsonb
  -- i.e. a case-sensitive EXACT match against CamelCase, while the scanner
  -- persists kebab-case ('uniswap-v2', 'uniswap-v3', 'universal-router'). A REAL
  -- 2-leg UniswapV2 route was therefore dropped here and the evidence row kept
  -- saying "0 distinct topologies" — an instrument lying, not an empty market.
  -- adapter_to_semantic() accepts UniswapV2 | uniswap-v2 | uniswapv2 and
  -- SushiSwap | sushi | sushiswap (trim + case-sensitive match), so the EXACT
  -- mirror is those six spellings, compared as written. The pre-2026-10-23
  -- mirror (lower(leg) IN four lowercase tokens) accepted every casing that
  -- lowercases to those tokens — a superset that exported rows the harness
  -- itself skips as unsupported_adapter; same population-contract violation
  -- as the token-path half fixed above.
  AND (
    SELECT bool_and(trim(leg) IN ('UniswapV2', 'uniswap-v2', 'uniswapv2',
                                 'SushiSwap', 'sushi', 'sushiswap'))
    FROM jsonb_array_elements_text(o.route_metadata->'dex_adapters') AS legs(leg)
  )
  AND o.amount_in_wei IS NOT NULL
  AND o.amount_in_wei <> '0'
  AND o.detected_at > now() - interval '2 hours'
ORDER BY o.dex_a,
         o.token_in,
         o.token_out,
         o.route_metadata->'pool_addresses',
         o.detected_at DESC
LIMIT 500;
