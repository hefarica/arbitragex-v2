-- ArbitrageX v2 — Migration 009: risk_events
-- Observable risk signals: kill-switch triggers, circuit-breaker trips, blacklist hits, degradations.

CREATE TABLE IF NOT EXISTS risk_events (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  event_type TEXT NOT NULL CHECK (event_type IN (
    'circuit_breaker','kill_switch','blacklist_hit','degradation','manual'
  )),
  severity TEXT NOT NULL CHECK (severity IN ('info','warning','critical')),
  source_service TEXT NOT NULL,
  payload JSONB NOT NULL,
  trace_id UUID,
  opportunity_id UUID REFERENCES opportunities(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_risk_sev_time ON risk_events(severity, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_risk_type_time ON risk_events(event_type, created_at DESC);

GRANT SELECT, INSERT ON risk_events TO arbx_rw;
GRANT SELECT ON risk_events TO arbx_ro;
