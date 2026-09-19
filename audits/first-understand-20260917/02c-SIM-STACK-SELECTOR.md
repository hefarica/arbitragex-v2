# 02c — Fichas del stack de simulación estocástica aislada + selección

> WO-02c · kind: design · 2026-09-17 · ecc:rust-reviewer (Gang Omniscience, oleada 1)
> Alcance: `backend/simulator-v2/`, `backend/sim-core/`, `backend/sim-ctl/`, `backend/selector-api/`
> Método: SOLO Read/Grep (PROHIBIDO cargo/npm/build — board GOAL-WORKORDERS.md:17-19). Cero git, cero VPS-mutación.
> Lexicon OMEGA: simulación = simulación estocástica aislada (PAPER_SHADOW, sin broadcast); TLS = flash loan; Topological Yield; Variedad de Liquidez.

## 0. Sincronía de mesa redonda (estado al inicio de este WO)

- `GOAL-WORKORDERS.md` leído completo (board de la oleada).
- Pares 02a/02b/02d: **sin reportes aún** en `audits/first-understand-20260917/` al iniciar (solo `02-VPS-REMAP-20260917.md` + `lines-per-file.txt`). Este reporte es la primera ficha del stack sim → los pares posteriores deben citarlo.
- `02-VPS-REMAP-20260917.md` (orquestador) — dato CRÍTICO para el estado post-fix de SEL-GATE-01: el VPS corre `a06a968d` (main) y los 3 commits locales **fdb40401 (SEL-GATE-01), 125b1e0b (SIM-FUND-01), dcfe890c (PANCAKE-ROUTER-01) NO están desplegados** (evidencia: 02-VPS-REMAP-20260917.md:4-5). El fix existe en el árbol local; en runtime VPS el selector sigue publicando producer-rejected hasta deploy.
- Memoria de misión citada: "SEL-GATE-01 dejó de publicar opportunities producer-rejected al stream validado — verifica el estado post-fix en el código actual" → verificado en §4.

## 1. Flujo de la Opportunity: validar → simular → seleccionar (orden RUNTIME observado)

Contrario a la numeración del charter, el **orden runtime** (evidencia grep de streams) es:

```
searcher-rs (detect)
  └─ XADD arbx:opps:detected
selector-api (fase SELECCIONAR/VALIDAR — S3)         [consumer.ts:23-25, grupo selector-g0]
  ├─ prefilter (kill-switch, SEL-GATE-01, blacklist, CB)   policy/engine.ts:41-80
  ├─ token safety worst-of-pair                            consumer.ts:208-210
  ├─ score (6 factores ponderados)                         scoring/engine.ts:35-52
  ├─ decide (4 gates)                                      policy/engine.ts:89-125
  ├─ persistDecision → opportunities.status = validated|rejected + risk_events  persistence.ts:25-49
  └─ si accept → XADD arbx:opps:validated
sim-ctl (fase SIMULAR — S4)                          [consumer.rs:48-50, grupo sim-ctl-g0]
  ├─ carrier #567 arbx:validated_plan:{id} (TTL 300s)      canonical_plan_consumer.rs:41,76-89
  ├─ fallback A3: PG route_metadata + tokens.decimals       route_lookup.rs / consumer.rs:534
  ├─ encoder OpportunityCandidate→RoundTripContext           sim_core::sim_encoder:421
  └─ execute_multistep_revm (paper_mode=true MANDATORIO)    sim_core::sim_multistep:545
  ├─ insert_simulation → status simulated|rejected          persistence.rs:15-113
  └─ si passed → XADD arbx:opps:simulated
relays-client (terminus §34.3 — WO-02d, NO duplicado aquí)  [relays-client/src/consumer.rs:21]
```

Consumidores observacionales de los streams: `api-server` readiness G-PIPE-1 (g-pipe-1.ts:168-195), gates-status, health — solo lectura.

Tipos canónicos (ficha completa en WO-02d — shared-rs, NO duplicada aquí, solo citada):
- `shared_rs::contracts::{Opportunity, SimulationResult, SimulatorKind}` (consumidos en sim-ctl/src/main.rs:31-39, consumer.rs:36).
- `shared_rs::candidates::{OpportunityCandidate, DecimalsMap, RouteMetadata}` (consumer.rs:598-610, route_lookup.rs:18).
- OJO: existen **DOS** `OpportunityCandidate` — la de shared-rs (contrato HTTP/stream) y `prioritization_spine::types::OpportunityCandidate` (entrada del encoder); `sim_runner.rs:23-28` documenta la conversión `to_spine_candidate` (:39-51).

## 2. Fichas — simulator-v2 (crate lib, 2.640 LOC src)

### 2.1 `simulator-v2/src/lib.rs` — SimulatorV2 (núcleo)
| campo | valor |
|---|---|
| ruta | `backend/simulator-v2/src/lib.rs` |
| firma | `Simulator::simulate(&CandidateInput{chain_id, block, from, to, calldata, value_wei, gas_price_wei}) -> Result<SimResult{net_profit_wei: i128, gas_used: u64, trace_hash}, SimError>` (lib.rs:108-148, impl :208-266) |
| misión | motor REVM single-shot sobre estado on-demand; net-of-gas por diseño (`gas_price_wei` deducido dentro de revm, G-NET-1) |
| op_origen | **multi-dex** (motor de simulación, agnóstico de operador; los routers vienen del catálogo `shared_rs::chains::routers_for_chain` vía sim_encoder) |
| dependencias (inversas) | **consumen su salida**: searcher-rs (main.rs:696-731, sim_orchestrator.rs:287-343, candidate_simulation.rs:466-469), sim-ctl (main.rs:733-747, consumer.rs:632), relays-client (execution_admission.rs:108), mcp-sim-engine (main.rs). **consume**: revm, ethers-provider (LazyDb), sim-core (no — al revés: sim-core consume simulator-v2) |
| estado | PURO respecto a cadena (solo lectura RPC); memoiza bloque vía `OnceLock` (lib.rs:172) — muta estado interno |
| prueba | `SimulatorV2::new/with_block/pinned_block` lib.rs:177-205; `SimResult` :108-118; `Capabilities`/`BACKEND_TAG="v2"` :35-74 |
| destino | **ACTIVO** (consumidores múltiples verificados por grep) |

### 2.2 `simulator-v2/src/lazy_db.rs` — Database lazy sobre RPC
- Firma: `LazyDb::new(rpc_url, Option<block>) -> Result<Self>` (lazy_db.rs:179); `new_verified` :238; `assert_canonical` :287; `pinned_block_number` :347; helpers de seed :652-682.
- Misión: fetch de estado on-demand con caché `DashMap` pinneado a bloque (linealizabilidad). Mutación: solo caché local, cadena intocada.
- Prueba: `ChainSnapshot` :131, `LazyDb` :148. Consumidores: revm_runner, sequence_runner, verified_simulation, searcher tests.
- Destino: **ACTIVO**.

### 2.3 `simulator-v2/src/revm_runner.rs` — ejecutor single-shot
- Firma: `pub fn run<DB>(...)` (revm_runner.rs:55). Entrada CandidateInput+LazyDb+block → ejecuta EVM → SimResult. Puro respecto a cadena.
- Destino: **ACTIVO** (ruta `SimulatorV2::simulate` → lib.rs:264).

### 2.4 `simulator-v2/src/sequence_runner.rs` — secuencias multi-step persistentes
- Firma: `SequenceContext::new(lazy_db, chain_id, block)` :189; `new_verified` :216; `apply_storage` :271; `call` :288; `read_balance` :366; `read_amounts_out` :442; `finalize` :496 → `SequenceResult` :155.
- Misión: CacheDB persistente para round-trips multi-paso (fase A.3.c.3). Es el sustrato de `execute_multistep_revm`.
- Destino: **ACTIVO** (sim-core/sim_multistep lo conduce; searcher multistep_fork test).

### 2.5 `simulator-v2/src/bellman_ford.rs` — detección de ciclos
- Firma: `ArbitrageGraph::find_arbitrage_cycle(&self, start, max_depth)` (bellman_ford.rs:40).
- Misión declarada (lib.rs:13): detección de ciclos negativos en el grafo token-pool.
- Grafo inverso: **CERO consumidores externos** (grep bellman_ford: solo doc-comment en lib.rs:13; searcher usa SU PROPIA implementación `spanning_tree_engine.rs:291 bellman_ford_cycles`; math-engine tiene su propio `rate_to_bellman_ford_weight` route_math.rs:25).
- Destino: **HUEHUFO** (duplicación funcional con searcher/math-engine; nadie lo llama).

### 2.6 `simulator-v2/src/verified_environment.rs`
- Misión: verificación del entorno de la cadena (spec/estado canónico) para `new_verified/assert_canonical`. Consumido por sim-core/verified_simulation.rs:64-66. Destino: **ACTIVO** (vía relays-client execution_admission).

## 3. Fichas — sim-core (crate lib, 3.419 LOC src; extraído VERBATIM de searcher-rs, G-SIM-1 PR-B1)

### 3.1 `sim-core/src/sim_encoder.rs` — encoder candidato→contexto
- Firma: `build_round_trip_context_from_candidate(&OpportunityCandidate(spine), chain_id, executor, &TokenDecimalsProvider, &RouteEncodingConfig) -> Result<RoundTripContext>` (sim_encoder.rs:421); `parse_executor_address` :264; `parse_dex_kind` :365; `resolve_router_address` :386 (catálogo `shared_rs::chains::routers_for_chain` — multi-dex canónico, CERO routers hardcodeados, sim_encoder.rs:33-45).
- Misión: proyección determinista, fail-closed, PURE (sin REVM, sin firma, sin broadcast — header :1-7).
- Alcance documentado: 2 piernas V2/Sushi; V3/Curve/Balancer/triangular/1-pierna = typed error (header :11-22).
- Grafo inverso: searcher-rs (main.rs:39 re-export; candidate_simulation.rs:175-236, 424), sim-ctl (sim_runner.rs:30-32,173).
- Destino: **ACTIVO** (el MISMO encoder en producer y en Canal B — anti-divergencia).

