-- ============================================================
-- MIGRACIONES SED — Sequential Equilibrium Dispatcher
-- Protocol: OMEGA AGENT TEAMS
-- Fecha: 2026-05-13
-- Sprint Base: S1 (estructura), S2+ (datos)
-- ============================================================

-- ============================================================
-- Migration 012: sed_filtrations
-- Sprint: S2+
-- Módulo: stochastic_wave_filtration.rs
-- Descripción: Telemetría de filtración estocástica
-- ============================================================

CREATE TABLE IF NOT EXISTS sed_filtrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identificación de mercado
    market_id VARCHAR(64) NOT NULL,
    token_pair VARCHAR(32) NOT NULL,
    chain_id INTEGER NOT NULL DEFAULT 1,

    -- Parámetros del proceso de Markov con saltos
    drift_vector JSONB NOT NULL DEFAULT '[]'::jsonb,
    diffusion_matrix JSONB NOT NULL DEFAULT '[]'::jsonb,
    jump_intensity FLOAT8 NOT NULL DEFAULT 0.0,
    jump_distribution JSONB NOT NULL DEFAULT '{}'::jsonb,
    jump_compensator FLOAT8 NOT NULL DEFAULT 0.0,

    -- Coeficiente de Divergencia de Estado (CDC)
    cdc_value FLOAT8 NOT NULL,
    cdc_critical_threshold FLOAT8 NOT NULL DEFAULT 2.706,
    cdc_is_inefficiency BOOLEAN NOT NULL GENERATED ALWAYS AS (
        cdc_value > cdc_critical_threshold
    ) STORED,
    cdc_confidence FLOAT8 NOT NULL CHECK (cdc_confidence BETWEEN 0.0 AND 1.0),

    -- Estado predicho y envolvente de varianza
    predicted_state JSONB NOT NULL DEFAULT '[]'::jsonb,
    variance_envelope FLOAT8 NOT NULL DEFAULT 0.0,
    optimal_intervention_time FLOAT8,

    -- Metadatos de ejecución
    block_number BIGINT,
    block_timestamp TIMESTAMPTZ,
    execution_time_ms INTEGER,

    -- Estado de honestidad del repo
    status VARCHAR(16) NOT NULL DEFAULT 'PENDIENTE' 
        CHECK (status IN ('OK', 'PARCIAL', 'PENDIENTE', 'BLOQUEADO')),

    -- Telemetría de infraestructura
    kill_switch_state VARCHAR(16),
    prerequisite_services JSONB DEFAULT '[]'::jsonb,

    CONSTRAINT chk_cdc_confidence_range CHECK (cdc_confidence >= 0.0 AND cdc_confidence <= 1.0)
);

