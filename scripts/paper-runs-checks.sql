-- paper-runs-checks.sql — C01..C16b del contrato docs/contrato-telemetria.md v1.6
-- v1.6: +C16b (D-10a, valor almacenado; el payload se mide en scripts/check-live-contract.py C16a).
-- v1.4.1: poblacion cero => NOT COMPUTED en C01-C06/C09 (contrato 9.4); C06 cortado por dia (day, no created_at).
-- v1.4: C15/C15b (gas modelado vs medido) y C06 distingue n NULL de n = 0.
-- v1.6.4 (2026-10-20 13:5xZ, ronda de A1): tres reglas de la revisión, ninguna de criterio.
--   (1) C15 publica el INVARIANTE `pares_en_p95 + pares_con_medido_cero = pares` y la
--       cláusula de exclusión sólo se imprime cuando el contador es > 0 (Rachel 6989686).
--   (2) C15b: `deriva_no_cuadra = 0` con `gas_drift_pct` sin escritor es falso verde =>
--       NOT COMPUTED con `con_gas_drift_pct` publicado (MANTIS 6989694).
--   (3) C10, C12b y C13 (listados sin veredicto): en denominador 0 publican fila
--       `NOT COMPUTED` con `filas_ventana`, en vez de quedar en silencio (MANTIS 6989694).
--   El intermediario de esta ronda fue v1.6.3 (`73fd71d3…`, sólo punto 1): no llegó a
--   commit y queda superado por este fichero.
-- v1.6.3 (2026-10-20 13:4xZ): C15 publica `pares_en_p95` y `pares_con_medido_cero`; el veredicto nombra
--   el denominador real (defecto de Rachel 6989586: `d` excluia medido = 0 y `pares` no).
-- v1.6.2 (2026-10-20 13:1xZ): C15 reescrito como DECISION vs MEDIDO, sobre columnas que
--   existen hoy: `opportunities.economics->>'gas_usd'` (migracion 126, ALWAYS-COMPUTE;
--   searcher-rs/src/economics.rs:120,153-157) contra `paper_trade_runs.actual_gas_cost_usd`.
--   NO entra el `6+4*hops` de quant/layers.ts (modelo de PANEL) ni `sim_gas_cost_usd`
--   (gross-net = coste total). Evidencia LEVI 6987463, verificada en 4873c53.
-- v1.6.1: C15 quedo en NOT COMPUTED por el objeto de coste (pesimista: el objeto existe).
-- Autor: Jeffrey Lambert · 2026-10-20 · READ-ONLY: una sola transaccion, cero DDL, cero escrituras.
--
-- Uso:
--   psql "$DATABASE_URL" -v cutoff=2026-10-20T02:00:00Z -f scripts/paper-runs-checks.sql
--   El valor va DESNUDO: en SQL se usa :'cutoff', que ya pone las comillas.
--   Si se pasa -v cutoff="'...'" el literal llega con comillas dentro y Postgres falla al parsear.
--
-- Publicar junto al resultado: cutoff_utc, window_hours y sha256 del export.
-- Reglas (contrato §3 / §9): NULL != 0 · un hueco es NOT COMPUTED, nunca 0 · un check en rojo
-- no se promedia: se publica con su fila de ejemplo · un check VACUO se publica VACUO, no verde.

\set ON_ERROR_STOP on
BEGIN TRANSACTION READ ONLY;

-- C01 · duplicados de escritura por oportunidad (el grano es evento, no oportunidad)
SELECT 'C01' AS check_id, 'A' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) AS oportunidades_con_mas_de_una_fila,
       CASE WHEN (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0
              THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) = 0 THEN 'OK'
            ELSE 'WARN: usar count(DISTINCT opportunity_id) en el denominador' END AS verdict
FROM (SELECT opportunity_id FROM paper_trade_runs
      WHERE created_at >= :'cutoff' AND opportunity_id IS NOT NULL
      GROUP BY 1 HAVING count(*) > 1) d;

-- C02 · join sin clave: huerfanas por retencion vs sin padre real
SELECT 'C02' AS check_id, 'R' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) FILTER (WHERE p.opportunity_id IS NULL) AS opportunity_id_null,
       count(*) FILTER (WHERE p.opportunity_id IS NOT NULL AND o.id IS NULL) AS sin_padre,
       CASE WHEN count(*) = 0 THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) FILTER (WHERE p.opportunity_id IS NOT NULL AND o.id IS NULL) = 0
            THEN 'OK' ELSE 'ROJO: join roto' END AS verdict
FROM paper_trade_runs p
LEFT JOIN opportunities o ON o.id = p.opportunity_id
WHERE p.created_at >= :'cutoff';

-- C03 · cero ambiguo en sim_expected_profit_usd (D-2: archiver omite la fila, relays escribe 0.0)
SELECT 'C03' AS check_id, 'A' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) AS ceros_en_sim_expected_profit,
       CASE WHEN (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0
              THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) = 0 THEN 'OK'
            ELSE 'AMBIGUO: 0 medido y hueco disfrazado no son distinguibles hoy' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff' AND sim_expected_profit_usd = 0;

