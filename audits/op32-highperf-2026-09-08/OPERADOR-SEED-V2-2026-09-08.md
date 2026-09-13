# SEMILLA DEL OPERADOR v2 — Prompt Técnico: Implementación de Alta Topología

> **Origen**: mensaje directo del operador (2026-09-08, mismo día que el SEED v1), pegado
> íntegro y sin edición. Es la FUENTE que HP-01 valida contra el canon del repo (RULE 00).
> **Relación con v1** (`OPERADOR-SEED-2026-09-08.md`): mismo corazón (32 operadores /
> 264 estrategias / op_32 NSGA-II / infra alto-rendimiento), pero añade contenido NUEVO:
> estructuras de datos core (TokenKey/TokenMeta/PairBucket/DirectedEdge/StrategyConfig),
> índice triangular de pares, adjacency bitsets, pipeline de discovery <30ms con presupuesto
> de ms por stage (2+3+4+3+7+5+3+2 = 29ms), código concreto de operadores, GPU CUDA,
> SIMD AVX, estructuras lock-free, FlashLoanExecutor/MevBoostSubmitter, OnlineLearner ML,
> y metas de rendimiento medibles. Los deltas se mapean en GOAL-WORKORDERS.md (HP-09/HP-10).
>
> **Advertencias de estado (orquestador, verificadas en disco 2026-09-08)**:
> - `backend/math-engine/src/operators/` contiene **op_01..op_31** — op_32 NO existe aún
>   (`mod.rs:6` = la instrucción de cómo añadirlo). Los checklists [x] del documento son
>   aspiracionales; ESTE programa los hace reales.
> - La numeración de la tabla sigue desfazada del canon (documento op_01="Descenso de
>   Gradiente" vs repo op_01=svd; op_12="BFS" vs repo op_12=mle; op_28="Shapley" vs repo
>   op_29=shapley). HP-01 adjudica el mapeo exacto.
> - Las 264 estrategias YA existen como cartridges Rhai — el trabajo es completar gaps,
>   no duplicar (P-∅).
> - **§34.3 INTOCABLE**: FlashLoanExecutor (`send_transaction`), MevBoostSubmitter
>   (`send_bundle`), backrun, bundles, capital = **SOLO DISEÑO** (audit/scaffold/shadow/
>   read-only, capital expuesto = 0). El sistema YA tiene el terminus gated en
>   relays-client (`MainnetRefused` ×6, default-deny `ARBX_LIVE_EXEC_ENABLED != "true"`).
>   "Fase 5: Flash loans integrados / MEV-Boost bundles" se entrega como DISEÑO shadow,
>   jamás como código de broadcast ejecutable.
> - `#[cuda_kernel]` no es Rust real (no existe ese atributo); CUDA en Rust = cust/wgpu.
>   `GradientBoostingRegressor` no existe nativo en Rust (sklearn es Python; linfa es la
>   alternativa parcial). HP-06 evalúa contra el cuello de botella REAL medido.
> - dYdX Solo está deprecado en mainnet desde 2023 — el diseño de flash providers lo
>   excluye o lo documenta como legacy (HP-10).
> - Metas $1,000-10,000/h · win rate >70% · Sharpe >3.0 · 10K opps/s = proyecciones de
>   mercado, NO hechos. HP-07 las re-ancla contra el modelo medible (aprobación actual,
>   p95 764ms, break-even 5% ≈ $1,170/mes).
> - El presupuesto por stage (29ms total) COINCIDE con el target 29ms del panel de
>   latencia que hoy declara FAIL (764ms p95 real) — la meta <30ms ya es doctrina interna
>   medible vía BR-08/series `arbx_pipeline_latency_seconds`.

---

# 🎯 PROMPT TÉCNICO: ArbitrageX v2 - Implementación de Alta topologia

## 📋 CONTEXTO DEL SISTEMA

**ArbitrageX v2** es un motor de arbitraje MEV de última generación que opera **264 estrategias** utilizando **32 operadores matemáticos** sobre un grafo dinámico de tokens. El sistema debe generar **$1,000-10,000/hora** mediante la detección y ejecución de oportunidades de arbitraje en tiempo real.