CREATE INDEX IF NOT EXISTS idx_sed_filtrations_market_time 
    ON sed_filtrations(market_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_sed_filtrations_inefficiency 
    ON sed_filtrations(cdc_is_inefficiency, created_at DESC) 
    WHERE cdc_is_inefficiency = TRUE;

CREATE INDEX IF NOT EXISTS idx_sed_filtrations_status 
    ON sed_filtrations(status);

CREATE INDEX IF NOT EXISTS idx_sed_filtrations_block 
    ON sed_filtrations(block_number, chain_id);

-- Trigger para updated_at
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

DROP TRIGGER IF EXISTS trigger_sed_filtrations_updated_at ON sed_filtrations;
CREATE TRIGGER trigger_sed_filtrations_updated_at
    BEFORE UPDATE ON sed_filtrations
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();


-- ============================================================
-- Migration 013: sed_eigenstates
-- Sprint: S4+
-- Módulo: eigenstate_transition_projector.rs
-- Descripción: Estados propios y fronteras de equilibrio
-- ============================================================

CREATE TABLE IF NOT EXISTS sed_eigenstates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identificación
    market_id VARCHAR(64) NOT NULL,
    token_pair VARCHAR(32) NOT NULL,
    chain_id INTEGER NOT NULL DEFAULT 1,

    -- Parámetros del Hamiltoniano efectivo
    hamiltonian_kinetic JSONB NOT NULL DEFAULT '[]'::jsonb,
    hamiltonian_potential JSONB NOT NULL DEFAULT '[]'::jsonb,
    metric_tensor JSONB NOT NULL DEFAULT '[]'::jsonb,
    discretization_grid JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Eigenstates resueltos
    eigenstate_index INTEGER NOT NULL,
    eigenstate_energy FLOAT8 NOT NULL,
    eigenstate_amplitude JSONB NOT NULL DEFAULT '[]'::jsonb,
    eigenstate_degeneracy INTEGER NOT NULL DEFAULT 1,
    quantum_numbers JSONB DEFAULT '[]'::jsonb,

    -- Estado de equilibrio objetivo
    is_equilibrium_state BOOLEAN NOT NULL DEFAULT FALSE,
    equilibrium_probability FLOAT8,
    transition_amplitude JSONB,  -- { re: float, im: float }

    -- Frontera de equilibrio post-perturbación
    boundary_hypersurface JSONB DEFAULT '[]'::jsonb,
    critical_probability FLOAT8,
    convergence_radius FLOAT8,
    stability_exponent FLOAT8,

    -- Metadatos de simulación
    block_number BIGINT,
    simulation_duration_ms INTEGER,
    lanczos_iterations INTEGER,
    lanczos_tolerance FLOAT8 DEFAULT 1e-10,

    -- Estado de honestidad
    status VARCHAR(16) NOT NULL DEFAULT 'PENDIENTE'
        CHECK (status IN ('OK', 'PARCIAL', 'PENDIENTE', 'BLOQUEADO')),

    -- Telemetría
    kill_switch_state VARCHAR(16),
    prerequisite_services JSONB DEFAULT '[]'::jsonb,

    -- Relación con filtración (opcional)
    filtration_id UUID REFERENCES sed_filtrations(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_sed_eigenstates_market_index 
    ON sed_eigenstates(market_id, eigenstate_index);

CREATE INDEX IF NOT EXISTS idx_sed_eigenstates_equilibrium 
    ON sed_eigenstates(is_equilibrium_state, created_at DESC) 
    WHERE is_equilibrium_state = TRUE;

CREATE INDEX IF NOT EXISTS idx_sed_eigenstates_filtration 
    ON sed_eigenstates(filtration_id) 
    WHERE filtration_id IS NOT NULL;

DROP TRIGGER IF EXISTS trigger_sed_eigenstates_updated_at ON sed_eigenstates;
CREATE TRIGGER trigger_sed_eigenstates_updated_at
    BEFORE UPDATE ON sed_eigenstates
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();


-- ============================================================
-- Migration 014: sed_allocations
-- Sprint: S5+
-- Módulo: dirac_manifold_allocator.rs
-- Descripción: Asignaciones de control óptimo e impulsos Dirac
-- ============================================================

CREATE TABLE IF NOT EXISTS sed_allocations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identificación
    market_id VARCHAR(64) NOT NULL,
    token_pair VARCHAR(32) NOT NULL,
    chain_id INTEGER NOT NULL DEFAULT 1,

    -- Variedad de liquidez CPMM
    constant_product NUMERIC(78, 0) NOT NULL,
    token0_reserve NUMERIC(78, 0) NOT NULL,
    token1_reserve NUMERIC(78, 0) NOT NULL,
    metric_tensor JSONB NOT NULL DEFAULT '[]'::jsonb,
    pool_address VARCHAR(42),

    -- Control óptimo
    optimal_control JSONB NOT NULL DEFAULT '[]'::jsonb,
    value_functional FLOAT8 NOT NULL DEFAULT 0.0,
    costate_trajectory JSONB NOT NULL DEFAULT '[]'::jsonb,
    time_horizon FLOAT8 NOT NULL DEFAULT 0.0,

    -- Impulso de Dirac sobre variedad
    injection_point JSONB NOT NULL DEFAULT '[]'::jsonb,
    dirac_amplitude FLOAT8 NOT NULL DEFAULT 0.0,
    support_radius FLOAT8 NOT NULL DEFAULT 0.0,

    -- Restricciones hiperbólicas
    hyperbolic_constraint_satisfied BOOLEAN NOT NULL DEFAULT FALSE,
    hyperbolic_tolerance FLOAT8 NOT NULL DEFAULT 1e-9,
    hyperbolic_violation NUMERIC(78, 18),

    -- Ventana de ejecución
    execution_window_start TIMESTAMPTZ,
    execution_window_end TIMESTAMPTZ,

    -- Relación con eigenstate (fuente de frontera)
    eigenstate_id UUID REFERENCES sed_eigenstates(id) ON DELETE SET NULL,

    -- Bundle result (post-ejecución)
    bundle_hash VARCHAR(66),
    tx_hash VARCHAR(66),
    relay_response JSONB,

    -- Metadatos
    block_number BIGINT,
    solver_iterations INTEGER,
    solver_convergence_error FLOAT8,

    -- Estado de honestidad
    status VARCHAR(16) NOT NULL DEFAULT 'PENDIENTE'
        CHECK (status IN ('OK', 'PARCIAL', 'PENDIENTE', 'BLOQUEADO')),

    -- Telemetría
    kill_switch_state VARCHAR(16),
    prerequisite_services JSONB DEFAULT '[]'::jsonb,

    -- Métricas de varianza
    variance_captured FLOAT8,
    variance_target FLOAT8
);

CREATE INDEX IF NOT EXISTS idx_sed_allocations_market_time 
    ON sed_allocations(market_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_sed_allocations_hyperbolic_violation 
    ON sed_allocations(hyperbolic_constraint_satisfied) 
    WHERE hyperbolic_constraint_satisfied = FALSE;

CREATE INDEX IF NOT EXISTS idx_sed_allocations_eigenstate 
    ON sed_allocations(eigenstate_id) 
    WHERE eigenstate_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_sed_allocations_bundle 
    ON sed_allocations(bundle_hash) 
    WHERE bundle_hash IS NOT NULL;

DROP TRIGGER IF EXISTS trigger_sed_allocations_updated_at ON sed_allocations;
CREATE TRIGGER trigger_sed_allocations_updated_at
    BEFORE UPDATE ON sed_allocations
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();


-- ============================================================
-- Migration 015: sed_hedges
-- Sprint: S3+
-- Módulo: orthogonal_variance_hedger.rs
-- Descripción: Resultados de hedging ortogonal y entrelazamiento
-- ============================================================

CREATE TABLE IF NOT EXISTS sed_hedges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Mercados entrelazados
    market_a_id VARCHAR(64) NOT NULL,
    market_b_id VARCHAR(64) NOT NULL,
    token_pair_a VARCHAR(32) NOT NULL,
    token_pair_b VARCHAR(32) NOT NULL,
    chain_id INTEGER NOT NULL DEFAULT 1,

    -- Estado entrelazado |Ψ_AB⟩
    entangled_amplitude JSONB NOT NULL DEFAULT '[]'::jsonb,
    entanglement_entropy FLOAT8 NOT NULL DEFAULT 0.0,
    market_a_dimension INTEGER NOT NULL DEFAULT 2,
    market_b_dimension INTEGER NOT NULL DEFAULT 2,

    -- Hiperplano ortogonal Π_⟂
    normal_vector JSONB NOT NULL DEFAULT '[]'::jsonb,
    basis_vectors JSONB NOT NULL DEFAULT '[]'::jsonb,
    null_covariance_constraint JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Vectores direccionales
    original_direction JSONB NOT NULL DEFAULT '[]'::jsonb,
    projected_direction JSONB NOT NULL DEFAULT '[]'::jsonb,
    residual_direction JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Resultado de neutralización
    is_fully_neutralized BOOLEAN NOT NULL DEFAULT FALSE,
    orthogonality_error FLOAT8 NOT NULL DEFAULT 0.0,
    covariance_matrix JSONB NOT NULL DEFAULT '[]'::jsonb,
    covariance_off_diagonal FLOAT8 NOT NULL DEFAULT 0.0,

    -- Validación computada
    null_covariance_verified BOOLEAN NOT NULL GENERATED ALWAYS AS (
        ABS(covariance_off_diagonal) < 1e-9 AND orthogonality_error < 1e-9
    ) STORED,

    -- Relaciones con allocaciones
    allocation_a_id UUID REFERENCES sed_allocations(id) ON DELETE SET NULL,
    allocation_b_id UUID REFERENCES sed_allocations(id) ON DELETE SET NULL,

    -- Scoring para selector-api
    risk_score FLOAT8,
    hedge_efficiency FLOAT8,

    -- Metadatos
    block_number BIGINT,
    sync_duration_ms INTEGER,
    decoherence_rate FLOAT8,
    correlation_threshold FLOAT8,

    -- Estado de honestidad
    status VARCHAR(16) NOT NULL DEFAULT 'PENDIENTE'
        CHECK (status IN ('OK', 'PARCIAL', 'PENDIENTE', 'BLOQUEADO')),

    -- Telemetría
    kill_switch_state VARCHAR(16),
    prerequisite_services JSONB DEFAULT '[]'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_sed_hedges_markets_time 
    ON sed_hedges(market_a_id, market_b_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_sed_hedges_neutralized 
    ON sed_hedges(is_fully_neutralized, created_at DESC) 
    WHERE is_fully_neutralized = TRUE;

CREATE INDEX IF NOT EXISTS idx_sed_hedges_verified 
    ON sed_hedges(null_covariance_verified, created_at DESC) 
    WHERE null_covariance_verified = TRUE;

CREATE INDEX IF NOT EXISTS idx_sed_hedges_allocation_a 
    ON sed_hedges(allocation_a_id) 
    WHERE allocation_a_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_sed_hedges_allocation_b 
    ON sed_hedges(allocation_b_id) 
    WHERE allocation_b_id IS NOT NULL;

DROP TRIGGER IF EXISTS trigger_sed_hedges_updated_at ON sed_hedges;
CREATE TRIGGER trigger_sed_hedges_updated_at
    BEFORE UPDATE ON sed_hedges
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();


-- ============================================================
-- VISTAS DE AGREGACIÓN (Dashboard / Analytics)
-- ============================================================

CREATE OR REPLACE VIEW v_sed_convergence_dashboard AS
SELECT 
    DATE_TRUNC('hour', created_at) AS hour_bucket,
    market_id,
    COUNT(*) AS total_filtrations,
    SUM(CASE WHEN cdc_is_inefficiency THEN 1 ELSE 0 END) AS inefficiency_count,
    AVG(cdc_value) AS avg_cdc,
    MAX(cdc_value) AS max_cdc,
    AVG(variance_envelope) AS avg_variance
FROM sed_filtrations
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY hour_bucket, market_id
ORDER BY hour_bucket DESC, market_id;

CREATE OR REPLACE VIEW v_sed_equilibrium_boundary AS
SELECT 
    e.market_id,
    e.token_pair,
    e.eigenstate_index,
    e.eigenstate_energy,
    e.equilibrium_probability,
    e.convergence_radius,
    e.stability_exponent,
    f.cdc_value AS triggering_cdc,
    f.created_at AS filtration_time,
    e.created_at AS eigenstate_time
FROM sed_eigenstates e
LEFT JOIN sed_filtrations f ON e.filtration_id = f.id
WHERE e.is_equilibrium_state = TRUE
ORDER BY e.created_at DESC;

CREATE OR REPLACE VIEW v_sed_hedge_effectiveness AS
SELECT 
    h.market_a_id,
    h.market_b_id,
    h.token_pair_a,
    h.token_pair_b,
    h.is_fully_neutralized,
    h.null_covariance_verified,
    h.orthogonality_error,
    h.entanglement_entropy,
    h.risk_score,
    h.hedge_efficiency,
    a_a.value_functional AS allocation_a_value,
    a_b.value_functional AS allocation_b_value
FROM sed_hedges h
LEFT JOIN sed_allocations a_a ON h.allocation_a_id = a_a.id
LEFT JOIN sed_allocations a_b ON h.allocation_b_id = a_b.id
WHERE h.created_at > NOW() - INTERVAL '7 days'
ORDER BY h.created_at DESC;


-- ============================================================
-- FUNCIONES AUXILIARES
-- ============================================================

CREATE OR REPLACE FUNCTION get_recent_inefficiencies(
    p_market_id VARCHAR(64),
    p_limit INTEGER DEFAULT 100
)
RETURNS TABLE (
    id UUID,
    created_at TIMESTAMPTZ,
    cdc_value FLOAT8,
    cdc_confidence FLOAT8,
    predicted_state JSONB,
    variance_envelope FLOAT8
) AS $$
BEGIN
    RETURN QUERY
    SELECT f.id, f.created_at, f.cdc_value, f.cdc_confidence, 
           f.predicted_state, f.variance_envelope
    FROM sed_filtrations f
    WHERE f.market_id = p_market_id 
      AND f.cdc_is_inefficiency = TRUE
    ORDER BY f.created_at DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION get_hedge_pairs_with_null_covariance(
    p_tolerance FLOAT8 DEFAULT 1e-9
)
RETURNS TABLE (
    market_a_id VARCHAR(64),
    market_b_id VARCHAR(64),
    covariance_off_diagonal FLOAT8,
    orthogonality_error FLOAT8,
    created_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT h.market_a_id, h.market_b_id, h.covariance_off_diagonal,
           h.orthogonality_error, h.created_at
    FROM sed_hedges h
    WHERE ABS(h.covariance_off_diagonal) < p_tolerance
      AND h.orthogonality_error < p_tolerance
    ORDER BY h.created_at DESC;
END;
$$ LANGUAGE plpgsql;


-- ============================================================
-- COMENTARIOS DE DOCUMENTACIÓN
-- ============================================================

COMMENT ON TABLE sed_filtrations IS 'Telemetría de filtración estocástica: procesos de Markov con saltos y CDC';
COMMENT ON TABLE sed_eigenstates IS 'Estados propios del Hamiltoniano efectivo y fronteras de equilibrio';
COMMENT ON TABLE sed_allocations IS 'Asignaciones de control óptimo e impulsos de Dirac sobre variedades de liquidez';
COMMENT ON TABLE sed_hedges IS 'Resultados de hedging ortogonal y estados entrelazados inter-mercado';

COMMENT ON COLUMN sed_filtrations.cdc_value IS 'Coeficiente de Divergencia de Estado. > 2.706 indica ineficiencia transitoria';
COMMENT ON COLUMN sed_eigenstates.eigenstate_energy IS 'Autovalor E_n del Hamiltoniano efectivo';
COMMENT ON COLUMN sed_allocations.hyperbolic_constraint_satisfied IS 'Verificación de x*y = k dentro de tolerancia';
COMMENT ON COLUMN sed_hedges.null_covariance_verified IS 'Verificación computada: |Cov(A,B)| < 1e-9 AND orthogonality_error < 1e-9';