-- C04 · structural con etiqueta de calibracion (111:17-23: debe ser FALSE)
SELECT 'C04' AS check_id, 'R' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) AS structural_elegible,
       CASE WHEN (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0 THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) = 0 THEN 'OK' ELSE 'ROJO: contaminaria Stage 2b' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff' AND sim_fail_family = 'structural' AND calibration_eligible;

-- C05a · resuelta SIN profit medido. Unica condicion que el brazo PASS promete
--        (resolved_update_sql, drift_tracker.rs:210-218: actual_profit_usd = $2).
--        R: una fila terminal que no se puede etiquetar no es un label.
SELECT 'C05a' AS check_id, 'R' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) AS resueltas_sin_profit,
       CASE WHEN (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0 THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) = 0 THEN 'OK' ELSE 'ROJO: resolucion sin medicion de profit' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff' AND actual_timestamp IS NOT NULL AND actual_profit_usd IS NULL;

-- C05b · resuelta con 0 y SIN clase. El brazo ECONOMIC/MARKET escribe 0.0 Y familia
--        en la misma sentencia (:228-236); un 0 sin clase o es un rechazo sin etiqueta
--        o es un pass de profit exactamente 0 (legitimo). No distinguible => AMBIGUO.
SELECT 'C05b' AS check_id, 'A' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) AS ceros_sin_clase,
       CASE WHEN (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0
              THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) = 0 THEN 'OK'
            ELSE 'AMBIGUO: 0 sin familia (rechazo sin etiqueta o pass a 0)' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff' AND actual_timestamp IS NOT NULL
      AND actual_profit_usd = 0 AND sim_fail_family IS NULL;

-- C05c · resuelta con profit != 0 y CON clase. El brazo PASS nunca escribe familia (§5.1):
--        una clase sobre un pass es contradiccion productor<->contrato, no un nulo legitimo.
SELECT 'C05c' AS check_id, 'A' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) AS clase_sobre_pass,
       CASE WHEN (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0 THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) = 0 THEN 'OK' ELSE 'WARN: familia escrita sobre profit != 0' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff' AND actual_timestamp IS NOT NULL
      AND actual_profit_usd <> 0 AND sim_fail_family IS NOT NULL;

-- C06 · rollup: 0 sin medicion (COALESCE(sum(actual_profit_usd),0), 116:139-140)
--        VENTANA: el rollup tiene grano dia; se corta por `day` (no tiene created_at)
--        y su poblacion es `dias_en_ventana`: 0 defectos sobre 0 dias no es verde.
SELECT 'C06' AS check_id, 'R' AS sev,
       (SELECT count(*) FROM paper_trade_runs_daily
         WHERE day >= ((:'cutoff')::timestamptz AT TIME ZONE 'UTC')::date) AS dias_en_ventana,
       count(*) AS dias_con_suma_sin_n,
       count(*) FILTER (WHERE actual_profit_n IS NULL) AS de_ellos_n_nulo,
       CASE WHEN (SELECT count(*) FROM paper_trade_runs_daily
                   WHERE day >= ((:'cutoff')::timestamptz AT TIME ZONE 'UTC')::date) = 0
              THEN 'NOT COMPUTED: sin dias del rollup en la ventana'
            WHEN count(*) = 0 THEN 'OK'
            ELSE 'ROJO: 0 no respaldado por actual_profit_n (' ||
                 count(*) FILTER (WHERE actual_profit_n IS NULL) || ' de ' || count(*) || ' con n NULO)' END AS verdict
FROM paper_trade_runs_daily
-- COALESCE y no `= 0`: un n NULL tampoco respalda una suma distinta de 0, y
-- `actual_profit_n = 0` no lo ve (NULL no es igual a 0). La columna de arriba
-- separa el hueco (NULL) del cero medido: son dos causas, no una.
-- VENTANA: el rollup tiene grano dia; se corta por `day`, no por created_at (no existe).
WHERE day >= ((:'cutoff')::timestamptz AT TIME ZONE 'UTC')::date
  AND COALESCE(actual_profit_n, 0) = 0 AND actual_profit_sum <> 0;