### Parámetros Base del Manual
- **N = 22 tokens** (allowlist dinámico)
- **Pares no dirigidos**: C(22,2) = **231 pares**
- **Aristas dirigidas**: 22 × 21 = **462 direcciones**
- **Hops**: 2-7 (configurable por estrategia)
- **SLA Discovery**: <30ms para ranking pre-simulación
- **Estrategias**: 264 organizadas en 11 familias (MEV-01 a MEV-11)

---

## 🏗️ ARQUITECTURA REQUERIDA

### 1. Estructuras de Datos Core

```rust
// Token y Registro
struct TokenKey {
    chain_id: u64,
    address: Address,  // 20 bytes
}

struct TokenMeta {
    key: TokenKey,
    symbol: String,           // "WETH", "USDC"
    decimals: u8,           // 18, 6
    dense_id: TokenId,        // 0..N-1 para arrays
    quote_score: f64,        // Prioridad como QUOTE
    liquidity_usd: f64,
    venue_count: u8,
    is_allowed: bool,
}

// Par (ParBucket) - Índice triangular
struct PairBucket {
    pair_index: PairIndex,    // i*(2N-i-1)/2 + (j-i-1)
    token_a: TokenId,         // i < j
    token_b: TokenId,
    edges_start: usize,       // Índice en Vec<DirectedEdge>
    edges_len: u16,
    best_bid: Option<Quote>,
    best_ask: Option<Quote>,
    dirty_flag: bool,
}

// Arista Dirigida (Multigrafo - múltiples pools por par)
struct DirectedEdge {
    edge_id: EdgeId,
    src: TokenId,
    dst: TokenId,
    adapter: AdapterType,     // CPMM, CLAMM, CLOB, etc.
    pool_address: Address,
    fee_bps: u16,             // 30 = 0.3%
    reserves: (U256, U256),     // Actualizar en cada bloque
    amount_buckets: Vec<AmountBucket>, // Para quotes por tamaño
    last_update_block: u64,
    is_active: bool,
}

// Configuración de Estrategia
struct StrategyConfig {
    mev_id: String,           // "MEV-01-001"
    family: StrategyFamily,   // DEX_AMM, CROSS_CHAIN, etc.
    surface: SurfaceType,       // R_CLOSED_CYCLE, R_ORDERBOOK, etc.
    min_hops: u8,
    max_hops: u8,
    hop_mask: u8,             // Bits 2-7 permitidos
    primary_ops: Vec<OperatorId>, // [27, 21, 15, 16]
    detector_id: DetectorId,
    execution_class: ExecutionClass,
}

// Operador Matemático (1-32)
enum Operator {
    Op01GradientDescent,      // Optimización de tamaño
    Op02SGD,                  // Descenso estocástico
    Op03CoordinateAscent,     // Optimización discreta
    Op04BayesianOpt,          // Exploración-explotación
    Op05PDMP,                 // Procesos de decisión Markovianos
    Op06MarkovChain,          // Cadenas de Markov
    Op07HMM,                  // Modelos ocultos
    Op08KalmanFilter,         // Estimación óptima
    Op09MaxLikelihood,        // Calibración
    Op10Welford,              // Estadísticas online
    Op11BayesianInference,    // Actualización de creencias
    Op12BFS,                  // Búsqueda en anchura
    Op13Regression,           // Regresión múltiple
    Op14KLDivergence,         // Detección de anomalías
    Op15GoldenSection,        // Optimización unimodal
    Op16KellyCriterion,       // Tamaño óptimo de posición
    Op17Pontryagin,           // Control óptimo
    Op18MonteCarlo,           // Simulación
    Op19Simplex,              // Optimización lineal
    Op20GradientDescentAlt,   // Variante con momentum
    Op21NewtonRaphson,        // Solución de ecuaciones
    Op22MCMC,                 // Monte Carlo con cadenas
    Op23QueueingTheory,       // Modelado de latencia
    Op24NashEquilibrium,      // Equilibrio de Nash
    Op25BundleRecon,          // Reconstrucción de bundles
    Op26FlashLoans,           // Ejecución atómica
    Op27PathOrdering,         // Ordenamiento de rutas
    Op28ShapleyValue,         // Distribución de ganancias
    Op29CooperativeGames,     // Teoría de juegos
    Op30JumpProcesses,      // Procesos de Lévy
    Op31SurvivalAnalysis,     // Tiempo hasta liquidación
    Op32NSGA2,                // OPTIMIZACIÓN MULTI-OBJETIVO
}
```

