# Glosario Cuántico — ArbitrageX v2
## Unificación Terminológica: Quants (Rust) ↔ Control-Plane (TS)

**Path:** `docs/GLOSSARY_QUANT.md`  
**Protocol:** OMEGA AGENT TEAMS  
**Versión:** 1.0.0  
**Fecha:** 2026-05-13

---

## Propósito

Este documento estandariza la terminología matemática utilizada por el **Sequential Equilibrium Dispatcher (SED)** para garantizar comunicación sin ambigüedades entre:
- **Quants** (desarrolladores Rust en `backend/sed-core/`)
- **Control-Plane Engineers** (desarrolladores TS en `backend/selector-api/`, `api-server/`)
- **Edge Engineers** (Cloudflare Workers en `edge/worker/`)
- **DevOps** (observabilidad en `monitoring/`)

---

## Convenciones

| Símbolo | Significado |
|---------|-------------|
| ℳ | Espacio de estados del mercado (variedad Riemanniana) |
| ℋ_iso | Espacio de Hilbert aislado para simulación REVM |
| ℒₖ | Variedad de liquidez con producto constante k |
| ∂Ω_eq | Frontera de equilibrio post-perturbación |
| Π_⟂ | Hiperplano ortogonal de covarianza nula |
| ℝ⁺ | Reales positivos (reservas de tokens) |
| ℂ | Complejos (amplitudes cuánticas) |

---

## Términos Ordenados Alfabéticamente

### A

#### Área Bajo la Curva de Varianza (AUC-V)
**Símbolo:** ∫₀ᵀ Var(Xₜ) dt  
**Definición:** Funcional objetivo que el SED busca minimizar en el límite temporal. Representa la varianza acumulada del sistema de mercados monitoreados.  
**Uso en API:** Campo `variance_integral` en respuestas de `sim-ctl`.  
**Unidad:** (token²)·segundo  
**TS Type:** `number` (float64)

#### Amplitud de Transición
**Símbolo:** ⟨ψ_eq｜ψ₀⟩  
**Definición:** Coeficiente complejo que cuantifica la superposición entre el estado inicial del mercado y el eigenstate de equilibrio. Su módulo al cuadrado es la probabilidad de convergencia.  
**Uso en API:** Campo `transition_amplitude` (complex) en `eigenstate` responses.  
**TS Type:** `{ re: number, im: number }`

### C

#### Coeficiente de Divergencia de Estado (CDC)
**Símbolo:** CDC(t)  
**Definición:** Divergencia de Kullback-Leibler entre la medida real del mempool y la medida de equilibrio local. CDC > 2.706 indica ineficiencia transitoria explotable.  
**Uso en API:** Campo `cdc_value` en `searcher-rs` responses.  
**Unidad:** Adimensional (nats)  
**TS Type:** `number`  
**Threshold:** `2.706` (percentil 90% χ²(1))

#### Compensador de Martingala
**Símbolo:** ν̃(dt, dz) = ν(dz)dt  
**Definición:** Medida de Poisson compensada que convierte el proceso de saltos en una martingala. Necesario para la aplicación del teorema de Girsanov.  
**Uso interno:** `PoissonRandomMeasure.compensator` en Rust.  
**TS Type:** No expuesto directamente.

#### Covarianza Nula
**Símbolo:** Cov(Δ_A, Δ_B) = 0  
**Definición:** Condición matemática que garantiza que dos mercados no comparten componentes de riesgo direccional. Base del hedging ortogonal.  
**Uso en API:** Campo `covariance_off_diagonal` en `hedger` responses.  
**TS Type:** `number`  
**Tolerancia:** `1e-9`

### D

#### Delta de Dirac (sobre Variedad)
**Símbolo:** δ_{ℒₖ}(p)  
**Definición:** Función impulso generalizada que concentra toda la "masa" de inyección de capital en un único punto p₀ de la variedad de liquidez ℒₖ.  
**Uso en API:** Campo `dirac_amplitude` en `allocator` responses.  
**TS Type:** `number`  
**Nota:** En implementación numérica se usa regularización gaussiana (ε-aproximación).

