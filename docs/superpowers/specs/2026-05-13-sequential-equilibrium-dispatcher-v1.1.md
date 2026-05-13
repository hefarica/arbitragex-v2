# Sequential Equilibrium Dispatcher (SED)
## Design Specification Document — Versión 1.1

**Protocol:** OMEGA AGENT TEAMS  
**Doctrine:** Autonomía Total Controlada y Ejecución Enterprise (Zero-Prompt)  
**Directiva Estratégica:** Convergencia Estocástica Autónoma  
**Date:** 2026-05-13  
**Classification:** TOP SECRET — QUANTITATIVE ARCHITECTURE  
**Version:** 1.1.0-ENTERPRISE  
**Path:** `docs/superpowers/specs/2026-05-13-sequential-equilibrium-dispatcher-v1.1.md`

---

## Changelog v1.0 → v1.1

| Cambio | Motivación | Sección |
|--------|-----------|---------|
| Crate unificada `sed-core/` | Aislamiento matemático + consumo por features | §3.5 |
| Integración Kill-Switch + 501 | Cumplimiento de reglas obligatorias del repo | §4.6, §6.4 |
| Esquema de persistencia SQL | Telemetría estocástica versionada | §7 |
| Roadmap Mapping S2-S6 | Alineación con `ROADMAP_FASES.md` | §8 |
| Glosario Cuántico `GLOSSARY_QUANT.md` | Unificación terminología Rust↔TS | Apéndice A |
| `BundlePosition` + `KillSwitchGate` | Tipado terminal con prerequisitos de infraestructura | §4.7 |

---

## 1. Resumen Ejecutivo

El **Sequential Equilibrium Dispatcher (SED) v1.1** es la capa de orquestación matemática de más alto nivel dentro de la arquitectura OMEGA, encapsulada en la crate unificada `backend/sed-core/`. Su propósito es mantener el **equilibrio asintótico del sistema** mediante la integración óptima del área bajo la curva de varianza de la red (AUC-V), operando como estabilizador de red y gestor de riesgo institucional.

**Principio rector v1.1:** *"Si compila, está matemáticamente garantizado. Si el kill-switch está activo, no ejecuta. Si falta infraestructura, responde 501. Si persiste, queda auditado."*

---

## 2. Marco Teórico Matemático

### 2.1 Espacio de Estados del Mercado (ℳ)

Variedad Riemanniana ℳ de dimensión *n*, métrica inducida por CPMM de Uniswap V2:

```
gᵢⱼ(p) = δᵢⱼ · (∂²L/∂rᵢ∂rⱼ)
```

### 2.2 Proceso Estocástico de Perturbación (PDMP)

```
dXₜ = μ(Xₜ, t)dt + σ(Xₜ, t)dWₜ + ∫ᴢ J(Xₜ₋, z)N(dt, dz)
```

### 2.3 Variedades de Liquidez y Funciones Impulso

```
ℒₖ = { (x, y) ∈ ℝ²⁺ | x · y = k }
δℒ(Δ) = lim_{ε→0} ∫_{ℒ} f(p) · δ_{ε}(p - p₀) dvol_g(p)
```

### 2.4 Espacios de Hilbert Aislados (REVM)

```
ℋ_iso = L²(ℳ, μ_g)
Ĥ = -½∇²_g + V_eff(p)
V_eff(p) = (r₁·r₂ - k)² / (2σ²)
```

---

## 3. Arquitectura de Módulos Canónicos

### 3.1 Módulo I: `stochastic_wave_filtration.rs`

#### 3.1.1 Propósito
Filtración de procesos de Markov con saltos. Cálculo del **Coeficiente de Divergencia de Estado (CDC)**.

#### 3.1.2 Formulación

```
ℱₜ = σ({Xₛ : 0 ≤ s ≤ t})
CDC(t) = D_KL( ℙ_{mempool} || ℙ_{equilibrio} | ℱₜ )
       = ½∫₀ᵗ ||σ⁻¹(μ - μ̂)||² ds + ∫₀ᵗ∫ᴢ [φ(s,z)log(φ(s,z)) - φ(s,z) + 1] ν(dz)ds
```

#### 3.1.3 Tipos Rust

```rust
use nalgebra::{DVector, DMatrix};
use rand_distr::{Distribution, Normal};

pub struct MarkovJumpProcess {
    pub drift: DVector<f64>,
    pub diffusion: DMatrix<f64>,
    pub jump_measure: PoissonRandomMeasure,
    pub filtration: NaturalFiltration,
}

pub struct PoissonRandomMeasure {
    pub intensity: f64,
    pub jump_distribution: JumpDistribution,
    pub compensator: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateDivergenceCoefficient {
    pub value: f64,
    pub timestamp: u64,
    pub confidence: f64,
}

impl StateDivergenceCoefficient {
    pub const CRITICAL_THRESHOLD: f64 = 2.706;
    pub fn is_transient_inefficiency(&self) -> bool {
        self.value > Self::CRITICAL_THRESHOLD && self.confidence > 0.95
    }
}

pub struct FiltrationResult {
    pub cdc: StateDivergenceCoefficient,
    pub predicted_state: DVector<f64>,
    pub variance_envelope: f64,
    pub optimal_intervention_time: f64,
    pub market_id: String,
    pub token_pair: (String, String),
}
```

#### 3.1.4 Algoritmo

1. Ingesta de MemPool como proceso puntual.
2. Estimación ML recursiva de μ, σ, λ.
3. Simulación Monte Carlo condicionada a ℱₜ.
4. Cálculo de CDC.
5. Emisión de `FiltrationResult` si CDC > CDC_CRITICO.

---

### 3.2 Módulo II: `eigenstate_transition_projector.rs`

#### 3.2.1 Propósito
Resolución de autovectores sobre REVM en ℋ_iso. Determinación de fronteras exactas del equilibrio post-perturbación.

#### 3.2.2 Formulación

```
Û(t) = exp(-iĤt/ℏ)
Ĥ|ψₙ⟩ = Eₙ|ψₙ⟩
P_eq = |⟨ψ_eq|ψ₀⟩|² = |c_eq|²
∂Ω_eq = { p ∈ ℳ | ⟨p|ψ_eq⟩⟨ψ_eq|p⟩ = P_CRITICO }
```

#### 3.2.3 Tipos Rust

```rust
use nalgebra::{Complex, DVector, DMatrix};
use num_complex::Complex64;

pub struct EigenState {
    pub amplitude: DVector<Complex64>,
    pub energy: f64,
    pub degeneracy: u32,
    pub quantum_numbers: Vec<u32>,
}

pub struct EffectiveHamiltonian {
    pub kinetic_term: DMatrix<Complex64>,
    pub potential_term: DMatrix<Complex64>,
    pub metric: RiemannianMetric,
}

pub struct TransitionProjector {
    pub initial_state: EigenState,
    pub target_state: EigenState,
    pub transition_amplitude: Complex64,
    pub probability: f64,
}

pub struct EquilibriumBoundary {
    pub boundary_hypersurface: Vec<DVector<f64>>,
    pub critical_probability: f64,
    pub convergence_radius: f64,
    pub stability_exponent: f64,
}

impl EquilibriumBoundary {
    pub fn contains(&self, p: &DVector<f64>) -> bool {
        let distance = self.signed_distance(p);
        distance < 0.0 && distance.abs() < self.convergence_radius
    }
    fn signed_distance(&self, p: &DVector<f64>) -> f64 {
        todo!("Distancia geodésica firmada a hipersuperficie")
    }
}
```