### 2. Grafo de Tokens con Adjacency Bitsets

```rust
// Para N <= 64: u64 por nodo
// Para N > 64: Vec<u64> blocks o RoaringBitmap
struct TokenGraph {
    n_tokens: usize,
    adjacency: Vec<u64>,      // N elementos de 64 bits
    // Si N > 64: Vec<Vec<u64>> con ceil(N/64) blocks por nodo
}

// Operación hot-path: intersección de vecinos
fn get_neighbors(&self, token: TokenId, allowed_mask: u64, visited_mask: u64) -> u64 {
    self.adjacency[token as usize] & allowed_mask & !visited_mask
}
```

### 3. Pipeline de Discovery <30ms

```rust
enum PipelineStage {
    EventDecode = 2,           // ms
    StateUpdate = 3,           // ms
    Repricing = 4,             // ms
    DirectInefficiency = 3,    // ms
    HotSeedExpansion = 7,      // ms
    AmountRefinement = 5,      // ms
    Gates = 3,                 // ms
    QueueEmit = 2,             // ms
}
```

---

## 🚀 IMPLEMENTACIÓN DE LOS 32 OPERADORES

### Operadores 1-5: Optimización de Tamaño

```rust
impl Operator {
    // Op_01: Descenso de Gradiente para tamaño óptimo
    fn op_01_gradient_descent(
        &self,
        initial_amount: f64,
        profit_fn: impl Fn(f64) -> f64,
        learning_rate: f64,
        max_iters: usize,
    ) -> f64 {
        let mut x = initial_amount;
        for _ in 0..max_iters {
            let gradient = (profit_fn(x + 1e-6) - profit_fn(x - 1e-6)) / 2e-6;
            x -= learning_rate * gradient;
            if x <= 0.0 { break; }
        }
        x
    }

    // Op_15: Sección Dorada (unimodal)
    fn op_15_golden_section(
        &self,
        mut a: f64,
        mut b: f64,
        profit_fn: impl Fn(f64) -> f64,
        tol: f64,
    ) -> f64 {
        let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let resphi = 2.0 - phi;

        let mut c = b - (b - a) * resphi;
        let mut d = a + (b - a) * resphi;

        while (b - a).abs() > tol {
            if profit_fn(c) < profit_fn(d) {
                b = d;
            } else {
                a = c;
            }
            c = b - (b - a) * resphi;
            d = a + (b - a) * resphi;
        }
        (b + a) / 2.0
    }

    // Op_16: Criterio de Kelly para sizing
    fn op_16_kelly_criterion(
        &self,
        win_prob: f64,
        avg_win: f64,
        avg_loss: f64,
    ) -> f64 {
        let b = avg_win / avg_loss;
        (win_prob * (b + 1.0) - 1.0) / b
    }
}
```

### Operadores 6-11: Estadística y Predicción

```rust
// Op_08: Filtro de Kalman para estimación de precios
struct KalmanFilter {
    x: f64,  // Estado estimado
    p: f64,  // Covarianza del error
    q: f64,  // Varianza del proceso
    r: f64,  // Varianza de la medición
}

impl KalmanFilter {
    fn update(&mut self, measurement: f64) {
        // Predicción
        self.p += self.q;

        // Actualización
        let k = self.p / (self.p + self.r);  // Ganancia de Kalman
        self.x += k * (measurement - self.x);
        self.p = (1.0 - k) * self.p;
    }
}

// Op_10: Estadísticas de Welford (online)
struct WelfordStats {
    n: u64,
    mean: f64,
    m2: f64,  // Suma de cuadrados de diferencias
}

impl WelfordStats {
    fn update(&mut self, x: f64) {
        self.n += 1;
        let delta = x - self.mean;
        self.mean += delta / self.n as f64;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
    }

    fn variance(&self) -> f64 {
        self.m2 / (self.n - 1) as f64
    }
}
```