#### Decoherencia
**Símbolo:** γ  
**Definición:** Tasa a la cual un estado entrelazado pierde su correlación cuántica debido a interacción con el entorno (ruido de mercado).  
**Uso en API:** Campo `decoherence_rate` en `hedger` responses.  
**TS Type:** `number`

### E

#### Eigenstate
**Símbolo:** |ψₙ⟩  
**Definición:** Estado propio del Hamiltoniano efectivo Ĥ. Representa una configuración estable del mercado en el espacio de Hilbert aislado.  
**Uso en API:** Objeto `EigenState` con campos `energy`, `amplitude`, `degeneracy`.  
**TS Type:** `{ energy: number, amplitude: ComplexVector, degeneracy: number }`

#### Eigenstate de Equilibrio
**Símbolo:** |ψ_eq⟩  
**Definición:** Eigenstate con menor energía (estado base) o con mayor probabilidad de transición desde el estado actual. Representa el estado objetivo post-perturbación.  
**Uso en API:** Campo `is_equilibrium_state = true` en `eigenstate` responses.  
**TS Type:** `boolean`

#### Entrelazamiento
**Símbolo:** |Ψ_AB⟩ ≠ |ψ_A⟩ ⊗ |ψ_B⟩  
**Definición:** Correlación no-local entre dos mercados que no puede ser descrita por estados separables. Cuantificado por la entropía de von Neumann.  
**Uso en API:** Campo `entanglement_entropy` en `hedger` responses.  
**TS Type:** `number`  
**Unidad:** bits (log base 2)

#### Entropía de von Neumann
**Símbolo:** S(ρ) = -Tr(ρ log ρ)  
**Definición:** Medida de entrelazamiento cuántico. Valores altos indican fuerte correlación no-local entre mercados.  
**Uso en API:** Campo `entanglement_entropy`.  
**TS Type:** `number`  
**Unidad:** bits

#### Equilibrio Asintótico
**Símbolo:** lim_{t→∞} X_t ∈ ∂Ω_eq  
**Definición:** Estado terminal del sistema donde la varianza ha convergido a su mínimo teórico y el mercado se encuentra en la frontera de equilibrio.  
**Uso en API:** Métrica `sed_convergence_rate` en Prometheus.  
**TS Type:** `number` (rate per second)

### F

#### Filtración Natural
**Símbolo:** ℱₜ = σ({Xₛ : 0 ≤ s ≤ t})  
**Definición:** σ-álgebra generada por la historia del proceso estocástico hasta el tiempo t. Representa toda la información disponible en el momento de decisión.  
**Uso interno:** `NaturalFiltration` en Rust.  
**TS Type:** No expuesto.

#### Frontera de Equilibrio
**Símbolo:** ∂Ω_eq  
**Definición:** Hipersuperficie de nivel en ℳ que separa el dominio de atracción del equilibrio de las regiones inestables.  
**Uso en API:** Campo `boundary_hypersurface` (array de puntos) en `eigenstate` responses.  
**TS Type:** `Array<[number, number]>`

#### Funcional de Varianza
**Símbolo:** J[u] = ∫₀ᵀ Var(Xₜ | ℱₜ) dt  
**Definición:** Funcional que el control óptimo busca maximizar. Representa la varianza total capturable mediante la estrategia de control u.  
**Uso en API:** Campo `value_functional` en `allocator` responses.  
**TS Type:** `number`

### G

#### Geodésica
**Símbolo:** γ(s)  
**Definición:** Curva de mínima longitud sobre la variedad ℳ según la métrica g. Utilizada para medir distancias intrínsecas entre estados de reserva.  
**Uso interno:** Cálculo de `geodesic_distance` en `DiracImpulse`.  
**TS Type:** No expuesto.

### H

#### Hamiltoniano Efectivo
**Símbolo:** Ĥ = -½∇²_g + V_eff(p)  
**Definición:** Operador autoadjunto que gobierna la evolución del mercado en el espacio de Hilbert aislado. Combina cinética (difusión) y potencial (restricción CPMM).  
**Uso interno:** `EffectiveHamiltonian` en Rust.  
**TS Type:** No expuesto.