### 3.2 `sim-core/src/sim_multistep.rs` — orquestador wrapped-flash multi-step
- Firma: `execute_multistep_revm(&RoundTripContext, &Arc<SimulatorV2>, &MultiStepExecutionConfig) -> SimulationOutcome` (sim_multistep.rs:545); `MultiStepExecutionConfig` :201.
- Misión: TLS real (`requestFlashLoan` 0x5107d61e envolviendo `executeArbitrageFlashFunded` 0xdde0bf51) contra bytecode forkeado; único storage-cheat = 1 bit de rol `caller→FLE`; Topological Yield = spread retenido por el FLE (`fle_post − fle_pre`), fail-closed si ≤0 (invariantes 1-5, header :60-87).
- **GATE §34/§32**: `paper_mode == true && enable_storage_cheats == true` MANDATORIOS — el orquestador rechaza cualquier flag live (invariante 2, :70-74; sim_runner.rs:193-209 fija ambos a true).
- Destino: **ACTIVO** (searcher candidate_simulation.rs:453-469; sim-ctl sim_runner.rs:213-215; y con feature v2-simulator delega a verified_simulation :555).

### 3.3 `sim-core/src/sim_prefund.rs` — overrides de storage / roles
- Misión: helpers PUROS de cómputo de slots ERC-20/OZ-AccessControl + `build_role_grant_override` (import sim_multistep.rs:88-90). No muta estado REVM por sí mismo. Destino: **ACTIVO** (consumido por sim_multistep).

### 3.4 `sim-core/src/verified_simulation.rs` (feature `v2-simulator`)
- Firma: `execute(...)` (referenciado en relays-client/src/execution_admission.rs:108 y sim_multistep.rs:555). Misión: ruta de simulación con verificación de entorno canónico (`new_verified`/`assert_canonical`). Destino: **ACTIVO** (relays-client — terminus, ficha WO-02d).

## 4. Fichas — sim-ctl (bin + lib, 4.931 LOC src) — fase SIMULAR

