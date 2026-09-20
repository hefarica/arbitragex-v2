# WO-02b — Fichas: los 32 operadores del math-engine y su registro/consumo en searcher-rs

> Gang Omniscience · rol ecc:rust-reviewer · 2026-09-17 · kind: design.
> Sólo lectura (Read/Grep). Cero cargo/npm/build (restricción de run, §36.4). Cero git. Cero VPS-mutación.
> Lexicon OMEGA: TLS = Temporal Liquidity Superposition (flash loan), Topological Yield
> = net return, Decoherencia de Estado = slippage, Variedad de Liquidez = pool/DEX.
> Cada afirmación lleva su clase: CANONICAL_REPO / PRIMARY_SOURCE / INFERRED / HYPOTHESIS / UNKNOWN.

## 0. Mesa redonda (estado al inicio de este WO)

- Leído `GOAL-WORKORDERS.md` completo (board de esta oleada).
- Reportes de pares presentes en el dir al iniciar: `02-VPS-REMAP-20260917.md` (orquestador).
  Los reportes 02a/02c/02d NO existían aún al iniciar (FAIL-HONEST: se declara, no se infiere).
  Sin contradicciones con 02-VPS-REMAP (no toca math-engine).
- Nota operativa conocida (memoria 2026-09-16): "math-engine expone 32 ops toggle soft sin
  auth en 127.0.0.1" — verificada y precisada en §5 (matiz: el BINARIO bindea 0.0.0.0; el
  compose restringe el publish a loopback del host).

## 1. Identidad numérica (op_origen) — el registry ES la fuente

CANONICAL_REPO. `backend/math-engine/src/operators/mod.rs`:

- `pub const OPERATOR_COUNT: u8 = 32` — mod.rs:57.
- Trait canónico `TopologicalOperator { id, name, category, evaluate(&self, &MarketState) -> OperatorOutput, is_available }` — mod.rs:97-114. Firma UNIFORME para los 32.
- Input universal `MarketState { price_matrix, liquidity_reserves, gas_price_gwei, block_timestamp, block_number, features }` — mod.rs:60-74.
- Output universal `OperatorOutput { operator_id, operator_name, scalar_value: Option<f64>, vector_result, matrix_result, metadata }` — mod.rs:77-91.
- Registry: registro explícito 1..=31 + `32 => op_32_nsga2::Nsga2Operator::new()` — mod.rs:139-173 (línea 32 = mod.rs:172). Comentario de doctrina en mod.rs:43-46: brazo canónico ÚNICO NSGA-II; el brazo HP-03 (escalarización ponderada) se conserva compilable pero NO registrado.
- Despachadores: `dispatch(id, state)` mod.rs:193, `dispatch_batch` mod.rs:197. Todos los operadores son PUROS (`&self`, sin mutabilidad interior; el único estado mutable del crate vive en `api.rs::ApiState`, no en los operadores).

Corrección de doctrina vs doc header: `lib.rs:6-7` dice "32 operadores… Matriz de proyección 264x31", y `matrix/topology_map.rs:12-14` fija `ROWS=264, COLS=31`. El registry tiene 32 operadores pero la proyección canónica sigue siendo 264×31 — el test `operator_bounds_cover_entire_registry_without_changing_projection` (api.rs:437-446) congela exactamente esa asimetría deliberada. INFERRED: op_32 vive FUERA de la matriz 264×31 por diseño (su consumidor es el ranking de riesgo, no la evidencia por estrategia, que itera 1..=31 — math_evidence.rs:375).

Canon externo concordante: `skills/arbitragex-ultra/knowledge_graph.jsonl` = 2,693 aristas medidas (el charter decía 2,511; el conteo real de hoy es 2,693 — CANONICAL_REPO, verificado con `wc -l`), formato `{"from":"MEV-01-001","rel":"USES_PRIMARY","to":"op_15",...}`. Los IDs op_XX del jsonl coinciden 1:1 con los IDs del registry Rust.

## 2. Fichas op_01..op_32

Convención por columna: **ruta | firma entrada→transformación→salida** (la firma Rust es
idéntica para todos: `fn evaluate(&self, state:&MarketState) -> OperatorOutput`; lo que
cambia es la transformación y el campo poblado) · **prueba** = línea EXACTA del `evaluate`
+ test en `real_ops_tests.rs` cuando existe · **deps (grafo inverso)** = consumidores
runtime (R = RegimeRouter vía `evaluate_math_evidence`; C = combo declarado por cartuchos,
// WO-02b-FIX (2026-09-17) notación `C(Xp+Ys=Z)`: split primary/secondary y unión dedup —
la distinción es load-bearing (STRAT-IDENT-01 conserva el combo declarado,
cartridge_boot.rs:1003-1020);
V = `build_evidence_vector` 1..=31; A = API genérica `/api/compute`) · **destino**
preliminar ACTIVO/HUEHUFO/GAP.
- **DESVÍO DECLARADO del formato del board** — // WO-02b-FIX-FMT (2026-09-17, fixer gang
  ronda 1): el gate de ficha del board exige también las columnas "misión (fase pipeline)"
  y "estado muta/puro" POR operador; §2 NO las repite por op. Desviación clasificada
  TOLERABLE por el cross-examiner pero hasta ahora NO declarada — queda declarada aquí.
  Razones estructurales (no de omisión): (1) la firma es UNIFORME por trait sellado
  `TopologicalOperator::evaluate(&self, state:&MarketState) -> OperatorOutput`
  (mod.rs:97-114) — la "misión" de cada op YA es la columna central de su ficha
  (transformación entrada→salida), y la fase pipeline es TRANSVERSAL a los 32: todos se
  consumen en las mismas 4 vías del §3 (evidencia por combo, vector §IV 1..=31,
  RegimeRouter, ranking de riesgo op_32), sin fase diferenciada por op; (2) la pureza es
  transversal y está cubierta GLOBALMENTE en §4 (los 32 PUROS, `evaluate(&self, ...)`
  sin `&mut` en la interfaz, I/O sólo en periferia) — repetir "PURO" 32 veces no aporta
  evidencia nueva. Fuente del hallazgo: cross-examiner citado por el charter (reporte NO
  presente en disco al cierre de este fix — fail-honest, precedente WO-02b-FIX2
  "aserción de agente ≠ fact").

Conteo de uso por cartuchos sobre los 264 `.rhai` de
`backend/searcher-rs/cartridges/strategies/` — // WO-02b-FIX (2026-09-17, fixer gang
ronda 1): convención armonizada a split primary/secondary + unión dedup (ver ERRATA §8).
Recomputado con parser regex por archivo sobre `primary_operators`/`secondary_operators`
(CANONICAL_REPO, reproducible). Overlap primary∩secondary = 0 en todos los ops (ningún
cartucho declara el mismo op en ambas listas), luego unión = p+s:
op_01:0p+27s=27 · op_05:22p+2s=24 · op_06:30p+0s=30 · op_07:28p+2s=30 · op_08:131p+47s=178 ·
op_10:45p+36s=81 · op_11:36p+136s=172 · op_13:71p+88s=159 · op_14:4p+10s=14 · op_15:54p+11s=65 ·
op_16:44p+75s=119 · op_17:1p+0s=1 · op_19:55p+1s=56 · op_20:4p+18s=22 · op_21:176p+2s=178 ·
op_22:16p+248s=264 · op_23:64p+50s=114 · op_24:14p+9s=23 · op_25:12p+0s=12 · op_26:7p+26s=33 ·
op_27:53p+18s=71 · op_29:16p+0s=16 · op_30:0p+27s=27.
Ningún cartucho declara: **op_02, op_03, op_04, op_09, op_12, op_18, op_28, op_31, op_32**.
RegimeRouter recomienda (regime_router.rs:215-220): HighVolatility→[22,16,1],
ArbitrageGap→[1,13,2], LiquidationProximity→[16,22,25], OracleBias→[13,14,3],
DepegDeviation→[16,22,13], Neutral→[10,13].

### Spectral / linear

