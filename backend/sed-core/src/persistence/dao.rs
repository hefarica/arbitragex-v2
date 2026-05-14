//! `dao` — Data Access Objects for SED pipeline persistence.
//!
//! Each DAO corresponds to one of the four SED tables from migration 062.
//! All insertions use parameterized queries via sqlx. No hardcoded SQL
//! values. No fabricated data. R8 Fail-Honest: if the DB is down, the
//! error propagates — we do NOT return dummy UUIDs.

use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Result alias for persistence operations.
pub type PersistenceResult<T> = Result<T, PersistenceError>;

/// Error types for persistence operations.
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("database query failed: {0}")]
    QueryFailed(String),

    #[error("serialization failed: {0}")]
    SerializationFailed(String),

    #[error("record not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },
}

/// Insert parameters for `sed_filtrations`.
#[derive(Debug, Clone)]
pub struct FiltrationInsert {
    pub market_id: String,
    pub token_pair: String,
    pub chain_id: i32,
    pub cdc_value: f64,
    pub cdc_confidence: f64,
    pub predicted_state: JsonValue,
    pub drift_vector: JsonValue,
    pub diffusion_matrix: JsonValue,
    pub jump_intensity: f64,
    pub variance_envelope: f64,
    pub block_number: Option<i64>,
    pub execution_time_ms: Option<i32>,
}

/// Insert parameters for `sed_eigenstates`.
#[derive(Debug, Clone)]
pub struct EigenstateInsert {
    pub market_id: String,
    pub token_pair: String,
    pub chain_id: i32,
    pub eigenstate_index: i32,
    pub eigenstate_energy: f64,
    pub eigenstate_amplitude: JsonValue,
    pub spectral_gap: f64,
    pub equilibrium_probability: Option<f64>,
    pub convergence_radius: Option<f64>,
    pub is_equilibrium_state: bool,
    pub lanczos_iterations: Option<i32>,
    pub filtration_id: Option<Uuid>,
    pub block_number: Option<i64>,
}

/// Insert parameters for `sed_allocations`.
#[derive(Debug, Clone)]
pub struct AllocationInsert {
    pub market_id: String,
    pub token_pair: String,
    pub chain_id: i32,
    pub constant_product: f64,
    pub token0_reserve: f64,
    pub token1_reserve: f64,
    pub optimal_control: JsonValue,
    pub value_functional: f64,
    pub dirac_amplitude: f64,
    pub support_radius: f64,
    pub injection_point: JsonValue,
    pub hyperbolic_constraint_satisfied: bool,
    pub hyperbolic_violation: Option<f64>,
    pub solver_iterations: Option<i32>,
    pub solver_convergence_error: Option<f64>,
    pub eigenstate_id: Option<Uuid>,
    pub block_number: Option<i64>,
}

/// Insert parameters for `sed_hedges`.
#[derive(Debug, Clone)]
pub struct HedgeInsert {
    pub market_a_id: String,
    pub market_b_id: String,
    pub token_pair_a: String,
    pub token_pair_b: String,
    pub chain_id: i32,
    pub entangled_amplitude: JsonValue,
    pub entanglement_entropy: f64,
    pub covariance_matrix: JsonValue,
    pub covariance_off_diagonal: f64,
    pub is_fully_neutralized: bool,
    pub orthogonality_error: f64,
    pub residual_direction: JsonValue,
    pub allocation_a_id: Option<Uuid>,
    pub allocation_b_id: Option<Uuid>,
    pub block_number: Option<i64>,
}

/// Read result from `sed_filtrations`.
#[derive(Debug, Clone)]
pub struct FiltrationRecord {
    pub id: Uuid,
    pub market_id: String,
    pub cdc_value: f64,
    pub cdc_confidence: f64,
    pub cdc_is_inefficiency: bool,
    pub status: String,
}

/// Read result from `sed_hedges`.
#[derive(Debug, Clone)]
pub struct HedgeRecord {
    pub id: Uuid,
    pub market_a_id: String,
    pub market_b_id: String,
    pub is_fully_neutralized: bool,
    pub covariance_off_diagonal: f64,
    pub orthogonality_error: f64,
    pub status: String,
}

/// Data Access Object for SED filtrations.
///
/// Writes to `sed_filtrations` table. Status defaults to 'PENDIENTE'.
pub struct SedFiltrationDao;