### 4.1 `sim-ctl/src/main.rs` — boot, HTTP, selección de backend
- Rutas HTTP: `POST /simulate` (:834), `GET /fork-status` (:835, R8 fail-honest 503/200), `GET /capabilities` (:839-842).
- Entrada /simulate: `SimulateRequest{route_source: pg_metadata|searcher_api|simctl_lookup, candidate?, opportunity_id?, block_number?}` (:78-96) o legacy `Opportunity` (:149-162).
- Caminos de enriquecimiento: A1 (PG, api-server) / A2 (searcher API) / A3 (simctl PG autónomo, :180-328, con gates de completitud → 422 `candidate_incomplete` :361-373).
- Selección de backend: `SIM_BACKEND` (anvil default | revm) (:661-692); REVM exige REDIS_URL fail-fast (:670-674).
- B2c real-sim: `dispatch_b2c_real_sim` (:388-464) — typed 501/503 fail-honest; `passed=false` es 200 (R8).
- GAS: `read_gas_price` lee `gas_price_wei_key(chain_id)` de Redis, rechaza ausente/zero (:471-499) — gas_oracle_worker escribe cada ~10s.
- SIM_SIGNER_ADDRESS: crash-on-boot fuera de dev (:628-645); dev sentinel 0x…dEaD SOLO development (:52-54) — cumple RULE 02.
- Destino: **ACTIVO** (api-server proxy /api/sim-ctl/* + drift-tracker A3).

### 4.2 `sim-ctl/src/consumer.rs` — consumidor del stream validado
- Constantes: `STREAM_IN="arbx:opps:validated"`, `STREAM_OUT="arbx:opps:simulated"`, `GROUP="sim-ctl-g0"` (:48-50).
- Flujo `process_message` (:383-483): parse Opportunity → simulate (B2c o legacy) → `insert_simulation` (idempotente por redelivery, ON CONFLICT DO NOTHING :35 — **solo filas simulator='revm'**, ver ficha 4.5 // WO-02c-FIX-G3) → XADD solo si `passed && inserted_fresh` (:463-475) → XACK.
- `simulate_b2c` (:496-665): 0) carrier #567 PRIMERO (:511-531) — Err Redis = PEL retry, Parse = typed gap; 1-3) reconstrucción A3 con gates; 4) FLASHLOAN_EXECUTOR fail-closed (:620-622); 5) gas live; 6) **block-pinned replay** `simulator_for_candidate` (:632, :776-784 — estado de detección, no latest); 7) run_real_simulation; 8) errores de estado REVM = transitorios → Err (PEL) (:639-647); 9) `simulated_profit_usd: None` PRICES-FREE (R8, :649-664).
- PEL recovery: `recover_stale_pending`/`observe_pel`/`ghost_reason` (:189-381, XAUTOCLAIM cada 60s, claim_min_idle 120s :54-60); ghost = SOLO fila PG confirmada ausente (`ghost_verdict` :761-766).
- Kill-switch: halt con logs de transición + resumen 10-min (:90-127, A5-STALL).
- Destino: **ACTIVO**.

### 4.3 `sim-ctl/src/canonical_plan_consumer.rs` — carrier #567
- Firma: `fetch(&mut ConnectionManager, Uuid) -> Result<Option<ValidatedPlan>, FetchError>` (:76-89); `resimulate(B2cCtx, &Opportunity, ValidatedPlan)` (:99+).
- Misión: el plan del productor (`arbx:validated_plan:{id}`, TTL 300s, escrito por scanner M2 carrier-B) es la **ruta de record** — re-simula el ciclo COMPLETO 2-5 hops en vez de reconstruir desde token_in/token_out (cierra la clase 712/1000 `strategy_cyclic_route_not_simulatable_in_s4`). Mismo carrier que relays-client lee en admisión live (submit_engine.rs:161,442-445 — coherencia producer/consumer/terminus).
- Destino: **ACTIVO**.

### 4.4 `sim-ctl/src/sim_runner.rs` — runner real
- Firma: `run_real_simulation(SharedOpportunityCandidate, Arc<SimulatorV2>, &RealSimEnvConfig, gas_price_wei) -> SimulationOutcome` (:148-233). `RealSimEnvConfig::from_env` (:74-107): ARBITRAGE_EXECUTOR obligatorio fail-closed; SIM_GAS_LIMIT_PER_STEP=500k, SIM_MIN_PROFIT_WEI=0, SIM_ROUTE_DEADLINE_SECS=300 (defaults no-económicos documentados).
- paper_mode+storage_cheats fijos true (:204-205). REVM sync vía spawn_blocking (:213). Contador SIMULATIONS_TOTAL propio (:218-231, anti doble-contaje documentado en consumer.rs:433-438).
- Destino: **ACTIVO**.

### 4.5 `sim-ctl/src/persistence.rs`
- Firma: `insert_simulation(&PgPool, &SimulationResult) -> Result<bool>` (:15). Tx: INSERT simulations + UPDATE opportunities status (`simulated`/`rejected`) SOLO si no es capability-gap (:73-88 — el gap deja la opp no-rechecada, doctrina §34). Prueba de idempotencia: ON CONFLICT :28-36, retorno false :53-61. // WO-02c-FIX-G3 (2026-09-17, cross-exam G3): **la idempotencia es revm-scoped POR DISEÑO** — el ON CONFLICT arbitra sobre el índice PARCIAL `simulations_revm_idempotency_uq ON simulations(opportunity_id) WHERE simulator='revm'` (migrations/113:27-29; comment :12-14 "PARTIAL on purpose: legacy anvil history"; diseño original multi-attempt en migrations/004_simulations.sql:2). El path legacy anvil produce `SimulatorKind::Anvil` (sim_engine.rs:163,205 — default de compose según ficha 4.6): para esas filas el ON CONFLICT nunca dispara → `inserted_fresh` SIEMPRE true → un PEL-redelivery de una sim anvil passed re-publicaría duplicado a `arbx:opps:simulated` (el comentario de consumer.rs:443-450 reconoce que el skip evita "double-count the opportunity downstream"). Impacto HOY nulo (passed=true = 0 en toda la historia, real-cycles-audit-20260916/00-SYNTHESIS). Extender la dedup a anvil, si se desea, = WO propio con diff, operator-gated.
- Destino: **ACTIVO**.

### 4.6 `sim-ctl/src/tx_builder.rs` + `sim_engine.rs` + `anvil_backend.rs` — path legacy anvil
- `build_probe(&Opportunity, signer) -> ProbeTx` (tx_builder.rs:52): sonda single-hop V2/V3 **solo chain 1** (:53-55); rutas cíclicas (token_in==token_out) → `CyclicRouteNotRepresentable` (:66-75) — el gap estructural que #567 resuelve por arriba.
- `SimEngine::simulate` (sim_engine.rs:33): snapshot→eth_call→revert sobre fork anvil; SIEMPRE retorna SimulationResult (fail-honest). SIM-FUND-01: `funder` opcional (sim_engine.rs:25-29; commit 125b1e0b — NO desplegado en VPS).
- `AnvilBackend` adapter delega a SimEngine (anvil_backend.rs:26-35). Destino: **ACTIVO** (es el default SIM_BACKEND=anvil en compose — docker/compose.prod.yml:144-158 NO define SIM_BACKEND → default anvil). // WO-02c-FIX-G5 (2026-09-17): clasificación acotada — CANONICAL_REPO SOLO para el plano compose (el bloque sim-ctl :145-161 no define SIM_BACKEND en `environment`). Para RUNTIME VPS el valor efectivo es **UNKNOWN**: ese mismo bloque monta `env_file: ../.env` (:150-151) y, si el .env del VPS define SIM_BACKEND=revm, el default se sobreescribe (lectura main.rs:661-664) y cambia TODO el análisis de cuál branch corre — incluido B2c carrier-first (exige revm, main.rs:734). 02-VPS-REMAP no leyó el contenido del .env (solo `stat` de permisos, 02-VPS-REMAP-20260917.md:12) → runtime = INFERRED/UNKNOWN, compose = CANONICAL_REPO.

### 4.7 `sim-ctl/src/revm_backend.rs` — wrapper legacy del trait sobre SimulatorV2
- `RevmBackend::from_env` (revm_backend.rs:55); calldata **vacío por construcción** en el path stream (`route_encoding_not_available`, documentado consumer.rs:12-15) — por eso el drain-guard existe. Sirve HTTP /simulate legacy. Destino: **ACTIVO** (opt-in SIM_BACKEND=revm; el flip real es operador-only, .env.example:319).

### 4.8 `sim-ctl/src/lib.rs` + `capabilities.rs` + `fork_manager.rs` + `signer_funding.rs` + `simulator_backend.rs`
- `backend_available_for(sim_backend, fork_ready, b2c_ready)` (lib.rs:23-29): drain-guard PER-BACKEND (SIMWIRE-02c P1-2 — jamás `fork || b2c`); tests de matriz :67-81.
- `flashloan_executor_boot_ready()` (lib.rs:45-47): fail-closed vía `shared_rs::chains::resolve_flashloan_executor_address(1)`.
- `capabilities.rs`: GET /capabilities — verdad de build por LINKAGE (relay de `simulator_v2::capabilities()`, capabilities.rs:30-54); gap honesto documentado: compose NO pasa ARBX_USE_SIMULATOR_V2 a ambos servicios (:49-54).
- `signer_funding.rs` (303 LOC): fondeo del signer probe en el fork anvil (SIM-FUND-01). `fork_manager.rs`: pool de forks + current_block. `simulator_backend.rs`: trait común.
- Destino: **ACTIVO**.

## 5. Fichas — selector-api (TS, 2.125 LOC src) — fase SELECCIONAR/VALIDAR

### 5.1 `selector-api/src/index.ts` — servicio
- Boot: requireEnv DATABASE_URL/REDIS_URL fail-fast (:39-40); PG pool con timeouts (:44-49); kill-switch + 3 circuit breakers (token_safety_api, db_writes, stream_consumer) (:63-86).
- HTTP: `POST /score` (shim síncrono admin/debug, :108-122), `GET /opportunities?status=` (:124-146, lectura PG).
- Consumer del stream arrancado en :149-163. Puerto 3002 (:166). Consumido por api-server (index.ts:65 SELECTOR_URL) y verificador G-TOK-1 (g-tok-1.ts:7).
- Destino: **ACTIVO**.

### 5.2 `selector-api/src/consumer.ts` — corazón S3
- Constantes: `STREAM_IN="arbx:opps:detected"`, `STREAM_OUT="arbx:opps:validated"`, `GROUP="selector-g0"` (:23-25).
- `processOne` (:152-193): parse Zod → decideForOpportunity → persistDecision (bajo CB db_writes) → si accept `publishValidated` (:216-222, añade risk_score) → XACK. At-least-once: no-ack en error (:184-189).
- `decideForOpportunity` (:195-214): prefilter → worst-of-pair token safety (token_in Y token_out, :208-210, fix 2026-08-18) → score con pesos de config → decide.
- Kill-switch halt audible (:87-112, A5-STALL). Destino: **ACTIVO**.

### 5.3 `selector-api/src/policy/engine.ts` — prefilter/decide + **SEL-GATE-01**
- `producerRejected(opp)` (engine.ts:36-39): true si `status==="rejected"` O `rejection_reason` no vacío. Clasificación lifecycle-only conservadora (status Y reason ausentes = NO dropea, :30-35).
- Wire: en `prefilter` ANTES del trabajo caro y del tail persist/publish (engine.ts:56-58, reason `producer_rejected` severity info).
- **ESTADO POST-FIX (misión del WO)**: ✅ presente en el árbol local (commit fdb40401 "fix(selector): SEL-GATE-01 stop publishing producer-rejected opportunities to validated stream"), ✅ con test de regresión `sel-gate01.test.ts` que además ASSERTA el orden del call-site leyendo el fuente de consumer.ts (test :18-22, candado 2 del header :22-25). ❌ **NO desplegado en VPS** (02-VPS-REMAP-20260917.md:4-5: VPS=a06a968d). Evidencia de incidente que motiva: audits/real-cycles-audit-20260916/00-SYNTHESIS.md — **fracción mayoritaria NO cuantificada** de sims quemadas en rows ya rechazadas upstream (74 sims/9h S4, detección 100% rejected — ver C2 §10.4). // WO-02c-FIX-G1 (2026-09-17): la cifra "87%" que estaba aquí fue RE-BAJADA — no reproducible en 00-SYNTHESIS (0 hits de "87%"/"funnel 2026-09-17T04:00Z"); existía SOLO en sel-gate01.test.ts:5-9 y en esta ficha (CIRCULARIDAD — el test header y esta ficha se citaban mutuamente como fuente). Fenómeno canónico intacto (código pre-fix + 00-SYNTHESIS).
- `decide` (:89-125): 4 gates — safety floor, simulation_failed, revert-risk 2×, score threshold. Nótese que decide corre con `sim: null` en el stream path (consumer.ts:213) → los gates 2-3 están latentes en runtime stream (solo activos vía /score con sim).
- Destino: **ACTIVO** (con deuda de deploy).

### 5.4 `selector-api/src/policy/blacklist.ts`
- `addToBlacklist/removeFromBlacklist/isBlacklisted/listBlacklist/addToWhitelist...` (:25-39); `pairAllowed` consumido en engine.ts:60. Set Redis por chain. Destino: **ACTIVO**.

### 5.5 `selector-api/src/scoring/{engine,factors}.ts`
- `scoreOpportunity(opp, sim, safety_score, weights, maxGasPriceGwei) -> {score, factors}` (engine.ts:35-52): suma ponderada 6 factores redondeada a 2 decimales; pesos desde `cfg.scoring.*` (weightsFromConfig :17-27).
- `factors.ts` PURO (:1-7 "pure functions. No I/O"): liquidity=log10(profit_usd) (:24-27), depth=proxy 60/40 (:30-33), safety=passthrough (:36), slippage/gas/risk con neutrales documentados (:41-60). Nota honesta: proxies rudimentarios hasta estado on-chain (S6, :29).
- Destino: **ACTIVO**.

### 5.6 `selector-api/src/scoring/bayesian.ts` — BayesianToxicityEngine
- Inferencia Beta-likelihood VPIN (header :1-24, posterior normalizado en log-space).
- Grafo inverso: **CERO consumidores** — solo lo importa su propio test (`grep bayesian selector-api/src` → únicamente bayesian.test.ts:2). Ni consumer.ts ni engine.ts lo referencian.
- Destino: **HUEHUFO** (implementado, testeado, NO cableado al flujo de decisión).

### 5.7 `selector-api/src/token_safety/*`
- `checkToken(pool, cb, cfg, chain_id, token)` (client.ts:24): worst-of-pair consume esto; caché PG+memoria (cache.ts), proveedor GoPlus externo (goplus.ts) + heurística interna (internal_heuristic.ts). Verificador G-TOK-1 lo vigila (g-tok-1.ts:45-46). Destino: **ACTIVO**.

### 5.8 `selector-api/src/persistence.ts`
- `persistDecision` (:14-58): tx única — UPDATE opportunities status validated|rejected (WHERE status IN detected/validated/scored, :25-35) + INSERT risk_events si severity warning/critical (:37-49). Destino: **ACTIVO**.

### 5.9 `selector-api/src/score.ts`
- Shim retro-compatible del endpoint S1 /score (header :1-7) con pesos default (:27-32). Documentado como admin/debug; el flujo producción es el stream. Destino: **ACTIVO** (surface HTTP mantenida) con nota de deprecación intencional.

## 6. SEL-GATE-01 — veredicto consolidado

| dimensión | estado | evidencia |
|---|---|---|
| Código local | PRESENTE | policy/engine.ts:26-39 (fn), :56-58 (wire), sel-gate01.test.ts (regresión + orden) |
| Commits | fdb40401 local, main local | git status/branch snapshot |
| Deploy VPS | **AUSENTE** | 02-VPS-REMAP-20260917.md:4-5 (VPS=a06a968d) |
| Efecto runtime VPS | selector sigue re-decidiendo rows producer-rejected → quema RPC de fork en sim-ctl | inferencia directa del gap de deploy (INFERRED) |
| Acción requerida | deploy del main local (gated operador, NO-GIT protocol) | board |

Clasificación de evidencia: CANONICAL_REPO para todo lo de código; PRIMARY_SOURCE para 02-VPS-REMAP (ssh del orquestador); INFERRED para el efecto runtime VPS.

## 7. objetivo_usd (RULE 00)

- **GAP** en los 4 crates: no existe en el árbol ningún objetivo_usd, umbral de Target Yield ni blanco económico en config/versionado para el stack sim/selector. Los únicos umbrales económicos son de GATE no de objetivo: `cfg.scoring.min_accept_score` (app.toml), `SIM_MIN_PROFIT_WEI` (default 0, sim_runner.rs:89-93), `cfg.token_safety.min_acceptable_score`. `.env.example` no define objetivo_usd. Prohibido inventar.

## 8. Matriz de destinos (resumen)

| pieza | destino | nota |
|---|---|---|
| simulator-v2 lib/lazy_db/revm_runner/sequence_runner/verified_environment | ACTIVO | núcleo REVM multi-consumidor |
| simulator-v2 bellman_ford | **HUEHUFO** | 0 consumidores; duplicado en searcher/math-engine |
| sim-core sim_encoder/sim_multistep/sim_prefund/verified_simulation | ACTIVO | encoder compartido producer/Canal B/terminus |
| sim-ctl main/consumer/canonical_plan/sim_runner/persistence/route_lookup/lib/capabilities/fork_manager/signer_funding/backends | ACTIVO | fase simular completa |
| sim-ctl tx_builder+sim_engine (probe anvil single-hop) | ACTIVO (legacy default) | supeditado a SIM_BACKEND vigente — compose no lo define (CANONICAL_REPO), runtime VPS UNKNOWN vía env_file ../.env (ver ficha 4.6) // WO-02c-FIX-G5 (2026-09-17) |
| selector-api index/consumer/policy/scoring(engine,factors)/token_safety/persistence/score/blacklist | ACTIVO | SEL-GATE-01 pendiente de deploy |
| selector-api scoring/bayesian | **HUEHUFO** | motor VPIN completo sin cablear |
| GAPs declarados | objetivo_usd (todos); ARBX_USE_SIMULATOR_V2 no propagado a ambos servicios (capabilities.rs:49-54); gates 2-3 de decide() latentes en stream path (sim:null) | fail-honest |
| sim-ctl route_lookup.rs | **ACTIVO** | // WO-02c-FIX-G2 (2026-09-17): ficha 4.9 §12.1 — núcleo del enriquecimiento A3; antes citado solo inline §1 |
| doc-drift route_lookup.rs:15 (header "None on PG error" vs Err real :116-127) | **GAP documental** | // WO-02c-FIX-G2: hallazgo verify §10.2 — comportamiento correcto (Err→PEL consumer.rs:542), el header miente; corrección de código gated operador |
| degradación silenciosa de pin (block_number NULL → replay unpinned contra latest) | **observación** (no defecto R8) | // WO-02c-FIX-G2: hallazgo verify §10.2 — consumer.rs:608 + simulator_for_candidate :776-784; candidato `sim_consumer.unpinned_replay` debug-log, gated |
| doc-drift route_lookup.rs:159-162 (cita test `route_lookup_integration.rs` INEXISTENTE) | **GAP documental** | // WO-02c-FIX-G2: hallazgo NUEVO del fixer — cobertura real = simwire02_route_aware.rs:36-37,138; corrección gated operador |

## 9. Propuestas de diseño (cero edición de código de producción — este WO es design)

1. **DEPLOY-DEBT-01 (prioridad operador)**: deploy del main local (fdb40401+125b1e0b+dcfe890c) — sin él, SEL-GATE-01 y SIM-FUND-01 son código muerto en runtime. Gate: post-deploy `git rev-parse HEAD` == SHA (memoria auto-deploy silent failure).
2. **WIRE-BAYES-01**: conectar `BayesianToxicityEngine` como factor `risk` (o gate VPIN) en scoring/engine.ts — hoy riskFactor es neutral-50 sin sim. Invariante: el posterior solo con datos reales de revert (RULE 00); gate = test de integración consumer→decide.
3. **DEPRECATE-BF-01**: marcar `simulator-v2/src/bellman_ford.rs` HUEHUFO en el inventario o consolidarlo con spanning_tree_engine (decisión de operador; no borrar — §3 surgical).
4. **SEL-GATE-01bis (endurecimiento)**: hoy `producerRejected` confía en los campos del MENSAJE; un producer viejo que no emita status/reason no es dropeado (documentado :30-35). Si el incidente (fracción mayoritaria NO cuantificada de sims en rows ya rechazadas — cifra re-bajada, ver C2 §10.4 y WO-02c-FIX-G1 en §5.3) persiste post-deploy, evaluar cruzar contra PG (status de fila) en el prefilter — con coste de latencia declarado.

## 10. VERIFICACIÓN (WO-02c-verify · cs-validator · 2026-09-17)

> Dictamen: **PASS CON 2 CORRECCIONES** (ninguna invalida la estructura del reporte;
> ambas son de estándar de evidencia, no de wiring). Método: Read/Grep solamente,
> 0 cargo/npm/build/git/HTTP (0/5 requests del presupuesto dominio usados — la
> pregunta runtime-VPS ya estaba evidenciada por 02-VPS-REMAP, PRIMARY_SOURCE;
> re-verificarla por HTTP habría quemado presupuesto sin agregar certeza).

### 10.1 Muestreo de fichas (18 re-abiertas con file:line exacta — requisito ≥8 cumplido)

| ficha | marcadores re-abiertos | resultado |
|---|---|---|
| 2.1 lib.rs | SimResult :108-118, Simulator :143-148, new/with_block/pinned_block :177-205, OnceLock :172, BACKEND_TAG :35, impl :208 | ✅ EXACTA |
| 2.2 lazy_db.rs | ChainSnapshot :131, LazyDb :148, new :179, new_verified :238, assert_canonical :287, pinned_block_number :347 | ✅ EXACTA |
| 2.3 revm_runner.rs | `pub fn run<DB>` :55 | ✅ EXACTA |
| 2.4 sequence_runner.rs | SequenceResult :155, new :189, new_verified :216, apply_storage :271, call :288, read_balance :366, read_amounts_out :442, finalize :496 | ✅ EXACTA |
| 2.5 bellman_ford.rs | `find_arbitrage_cycle` :40; grep inverso: 0 consumidores externos (solo lib.rs:13,20,61) | ✅ EXACTA (ver 10.5 n.1) |
| 3.1 sim_encoder.rs | parse_executor_address :264, parse_dex_kind :365, resolve_router_address :386, build_round_trip_context_from_candidate :421; header RULE 00 :20-22, catálogo :33-45 | ✅ EXACTA |
| 3.2 sim_multistep.rs | execute_multistep_revm :545, invariantes header :60-87, validate :250-257, tests :925-973 | ⚠️ EXACTA CON MATIZ (ver 10.3-C1) |
| 4.1 sim-ctl main.rs | SimulateRequest :79, A3 gates :234-285, 422 :361-373, dispatch_b2c :388, read_gas_price :471, SIM_SIGNER :628-645, backend :661-692, rutas :834-842, sentinel :54 | ✅ EXACTA |
| 4.2 consumer.rs | :48-50, process_message :383-483, simulate_b2c :496-665, carrier :511-531, XADD gate :463-475, PEL :54-60, ghost_verdict :761-766, simulator_for_candidate :776-784 | ✅ EXACTA |
| 4.3 canonical_plan_consumer.rs | CARRIER_KEY_PREFIX :41, fetch :76, resimulate :99, TTL 300s doc :4 | ✅ EXACTA |
| 4.4 sim_runner.rs | run_real_simulation :148, from_env :74-107 (500k/0/300 defaults verificados), flags :204-205, spawn_blocking :213, contador :218-232, to_spine_candidate :39-51 | ✅ EXACTA |
| 4.5 persistence.rs | insert_simulation :15, ON CONFLICT :35, Ok(false) :53-61, gap no-rechazo :73-88 + 6 familias de tests del clasificador :181-295 | ✅ EXACTA |
| 4.6 tx_builder/sim_engine | build_probe :52, chain-1 :53-55, cíclica :66-76, funder SIM-FUND-01 :25-29 | ✅ EXACTA |
| 5.2 selector consumer.ts | :23-25, processOne :152-193, decideForOpportunity :195-214, worst-of-pair :208-210, sim:null :213, publishValidated :216-222 | ✅ EXACTA |
| 5.3 policy/engine.ts | producerRejected :36-39, wire :56-58, clasificación conservadora :33-34, decide :89-125 (gates 2-3 con `sim &&` :101/:109) | ✅ EXACTA |
| 5.5 scoring | scoreOpportunity :35-52, weightsFromConfig :17-27, factors PURO :1-7 | ✅ EXACTA |
| 5.6 bayesian.ts | grep: único import = bayesian.test.ts:2 | ✅ HUEHUFO confirmado |
| 5.8 selector persistence.ts | persistDecision :14-58, WHERE status IN :33, risk_events :37-49 | ✅ EXACTA |

**Tally: 18 fichas re-abiertas · 17 EXACTAS · 1 exacta-con-matiz · 0 erróneas.**

### 10.2 Traza E2E de una Opportunity (validar→simular) — búsqueda de degradación Option silenciosa

Recorrido completo re-ejecutado por lectura: `XADD arbx:opps:detected` →
selector `processOne` (consumer.ts:152) → `prefilter` (engine.ts:41) → worst-of-pair
(consumer.ts:208-210) → `decide` con `sim:null` (:213) → `persistDecision` →
publish solo si accept (:173-175) → sim-ctl `process_message` (consumer.rs:383) →
carrier #567 primero (:511) → A3 `route_lookup::fetch_candidate_inputs` (:534) →
gates de completitud (:547-594) → candidate (:598-610) → `run_real_simulation`
(sim_runner.rs:148) → `insert_simulation` → XADD si `passed && inserted_fresh` (:463).

**¿Hay salto de wiring donde el tipo se degrada a Option vacío silencioso?**
- **Ninguna fabricación R8 encontrada.** Todo ausente se declara con typed reason
  (`counted_gap`, `candidate_incomplete:*`, `route_metadata_not_available`,
  `validated_plan_parse_error:*`) y `simulated_profit_usd: None` llega `None` a PG
  (consumer.rs:659, persistence.rs:44). Los `0.0` de `expected_amount_out`/`gross_profit`
  (consumer.rs:605-606) fueron verificados inocentes: el encoder NO consume esos campos
  (solo aparecen en fixtures de test, sim_encoder.rs:545-546).
- **1 degradación silenciosa REAL (menor, de determinismo — no de R8)**:
  `block_number: inputs.block_number.filter(|b| *b >= 0)...` (consumer.rs:608) → si la
  fila PG trae `block_number NULL`, `simulator_for_candidate` (:780-783) devuelve el
  simulador compartido SIN pin → la simulación corre contra estado `latest`, no contra
  el bloque de detección, **documentado en el doc-comment de la función
  (consumer.rs:768-775: "the shared simulator is reused untouched when no pin
  applies") pero sin trazabilidad runtime (log/métrica) que lo declare**.
  // WO-02c-FIX-UNPINNED (2026-09-17): acota "sin log ni observación" del verify —
  GAP-3 del cross-exam del verify (WO-02c-verify-CROSS-EXAM.md:54-60): lo ausente es
  observabilidad runtime, NO documentación; candidato `sim_consumer.unpinned_replay`
  debug-log sigue en pie (gated). El hallazgo de degradación silenciosa SOBREVIVE
  acotado. Es diseño
  consistente con la convención SimulatorV2 (`None` = latest), pero es el único punto
  del wiring donde un Option vacío degrada calidad sin trazabilidad. Se registra como
  observación para la mesa (posible `sim_consumer.unpinned_replay` debug-log), NO como
  defecto R8.
- **Doc-drift descubierto (no reportado por el diseñador)**: route_lookup.rs:15
  dice "returns `None` on PG error" — la implementación REAL devuelve `Err` en error de
  PG (doc de `fetch_candidate_inputs` :108 + impl :116-127) y `Ok(None)` solo para
  fila ausente/route_metadata vacío. El COMPORTAMIENTO es el correcto (Err→PEL,
  consumer.rs:542); el header miente. Es exactamente la confluencia
  Err/None que `ghost_verdict` (consumer.rs:746-754) documenta como anti-patrón.
  Corrección documental candidata (gated, NO aplicada aquí).

### 10.3 Pregunta (3) del charter — ¿el selector aún publica producer-rejected al stream validado?

- **Código local (post-fdb40401): NO.** `producerRejected` (engine.ts:36-39) corre en
  `prefilter` ANTES del trabajo caro (:56-58) y antes del tail persist/publish; el
  publish es accept-only (consumer.ts:173-175). Test de regresión sel-gate01.test.ts
  existe y además ASSERTA el orden del call-site leyendo el fuente (test :89-100) —
  verificado línea a línea, los 4 casos + candado de orden presentes.
- **Runtime VPS: SÍ, hasta deploy.** VPS en `a06a968d` sin fdb40401
  (02-VPS-REMAP-20260917.md:4-5, PRIMARY_SOURCE del orquestador). El veredicto
  consolidado §6 del diseñador es correcto y bien clasificado (INFERRED para el efecto
  runtime).

### 10.4 RULE 00 en números citados

- `objetivo_usd`: grep en las 4 piezas = **0 hits** → GAP §7 CONFIRMADO. ✅
- Números verificados exactos: TTL 300s (canonical_plan_consumer.rs:4), CLAIM_MIN_IDLE
  120s / recovery 60s (consumer.rs:54-60), SIM_GAS_LIMIT_PER_STEP 500k /
  SIM_MIN_PROFIT_WEI 0 / deadline 300 (sim_runner.rs:84-98), MAXLEN 10_000
  (consumer.ts:27, consumer.rs:51), grupos selector-g0/sim-ctl-g0.
- **CORRECCIÓN C2 — la cifra "87% de sims quemadas en rows ya rechazadas" NO es
  reproducible en la fuente citada.** sel-gate01.test.ts:5-9 y §5.3/§6 de este reporte
  la atribuyen a `audits/real-cycles-audit-20260916/00-SYNTHESIS.md` "funnel
  2026-09-17T04:00Z": ese archivo NO contiene 87% (sí contiene "74 sims/9h",
  "100% rejected", "v3_quote_unavailable 60/76"). Grep en todos los audits: "87%"
  aparece en cerebro-2026-09-07 (77.87% S5, κ=20→86.96% — contextos distintos) y en el
  propio test/ficha (circular). El FENÓMENO (selector re-decidiendo rows
  producer-rejected) está canónicamente soportado por el código pre-fix y por
  00-SYNTHESIS (74 sims consumidas con 0 passed y detección 100% rejected), pero la
  cifra exacta carece de artefacto reproducible — estándar de evidencia §34.5.3. Acción:
  o se archiva el query/embudo que produjo 87%, o la cifra se re-baja a "mayoría/fracción
  no cuantificada" en test header + este reporte. (Edición de código = gated operador;
  aquí solo se declara.)

### 10.5 Omisiones (vs lines-per-file.txt, árbol principal, src no-test)

- **selector-api: COMPLETO** (13/13 archivos src cubiertos; // WO-02c-FIX-G4: era
  "12/12"). **simulator-v2: COMPLETO**
  (6/6). **sim-ctl: 14/16** // WO-02c-FIX-TALLY (2026-09-17): era "15/16" — off-by-one:
  eran DOS archivos sin ficha (route_lookup.rs y build.rs), no uno; ver errata §15 —
  `route_lookup.rs` (232 LOC, núcleo del enriquecimiento A3
  con `merge_decimals` y resolución tokens) solo se cita inline en §1, sin ficha propia:
  **OMISIÓN REAL** (la de mayor peso de esta verificación; su comportamiento Err/None es
  precisamente material de traza E2E, ver 10.2). `build.rs` (157 LOC, script de build)
  y `sim-core/src/lib.rs` (49 LOC, raíz de re-exports `pub mod` — verificada trivial:
  0 lógica) sin ficha: omisiones menores defendibles.
- Nota n.1 sobre 2.5: bellman_ford sigue ANUNCIADO en `/capabilities`
  (lib.rs:61 `modules: ["bellman_ford", ...]`) — el módulo HUEHUFO es parte de la verdad
  de build que expone el servicio. No cambia el destino, sí la nota.

<!-- WO-02c-FIX-G4 (2026-09-17): nota correctiva append-only al tally de arriba -->
> **// WO-02c-FIX-G4 (2026-09-17, fixer gang ronda 1 — GAP-4 del cross-exam
> `WO-02c-CROSS-EXAM.md:173` / GAP-1 de `WO-02c-verify-CROSS-EXAM.md:32-42`)** —
> ERRATA de conteo en la línea "selector-api: COMPLETO (12/12)" de arriba: el ground
> truth es **13 archivos src no-test**, reproducible con
> `cd backend/selector-api && find src -type f ! -name '*.test.ts' | wc -l` → 13
> (re-ejecutado por este fixer, RULE 00 — no heredado; concuerda con el conteo del
> charter y del cross-exam). La cobertura real es **13/13**: las fichas §5.1–5.9
> cubren los 13 exactos (5.1 index, 5.2 consumer, 5.3 policy/engine, 5.4
> policy/blacklist, 5.5 scoring/{engine,factors}×2, 5.6 scoring/bayesian, 5.7
> token_safety/{cache,client,goplus,internal_heuristic}×4, 5.8 persistence, 5.9
> score). **La conclusión de cobertura (COMPLETO) SOBREVIVE; el conteo no.** Origen
> del "12": NO reproducible desde `lines-per-file.txt` — ninguno de sus dos árboles
> (principal :9141-9161 = 13 no-test de 21; worktree stale :4896-4914 = 13 no-test
> de 19, sin sel-gate01.test.ts) produce 12 → clasificado UNKNOWN (desliz de conteo
> manual; el hazard de doble árbol del cross-exam §3 NO lo explica). Ironía
> registrada: el propio verify aplicó en C2 (§10.4) el estándar de evidencia
> reproducible §34.5.3 a la cifra "87%" y no lo aplicó a su propio tally. Corrección
> espejo aplicada en `WO-02c-verify-VERIFY.md` §6 + errata al pie de ese archivo.
> La parte sim-ctl de este mismo §10.5 ("15/16") ya quedó superseded por §12.3 del
> fix G2 — **pero NO a 16/16**: // WO-02c-FIX-TALLY (2026-09-17) corrige esa cadena:
> pre-G2 = **14/16** (route_lookup.rs Y build.rs sin ficha), post-G2 = **15/16**
> (solo build.rs pendiente; el "16/16" de §12.3 era aritméticamente erróneo). Ver §15.

### 10.6 Matiz C1 sobre ficha 3.2 (exacta-con-matiz)

§3.2 dice "el orquestador rechaza cualquier flag live (invariante 2, :70-74)". Preciso:
el rechazo tipado (`PaperModeRequired`/`StorageCheatsDisabled`) vive en
`MultiStepExecutionConfig::validate()` (:250-257, invocado por build_plan :355); pero
`execute_multistep_revm` con `paper_mode=false` NO retorna error — DELEGA a
`verified_simulation::execute` (:554-556), la ruta cheat-free del terminus. Es decir:
ningún storage-cheat fuera de paper mode (invariante de aislamiento INTACTO — §32/§33/§34.3
OK, confirmado también por sim_runner.rs:204-205 y tests :925-936), pero la frase
"rechaza cualquier flag live" debe leerse "rechaza en el path paper; el path no-paper se
enruta a la simulación verificada sin cheats". WO-02d ya documentó el drift
doctrina↔código del terminus en la misma dirección.

### 10.7 Tally final

| dimensión | conteo |
|---|---|
| fichas muestreadas (req ≥8) | 18 |
| file:line exactas | 60+ marcadores re-abiertos, 0 desviaciones >±2 líneas |
| correcciones emitidas | 2 (C1 matiz paper_mode §10.6; C2 cifra 87% no reproducible §10.4) |
| omisiones | 1 real (route_lookup.rs sin ficha) + 2 menores (build.rs, sim-core/lib.rs) |
| hallazgos NUEVOS del verificador | 2 (doc-drift route_lookup.rs:15; degradación silenciosa de pin block_number NULL §10.2) |
| violaciones RULE 00 / R8 / §32-34 / NO-GIT | 0 en el código auditado; 0 acciones prohibidas ejecutadas |

**Dictamen: PASS CON CORRECCIONES (C1, C2) — la estructura, el flujo E2E, el veredicto
SEL-GATE-01 (local presente / VPS sin deploy) y los destinos ACTIVO/HUEHUFO/GAP del
diseñador quedan CONFIRMADOS con evidencia re-abierta.**

## 11. ERRATA // WO-02c-FIX-G1 (2026-09-17, fixer gang ronda 1 — GAP-1 del cross-exam)

> Registro append-only de la corrección quirúrgica aplicada en §5.3 (línea :171) y
> §9.4 (línea :231) por G1 de `WO-02c-verify-CROSS-EXAM.md:170`. La corrección C2
> (§10.4) fue declarada por el verificador pero NUNCA aplicada al cuerpo del
> entregable, que seguía citando "87% de sims quemadas" como evidencia (violación del
> estándar de evidencia reproducible §34.5.3, en circularidad con el header de
> `sel-gate01.test.ts:5-9`).

- **Cambio 1 (§5.3, :171)**: "(87% de sims quemadas en rows ya rechazadas — citado en
  el test header :5-9)" → "fracción mayoritaria NO cuantificada de sims quemadas en
  rows ya rechazadas upstream (74 sims/9h S4, detección 100% rejected — ver C2 §10.4)"
  + nota de circularidad inline.