#### 3.2.4 Algoritmo

1. Discretización espectral de ℳ.
2. Construcción de Ĥ sparse.
3. Resolución Lanczos de primeros *k* eigenstates.
4. Cálculo de proyecciones ⟨ψₙ|ψ₀⟩.
5. Determinación de ∂Ω_eq por bisección sobre P_eq.
6. Emisión de `EquilibriumBoundary`.

---

### 3.3 Módulo III: `dirac_manifold_allocator.rs`

#### 3.3.1 Propósito
Inyección de funciones impulso de Dirac sobre variedades de liquidez. Control óptimo para maximizar varianza capturada sujeto a restricciones hiperbólicas CPMM.

#### 3.3.2 Formulación

**Problema de Control Óptimo:**

```
J[u] = ∫₀ᵀ Var(Xₜ | ℱₜ) dt → max
s.a.  dXₜ = f(Xₜ, uₜ)dt + σdWₜ
      x(t)·y(t) = k, ∀t ∈ [0,T]
      uₜ ∈ 𝒰_admisible
      X_T ∈ ∂Ω_eq
```

Hamiltoniano de Pontryagin:

```
ℋ(x, y, p_x, p_y, u) = p_x·f₁ + p_y·f₂ - L(x,y,u)
```

#### 3.3.3 Tipos Rust

```rust
use nalgebra::{DVector, DMatrix, Vector2};

pub struct LiquidityManifold {
    pub constant_product: f64,
    pub token0_reserve: f64,
    pub token1_reserve: f64,
    pub metric_tensor: DMatrix<f64>,
}

pub struct DiracImpulse {
    pub manifold: LiquidityManifold,
    pub injection_point: DVector<f64>,
    pub amplitude: f64,
    pub support_radius: f64,
}

impl DiracImpulse {
    pub fn evaluate(&self, point: &DVector<f64>) -> f64 {
        let distance = self.geodesic_distance(point);
        if distance < self.support_radius {
            self.amplitude * (1.0 / (self.support_radius * std::f64::consts::PI.sqrt()))
                * (-distance.powi(2) / self.support_radius.powi(2)).exp()
        } else { 0.0 }
    }
    fn geodesic_distance(&self, point: &DVector<f64>) -> f64 {
        let dx = point[0] - self.injection_point[0];
        let dy = point[1] - self.injection_point[1];
        (dx.powi(2) / self.token0_reserve.powi(2) + dy.powi(2) / self.token1_reserve.powi(2)).sqrt()
    }
}

pub struct VarianceFunctional {
    pub trajectory: Vec<DVector<f64>>,
    pub variance_integral: f64,
    pub time_horizon: f64,
}

pub struct OptimalControlSolution {
    pub optimal_control: DVector<f64>,
    pub value_functional: f64,
    pub costate_trajectory: Vec<DVector<f64>>,
    pub manifold_injection: DiracImpulse,
    pub hyperbolic_constraint_satisfied: bool,
    pub execution_window: (u64, u64),
}

pub struct HyperbolicConstraint {
    pub constant_product: f64,
    pub tolerance: f64,
}

impl HyperbolicConstraint {
    pub fn verify(&self, x: f64, y: f64) -> bool {
        (x * y - self.constant_product).abs() < self.tolerance
    }
}
```

#### 3.3.4 Algoritmo

1. Recepción de `EquilibriumBoundary`.
2. Formulación del OCP con restricciones.
3. Resolución numérica (colocación pseudoespectral / disparo múltiple).
4. Verificación x·y = k en cada paso.
5. Construcción de `DiracImpulse` en punto óptimo.
6. Emisión de `OptimalControlSolution`.

---

### 3.4 Módulo IV: `orthogonal_variance_hedger.rs`

#### 3.4.1 Propósito
Sincronización de estados entrelazados en hiperplanos ortogonales inter-mercado. Covarianza nula y neutralización total del vector direccional.

#### 3.4.2 Formulación

```
|Ψ_AB⟩ ≠ |ψ_A⟩ ⊗ |ψ_B⟩
ρ_A = Tr_B(|Ψ_AB⟩⟨Ψ_AB|)
S(ρ_A) = -Tr(ρ_A log ρ_A)
Π_⟂ = { (Δ_A, Δ_B) | Cov(Δ_A, Δ_B) = 0 }
v_⟂ = v - ⟨v, n⟩ / ||n||² · n
```

#### 3.4.3 Tipos Rust

```rust
use nalgebra::{DVector, DMatrix, Matrix2};
use num_complex::Complex64;

pub struct EntangledState {
    pub amplitude: DVector<Complex64>,
    pub market_a_dimension: usize,
    pub market_b_dimension: usize,
    pub entanglement_entropy: f64,
}

pub struct OrthogonalHyperplane {
    pub normal_vector: DVector<f64>,
    pub basis_vectors: Vec<DVector<f64>>,
    pub null_covariance_constraint: DMatrix<f64>,
}

pub struct DirectionalVector {
    pub components: DVector<f64>,
    pub market_origin: String,
    pub norm: f64,
}

pub struct OrthogonalHedgeResult {
    pub hedged_state: EntangledState,
    pub covariance_matrix: DMatrix<f64>,
    pub is_fully_neutralized: bool,
    pub residual_direction: DVector<f64>,
    pub orthogonality_error: f64,
}

impl OrthogonalHedgeResult {
    pub fn verify_null_covariance(&self, tolerance: f64) -> bool {
        let off_diagonal = self.covariance_matrix[(0, 1)].abs();
        off_diagonal < tolerance && self.orthogonality_error < tolerance
    }
}

pub struct EntanglementSynchronizer {
    pub correlation_threshold: f64,
    pub decoherence_rate: f64,
}
```

#### 3.4.4 Algoritmo

1. Recepción de estados de mercados A y B.
2. Cálculo de Cov(Δ_A, Δ_B) por muestreo estocástico.
3. Construcción de Π_⟂ con base ortonormal.
4. Proyección del vector direccional combinado.
5. Verificación ||v_residual|| < ε y Cov ≈ 0.
6. Emisión de `OrthogonalHedgeResult`.

---

## 4. Sistema de Tipado Terminal y Control de Infraestructura

### 4.1 `BundlePosition<T>` — Tipado Terminal

