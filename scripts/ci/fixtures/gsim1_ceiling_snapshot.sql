-- GSIM-COVERAGE-CEILING-01: ONE statement, so every count shares one now().
WITH w AS (
  SELECT DISTINCT ON (o.dex_a, o.token_in, o.token_out, o.route_metadata->'pool_addresses')
         o.strategy_kind AS kind,
         o.route_metadata->'dex_adapters' AS adapters,
         (o.amount_in_wei IS NOT NULL AND o.amount_in_wei <> '0') AS amount_nonzero
  FROM opportunities o
  WHERE o.chain_id = 1
    AND o.route_metadata IS NOT NULL
    AND o.route_metadata ? 'dex_adapters'
    AND o.detected_at > now() - interval '2 hours'
  ORDER BY o.dex_a, o.token_in, o.token_out, o.route_metadata->'pool_addresses', o.detected_at DESC
)
SELECT jsonb_array_length(adapters) AS legs,
       kind AS kind,
       adapters::text AS adapters_json,
       amount_nonzero AS amount_nonzero,
       count(*) AS n
FROM w
GROUP BY 1, 2, 3, 4
ORDER BY 5 DESC, 1;