- **Cambio 2 (§9.4, :231)**: "Si el incidente 87% persiste post-deploy" → "Si el
  incidente (fracción mayoritaria NO cuantificada... ver C2 §10.4) persiste
  post-deploy".
- **NO tocado (operator-gated)**: header de `backend/selector-api/src/policy/
  sel-gate01.test.ts:5-9` — sigue conteniendo "87%" y "funnel 2026-09-17T04:00Z".
  Su edición requiere autorización del operador (GAP-2/cierre del cross-exam :183).
  Cualquier pares que lea ese header debe cruzarlo con C2 §10.4 y esta errata.
- **Evidencia base re-verificada por este fixer (RULE 00, no heredada)**: grep
  "87%|funnel 2026-09-17T04:00Z" en `audits/real-cycles-audit-20260916/` = **0 hits**;
  00-SYNTHESIS.md:24 contiene "76 opps/9h post-deploy ... 100% rejected",
  "v3_quote_unavailable 60/76"; GOAL-WORKORDERS.md:17 (real-cycles) contiene "74 sims".
  Coincide con C2 §10.4 y con la confirmación independiente del cross-exam
  (WO-02c-verify-CROSS-EXAM.md:44-50).
- **Nota de precisión sobre el cross-exam**: su GAP-2 cita "§5.3 (:171) y §6" — el
  grep de este fixer NO encuentra "87%" en §6 (:197-207); las ocurrencias
  claim-bearing eran SOLO :171 y :231 (§9.4). Imprecisión menor del cross-examiner,
  documentada sin tocar su archivo.