-- C07 · VACUO mientras opportunity_id sea NOT NULL: count de nulos es 0 por DDL.
--        Lee el catalogo para no poder mentir: si la columna es NOT NULL, es VACUO, no OK.
--        Tras el arreglo de D-1 (nullable) este check empieza a poder fallar. El vivo hoy es C07b.
WITH c7 AS (
  SELECT (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
         (SELECT count(*) FROM paper_trade_runs
           WHERE created_at >= :'cutoff' AND opportunity_id IS NULL) AS nulos,
         (SELECT is_nullable FROM information_schema.columns
           WHERE table_schema = 'public' AND table_name = 'paper_trade_runs'
             AND column_name = 'opportunity_id') AS nullable
)
SELECT 'C07' AS check_id, 'VACUO' AS sev, filas_ventana, nullable, nulos,
       CASE WHEN nullable IS NULL THEN 'NOT COMPUTED: columna ausente'
            WHEN nullable = 'NO' THEN 'VACUO: NOT NULL por DDL hace el caso imposible; C07b es el vivo'
            WHEN nulos > 0 THEN 'ROJO: nulos con la columna ya nullable'
            ELSE 'OK' END AS verdict
FROM c7;

-- C07b · contradiccion DDL viva: NOT NULL (051:18) + FK ON DELETE SET NULL (116:84).
--        Si la FK no existe, NOT COMPUTED: no es "OK".
SELECT 'C07b' AS check_id, 'R' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       c.is_nullable, con.conname AS fk, con.confdeltype,
       CASE WHEN c.is_nullable IS NULL THEN 'NOT COMPUTED: columna ausente'
            WHEN con.conname IS NULL THEN 'NOT COMPUTED: FK paper_trade_runs_opportunity_id_fkey ausente'
            WHEN c.is_nullable = 'NO' AND con.confdeltype = 'n'
              THEN 'CONTRADICCION: NOT NULL no soporta SET NULL'
            WHEN c.is_nullable = 'YES' AND con.confdeltype = 'n' THEN 'OK: nullable + SET NULL'
            ELSE 'REVISAR: combinacion no prevista' END AS verdict
FROM information_schema.columns c
LEFT JOIN pg_constraint con
       ON con.conname = 'paper_trade_runs_opportunity_id_fkey'
      AND con.conrelid = 'paper_trade_runs'::regclass
WHERE c.table_schema = 'public' AND c.table_name = 'paper_trade_runs'
  AND c.column_name = 'opportunity_id';

-- C08 · frescura: sin filas NO es verde, es NOT COMPUTED
SELECT 'C08' AS check_id, 'R' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       max(created_at) AS ultima_fila,
       now() - max(created_at) AS edad,
       CASE WHEN max(created_at) IS NULL THEN 'NOT COMPUTED: tabla vacia (bucle parado)'
            WHEN now() - max(created_at) > interval '60 minutes' THEN 'AMARILLO: sin filas en 60 min'
            ELSE 'OK' END AS verdict
FROM paper_trade_runs;

-- C09 · pendientes terminales sin clasificar de gas
SELECT 'C09' AS check_id, 'A' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) AS pendientes_terminales_sin_estado,
       CASE WHEN (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0 THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) = 0 THEN 'OK' ELSE 'WARN: clasificar con el backfill de 126' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff' AND actual_timestamp IS NULL
      AND NOT calibration_eligible AND gas_measurement_state IS NULL;