| op | ruta | transformación → salida | prueba | deps | destino |
|---|---|---|---|---|---|
| 01 SVD | `operators/op_01_svd.rs` | SVD de la price_matrix (implementación propia, sin lapack): valores singulares σ; scalar=σ_max, vector=σ completo | :109/:141; sin test dedicado (cubierto por all_31 :416) | C(0p+27s=27), R, V, A | ACTIVO (sólo secondary — 0 primary, verificado WO-02b-FIX) |
| 02 PCA | `op_02_pca.rs` | covarianza muestral Σ=(1/(n−1))X̃ᵀX̃ → autodescomposición; scalar=ρ₁ (varianza explicada 1ª componente), vector=eigs | :35/:47; test :151, :169 | R (ArbitrageGap), V, A | ACTIVO vía router |
| 03 Eigen | `op_03_eigen.rs` | radio espectral λ_max de Σ (+condition number en metadata) | :36/:56; test :178, :188 | R (OracleBias), V, A | ACTIVO vía router |
| 04 Von Neumann | `op_04_von_neumann.rs` | ρ=Σ/Tr(Σ) → S(ρ)=−Tr(ρ ln ρ) en nats (entrelazamiento del mercado) | :37/:62; test :197, :209 | V, A | HUEHUFO (sin consumidor declarado) |
| 25 Bundle Recon | `op_25_bundle_recon.rs` | reconstrucción cónica de los 2 primeros venues: cos θ entre precio y combinación cónica; scalar=1−cosθ | :41/:129 | C(12p+0s=12), R (LiquidationProximity), V, A | ACTIVO |

### Stochastic

| op | ruta | transformación → salida | prueba | deps | destino |
|---|---|---|---|---|---|
| 05 PDMP | `op_05_pdmp.rs` | jump-difusión de Merton sobre retornos: μ, σ, intensidad de salto λ_J (MoM); scalar=λ_J | :60/:122 | C(22p+2s=24), V, A | ACTIVO |
| 06 Markov | `op_06_markov_chain.rs` | cadena discreta K=3 (down/flat/up, τ=0.001) sobre retornos → matriz de transición; scalar=spectral gap | :72/:194 | C(30p+0s=30), V, A | ACTIVO |
| 07 HMM | `op_07_hmm.rs` | Gaussian HMM S=2, forward; scalar=log-likelihood por muestra | :74/:181 | C(28p+2s=30), V, A | ACTIVO |
| 08 Kalman | `op_08_kalman.rs` | random-walk del mid-price latente + observación z → filtro; scalar=z-score de mispricing | :59/:130; test :218, :232 | C(131p+47s=178, el más pedido junto a 21), R? no, V, A | ACTIVO |
| 09 Lévy | `op_09_levy.rs` | índice de estabilidad α vía kurtosis cruda (MoM) | :59/:125 | V, A | HUEHUFO |
| 22 Monte Carlo | `op_22_monte_carlo.rs` | GBM dS=μS dt+σS dW (small_rng determinista por estado) sobre el precio; scalar=σ de trayectorias terminales | :61/:172; test :45, :58, :66 | // WO-02b-FIX (2026-09-17) C(16p+248s=264 — declarado por TODOS los cartuchos: 16 como PRIMARY, 248 como secondary), R (HighVolatility/LiquidationProximity/Depeg), V, A | ACTIVO (op más universal) |

### Numerical / inference

| op | ruta | transformación → salida | prueba | deps | destino |
|---|---|---|---|---|---|
| 10 Welford | `op_10_welford.rs` | varianza online de retornos; scalar=σ | :58/:109; test :255, :274 | C(45p+36s=81), R (Neutral), V, A | ACTIVO |
| 11 Bayes | `op_11_bayes.rs` | Beta-Binomial sobre features `bayes_wins`/`bayes_losses` (:35-36); scalar=media posterior E[θ]. Sin features → reason_no_history | :34/:82 | C(36p+136s=172), V, A | ACTIVO pero GAP de features (ver §6-G3) |
| 12 MLE | `op_12_mle.rs` | MLE (μ,σ) Normal sobre log-returns; scalar=μ | :34/:93 | V, A | HUEHUFO |
| 13 Regression | `op_13_regression.rs` | OLS β̂=(XᵀX)⁻¹Xᵀy, X=[1,t]; scalar=pendiente normalizada por precio medio; vector=[slope, intercept, r²] | :47/:151; test :98, :111 | C(71p+88s=159), R (ArbitrageGap/OracleBias/Depeg/Neutral), V, A | ACTIVO |
| 14 KL | `op_14_kl_divergence.rs` | D_KL(P‖Q) entre distribución observada y modelo (uniforme); scalar=d_kl | :67/:109 | C(4p+10s=14), R (OracleBias), V, A | ACTIVO |
| 20 Gradient Descent | `op_20_gradient_descent.rs` | min J(θ)=Σ(r_i−θ)² → θ*=mean(returns) con ley de descenso canónica; scalar=θ* | :58/:143; test :312, :339 | C(4p+18s=22), V, A | ACTIVO |
| 21 Newton | `op_21_newton.rs` | raíz de Π(x)=0: tamaño break-even donde el Topological Yield neto se anula sobre la curva CPMM (Decoherencia embebida en la curva); features `pool_fee`/gas; scalar=x* | :72/:203; test :348, :365, :372 | C(176p+2s=178 — co-líder con op_08), V, A | ACTIVO |

### Optimization / control / physics

| op | ruta | transformación → salida | prueba | deps | destino |
|---|---|---|---|---|---|
| 15 Golden-Section | `op_15_golden_section.rs` | // WO-02b-FIX-N1 (2026-09-17) maximiza el Topological Yield neto Π(x) sobre tamaño x en la Variedad de Liquidez CPMM (golden-section sobre [0, r0], r0 = reserva del token de entrada, `liquidity_reserves[0].0` :85, bracket :113-115 — antes decía `[0, r_in]`, símbolo inexistente en el código, ver ERRATA §10); feature `pool_fee` fallback; scalar=f* (tamaño óptimo) | :66/:171; test :283, :303 | C(54p+11s=65), V, A | ACTIVO |
| 16 Kelly | `op_16_kelly.rs` | f*=(b·p−q)/b sobre features win/loss; scalar=f* | :56/:123; test :76, :88 | C(44p+75s=119), R (HighVolatility/LiquidationProximity/Depeg), V, A | ACTIVO |
| 17 Pontryagin | `op_17_pontryagin.rs` | PMP: hamiltoniano del sizing con costate λ; scalar=h* (control óptimo) | :58/:92 | C(1p+0s=1 — un solo cartucho, como primary), V, A | ACTIVO marginal |
| 18 Lagrangian | `op_18_lagrangian.rs` | L=T−V sobre retornos (T~varianza, V~mean_return); scalar=L | :58/:97 | V, A | HUEHUFO |
| 19 Simplex | `op_19_simplex.rs` | PL max cᵀx s.a. Ax≤b, x≥0 sobre asignación de capital entre assets; feature `max_capital` (default 1.0); scalar=valor objetivo | :141/:201 | C(55p+1s=56), V, A | ACTIVO |
| 24 Nash | `op_24_nash.rs` | equilibrio 2×2 puro y mixto de UNA MATRIZ DE PAYOFFS FIJA (no derivada del estado: `_state` sin usar, payoff hardcoded [2,0,3,1]/[2,3,0,1] en :75-76); scalar=p del mixto | :74/:97 | C(14p+9s=23), V, A | ACTIVO-técnico / GAP semántico (ver §6-G4) |
| 32 MultiObjective (HP-03) | `op_32_multi_objective.rs` | escalarización ponderada (features `mo_weight_yield/risk/latency` :566-568) sobre pools; scalar=net yield | :549/:736; test :1011, :1017 | NADIE — compilable, NO registrado en el registry (mod.rs:43-46) | HUEHUFO (capacidad de referencia, congelada por decisión canónica 2026-09-11) |

### Operations / finance / game / ml

