-- deploy-population-counters.sql — v1.2 (2026-10-20)
-- Contadores de población que sustituyen a `paper_trade_runs` como criterio de
-- muerte de un merge (LEVI 6991955 → corrección aceptada 6992259).
--
-- El predicado se escribe sobre columnas que EXISTEN en el DDL, no sobre el
-- vocabulario del payload (es la lección de C16b: `confidence_state` no existe
-- en el almacén y el predicado quedó no evaluable):
--   * opportunities.dex_b          TEXT NULL      — database/migrations/003_opportunities.sql:11
--   * opportunities.route_metadata JSONB NOT NULL DEFAULT '{}'
--                                   — 099_opportunities_route_metadata.sql:39
--                                     {pool_addresses[], token_addresses[], dex_adapters[], decimals{}}
--   * opportunities.detected_at    TIMESTAMPTZ   — 003:26
--   * opportunities.status         TEXT          — 003:20
-- Lo que no esté ahí se declara NO EVALUABLE; no se traduce.
--
-- NULL no es 0: `dex_b IS NULL` es una pata sin resolver, no «sin arbitraje».
--
-- Uso:
--   psql -v corte='2026-10-20T14:10:00Z' -v ventana='24 hours' \
--        [-v silencio_max=3600] -f scripts/deploy-population-counters.sql
--
-- Reglas de veredicto (la cuarta máscara del 0):
--   poblacion = 0                => NOT COMPUTED, nunca PASS
--   poblacion > 0, presentes = 0 => FAIL
--   presentes > 0                => PASS sólo si nulos = 0; si no, FAIL parcial
--
-- Sólo lee: transacción READ ONLY con ROLLBACK. Un contador no escribe en producción.

\set ON_ERROR_STOP on
\if :{?corte}
\else
  \echo 'falta -v corte=<ISO-8601 UTC>'
  \quit 3
\endif
\if :{?ventana}
\else
  \echo 'falta -v ventana=<intervalo, p.ej. "24 hours">'
  \quit 3
\endif
\if :{?silencio_max}
\else
  \set silencio_max 3600
\endif

BEGIN;
SET TRANSACTION READ ONLY;
SET LOCAL TimeZone = 'UTC';

\echo 'ventana:' :'ventana' '| corte:' :'corte' '| umbral de silencio:' :silencio_max 's'