- **Clasificación de evidencia del reemplazo**: los números "74 sims/9h" y
  "100% rejected" son PRIMARY_SOURCE (00-SYNTHESIS.md:24 + real-cycles
  GOAL-WORKORDERS.md:17); "fracción mayoritaria" es INFERRED conservador (el
  embudo exacto sims-sobre-rows-rechazadas no tiene query archivado — fail-honest).
- Restricciones cumplidas: cero git/cargo/npm/build/VPS/HTTP (0/5 requests).
  Archivos tocados: solo este .md de auditoría (no es código, no es git-gated).

## 12. ERRATA // WO-02c-FIX-G2 (2026-09-17, fixer gang ronda 1 — GAP-2 del cross-exam)

> Cierra G2 de `WO-02c-verify-CROSS-EXAM.md:88-97/:171`: la omisión que el propio
> verify calificó de "OMISIÓN REAL, la de mayor peso" (§10.5) quedó sin cerrar antes
> del DONE del WO. Este fix añade la ficha faltante (§12.1, numerada 4.9 por
> convención de sección §4 sim-ctl) y SUBE a la matriz §8 los hallazgos del verify
> que vivían solo en el adendum. Todos los marcadores re-verificados por este fixer
> con Read/Grep directo (RULE 00: no heredados).

