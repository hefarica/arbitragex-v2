-- =====================================================================
-- OMEGA Migration 066 — OMNI ENTITY REGISTRIES (canonical 12-entity fabric)
-- =====================================================================
-- Closes the Mirror Law gap: every operational entity owns a canonical
-- table with config_hash, idempotency_key, audit_event linkage and
-- per-resource hot-reload notification.
--
-- Pre-existing tables NOT touched: chains, dexes, pools, tokens, wallets,
-- strategy_configs, risk_events, audit_log, killswitch.
-- We ADD missing canonical registries and extend audit semantics.
--
-- Doctrine: Zero-Mocks, Ghost Protocol, Lexicón Absoluto.
-- 7-Layer Rule: this migration is layer 4 (persistence) for new entities.
-- =====================================================================

BEGIN;

-- ---------------------------------------------------------------------
-- 1. RPC Registry — per-chain RPC endpoints with health + auth tier
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS rpc_endpoints (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id           BIGINT NOT NULL REFERENCES chains(chain_id) ON DELETE CASCADE,
    url                TEXT NOT NULL,
    transport          TEXT NOT NULL CHECK (transport IN ('http','https','ws','wss','ipc')),
    tier               TEXT NOT NULL DEFAULT 'primary' CHECK (tier IN ('primary','secondary','fallback','archive')),
    auth_kind          TEXT NOT NULL DEFAULT 'none' CHECK (auth_kind IN ('none','header','bearer','basic','query')),
    auth_credential_id UUID REFERENCES service_credentials(id) ON DELETE SET NULL,
    weight             INTEGER NOT NULL DEFAULT 100 CHECK (weight BETWEEN 0 AND 10000),
    rate_limit_rps     INTEGER,
    max_concurrency    INTEGER NOT NULL DEFAULT 16,
    enabled            BOOLEAN NOT NULL DEFAULT TRUE,
    status             TEXT NOT NULL DEFAULT 'pending_validation'
                       CHECK (status IN ('active','paused','blocked','pending_validation','deprecated')),
    config_hash        CHAR(64) NOT NULL,
    last_probe_at      TIMESTAMPTZ,
    last_latency_ms    INTEGER,
    notes              TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by         TEXT NOT NULL,
    updated_by         TEXT NOT NULL,
    UNIQUE (chain_id, url)
);
CREATE INDEX IF NOT EXISTS idx_rpc_endpoints_chain ON rpc_endpoints (chain_id, enabled, tier);

-- ---------------------------------------------------------------------
-- 2. Contract Registry — deterministic CREATE2 deployments per chain
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS contract_registry (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id           BIGINT NOT NULL REFERENCES chains(chain_id) ON DELETE CASCADE,
    label              TEXT NOT NULL,
    address            TEXT NOT NULL CHECK (address ~* '^0x[0-9a-f]{40}$'),
    deployer           TEXT NOT NULL,
    salt               TEXT NOT NULL,
    init_code_hash     TEXT NOT NULL,
    abi_version        TEXT NOT NULL,
    contract_kind      TEXT NOT NULL CHECK (contract_kind IN (
                          'factory','resolution_core','adapter_uniswap_v2','adapter_uniswap_v3',
                          'adapter_balancer','adapter_curve','adapter_pancake_v3','adapter_gmx',
                          'adapter_synthetix','flashloan_aave','flashloan_balancer','flashloan_dydx',
                          'flashloan_uniswap_v3','wallet_topology','gas_sponsor','cold_treasury',
                          'execution_signer_guard','allowance_manager','admin_timelock','custom')),
    proxy_kind         TEXT CHECK (proxy_kind IN ('uups','transparent','beacon','minimal','none')),
    implementation     TEXT,
    deploy_tx          TEXT,
    deploy_block       BIGINT,
    verified           BOOLEAN NOT NULL DEFAULT FALSE,
    enabled            BOOLEAN NOT NULL DEFAULT TRUE,
    status             TEXT NOT NULL DEFAULT 'pending_validation'
                       CHECK (status IN ('active','paused','blocked','pending_validation','deprecated')),
    config_hash        CHAR(64) NOT NULL,
    metadata           JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by         TEXT NOT NULL,
    updated_by         TEXT NOT NULL,
    UNIQUE (chain_id, label),
    UNIQUE (chain_id, address)
);
CREATE INDEX IF NOT EXISTS idx_contract_registry_chain_kind ON contract_registry (chain_id, contract_kind);

-- ---------------------------------------------------------------------
-- 3. Capital Gate Registry — global/per-chain/per-wallet/per-strategy
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS capital_gates (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope                 TEXT NOT NULL CHECK (scope IN ('global','chain','wallet','strategy','token','route')),
    scope_ref             TEXT,
    name                  TEXT NOT NULL,
    capital_cap_usd       NUMERIC(38,8) NOT NULL DEFAULT 0 CHECK (capital_cap_usd >= 0),
    max_gas_burn_usd      NUMERIC(38,8) NOT NULL DEFAULT 0 CHECK (max_gas_burn_usd >= 0),
    max_drawdown_pct      NUMERIC(7,4) NOT NULL DEFAULT 0 CHECK (max_drawdown_pct BETWEEN 0 AND 100),
    max_consecutive_err   INTEGER NOT NULL DEFAULT 0,
    max_revert_rate_pct   NUMERIC(7,4) NOT NULL DEFAULT 0,
    max_latency_ms        INTEGER NOT NULL DEFAULT 0,
    window_seconds        INTEGER NOT NULL DEFAULT 0,
    enabled               BOOLEAN NOT NULL DEFAULT TRUE,
    status                TEXT NOT NULL DEFAULT 'active'
                          CHECK (status IN ('active','paused','blocked','pending_validation','deprecated')),
    config_hash           CHAR(64) NOT NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by            TEXT NOT NULL,
    updated_by            TEXT NOT NULL,
    UNIQUE (scope, scope_ref, name)
);
CREATE INDEX IF NOT EXISTS idx_capital_gates_scope ON capital_gates (scope, enabled);