-- C10 · cobertura de gas por estado sobre la ventana (incluye no resueltas: fotografia del ledger)
SELECT 'C10' AS check_id, COALESCE(gas_measurement_state, '(NULL)') AS gas_measurement_state,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) AS filas,
       -- v1.6.3: un listado sin veredicto sobre denominador vacio no puede quedar en
       -- silencio (no imprime fila y parece que no hay nada que mirar). Publica la fila.
       CASE WHEN (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0
              THEN 'NOT COMPUTED: sin filas en ventana (listado sin veredicto)' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff'
GROUP BY 1, 2
UNION ALL
SELECT 'C10', '(sin filas)', 0, 0, 'NOT COMPUTED: sin filas en ventana (listado sin veredicto)'
WHERE (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0
ORDER BY 1, 2;

-- ---------------------------------------------------------------------------
-- Clasificador: UNA sola tabla de marcadores, en el orden del productor
-- (sim_taxonomy.rs:113-131: ECONOMIC -> MARKET -> STRUCTURAL -> default structural).
-- Se repite literal en C11 y C12: si cambia la taxonomia, cambian los dos.
-- `familia_calc IS NULL` = la razon NO tenia marcador y cayo al default fail-closed.
-- ---------------------------------------------------------------------------

-- C11 · familia escrita != classify_fail_reason(reason)
WITH marcadores(familia, patron, ord) AS (
  VALUES
    ('economic','%negative_net_profit%',1),('economic','%non_positive_profit%',2),
    ('economic','%slippage_too_high%',3),('economic','%price_impact%',4),
    ('economic','%insufficient_liquidity%',5),('economic','%gas_unprofitable%',6),
    ('economic','%min_profit_gate%',7),('economic','%min_profit%',8),
    ('market','%route_revert%',9),('market','%state_changed%',10),
    ('market','%pool_state_invalid%',11),('market','%deadline%',12),
    ('market','%token_behavior%',13),('market','%stale_state%',14),
    ('structural','%signer_balance_missing%',15),('structural','%allowance_missing%',16),
    ('structural','%transfer_from_failed%',17),('structural','%fork_unavailable%',18),
    ('structural','%archive_rpc_failed%',19),('structural','%gas_oracle_missing%',20),
    ('structural','%candidate_incomplete%',21),('structural','%route_encoding_missing%',22),
    ('structural','%decimals_missing%',23),('structural','%backend_not_configured%',24),
    ('structural','%real_sim_unavailable%',25),('structural','%real_sim_env_missing%',26),
    ('structural','%gas_price_unavailable%',27),('structural','%gas_price_read_failed%',28),
    ('structural','%gas_price_wei%',29),('structural','%b2c_encode_failed%',30),
    ('structural','%b2c_spawn_blocking_join%',31),('structural','%route_metadata_not_found%',32),
    ('structural','%missing_opportunity_id%',33),('structural','%a3_unavailable%',34),
    ('structural','%amount_in%',35),('structural','%missing decimals%',36),
    ('structural','%missing route%',37)
),
cls AS (
  SELECT p.id, p.reason, p.sim_fail_family,
         COALESCE(m.familia, 'structural') AS familia_calc,
         m.familia AS marcador_familia
  FROM paper_trade_runs p
  LEFT JOIN LATERAL (
    SELECT mm.familia FROM marcadores mm
    WHERE lower(p.reason) LIKE mm.patron
    ORDER BY mm.ord LIMIT 1
  ) m ON true
  WHERE p.created_at >= :'cutoff' AND p.reason IS NOT NULL AND p.sim_fail_family IS NOT NULL
)
SELECT 'C11' AS check_id, 'R' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       count(*) AS filas_clasificables,
       count(*) FILTER (WHERE sim_fail_family IS DISTINCT FROM familia_calc) AS familias_discrepantes,
       CASE WHEN count(*) = 0 THEN 'NOT COMPUTED: sin filas con reason y familia'
            WHEN count(*) FILTER (WHERE sim_fail_family IS DISTINCT FROM familia_calc) = 0 THEN 'OK'
            ELSE 'ROJO: productor y taxonomia discrepan' END AS verdict
FROM cls;

-- C12 · la clase que C11 no puede ver: razones DISTINTAS que cayeron al default
--        fail-closed (sim_taxonomy.rs:130-131). Una razon nueva aqui entra en
--        structural -> calibration_eligible=FALSE -> fuera de calibracion para siempre (D-7).
WITH marcadores(familia, patron, ord) AS (
  VALUES
    ('economic','%negative_net_profit%',1),('economic','%non_positive_profit%',2),
    ('economic','%slippage_too_high%',3),('economic','%price_impact%',4),
    ('economic','%insufficient_liquidity%',5),('economic','%gas_unprofitable%',6),
    ('economic','%min_profit_gate%',7),('economic','%min_profit%',8),
    ('market','%route_revert%',9),('market','%state_changed%',10),
    ('market','%pool_state_invalid%',11),('market','%deadline%',12),
    ('market','%token_behavior%',13),('market','%stale_state%',14),
    ('structural','%signer_balance_missing%',15),('structural','%allowance_missing%',16),
    ('structural','%transfer_from_failed%',17),('structural','%fork_unavailable%',18),
    ('structural','%archive_rpc_failed%',19),('structural','%gas_oracle_missing%',20),
    ('structural','%candidate_incomplete%',21),('structural','%route_encoding_missing%',22),
    ('structural','%decimals_missing%',23),('structural','%backend_not_configured%',24),
    ('structural','%real_sim_unavailable%',25),('structural','%real_sim_env_missing%',26),
    ('structural','%gas_price_unavailable%',27),('structural','%gas_price_read_failed%',28),
    ('structural','%gas_price_wei%',29),('structural','%b2c_encode_failed%',30),
    ('structural','%b2c_spawn_blocking_join%',31),('structural','%route_metadata_not_found%',32),
    ('structural','%missing_opportunity_id%',33),('structural','%a3_unavailable%',34),
    ('structural','%amount_in%',35),('structural','%missing decimals%',36),
    ('structural','%missing route%',37)
),
cls AS (
  SELECT p.reason, p.sim_fail_family,
         COALESCE(m.familia, 'structural') AS familia_calc,
         (m.familia IS NULL) AS es_default
  FROM paper_trade_runs p
  LEFT JOIN LATERAL (
    SELECT mm.familia FROM marcadores mm
    WHERE lower(p.reason) LIKE mm.patron
    ORDER BY mm.ord LIMIT 1
  ) m ON true
  WHERE p.created_at >= :'cutoff' AND p.reason IS NOT NULL AND p.sim_fail_family IS NOT NULL
)
SELECT 'C12' AS check_id, 'A' AS sev,
       count(*) FILTER (WHERE sim_fail_family = 'structural' AND NOT es_default) AS structural_known,
       count(DISTINCT reason) FILTER (WHERE es_default) AS razones_no_reconocidas,
       count(*) FILTER (WHERE es_default) AS structural_unrecognized,
       count(*) AS filas_clasificables,
       CASE WHEN count(*) = 0 THEN NULL
            ELSE round(100.0 * count(*) FILTER (WHERE es_default) / count(*), 2) END AS pct_unrecognized,
       CASE WHEN count(*) = 0 THEN 'NOT COMPUTED: sin filas con reason y familia'
            WHEN count(*) FILTER (WHERE es_default) = 0 THEN 'OK: ninguna razon cayo al default'
            ELSE 'WARN: hay razones fuera de la taxonomia, invisibles para C11 (dispara D-7 a P1)' END AS verdict
FROM cls;

-- C12b · el detalle de C12 (si C12 da 0, este result set es vacio a proposito)
WITH marcadores(familia, patron, ord) AS (
  VALUES
    ('economic','%negative_net_profit%',1),('economic','%non_positive_profit%',2),
    ('economic','%slippage_too_high%',3),('economic','%price_impact%',4),
    ('economic','%insufficient_liquidity%',5),('economic','%gas_unprofitable%',6),
    ('economic','%min_profit_gate%',7),('economic','%min_profit%',8),
    ('market','%route_revert%',9),('market','%state_changed%',10),
    ('market','%pool_state_invalid%',11),('market','%deadline%',12),
    ('market','%token_behavior%',13),('market','%stale_state%',14),
    ('structural','%signer_balance_missing%',15),('structural','%allowance_missing%',16),
    ('structural','%transfer_from_failed%',17),('structural','%fork_unavailable%',18),
    ('structural','%archive_rpc_failed%',19),('structural','%gas_oracle_missing%',20),
    ('structural','%candidate_incomplete%',21),('structural','%route_encoding_missing%',22),
    ('structural','%decimals_missing%',23),('structural','%backend_not_configured%',24),
    ('structural','%real_sim_unavailable%',25),('structural','%real_sim_env_missing%',26),
    ('structural','%gas_price_unavailable%',27),('structural','%gas_price_read_failed%',28),
    ('structural','%gas_price_wei%',29),('structural','%b2c_encode_failed%',30),
    ('structural','%b2c_spawn_blocking_join%',31),('structural','%route_metadata_not_found%',32),
    ('structural','%missing_opportunity_id%',33),('structural','%a3_unavailable%',34),
    ('structural','%amount_in%',35),('structural','%missing decimals%',36),
    ('structural','%missing route%',37)
),
cls AS (
  SELECT p.reason, (m.familia IS NULL) AS es_default
  FROM paper_trade_runs p
  LEFT JOIN LATERAL (
    SELECT mm.familia FROM marcadores mm
    WHERE lower(p.reason) LIKE mm.patron
    ORDER BY mm.ord LIMIT 1
  ) m ON true
  WHERE p.created_at >= :'cutoff' AND p.reason IS NOT NULL AND p.sim_fail_family IS NOT NULL
)
SELECT 'C12b' AS check_id, 'unrecognized' AS lista, reason, count(*) AS filas
FROM cls WHERE es_default
GROUP BY 1, 2, 3
UNION ALL
SELECT 'C12b' AS check_id, 'structural_known' AS lista, reason, count(*) AS filas
FROM cls WHERE sim_fail_family = 'structural' AND NOT es_default
GROUP BY 1, 2, 3
UNION ALL
-- v1.6.3: con `cls` vacio este listado no imprimia NADA (silencio, no verde). Publica fila.
SELECT 'C12b' AS check_id, '(sin filas)' AS lista, '(sin reason)' AS reason, 0 AS filas
WHERE NOT EXISTS (SELECT 1 FROM cls)
ORDER BY 1, 2, 4 DESC;

-- C13 · gas medido: distribucion por estado en filas RESUELTAS + invariante I3a/I3b
--        (126: el estado se escribe en la misma sentencia que el valor;
--         measured <=> actual_gas_cost_usd IS NOT NULL).
SELECT 'C13' AS check_id, 'A' AS sev,
       COALESCE(gas_measurement_state, '(NULL)') AS estado,
       (SELECT count(*) FROM paper_trade_runs
         WHERE created_at >= :'cutoff' AND actual_timestamp IS NOT NULL) AS filas_ventana,
       count(*) AS filas,
       count(*) FILTER (WHERE actual_gas_cost_usd IS NOT NULL) AS con_valor,
       count(*) FILTER (WHERE gas_measurement_state = 'measured' AND actual_gas_cost_usd IS NULL) AS i3_violada_measured_sin_valor,
       count(*) FILTER (WHERE gas_measurement_state IS DISTINCT FROM 'measured' AND actual_gas_cost_usd IS NOT NULL) AS valor_sin_measured,
       -- v1.6.3: mismas dos reglas que C15b — denominador 0 => NOT COMPUTED, y las
       -- violaciones del invariante se nombran en lugar de dejarse en 0 sin veredicto.
       CASE WHEN count(*) FILTER (WHERE gas_measurement_state = 'measured' AND actual_gas_cost_usd IS NULL) > 0
              THEN 'ROJO: I3 violada (measured sin valor)'
            WHEN count(*) FILTER (WHERE gas_measurement_state IS DISTINCT FROM 'measured' AND actual_gas_cost_usd IS NOT NULL) > 0
              THEN 'ROJO: I3b violada (valor sin estado measured)'
            ELSE 'OK: invariante I3/I3b sin violaciones' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff' AND actual_timestamp IS NOT NULL
GROUP BY 3
UNION ALL
SELECT 'C13', 'A', '(sin filas)', 0, 0, 0, 0, 0,
       'NOT COMPUTED: sin filas resueltas (bucle parado o ledger sin admisiones)'
WHERE (SELECT count(*) FROM paper_trade_runs
        WHERE created_at >= :'cutoff' AND actual_timestamp IS NOT NULL) = 0
ORDER BY 1, 3;

-- C13b · cobertura gas medido / TODAS las resueltas. OJO: incluye el brazo
--         ECONOMIC/MARKET, que escribe not_applicable y gas NULL POR DISEÑO (no se
--         transmitio nada). Este numero se hunde con la fraccion de rechazos y NO es
--         la cobertura del coste: la del coste es C13c. Se publica igual porque es la
--         foto del denominador completo.
SELECT 'C13b' AS check_id, 'A' AS sev,
       count(*) AS resueltas_todas,
       count(*) FILTER (WHERE actual_gas_cost_usd IS NOT NULL) AS gas_medido,
       count(*) FILTER (WHERE sim_fail_family IS NOT NULL) AS rechazos_sin_gas_por_diseno,
       CASE WHEN count(*) = 0 THEN NULL
            ELSE round(100.0 * count(*) FILTER (WHERE actual_gas_cost_usd IS NOT NULL) / count(*), 2) END AS cobertura_pct,
       CASE WHEN count(*) = 0 THEN 'NOT COMPUTED: sin filas resueltas (bucle parado)'
            ELSE 'foto del denominador completo: NO usar como cobertura del coste' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff' AND actual_timestamp IS NOT NULL;

-- C13c · cobertura del coste dominante sobre la poblacion que DEBIA medir gas:
--         filas resueltas sin clase escrita (el brazo PASS nunca escribe familia, §5.1).
--         Este es el numero que alimenta A.6.
SELECT 'C13c' AS check_id, 'A' AS sev,
       count(*) AS resueltas_pass,
       count(*) FILTER (WHERE actual_gas_cost_usd IS NOT NULL) AS gas_medido,
       count(*) FILTER (WHERE gas_measurement_state = 'unavailable') AS unavailable,
       CASE WHEN count(*) = 0 THEN NULL
            ELSE round(100.0 * count(*) FILTER (WHERE actual_gas_cost_usd IS NOT NULL) / count(*), 2) END AS cobertura_pct,
       CASE WHEN count(*) = 0 THEN 'NOT COMPUTED: sin filas PASS resueltas'
            ELSE 'cobertura del coste (A.6)' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff' AND actual_timestamp IS NOT NULL AND sim_fail_family IS NULL;

-- C14 · D-6 precision cruzada: misma cantidad, distinta escala a cada lado del join
SELECT 'C14' AS check_id, 'A' AS sev,
       (SELECT numeric_scale FROM information_schema.columns
         WHERE table_schema='public' AND table_name='paper_trade_runs'
           AND column_name='sim_expected_profit_usd') AS ledger_scale,
       (SELECT numeric_scale FROM information_schema.columns
         WHERE table_schema='public' AND table_name='opportunities'
           AND column_name='expected_profit_usd') AS origen_scale,
       (SELECT string_agg(column_name||'('||numeric_precision||','||numeric_scale||')', ', ' ORDER BY column_name)
          FROM information_schema.columns
         WHERE table_schema='public' AND table_name='paper_trade_runs'
           AND data_type='numeric' AND column_name LIKE '%\_usd') AS ledger_usd_cols,
       CASE WHEN (SELECT numeric_scale FROM information_schema.columns
                   WHERE table_schema='public' AND table_name='paper_trade_runs'
                     AND column_name='sim_expected_profit_usd') IS NULL
              OR (SELECT numeric_scale FROM information_schema.columns
                   WHERE table_schema='public' AND table_name='opportunities'
                     AND column_name='expected_profit_usd') IS NULL THEN 'NOT COMPUTED: columna ausente'
            WHEN (SELECT numeric_scale FROM information_schema.columns
                   WHERE table_schema='public' AND table_name='paper_trade_runs'
                     AND column_name='sim_expected_profit_usd')
               = (SELECT numeric_scale FROM information_schema.columns
                   WHERE table_schema='public' AND table_name='opportunities'
                     AND column_name='expected_profit_usd') THEN 'OK'
            ELSE 'RIESGO D-6: el ledger trunca a menos decimales que el origen' END AS verdict;

-- C15 · gas: DECISION vs MEDIDO, por fila.
--        v1.6.2 (2026-10-20 13:1xZ): reescrito sobre la evidencia de LEVI (msg 6987463),
--        verificada por mi en 4873c53. La version anterior (v1.6.1, "bloqueado por el
--        objeto de coste") era pesimista: el objeto SI existe por fila.
--        Lado DECISION (congelado en el insert, no se recalcula):
--          · `opportunities.economics->>'gas_usd'` — JSONB de la migracion
--            126_opportunities_economics_computation.sql:32 (ALWAYS-COMPUTE 2026-09-27),
--            escrita por searcher-rs/src/persistence.rs:187-198 en LAS DOS ramas (aceptadas
--            y rechazadas).
--          · es el gas DE LA DECISION: gross/net y gas_usd salen del mismo SizedCandidate
--            (searcher-rs/src/economics.rs:120,153-157).
--          · NO entra el `6 + 4*hops` de quant/layers.ts:490: ese es un modelo de PANEL
--            (la rama `cfg.gas_cost_usd()`, economics.rs:327,353), no el gas de la decision.
--            Ese fue el falso amigo de las tres versiones anteriores.
--          · procedencia por campo: `economics.computation_status` + `not_computed_reasons`
--            (el ausente declara por que).
--        Lado MEDIDO: `paper_trade_runs.actual_gas_cost_usd` (drift-tracker).
--        NO entra `sim_gas_cost_usd`: los dos escritores lo derivan `gross - net` = COSTE
--        TOTAL (relays-client/src/persistence.rs:196-202; paper-trade-archiver.ts:304-307),
--        no gas. Compararlo con gas medido seria un falso positivo estructural.
--        COBERTURA: las dos poblaciones se publican SEPARADAS. Poblacion 0 => NOT COMPUTED,
--        jamas OK. Hoy `economics` esta nulo en 50/50 (corte 13:09-13:12Z) porque las filas
--        mueren en la allowlist antes de la economia: C15 hoy es NOT COMPUTED con cobertura
--        0/N — es POBLACION, no esquema (la parte de esquema ya esta resuelta).
--        ACTUAL-SIM-01 (MANTIS; verificado `paper/executor.ts:366-367` + drift_tracker.rs:351):
--        el ejecutor escribe `actual_profit_usd = netYieldUsd` y `actual_timestamp = now()`,
--        asi que sus filas no pueden llegar a tener gas medido; se cuentan aparte.
--        Tolerancia CONGELADA ANTES DE MEDIR: p95 de |medido - decision| / |medido| <= 0,25
--        (25 % de 14 USD = 3,50 USD = 0,35 % de un nocional de 1.000 USD, por debajo del 2 % del gate).
WITH base AS (
  SELECT (o.economics->>'gas_usd')::numeric AS decision_gas,
         p.actual_gas_cost_usd             AS medido,
         (p.actual_timestamp IS NOT NULL AND p.reason LIKE 'paper_executor:%') AS actual_sim
  FROM paper_trade_runs p
  LEFT JOIN opportunities o ON o.id = p.opportunity_id
  WHERE p.created_at >= :'cutoff'
), m AS (
  SELECT count(*) AS resueltas,
         count(*) FILTER (WHERE decision_gas IS NOT NULL) AS con_decision,
         count(*) FILTER (WHERE medido IS NOT NULL) AS con_medido,
         count(*) FILTER (WHERE actual_sim) AS actual_sim_excluidas,
         count(*) FILTER (WHERE decision_gas IS NOT NULL AND medido IS NOT NULL) AS pares,
         -- v1.6.3 (defecto de Rachel 6989586): `d` excluye medido = 0 por division por
         -- cero, pero `pares` no. Si existe una sola fila con medido = 0, el denominador
         -- real del p95 es MENOR que `pares` y nadie publicaba la diferencia: verde sobre
         -- un denominador que se encogio en silencio.
         count(*) FILTER (WHERE decision_gas IS NOT NULL AND medido = 0) AS pares_con_medido_cero
   FROM base
), d AS (
  SELECT percentile_cont(0.95) WITHIN GROUP (ORDER BY abs(medido - decision_gas) / abs(medido)) AS p95_rel,
         percentile_cont(0.50) WITHIN GROUP (ORDER BY abs(medido - decision_gas)) AS p50_abs,
         max(abs(medido - decision_gas)) AS peor_abs,
         count(*) AS pares_en_p95
  FROM base
  WHERE decision_gas IS NOT NULL AND medido IS NOT NULL AND medido <> 0
)
SELECT 'C15' AS check_id, 'A' AS sev,
       m.resueltas, m.con_decision, m.con_medido, m.pares, m.actual_sim_excluidas,
       d.pares_en_p95,
       m.pares_con_medido_cero,
       -- Invariante publicado, no solo los dos contadores (Rachel 6989686): aritmetica
       -- cerrada. Si no cuadra, el check esta mal escrito y se ve SIN mirar datos.
       (d.pares_en_p95 + m.pares_con_medido_cero = m.pares) AS invariante_pares_ok,
       round(d.p95_rel::numeric, 6) AS p95_rel,
       round(d.p50_abs::numeric, 6) AS p50_abs_usd,
       round(d.peor_abs::numeric, 6) AS peor_abs_usd,
       CASE WHEN m.pares = 0
              THEN 'NOT COMPUTED: cero filas con gas de decision (economics.gas_usd) Y gas medido (actual_gas_cost_usd). Cobertura decision=' || m.con_decision || '/' || m.resueltas || ', medido=' || m.con_medido || '/' || m.resueltas
            WHEN d.p95_rel IS NOT NULL AND d.p95_rel > 0.25
              THEN 'ROJO: p95 |medido - decision| / medido = ' || round(d.p95_rel::numeric, 4) || ' > 0,25 (umbral congelado antes de medir)'
            -- La clausula de exclusion NO se imprime cuando el contador es 0 (Rachel
            -- 6989686): si no, el veredicto se ensucia con "0 excluidos" en cada fila.
            WHEN m.pares_con_medido_cero = 0
              THEN 'OK: p95 relativo <= 0,25 (denominador p95 = ' || d.pares_en_p95 || '/' || m.pares || ')'
            ELSE 'OK: p95 relativo <= 0,25 (denominador p95 = ' || d.pares_en_p95 || '/' || m.pares
                 || '; ' || m.pares_con_medido_cero || ' par(es) con medido = 0 excluidos del p95)'
       END AS verdict
FROM m, d;

-- C15b · procedencia y columna derivada: gas_drift_pct.
--        En 4873c53 SOLO se escribe profit_drift_pct (drift_tracker.rs:216, bind $5);
--        gas_drift_pct no tiene escritor en el repo => si llega NULL con los dos
--        lados presentes es columna muerta, no dato que falta. Se separa por
--        escritor porque sim_gas_cost_usd significa cosas distintas en cada uno.
--        AVISO: la columna `deriva_no_cuadra` de abajo compara gas_drift_pct contra una
--        deriva calculada con sim_gas_cost_usd COMO SI fuera gas; no lo es (coste total,
--        ver C15). Hoy es vacua de todos modos (gas_drift_pct sin escritor), pero cuando
--        exista el escritor ese cociente hay que rehacerlo sobre el componente gas.
SELECT 'C15b' AS check_id, 'A' AS sev,
       (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') AS filas_ventana,
       CASE WHEN reason LIKE 'paper_executor:%' THEN 'executor (inserta NULL)'
            WHEN reason IS NULL THEN '(sin reason)'
            ELSE 'archiver/relay (gross-net)' END AS procedencia,
       count(*) AS filas,
       count(*) FILTER (WHERE sim_gas_cost_usd IS NOT NULL) AS con_modelado,
       count(*) FILTER (WHERE gas_drift_pct IS NOT NULL) AS con_gas_drift_pct,
       count(*) FILTER (WHERE sim_gas_cost_usd IS NOT NULL AND actual_gas_cost_usd IS NOT NULL
                        AND gas_drift_pct IS NULL) AS deriva_faltante,
       count(*) FILTER (WHERE gas_drift_pct IS NOT NULL AND sim_gas_cost_usd IS NOT NULL
                        AND actual_gas_cost_usd IS NOT NULL
                        AND abs(gas_drift_pct
                                - round(100.0 * (actual_gas_cost_usd - sim_gas_cost_usd)
                                        / NULLIF(sim_gas_cost_usd, 0), 4)) > 0.0001) AS deriva_no_cuadra,
       -- v1.6.3 (MANTIS 6989694): `deriva_no_cuadra = 0` con `gas_drift_pct` sin escritor
       -- es el mismo falso verde que C01-C06 sobre denominador vacio. Sin escritor, el 0
       -- no es una medida: NOT COMPUTED y `filas_ventana` publicado.
       CASE WHEN (SELECT count(*) FROM paper_trade_runs WHERE created_at >= :'cutoff') = 0
              THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) FILTER (WHERE gas_drift_pct IS NOT NULL) = 0
              THEN 'NOT COMPUTED: gas_drift_pct no tiene escritor en el repo (0 filas con valor); el 0 de deriva_no_cuadra no es una medicion'
            WHEN count(*) FILTER (WHERE gas_drift_pct IS NOT NULL AND sim_gas_cost_usd IS NOT NULL
                                  AND actual_gas_cost_usd IS NOT NULL
                                  AND abs(gas_drift_pct
                                          - round(100.0 * (actual_gas_cost_usd - sim_gas_cost_usd)
                                                  / NULLIF(sim_gas_cost_usd, 0), 4)) > 0.0001) = 0
              THEN 'OK'
            ELSE 'ROJO: deriva_no_cuadra > 0' END AS verdict
FROM paper_trade_runs
WHERE created_at >= :'cutoff'
GROUP BY 2, 4 ORDER BY 5 DESC;

-- C16b · D-10a · el valor ALMACENADO, no el payload. Una fila rechazada no puede llevar
--         roi_pct/risk_score con valor: en scanner.rs:2687-2688 / 2733-2734 / 2784-2785
--         el emisor escribe Some(0.0) y persistence.rs:148 lo guarda tal cual. Esto mide
--         el almacen, asi que un shim 0->null en la API NO puede ponerlo verde.
--         La evidencia del rechazo ya viaja en rejection_reason: el 0 no aporta nada.
SELECT 'C16b' AS check_id, 'R' AS sev,
       (SELECT count(*) FROM opportunities WHERE detected_at >= :'cutoff') AS filas_ventana,
       count(*) FILTER (WHERE rejection_reason IS NOT NULL) AS filas_rechazadas,
       count(*) FILTER (WHERE rejection_reason IS NOT NULL
                          AND (roi_pct IS NOT NULL OR risk_score IS NOT NULL)) AS con_cero_fabricado,
       count(*) FILTER (WHERE rejection_reason IS NOT NULL AND roi_pct = 0) AS roi_cero,
       count(*) FILTER (WHERE rejection_reason IS NOT NULL AND risk_score = 0) AS risk_cero,
       CASE WHEN (SELECT count(*) FROM opportunities WHERE detected_at >= :'cutoff') = 0
              THEN 'NOT COMPUTED: sin filas en ventana'
            WHEN count(*) FILTER (WHERE rejection_reason IS NOT NULL) = 0
              THEN 'NOT COMPUTED: sin rechazadas en ventana'
            WHEN count(*) FILTER (WHERE rejection_reason IS NOT NULL
                                   AND (roi_pct IS NOT NULL OR risk_score IS NOT NULL)) = 0
              THEN 'OK'
            ELSE 'ROJO: 0 fabricado en el almacen (D-10a)' END AS verdict
FROM opportunities
WHERE detected_at >= :'cutoff';

ROLLBACK;