```rust
use std::marker::PhantomData;

pub struct Unresolved;
pub struct Resolved;
pub struct OrthogonalEquilibrium;
pub struct DiracImpulseOnly;

pub struct BundlePosition<T> {
    pub market_id: String,
    pub token_pair: (String, String),
    pub liquidity_commitment: f64,
    pub variance_exposure: f64,
    pub topology_state: PhantomData<T>,
}

pub trait PostResolutionTopology: private::Sealed {}
mod private {
    pub trait Sealed {}
    impl Sealed for super::OrthogonalEquilibrium {}
    impl Sealed for super::DiracImpulseOnly {}
}
impl PostResolutionTopology for OrthogonalEquilibrium {}
impl PostResolutionTopology for DiracImpulseOnly {}

impl BundlePosition<OrthogonalEquilibrium> {
    pub fn new_orthogonal_equilibrium(
        market_id: String,
        token_pair: (String, String),
        liquidity_commitment: f64,
        variance_exposure: f64,
        orthogonality_proof: &OrthogonalHedgeResult,
    ) -> Result<Self, TopologyValidationError> {
        if !orthogonality_proof.verify_null_covariance(1e-9) {
            return Err(TopologyValidationError::NonOrthogonalCovariance);
        }
        Ok(Self { market_id, token_pair, liquidity_commitment, variance_exposure, topology_state: PhantomData })
    }
}

impl BundlePosition<DiracImpulseOnly> {
    pub fn new_dirac_impulse_only(
        market_id: String,
        token_pair: (String, String),
        liquidity_commitment: f64,
        variance_exposure: f64,
        dirac_solution: &OptimalControlSolution,
    ) -> Result<Self, TopologyValidationError> {
        if !dirac_solution.hyperbolic_constraint_satisfied {
            return Err(TopologyValidationError::HyperbolicViolation);
        }
        Ok(Self { market_id, token_pair, liquidity_commitment, variance_exposure, topology_state: PhantomData })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopologyValidationError {
    NonOrthogonalCovariance,
    HyperbolicViolation,
    EigenstateUnresolved,
    FiltrationIncomplete,
    InvalidManifold,
}
```

### 4.2 `KillSwitchGate` — Control de Infraestructura

```rust
use serde::{Deserialize, Serialize};

/// Estado del kill-switch global consultado antes de cualquier acción crítica
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum KillSwitchState {
    Active,      // Sistema operativo normal
    Suspended,   // Pausa temporal, no nuevos despachos
    Terminated,  // Apagado total, rechazar todo
}

/// Gate de validación que bloquea el pipeline si el kill-switch no está Active
pub struct KillSwitchGate;

impl KillSwitchGate {
    /// Consulta el estado del kill-switch vía shared-rs/config
    pub async fn check() -> Result<KillSwitchState, InfrastructureError> {
        // Integración con backend/shared-rs/ health + config
        // Endpoint: GET /admin/killswitch (api-server)
        todo!("Consultar estado vía HTTP o lectura de config centralizada")
    }

    /// Valida que el estado sea Active. Si no, retorna error bloqueante.
    pub fn require_active(state: KillSwitchState) -> Result<(), DispatchError> {
        match state {
            KillSwitchState::Active => Ok(()),
            KillSwitchState::Suspended => Err(DispatchError::KillSwitchSuspended),
            KillSwitchState::Terminated => Err(DispatchError::KillSwitchTerminated),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DispatchError {
    TopologyMismatch,
    EquilibriumBoundaryViolation,
    VarianceOverflow,
    NetworkDesynchronization,
    KillSwitchSuspended,
    KillSwitchTerminated,
    InfrastructureNotReady { requires: Vec<String>, sprint: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum InfrastructureError {
    ConfigUnreadable,
    NetworkPartition,
    ServiceUnavailable,
}
```

### 4.3 `InfrastructurePrerequisite` — Validación de 501

```rust
/// Prerrequisito de infraestructura para cada módulo SED
pub struct InfrastructurePrerequisite {
    pub required_services: Vec<String>,
    pub minimum_sprint: String,
    pub fallback_response: NotImplementedResponse,
}

/// Respuesta 501 canónica del repo
#[derive(Debug, Clone, Serialize)]
pub struct NotImplementedResponse {
    pub requires: Vec<String>,
    pub sprint: String,
    pub message: String,
}

impl InfrastructurePrerequisite {
    /// Verifica que todos los servicios requeridos estén healthy
    pub async fn verify(&self, health_checker: &HealthChecker) -> Result<(), NotImplementedResponse> {
        let unhealthy: Vec<String> = self.required_services.iter()
            .filter(|s| !health_checker.is_healthy(s).await)
            .cloned()
            .collect();

        if !unhealthy.is_empty() {
            return Err(NotImplementedResponse {
                requires: unhealthy,
                sprint: self.minimum_sprint.clone(),
                message: format!("SED module requires infrastructure not yet available in {}", self.minimum_sprint),
            });
        }
        Ok(())
    }
}

/// Health checker integrado con shared-rs
pub struct HealthChecker;
impl HealthChecker {
    pub async fn is_healthy(&self, service: &str) -> bool {
        // Consulta endpoint /status de cada servicio
        todo!("Integrar con shared-rs health checks")
    }
}
```

### 4.4 Pipeline SED con Kill-Switch y 501

