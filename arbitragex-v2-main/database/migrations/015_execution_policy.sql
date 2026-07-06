-- ArbitrageX v2 — Migration 015: execution_policy (operator-editable caps, versioned).
--
-- No-hardcode doctrine: capital-at-risk caps MUST be explicit, signed-off,
-- rotatable. They never live in TOML under the operator's feet.
--
-- Typical row: one active per (chain_id). `paper_mode` starts TRUE and is
-- flipped to FALSE only by an explicit Phase-5 onboarding step (with audit entry).

CREATE TABLE IF NOT EXISTS execution_policy (
  id                           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  chain_id                     INTEGER     NOT NULL,
  version                      INTEGER     NOT NULL,
  paper_mode                   BOOLEAN     NOT NULL DEFAULT TRUE,
  private_only                 BOOLEAN     NOT NULL DEFAULT TRUE,
  max_value_eth                NUMERIC(20,8) NOT NULL CHECK (max_value_eth >= 0),
  max_gas_price_gwei           NUMERIC(12,2) NOT NULL CHECK (max_gas_price_gwei >= 0),
  max_slippage_pct             NUMERIC(6,4)  NOT NULL CHECK (max_slippage_pct >= 0 AND max_slippage_pct <= 50),
  max_parallel_executions      INTEGER     NOT NULL CHECK (max_parallel_executions >= 1 AND max_parallel_executions <= 64),
  target_block_offset          INTEGER     NOT NULL DEFAULT 1 CHECK (target_block_offset >= 0),
  max_inclusion_wait_blocks    INTEGER     NOT NULL DEFAULT 5 CHECK (max_inclusion_wait_blocks >= 1),
  priority_fee_increment_pct   INTEGER     NOT NULL DEFAULT 10 CHECK (priority_fee_increment_pct >= 0),
  retry_limit                  INTEGER     NOT NULL DEFAULT 2 CHECK (retry_limit >= 0),
  flashbots_submit_timeout_ms  INTEGER     NOT NULL DEFAULT 5000 CHECK (flashbots_submit_timeout_ms >= 500),
  description                  TEXT        NOT NULL,
  activated_by                 TEXT        NOT NULL,
  effective_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  superseded_at                TIMESTAMPTZ,
  created_at                   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT execution_policy_uq UNIQUE (chain_id, version)
);

CREATE INDEX IF NOT EXISTS idx_execution_policy_active
  ON execution_policy(chain_id, effective_at DESC)
  WHERE superseded_at IS NULL;

GRANT SELECT, INSERT, UPDATE ON execution_policy TO arbx_rw;
GRANT SELECT ON execution_policy TO arbx_ro;