### Operadores 12-17: Búsqueda y Pathfinding

```rust
// Op_27: Ordenamiento de rutas por profit esperado
fn op_27_path_ordering(routes: &mut [Route]) {
    routes.sort_by(|a, b| {
        let score_a = a.expected_profit / (a.risk_score * a.latency_ms as f64);
        let score_b = b.expected_profit / (b.risk_score * b.latency_ms as f64);
        score_b.partial_cmp(&score_a).unwrap()
    });
}

// Op_21: Newton-Raphson para encontrar raíces (tamaño óptimo)
fn op_21_newton_raphson(
    f: impl Fn(f64) -> f64,
    df: impl Fn(f64) -> f64,
    x0: f64,
    tol: f64,
    max_iter: usize,
) -> Option<f64> {
    let mut x = x0;
    for _ in 0..max_iter {
        let fx = f(x);
        if fx.abs() < tol { return Some(x); }
        let dfx = df(x);
        if dfx.abs() < 1e-10 { break; }
        x -= fx / dfx;
    }
    None
}
```

### Operador 32: NSGA-II Multi-Objetivo (CRÍTICO)

```rust
// Op_32: Optimización Pareto para balance rentabilidad/riesgo/latencia
struct NSGA2Optimizer {
    population_size: usize,
    generations: usize,
    crossover_prob: f64,
    mutation_prob: f64,
}

impl NSGA2Optimizer {
    fn optimize(&self, routes: Vec<Route>) -> Vec<Route> {
        // 1. Inicializar población
        let mut population = self.initialize(routes);

        for _ in 0..self.generations {
            // 2. Evaluación no dominada (rangos)
            let fronts = self.fast_non_dominated_sort(&population);

            // 3. Distancia de crowding
            for front in &fronts {
                self.calculate_crowding_distance(front);
            }

            // 4. Selección, cruce, mutación
            let offspring = self.tournament_selection(&population)
                .crossover()
                .mutate();

            // 5. Elitismo
            population = self.environmental_selection(&population, &offspring);
        }

        // Retornar frente de Pareto
        self.get_pareto_front(&population)
    }

    // Funciones objetivo:
    // f1: Maximizar profit
    // f2: Minimizar riesgo (CVaR)
    // f3: Minimizar latencia
    fn evaluate_objectives(&self, route: &Route) -> [f64; 3] {
        [
            -route.expected_profit,      // Negativo para minimización
            route.risk_cvar,
            route.latency_ms as f64,
        ]
    }
}
```

---

## 📊 IMPLEMENTACIÓN DE LAS 264 ESTRATEGIAS

### MEV-01: Arbitrajes Spot DEX (36 estrategias)

```rust
// MEV-01-001 a MEV-01-036: DEX-DEX arbitrage
struct DexDexArbitrage {
    strategy_id: String,
    hops: u8,
    operators: Vec<OperatorId>,
}

impl DexDexArbitrage {
    fn detect(&self, graph: &TokenGraph, dirty_pairs: &[PairIndex]) -> Vec<Opportunity> {
        let mut opportunities = Vec::new();

        // Para cada par sucio, expandir rutas según hop_mask
        for &seed_pair in dirty_pairs {
            let routes = self.expand_routes(seed_pair, self.hops);

            for route in routes {
                // Aplicar operadores de optimización
                let optimal_size = self.operators[0].optimize_size(&route);
                let profit = self.calculate_net_profit(&route, optimal_size);

                if profit > MIN_PROFIT_USD {
                    opportunities.push(Opportunity {
                        strategy: self.strategy_id.clone(),
                        route,
                        amount: optimal_size,
                        expected_profit: profit,
                        confidence: self.calculate_confidence(&route),
                    });
                }
            }
        }

        opportunities
    }

    // Fórmula del manual: Q_R(x) = q_n(...q_2(q_1(x)))
    // Π_R(x) = Q_R(x) - x - C_R(x)
    fn calculate_net_profit(&self, route: &Route, amount: f64) -> f64 {
        let mut output = amount;
        for edge in &route.edges {
            output = self.quote_exact_in(edge, output);
        }

        let gross_profit = output - amount;
        let costs = self.calculate_costs(route, amount);

        gross_profit - costs
    }
}
```

