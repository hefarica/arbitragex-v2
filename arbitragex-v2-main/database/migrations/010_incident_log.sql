-- ArbitrageX v2 — Migration 010: incident_log
-- Incident postmortems. One record per operational incident.

CREATE TABLE IF NOT EXISTS incident_log (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  title TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('open','investigating','mitigated','resolved')),
  severity TEXT NOT NULL CHECK (severity IN ('info','warning','critical')),
  started_at TIMESTAMPTZ NOT NULL,
  resolved_at TIMESTAMPTZ,
  root_cause TEXT,
  remediation TEXT,
  related_risk_event_ids UUID[]
);

CREATE INDEX IF NOT EXISTS idx_incident_status ON incident_log(status, started_at DESC);

GRANT SELECT, INSERT, UPDATE ON incident_log TO arbx_rw;
GRANT SELECT ON incident_log TO arbx_ro;