| op | ruta | transformación → salida | prueba | deps | destino |
|---|---|---|---|---|---|
| 23 Queueing | `op_23_queueing.rs` | M/G/1: L=λW, espera media W_q con features de llegada/servicio (defaults); scalar=w_q | :55/:135 | C(64p+50s=114), V, A | ACTIVO |
| 26 Flash Loan (TLS) | `op_26_flash_loan.rs` | óptimo TLS CPMM: principal óptimo x* de superposición temporal de liquidez; features `pool_fee`(0.003), `flash_premium`, `gas_units`, `token0_per_eth` | :41/:119 | C(7p+26s=33), V, A | ACTIVO |
| 27 Path Ordering | `op_27_path_ordering.rs` | orden buy-low/sell-high del mismo asset entre venues; scalar=spread (max−min)/min; vector=[idx_min, idx_max] | :37/:111 | C(53p+18s=71), V, A | ACTIVO |
| 28 JIT Liquidity | `op_28_jit_liquidity.rs` | decaimiento efímero L(t)=L_target·e^{−kt}; ajusta k desde feature `jit_decay_rate`; scalar=k | :39/:119 | V, A | HUEHUFO |
| 29 Shapley | `op_29_shapley.rs` | juego cooperativo venues=como-jugadores sobre precios p_i; scalar=φ_max (contribución marginal máxima) | :42/:160 | C(16p+0s=16), V, A | ACTIVO |
| 30 GNN Encoder | `op_30_gnn_encoder.rs` | encoder tipo graph-attention sobre el grafo de venues (hidden_dim fijo); scalar=‖embedding‖ medio | :83/:127 | C(0p+27s=27), V, A | ACTIVO |
| 31 DRL Agent | `op_31_drl_agent.rs` | gate honesto: `labeled_trajectories_available(_state) -> 0` SIEMPRE (:36-38) → `reason_untrained_policy` / `reason_model_not_loaded`, scalar=None para siempre (path futuro V_θ(s_t) comentado :86-92) | :61/:93-94 | V, A | HUEHUFO por diseño (fail-honest permanente hasta que exista pipeline de entrenamiento) |
| 32 NSGA-II (canónico) | `op_32_nsga2/mod.rs` + `core.rs` | (a) wrapper trait: lee candidates del feature-map `nsga2.{i}.net_profit_usd / risk_cvar_usd / latency_ms` (:77-107), selección-only, scalar=tamaño del frente Pareto. (b) `core.rs` typed API: `Nsga2Optimizer::new(Nsga2Config)` (:198), `.select(&[Objectives]) -> Selection` (:234), `.evolve<T>()` (:301) | mod.rs:77; core.rs:22 (`Objectives`), :198, :234, :301; test real_ops_tests.rs:460 | searcher `live_risk_ranker.rs` (core directo), api.rs /api/compute (wrapper) | ACTIVO — único op con efecto DECISORIO runtime (ver §4) |

Prueba transversal: `all_31_operators_dispatch_and_are_fail_honest` (real_ops_tests.rs:416)
y `all_32_operators_dispatch_and_are_fail_honest` (:519) — despachan todos los IDs y
verifican la honestidad R8 (None + reason, jamás fabricado).

## 3. Registro en searcher-rs — dónde se despachan y quiénes los usan

CANONICAL_REPO. NO existe `backend/searcher-rs/src/operators/` (verificado con ls; el
claim de directorio del charter es GAP estructural: los operadores viven SOLO en
math-engine; searcher los consume vía crate `math_engine`).

Puntos de construcción y consumo (grafo inverso):

1. **Construcción del registry** — `scanner.rs:566` `math_registry: Arc::new(math_engine::OperatorRegistry::new())` (+ `regime_router: RegimeRouter::default()` :567); guardado en `orchestrator.rs:138-140` (`pub math_registry`, `pub regime_router`).
2. **Vía régimen (Fix B, observe-only)** — `orchestrator.rs:451-466` spawnea `math_evidence::evaluate_math_evidence` con los pools del intent; `math_evidence.rs:251-360`: construye `MarketState` desde `ReservesCache` (precio = r1/r0, :26-41), `router.route(&state)` → despacha los op_ids del régimen, persiste snapshot en Redis `arbx:math_evidence:{chain}:{strategy}` TTL 120s (:347-348). **Degradación documentada**: gas=0.0, block=0 (:459-461) porque el RouteIntent no los carga aún — los 7 ops que leen `gas_price_gwei` (11,12,14,15,21,26,32-multi) computan con fricción termodinámica cero (INFERRED de grep).
3. **Vía combo declarado por estrategia (STRAT-IDENT-01)** — `cartridge_boot.rs:1003-1020` toma `meta.primary_operators/secondary_operators` de cada cartucho ACTIVO pertinente; `cartridge_boot.rs:1214-1248` spawnea `math_evidence::publish_declared_combo_evidence` (definido en `math_evidence.rs:178-248`, evalúa con `evaluate_strategy_operators` :95-107, publica a `strategy_evidence_key` :158-160). Los IDs llegan del Rhai: `cartridge/runner.rs:538-553` parsea `primary_operators`/`secondary_operators` del mapa de `init_strategy()` a `CartridgeMetadata` (`cartridge/types.rs:25-30`). Cada `.rhai` de los 264 declara su combo (ejemplo `mev_01_001_dex_dex_arbitrage.rhai:33-34`: primary [27,21,15,16], secondary [1,22,26,30]).

**Contraste canónico math_map.json ↔ .rhai** — // WO-02b-FIX-MATHMAP (2026-09-17,
fixer gang ronda 1): la derivación original de esta ficha (conteos por grep de los 264
`.rhai`) quedó sin respaldo canónico explícito — el punto de acople nombrado por
WO-02a-DESIGN.md ("math_map.json (op_origen canónico, no re-derivar)", board línea 44-45
y WO-02a-DESIGN.md:246-249,468) NO fue consultado entonces. Excusa temporal declarada en
§0: WO-02a se publicó después del inicio de este WO. Contraste ejecutado y reproducible
(parser regex de `primary_operators` sobre los 264 `.rhai` vs `primary_ops` del manifiesto
`backend/searcher-rs/cartridges/manifests/math_map.json`, emparejados por `mev_id`):
**264/264 concordantes — 0 mismatches, 0 diferencias de orden, biyección mev_id limpia
(0 huérfanos por lado; manifiesto sin mev_id duplicados)**. Reportado primero por el
cross-examiner del gang (2026-09-17) y re-verificado de forma independiente por este fixer
(recomputado, no heredado — RULE 00; comando reproducible en
`WO-02b-FIX-MATHMAP-20260917.md` §2). Consecuencia: los conteos split p/s del §2 quedan
con respaldo canónico DOBLE (.rhai + manifiesto); NO existía contradicción silenciosa.
Nota de alcance: el manifiesto solo contrasta `primary_ops` — `secondary_operators` no
existe en math_map.json, por lo que la mitad "s" de los conteos §2 sigue respaldada
ÚNICAMENTE por los .rhai (CANONICAL_REPO, fail-honest).
4. **Vía vector de evidencia §IV (puras)** — `math_evidence.rs:373-384` `build_evidence_vector` despacha 1..=31 (excluye op_32 por diseño 264×31) y `evidence_posterior_log_odds` :393-415 (calibrado hoy = flat_prior: cableado OFF honesto). Tests :423-463.
5. **op_32 NSGA-II — el ÚNICO con efecto decisorio** — `live_risk_ranker.rs:3` importa el core tipado; `LiveRiskRanker::rank` (:59-112) lee cohorts de receipts finalizados de Redis (LRANGE history_key, ≤128 observaciones), calcula `empirical_risk` (CVaR95 + latency p95), arma `Objectives` y reordena el batch sized con `Nsga2Optimizer::select` (:221-233) — NSGA-II con generations=0 (selección de frente Pareto, sin evolución). Consumidor: `orchestrator.rs:1020-1035` (evento `orchestrator.empirical_nsga2` :1037). Reglas anti-fábrica: entries sin historia conservan orden Net_bps y NUNCA implican riesgo cero (:1-2, tests :287-296); permutación sin pérdida de payloads (:236-246).
6. **Consumidor tangencial (fuera de los 32 ops)** — `prioritization-spine/src/config_aware.rs:13-14,32-37` usa los módulos LEGACY de math-engine (`roi_engine`, `risk_engine`, `amm_math`), no el registry de operadores. Frontera documentada para no duplicar el WO de 02c.