```
┌─────────────────────────────────────────────────────────────────────┐
│                    MEMPOOL RAW DATA                                │
└──────────────────────┬──────────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  [GATE 1] InfrastructurePrerequisite::verify()                       │
│  • Requiere: mempool-node, websocket-conn                            │
│  • Sprint mínimo: S2                                                │
│  • Si falla → 501 {requires:["mempool-node"], sprint:"S2"}         │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OK
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  [GATE 2] KillSwitchGate::require_active()                         │
│  • Consulta /admin/killswitch                                       │
│  • Si Suspended → Err(KillSwitchSuspended)                          │
│  • Si Terminated → Err(KillSwitchTerminated)                        │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OK
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│     stochastic_wave_filtration.rs                                  │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │
│  │  Filtración │───▶│    CDC      │───▶│   Señal     │              │
│  │   Markov    │    │  Cálculo    │    │  Emisión    │              │
│  └─────────────┘    └─────────────┘    └─────────────┘              │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ FiltrationResult
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  [GATE 3] InfrastructurePrerequisite::verify()                       │
│  • Requiere: anvil-node, fork-url, revm-cache                       │
│  • Sprint mínimo: S4                                                │
│  • Si falla → 501 {requires:["anvil-node"], sprint:"S4"}           │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OK
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  [GATE 4] KillSwitchGate::require_active()                         │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OK
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│   eigenstate_transition_projector.rs                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │
│  │  Eigenstate │───▶│ Proyección  │───▶│  Frontera   │              │
│  │  Resolución │    │ Transición  │    │  Equilibrio │              │
│  └─────────────┘    └─────────────┘    └─────────────┘              │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ EquilibriumBoundary
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  [GATE 5] InfrastructurePrerequisite::verify()                       │
│  • Requiere: flashbots-relay, private-key, bundle-api               │
│  • Sprint mínimo: S5                                                │
│  • Si falla → 501 {requires:["flashbots-relay"], sprint:"S5"}      │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OK
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  [GATE 6] KillSwitchGate::require_active()                         │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OK
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│      dirac_manifold_allocator.rs                                     │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │
│  │    OCP      │───▶│   Dirac     │───▶│  Solución   │              │
│  │  Resolución │    │   Impulso   │    │   Óptima    │              │
│  └─────────────┘    └─────────────┘    └─────────────┘              │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OptimalControlSolution
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  [GATE 7] InfrastructurePrerequisite::verify()                       │
│  • Requiere: multi-market-feeds, correlation-engine                  │
│  • Sprint mínimo: S3                                                │
│  • Si falla → 501 {requires:["multi-market-feeds"], sprint:"S3"}   │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OK
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  [GATE 8] KillSwitchGate::require_active()                         │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OK
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│     orthogonal_variance_hedger.rs                                    │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │
│  │ Entangled   │───▶│  Proyección │───▶│  Hedge      │              │
│  │   States    │    │  Ortogonal  │    │  Result     │              │
│  └─────────────┘    └─────────────┘    └─────────────┘              │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OrthogonalHedgeResult
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│              BUNDLE POSITION (TIPADO TERMINAL)                     │
│   ┌─────────────────────┐   ┌─────────────────────┐                │
│   │ OrthogonalEquilibrium│   │  DiracImpulseOnly   │                │
│   │    (Covarianza 0)   │   │ (Restricción x·y=k) │                │
│   └─────────────────────┘   └─────────────────────┘                │
└──────────────────────┬──────────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  [GATE FINAL] KillSwitchGate + InfrastructurePrerequisite            │
│  • Requiere: rpc-node, gas-oracle, nonce-manager                     │
│  • Sprint mínimo: S5                                                │
│  • Si falla → 501 o Error de dispatch                              │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ OK
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│         SEQUENTIAL EQUILIBRIUM DISPATCHER (SED)                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  VALIDACIÓN FINAL:                                        │    │
│  │  • Tipo de topología verificado en compilación            │    │
│  │  • Evidencia matemática validada en runtime               │    │
│  │  • Kill-switch Active verificado                          │    │
│  │  • Infraestructura healthy verificada                     │    │
│  │  • Invariantes de equilibrio chequeadas                   │    │
│  └─────────────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  DESPACHO SECUENCIAL A RED:                                 │    │
│  │  • BundlePosition<OrthogonalEquilibrium>                    │    │
│  │  • BundlePosition<DiracImpulseOnly>                         │    │
│  └─────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 5. Crate Unificada `backend/sed-core/`

### 5.1 Estructura de Directorios

```
backend/sed-core/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── prelude.rs
│   ├── types/
│   │   ├── mod.rs
│   │   ├── bundle_position.rs
│   │   ├── kill_switch.rs
│   │   ├── infrastructure.rs
│   │   └── errors.rs
│   ├── filtration/
│   │   ├── mod.rs
│   │   ├── markov_jump.rs
│   │   ├── poisson_measure.rs
│   │   ├── cdc_calculator.rs
│   │   └── filtration_result.rs
│   ├── eigenstate/
│   │   ├── mod.rs
│   │   ├── hamiltonian.rs
│   │   ├── lanczos_solver.rs
│   │   ├── equilibrium_boundary.rs
│   │   └── transition_projector.rs
│   ├── allocator/
│   │   ├── mod.rs
│   │   ├── liquidity_manifold.rs
│   │   ├── dirac_impulse.rs
│   │   ├── optimal_control.rs
│   │   └── hyperbolic_constraint.rs
│   ├── hedger/
│   │   ├── mod.rs
│   │   ├── entangled_state.rs
│   │   ├── orthogonal_hyperplane.rs
│   │   ├── covariance_engine.rs
│   │   └── hedge_result.rs
│   ├── pipeline/
│   │   ├── mod.rs
│   │   ├── stochastic_pipeline.rs
│   │   ├── gate_manager.rs
│   │   └── checkpoint.rs
│   └── metrics/
│       ├── mod.rs
│       ├── sed_metrics.rs
│       └── convergence_tracker.rs
├── tests/
│   ├── unit/
│   │   ├── test_filtration.rs
│   │   ├── test_eigenstate.rs
│   │   ├── test_allocator.rs
│   │   └── test_hedger.rs
│   └── integration/
│       ├── test_pipeline_full.rs
│       ├── test_kill_switch_integration.rs
│       └── test_501_responses.rs
└── benches/
    ├── bench_filtration.rs
    ├── bench_eigenstate.rs
    └── bench_allocator.rs
```

### 5.2 `Cargo.toml`

```toml
[package]
name = "sed-core"
version = "1.1.0"
edition = "2021"
authors = ["OMEGA Agent Team <quant@arbitragex.io>"]
description = "Sequential Equilibrium Dispatcher — Capa matemática cuantitativa"
license = "PROPRIETARY"

[features]
default = ["filtration", "eigenstate", "allocator", "hedger", "metrics", "persistence"]

# Módulos matemáticos individuales (para consumo selectivo por crates operativos)
filtration = ["dep:nalgebra", "dep:rand", "dep:rand_distr"]
eigenstate = ["dep:nalgebra", "dep:num-complex", "dep:rand"]
allocator = ["dep:argmin", "dep:argmin-math", "dep:nalgebra"]
hedger = ["dep:nalgebra", "dep:num-complex", "dep:rand"]

# Infraestructura y observabilidad
metrics = ["dep:metrics", "dep:tracing"]
persistence = ["dep:sqlx", "dep:serde"]
kill_switch = ["dep:reqwest", "dep:serde"]

# Simulación EVM (pre-Anvil / lightweight)
revm_sim = ["dep:revm", "dep:alloy-primitives"]

[dependencies]
# Álgebra lineal
nalgebra = { version = "0.33", optional = true }
ndarray = { version = "0.15", optional = true }
num-complex = { version = "0.4", optional = true }
num-traits = "0.2"

# Procesos estocásticos
rand = { version = "0.8", optional = true }
rand_distr = { version = "0.4", optional = true }

# Optimización y control
argmin = { version = "0.10", optional = true }
argmin-math = { version = "0.4", optional = true }

# EVM / Blockchain
revm = { version = "14.0", optional = true }
alloy-primitives = { version = "0.7", optional = true }

# Async runtime
tokio = { version = "1.37", features = ["full"] }
futures = "0.3"

# HTTP / Infraestructura
reqwest = { version = "0.12", features = ["json"], optional = true }

# Serialización
serde = { version = "1.0", features = ["derive"], optional = true }
serde_json = "1.0"

# Observabilidad
metrics = { version = "0.22", optional = true }
tracing = { version = "0.1", optional = true }

# Persistencia
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "chrono", "uuid"], optional = true }

# Utilidades
thiserror = "1.0"
anyhow = "1.0"
chrono = "0.4"
uuid = { version = "1.8", features = ["v4", "serde"] }

# Crate compartida del workspace (path dependency)
shared-rs = { path = "../shared-rs" }

[dev-dependencies]
tokio-test = "0.4"
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1.4"
```

### 5.3 Consumo por Crates Operativos

#### `backend/searcher-rs/Cargo.toml`

```toml
[dependencies]
sed-core = { path = "../sed-core", features = ["filtration", "metrics"] }
shared-rs = { path = "../shared-rs" }
tokio = { version = "1.37", features = ["full"] }
```

#### `backend/sim-ctl/Cargo.toml`

```toml
[dependencies]
sed-core = { path = "../sed-core", features = ["eigenstate", "allocator", "revm_sim", "metrics"] }
shared-rs = { path = "../shared-rs" }
tokio = { version = "1.37", features = ["full"] }
```

#### `backend/relays-client/Cargo.toml`

```toml
[dependencies]
sed-core = { path = "../sed-core", features = ["allocator", "hedger", "kill_switch", "persistence", "metrics"] }
shared-rs = { path = "../shared-rs" }
tokio = { version = "1.37", features = ["full"] }
```

#### `backend/recon/Cargo.toml`

```toml
[dependencies]
sed-core = { path = "../sed-core", features = ["hedger", "persistence", "metrics"] }
shared-rs = { path = "../shared-rs" }
tokio = { version = "1.37", features = ["full"] }
```

---

## 6. Integración Kill-Switch y 501 Responses

### 6.1 Reglas Obligatorias del Repo (Recordatorio)

1. **Kill-switch global** siempre consultado antes de actions críticas.
2. Endpoints sin infra externa configurada responden `501 {requires:[…], sprint:"SN"}`.
3. **Honestidad de estado**: `[OK] [PARCIAL] [PENDIENTE] [BLOQUEADO]`.

### 6.2 Implementación en `gate_manager.rs`

```rust
use sed_core::types::{KillSwitchState, InfrastructurePrerequisite, NotImplementedResponse};
use shared_rs::health::HealthChecker;
use shared_rs::config::KillSwitchConfig;