WITH v AS (
  SELECT :'corte'::timestamptz AS corte, :'ventana'::interval AS ventana
),
pob AS (
  SELECT o.id, o.detected_at, o.status, o.dex_b, o.rejection_reason, o.route_metadata
  FROM opportunities o, v
  WHERE o.detected_at >  v.corte - v.ventana
    AND o.detected_at <= v.corte
),
-- K1: topología 2-leg resuelta (la columna que existe).
k1 AS (
  SELECT 'K1' AS contador,
         'opportunities.dex_b no nulo (2-leg resuelta)' AS que_mide,
         count(*) AS poblacion,
         count(*) FILTER (WHERE dex_b IS NOT NULL AND btrim(dex_b) <> '') AS presentes,
         count(*) FILTER (WHERE dex_b IS NULL OR btrim(dex_b) = '') AS nulos,
         NULL::bigint AS silencio_s
  FROM pob
),
-- K2: metadatos de ruta almacenados = análogo almacenado de D-12.
k2 AS (
  SELECT 'K2' AS contador,
         'route_metadata.pool_addresses y .decimals no vacíos' AS que_mide,
         count(*) AS poblacion,
         count(*) FILTER (
           WHERE jsonb_typeof(route_metadata->'pool_addresses') = 'array'
             AND jsonb_array_length(route_metadata->'pool_addresses') > 0
             AND jsonb_typeof(route_metadata->'decimals') = 'object'
             AND route_metadata->'decimals' <> '{}'::jsonb
         ) AS presentes,
         count(*) FILTER (WHERE route_metadata IS NULL OR route_metadata = '{}'::jsonb) AS nulos,
         NULL::bigint AS silencio_s
  FROM pob
),
-- K3a: silencio del escritor, en segundos desde el último INSERT de la ventana.
-- No se cuenta filas: contar filas de la ventana y llamarlas «presentes» sería
-- un PASS por construcción.
k3a AS (
  SELECT 'K3a' AS contador,
         'silencio del escritor: corte - max(detected_at), segundos' AS que_mide,
         count(*) AS poblacion,
         count(*) FILTER (WHERE detected_at > (SELECT corte FROM v)) AS presentes,
         COALESCE(EXTRACT(EPOCH FROM ((SELECT corte FROM v) - max(detected_at)))::bigint, -1) AS nulos,
         COALESCE(EXTRACT(EPOCH FROM ((SELECT corte FROM v) - max(detected_at)))::bigint, -1) AS silencio_s
  FROM pob
),
-- K3b: inserciones acumuladas de pg_stat. NO es por ventana: una sola lectura
-- no dice si el escritor está vivo. Se publica para que dos lecturas den el delta.
k3b AS (
  SELECT 'K3b' AS contador,
         'pg_stat n_tup_ins ACUMULADO (delta = lectura2 - lectura1)' AS que_mide,
         (SELECT n_tup_ins FROM pg_stat_user_tables WHERE relname = 'opportunities') AS poblacion,
         (SELECT n_tup_upd FROM pg_stat_user_tables WHERE relname = 'opportunities') AS presentes,
         (SELECT n_tup_del FROM pg_stat_user_tables WHERE relname = 'opportunities') AS nulos,
         NULL::bigint AS silencio_s
),
-- K4 = primaria del pre-registro ratificado por MANTIS (6992516): filas
-- almacenadas con `dex_b` no nulo Y `dex_adapters` no vacío. `dex_adapters` no
-- es columna: vive en route_metadata->'dex_adapters' (099:17).
k4 AS (
  SELECT 'K4' AS contador,
         'dex_b no nulo Y route_metadata.dex_adapters no vacío (primaria)' AS que_mide,
         count(*) AS poblacion,
         count(*) FILTER (
           WHERE dex_b IS NOT NULL AND btrim(dex_b) <> ''
             AND jsonb_typeof(route_metadata->'dex_adapters') = 'array'
             AND jsonb_array_length(route_metadata->'dex_adapters') > 0
         ) AS presentes,
         count(*) FILTER (
           WHERE dex_b IS NULL OR btrim(dex_b) = ''
              OR jsonb_typeof(route_metadata->'dex_adapters') IS DISTINCT FROM 'array'
              OR jsonb_array_length(COALESCE(route_metadata->'dex_adapters', '[]'::jsonb)) = 0
         ) AS nulos,
         NULL::bigint AS silencio_s
  FROM pob
)
SELECT contador, que_mide, poblacion, presentes, nulos,
       -- Tercer estado explícito: hay filas ni presentes ni nulas (metadatos
       -- parciales, p.ej. pool_addresses sí y decimals no). Sin esta columna
       -- los números «no cuadran» y alguien los leería como error de conteo.
       CASE WHEN contador IN ('K1','K2','K4') THEN poblacion - presentes - nulos END AS parciales,
       CASE
         WHEN contador = 'K3b' THEN
           'INSTRUMENTO ACUMULADO: no da veredicto en una lectura; comparar dos'
         WHEN contador = 'K3a' THEN
           CASE WHEN poblacion = 0 THEN 'NOT COMPUTED: población 0 en la ventana'
                WHEN silencio_s < 0 THEN 'NOT COMPUTED: sin detected_at en la ventana'
                WHEN silencio_s <= :silencio_max THEN format('PASS: último insert hace %ss (umbral %ss)', silencio_s, :silencio_max)
                ELSE format('FAIL: escritor callado %ss (umbral %ss)', silencio_s, :silencio_max) END
         WHEN poblacion = 0 THEN 'NOT COMPUTED: población 0 en la ventana'
         WHEN presentes = 0 THEN format('FAIL: %s/%s presentes (población entera en el defecto)', presentes, poblacion)
         WHEN nulos = 0 THEN format('PASS: %s/%s presentes, 0 nulos', presentes, poblacion)
         ELSE format('FAIL parcial: %s/%s presentes, %s nulos', presentes, poblacion, nulos)
       END AS veredicto
