-- ArbitrageX v2 — Migration 019: audit_log monthly partitioning

BEGIN;

-- 1. Renombrar tabla actual e índices
ALTER TABLE audit_log RENAME TO audit_log_old;
ALTER INDEX idx_audit_actor_time RENAME TO idx_audit_actor_time_old;
ALTER INDEX idx_audit_action_time RENAME TO idx_audit_action_time_old;

-- 2. Crear nueva tabla particionada
-- NOTA: En tablas particionadas, la PRIMARY KEY debe incluir la clave de partición (created_at).
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
) PARTITION BY RANGE (created_at);

-- 3. Crear partición por defecto (para rows antiguas o desfasadas)
CREATE TABLE audit_log_default PARTITION OF audit_log DEFAULT;

-- 4. Crear particiones para el mes actual y el mes siguiente mediante PL/pgSQL
DO $$
DECLARE
    start_date DATE := date_trunc('month', CURRENT_DATE);
    next_date DATE := start_date + INTERVAL '1 month';
    next_next_date DATE := start_date + INTERVAL '2 months';
    part_current TEXT := 'audit_log_' || to_char(start_date, 'YYYY_MM');
    part_next TEXT := 'audit_log_' || to_char(next_date, 'YYYY_MM');
BEGIN
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF audit_log FOR VALUES FROM (%L) TO (%L);', part_current, start_date, next_date);
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF audit_log FOR VALUES FROM (%L) TO (%L);', part_next, next_date, next_next_date);
END
$$;

-- 5. Migrar datos existentes
INSERT INTO audit_log SELECT * FROM audit_log_old;

-- 6. Eliminar tabla vieja
DROP TABLE audit_log_old;

-- 7. Crear índices en la tabla particionada
CREATE INDEX idx_audit_actor_time ON audit_log(actor, created_at DESC);
CREATE INDEX idx_audit_action_time ON audit_log(action, created_at DESC);

-- 8. Restaurar permisos (arbx_rw no puede hacer UPDATE ni DELETE)
REVOKE UPDATE, DELETE ON audit_log FROM arbx_rw;
GRANT SELECT, INSERT ON audit_log TO arbx_rw;
GRANT SELECT ON audit_log TO arbx_ro;

COMMIT;
