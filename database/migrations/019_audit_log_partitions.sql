-- ArbitrageX v2 — Migration 019: audit_log monthly partitioning
--
-- IDEMPOTENT (2026-06-11): on databases migrated out-of-band before
-- schema_migrations tracking existed, audit_log is ALREADY partitioned and the
-- original unguarded `CREATE TABLE audit_log_default` aborted the whole runner
-- ("relation audit_log_default already exists" under ON_ERROR_STOP, blocking
-- every later migration). The transform is now wrapped in PL/pgSQL with an
-- early-exit guard: if audit_log is already a partitioned table, this is a
-- no-op. Semantics for a fresh (unpartitioned) database are unchanged.

BEGIN;

DO $mig$
DECLARE
    start_date DATE := date_trunc('month', CURRENT_DATE);
    next_date DATE := start_date + INTERVAL '1 month';
    next_next_date DATE := start_date + INTERVAL '2 months';
    part_current TEXT := 'audit_log_' || to_char(start_date, 'YYYY_MM');
    part_next TEXT := 'audit_log_' || to_char(next_date, 'YYYY_MM');
BEGIN
    -- Guard: relkind 'p' = partitioned table → migration already applied.
    IF EXISTS (
        SELECT 1 FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relname = 'audit_log' AND n.nspname = 'public' AND c.relkind = 'p'
    ) THEN
        RAISE NOTICE 'Migration 019: audit_log is already partitioned — skipping (idempotent no-op).';
        RETURN;
    END IF;

    -- 1. Renombrar tabla actual e índices
    EXECUTE 'ALTER TABLE audit_log RENAME TO audit_log_old';
    EXECUTE 'ALTER INDEX idx_audit_actor_time RENAME TO idx_audit_actor_time_old';
    EXECUTE 'ALTER INDEX idx_audit_action_time RENAME TO idx_audit_action_time_old';

    -- 2. Crear nueva tabla particionada
    -- NOTA: En tablas particionadas, la PRIMARY KEY debe incluir la clave de partición (created_at).
    EXECUTE '
    CREATE TABLE audit_log (
      id UUID DEFAULT gen_random_uuid(),
      actor TEXT NOT NULL,
      action TEXT NOT NULL,
      target_kind TEXT,
      target_id TEXT,
      before_state JSONB,
      after_state JSONB,
      ip_address INET,
      user_agent TEXT,
      trace_id UUID,
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      PRIMARY KEY (id, created_at)
    ) PARTITION BY RANGE (created_at)';

    -- 3. Crear partición por defecto (para rows antiguas o desfasadas)
    EXECUTE 'CREATE TABLE audit_log_default PARTITION OF audit_log DEFAULT';

    -- 4. Crear particiones para el mes actual y el siguiente
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF audit_log FOR VALUES FROM (%L) TO (%L);', part_current, start_date, next_date);
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF audit_log FOR VALUES FROM (%L) TO (%L);', part_next, next_date, next_next_date);

    -- 5. Migrar datos existentes (applying PII anonymization to comply with Omega-8 Doctrine)
    EXECUTE '
    INSERT INTO audit_log (id, actor, action, target_kind, target_id, before_state, after_state, ip_address, user_agent, trace_id, created_at)
    SELECT
      id, actor, action, target_kind, target_id, before_state, after_state,
      arbx_anonymize_ip(ip_address),
      arbx_hash_user_agent(user_agent),
      trace_id, created_at
    FROM audit_log_old';

    -- 6. Eliminar tabla vieja
    EXECUTE 'DROP TABLE audit_log_old';

    -- 7. Crear índices en la tabla particionada
    EXECUTE 'CREATE INDEX idx_audit_actor_time ON audit_log(actor, created_at DESC)';
    EXECUTE 'CREATE INDEX idx_audit_action_time ON audit_log(action, created_at DESC)';

    -- 8. Restaurar permisos (arbx_rw no puede hacer UPDATE ni DELETE)
    EXECUTE 'REVOKE UPDATE, DELETE ON audit_log FROM arbx_rw';
    EXECUTE 'GRANT SELECT, INSERT ON audit_log TO arbx_rw';
    EXECUTE 'GRANT SELECT ON audit_log TO arbx_ro';
END
$mig$;

COMMIT;