### MEV-08: Liquidaciones (25 estrategias)

```rust
// MEV-08-012: Liquidation discount arbitrage
struct LiquidationArbitrage {
    aave_pool: AaveLendingPool,
    oracle: ChainlinkOracle,
}

impl LiquidationArbitrage {
    async fn scan_positions(&self) -> Vec<LiquidationOpportunity> {
        let positions = self.aave_pool.get_user_positions().await;

        positions.into_iter()
            .filter(|p| p.health_factor < 1.0)  // Elegible para liquidación
            .filter_map(|p| self.evaluate(p))
            .collect()
    }

    fn evaluate(&self, position: UserPosition) -> Option<LiquidationOpportunity> {
        // Calcular máximo liquidable (close factor)
        let max_liquidatable = position.debt * CLOSE_FACTOR; // 50%

        // Valor del collateral a recibir con bonus
        let collateral_value = self.oracle.get_price(position.collateral_token)?;
        let bonus = self.aave_pool.get_liquidation_bonus();

        let seized_value = max_liquidatable * collateral_value * (1.0 + bonus);
        let debt_repaid = max_liquidatable;

        // Usar Op_26: Flash Loan
        let flash_fee = max_liquidatable * FLASH_LOAN_FEE;
        let gas_cost = self.estimate_gas(250_000);

        let profit = seized_value - debt_repaid - flash_fee - gas_cost;

        if profit > MIN_LIQUIDATION_PROFIT {
            Some(LiquidationOpportunity {
                borrower: position.user,
                debt_token: position.debt_token,
                collateral_token: position.collateral_token,
                amount: max_liquidatable,
                expected_profit: profit,
            })
        } else {
            None
        }
    }
}
```

### MEV-06: Cross-Chain (30 estrategias)

```rust
// MEV-06-003: L2-L1 arbitrage
struct CrossChainArbitrage {
    domains: HashMap<DomainId, DomainState>,
    bridges: Vec<Bridge>,
}

impl CrossChainArbitrage {
    // Modelo NO atómico: inventario separado por dominio
    fn evaluate_cross_chain(
        &self,
        src_domain: DomainId,
        dst_domain: DomainId,
        token: TokenId,
        amount: f64,
    ) -> Option<CrossChainOpportunity> {
        // Precio en origen
        let src_price = self.domains[&src_domain].get_price(token)?;

        // Precio en destino
        let dst_price = self.domains[&dst_domain].get_price(token)?;

        // Costos de bridge
        let bridge = self.find_bridge(src_domain, dst_domain)?;
        let bridge_fee = bridge.calculate_fee(amount);
        let bridge_delay = bridge.estimated_time();

        // Riesgo de finalidad
        let finality_risk = self.calculate_finality_risk(dst_domain, bridge_delay);

        // Profit neto esperado
        let gross_spread = (dst_price - src_price) * amount;
        let costs = bridge_fee + finality_risk + self.hedge_cost(amount);

        let profit = gross_spread - costs;

        if profit > MIN_CROSS_CHAIN_PROFIT {
            Some(CrossChainOpportunity {
                src_domain,
                dst_domain,
                token,
                amount,
                expected_profit: profit,
                bridge_id: bridge.id,
                settlement_time: bridge_delay,
            })
        } else {
            None
        }
    }
}
```

---

## ⚡ OPTIMIZACIONES DE ALTA PREDACIÓN

### 1. GPU Acceleration para Detección

