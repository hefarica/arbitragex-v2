-- ARBX-RETENTION-01 (2026-09-04): índices de purge + rollups diarios + FK ledger-safe.
--
-- RCA (docs/RETENTION_POLICY.md): el cron pg_retention.sh v1 falló TODOS los días
-- desde 2026-08-18 con statement_timeout=20min sobre un DELETE de 13.7M filas en
-- opportunities, agravado por risk_events.opportunity_id FK ON DELETE SET NULL
-- SIN índice (seqscan por fila borrada). FREEZE-01 (docs/incidents/2026-08-17)
-- exige: batched deletes + lock_timeout + statement_timeout acotados — el script
-- v2 lo implementa; esta migración le prepara el terreno:
--   A. Índices de rango/FK para que cada batch haga index scan, no seqscan.
--   B. FK paper_trade_runs CASCADE→SET NULL: purgar opportunities NO puede
--      destruir el ledger paper (599K runs son el historial del operador).
--   C. Rollups diarios (paper_trade_runs_daily, pool_reserves_daily): el resumen
--      acumulado persiste para siempre; el detalle crudo se purga por ventana.
--
-- Rerun-safety: CONCURRENTLY IF NOT EXISTS (catálogo-check, sin rebuild) para
-- índices; DDL sobre HOT TABLES (paper_trade_runs) va en DO blocks
-- catalog-guarded (automation/tools/lint-migration-rerun-lock-safety.sh).

SET statement_timeout = '40min';

-- ============================================================================
-- A. Índices de purge (CONCURRENTLY: ShareUpdateExclusiveLock, no bloquea
--    writers; rerun = IF NOT EXISTS catálogo-check, costo cero).
-- ============================================================================

-- FK ON DELETE SET NULL: cada DELETE de opportunities hace un lookup por fila
-- aquí. Sin índice = seqscan 2.8GB por batch. Partial (WHERE NOT NULL) porque
-- la mayoría de risk_events ya huérfanas quedan NULL.
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_risk_events_opportunity_id
    ON risk_events (opportunity_id)
    WHERE opportunity_id IS NOT NULL;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_opportunity_observations_opportunity_id
    ON opportunity_observations (opportunity_id)
    WHERE opportunity_id IS NOT NULL;

-- Purge por rango de tiempo: el índice compuesto existente
-- (chain_id, observed_at DESC) no sirve para DELETE observerved_at < X sin
-- chain_id fijo.
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_opportunity_observations_observed_at
    ON opportunity_observations (observed_at);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_pool_reserves_timestamp
    ON pool_reserves ("timestamp");

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_paper_trade_runs_created_at
    ON paper_trade_runs (created_at);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_sim_simulated_at
    ON simulations (simulated_at);

-- Los índices existentes de risk_events son compuestos con created_at en
-- 2ª posición ((chain_id, created_at), (severity, ...), (event_type, ...)):
-- inútiles para WHERE created_at < X. Purge por rango necesita el leading.
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_risk_events_created_at
    ON risk_events (created_at);

-- NOTA: scored_opportunities ya tiene idx_scored_opportunities_created
-- (created_at DESC) — no se duplica.

-- ============================================================================
-- B. FK ledger-safe para paper_trade_runs (HOT TABLE → DO block catalog-guarded).
--
-- Hoy: ON DELETE CASCADE → purge de opportunities (>60d) borraría runs paper
-- (>90d de ventana deseada) en cascada. Swap online en 3 pasos:
--   1. DROP CONSTRAINT (AEL, metadata-only, sub-segundo en 599K filas)
--   2. ADD CONSTRAINT ... ON DELETE SET NULL NOT VALID (AEL, metadata-only)
--   3. VALIDATE CONSTRAINT (ShareUpdateExclusiveLock: NO bloquea INSERTs)
-- El guard por pg_get_constraintdef hace el bloque idempotente: si ya migró a
-- SET NULL (o el constraint no existe), no ejecuta nada.
-- ============================================================================

-- Los ALTER van por EXECUTE dinámico (forma guarded del lint; el string no se
-- parte en chunks bare-DDL) y cada uno es metadata-only: sub-segundo.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'paper_trade_runs_opportunity_id_fkey'
          AND conrelid = 'paper_trade_runs'::regclass
          AND pg_get_constraintdef(oid) LIKE '%ON DELETE CASCADE%'
    ) THEN
        EXECUTE 'ALTER TABLE paper_trade_runs DROP CONSTRAINT paper_trade_runs_opportunity_id_fkey';
        EXECUTE 'ALTER TABLE paper_trade_runs ADD CONSTRAINT paper_trade_runs_opportunity_id_fkey FOREIGN KEY (opportunity_id) REFERENCES opportunities (id) ON DELETE SET NULL NOT VALID';
    END IF;