/// Manager de gates que orquesta Kill-Switch + 501 + Pipeline Matemático
pub struct GateManager {
    pub kill_switch_config: KillSwitchConfig,
    pub health_checker: HealthChecker,
    pub prerequisites: Vec<InfrastructurePrerequisite>,
}

impl GateManager {
    /// Ejecuta el pipeline completo con todas las validaciones
    pub async fn execute_pipeline(
        &self,
        pipeline: StochasticPipeline,
    ) -> Result<Vec<DispatchReceipt>, PipelineError> {
        // Gate 1: Kill-Switch
        let ks_state = self.check_kill_switch().await?;
        Self::require_kill_switch_active(ks_state)?;

        // Gate 2: Infraestructura
        for prereq in &self.prerequisites {
            if let Err(resp_501) = prereq.verify(&self.health_checker).await {
                return Err(PipelineError::NotImplemented(resp_501));
            }
        }

        // Pipeline matemático
        pipeline.run().await
    }

    async fn check_kill_switch(&self) -> Result<KillSwitchState, PipelineError> {
        // Consulta shared-rs/config o endpoint HTTP
        self.kill_switch_config.get_state().await
            .map_err(|e| PipelineError::Infrastructure(e))
    }

    fn require_kill_switch_active(state: KillSwitchState) -> Result<(), PipelineError> {
        match state {
            KillSwitchState::Active => Ok(()),
            KillSwitchState::Suspended => Err(PipelineError::KillSwitchSuspended),
            KillSwitchState::Terminated => Err(PipelineError::KillSwitchTerminated),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Kill-Switch suspended")]
    KillSwitchSuspended,
    #[error("Kill-Switch terminated")]
    KillSwitchTerminated,
    #[error("Infrastructure not ready: {0:?}")]
    NotImplemented(NotImplementedResponse),
    #[error("Infrastructure failure: {0}")]
    Infrastructure(String),
    #[error("Mathematical pipeline failure: {0}")]
    Mathematical(String),
}
```

### 6.3 Tabla de Gates por Módulo

| Gate | Módulo | Servicios Requeridos | Sprint Mínimo | 501 Response |
|------|--------|---------------------|---------------|--------------|
| G1 | `stochastic_wave_filtration` | `mempool-node`, `ws-conn` | S2 | `{requires:["mempool-node"], sprint:"S2"}` |
| G2 | `eigenstate_transition_projector` | `anvil-node`, `fork-url`, `revm-cache` | S4 | `{requires:["anvil-node"], sprint:"S4"}` |
| G3 | `dirac_manifold_allocator` | `flashbots-relay`, `private-key`, `bundle-api` | S5 | `{requires:["flashbots-relay"], sprint:"S5"}` |
| G4 | `orthogonal_variance_hedger` | `multi-market-feeds`, `correlation-engine` | S3 | `{requires:["multi-market-feeds"], sprint:"S3"}` |
| G5 | `SED Dispatch` | `rpc-node`, `gas-oracle`, `nonce-manager` | S5 | `{requires:["rpc-node"], sprint:"S5"}` |

### 6.4 Degradación Elegante

```rust
/// Estrategia de degradación cuando un gate falla
pub enum DegradationStrategy {
    /// Responder 501 inmediatamente (comportamiento por defecto)
    Immediate501,
    /// Bufferizar señales matemáticas y reintentar en Δt
    BufferedRetry { max_retries: u32, backoff_ms: u64 },
    /// Operar en modo degradado (solo filtración, sin allocación)
    DegradedMode { active_modules: Vec<String> },
}

impl GateManager {
    pub async fn execute_with_degradation(
        &self,
        pipeline: StochasticPipeline,
        strategy: DegradationStrategy,
    ) -> Result<Vec<DispatchReceipt>, PipelineError> {
        match strategy {
            DegradationStrategy::Immediate501 => {
                self.execute_pipeline(pipeline).await
            }
            DegradationStrategy::BufferedRetry { max_retries, backoff_ms } => {
                for attempt in 0..max_retries {
                    match self.execute_pipeline(pipeline.clone()).await {
                        Ok(receipts) => return Ok(receipts),
                        Err(PipelineError::NotImplemented(_)) => {
                            tokio::time::sleep(Duration::from_millis(backoff_ms * 2_u64.pow(attempt))).await;
                        }
                        Err(e) => return Err(e),
                    }
                }
                Err(PipelineError::Infrastructure("Max retries exceeded".to_string()))
            }
            DegradationStrategy::DegradedMode { active_modules } => {
                // Ejecutar solo módulos que pasan sus gates
                pipeline.run_partial(active_modules).await
            }
        }
    }
}
```

---

## 7. Esquema de Persistencia Matemática

### 7.1 Migraciones SQL

#### `database/migrations/012_create_sed_filtrations.sql`

```sql
-- Migration: 012_create_sed_filtrations
-- Description: Telemetría de filtración estocástica (Markov + saltos + CDC)
-- Sprint: S2+

CREATE TABLE sed_filtrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identificación
    market_id VARCHAR(64) NOT NULL,
    token_pair VARCHAR(32) NOT NULL,  -- ej: "WETH/USDC"

    -- Parámetros del proceso de Markov
    drift_vector JSONB NOT NULL,      -- Array de floats: [μ₁, μ₂, ..., μₙ]
    diffusion_matrix JSONB NOT NULL,    -- Matriz σ como array 2D
    jump_intensity FLOAT8 NOT NULL,     -- λ: tasa de llegada Poisson
    jump_distribution JSONB NOT NULL,   -- Parámetros de la distribución de saltos

    -- Coeficiente de Divergencia de Estado
    cdc_value FLOAT8 NOT NULL,
    cdc_critical_threshold FLOAT8 NOT NULL DEFAULT 2.706,
    cdc_is_inefficiency BOOLEAN NOT NULL GENERATED ALWAYS AS (
        cdc_value > cdc_critical_threshold
    ) STORED,
    cdc_confidence FLOAT8 NOT NULL CHECK (cdc_confidence BETWEEN 0.0 AND 1.0),

    -- Estado predicho y envolvente
    predicted_state JSONB NOT NULL,     -- Vector de reservas predichas
    variance_envelope FLOAT8 NOT NULL,
    optimal_intervention_time FLOAT8,

    -- Metadatos de ejecución
    block_number BIGINT,
    block_timestamp TIMESTAMPTZ,
    execution_time_ms INTEGER,

    -- Estado de honestidad
    status VARCHAR(16) NOT NULL DEFAULT 'PENDIENTE' 
        CHECK (status IN ('OK', 'PARCIAL', 'PENDIENTE', 'BLOQUEADO')),

    -- Índices
    CONSTRAINT chk_cdc_confidence CHECK (cdc_confidence >= 0.0 AND cdc_confidence <= 1.0)
);

CREATE INDEX idx_sed_filtrations_market ON sed_filtrations(market_id, created_at DESC);
CREATE INDEX idx_sed_filtrations_inefficiency ON sed_filtrations(cdc_is_inefficiency, created_at DESC) 
    WHERE cdc_is_inefficiency = TRUE;
CREATE INDEX idx_sed_filtrations_status ON sed_filtrations(status);
```

#### `database/migrations/013_create_sed_eigenstates.sql`

```sql
-- Migration: 013_create_sed_eigenstates
-- Description: Estados propios (eigenstates) y fronteras de equilibrio
-- Sprint: S4+

CREATE TABLE sed_eigenstates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identificación
    market_id VARCHAR(64) NOT NULL,
    token_pair VARCHAR(32) NOT NULL,