## 4. Mutabilidad y pureza

- Los 32 operadores: PUROS (transformación sin I/O ni estado). CANONICAL_REPO (trait mod.rs:97-114, sin `&mut`).
- I/O sólo en la periferia: `math_evidence` (logs + Redis SETEX), `live_risk_ranker` (Redis LRANGE en spawn con timeout 50ms :160), `api.rs` (RwLock de toggles).
- `op_32_nsga2::core::evolve` existe como API tipada pero ningún consumidor runtime lo invoca con offspring reales (sólo `select`). INFERRED.

## 5. Endpoint de toggle — riesgo documentado (memoria 2026-09-16, sin tocar nada)

CANONICAL_REPO:
- `api.rs:212` ruta `POST /api/operators/:id/toggle`; handler `toggle_operator_handler` :259-286; `set_enabled` :69-76 es SOFT (HashSet de deshabilitados; el op permanece en el registry, `/api/compute` lo salta y lo lista en `skipped` :296-306).
- SIN AUTENTICACIÓN: ningún middleware/auth en `create_router` :207-218; `ToggleRequest` es sólo `{"enabled":bool}` :143-147.
- Bind del binario: `main.rs:29` `SocketAddr::from(([0,0,0,0], port))` (default 3006, env `MATH_ENGINE_PORT` :25-28).
- Exposición real en el VPS: `docker/compose.prod.yml:196` `- 127.0.0.1:3006:3006` — el publish está restringido a loopback del HOST. La memoria "en 127.0.0.1" es correcta para el host, pero el binario en sí escucha 0.0.0.0 dentro del container: cualquier contembro de la red docker interna (o un `-p` descuidado futuro) expone el toggle sin auth.
- RIESGO (clasificado): un POST anónimo puede deshabilitar/forzar operadores del SERVICIO math-engine HTTP. NOTA de alcance: el searcher NO consulta ese servicio HTTP para decidir (usa el crate in-proceso `OperatorRegistry::new()`); el toggle sólo afecta a clientes HTTP de `/api/compute`. Hoy el impacto runtime del toggle sobre el hot-path de searcher es NULO (INFERRED — no hay llamada HTTP de searcher a math-engine; grep `MATH_ENGINE`/url en searcher sin matches). El riesgo es de superficie, no de hot-path.
- **DRIFT DOCUMENTAL (comentario del compose) — // WO-02b (2026-09-17, fixer gang ronda 1)**: el comentario citado arriba dice "Serves the 31 topological operators (list/toggle/compute/264×31 matrix projection)" (`docker/compose.prod.yml:197-198`), pero el registry que sirve `/api/operators` · `/api/operators/:id/toggle` · `/api/compute` registra **32** operadores (ids 1..=32, op_32_nsga2 en `operators/mod.rs:172`; ver §1 y DESIGN §4.1). El "31" es correcto SOLO para la matriz de proyección canónica (`/api/matrix/projection`, `COLS=31` congelado por test `api.rs:445` + `matrix/topology_map.rs`) — el comentario mezcla ambos conteos. Origen del drift: comentario escrito con 31 ops (commit `c7ad837e`, 2026-07-28 "run the 31 topological operators as a service"); op_32_nsga2 registrado después (`95c69d47`, 2026-09-11) sin actualizar el comentario. Impacto runtime: CERO (es un comentario YAML, no afecta scheduling). Corrección del YAML SOLO vía PR gated del operador — NO aplicada aquí (ver errata §11 y reporte `WO-02b-FIX-DRIFT-COMPOSE-20260917.md`).

## 6. GAPs (fail-honest, RULE 00)