END
$$;

-- VALIDATE separado: ShareUpdateExclusiveLock (NO bloquea INSERTs del paper
-- archiver); escanea la tabla una vez para certificar las filas existentes.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'paper_trade_runs_opportunity_id_fkey'
          AND conrelid = 'paper_trade_runs'::regclass
          AND NOT convalidated
    ) THEN
        EXECUTE 'ALTER TABLE paper_trade_runs VALIDATE CONSTRAINT paper_trade_runs_opportunity_id_fkey';
    END IF;
END
$$;

-- ============================================================================
-- C. Rollups diarios — el resumen acumulado que sobrevive al purge.
--    Patrón 115_route_discovery_outcomes_rollup_5m.sql: seed one-time guarded
--    por probe de vacío (rerun = no-op). El mantenimiento incremental lo hace
--    scripts/pg_retention.sh con upsert de los últimos 4 días (idempotente).
-- ============================================================================

CREATE TABLE IF NOT EXISTS paper_trade_runs_daily (
    day                date    NOT NULL,
    chain_id           integer NOT NULL,
    strategy_kind      text    NOT NULL,
    runs               bigint  NOT NULL DEFAULT 0,
    runs_with_actual   bigint  NOT NULL DEFAULT 0,
    sim_profit_sum     numeric NOT NULL DEFAULT 0,
    sim_profit_n       bigint  NOT NULL DEFAULT 0,
    actual_profit_sum  numeric NOT NULL DEFAULT 0,
    actual_profit_n    bigint  NOT NULL DEFAULT 0,
    actual_profit_gt0  bigint  NOT NULL DEFAULT 0,
    sim_fails          bigint  NOT NULL DEFAULT 0,
    PRIMARY KEY (day, chain_id, strategy_kind)
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM paper_trade_runs_daily LIMIT 1) THEN
        INSERT INTO paper_trade_runs_daily (
            day, chain_id, strategy_kind, runs, runs_with_actual,
            sim_profit_sum, sim_profit_n, actual_profit_sum, actual_profit_n,
            actual_profit_gt0, sim_fails)
        SELECT
            (created_at AT TIME ZONE 'UTC')::date,
            chain_id,
            COALESCE(NULLIF(strategy_kind, ''), '(unset)'),
            count(*),
            count(*) FILTER (WHERE actual_timestamp IS NOT NULL),
            COALESCE(sum(sim_expected_profit_usd), 0),
            count(sim_expected_profit_usd),
            COALESCE(sum(actual_profit_usd), 0),
            count(actual_profit_usd),
            count(*) FILTER (WHERE actual_profit_usd > 0),
            count(*) FILTER (WHERE sim_fail_family IS NOT NULL)
        FROM paper_trade_runs
        GROUP BY 1, 2, 3;
    END IF;
END
$$;

-- pool_reserves es un sink write-only (pool_sync_worker): el runtime lee las
-- reservas VIVAS de Redis; PG solo guarda historia. Un snapshot por pool por
-- día basta para reconstruir series y backtests de grano diario.
CREATE TABLE IF NOT EXISTS pool_reserves_daily (
    day              date                       NOT NULL,
    pool_id          uuid                       NOT NULL,
    last_block       bigint,
    reserve0         numeric,
    reserve1         numeric,
    snapshot_at      timestamp with time zone  NOT NULL,
    PRIMARY KEY (day, pool_id)
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pool_reserves_daily LIMIT 1) THEN
        INSERT INTO pool_reserves_daily (
            day, pool_id, last_block, reserve0, reserve1, snapshot_at)
        SELECT DISTINCT ON (pool_id, ("timestamp" AT TIME ZONE 'UTC')::date)
            ("timestamp" AT TIME ZONE 'UTC')::date,
            pool_id,
            block_number,
            reserve0,
            reserve1,
            "timestamp"
        FROM pool_reserves
        ORDER BY pool_id, ("timestamp" AT TIME ZONE 'UTC')::date, "timestamp" DESC;
    END IF;
END
$$;