-- ---------------------------------------------------------------------
-- 4. Risk Gate Registry — orthogonal to risk_events
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS risk_gates (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope                 TEXT NOT NULL CHECK (scope IN ('global','chain','wallet','strategy','token','route','dex','time_window')),
    scope_ref             TEXT,
    gate_kind             TEXT NOT NULL CHECK (gate_kind IN (
                            'min_confidence','max_slippage_bps','min_liquidity_usd',
                            'max_price_impact_bps','min_spectral_gap','max_decoherence',
                            'blacklist_token','blacklist_route','time_lock','circuit_breaker')),
    threshold_value       NUMERIC(38,8) NOT NULL,
    comparator            TEXT NOT NULL CHECK (comparator IN ('lt','lte','gt','gte','eq','neq','in','nin')),
    action_on_breach      TEXT NOT NULL CHECK (action_on_breach IN ('reject','warn','throttle','pause_chain','pause_global')),
    enabled               BOOLEAN NOT NULL DEFAULT TRUE,
    status                TEXT NOT NULL DEFAULT 'active'
                          CHECK (status IN ('active','paused','blocked','pending_validation','deprecated')),
    config_hash           CHAR(64) NOT NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by            TEXT NOT NULL,
    updated_by            TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_risk_gates_scope ON risk_gates (scope, gate_kind, enabled);

-- ---------------------------------------------------------------------
-- 5. Relay Registry — private transaction relays per chain
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS relay_endpoints (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id           BIGINT NOT NULL REFERENCES chains(chain_id) ON DELETE CASCADE,
    name               TEXT NOT NULL,
    url                TEXT NOT NULL,
    relay_kind         TEXT NOT NULL CHECK (relay_kind IN ('flashbots','bloxroute','eden','manifold','public','custom')),
    auth_credential_id UUID REFERENCES service_credentials(id) ON DELETE SET NULL,
    priority           INTEGER NOT NULL DEFAULT 100,
    timeout_ms         INTEGER NOT NULL DEFAULT 5000,
    enabled            BOOLEAN NOT NULL DEFAULT TRUE,
    status             TEXT NOT NULL DEFAULT 'pending_validation'
                       CHECK (status IN ('active','paused','blocked','pending_validation','deprecated')),
    config_hash        CHAR(64) NOT NULL,
    last_health_at     TIMESTAMPTZ,
    last_health_ok     BOOLEAN,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by         TEXT NOT NULL,
    updated_by         TEXT NOT NULL,
    UNIQUE (chain_id, name)
);
CREATE INDEX IF NOT EXISTS idx_relay_endpoints_chain ON relay_endpoints (chain_id, enabled, priority);

-- ---------------------------------------------------------------------
-- 6. Agent Registry — Sindicato OMEGA roles + evidence binding
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS agent_registry (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role               TEXT NOT NULL UNIQUE,
    discipline         TEXT NOT NULL,
    rank               TEXT NOT NULL CHECK (rank IN ('phd','nobel','principal','architect','auditor')),
    active             BOOLEAN NOT NULL DEFAULT TRUE,
    capabilities       JSONB NOT NULL DEFAULT '[]'::jsonb,
    evidence_bindings  JSONB NOT NULL DEFAULT '[]'::jsonb,
    status             TEXT NOT NULL DEFAULT 'active'
                       CHECK (status IN ('active','paused','blocked','pending_validation','deprecated')),
    config_hash        CHAR(64) NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by         TEXT NOT NULL,
    updated_by         TEXT NOT NULL
);

-- ---------------------------------------------------------------------
-- 7. AUDIT EVENT — supersedes audit_log for canonical registry changes
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS audit_event (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor                 TEXT NOT NULL,
    action                TEXT NOT NULL CHECK (action IN (
                            'create','update','delete','enable','disable','reload','rollback')),
    entity_type           TEXT NOT NULL CHECK (entity_type IN (
                            'chain','rpc','dex','pool','token','wallet','strategy',
                            'contract','risk_gate','capital_gate','relay','agent')),
    entity_id             TEXT NOT NULL,
    chain_id              BIGINT,
    before_state          JSONB,
    after_state           JSONB,
    config_hash_before    CHAR(64),
    config_hash_after     CHAR(64),
    idempotency_key       TEXT NOT NULL,
    result                TEXT NOT NULL CHECK (result IN ('success','partial','failed')),
    error                 TEXT,
    ip_address            INET,
    user_agent            TEXT,
    trace_id              TEXT,
    timestamp             TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (idempotency_key)
);
CREATE INDEX IF NOT EXISTS idx_audit_event_entity   ON audit_event (entity_type, entity_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_event_actor    ON audit_event (actor, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_event_chain    ON audit_event (chain_id, timestamp DESC);

COMMIT;
