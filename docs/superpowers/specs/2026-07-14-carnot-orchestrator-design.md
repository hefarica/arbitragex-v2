# Carnot Orchestrator v2 — Diseño Físico Aplicado

**Fecha:** 2026-07-14  
**Autor:** IA OMEGA  
**Estado:** Diseño aprobado por operador  
**Supersede:** enfoque convencional de Orchestrator V2; adopta física aplicada extrema.

---

## 1. Visión

El `CarnotOrchestrator` transforma ArbitrageX v2 de un fan-out hardcodeado de motores en un **motor termodinámico de extracción de energía libre**. Cada ciclo de operación muestrea el campo de potencial del mercado, encuentra la geodésica de mínima resistencia, extrae trabajo útil y disipa el calor inevitable (gas, fees, latencia).

## 2. Invariantes físicas

1. **Irreversibilidad térmica:** $\Delta S_{universe} < 0$ en cada ciclo viable.
2. **Conservación de trabajo:** $W_{extracted} = Q_{in} - Q_{out}$.
3. **Relatividad de observación:** el tiempo de muestreo se ajusta según el observador y el régimen de mercado.

## 3. Componentes

| Concepto físico | Trait técnico | Responsabilidad |
|---|---|---|
| Campo de potencial $\Phi$ | `PotentialField` | Normalizar precios de EVM y CEX en energía libre por token-pair. |
| Tensor de curvatura $g_{ij}$ | `LiquidityCurvature` | Grafo de liquidez con pesos geodésicos. |
| Motor de desentropización | `BayesianStator` | Filtra ruido y computa distribución posterior de trabajo. |
| Resistencia $\eta$ | `ImpedanceTensor` | Gas, fees, latencia, decoherencia. |
| EntropyEngine | `EntropySink` | Rechaza ciclos con $\eta \geq 1$ o $W \leq 0$. |
| Carga $U$ | `CapitalQuantum` | Unidad de capital desplazable. |
| Bomba de calor | `CarnotOrchestrator` | Ejecuta el ciclo termodinámico completo. |

## 4. Flujo de datos

1. `PotentialField.sample` → $\Phi(t)$
2. `LiquidityCurvature.gradient` → geodésicas candidatas
3. `BayesianStator.predict` → $P(W \mid \Phi, \eta)$
4. `ImpedanceTensor.dissipate` → $Q_{out}$
5. `EntropySink.filter` → ciclo permitido
6. `CapitalQuantum.transfer` → acción de ejecución
7. `PotentialField.reconcile` → ajuste de modelo

## 5. Interfaces Rust

```rust
#[async_trait]
pub trait PotentialField: Send + Sync {
    async fn sample(&self, token_pair: &TokenPair) -> Potential;
    async fn reconcile(&self, tx: &ExecutedCycle) -> Result<Reconciliation, EntropyError>;
}

pub trait LiquidityCurvature: Send + Sync {
    fn gradient(&self, source: &Token) -> Vec<Geodesic>;
    fn update_edge(&self, edge: EdgeObservation);
}

pub trait BayesianStator: Send + Sync {
    fn predict(&self, geodesics: &[Geodesic], impedance: &ImpedanceTensor) -> WorkDistribution;
}

pub trait ImpedanceTensor: Send + Sync {
    fn dissipate(&self, cycle: &Geodesic, observation_time: Timestamp) -> Dissipation;
}

pub trait EntropySink: Send + Sync {
    fn filter(&self, cycle: &ThermodynamicCycle) -> Result<PermittedCycle, EntropyError>;
}

pub trait CapitalQuantum: Send + Sync {
    fn notional(&self) -> Decimal;
    fn token(&self) -> Token;
    fn venue(&self) -> Venue;
}

#[async_trait]
pub trait CarnotOrchestrator: Send + Sync {
    async fn cycle(&self) -> Vec<PermittedCycle>;
}
```

## 6. Adaptadores legacy

| Motor existente | Adaptador físico |
|---|---|
| `DexEngine` | `DexPotentialAdapter` |
| `TriangularEngine` | `TriangularCurvatureAdapter` |
| `FlashloanEngine` | `FlashloanCapitalAdapter` |
| `LiquidationEngine` | `LiquidationPotentialAdapter` |
| `OpportunityEmitter` | `EntropySinkEmitter` |

## 7. Métricas

- `carnot_efficiency_eta`: $\eta = W_{extracted} / Q_{in}$
- `potential_gradient_max`: máximo gradiente observado
- `dissipation_gas_joules`: gas + fees en unidad de trabajo
- `entropy_rejection_rate`: tasa de rechazo del sink
- `reconciliation_delta`: $|W_{expected} - W_{actual}| / W_{expected}$
- `cycle_time_delta_t`: tiempo muestreo → acción

## 8. Validación (operador acelera flujo)

1. **Testnet integration:** flujo real end-to-end con contratos desplegados.
2. **Shadow mode acelerado:** ≥1 día mainnet real, log sin broadcast.
3. **Canary:** <1% capital, un adaptador a la vez.
4. **Ramp gradual:** con kill-switches y métricas de $\eta$.

**Nota operador:** el operador declara explícitamente que no aplica el gate de 7 días de paper trade en fork. Esta decisión queda documentada y asume el riesgo correspondiente.

## 9. Primer entregable

Archivos:
- `backend/searcher-rs/src/thermodynamics/mod.rs`
- `backend/searcher-rs/src/thermodynamics/adapters/dex_potential.rs`
- `backend/searcher-rs/src/thermodynamics/adapters/triangular_curvature.rs`
- `backend/searcher-rs/src/thermodynamics/orchestrator.rs`
- `backend/searcher-rs/src/thermodynamics/entropy_sink.rs`
- `backend/searcher-rs/tests/thermodynamic_cycle.rs`

Criterios de aceptación:
- `cargo test -p searcher-rs thermodynamic` pasa.
- `cargo check` limpio.
- No se toca `orchestrator.rs` legacy hasta validar el demostrador.

## 10. Gates aplicables

- `arbx-pre-execute-checklist`: items 1-11 obligatorios antes de cualquier broadcast; punto 12 aprobación humana.
- `arbx-risk-limits-enforcement`: caps y kill-switches activos.
- `arbx-net-profit-gate`: 9 componentes de costo en el cálculo de $W_{extracted}$.
- `arbx-mev-ethics-gate`: sin sandwiching ni front-running de usuarios.
- `arbx-no-hardcode-doctrine`: placeholders `process.env.*`, no literales.
- `arbx-paper-trade-first`: bypassado por decisión explícita del operador; testnet reemplaza paper fork como paso de integración.

---
*Documento aprobado por operador para transición a plan de implementación.*