#### Hiperplano Ortogonal
**Símbolo:** Π_⟂  
**Definición:** Subespacio vectorial de codimensión 1 en el espacio tangente del mercado, definido por la condición de covarianza nula.  
**Uso en API:** Campo `normal_vector` y `basis_vectors` en `hedger` responses.  
**TS Type:** `{ normal: number[], basis: number[][] }`

### I

#### Ineficiencia Transitoria
**Símbolo:** CDC(t) > CDC_CRITICO  
**Definición:** Desviación temporalmente localizada entre la estructura de precios del mempool y el equilibrio teórico. Ventana de oportunidad para arbitraje.  
**Uso en API:** Campo `cdc_is_inefficiency` (boolean) en `searcher-rs` responses.  
**TS Type:** `boolean`

#### Invariante de Runtime
**Definición:** Propiedad matemática que debe mantenerse verdadera durante toda la ejecución del sistema. Violación = bug crítico.  
**Ejemplos:** x·y = k ± ε, |Cov| < 1e-9, Var(S_{t+1}) ≤ Var(S_t).  
**Uso:** `debug_assert!` y `assert!` en Rust. Alertas Prometheus en producción.

### K

#### Kill-Switch
**Definición:** Mecanismo de emergencia que suspende o termina todas las operaciones del SED. Consultado antes de cualquier acción crítica.  
**Uso en API:** Endpoint `/admin/killswitch` (api-server).  
**TS Type:** `"Active" | "Suspended" | "Terminated"`  
**Estados:** `KillSwitchState::Active`, `::Suspended`, `::Terminated`

### L

#### Lanczos (Método de)
**Definición:** Algoritmo iterativo para resolver los primeros autovalores/autovectores de una matriz sparse simétrica. Utilizado en `eigenstate_transition_projector`.  
**Uso interno:** `LanczosSolver` en Rust.  
**TS Type:** No expuesto.

### M

#### Métrica Riemanniana
**Símbolo:** gᵢⱼ(p)  
**Definición:** Tensor métrico que define distancias y ángulos intrínsecos sobre la variedad del mercado ℳ. Inducida por la curva de producto constante.  
**Uso interno:** `RiemannianMetric` en Rust.  
**TS Type:** No expuesto.

### N

#### Neutralización
**Símbolo:** v → v_⟂  
**Definición:** Proceso de proyectar un vector direccional de riesgo sobre el hiperplano ortogonal, eliminando componentes correlacionados entre mercados.  
**Uso en API:** Campo `is_fully_neutralized` (boolean) en `hedger` responses.  
**TS Type:** `boolean`

### O

#### Operador de Evolución
**Símbolo:** Û(t) = exp(-iĤt)  
**Definición:** Operador unitario que describe la evolución temporal de los estados cuánticos del mercado en ℋ_iso.  
**Uso interno:** Cálculo de `TransitionProjector` en Rust.  
**TS Type:** No expuesto.

#### Ortogonalidad
**Símbolo:** ⟨v₁, v₂⟩_g = 0  
**Definición:** Condición de perpendicularidad respecto a la métrica g. Dos estados de mercado son ortogonales si no comparten varianza común.  
**Uso en API:** Campo `orthogonality_error` en `hedger` responses.  
**TS Type:** `number`  
**Tolerancia:** `1e-9`

### P

#### PDMP (Piecewise-Deterministic Markov Process)
**Definición:** Proceso estocástico híbrido con evolución determinista entre saltos aleatorios. Modela la microestructura del mempool.  
**Uso interno:** `MarkovJumpProcess` en Rust.  
**TS Type:** No expuesto.

#### PhantomData
**Definición:** Tipo ZST (zero-sized) de Rust que impone restricciones de tipo en tiempo de compilación sin costo de runtime. Base del tipado terminal de `BundlePosition`.  
**Uso interno:** `BundlePosition<T>` en Rust.  
**TS Type:** No aplica (constructo Rust exclusivo).

#### Principio del Máximo de Pontryagin
**Definición:** Condición necesaria de optimalidad para problemas de control óptimo. El Hamiltoniano debe alcanzar su máximo respecto al control.  
**Uso interno:** Verificación en `OptimalControlSolution`.  
**TS Type:** No expuesto.

