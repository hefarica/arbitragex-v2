-- =====================================================================
-- OMEGA-8 / M3 Migration 071 — Capa 2 P2 fixes
-- =====================================================================
-- Forward-only · idempotent · safe.
--
-- Cierra P2 fixes seleccionados del audit Capa 2:
--   • P2-2: retention sobre `runtime_ack`. Función `arbx_prune_runtime_ack`
--           con piso conservador 7 días, default 30 días, máximo 90 días
--           (alineada con `arbx_drop_old_audit_partitions` de mig 052).
--   • P2-3: `config_hash_registry` UNIQUE débil por NULLs distinct. PG15+
--           expone `NULLS NOT DISTINCT`. Si la versión soporta, recreamos
--           el constraint correctamente; si no (PG <15), creamos índice
--           parcial separando `chain_id IS NULL` de `IS NOT NULL`.
--
-- Idempotencia: DO blocks + IF NOT EXISTS + DROP+CREATE seguro del UNIQUE
-- recreado (sólo si difiere).
--
-- Rollback manual:
--   DROP FUNCTION IF EXISTS arbx_prune_runtime_ack(integer);
--   -- Re-crear el UNIQUE viejo (NULLs distinct):
--   ALTER TABLE config_hash_registry
--     DROP CONSTRAINT IF EXISTS config_hash_registry_resource_chain_id_computed_at_key,
--     ADD CONSTRAINT config_hash_registry_resource_chain_id_computed_at_key
--     UNIQUE (resource, chain_id, computed_at);
-- =====================================================================

BEGIN;

-- 1. retention helper para runtime_ack
CREATE OR REPLACE FUNCTION arbx_prune_runtime_ack(days_to_keep INTEGER DEFAULT 30)
RETURNS INTEGER
LANGUAGE plpgsql
AS $$
DECLARE
    v_deleted INTEGER := 0;
BEGIN
    -- Floor mínimo 7 días (alineado con arbx_drop_old_audit_partitions); tope
    -- 90 días para impedir limpiezas accidentales agresivas. Operadores
    -- pueden eliminar el tope manualmente si necesitan retention extendida.
    IF days_to_keep IS NULL OR days_to_keep < 7 THEN
        RAISE EXCEPTION 'arbx_prune_runtime_ack: days_to_keep must be >= 7, got %', days_to_keep;
    END IF;
    IF days_to_keep > 90 THEN
        RAISE EXCEPTION 'arbx_prune_runtime_ack: days_to_keep must be <= 90, got % (operator unsafety guard)', days_to_keep;
    END IF;

    DELETE FROM runtime_ack
     WHERE received_at < (NOW() - (days_to_keep || ' days')::interval);

    GET DIAGNOSTICS v_deleted = ROW_COUNT;
    RAISE NOTICE 'arbx_prune_runtime_ack: deleted % rows older than % days', v_deleted, days_to_keep;
    RETURN v_deleted;
END;
$$;

COMMENT ON FUNCTION arbx_prune_runtime_ack(INTEGER) IS
    'OMEGA-8/M3 P2-2: runtime_ack retention. Floor 7d (audit-aligned), '
    'default 30d, ceiling 90d. Invoked by systemd timer or cron.';

-- 2. config_hash_registry — corregir UNIQUE débil por NULLs distinct.
--    PG15+ admite NULLS NOT DISTINCT. Detectamos versión; si <15 caemos a
--    índices parciales separando IS NULL / IS NOT NULL.
DO $$
DECLARE
    v_major INTEGER;
BEGIN
    SELECT current_setting('server_version_num')::int / 10000 INTO v_major;

    IF v_major >= 15 THEN
        -- Intentar reemplazar el UNIQUE existente con NULLS NOT DISTINCT.
        IF EXISTS (
            SELECT 1 FROM pg_constraint
             WHERE conname = 'config_hash_registry_resource_chain_id_computed_at_key'
        ) THEN
            ALTER TABLE config_hash_registry
              DROP CONSTRAINT config_hash_registry_resource_chain_id_computed_at_key;
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint
             WHERE conname = 'config_hash_registry_uniq_nulls_not_distinct'
        ) THEN
            ALTER TABLE config_hash_registry
              ADD CONSTRAINT config_hash_registry_uniq_nulls_not_distinct
              UNIQUE NULLS NOT DISTINCT (resource, chain_id, computed_at);
            RAISE NOTICE 'Migration 071: config_hash_registry UNIQUE NULLS NOT DISTINCT installed (PG15+)';
        END IF;
    ELSE
        -- PG <15 fallback: índices parciales para cubrir NULL y non-NULL.
        RAISE NOTICE 'Migration 071: PG %.x detected, falling back to partial unique indexes', v_major;
        CREATE UNIQUE INDEX IF NOT EXISTS config_hash_registry_uniq_chain_notnull
            ON config_hash_registry (resource, chain_id, computed_at)
            WHERE chain_id IS NOT NULL;
        CREATE UNIQUE INDEX IF NOT EXISTS config_hash_registry_uniq_chain_null
            ON config_hash_registry (resource, computed_at)
            WHERE chain_id IS NULL;
    END IF;
END$$;

COMMIT;
