-- ============================================================================
-- Migration 052: enforce minimum retention >= 7 days on audit_log purge fn
--
-- Per audit 2026-05-10 finding M6: previous function accepted any value
-- including 0, allowing total wipe of audit_log. Enforce 7-day floor.
--
-- Reversible: restore migration 020 function body to remove the guard.
-- ============================================================================

BEGIN;

CREATE OR REPLACE FUNCTION arbx_drop_old_audit_partitions(days_to_keep INT)
RETURNS INT AS $$
DECLARE
    partition_name TEXT;
    cutoff_date DATE;
    dropped_count INT := 0;

    -- Variables para crear particiones (mantenimiento proactivo)
    start_date DATE := date_trunc('month', CURRENT_DATE);
    next_date DATE := start_date + INTERVAL '1 month';
    next_next_date DATE := start_date + INTERVAL '2 months';
    part_current TEXT := 'audit_log_' || to_char(start_date, 'YYYY_MM');
    part_next TEXT := 'audit_log_' || to_char(next_date, 'YYYY_MM');
BEGIN
    -- Audit hardening (M6): enforce minimum retention period.
    -- The audit log is the forensic last-resort — protect it from accidental
    -- and malicious total-wipe scenarios. NULL is treated as an invalid input.
    IF days_to_keep IS NULL OR days_to_keep < 7 THEN
        RAISE EXCEPTION
            'arbx_drop_old_audit_partitions: days_to_keep must be >= 7 (got %)',
            days_to_keep
            USING ERRCODE = 'invalid_parameter_value';
    END IF;

    -- 1. Calcular fecha de corte (si una partición corresponde a un mes completamente anterior al cutoff, se borra)
    cutoff_date := CURRENT_DATE - days_to_keep;

    -- 2. Iterar sobre las particiones hijas de audit_log (ignorando audit_log_default)
    FOR partition_name IN
        SELECT c.relname
        FROM pg_inherits i
        JOIN pg_class c ON c.oid = i.inhrelid
        JOIN pg_class p ON p.oid = i.inhparent
        WHERE p.relname = 'audit_log'
          AND c.relname != 'audit_log_default'
          AND c.relkind = 'r'
    LOOP
        -- Extraer el YYYY_MM del nombre de la partición (audit_log_YYYY_MM)
        DECLARE
            part_date_str TEXT := substring(partition_name from 'audit_log_(\d{4}_\d{2})');
            part_end_date DATE;
        BEGIN
            IF part_date_str IS NOT NULL THEN
                -- Calcular el final de ese mes (inicio del siguiente)
                part_end_date := to_date(part_date_str || '_01', 'YYYY_MM_DD') + INTERVAL '1 month';

                -- Si el fin del mes de esa partición es más antiguo que el cutoff, lo borramos
                IF part_end_date <= cutoff_date THEN
                    EXECUTE format('DROP TABLE %I', partition_name);
                    dropped_count := dropped_count + 1;
                END IF;
            END IF;
        END;
    END LOOP;

    -- 3. Crear proactivamente la partición del mes actual y del mes siguiente
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF audit_log FOR VALUES FROM (%L) TO (%L);', part_current, start_date, next_date);
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF audit_log FOR VALUES FROM (%L) TO (%L);', part_next, next_date, next_next_date);

    RETURN dropped_count;
END;
$$ LANGUAGE plpgsql;

-- Sanity: verify the function rejects days_to_keep values below the floor.
-- This block executes inside the same transaction and rolls back on any assertion failure.
DO $$
DECLARE
    rejected_zero    BOOLEAN := FALSE;
    rejected_six     BOOLEAN := FALSE;
BEGIN
    -- Case 1: days_to_keep = 0 must raise invalid_parameter_value
    BEGIN
        PERFORM arbx_drop_old_audit_partitions(0);
    EXCEPTION WHEN invalid_parameter_value THEN
        rejected_zero := TRUE;
    END;

    -- Case 2: days_to_keep = 6 (one below the floor) must also raise
    BEGIN
        PERFORM arbx_drop_old_audit_partitions(6);
    EXCEPTION WHEN invalid_parameter_value THEN
        rejected_six := TRUE;
    END;

    IF NOT rejected_zero THEN
        RAISE EXCEPTION 'invariant violation: function must reject days_to_keep=0';
    END IF;

    IF NOT rejected_six THEN
        RAISE EXCEPTION 'invariant violation: function must reject days_to_keep=6';
    END IF;

    RAISE NOTICE 'Migration 052: arbx_drop_old_audit_partitions guard verified — rejects 0 and 6, floor is 7.';
END $$;

COMMIT;
