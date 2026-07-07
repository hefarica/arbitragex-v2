-- ArbitrageX v2 — Migration 020: arbx_drop_old_audit_partitions

BEGIN;

-- Esta función es invocada por el systemd timer para implementar la retention policy.
-- Se encarga de tres cosas:
-- 1. Elimina particiones cuyo nombre indica que son más antiguas que "days_to_keep".
-- 2. Asegura que la partición del mes en curso y del mes siguiente siempre existan.
-- 3. Devuelve el número de particiones eliminadas.

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

-- Permitir a arbx_rw y arbx_migrator ejecutar esta función
GRANT EXECUTE ON FUNCTION arbx_drop_old_audit_partitions(INT) TO arbx_rw;
GRANT EXECUTE ON FUNCTION arbx_drop_old_audit_partitions(INT) TO arbx_migrator;

COMMIT;
