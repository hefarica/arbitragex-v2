-- ArbitrageX v2 — Migration 011: audit_log
-- Tamper-evident trail for admin actions, config mutations, killswitch toggles, rejections.

CREATE TABLE IF NOT EXISTS audit_log (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  actor TEXT NOT NULL,
  action TEXT NOT NULL,
  target_kind TEXT,
  target_id TEXT,
  before_state JSONB,
  after_state JSONB,
  ip_address INET,
  user_agent TEXT,
  trace_id UUID,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_actor_time ON audit_log(actor, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_action_time ON audit_log(action, created_at DESC);

REVOKE UPDATE, DELETE ON audit_log FROM arbx_rw;
GRANT SELECT, INSERT ON audit_log TO arbx_rw;
GRANT SELECT ON audit_log TO arbx_ro;