### 12.1 Ficha 4.9 — `sim-ctl/src/route_lookup.rs` (232 LOC exactos, wc -l) — núcleo del enriquecimiento A3

| campo | valor |
|---|---|
| ruta | `backend/sim-ctl/src/route_lookup.rs` |
| firma | `fetch_candidate_inputs(&PgPool, Uuid) -> Result<Option<CandidateInputs>, sqlx::Error>` (:113-116); `merge_decimals(&DecimalsMap, &[(String, i16)]) -> DecimalsMap` (:60, PURA); `resolve_route_decimals(&PgPool, i32, &[String]) -> Result<Vec<(String, i16)>, sqlx::Error>` (:82-86) |
| semántica Err/None (el corazón de la traza E2E) | `Ok(Some)` = fila existe Y `route_metadata.is_populated()` (:129-137; `is_populated` def shared-rs/candidates.rs:188). `Ok(None)` = fila ausente (:130) O route_metadata vacío/`{}` (:132-134) — tratado como ausente, R8: nunca fabrica topología (doc :104-108). `Err(sqlx::Error)` = error PG (query :117-127 o resolve_route_decimals :86-100) — el caller lo enruta a **PEL retry**, NO dead-letter (consumer.rs:534-543: `Err(e) => return Err(...)` :542), coherente con `ghost_verdict` :746-766 (Err ≠ confirmado ausente) |
| misión | fase SIMULAR (S4) — fallback A3: consulta `opportunities.route_metadata` JSONB DIRECTO (mig `099_opportunities_route_metadata.sql`, sin api-server ni searcher-rs — header :1-13) + campos económicos de la fila + resolución de decimals (tabla `tokens`, mig 098, overlay route_metadata) |
| merge_decimals | puro, unit-testeable sin PG (:54-59); skip honesto de decimals fuera `0..=255` (SMALLINT→u8, :62-66, test :219-231); la claim de la fila gana en el overlay (:67-70, test :208-216) |
| op_origen | **multi-dex** (enriquecimiento de ruta, agnóstico de operador) |
| dependencias (inversas) | consumer.rs:534 (path stream B2c — "same source as the A3 HTTP path" :533), main.rs:208 (path HTTP A3), mod decl main.rs:17; doc-refs consumer.rs:66, main.rs:795. Prueba con PG vivo: `sim-ctl/tests/simwire02_route_aware.rs` (incluye el módulo por `#[path = "../src/route_lookup.rs"]` :36-37; llama `fetch_candidate_inputs` :138-147 + `validate_complete` :147) |
| estado | I/O PG asíncrono (2 queries por llamada); merge_decimals PURO. Sin mutación de cadena, sin firma, sin broadcast (§32/§33 OK) |
| destino | **ACTIVO** (2 call-sites productivos + prueba de integración viva) |
| objetivo_usd | **GAP** (hereda §7 — grep 0 hits, RULE 00) |

### 12.2 Hallazgos subidos a la matriz §8 (antes solo en adendum §10.2)

1. **doc-drift :15 (hallazgo del verify)** — el header del módulo dice "R8 fail-honest:
   returns `None` on PG error"; la implementación devuelve `Err` en error PG (doc real
   :108, impl :116-127) y `Ok(None)` solo para fila ausente/metadata vacío. El
   COMPORTAMIENTO es el correcto; el header miente. Corrección de 1 línea en código =
   gated operador (NO aplicada).
2. **unpinned_replay (hallazgo del verify)** — `block_number` NULL en la fila PG →
   candidate sin pin (consumer.rs:608) → `simulator_for_candidate` :780-783 devuelve el
   simulador compartido → replay contra `latest`, no el bloque de detección, sin
   trazabilidad runtime (log/métrica) — el caso None SÍ está documentado en el
   doc-comment consumer.rs:768-775. // WO-02c-FIX-UNPINNED (2026-09-17): acota el
   "SIN log" del hallazgo original (GAP-3 del cross-exam del verify); la observación
   (no defecto R8) y el candidato `sim_consumer.unpinned_replay` debug-log (gated)
   quedan en pie.
3. **doc-drift :159-162 (hallazgo NUEVO de este fixer)** — el comentario del bloque de
   tests cita `sim-ctl/tests/route_lookup_integration.rs` como "the live-PG test":
   ese archivo **NO existe** (grep del árbol completo: la única ocurrencia del nombre
   es la propia cita). La cobertura real vive en `simwire02_route_aware.rs` (misma
   función, PG vivo). Drift de puntero, no de cobertura — la prueba EXISTE bajo otro
   nombre. Corrección gated operador.

### 12.3 Verificación del fix