#### Proceso de Markov con Saltos
**Símbolo:** dXₜ = μdt + σdWₜ + ∫ J N(dt,dz)  
**Definición:** Extensión de la difusión de Itô que incluye discontinuidades (saltos) modeladas por una medida de Poisson compuesta.  
**Uso interno:** `MarkovJumpProcess` en Rust.  
**TS Type:** No expuesto.

#### Proyección Ortogonal
**Símbolo:** v_⟂ = P_{Π_⟂}(v)  
**Definición:** Operación lineal que proyecta un vector sobre el hiperplano ortogonal, eliminando la componente correlacionada.  
**Uso en API:** Campo `projected_direction` en `hedger` responses.  
**TS Type:** `number[]`

### R

#### Regularización Gaussiana
**Símbolo:** δ_ε(p) = (1/ε√π) exp(-||p-p₀||²/ε²)  
**Definición:** Aproximación suave de la delta de Dirac mediante una gaussiana de ancho ε. Necesaria para computación numérica.  
**Uso interno:** `DiracImpulse.evaluate()` en Rust.  
**TS Type:** No expuesto.

#### Restricción Hiperbólica
**Símbolo:** x·y = k  
**Definición:** Ecuación que define la variedad de liquidez ℒₖ en un pool CPMM. Toda operación del SED debe respetar esta restricción.  
**Uso en API:** Campo `hyperbolic_constraint_satisfied` (boolean) en `allocator` responses.  
**TS Type:** `boolean`

### S

#### Salto (Jump)
**Símbolo:** J(Xₜ₋, z)  
**Definición:** Función que describe el cambio discontinuo en el estado del mercado cuando ocurre un evento de mempool (transacción insertada/atada).  
**Uso interno:** `JumpDistribution` en Rust.  
**TS Type:** No expuesto.

#### Secuencial Equilibrium Dispatcher (SED)
**Definición:** Capa de orquestación matemática de más alto nivel en ArbitrageX v2. Coordina los 4 módulos canónicos para mantener el equilibrio asintótico del sistema.  
**Uso:** Nombre de la crate `backend/sed-core/`.  
**TS Type:** No aplica (sistema Rust).

#### Simulación de Trayectorias
**Símbolo:** {Xₜ^(i)}_{i=1..N}  
**Definición:** Conjunto de N caminos muestrales generados por Monte Carlo condicionados a la filtración ℱₜ. Utilizados para estimar el CDC.  
**Uso interno:** `MonteCarloEngine` en Rust.  
**TS Type:** No expuesto.

#### Sprint
**Definición:** Fase del roadmap de desarrollo (S1-S8). Cada módulo SED está mapeado a un sprint mínimo de activación.  
**Uso:** Campo `sprint` en respuestas 501.  
**TS Type:** `string` (ej: "S2", "S4", "S5")

#### State Divergence Coefficient (CDC)
*Ver:* **Coeficiente de Divergencia de Estado**

### T

#### Teorema de Girsanov
**Definición:** Teorema que permite cambiar la medida de probabilidad de un proceso estocástico, transformando drift pero preservando la estructura de difusión. Base del cálculo del CDC.  
**Uso interno:** `CDCCalculator` en Rust.  
**TS Type:** No expuesto.

#### Tipado Terminal
**Definición:** Sistema de tipos de Rust donde el compilador impide la construcción de estados inválidos. `BundlePosition<T>` solo permite `OrthogonalEquilibrium` y `DiracImpulseOnly`.  
**Uso interno:** `BundlePosition<T>` + `PhantomData` + `Sealed` trait.  
**TS Type:** No aplica.

#### Topología Post-Resolución
**Definición:** Estado matemático del mercado donde todas las perturbaciones han sido resueltas y el sistema se encuentra en equilibrio o listo para impulso controlado.  
**Uso interno:** `PostResolutionTopology` trait en Rust.  
**TS Type:** No expuesto.

#### Trajectoria de Costados (Costate)
**Símbolo:** p(t) = (p_x(t), p_y(t))  
**Definición:** Variables adjuntas en el problema de control óptimo. Representan el "precio sombra" de violar la restricción hiperbólica.  
**Uso en API:** Campo `costate_trajectory` en `allocator` responses.  
**TS Type:** `Array<{ x: number, y: number }>`