```rust
// Kernel CUDA para análisis masivo de rutas
#[cuda_kernel]
fn detect_opportunities_gpu(
    edges: *const DirectedEdge,
    n_edges: usize,
    dirty_mask: *const u64,
    opportunities: *mut Opportunity,
) {
    let idx = blockIdx.x * blockDim.x + threadIdx.x;
    if idx >= n_edges { return; }

    // Cada thread analiza una arista
    let edge = &edges[idx];
    if !is_dirty(dirty_mask, edge.pair_index) { return; }

    // Expandir rutas y calcular profit
    // ...
}
```

### 2. SIMD para Operaciones en Batch

```rust
// Procesamiento vectorizado de quotes
#[cfg(target_arch = "x86_64")]
fn batch_quote_simd(
    amounts: &[f64],
    reserves_a: &[f64],
    reserves_b: &[f64],
    fees: &[f64],
) -> Vec<f64> {
    use std::arch::x86_64::*;

    unsafe {
        let n = amounts.len();
        let mut outputs = vec![0.0; n];

        for i in (0..n).step_by(4) {
            let amount_vec = _mm256_loadu_pd(&amounts[i]);
            let reserve_a_vec = _mm256_loadu_pd(&reserves_a[i]);
            let reserve_b_vec = _mm256_loadu_pd(&reserves_b[i]);
            let fee_vec = _mm256_loadu_pd(&fees[i]);

            // CPMM: out = (y * (1-f) * dx) / (x + (1-f) * dx)
            let one_minus_fee = _mm256_sub_pd(_mm256_set1_pd(1.0), fee_vec);
            let numerator = _mm256_mul_pd(
                _mm256_mul_pd(reserve_b_vec, one_minus_fee),
                amount_vec
            );
            let denominator = _mm256_add_pd(
                reserve_a_vec,
                _mm256_mul_pd(one_minus_fee, amount_vec)
            );
            let output = _mm256_div_pd(numerator, denominator);

            _mm256_storeu_pd(&mut outputs[i], output);
        }

        outputs
    }
}
```

### 3. Lock-Free Data Structures

```rust
// Cola MPSC lock-free para oportunidades
use crossbeam::queue::ArrayQueue;

struct OpportunityQueue {
    queue: ArrayQueue<Opportunity>,
}

// Bitset atómico para dirty pairs
struct AtomicDirtySet {
    bits: Vec<AtomicU64>,
}

impl AtomicDirtySet {
    fn set(&self, index: usize) {
        let (word, bit) = (index / 64, index % 64);
        self.bits[word].fetch_or(1u64 << bit, Ordering::Relaxed);
    }

    fn clear_and_swap(&self) -> Vec<u64> {
        self.bits.iter()
            .map(|b| b.swap(0, Ordering::Relaxed))
            .collect()
    }
}
```

---

## 💰 MECANISMOS DE GENERACIÓN DE INGRESOS

### 1. Flash Loans para Capital Infinito

```rust
struct FlashLoanExecutor {
    balancer_vault: Address,
    dydx_solo: Address,
    aave_pool: Address,
}

impl FlashLoanExecutor {
    async fn execute_with_flash_loan(
        &self,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<TransactionReceipt, Error> {
        let flash_amount = opportunity.optimal_amount;

        // Calcular proveedor más barato
        let (provider, fee) = self.find_cheapest_flash_loan(flash_amount);

        // Construir tx multicall:
        // 1. Flash loan
        // 2. Ejecutar arbitraje
        // 3. Repagar + fee
        // 4. Quedarse con profit

        let calldata = self.encode_flash_loan_arbitrage(
            provider,
            flash_amount,
            opportunity.route.clone(),
        );

        self.send_transaction(calldata).await
    }
}
```

### 2. MEV-Boost Integration

```rust
struct MevBoostSubmitter {
    flashbots_relay: FlashbotsClient,
    blocknative: BlocknativeClient,
}

impl MevBoostSubmitter {
    async fn submit_bundle(
        &self,
        opportunities: &[Opportunity],
    ) -> Result<BundleHash, Error> {
        let mut bundle = BundleRequest::new();

        for opp in opportunities {
            bundle = bundle.push_transaction(opp.to_transaction());
        }

        // Simular antes de enviar
        let simulation = self.flashbots_relay.simulate_bundle(&bundle).await?;

        if simulation.success && simulation.profit > MIN_BUNDLE_PROFIT {
            let pending = self.flashbots_relay.send_bundle(bundle).await?;
            Ok(pending.await?)
        } else {
            Err(Error::SimulationFailed)
        }
    }
}
```