- Todos los marcadores de la ficha re-abiertos con Read/Grep por este fixer (232 LOC
  wc -l; :15, :60, :82-86, :104-108, :113-116, :117-127, :129-137, :159-162;
  consumer.rs:534-543/:608/:746-766/:776-784; main.rs:17/:208; shared-rs/candidates.rs:188;
  mig 098/099 en database/migrations/; tests/simwire02_route_aware.rs:36-37/:138-147).
- Sim-ctl pasa de **14/16** a **15/16** fichado // WO-02c-FIX-TALLY (2026-09-17):
  esta línea decía "de 15/16 a 16/16" — errata aritmética; con build.rs SIN ficha
  el máximo alcanzable era 15/16, y el punto de partida (verify §10.5) era 14/16
  (dos sin ficha: route_lookup.rs y build.rs). Denominador 16 = 15 src .rs +
  build.rs (árbol principal, filtro `./backend/sim-ctl/` sobre lines-per-file.txt,
  excluye tests/ y worktree). sim-core/lib.rs es OTRO crate (sim-core), fuera del
  denominador (las omisiones menores build.rs y sim-core/lib.rs permanecen
  declaradas, verify §10.5). Ver errata §13.
- Restricciones: cero cargo/npm/build/git/VPS/HTTP (0/5 requests). Archivos tocados:
  solo este .md de auditoría + fila en §8 (aditiva). Colisión de sección con el fixer
  G1 (que anexó §11 en paralelo): resuelta renumerando la mía a §12 y corrigiendo la
  referencia de la fila §8 — cero contenido de G1 tocado.

## 13. ERRATA // WO-02c-FIX-G5 (2026-09-17, fixer gang ronda 1 — GAP-5 del cross-exam)

> Cierra G5 de `WO-02c-CROSS-EXAM.md:131-139/:174` (nota de clasificación en §7 del
> DESIGN / ficha 4.6 de este entregable): el claim "default SIM_BACKEND=anvil" se
> presentaba como verdad RUNTIME siendo solo verdad de COMPOSE. Reclasificado:
> **compose = CANONICAL_REPO · runtime VPS = INFERRED/UNKNOWN**.

### 13.1 Evidencia re-verificada por este fixer (no heredada, RULE 00)

1. `docker/compose.prod.yml:145-161` — bloque `sim-ctl:`: `environment:` pasa solo
   ARBX_CONFIG_PATH/SIM_PORT/DATABASE_URL/REDIS_URL/ANVIL_URL (:152-157); NO define
   SIM_BACKEND (grep 0 hits en compose.prod.yml — confirma WO-02c-verify-CROSS-EXAM.md:19).
2. `docker/compose.prod.yml:150-151` — el MISMO bloque monta `env_file: - ../.env`
   ANTES de `environment`: cualquier `SIM_BACKEND=...` del .env del VPS entra al
   contenedor y sobreescribe el default del código.
3. `backend/sim-ctl/src/main.rs:661-664` — `std::env::var("SIM_BACKEND")
   .as_deref().unwrap_or("anvil")`: el default "anvil" vive en el BINARIO, no en
   compose; `:734` exige `== "revm"` para poblar el simulador B2c (carrier-first).
4. `02-VPS-REMAP-20260917.md:12` — la evidencia VPS disponible solo hizo `stat -c "%a"`
   del .env (permisos 644 + baks); NO leyó su contenido → el valor efectivo runtime
   es UNKNOWN desde evidencia local. (Leerlo sería acción VPS read-only disponible
   para WO-06/operador: `ssh arbx grep -c '^SIM_BACKEND=' /opt/arbitragex-v2/.env`.)

### 13.2 Cambios aplicados (todos marcados `// WO-02c-FIX-G5 (2026-09-17)`)

- Ficha 4.6 (línea :142 de este archivo): nota de clasificación acotando el claim
  ACTIVO a compose + UNKNOWN runtime.
- Matriz §8 (fila tx_builder+sim_engine, :221): "supeditado a SIM_BACKEND vigente —
  compose no lo define (CANONICAL_REPO), runtime VPS UNKNOWN vía env_file ../.env".
- `WO-02c-DESIGN.md` hallazgo 7 (:22): mismo acotamiento — el sello CANONICAL_REPO
  original queda RESTRINGIDO al plano compose; runtime = INFERRED/UNKNOWN.

### 13.3 Impacto y verificación del fix

- El destino ACTIVO de la ficha 4.6 SOBREVIVE condicionado: el binario VPS corre
  a06a968d (02-VPS-REMAP) que YA contiene la lectura SIM_BACKEND (misma :661), por lo
  que la incertidumbre es puramente del VALOR del .env, no del mecanismo.
- Cadena de dependencia afectada si el .env dijera revm: B2c carrier-first activo,
  RevmBackend en stream, y G3 (dedup parcial WHERE simulator='revm') pasa a ser el
  camino caliente — por eso la clasificación es load-bearing, no cosmética.
- Re-verificación post-edit: grep "SIM_BACKEND" en este archivo = citas acotadas en
  :111 (default de CÓDIGO, correcto), :142 (nota G5), :221 (nota G5); grep en
  WO-02c-DESIGN.md = :22 con nota G5. Cero .rs tocados; cero código gated modificado.
- Restricciones: cero git/cargo/npm/build/VPS/HTTP (0/5 requests). Archivos tocados:
  este .md (ficha 4.6 + fila §8 + esta errata append-only) + 1 línea en
  WO-02c-DESIGN.md (mismo WO-02c, no es archivo de otro owner).

## 14. ERRATA // WO-02c-FIX-G3 (2026-09-17, fixer gang ronda 1 — GAP-3 del cross-exam)

> GAP-3 (hallazgo NUEVO del cross-examiner, WO-02c-CROSS-EXAM.md §2-G3): el hard
> finding 4 "Idempotencia de redelivery: ON CONFLICT DO NOTHING + skip de XADD"
> estaba SOBRE-GENERALIZADO — se presentaba como verdad canónica de wiring
> incondicional cuando el índice árbitro es PARCIAL a propósito.

### 14.1 Evidencia re-verificada de primera mano por este fixer (no heredada)

- `database/migrations/113_simulations_revm_idempotency.sql:27-29`:
  `CREATE UNIQUE INDEX CONCURRENTLY ... simulations_revm_idempotency_uq ON
  simulations (opportunity_id) WHERE simulator = 'revm'`. Comment :12-14:
  "PARTIAL (WHERE simulator = 'revm') on purpose: legacy anvil history
  legitimately holds multiple attempts per opportunity".
- `database/migrations/004_simulations.sql:2` (diseño original): "One simulation
  per attempt; an opportunity may have multiple (e.g., retried with tweaked
  params)" — la invariante una-fila-por-opportunidad es de la era revm, no del
  schema base.
- `backend/sim-ctl/src/persistence.rs:35` — el ON CONFLICT es
  `ON CONFLICT (opportunity_id) WHERE simulator = 'revm' DO NOTHING`: para una
  fila `simulator='anvil'` el conflicto NUNCA arbitra (el índice parcial no la
  indexa) → INSERT siempre inserta → `rows_affected()==1` → `Ok(true)`.
- `backend/sim-ctl/src/sim_engine.rs:163,205` — el path legacy produce
  `SimulatorKind::Anvil` (y es el default de compose, ficha 4.6 + nota G5);
  `SimulatorKind::Revm` solo en B2c (canonical_plan_consumer.rs:205,
  consumer.rs:660, revm_backend.rs).
- `backend/sim-ctl/src/consumer.rs:451-463` — `inserted_fresh` y el gate
  `sim.passed && inserted_fresh` para el XADD; el comentario :443-450 documenta
  que el skip existe para no "double-count the opportunity downstream".

**Consecuencia (contraejemplo canónico):** un PEL-redelivery de una sim
**anvil passed** re-persiste una fila nueva (multi-attempt, por diseño de la
era anvil) y re-publica duplicado a `arbx:opps:simulated`. Impacto HOY nulo:
`passed=true` = 0 en toda la historia (audits/real-cycles-audit-20260916/
00-SYNTHESIS.md). La dedup SOLO es exactamente-once para `simulator='revm'`.

### 14.2 Cambios aplicados (todos marcados `// WO-02c-FIX-G3 (2026-09-17)`)

1. `WO-02c-DESIGN.md:19` (hallazgo 4): reclasificado
   `[CANONICAL_REPO]` → `[CANONICAL_REPO — scope ACOTADO por // WO-02c-FIX-G3]`
   con la cadena de evidencia completa y la nota de que extender la dedup a
   anvil = WO propio operator-gated (coincide con G6/G7 del cross-exam).
2. Ficha 4.5 (`persistence.rs`, este archivo): nota inline con el scope
   revm-only, migraciones 113/004, sim_engine.rs:163,205 y el contraejemplo de
   double-publish bajo redelivery.
3. Ficha 4.2 (`consumer.rs`, este archivo): el parenthetical "idempotente por
   redelivery" acotado a "solo filas simulator='revm'" con remisión a 4.5
   (misma sobre-generalización, tercera ocurrencia del claim — contaminación
   intra-entregable cerrada).
4. NO tocados (verificados inocentes): :278 (§10.2 traza E2E — describe el gate
   `passed && inserted_fresh` tal como el código lo escribe, factual);
   :260 (fila §8 de la matriz del verify — citas file:line, factual);
   `WO-02c-verify-VERIFY.md` — grep "idempoten" = 0 claims del hallazgo 4
   (solo una nota de drift ±2 líneas sobre ON CONFLICT :20). Código de
   producción (`persistence.rs`, `consumer.rs`, migraciones): INTOCADO —
   extender la dedup a anvil es decisión de diseño del operador (¿rompería la
   semántica multi-attempt-diagnostics de la era anvil documentada en mig 113
   :12-14 y mig 004:2?).