### U

#### Unidad Natural
**Definición:** Sistema de unidades donde ℏ = 1, c = 1. Simplifica las ecuaciones del Hamiltoniano cuántico.  
**Uso interno:** Cálculos en `EffectiveHamiltonian`.  
**TS Type:** No expuesto.

### V

#### Varianza Capturada
**Símbolo:** Var(Xₜ | ℱₜ)  
**Definición:** Varianza condicional del estado del mercado dada la información disponible. El SED busca maximizar su integral temporal.  
**Uso en API:** Campo `variance_envelope` en `filtration` responses.  
**TS Type:** `number`

#### Variedad de Liquidez
**Símbolo:** ℒₖ = {(x,y) | x·y = k}  
**Definición:** Subvariedad bidimensional de ℳ definida por la restricción del CPMM. Superficie de operación para la inyección de capital.  
**Uso en API:** Objeto `LiquidityManifold` con campos `constant_product`, `token0_reserve`, `token1_reserve`.  
**TS Type:** `{ constantProduct: string, token0Reserve: string, token1Reserve: string }`

#### Vector Direccional
**Símbolo:** v ∈ Tℳ  
**Definición:** Elemento del espacio tangente de la variedad del mercado. Representa una dirección de cambio en las reservas de tokens.  
**Uso en API:** Campo `original_direction` en `hedger` responses.  
**TS Type:** `number[]`

### W

#### Wiener (Proceso de)
**Símbolo:** Wₜ  
**Definición:** Proceso estocástico continuo con incrementos independientes y distribución normal. Modela la difusión de reservas en el mempool.  
**Uso interno:** Generador de trayectorias en `MonteCarloEngine`.  
**TS Type:** No expuesto.

---

## Mapeo TS ↔ Rust

| Concepto Matemático | Rust Type (`sed-core`) | TS Type (API) | Campo JSON |
|--------------------|----------------------|---------------|------------|
| CDC | `StateDivergenceCoefficient` | `number` | `cdcValue` |
| Eigenstate | `EigenState` | `{ energy, amplitude, degeneracy }` | `eigenstate` |
| Frontera de equilibrio | `EquilibriumBoundary` | `Array<[number, number]>` | `boundaryHypersurface` |
| Impulso Dirac | `DiracImpulse` | `{ injectionPoint, amplitude, supportRadius }` | `diracImpulse` |
| Control óptimo | `OptimalControlSolution` | `{ optimalControl, valueFunctional }` | `optimalControl` |
| Estado entrelazado | `EntangledState` | `{ amplitude, entanglementEntropy }` | `entangledState` |
| Covarianza | `DMatrix<f64>` | `number` | `covarianceOffDiagonal` |
| Bundle seguro | `BundlePosition<T>` | `{ marketId, tokenPair, liquidityCommitment }` | `bundlePosition` |
| Kill-Switch | `KillSwitchState` | `"Active" \| "Suspended" \| "Terminated"` | `killSwitchState` |
| Respuesta 501 | `NotImplementedResponse` | `{ requires: string[], sprint: string }` | `error` |

---

## Convenciones de Nomenclatura

### Rust (snake_case)
- `stochastic_wave_filtration`
- `state_divergence_coefficient`
- `equilibrium_boundary`
- `dirac_impulse`
- `orthogonal_hedge_result`

### TypeScript (camelCase)
- `stochasticWaveFiltration`
- `stateDivergenceCoefficient`
- `equilibriumBoundary`
- `diracImpulse`
- `orthogonalHedgeResult`

### PostgreSQL (snake_case)
- `sed_filtrations`
- `sed_eigenstates`
- `sed_allocations`
- `sed_hedges`

### Prometheus Metrics (snake_case)
- `sed_cdc_value`
- `sed_eigenstate_energy`
- `sed_dirac_amplitude`
- `sed_hedge_covariance`
- `sed_convergence_rate`

---

**Documento mantenido por OMEGA Agent Team.**  
**Actualización:** Cada vez que se agrega un nuevo módulo SED o se modifica la API.  
**Contacto:** quant@arbitragex.io