impl SedFiltrationDao {
    /// Insert a new filtration record. Returns the generated UUID.
    ///
    /// The SQL uses `$N` placeholders via sqlx. No hardcoded values.
    pub fn insert_sql() -> &'static str {
        r#"INSERT INTO sed_filtrations 
           (market_id, token_pair, chain_id, cdc_value, cdc_confidence,
            predicted_state, drift_vector, diffusion_matrix, jump_intensity,
            variance_envelope, block_number, execution_time_ms)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
           RETURNING id"#
    }

    /// Query recent inefficiencies for a market.
    pub fn recent_inefficiencies_sql() -> &'static str {
        r#"SELECT id, market_id, cdc_value, cdc_confidence, cdc_is_inefficiency, status
           FROM sed_filtrations
           WHERE market_id = $1 AND cdc_is_inefficiency = TRUE
           ORDER BY created_at DESC
           LIMIT $2"#
    }
}

/// Data Access Object for SED eigenstates.
pub struct SedEigenstateDao;

impl SedEigenstateDao {
    /// Insert a new eigenstate record. Returns the generated UUID.
    pub fn insert_sql() -> &'static str {
        r#"INSERT INTO sed_eigenstates
           (market_id, token_pair, chain_id, eigenstate_index, eigenstate_energy,
            eigenstate_amplitude, convergence_radius, equilibrium_probability,
            is_equilibrium_state, lanczos_iterations, filtration_id, block_number)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
           RETURNING id"#
    }
}

/// Data Access Object for SED allocations.
pub struct SedAllocationDao;

impl SedAllocationDao {
    /// Insert a new allocation record. Returns the generated UUID.
    pub fn insert_sql() -> &'static str {
        r#"INSERT INTO sed_allocations
           (market_id, token_pair, chain_id, constant_product, token0_reserve,
            token1_reserve, optimal_control, value_functional, dirac_amplitude,
            support_radius, injection_point, hyperbolic_constraint_satisfied,
            hyperbolic_violation, solver_iterations, solver_convergence_error,
            eigenstate_id, block_number)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
           RETURNING id"#
    }
}

/// Data Access Object for SED hedges.
pub struct SedHedgeDao;

impl SedHedgeDao {
    /// Insert a new hedge record. Returns the generated UUID.
    pub fn insert_sql() -> &'static str {
        r#"INSERT INTO sed_hedges
           (market_a_id, market_b_id, token_pair_a, token_pair_b, chain_id,
            entangled_amplitude, entanglement_entropy, covariance_matrix,
            covariance_off_diagonal, is_fully_neutralized, orthogonality_error,
            residual_direction, allocation_a_id, allocation_b_id, block_number)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
           RETURNING id"#
    }

    /// Query verified hedges (null covariance confirmed).
    pub fn verified_hedges_sql() -> &'static str {
        r#"SELECT id, market_a_id, market_b_id, is_fully_neutralized,
                  covariance_off_diagonal, orthogonality_error, status
           FROM sed_hedges
           WHERE null_covariance_verified = TRUE
           ORDER BY created_at DESC
           LIMIT $1"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtration_insert_has_12_placeholders() {
        let sql = SedFiltrationDao::insert_sql();
        let count = sql.matches('$').count();
        assert_eq!(count, 12, "Expected 12 bind parameters, got {count}");
    }

    #[test]
    fn eigenstate_insert_has_12_placeholders() {
        let sql = SedEigenstateDao::insert_sql();
        let count = sql.matches('$').count();
        assert_eq!(count, 12, "Expected 12 bind parameters, got {count}");
    }

    #[test]
    fn allocation_insert_has_17_placeholders() {
        let sql = SedAllocationDao::insert_sql();
        let count = sql.matches('$').count();
        assert_eq!(count, 17, "Expected 17 bind parameters, got {count}");
    }

    #[test]
    fn hedge_insert_has_15_placeholders() {
        let sql = SedHedgeDao::insert_sql();
        let count = sql.matches('$').count();
        assert_eq!(count, 15, "Expected 15 bind parameters, got {count}");
    }

    #[test]
    fn filtration_insert_contains_returning_id() {
        assert!(SedFiltrationDao::insert_sql().contains("RETURNING id"));
    }

    #[test]
    fn hedge_verified_query_filters_by_null_covariance() {
        assert!(SedHedgeDao::verified_hedges_sql().contains("null_covariance_verified = TRUE"));
    }
}