### 14.3 Sincronía de mesa y cumplimiento

- Pares citados: WO-02c-CROSS-EXAM.md §2-G3 (origen), 00-SYNTHESIS.md
  (passed=0 histórico), ficha 4.6 + errata §13 del fixer G5 (que ya anticipaba
  "G3 pasa a ser el camino caliente si .env dijera revm" — convergencia de
  criterios, cero contradicción).
- Convergencia con G5: si el .env del VPS define SIM_BACKEND=revm, la dedup
  revm-scoped es el camino caliente; si no, el branch por defecto (anvil) corre
  SIN dedup — el matiz de este fix es load-bearing para cualquier wire-audit
  de redelivery que WO-06/gates consuma.
- Restricciones: cero git/cargo/npm/build/VPS/HTTP (0/5 requests). Archivos
  tocados: este .md (fichas 4.2/4.5 + esta errata append-only) + 1 hallazgo en
  WO-02c-DESIGN.md (mismo WO-02c, no es archivo de otro owner). Colisión con
  fixers paralelos G1/G2/G4/G5: ninguna — secciones/erratas disjuntas, mis
  edits no tocan contenido ajeno (verificado post-edit con grep de marcadores).

## 15. ERRATA // WO-02c-FIX-TALLY (2026-09-17, fixer gang ronda 1 — tallies aritméticos de §10.5)

> Cierra la mitad sim-ctl del GAP de tallies del cross-exam (charter fixer ronda 1;
> la mitad selector-api 12/12→13/13 fue cerrada en paralelo por WO-02c-FIX-G4 —
> convergencia dual, ver abajo). Bajo FAIL-HONEST del board los conteos de evidencia
> deben ser exactos; había TRES cifras erróneas encadenadas, no una.

### 15.1 Aritmética correcta (re-derivada por este fixer, RULE 00 — no heredada)

- **Denominador sim-ctl = 16** archivos .rs no-test del árbol principal:
  15 en `src/` (anvil_backend, canonical_plan_consumer, capabilities, consumer,
  fork_manager, lib, main, persistence, revm_backend, route_lookup, signer_funding,
  sim_engine, sim_runner, simulator_backend, tx_builder) + `build.rs`.
  Reproducible: `find backend/sim-ctl/src -type f | wc -l` → 15; `ls
  backend/sim-ctl/build.rs` → existe (157 LOC). **Filtro documentado contra
  lines-per-file.txt**: SOLO rutas `./backend/sim-ctl/...` — el artefacto mezcla
  árbol principal (:9198+) con worktree stale
  `./.claude/worktrees/wf_257a24e1-859-2/backend/sim-ctl/...` (:4947-4959, sin
  canonical_plan_consumer/capabilities/lib/signer_funding y con LOC desfasadas,
  p.ej. consumer.rs 188 vs 937). `tests/` (3 archivos) fuera del universo "src
  no-test" por convención del propio §10.5.
- **Cobertura al momento del verify §10.5 (pre-FIX-G2) = 14/16**: fichas §4.1–4.8
  cubren 14 archivos (§4.6 agrupa tx_builder+sim_engine+anvil_backend; §4.8 agrupa
  lib+capabilities+fork_manager+signer_funding+simulator_backend); SIN ficha:
  `route_lookup.rs` (solo cita inline §1) Y `build.rs`. El verify reportó "15/16"
  contando UNA sola omisión — off-by-one.
- **Cobertura post-FIX-G2 (ficha §12.1 route_lookup.rs) = 15/16**: queda SOLO
  `build.rs` sin ficha (omisión menor defendible, script de build). El "pasa de
  15/16 a **16/16**" de §12.3 era auto-contradictorio (la misma línea admitía que
  build.rs "permanece declarada" como omisión) — corregido in situ a "de 14/16 a
  15/16" con marcador.
- **selector-api = 13/13** (confirmación independiente del fixer G5, concuerda con
  G4): 13 src no-test en disco; fichas §5.1–5.9 cubren los 13 exactos
  (5.5 = 2 archivos, 5.7 = 4 archivos). simulator-v2 6/6 correcto (6 src no-test
  del árbol principal: bellman_ford, lazy_db, lib, revm_runner, sequence_runner,
  verified_environment — ojo: el worktree tiene 5, sin verified_environment.rs).

### 15.2 Origen del error (clasificación de evidencia)

- El charter hipotetizaba "lines-per-file.txt mezcla árbol principal + worktree"
  como causa del "12/12". **REFUTADO por doble conteo**: el árbol principal
  (:9141-9161) da 13 no-test de 21 y el worktree (:4896-4914) da TAMBIÉN 13
  no-test de 19 (al worktree solo le faltan tests: sel-gate01.test.ts,
  internal_heuristic.test.ts). Ningún filtro de árbol produce 12 → el "12" se
  clasifica UNKNOWN (desliz manual), igual que el "15/16" (subconteo de omisiones:
  declaró 1 de 2). El hazard de doble árbol ES real para LOC (cifras divergentes)
  pero NO explica estos dos tallies. Coincide con la conclusión de G4.
- Propagación en cadena documentada: verify §10.5 "15/16" → FIX-G2 §12.3
  "de 15/16 a 16/16" (hereda el punto de partida erróneo Y agrega uno nuevo) →
  nota G4 en §10.5 "superseded a 16/16". Las tres correcciones in situ llevan
  marcador `// WO-02c-FIX-TALLY`; el contenido fáctico ajeno (ficha §12.1, hallazgos
  §12.2) quedó INTACTO.

### 15.3 Verificación del fix

- Post-edit grep en este archivo: "12/12" = 0 ocurrencias vivas (solo citas
  históricas en erratas G4/§15), "15/16 a 16/16" = 0; el cuerpo §10.5 lee
  "13/13" y "14/16"; §12.3 lee "de 14/16 a 15/16".
- Espejo aplicado en `WO-02c-verify-VERIFY.md` §6 (corrección in situ con
  marcador) + errata §10 append-only al pie de ese archivo.
- Restricciones: cero git/cargo/npm/build/VPS/HTTP (0/5 requests). Archivos
  tocados: SOLO `02c-SIM-STACK-SELECTOR.md` y `WO-02c-verify-VERIFY.md`
  (reportes del gang — no código de producción, no git-gated, no requiere gate
  arbx-*). Claims de archivo respetados: sin tocar fichas ni hallazgos de pares;
  conflicto con las cifras de FIX-G2/G4 resuelto por errata documentada, no por
  borrado.

## 16. ERRATA // WO-02c-FIX-UNPINNED (2026-09-17, fixer gang ronda 1 — GAP-3 del cross-exam del verify)

> Cierra el GAP-3 de `WO-02c-verify-CROSS-EXAM.md:54-60` (agent-fixable): el
> §10.2 de este entregable decía que el unpinned replay ocurre "**sin log ni
> observación que lo declare**", sobrestimando la falta de trazabilidad.

### 16.1 Evidencia (re-verificada por este fixer, RULE 00 — no heredada)

- `backend/sim-ctl/src/consumer.rs:768-775` — el doc-comment de
  `simulator_for_candidate` (:776) documenta EXPLICITAMENTE el caso None →
  simulador compartido sin pin: "The consumer's shared simulator carries
  whatever pin boot gave it (typically none — `latest`)... the shared
  simulator is reused untouched when no pin applies." La cita del
  cross-examiner ("consumer.rs:776-783") apunta a la firma; el comentario
  vive en :768-775 (verificado con Read directo).
- Lo ausente es **observabilidad RUNTIME** (log/métrica del evento en
  producción), no documentación. El hallazgo de degradación silenciosa de
  determinismo SOBREVIVE acotado; el candidato `sim_consumer.unpinned_replay`
  debug-log sigue gated (no aplicado — NO-GIT / corrección de código =
  operador).

### 16.2 Cambios aplicados (todos con marcador `// WO-02c-FIX-UNPINNED`)

1. §10.2 (:291 aprox.) — "sin log ni observación que lo declare" →
   "documentado en el doc-comment (consumer.rs:768-775) pero sin trazabilidad
   runtime (log/métrica) que lo declare" + nota de acote.
2. §12.2 hallazgo 2 (:473 aprox., texto del fixer G2 en este mismo
   entregable) — "SIN log" → acote equivalente con cita del doc-comment.
3. Espejo en `WO-02c-verify-VERIFY.md` §2 hallazgo 1 (:42-47) — corrección
   in situ con marcador (mismo precedente que FIX-G4/TALLY en ese archivo).

### 16.3 Contaminación residual (documentada, NO tocada)

- `WO-02c-FIX-G2-20260917.md:64` repite "sin log" — reporte histórico del
  fixer G2; se preserva como registro inmutable de su salida (disciplina
  append-only entre pares). Lectores: cruzar con esta errata.
- La matriz §8 (:227) NO contiene la sobrestimación (solo describe el
  fenómeno y el candidato debug-log) — sin cambio.

### 16.4 Verificación del fix

- Post-edit grep en ambos archivos: "SIN log ni observación" = 0 ocurrencias
  vivas; "SIN log" = 0 vivas en §10.2/§12.2 y en WO-02c-verify-VERIFY.md §2
  (queda SOLO la cita histórica dentro del propio marcador de corrección y
  el eco en el reporte de G2 declarado en §16.3).
- Cero git/cargo/npm/build/VPS/HTTP (0/5 requests). Archivos tocados:
  `02c-SIM-STACK-SELECTOR.md` + `WO-02c-verify-VERIFY.md` (ambos WO-02c,
  reportes del gang — no código de producción). Cero .rs tocados.