- **G1 objetivo_usd**: NO existe fuente real de objetivo_usd por operador (config/env/registry). El único flujo USD real hacia un operador es `RankingInput.principal_usd/expected_profit_usd` → Objectives de op_32 (live_rith_ranker.rs:21-22, orchestrator.rs:1031-1032) — valores OBSERVADOS por oportunidad, no objetivos. Todo "objetivo_usd por op" = GAP.
- **G2 searcher-rs/src/operators/**: directorio inexistente. El claim del charter no mapea a disco.
- **G3 features vacías**: ambas vías de evidencia llaman con `features` vacías (orchestrator.rs:462 `HashMap::new()`; math_evidence.rs:192-201 "carries none today"). Los ops que requieren features (11 bayes_wins/losses, 23 parámetros de cola, 26 flash_premium/gas_units, 28 jit_decay_rate, 17, 19 max_capital) computan None/default honestamente en producción. Su "ACTIVO" es de wiring; su señal real es mayormente cero hoy. INFERRED.
- **G4 op_24 Nash**: evaluate resuelve un juego 2×2 con payoff HARDCODED (:75-76), ignora el MarketState. Es matemática honesta sobre un juego fijo — pero su output no depende del mercado. Riesgo de interpretación (parece evidencia de mercado y no lo es). HYPOTHESIS sobre intención (demo/fixture); el hecho del hardcode es CANONICAL_REPO.
- **G5 HUEHUFO puros**: op_04, op_09, op_12, op_18, op_28 no son declarados por ningún cartucho ni recomendados por ningún régimen — sólo alcanzables vía `/api/compute` genérico o el vector §IV. op_31 permanentemente cerrado por gate. op_02/op_03 sólo viven por el RegimeRouter.
- **G6 gas=0**: la vía régimen inyecta gas_price_gwei=0.0 (orchestrator.rs:459) — 7 ops computan con fricción cero. La vía combo declara base_fee real (cartridge_boot.rs:1229-1233).

## 7. Destino preliminar (síntesis)

- ACTIVO con señal runtime real: **op_32 nsga2** (único decisorio: reordena batches en orchestrator) y, en capa observe-only, los ops con reservas reales suficientes (todas las vías construyen MarketState desde ReservesCache real).
- ACTIVO de wiring (evidencia observe-only, señal parcial por features vacías/gas cero): 01,02,03,05,06,07,08,10,11,13,14,15,16,17,19,20,21,22,23,25,26,27,29,30.
- HUEHUFO: 04, 09, 12, 18, 28, 31 (sin consumidor), 32_multi_objective (no registrado, congelado por doctrina).
- GAP: objetivo_usd por operador (G1); semántica de mercado de op_24 (G4).

## VERIFICACIÓN (WO-02b-verify · math-validator · 2026-09-17)

> Dictamen independiente por lectura de código (Read/Grep; cero cargo/npm/build; cero git; cero VPS).
> Cada ítem: PASS / CORRECCIÓN. Evidencia file:line re-verificada por el verificador, no heredada.

### (1) Censo exacto — PASS

- Disco: `backend/math-engine/src/operators/` contiene `mod.rs` + `op_01..op_31` (31 archivos .rs)
  + `op_32_multi_objective.rs` + `op_32_nsga2/` (directorio: mod.rs + core.rs) + `real_ops_tests.rs`.
- Registry: `register!` en mod.rs:139-173 registra EXACTAMENTE ids 1..=32; id 32 =
  `op_32_nsga2::Nsga2Operator::new()` (mod.rs:172). `OPERATOR_COUNT: u8 = 32` (mod.rs:57).
- **Cero fantasmas, cero faltantes**: `op_32_multi_objective` (HP-03) está declarado como módulo
  (mod.rs:47) pero NO registrado — la ficha del diseñador lo documenta explícitamente como
  "compilable, NO registrado" (línea 97 de la ficha), con la nota doctrinal mod.rs:43-46. No es un
  operador fantasma: es capacidad congelada correctamente clasificada.
- Test de identidad: `registry_preserves_existing_ids_and_adds_nsga2` (op_32_nsga2/mod.rs:198-207)
  congela ids = (1..=32) exactos.
- Respuesta al charter: son **32** registrados (31 históricos + NSGA-II canónico). La proyección
  matemática sigue siendo 264×31 (matrix/topology_map.rs:12-14, `COLS=31`), asimetría congelada por
  test api.rs:437-446 — la lectura INFERRED del diseñador (op_32 fuera de la matriz por diseño) es
  correcta.

### (2) Muestreo profundo (13 operadores + core NSGA-II) — PASS en todos

| op | dictamen | evidencia verificada |
|---|---|---|
| 16 Kelly | **PASS** — implementa Kelly real | f*=(b·p−q)/b en op_16_kelly.rs:107, clamp [0,1] :108, b=avg_win/avg_loss :102, fail-honest None+reason :64-77/:87-101. Tests reales: `kelly_computes_on_mixed_state` (real_ops_tests.rs:76), `kelly_none_when_no_losses` (:88) |
| 32 NSGA-II | **PASS** — NSGA-II genuinamente multiobjetivo (3 objetivos: profit max, CVaR min, latency min) | core.rs: fast non-dominated sort O(n²) (:240-277), dominancia estricta con desempate (:46-53), crowding distance normalizada por dimensión con manejo de rango constante e infinito (:396-437), selección elitista por fronteras + crowding (:278-288), `evolve` con torneos binarios + validación/evaluación de cada hijo (:301-369), SplitMix64 determinista (:475-490). Oracle-test de grid completo vs front-peeling independiente (:639-670). Wrapper selección-only: scalar = tamaño del frente Pareto (mod.rs:136), features wire `nsga2.{i}.*` (:78-107) |
| 24 Nash | **PASS** — hardcode confirmado | `_state` sin usar, payoffs fijos [2,0,3,1]/[2,3,0,1] en op_24_nash.rs:75-76, scalar=row_mixed_p :97. El GAP G4 del diseñador es exacto |
| 31 DRL | **PASS** — gate fail-honest permanente | `labeled_trajectories_available -> 0` SIEMPRE (op_31_drl_agent.rs:36-38), MIN_TRAJECTORIES=200 :24, reason_untrained_policy :71 / reason_model_not_loaded :94, scalar None |
| 15 Golden-Section | **PASS** | f(x)=r1·γ·x/(r0+γ·x)−x−gas (op_15_golden_section.rs:104-110), τ=(√5−1)/2 :119, maximización golden-section clásica sobre [0,r0] :128-135 |
| 21 Newton | **PASS** | Newton-Raphson x_{n+1}=x−f/f' (op_21_newton.rs:167-172), deriv_floor 1e-12 :157, fail-honest deriv_singular/divergence/non_converged |
| 02 PCA | **PASS** | covarianza muestral → autovalores, scalar ρ₁=λ_max/Tr (op_02_pca.rs:37-38). Tests :151/:169 ✓ |
| 11 Bayes | **PASS** | Beta-Binomial con features bayes_wins/losses + priors alpha/beta default 1 (op_11_bayes.rs:35-46). GAP G3 (features vacías runtime) coherente |
| 13 Regression | **PASS** | OLS con X=[1,t] (tendencia, no nivel), nalgebra (op_13_regression.rs:63-70). Tests :98/:111 ✓ |
| 26 Flash Loan (TLS) | **PASS** | CPMM γ, premium φ, gas, precio de referencia cross-venue (op_26_flash_loan.rs:41-75) |
| 27 Path Ordering | **PASS** | argmin/argmax sobre precios asset-0 por venue, fail-honest <2 venues (op_27_path_ordering.rs:37-70) |
| 29 Shapley | **PASS** | venues=jugadores, suma exacta capada n≤8=256 coaliciones (op_29_shapley.rs:42-79) |
| 05/08/22 (vía tests) | **PASS** | tests monte_carlo_* :45/:58/:66, `kalman_computes_mispricing_zscore` :218, contratos fail-honest |

- Pureza: grep de interior mutability (`RefCell|Mutex|RwLock|Atomic|Cell<`) en TODO el directorio
  operators/ = **0 matches**. Los 32 son puros. PASS.
- Transversal: `all_31_operators_dispatch_and_are_fail_honest` (real_ops_tests.rs:416) y
  `all_32_operators_dispatch_and_are_fail_honest` (:519) — verificados, ejercitan 1..=31 / 1..=32
  con contrato Some⇒finito. PASS.

### (3) op_origen / coherencia registry ↔ searcher-rs — PASS

- Construcción: scanner.rs:565-567 `math_registry: Arc::new(math_engine::OperatorRegistry::new())`
  + `regime_router` ✓ (precisión: línea exacta 565-566; la ficha citó :566 — dentro de tolerancia).
- Vía régimen: orchestrator.rs:449-466 — spawn de `evaluate_math_evidence` con **gas=0.0,
  block=0,0, features vacías** confirmado literal (:459-461, comentario "not carried in RouteIntent
  yet (observe-only)"). GAP G6 exacto.
- Vía combo: cartridge_boot.rs:1214-1248 — spawn de `publish_declared_combo_evidence` con
  **base_fee_gwei real** de handles atómicos del runner (:1229-1233) ✓. Origen de IDs:
  cartridge/runner.rs:538-553 parsea `primary_operators`/`secondary_operators` del Rhai ✓.
- Vector §IV: math_evidence.rs:372-385 `build_evidence_vector` despacha 1..=31 (Vec largo 31) ✓;
  posterior flat_prior honesto :387+.
- op_32 decisorio: live_risk_ranker.rs:3 importa el CORE tipado; `Nsga2Optimizer` con
  `generations: 0` (selección only, :221-225), `MAX_RANKED=128` :13; consumidor
  orchestrator.rs:1021-1036 (`orchestrator.empirical_nsga2`) ✓. Header anti-fábrica :1-2
  ("Unknown history retains Net_bps order and never implies zero risk") ✓.
- RegimeRouter: math-engine/src/control/regime_router.rs:214-220 — recomendaciones exactas
  HighVolatility→[22,16,1], ArbitrageGap→[1,13,2], LiquidationProximity→[16,22,25],
  OracleBias→[13,14,3], DepegDeviation→[16,22,13], Neutral→[10,13] ✓ (precisión: el archivo vive
  en `math-engine/src/control/`, la ficha citó "regime_router.rs:215-220" sin ruta completa).
- knowledge_graph.jsonl: **2,693 líneas** verificadas con wc -l — la corrección del charter (2,511)
  es correcta.
- `backend/searcher-rs/src/operators/` NO existe (ls exit 2) — GAP G2 del diseñador confirmado.

### (4) RULE 00 / objetivo_usd — PASS (GAP correctamente exigido)

- Grep repo-wide de `objetivo_usd|objective_usd|target_usd` en backend/ (*.rs, *.ts, *.toml,
  *.json): UN solo match = comentario de sizing floor en
  api-server/src/simulation/computeSimulatedNet.ts:96 ("usd-floor" en simulación de papel) — NO es
  una fuente de objetivo por operador. G1 del diseñador (objetivo_usd = GAP) es correcto y
  obligatorio bajo RULE 00.
- Anti-fábrica transversal verificada: op_32 core rechaza NaN/Inf/negativos (core.rs:29-40, test
  :575-593); op_31 se niega a inventar V(s_t); op_16 se niega a inventar f*. Ningún muestreo
  encontró dato fabricado.

### Puntos de riesgo (§5 de la ficha) — confirmados

- Toggle sin auth: confirmado — ruta `POST /api/operators/:id/toggle` api.rs:212, handler :259-286
  SIN header de auth, `set_enabled` soft HashSet :69-76; bind 0.0.0.0 main.rs:29. La precisión del
  diseñador (binario 0.0.0.0 vs compose loopback) es correcta y el matiz es valioso.
- Corrección menor (NO CRITICAL): WO-02b-DESIGN.md G1 cita "live_r**ith**_ranker.rs:21-22" — typo
  por `live_risk_ranker.rs` (RankingInput está en :20-25, verificado). El contenido de la cita es
  correcto.

### Tally final

| Ítem de verificación | Resultado |
|---|---|
| Censo 32/32, 0 fantasmas, 0 faltantes | **PASS** |
| Muestreo 13 ops + core NSGA-II: ficha ↔ código | **13/13 PASS** (0 correcciones de fondo; 2 precisiones de ruta/línea menores) |
| op_origen ↔ registry ↔ consumo searcher-rs | **PASS** (5 vías verificadas con líneas exactas) |
| objetivo_usd sin fuente → GAP (RULE 00) | **PASS** (única coincidencia repo = sizing-floor de simulación) |
| Pureza (muta/puro) | **PASS** (0 interior mutability en operators/) |
| Operador fantasma (criterio CRITICAL del charter) | **0 encontrados** |

**VEREDICTO: PASS.** La ficha 02b-OPERADORES-MATH-ENGINE.md es fiel al código en censo, matemática,
pureza, wiring y GAPs. Clasificación CANONICAL_REPO de las citas: sostenida. Los hallazgos G1-G6 y
la síntesis de destino (§7) quedan VALIDADOS. Navegación del dominio público: NO APLICA
(verificación resuelta íntegramente con evidencia de código local; 0 de 5 requests usados,
fail-honest).

### Adendum de alcance — // WO-02b-FIX-VERIFY (2026-09-17, fixer gang ronda 1)

> Append-only; no altera el dictamen del verificador, acota su alcance. Cruzado por el
> cross-examiner (ronda 1) y re-verificado de forma independiente por este fixer (recomputado,
> no heredado — RULE 00). Reporte: `WO-02b-FIX-VERIFY-20260917.md`.

El muestreo profundo de este adendum verificó matemática y líneas, pero **NO re-derivó los
conteos de la columna deps de §2 ni contrastó el manifiesto canónico**. Dos precisiones que
quedaron fuera del alcance declarado del "13/13 PASS (0 correcciones de fondo)":

1. **Celda op_22 de §2 contenía un claim falso no detectado** ("TODOS los cartuchos lo declaran
   secondary"). Recómputo independiente (parser regex `primary_operators`/`secondary_operators`
   sobre los 264 `.rhai`, set-dedup por archivo): op_22 = **16 PRIMARY + 248 secondary = 264**;
   overlap p∩s = 0 en todos los ops; los 23 conteos de unión originales sí eran correctos
   (defecto exclusivo de atribución p/s). Corregido in situ + ERRATA §8 por
   `WO-02b-FIX-20260917.md`. La distinción p/s es load-bearing (STRAT-IDENT-01,
   cartridge_boot.rs:1003-1020).
2. **El contraste con `manifests/math_map.json` (op_origen canónico, punto de acople de
   WO-02a-DESIGN.md:246-249) estaba ausente** — §(3) verificó registry↔searcher-rs pero nunca
   consultó el manifiesto. Cerrado por `WO-02b-FIX-MATHMAP-20260917.md`: **264/264
   concordantes** primary_ops↔primary_operators (0 mismatches, 0 order-diffs, biyección mev_id
   limpia). Re-ejecutado por este fixer con `reconcile_math_map_vs_rhai.py` = mismo resultado.
   Nota de alcance persistente: el manifiesto NO tiene campo secondary → la mitad "s" de §2
   sigue respaldada SOLO por los .rhai.

**Efecto neto sobre el veredicto**: PASS sobrevive para lo efectivamente muestreado (censo,
matemática, pureza, wiring, GAPs). La fila del tally "Muestreo 13 ops + core: 13/13 PASS
(0 correcciones de fondo)" debe leerse **acotada a la metodología del muestreo** — la columna
deps de §2 no estaba en su muestra y en ella vivía el único claim falso de la ficha.

## 8. ERRATA §2 — claim op_22 "TODOS secondary" + notación deps inconsistente — // WO-02b-FIX (2026-09-17, fixer gang ronda 1)

> Append-only. El texto original erróneo se preserva aquí como procedencia; las celdas de
> §2 fueron corregidas in situ. Reporte del fix: `WO-02b-FIX-20260917.md`.

**Defecto (señalado por cross-examiner):**
1. Celda op_22 decía `C(264 — TODOS los cartuchos lo declaran secondary)`. FALSO para 16
   cartuchos: op_22 es declarado PRIMARY por 16 y secondary por 248 (unión 264 correcta;
   el calificativo "secondary" era el error). La distinción primary/secondary es
   semánticamente load-bearing: STRAT-IDENT-01 conserva el combo declarado
   (cartridge_boot.rs:1003-1020; cartridge/runner.rs:538-553).
2. Columna deps con convención mezclada: op_07 `C(28)` (solo primary) contradecía el
   conteo de §2 `op_07:30` (unión); op_05 `C(22+24)` era ambiguo (leía como 22p+24s=46,
   real 22p+2s=24).

**Corrección aplicada:** convención armonizada `C(Xp+Ys=Z)` en TODA la columna deps y en
el párrafo de conteos de §2. Valores recomputados de forma independiente (RULE 00, no
heredados del hint) con parser regex por archivo sobre los 264 `.rhai` de
`backend/searcher-rs/cartridges/strategies/`: op_01 0p/27s · op_05 22p/2s · op_06 30p/0s ·
op_07 28p/2s · op_08 131p/47s · op_10 45p/36s · op_11 36p/136s · op_13 71p/88s · op_14 4p/10s ·
op_15 54p/11s · op_16 44p/75s · op_17 1p/0s · op_19 55p/1s · op_20 4p/18s · op_21 176p/2s ·
op_22 16p/248s · op_23 64p/50s · op_24 14p/9s · op_25 12p/0s · op_26 7p/26s · op_27 53p/18s ·
op_29 16p/0s · op_30 0p/27s. Overlap primary∩secondary = 0 en todos los ops (unión = p+s,
dedup sin pérdida). Los conteos de unión originales de §2 resultaron todos correctos; el
defecto era exclusivamente de atribución p/s.

**Nota colateral verificada:** la fila op_01 "ACTIVO (sólo secondary)" era y sigue siendo
CIERTA (0 primary, 27 secondary — precisada in situ). Contaminación cruzada del claim
falso: 0 archivos ANTES del fix (grep "TODOS los cartuchos lo declaran secondary" sobre el
dir de auditoría pre-edición: único match = la propia celda op_22 de este archivo, línea 73
original; INFORME.md:22 cita
`applicable_operators` de manifests frontend — campo distinto, otro WO, no tocado).

## 9. ADENDA — acople op_origen con math_map.json (fuente canónica) — // WO-02b-FIX2 (2026-09-17, fixer gang ronda 1)

> Append-only. Reporte del fix: `WO-02b-FIX2-20260917.md`. Script reproducible:
> `audits/first-understand-20260917/reconcile_math_map_vs_rhai.py`.

**Defecto de acople (señalado por cross-examiner, ronda 1):** WO-02a-DESIGN.md designa
explícitamente a `backend/searcher-rs/cartridges/manifests/math_map.json` como la fuente
**op_origen canónica** para este WO ("Este es el op_origen de partida para WO-02b — NO
re-derivar", §6.4 / :246-250; refrendado en :468 "WO-02b debe fichar los 32 ops contra
math_map.json SIN re-derivar" y :475). Este entregable midió los combos directamente de los
264 `.rhai` (§2 + ERRATA §8) SIN citar math_map.json: el diseñador corría en oleada
paralela y declaró en §0 que 02a no existía aún al iniciar (fail-honest, excusable). El
verificador (WO-02b-verify-VERIFY.md:11-14) SÍ leyó el punto de acople de 02a y no ejecutó
la instrucción de acople — sincronía de mesa incompleta. Este adendum cierra ese hueco.

**Reconciliación ejecutada (RULE 00 — medida por este fixer, no heredada del hint):**
para cada una de las 264 entradas de math_map.json se parseó `mev_id` + `primary_ops`
(strings `op_NN`) y se comparó como conjunto contra `primary_operators` del `.rhai` con el
mismo `mev_id`:

- math_map.json: 264 entradas, 264 mev_id únicos (0 duplicados).
- `.rhai`: 264 archivos, 264 mev_id únicos, 0 no-parseables.
- Intersección de mev_id: 264/264 — 0 solo-manifest, 0 solo-rhai.
- **`primary_ops` == `primary_operators` (como conjunto) en 264/264 entradas — 0 mismatches.**
- Verificación por op: conteos primary derivados de math_map.json idénticos a la columna
  "p" del ERRATA §8 (0 diffs): 21 ops con primary>0 (05:22, 06:30, 07:28, 08:131, 10:45,
  11:36, 13:71, 14:4, 15:54, 16:44, 17:1, 19:55, 20:4, 21:176, 22:16, 23:64, 24:14, 25:12,
  26:7, 27:53, 29:16); op_01 y op_30 con 0 primary; los 9 ops nunca declarados
  (02,03,04,09,12,18,28,31,32) tampoco aparecen como primary en math_map.json.
- **Conclusión: cero daño factual** — la derivación desde `.rhai` era equivalente a la
  fuente canónica; el defecto era de procedencia/auditabilidad, no de valores.

**Alcance de la fuente canónica (fail-honest):** math_map.json SOLO lleva `primary_ops`
(no hay lista secondary) — el split primary/secondary completo del ERRATA §8 sigue teniendo
al `.rhai` como fuente única para el lado secondary, complementaria no reemplazada. Dato
colateral observado: `mode` en math_map.json = 160 SHADOW + 104 PAPER (0 ACTIVE).

**Convergencia dual (paralelismo de mesa, NO duplicación):** este adendum fue redactado en
paralelo con `// WO-02b-FIX-MATHMAP (2026-09-17, fixer gang ronda 1)` — bloque in situ en
§3 tras el item 3 (reporte `WO-02b-FIX-MATHMAP-20260917.md`, entrada de board :118).
Ambos fixers recomputaron INDEPENDIENTEMENTE la misma reconciliación con el mismo
resultado: 264/264 concordantes, 0 mismatches, biyección mev_id limpia, manifiesto sin
duplicados ni primary_ops vacíos. Este fixer añadió dos verificaciones que el bloque §3 no
declara: (a) igualdad de VALOR Y ORDEN (0 order-diffs), y (b) igualdad per-op de los
conteos primary del manifiesto contra la columna "p" del ERRATA §8 (0 diffs, 21 ops con
primary>0). El 264/264 queda DOBLE-VERIFICADO (precedente CONVERGENCIA DUAL del board).
Cadena canónica de op_origen resultante: manifest `math_map.json` (generado por
`scripts/gen_math_manifest.py`, WO-02a-DESIGN.md §6.4) → cartucho `.rhai`
(`primary_operators`/`secondary_operators`, parseadas en cartridge/runner.rs:538-553) →
registry Rust (`operators/mod.rs:139-173`) → consumo runtime (§3 de este documento).
Script reproducible persistido: `audits/first-understand-20260917/reconcile_math_map_vs_rhai.py`.

## 10. ERRATA — ficha op_15 "[0, r_in]" vs código "[0, r0]": notación unificada y divergencia del verificador flaggeada — // WO-02b-FIX-N1 (2026-09-17, fixer gang ronda 1 · respawn-A)

> Append-only. El texto original erróneo se preserva aquí como procedencia; la celda de
> §2 fue corregida in situ. Reporte del fix: `WO-02b-FIX-N1-20260917.md`.

**Defecto (señalado por cross-examiner):**
1. La ficha del diseñador (fila op_15 de la tabla Optimization, línea 100 del archivo)
   decía "golden-section sobre **[0, r_in]**", mientras el código y la fila del verificador
   (sección VERIFICACIÓN, fila op_15: "maximización golden-section clásica sobre
   **[0,r0]**") usan r0. El símbolo `r_in` NO EXISTE en ninguna fuente del repo:
   `grep -n "r_in" backend/math-engine/src/operators/op_15_golden_section.rs` = 0 matches
   (y en TODO backend/math-engine solo falsos positivos de substring tipo `pair_index`,
   `for_inclusion`). Era notación huérfana del diseñador, no un alias declarado.
2. El verificador corrigió la notación **en silencio** — su fila PASS usa [0,r0] sin
   flaggear la divergencia contra la ficha que estaba validando. Eso oculta un posible
   desacuerdo semántico diseñador↔código (aquí resultó ser solo notacional, pero el
   protocolo exige declararlo, no absorberlo).

**Evidencia de que [0, r0] es lo canónico (re-verificada de fuente, RULE 00):**
- `op_15_golden_section.rs:85` — `let (r0, r1) = state.liquidity_reserves[0];` (r0 =
  reserva del token que se dimensiona = entrada del swap; f(x)=r1·γ·x/(r0+γ·x)−x−gas
  :104-110 es getAmountOut CPMM con x in-token contra reserves (r0 in, r1 out)).
- `:113-115` — `// Maximización por sección áurea sobre [a, b] = [0, r0].` con `b = r0`.
- `:120` — tolerancia escalada al bracket: `1e-9 * r0`.
- `:172` — `vector_result = [x_star, f_star, r0]` (exporta r0, no "r_in").
- `real_ops_tests.rs` (src/operators/) — test op_15: comentario del propio test
  "vector_result = [x*, f*, r0]; x* ∈ (0, r0]" y aserción `x_star > 0.0 && x_star <= 1_000_000.0`
  (pool_state con r0=1_000_000). Tests y metadata (:165 "r0") concuerdan.

**Corrección aplicada:** ficha §2 unificada in situ a
"[0, r0], r0 = reserva del token de entrada, `liquidity_reserves[0].0` :85, bracket
:113-115" con marcador `// WO-02b-FIX-N1 (2026-09-17)` apuntando a esta errata. Se eligió
unificar a [0, r0] (opción 1 del hint) en vez de declarar `r_in := r0` porque r0 es el
único símbolo con existencia en código, tests y metadata — declarar un alias habría
preservado la notación huérfana. Cero cambio semántico: el bracket siempre fue [0, r0].

**Alcance del cambio:** exclusivamente notacional/documental. No se tocó ningún .rs
(el código era y sigue siendo correcto). La fila PASS del verificador queda como estaba
(ya decía [0,r0]); esta errata es el flag faltante de la divergencia.

**Contaminación cruzada:** grep "r_in" sobre el dir de auditoría pre-edición = único
match la propia celda op_15 (los matches en 02d:188 `wait_for_inclusion`,
WO-02a-DESIGN.md:496 `pair_index`, WO-02a-verify-VERIFY.md:88 `pair_index` y
lines-per-file.txt `pair_index`/`market_maker_inventory` son falsos positivos de
substring, verificados uno a uno). Ningún otro WO propagó la notación r_in.

## 11. ERRATA — drift documental "31 operators" en compose.prod.yml no flaggeado por §5 — // WO-02b-FIX-DRIFT-COMPOSE (2026-09-17, fixer gang ronda 1)

**Gap (charter del fixer):** el §5 de esta ficha analiza `docker/compose.prod.yml:196`
(publish del puerto 3006) y NO flaggeó que el comentario inmediatamente posterior
(:197-198) dice "Serves the 31 topological operators (list/toggle/compute/264×31
matrix projection)" cuando el registry sirve 32. Omisión confirmada: §1, DESIGN §4.1
("OperatorRegistry registra EXACTAMENTE ids 1..=32") y la VERIFICACIÓN (1) censaron
32 ANTES de que §5 citara el archivo — el dato estaba en la propia ficha y el §5 no
cruzó los dos conteos.

**Evidencia (CANONICAL_REPO, reproducible):**
- Registry: `backend/math-engine/src/operators/mod.rs:139-172` — macro `register!`
  con ids 1..=32; `32 => op_32_nsga2::Nsga2Operator::new()` en :172.
- Servicio HTTP sobre ese registry: `api.rs:210-216` (`/api/operators`,
  `/api/operators/:id/toggle`, `/api/compute`, `/api/matrix/projection`,
  `/api/matrix/operators`); api.rs:57 construye `OperatorRegistry::new()`.
- El "31" que SÍ es correcto en el comentario es el de la matriz de proyección:
  `api.rs:445` `assert_eq!(COLS, 31)` (test) + `matrix/topology_map.rs::COLS` —
  matriz canónica 264×31 congelada, distinta del census del registry (32).
- Origen del drift (git): comentario introducido en `c7ad837e` (2026-07-28, "run the
  31 topological operators as a service") cuando el service tenía 31; op_32_nsga2
  registrado en `95c69d47` (2026-09-11) sin tocar el comentario.
  Recómputo: `git log -S 'Serves the 31 topological' -- docker/compose.prod.yml`.

**Clasificación:** drift DOCUMENTAL puro. Cero impacto runtime (comentario YAML no
afecta scheduling/healthcheck). No es mock ni dato fabricado (RULE 00 intacta) — es
un comentario desactualizado que induce a subestimar el census del service en 1 op.

**Corrección aplicada aquí (documental only):** bullet de drift agregado al §5
(arriba) con el marker `// WO-02b (2026-09-17)`.

**Corrección del YAML: NO aplicada — gated.** Propondría `s/31 topological
operators/32 topological operators/` en :197 y dejar "264×31" intacto (ese 31 es la
matriz canónica). Requiere PR gated del operador (§32/NO-GIT); nada fue commiteado.

**Contaminación cruzada:** grep -n "31" sobre los archivos que citan el compose
(WO-02b-DESIGN.md:58/:108, 02c-SIM-STACK-SELECTOR.md:142, INFORME.md:45,
WO-02c-DESIGN.md:22, WO-02c-verify-CROSS-EXAM.md:19): NINGUNO repite el claim "31
operators" del comentario — citan el publish :196 (loopback), SIM_BACKEND ausente y
:409 (dev-local). Contaminación = 0. La ficha WO-02c:142 usa "compose" para
sim-ctl, no para math-engine.

**Verificación post-edit:** grep -n "31 topological\|DRIFT DOCUMENTAL" en esta ficha
= bullet §5 + esta sección; `git status` del repo = solo los archivos de auditoría
tocados (cero .rs/.yml mutados). Cero git/cargo/VPS/HTTP (0/5 requests).

## 12. ADENDA — respuesta al cross-check de la mesa: mapa operador→cartucho vs `plan_validation::supports_strategy` — // WO-02b-FIX-TERMINUS (2026-09-17, fixer gang ronda 1 — GAP G-2 del cross-check)

> Cierra el GAP declarado por `WO-02d-verify-CROSS-EXAM.md` §4.3 ("02b cross-check
> sigue ABIERTO"): esta ficha no mencionaba el terminus ni
> `plan_validation::supports_strategy` (grep propio del cross-examiner: 0 hits —
> verificado de nuevo por este fixer antes de editar: 0 hits). La pregunta abierta de
> `WO-02d-DESIGN.md` §"Notas para la mesa" (:108-111) queda RESPUESTA aquí.

**Pregunta (02d:108-111):** ¿el whitelist de 11 strategy_kinds en
`relays-client/src/plan_validation.rs:17-32` es el cuello de botella que descarta los
demás cartuchos, o el mapa operador→cartucho de 02b (§2/§3) dice lo contrario?

**Respuesta: el whitelist ES el cuello de botella (terminal). El mapa de 02b NO dice
lo contrario — es compatible y lo confirma.** Evidencia (CANONICAL_REPO, re-derivada
por este fixer con parser regex sobre los 264 `.rhai` + grep, RULE 00):

1. **Upstream NO filtra por kind antes del terminus.** La identidad
   `strategy_kind == stem del .rhai` (`shared-rs/src/contracts.rs:34` "the `.rhai`
   filename stem (a canonical strategy_kind)"; emisión con
   `StrategyKind::cartridge(cartridge_id)` en `cartridge_boot.rs:1179`). El único
   filtro de kind aguas arriba es `sim-ctl/src/tx_builder.rs:144-147`
   (`is_non_swap_strategy_kind` = solo rechaza `liquidation`/`liquidation_snipe`,
   con comentario de que es para evitar errores de router engañosos) — NO existe
   whitelist de 11 antes del terminus.
2. **El whitelist admite exactamente 8 stems de cartucho de los 264** (3.0%):
   `mev_01_001_dex_dex_arbitrage`, `mev_01_002_cross_pool_arbitrage`,
   `mev_01_008_amm_amm_arbitrage`, `mev_01_015_two_leg_arbitrage`,
   `mev_01_016_triangular_arbitrage`, `mev_01_017_quadrangular_arbitrage`,
   `mev_01_018_n_leg_cyclic_arbitrage`, `mev_01_019_multi_hop_arbitrage`
   (plan_validation.rs:23-30). Verificado: `whitelist ∩ stems = 8`;
   **256/264 cartuchos (97.0%) quedan estructuralmente descartados en el terminus**.
   PRECISIÓN aritmética vs el GAP tal como fue formulado: el "253" del charter
   (264−11) trata los 3 kinds genéricos como cartuchos — los kinds
   `dex_arb`/`flashloan_arb`/`triangular` NO son stems de ningún .rhai de los 264
   (verificado: `whitelist − stems = {dex_arb, flashloan_arb, triangular}`). Los
   descartados por el whitelist son **256**, no 253.
3. **Las "3 familias base" del whitelist viven en productores legacy (HUEHUFO):**
   `flashloan_arb` lo emite `engines/flashloan_engine.rs:26` (:348), `triangular`
   solo aparece runtime-vía label mapping `cartridge_boot.rs:498,785-790` — ambos
   del stack legacy que WO-02a clasificó default-OFF post-Phase-15. El único
   `"triangular"` literal en `candidate_simulation.rs:660` es fixture
   `#[cfg(test)]` (mod tests :599). Coherente con 02d §F-06 ("8 cartuchos MEV-01 +
   3 familias base").
4. **El mapa operador→cartucho de 02b no restringe nada de esto — al contrario:**
   los 23 ops declarados en los .rhai cubren por unión los 264/264 cartuchos (§2,
   doble-respaldado .rhai+math_map §9). El cruce por operador contra los 8 stems
   admitidos (recomputado por este fixer, script inline en el reporte
   `WO-02b-FIX-TERMINUS-20260917.md` §2):
   - **8 ops tocan ≥1 cartucho admitido** (pueden acompañar un plan ejecutable):
     op_01 (sec), op_15, op_16, op_21, op_22 (sec), op_26 (sec), op_27, op_30 (sec).
     Nota: op_01 y op_30 SOLO como secondary (0 primary en los 264, ver §2) — su
     vía al terminus es exclusivamente la evidencia secundaria de los 8 admitidos.
   - **15 ops quedan 100% fuera del terminus**: op_05, 06, 07, 08, 10, 11, 13, 14,
     17, 19, 20, 23, 24, 25, 29 — ninguno de sus cartuchos (primary ni secondary)
     pasa el whitelist. Su señal solo es observable upstream (evidencia Redis
     `arbx:math_evidence:*` / `strategy_evidence_key`, §3 vías 2-4) y en el
     ranking paper; NUNCA en un plan con binding ejecutable.
   - Los 9 ops nunca declarados (02, 03, 04, 09, 12, 18, 28, 31, 32-registro;
     ver §2/G5) quedan doblemente fuera: ni cartucho ni terminus.
5. **Scope de modo (§34.1.3, precisión load-bearing):** `supports_strategy` vive
   SOLO en relays-client (grep backend completo: 3 call-sites —
   `bundle_builder.rs:60`, `execution_admission.rs:23` (mensaje
   `execution_surface_adapter_required`), `plan_validation.rs:182`
   (`validate_binding`, Err `unsupported_strategy`)). Paper/shadow jamás lo invoca
   → el whitelist NO descarta cartuchos de detección/simulación/scoring: los
   descarta SOLO del terminus de ejecución. Esto no contradice §34.1
   (mode-invariant hot-path): ES la frontera capital/broadcast donde los modos se
   diferencian. La causa raíz del whitelist es la superficie de calldata que el
   terminus sabe validar/construir: wrapper TLS `REQUEST_FLASH_LOAN_SELECTOR` con
   inner `EXECUTE_ARBITRAGE_FLASH_FUNDED` de dos routers
   (forward/backward, plan_validation.rs:53-80) — extender la superficie a más
   kinds = WO propio con diff, operator-gated (NO propuesto aquí).

**Discrepancia publicada en el board** (entrada `// WO-02b-FIX-TERMINUS` en
GOAL-WORKORDERS.md). Clasificación del hecho central: CANONICAL_REPO (re-derivado,
no heredado). Sin colisión: secciones §0-§11 intactas (append-only).

**Verificación post-edit:** grep "supports_strategy\|terminus" en esta ficha =
solo esta sección; cero .rs tocados; cero git/cargo/npm/VPS/HTTP (0/5 requests).