    -- Parámetros del Hamiltoniano
    hamiltonian_kinetic JSONB NOT NULL,   -- Representación sparse del operador -½∇²_g
    hamiltonian_potential JSONB NOT NULL, -- V_eff como función discretizada
    metric_tensor JSONB NOT NULL,           -- Métrica Riemanniana gᵢⱼ

    -- Eigenstates resueltos
    eigenstate_index INTEGER NOT NULL,      -- n: índice del eigenstate
    eigenstate_energy FLOAT8 NOT NULL,      -- Eₙ
    eigenstate_amplitude JSONB NOT NULL,    -- Vector complejo |ψₙ⟩
    eigenstate_degeneracy INTEGER NOT NULL DEFAULT 1,
    quantum_numbers JSONB,                  -- Array de números cuánticos

    -- Estado de equilibrio objetivo
    is_equilibrium_state BOOLEAN NOT NULL DEFAULT FALSE,
    equilibrium_probability FLOAT8,         -- P_eq = |c_eq|²

    -- Frontera de equilibrio
    boundary_hypersurface JSONB,            -- Array de puntos que definen ∂Ω_eq
    critical_probability FLOAT8,
    convergence_radius FLOAT8,
    stability_exponent FLOAT8,              -- Exponente de Lyapunov

    -- Metadatos
    block_number BIGINT,
    simulation_duration_ms INTEGER,
    lanczos_iterations INTEGER,

    status VARCHAR(16) NOT NULL DEFAULT 'PENDIENTE'
        CHECK (status IN ('OK', 'PARCIAL', 'PENDIENTE', 'BLOQUEADO'))
);

CREATE INDEX idx_sed_eigenstates_market ON sed_eigenstates(market_id, eigenstate_index);
CREATE INDEX idx_sed_eigenstates_equilibrium ON sed_eigenstates(is_equilibrium_state, created_at DESC) 
    WHERE is_equilibrium_state = TRUE;
```

#### `database/migrations/014_create_sed_allocations.sql`

```sql
-- Migration: 014_create_sed_allocations
-- Description: Asignaciones de control óptimo e impulsos de Dirac
-- Sprint: S5+

CREATE TABLE sed_allocations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identificación
    market_id VARCHAR(64) NOT NULL,
    token_pair VARCHAR(32) NOT NULL,

    -- Variedad de liquidez
    constant_product NUMERIC(78, 0) NOT NULL,  -- k = x·y (uint256 como numeric)
    token0_reserve NUMERIC(78, 0) NOT NULL,
    token1_reserve NUMERIC(78, 0) NOT NULL,
    metric_tensor JSONB NOT NULL,

    -- Control óptimo
    optimal_control JSONB NOT NULL,         -- Vector de control u*
    value_functional FLOAT8 NOT NULL,         -- J[u*]
    costate_trajectory JSONB NOT NULL,      -- Trayectoria de costados p(t)
    time_horizon FLOAT8 NOT NULL,

    -- Impulso de Dirac
    injection_point JSONB NOT NULL,         -- [x₀, y₀]
    dirac_amplitude FLOAT8 NOT NULL,
    support_radius FLOAT8 NOT NULL,

    -- Restricciones
    hyperbolic_constraint_satisfied BOOLEAN NOT NULL,
    hyperbolic_tolerance FLOAT8 NOT NULL DEFAULT 1e-9,
    hyperbolic_violation NUMERIC(78, 18),  -- |x·y - k| medido

    -- Ventana de ejecución
    execution_window_start TIMESTAMPTZ,
    execution_window_end TIMESTAMPTZ,

    -- Relación con eigenstate
    eigenstate_id UUID REFERENCES sed_eigenstates(id),

    -- Metadatos
    block_number BIGINT,
    solver_iterations INTEGER,
    solver_convergence_error FLOAT8,

    status VARCHAR(16) NOT NULL DEFAULT 'PENDIENTE'
        CHECK (status IN ('OK', 'PARCIAL', 'PENDIENTE', 'BLOQUEADO'))
);

CREATE INDEX idx_sed_allocations_market ON sed_allocations(market_id, created_at DESC);
CREATE INDEX idx_sed_allocations_hyperbolic ON sed_allocations(hyperbolic_constraint_satisfied) 
    WHERE hyperbolic_constraint_satisfied = FALSE;
```

#### `database/migrations/015_create_sed_hedges.sql`

```sql
-- Migration: 015_create_sed_hedges
-- Description: Resultados de hedging ortogonal y estados entrelazados
-- Sprint: S3+

CREATE TABLE sed_hedges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identificación de mercados entrelazados
    market_a_id VARCHAR(64) NOT NULL,
    market_b_id VARCHAR(64) NOT NULL,
    token_pair_a VARCHAR(32) NOT NULL,
    token_pair_b VARCHAR(32) NOT NULL,

    -- Estado entrelazado
    entangled_amplitude JSONB NOT NULL,     -- Vector |Ψ_AB⟩
    entanglement_entropy FLOAT8 NOT NULL,   -- S(ρ_A) = -Tr(ρ_A log ρ_A)
    market_a_dimension INTEGER NOT NULL,
    market_b_dimension INTEGER NOT NULL,

    -- Hiperplano ortogonal
    normal_vector JSONB NOT NULL,           -- Normal n a Π_⟂
    basis_vectors JSONB NOT NULL,           -- Base ortonormal de Π_⟂
    null_covariance_constraint JSONB NOT NULL, -- Matriz de restricción

    -- Vector direccional y proyección
    original_direction JSONB NOT NULL,      -- v original
    projected_direction JSONB NOT NULL,     -- v_⟂
    residual_direction JSONB NOT NULL,      -- v - v_⟂

    -- Resultado de neutralización
    is_fully_neutralized BOOLEAN NOT NULL,
    orthogonality_error FLOAT8 NOT NULL,
    covariance_matrix JSONB NOT NULL,       -- Matriz 2x2 de covarianza
    covariance_off_diagonal FLOAT8 NOT NULL, -- Cov(A,B) para índice rápido

    -- Validación
    null_covariance_verified BOOLEAN NOT NULL GENERATED ALWAYS AS (
        ABS(covariance_off_diagonal) < 1e-9 AND orthogonality_error < 1e-9
    ) STORED,

    -- Relaciones
    allocation_a_id UUID REFERENCES sed_allocations(id),
    allocation_b_id UUID REFERENCES sed_allocations(id),

    -- Metadatos
    block_number BIGINT,
    sync_duration_ms INTEGER,
    decoherence_rate FLOAT8,
    correlation_threshold FLOAT8,

    status VARCHAR(16) NOT NULL DEFAULT 'PENDIENTE'
        CHECK (status IN ('OK', 'PARCIAL', 'PENDIENTE', 'BLOQUEADO'))
);