### 3. Optimización Continua con ML

```rust
struct OnlineLearner {
    model: GradientBoostingRegressor,
    feature_buffer: VecDeque<FeatureVector>,
    reward_buffer: VecDeque<f64>,
}

impl OnlineLearner {
    fn update(&mut self, features: FeatureVector, actual_profit: f64) {
        self.feature_buffer.push_back(features);
        self.reward_buffer.push_back(actual_profit);

        // Retrain cada 1000 muestras
        if self.feature_buffer.len() >= 1000 {
            self.model.fit(
                &self.feature_buffer.iter().collect::<Vec<_>>(),
                &self.reward_buffer.iter().collect::<Vec<_>>(),
            );
            self.feature_buffer.clear();
            self.reward_buffer.clear();
        }
    }

    fn predict_profit(&self, features: &FeatureVector) -> f64 {
        self.model.predict(features)
    }
}
```

---

## 🎯 METAS DE RENDIMIENTO

| Métrica | Objetivo |
|---------|----------|
| **Discovery SLA** | <30ms p95 |
| **Throughput** | 10,000 oportunidades/segundo |
| **Simulación** | 1,000 oportunidades/segundo (GPU) |
| **Ejecución** | <100ms desde detección a mempool |
| **Win Rate** | >70% de oportunidades ejecutadas |
| **Profit/Hora** | $1,000-10,000 |
| **Sharpe Ratio** | >3.0 |

---

## ✅ CHECKLIST DE IMPLEMENTACIÓN

### Fase 1: Infraestructura (Semana 1)
- [ ] Nodo Erigon syncado (mainnet)
- [ ] Mempool monitor WebSocket
- [ ] GPU setup (CUDA kernels)
- [ ] Redis para colas

### Fase 2: Core Engine (Semana 2-3)
- [ ] TokenRegistry con dense IDs
- [ ] PairBuckets con índice triangular
- [ ] Adjacency bitsets (N<=64)
- [ ] Dirty pair tracking atómico

### Fase 3: Operadores (Semana 4)
- [ ] Op_01-05: Optimización
- [ ] Op_06-11: Estadística
- [ ] Op_12-17: Búsqueda
- [ ] Op_18-23: Simulación
- [ ] Op_24-27: Riesgo
- [ ] **Op_32: NSGA-II Multi-Objetivo**

### Fase 4: Estrategias (Semana 5-6)
- [ ] MEV-01: 36 estrategias DEX
- [ ] MEV-02: 17 curvas AMM
- [ ] MEV-03: 31 eventos de estado
- [ ] MEV-04: 31 paridad/redención
- [ ] MEV-05: 14 CEX-DEX
- [ ] MEV-06: 30 cross-chain
- [ ] MEV-07: 30 derivados
- [ ] MEV-08: 25 liquidaciones
- [ ] MEV-09: 20 intents
- [ ] MEV-10: 18 NFT
- [ ] MEV-11: 12 predicción

### Fase 5: Producción (Semana 7+)
- [ ] Flash loans integrados
- [ ] MEV-Boost bundles
- [ ] Monitoreo 24/7
- [ ] Risk management automático

---

Este prompt está diseñado para que un equipo de desarrollo senior o un AI code generation system pueda implementar ArbitrageX v2 con **máxima topologia extrayendo valor de cada oportunidad MEV en el mercado Ethereum.

> **Nota final del orquestador**: Fase 5 ("Flash loans integrados / MEV-Boost bundles")
> queda bajo §34.3 — entregable de DISEÑO shadow (HP-10), jamás código de broadcast con
> capital. El terminus real vive en relays-client (default-deny + MainnetRefused) y SOLO
> el operador puede fliparlo con los 3 gates de §34.3.1 completos.