FROM (SELECT * FROM k1 UNION ALL SELECT * FROM k2 UNION ALL
      SELECT * FROM k3a UNION ALL SELECT * FROM k3b UNION ALL
      SELECT * FROM k4) t;

-- Aceptación post-hot-sim en una fila: ¿hay ALGUNA oportunidad admitible?
WITH v AS (
  SELECT :'corte'::timestamptz AS corte, :'ventana'::interval AS ventana
)
SELECT 'ACEPTACION' AS contador,
       count(DISTINCT o.id) FILTER (WHERE o.dex_b IS NOT NULL AND btrim(o.dex_b) <> '') AS admitibles_2leg,
       count(DISTINCT o.id) AS poblacion,
       CASE WHEN count(DISTINCT o.id) = 0 THEN 'NOT COMPUTED'
            WHEN count(DISTINCT o.id) FILTER (WHERE o.dex_b IS NOT NULL AND btrim(o.dex_b) <> '') = 0 THEN 'FAIL'
            ELSE 'PASS' END AS veredicto
FROM opportunities o, v
WHERE o.detected_at >  v.corte - v.ventana
  AND o.detected_at <= v.corte;

-- EXITO del pre-registro: ≥1 `executed` con `actual_profit_usd > 0` Y
-- reconciliación > 0. Dos columnas del pre-registro NO existen con ese nombre y
-- se traducen aquí, declarado:
--   * `paper_trade_runs` no tiene columna `status` (verificado en 051/091/100/111/126)
--     => el `executed` se lee en opportunities.status (assumption de MANTIS 6992516).
--   * `recon.total` no existe: profit_reconciliation (024) tiene
--     `realized_profit_usd`. Se publican count(*) y sum(realized_profit_usd).
-- Potencia: < 3 candidatas 2-leg => INCONCLUSO, nunca FAIL.
WITH v AS (
  SELECT :'corte'::timestamptz AS corte, :'ventana'::interval AS ventana
),
cand AS (
  SELECT count(*) AS n
  FROM opportunities o, v
  WHERE o.detected_at >  v.corte - v.ventana
    AND o.detected_at <= v.corte
    AND o.dex_b IS NOT NULL AND btrim(o.dex_b) <> ''
    AND jsonb_typeof(o.route_metadata->'dex_adapters') = 'array'
    AND jsonb_array_length(o.route_metadata->'dex_adapters') > 0
),
ejec AS (
  SELECT count(DISTINCT p.opportunity_id) AS con_profit
  FROM paper_trade_runs p
  JOIN opportunities o ON o.id = p.opportunity_id
  CROSS JOIN v
  WHERE p.created_at > v.corte - v.ventana AND p.created_at <= v.corte
    AND o.status = 'executed'
    AND p.actual_profit_usd IS NOT NULL AND p.actual_profit_usd > 0
),
rec AS (
  SELECT count(DISTINCT r.opportunity_id) AS oportunidades,
         COALESCE(sum(r.realized_profit_usd), 0) AS realizado
  FROM profit_reconciliation r CROSS JOIN v
  WHERE r.created_at > v.corte - v.ventana AND r.created_at <= v.corte
)
SELECT 'EXITO' AS contador,
       (SELECT n FROM cand) AS candidatas_2leg,
       (SELECT con_profit FROM ejec) AS ejecutadas_con_profit,
       (SELECT oportunidades FROM rec) AS recon_oportunidades,
       (SELECT realizado FROM rec) AS recon_realizado,
       CASE WHEN (SELECT n FROM cand) < 3 THEN 'INCONCLUSO: <3 candidatas 2-leg (potencia)'
            WHEN (SELECT con_profit FROM ejec) = 0 THEN 'FAIL: ninguna executed con actual_profit_usd > 0'
            WHEN (SELECT oportunidades FROM rec) = 0 THEN 'FAIL: ejecutada sin reconciliación'
            ELSE 'PASS' END AS veredicto;

ROLLBACK;