CREATE INDEX idx_sed_hedges_markets ON sed_hedges(market_a_id, market_b_id, created_at DESC);
CREATE INDEX idx_sed_hedges_neutralized ON sed_hedges(is_fully_neutralized, created_at DESC) 
    WHERE is_fully_neutralized = TRUE;
CREATE INDEX idx_sed_hedges_verified ON sed_hedges(null_covariance_verified) 
    WHERE null_covariance_verified = TRUE;
```

### 7.2 DAOs Rust (sqlx)

```rust
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub struct SedFiltrationDao;

impl SedFiltrationDao {
    pub async fn insert(
        pool: &PgPool,
        market_id: &str,
        token_pair: &str,
        cdc_value: f64,
        confidence: f64,
        predicted_state: &serde_json::Value,
    ) -> Result<Uuid, sqlx::Error> {
        sqlx::query_scalar!(
            r#"
            INSERT INTO sed_filtrations (market_id, token_pair, cdc_value, cdc_confidence, predicted_state, drift_vector, diffusion_matrix, jump_intensity, jump_distribution, variance_envelope)
            VALUES ($1, $2, $3, $4, $5, '[]'::jsonb, '[]'::jsonb, 0.0, '{}'::jsonb, 0.0)
            RETURNING id
            "#,
            market_id, token_pair, cdc_value, confidence, predicted_state
        )
        .fetch_one(pool)
        .await
    }

    pub async fn get_recent_inefficiencies(
        pool: &PgPool,
        market_id: &str,
        limit: i64,
    ) -> Result<Vec<SedFiltrationRow>, sqlx::Error> {
        sqlx::query_as!(
            SedFiltrationRow,
            r#"
            SELECT * FROM sed_filtrations 
            WHERE market_id = $1 AND cdc_is_inefficiency = TRUE 
            ORDER BY created_at DESC LIMIT $2
            "#,
            market_id, limit
        )
        .fetch_all(pool)
        .await
    }
}
```

---

## 8. Roadmap Mapping: SED ↔ Sprints Operativos

### 8.1 Matriz de Activación

| Módulo SED | Sprint Repo | Servicios Requeridos | Credenciales | Estado Objetivo |
|------------|-------------|---------------------|--------------|-----------------|
| `stochastic_wave_filtration` | **S2: Detection** | mempool-node (WebSocket/HTTP), block-stream | RPC endpoint (Alchemy/Infura), WS subscription | `[PARCIAL]` — Filtración básica con CDC. Sin salto real hasta S3. |
| `eigenstate_transition_projector` | **S4: Simulation** | anvil-node, fork-url, revm-cache | Anvil binary, fork RPC (mainnet), state cache dir | `[PENDIENTE]` — En S2-S3: stub 501. En S4: resolución Lanczos sobre fork real. |
| `dirac_manifold_allocator` | **S5: Execution** | flashbots-relay, bundle-api, private-key | Flashbots auth key, builder endpoints, signer | `[PENDIENTE]` — En S1-S4: stub 501. En S5: control óptimo + envío a relay. |
| `orthogonal_variance_hedger` | **S3: Selector+Risk** | multi-market-feeds, correlation-engine | Múltiples RPC endpoints (uno por mercado), price oracle | `[PARCIAL]` — En S3: scoring de riesgo ortogonal. En S6: validación post-ejecución. |

### 8.2 Diagrama de Activación Temporal

```
S1 Foundations (Actual)
├── sed-core/ crate creada con esqueletos y tests unitarios
├── Todos los módulos: 501 {requires:[...], sprint:"SX"}
└── Estado: [PENDIENTE] para todos los módulos SED

S2 Detection
├── searcher-rs/ consume sed-core::filtration
├── stochastic_wave_filtration: [PARCIAL]
│   └── CDC calculado sobre mempool real
│   └── Sin saltos modelados (difusión pura)
├── eigenstate_transition_projector: [BLOQUEADO] → 501 {requires:["anvil"], sprint:"S4"}
├── dirac_manifold_allocator: [BLOQUEADO] → 501 {requires:["flashbots"], sprint:"S5"}
└── orthogonal_variance_hedger: [BLOQUEADO] → 501 {requires:["multi-market"], sprint:"S3"}

S3 Selector + Risk
├── selector-api/ (TS) consume sed-core::hedger vía HTTP/gRPC
├── orthogonal_variance_hedger: [PARCIAL]
│   └── Covarianza nula como factor de scoring multi-factor
│   └── Sin ejecución real (solo scoring)
├── stochastic_wave_filtration: [OK]
├── eigenstate_transition_projector: [BLOQUEADO] → 501
└── dirac_manifold_allocator: [BLOQUEADO] → 501

S4 Simulation
├── sim-ctl/ consume sed-core::eigenstate + sed-core::allocator
├── eigenstate_transition_projector: [PARCIAL]
│   └── Eigenstates sobre Anvil/fork real
│   └── Sin control óptimo aún
├── dirac_manifold_allocator: [PARCIAL]
│   └── OCP resuelto, impulso Dirac construido
│   └── Sin envío a relay (simulación local)
├── stochastic_wave_filtration: [OK]
└── orthogonal_variance_hedger: [PARCIAL]

S5 Execution
├── relays-client/ consume sed-core::allocator + sed-core::hedger
├── dirac_manifold_allocator: [OK]
│   └── Bundles optimizados enviados a Flashbots
├── orthogonal_variance_hedger: [OK]
│   └── Validación post-ejecución de covarianza nula
├── eigenstate_transition_projector: [OK]
└── stochastic_wave_filtration: [OK]

S6 Recon + Learn
├── recon/ consume sed-core::hedger para validación histórica
├── Todos los módulos SED: [OK]
├── Feedback loop: CDC histórico → ajuste de parámetros Hamiltoniano
└── Métricas de convergencia asintótica publicadas a Prometheus

S7 Edge + Frontend
├── Edge expone métricas SED (solo lectura)
├── Frontend dashboard de convergencia estocástica
└── Sin cambios en lógica SED

S8 Obs + E2E + Gov
├── E2E tests del pipeline SED completo
├── Auditoría de tipado terminal (BundlePosition)
└── Governance: aprobación para capital real
```

### 8.3 Checklist de Activación por Sprint

#### S2: Detection
- [ ] `sed-core/` compilable en workspace
- [ ] `stochastic_wave_filtration` consume mempool real vía WebSocket
- [ ] CDC calculado y persistido en `sed_filtrations`
- [ ] Tests de integración: filtración + DB
- [ ] Métricas Prometheus: `sed_cdc_value`, `sed_filtration_rate`

#### S3: Selector + Risk
- [ ] `orthogonal_variance_hedger` expone scoring vía HTTP/gRPC
- [ ] `selector-api` consume `OrthogonalHedgeResult` como factor de riesgo
- [ ] Covarianza nula calculada entre 2+ mercados
- [ ] Tests: hedging con datos históricos

#### S4: Simulation
- [ ] `eigenstate_transition_projector` resuelve eigenstates sobre Anvil/fork
- [ ] `dirac_manifold_allocator` resuelve OCP en simulación local
- [ ] Restricción hiperbólica verificada en cada paso de simulación
- [ ] Tests E2E: filtración → eigenstate → allocación (sin relay)

#### S5: Execution
- [ ] `dirac_manifold_allocator` envía bundles a Flashbots relay
- [ ] Kill-switch consultado antes de cada envío
- [ ] `orthogonal_variance_hedger` valida post-ejecución
- [ ] `BundlePosition<DiracImpulseOnly>` usado en producción
- [ ] Tests con capital de prueba (testnet)

#### S6: Recon + Learn
- [ ] Feedback loop automatizado: ajuste de parámetros Hamiltoniano
- [ ] Métricas de convergencia asintótica en dashboard
- [ ] Auditoría completa de pipeline SED
- [ ] Documentación de gobernanza para capital real

---

## 9. Métricas de Convergencia y Monitoreo

### 9.1 Métricas Prometheus

```rust
use metrics::{counter, gauge, histogram};

pub struct SedMetrics;

impl SedMetrics {
    pub fn record_cdc(value: f64, market: &str) {
        gauge!("sed_cdc_value", value, "market" => market.to_string());
    }

    pub fn record_filtration_duration_ms(duration: u64) {
        histogram!("sed_filtration_duration_ms", duration as f64);
    }

    pub fn record_eigenstate_energy(energy: f64, index: u32) {
        gauge!("sed_eigenstate_energy", energy, "index" => index.to_string());
    }

    pub fn record_dirac_amplitude(amplitude: f64, market: &str) {
        gauge!("sed_dirac_amplitude", amplitude, "market" => market.to_string());
    }

    pub fn record_hedge_covariance(cov: f64, market_a: &str, market_b: &str) {
        gauge!("sed_hedge_covariance", cov, 
            "market_a" => market_a.to_string(), 
            "market_b" => market_b.to_string()
        );
    }

    pub fn record_convergence_rate(rate: f64) {
        gauge!("sed_convergence_rate", rate);
    }

    pub fn record_kill_switch_check(state: &str) {
        counter!("sed_kill_switch_checks_total", "state" => state.to_string());
    }

    pub fn record_501_response(module: &str, sprint: &str) {
        counter!("sed_501_responses_total", 
            "module" => module.to_string(),
            "sprint" => sprint.to_string()
        );
    }
}
```

### 9.2 Dashboard Grafana (JSON Model)

```json
{
  "dashboard": {
    "title": "SED — Sequential Equilibrium Dispatcher",
    "panels": [
      {
        "title": "CDC en Tiempo Real",
        "type": "graph",
        "targets": [
          {
            "expr": "sed_cdc_value",
            "legendFormat": "{{market}}"
          }
        ],
        "alert": {
          "name": "CDC Critical",
          "condition": "sed_cdc_value > 2.706",
          "for": "30s"
        }
      },
      {
        "title": "Entropía de Entrelazamiento",
        "type": "graph",
        "targets": [
          {
            "expr": "sed_eigenstate_energy",
            "legendFormat": "Eigenstate {{index}}"
          }
        ]
      },
      {
        "title": "Tasa de Convergencia Asintótica",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(sed_convergence_rate[5m])",
            "legendFormat": "Convergence"
          }
        ]
      },
      {
        "title": "501 Responses por Módulo",
        "type": "table",
        "targets": [
          {
            "expr": "sum by (module, sprint) (sed_501_responses_total)",
            "format": "table"
          }
        ]
      }
    ]
  }
}
```

---

## 10. Invariantes de Runtime

1. **Invariante de Conservación del Producto:** ∀ `DiracImpulse` activo, x·y = k ± ε.
2. **Invariante de Covarianza Nula:** ∀ `OrthogonalEquilibrium` activo, |Cov| < 1e-9.
3. **Invariante de No-Reentrancia:** 1 `BundlePosition` por ventana temporal δt.
4. **Invariante de Convergencia:** Var(S_{t+1}) ≤ Var(S_t).
5. **Invariante de Kill-Switch:** Ningún dispatch sin `KillSwitchState::Active` verificado.
6. **Invariante de 501:** Ningún módulo opera sin prerequisitos healthy verificados.
7. **Invariante de Persistencia:** Toda señal matemática con CDC > 0 se persiste en `sed_filtrations`.

---

## 11. Referencias Bibliográficas

1. Øksendal, B. (2003). *Stochastic Differential Equations*. Springer.
2. Pontryagin, L.S. et al. (1962). *The Mathematical Theory of Optimal Processes*.
3. Nielsen, M.A. & Chuang, I.L. (2010). *Quantum Computation and Quantum Information*. Cambridge.
4. Lee, J.M. (2012). *Introduction to Smooth Manifolds*. Springer.
5. Bertsekas, D.P. (2005). *Dynamic Programming and Optimal Control*. Athena Scientific.
6. Uniswap V2 Whitepaper (2020).
7. Rust Documentation (2024). *The Rust Programming Language*.

---

## 12. Checklist de Implementación v1.1

### Fase 1: Fundamentos
- [ ] Crear `backend/sed-core/` con `Cargo.toml` y features
- [ ] Implementar `NaturalFiltration` con álgebra de σ-campos
- [ ] Implementar `PoissonRandomMeasure` con compensador
- [ ] Implementar cálculo de CDC con Girsanov
- [ ] Implementar `EffectiveHamiltonian` con discretización
- [ ] Implementar resolución Lanczos de eigenstates
- [ ] Implementar `EquilibriumBoundary` con distancia geodésica

### Fase 2: Control Óptimo
- [ ] Implementar `LiquidityManifold` con métrica hiperbólica
- [ ] Implementar `DiracImpulse` con regularización gaussiana
- [ ] Implementar resolución OCP (colocación pseudoespectral)
- [ ] Implementar verificación de restricción hiperbólica

### Fase 3: Entrelazamiento
- [ ] Implementar `EntangledState` con matriz de densidad reducida
- [ ] Implementar cálculo de entropía de von Neumann
- [ ] Implementar `OrthogonalHyperplane` con Gram-Schmidt
- [ ] Implementar proyección ortogonal
- [ ] Implementar verificación de covarianza nula

### Fase 4: Tipado y Control
- [ ] Implementar `BundlePosition<T>` con `PhantomData` y `Sealed`
- [ ] Implementar `KillSwitchGate` con consulta a shared-rs
- [ ] Implementar `InfrastructurePrerequisite` con health checks
- [ ] Implementar `GateManager` con degradación elegante
- [ ] Implementar `StochasticPipeline` con backpressure

### Fase 5: Persistencia
- [ ] Ejecutar migraciones 012-015 en PostgreSQL
- [ ] Implementar DAOs con sqlx
- [ ] Tests de integración DB + pipeline

### Fase 6: Métricas y Roadmap
- [ ] Integrar métricas Prometheus
- [ ] Crear dashboard Grafana SED
- [ ] Mapear módulos a sprints S2-S6
- [ ] Documentar 501 responses por módulo

### Fase 7: Validación
- [ ] Tests unitarios con casos límite matemáticos
- [ ] Tests de integración del pipeline completo
- [ ] Benchmarks de latencia < 10ms por batch
- [ ] Simulación Monte Carlo de convergencia
- [ ] Auditoría de seguridad del sistema de tipos
- [ ] E2E tests con kill-switch y 501

---

**Documento generado bajo el protocolo OMEGA AGENT TEAMS v1.1.**  
**Doctrina:** Autonomía Total Controlada y Ejecución Enterprise (Zero-Prompt).  
**Clasificación:** QUANTITATIVE ARCHITECTURE — TOP SECRET  
**Estado:** APROBADO PARA IMPLEMENTACIÓN.
