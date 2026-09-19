# 02d — FICHAS: relays-client (terminus §34.3) + shared-rs (tipos canónicos)

> WO-02d · rust-topology-engineer (PhD) · 2026-09-17 · READ-ONLY (§32/§33).
> CERO ejecución de cargo/npm/build (restricción del board), cero git, cero VPS-mutación.
> Lexicon OMEGA: TLS (Temporal Liquidity Superposition), Holonomic Loop Resolution,
> Topological Yield, Decoherencia de Estado, Variedad de Liquidez.
> Formato ficha: ruta | firma entrada→salida | misión | op_origen | dependencias |
> muta/puro | prueba (file:line) | destino preliminar.

## 0. Sincronía de mesa redonda (estado al inicio de este WO)

- Leído: `GOAL-WORKORDERS.md` (board) y `02-VPS-REMAP-20260917.md` (orquestador).
- **Reportes de pares 02a/02b/02c: NO presentes todavía** en
  `audits/first-understand-20260917/` al momento de escribir (FAIL-HONEST). Donde
  este reporte toca upstream (searcher-rs, sim stack) cita file:line propio, no
  fichas de pares. Cuando 02a/02b publiquen, verificar que su grafo de producción
  de `arbx:validated_plan` / `arbx:opps:simulated` coincida con §4 abajo.
- 01-INVENTARIO.md también ausente del dir (el board lo marca DONE con evidencia
  LOC; ver `lines-per-file.txt` en el mismo dir — conteos usados aquí provienen de
  `wc -l` propio, reproducible).

## 1. HALLAZGO CRÍTICO — default-deny y "MainnetRefused" (§34.3)

**El control existe y es fuerte, pero su nombre doctrinal NO existe en el código.**

| Afirmación §34.3 (CLAUDE.md) | Realidad en código | Evidencia |
|---|---|---|
| "default-deny (`ARBX_LIVE_EXEC_ENABLED != "true"`)" | ✅ EXACTO. `enabled: enabled == Some("true")` — solo el string exacto `"true"` habilita; `"TRUE"`, `"1"`, `"true "` quedan OFF | `backend/relays-client/src/live_exec_policy.rs:37`; test `exact_true_only` :86-89 |
| "`live_exec_policy` actualmente PHYSICALLY REFUSES mainnet (chain_id=1)" | ⚠️ VERDAD PARCIAL. Por DEFECTO sí (allowlist default = solo Sepolia 11155111, mainnet cae en `ChainNotAllowed`), pero el código SOPORTA activar mainnet explícito (`ARBX_LIVE_EXEC_CHAINS=1`) y un TEST lo exige | default: `live_exec_policy.rs:3` (`DEFAULT_LIVE_CHAINS = &[11_155_111]`) + test `enabled_default_is_sepolia` :92-96; soporte explícito: test `explicit_mainnet_is_supported` :71-76 y `bundle_builder.rs:433-434` |
| Variante `MainnetRefused` | ❌ NO EXISTE como variante Rust. El enum es `LiveExecDenied { NotEnabled, ChainNotAllowed }` | `live_exec_policy.rs:6-11`. `MainnetRefused` solo aparece en COMENTARIOS TS: `backend/api-server/src/routes/control-board.ts:94,173,742` y `services/control-board-drift.ts:90` |

**Lectura PhD:** la materialización física del rechazo mainnet es la COMBINACIÓN
default-deny + allowlist-default-Sepolia, reforzada en DOS puntos de enforcement
(boot y pre-firma). La narrativa "MainnetRefused" es drift terminológico
doctrina↔código (heredada de la api-server TS). El efecto de seguridad que §34.3
quiere ("sin `ARBX_LIVE_EXEC_ENABLED=true` + cadena explícita, no hay broadcast")
SÍ se cumple; lo que NO se cumple es la letra "physically refuses mainnet", porque
el diseño actual es allowlist explícito por cadena, no blacklist de chain_id=1.
Clasificación: CANONICAL_REPO (file:line arriba). Propuesta de remediación
documental (NO aplicada, gated): ver `WO-02d-DESIGN.md` §D1.

### Puntos de enforcement exactos (ambos INTOCABLES, solo documentados)

1. **Boot**: `main.rs:172-188` — si `live_mode` (paper off) y
   `policy.assert_broadcast_allowed(chain_id)` falla → `anyhow::bail!` (proceso no arranca).
2. **Pre-firma (primera statement de `build_and_sign`)**: `bundle_builder.rs:57-59` —
   `LiveExecPolicy::from_env().assert_broadcast_allowed(opp.chain_id)` →
   `BuildError::LiveExecDenied`. Reforzado por tests `bundle_builder.rs:416-434`.

Default-deny adicional (capa modo, distinta de la capa allowlist):
- Paper default ON: `shared-rs/src/config.rs:147-149` (`default_paper_mode() -> true`)
  y `shared-rs/src/paper_mode.rs:66-74` (`PaperModeState::default` = enabled **true**,
  "Safe default: paper mode ON").
- SECURE_BOOT audit A2: `main.rs:141-162` — paper off exige
  `ARBX_SIMULATOR_V2_READY == "true"` o el proceso no bootea.
- Spawn: `consumer_spawn.rs:29-39` — consumer solo nace con DB+RPC+(signer∨paper).

## 2. FICHAS relays-client (19 archivos, 5.8 K LOC aprox.)

### F-01 · live_exec_policy.rs (97 LOC)
- ruta: `backend/relays-client/src/live_exec_policy.rs`
- firma: `from_env() -> Self` / `from_raw(Option<&str>, Option<&str>) -> Self` /
  `assert_broadcast_allowed(chain_id: u64) -> Result<(), LiveExecDenied>`
- misión: switch de modo del terminus §34.3 (PAPER_SHADOW vs TESTNET/LIVE por allowlist).
- op_origen: **multi-dex total** — no filtra por estrategia; filtra por chain.
- deps (grafo inverso grep): `main.rs:173`, `bundle_builder.rs:57` (únicos dos call sites).
- muta/puro: **PURO** (lee env; sin I/O). Malformación de `ARBX_LIVE_EXEC_CHAINS`
  NUNCA expande la allowlist (parse `Option<Vec<_>>` → vacío; test :78-84).
- prueba: :3 (default Sepolia), :37 (exact-"true"), :41-52 (assert), tests :59-96.
- destino: **ACTIVO** (núcleo del terminus; congelado por §34.3).

### F-02 · main.rs (551 LOC) — boot del terminus
- firma: `#[tokio::main] main() -> anyhow::Result<()>`; handler `execute_handler(State, Json<Value>) -> impl IntoResponse`.
- misión: boot + `/execute` HTTP (puerto `RELAYS_PORT` default 3005, :448-451).
- flujo: killswitch+papermode Redis (:120-127) → SECURE_BOOT A2 (:141-162) →
  policy enforcement boot (:172-188) → signer (:190-202) → HttpRpcPool + health loop
  (:219-257) → PgPool opcional (:266-295) → multi-relay pool (Flashbots DB/env,
  BloXRoute, Titan; :319-416) → settlement worker (:425-427) → SubmitEngine (:428-439)
  → consumer spawn (:463-539).
- op_origen: multi-dex.
- muta: **MUTA** (binds, spawns); handler puro-decision.
- 501 sin signer y sin paper: :80-97 (`NotImplementedPayload`, sprint "S5").
- destino: **ACTIVO**.

### F-03 · submit_engine.rs (1537 LOC) — orquestador del terminus
- firma: `SubmitEngine::execute(&self, opp: &Opportunity) -> ExecutionResult`.
- misión: única ruta de firma/broadcast del sistema (§34.3: "el ÚNICO binario que
  puede firmar y broadcast").
- secuencia (file:line):
  1. **R-0001**: REJECTED nunca tradea, primera statement, mode-invariant :92-112
     (regla espejo: `searcher-rs/src/persistence.rs:53` `status_from_rejection_reason`).
  2. Resolución paper per-chain :146-147 (`arbx:papermode:<chain_id>`, B0.2).
  3. Live sin DB o sin simulador privado → drop `live_requires_database_and_private_simulator` :148-150.
  4. **Admisión live** (solo !paper): `execution_admission::refresh` + `economics` :152-180.
  5. Paper sin signer → `paper_trade_runs`, jamás broadcast :188-217.
  6. Checklist 12 pasos :227-384 (`PaperModeActive` = NotSubmitted no-fatal :334-371).
  7. ValidatedPlan fail-CLOSED (key ausente/Redis err/parse err → Dropped) :442-484.
  8. `build_and_sign` :487-522 (cap de principal → Dropped explícito :500-515).
  9. No-submit sim (3 wire-schemas, cero egress) :543-557.
  10. Short-circuit paper (post-build, pre-broadcast) :560-596.
  11. `eth_callBundle` re-sim fail-CLOSED (abort en error de endpoint) :598-748;
      EWMA de relay fee a Redis :682-728 (key `arbx:relay_fee_ewma:<chain>:<strategy>`).
  12. Broadcast multi-relay :757-812; pending_tx CODE-4 SET/DEL 180s :814-851;
      inclusión + `settlement_accounting::reconcile_and_publish` :832-923.
- op_origen: multi-dex (restricción final por `plan_validation::supports_strategy`, F-06).
- deps: consume `shared_rs::{contracts, config, killswitch, paper_mode, pre_execute_checklist, rpc_failover}`,
  `prioritization_spine::ValidatedPlan`, sim-core/simulator-v2 vía execution_admission.
- muta: **MUTA** (Redis, PG, red).
- destino: **ACTIVO**.

### F-04 · bundle_builder.rs (437 LOC)
- firma: `build_and_sign(opp, plan, signer, provider, nonce_mgr, max_value_eth, target_block_offset, priority_fee_gwei) -> Result<SignedBundle, BuildError>`.
- misión: firmar SOLO el calldata exacto, acotado (bound) y canónico aprobado por simulación.
- gates internos (en orden): policy §34.3 :57-59 · estrategia soportada :60-62 ·
  signer chain/addr match :63-68 · provider chain match :69-75 · resolución del
  ejecutor TLS `resolve_flashloan_executor_address` :76-77 (env
  `FLASHLOAN_EXECUTOR_<chain_id>`, fail-closed, `shared-rs/src/chains.rs:699-714`) ·
  binding :78-89 · **cap de principal** :90-107 (default `max_value_eth`, override
  env `ARBX_LIVE_PRINCIPAL_CAP_<chain>_<token_in-hex>`, comparación U256 exacta) ·
  head fresco (block_number+hash+timestamp idénticos al binding) :108-118 ·
  offset==1 (fresh simulation requires next block) :119-121 · funding :122-124 · gas :125+.
- op_origen: multi-dex restringido (delega a F-06).
- destino: **ACTIVO**.

### F-05 · execution_admission.rs (212 LOC) — el "análisis→ejecución" upgrade
- firma: `refresh(opp, original: &ValidatedPlan, caller) -> Result<ValidatedPlan>` ·
  `economics(opp, plan, redis) -> Result<Economics{net, gas, slippage_pct}>`.
- misión: "Only a fresh execution against real permissions upgrades an analysis
  carrier to a broadcast-authorizing plan" (:1-2). Si el binding del plan es fresco
  (<30 s, mismo head, sin overrides, mismo caller) lo reusa; si no, RE-SIMULA con
  `simulator_v2::SimulatorV2` sobre head real (semáforo 4, timeout 20 s,
  `require_positive_net_profit`, sin storage cheats) :41-137.
- economics: oracle EIP-1898 (asset + native + decimals) → gas USD, net USD;
  regla **three-gas** (`admitted >= gas*3`) :185-188; **cap 2 % del capital**
  (`trading_config.capital_usd / 50`) :193-201.
- op_origen: multi-dex. deps: `shared_rs::oracle_snapshot`, `trading_config`.
- destino: **ACTIVO** (gate económico del terminus live).

### F-06 · plan_validation.rs (481 LOC) — admisor de estrategia (op_origen REAL)
- firma: `supports_strategy(&StrategyKind) -> bool` :17-32 ·
  `validate_calldata(&ValidatedPlan) -> Check<()>` :53-173 · `validate_binding(...)` :175+.
- **`supports_strategy` define el op_origen del terminus**: admite EXACTAMENTE
  `dex_arb`, `flashloan_arb`, `triangular` (Holonomic Loop Resolution) + 8 cartuchos
  de la familia MEV-01: `mev_01_001_dex_dex_arbitrage`, `mev_01_002_cross_pool_arbitrage`,
  `mev_01_015_two_leg_arbitrage`, `mev_01_016_triangular_arbitrage`,
  `mev_01_017_quadrangular_arbitrage`, `mev_01_018_n_leg_cyclic_arbitrage`,
  `mev_01_019_multi_hop_arbitrage`, `mev_01_008_amm_amm_arbitrage`.
  ⇒ **op_origen del terminus = multi-dex, restringido a familia MEV-01 + 3 familias base**.
  El mapeo fino operador(1..32)↔cartucho vive en searcher/math-engine (mitades 02a/02b,
  reportes aún no publicados — GAP de cross-check).
- `validate_calldata`: doble-encode ABI canónico (rechaza trailing data) :45-51,
  selector externo `REQUEST_FLASH_LOAN_SELECTOR` (TLS wrap) :68-77, selector interno
  `EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR` :79-81, swap `0x38ed1739`
  (swapExactTokensForTokens) :127-129, exactamente 2 routers :104-109,
  paths 2-7 tokens sin repetición :113-125, hops totales 2-7 :169-171,
  cotizaciones/slippage acotados por el binding :153-167.
- destino: **ACTIVO**.

### F-07 · consumer.rs (464 LOC)
- `Consumer::run()` — XREADGROUP `arbx:opps:simulated` (grupo `relays-client-g0`) →
  `engine.execute` → `persist_execution` → XACK; DLQ con retry-counter
  `arbx:opps:simulated:retries:<id>` (default 3 reintentos) :21-103; emite a
  `arbx:opps:executed` (:24). op_origen: multi-dex. **ACTIVO**.

### F-08 · consumer_spawn.rs (79 LOC) — política de spawn paper/live (§ F-01 default-deny complementario). Puro. :29-39. **ACTIVO**.

### F-09 · signer.rs (231 LOC)
- `Signer::from_env(chain_id)` lee `FLASHBOTS_SIGNER_KEY` → `LocalWallet`; la clave
  cruda jamás se almacena/loguea; Debug redactado :56-63; header Flashbots
  `<addr>:<sig>` :38-52; EIP-155 chain_id anti-replay. **ACTIVO** (fail-closed: sin
  signer no hay broadcast, solo paper).

### F-10 · multi_relay.rs (407) + relay_flashbots.rs (758) + relay_bloxroute.rs (167) + relay_titan.rs (158) + relay_catalog.rs (109)
- `RelayBackend` trait (multi_relay.rs:58); broadcast paralelo `join_all`, la chain
  es fuente canónica de verdad (inclusion at most once). Flashbots: `eth_sendBundle`
  + `eth_callBundle` (BE-05 re-sim) + auth header firmado. Catálogo de relays
  **operator-owned en PG** (migración 013, tabla `relays`), sin hardcodes
  (relay_catalog.rs:1-10; orden DB → env `FLASHBOTS_RELAY_URL` → excluido, main.rs:303-372).
  BloXRoute/Titan con credenciales de env. **ACTIVO**.

### F-11 · relay_no_submit_sim.rs (545 LOC) — §32/§33 encarnado
- `validate_and_discard(SimBundleParams) -> NoSubmitReport` (:292). 100 % local,
  cero egress (la ausencia de imports de cliente ES la prueba, :1-14). **ACTIVO**.

### F-12 · soporte
- `nonce_manager.rs` (86): (chain,addr)→nonce in-memory, semáforo por dirección,
  refresh vía pool `with_retry`. **ACTIVO**.
- `tracker.rs` (73): `wait_for_inclusion` poll de receipt (alloy 1.0). **ACTIVO**.
- `persistence.rs` (337): INSERT `executions` (ON CONFLICT tx_hash DO NOTHING),
  UPDATE `opportunities.status` SOLO desde `('simulated','executing')` :55-66,
  UPSERT `relay_scores` ventana 1 h; `insert_paper_trade_run` :141+. **ACTIVO**.
- `settlement_accounting.rs` (965): contabilidad basada en receipts finalizados
  ("Expected profit is never a substitute for a receipt", :1); cola
  `arbx:accounting:pending`; decodifica eventos del ejecutor TLS, precios al
  block-hash del receipt; alimenta `shared_rs::settlement_risk` (observaciones
  empíricas). `start_worker` :698, `reconcile_and_publish` :563. **ACTIVO**.

## 3. FICHAS shared-rs (módulo-tipo, 24 archivos .rs en src/ — 25 crate-wide con tests/oracle_rpc_selection.rs)

> // WO-02d-FIX-G1 (2026-09-17, fixer gang ronda 1 — DEFECTO 1 del cross-exam
> `WO-02d-verify-CROSS-EXAM.md` §2): el encabezado decía "33 archivos" — cifra NO
> reproducible. Conteo canónico recomputado: `find shared-rs/src -name "*.rs" | wc -l`
> = **24** (incluye `oracle_snapshot/configured_rpc.rs` y `lib.rs`); +1 test fuera de
> src (`tests/oracle_rpc_selection.rs`) = 25 crate-wide. La tabla abajo tiene 19 filas
> que cubren 23 de los 24 archivos src (`lib.rs` solo citado en el preámbulo, :11).
> Errata completa: §ERRATA-01 al final del archivo.
> // WO-02d-FIX-GRAN (2026-09-17): post-split de la fila agrupada, la tabla tiene
> **22 filas** (19 + 3 por el desglose db_pool/health/logging/metrics); cobertura
> de archivos SIN cambio (23/24 src). Ver §ERRATA-02.

> Consumido por searcher-rs, sim-ctl, relays-client, recon (lib.rs:11). El terminus
> importa de aquí: config, contracts, health, killswitch, logging, metrics,
> paper_mode, rpc_failover, db_pool, chains, oracle_snapshot, trading_config,
> pre_execute_checklist, settlement_risk (grep main.rs:39-48, submit_engine.rs:19-30).

| Módulo (LOC) | Firma/esencia | Misión | muta | Prueba | Destino |
|---|---|---|---|---|---|
| `contracts.rs` (203) | `Opportunity`, `StrategyKind(String)` newtype (5 familias + **cada cartucho .rhai ES un strategy_kind**, :6-42), `SimulationResult`, `ExecutionResult/ExecutionStatus` (7 estados; `NotSubmitted` = paper/pre-submit, :131-141), `ReconReport`, `NotImplementedPayload` | **tipo canónico que consume todo el hot-path** (searcher produce → sim → selector → terminus ejecuta) | puro (serializable) | :44-96 `Opportunity` con `net_expected_profit_usd` (:74-75), `rejection_reason` (:85-86), `cartridge_id` (:92-93); doc gross≠net :57-73 | **ACTIVO** |
| `config.rs` (513) | `AppConfig::load()` TOML+env, validación opcional JSON-Schema; `ExecutionCfg` :121-146 (`max_value_eth` default **1.0 ETH** :156-158; `paper_mode` default **true** :147-149; `priority_fee_gwei` default 2.0 WO-04 2026-09-06 :165-168) | config SSOT | boot-muta | :122-168 | **ACTIVO** |
| `paper_mode.rs` (214) | `PaperModeClient::is_enabled_for_chain(chain)`; keys `arbx:papermode:<chain>` + fallback legacy 30 días desde 2026-05-13; default-ausente = paper ON | toggle dinámico de modo PAPER_SHADOW | Redis-RW | :24-37, :66-74, :101-115 | **ACTIVO** |
| `killswitch.rs` (130) | `KillSwitchClient::is_enabled()`; `arbx:killswitch` + pub/sub `arbx:killswitch:changes`, cache 1 s | kill-switch <10 ms doctrinal | Redis-R | :15-16 | **ACTIVO** |
| `pre_execute_checklist.rs` (1219) | `pre_execute_checklist(&mut PreExecuteContext)` — 12 checks secuenciales fail-closed (1 kill-switch :260, 2 paper per-chain :281, 3 chain activa :315, 4 trading_config+min_profit_usd :329, 5 RPC activa :345, 6 gas frescura ≤30 s :361, 7 net>floor :397, 8 tokens allowlist 2-tier :414, 9 fábricas activas :478, 10 slippage≤max :504, 11 mempool limpio `arbx:pending_tx:<addr>` :538, 12 circuit breaker off :553) | gate de seguridad unificado pre-broadcast | R Redis+PG | índice arriba | **ACTIVO** |
| `trading_config.rs` (1116) | `TradingConfigClient::state(chain)` — SSOT PG `trading_config` → cache Redis `arbx:trading_config:<chain>` → pub/sub; `capital_usd: f64` :183 (fuente REAL de objetivo_usd runtime), `simulation_capital_usd` :166 | parámetros operador hot-reload | Redis-R | :1-11, :183 | **ACTIVO** |
| `rpc_failover.rs` (1405) | `HttpRpcPool::from_env(chain)` (CSV `name=url`), health 15 s, drift >2 bloques/60 s, circuit breaker 5 err/60 s→Open 30 s, EWMA 0.30, rate-limit sticky 60 s/cooldown 120 s | disciplina arbx-rpc-failover | RW estado | :45-83, :212+ | **ACTIVO** |
| `chains.rs` (1239) | catálogo estático routers/WRAPPERS (WETH/USDC/... mainnet+L2+testnet), stables per-chain; `resolve_flashloan_executor_address(chain)` :699 — env `FLASHLOAN_EXECUTOR_<chain>`, fail-closed Missing/Invalid/Zero | direcciones de protocolo auditadas | puro | :699-714 + tests :1189-1235 | **ACTIVO** |
| `flashloan_math.rs` (278) | fees/capacidad TLS exactos a un estado EIP-1898 (AaveV3 percentMul half-up; Balancer V2 mulUp; ERC-3156 NO intercambiable) | matemática TLS mode-invariant §34.1 | puro | :1-6 | **ACTIVO** |
| `oracle_snapshot.rs` (232+configured_rpc 110) | `OracleRpc` lecturas de feeds USD pineadas EIP-1898; URLs nunca se filtran en errores | precios exactos sim+accounting | R red | :1-3; configured_rpc.rs:1-4 | **ACTIVO** |
| `candidates.rs` (528) | `OpportunityCandidate` + `RouteMetadata` (multi-hop completo para el encoder REVM) | enriquecimiento upstream (searcher/sim) | puro | :1-9 | **ACTIVO** (mitad 02a/02c) |
| `risk_ledger.rs` (423) | core PURO de circuit-breakers de drawdown/revert/gas sobre historia rolling (shadow, capital=0) | risk control-plane | puro | :1-5 | **ACTIVO** |
| `settlement_risk.rs` (277) | `RealizedObservation` + cohortas (chain+strategy+asset) SOLO de receipts finalizados | objetivos empíricos | puro | :1-4 | **ACTIVO** |
| `sim_taxonomy.rs` (228) | taxonomía STRUCTURAL/ECONOMIC/MARKET de fallos de sim (S4-03 no-contaminación) | etiquetado honesto | puro | :1-7 | **ACTIVO** |
| `cred_rotation.rs` (640) | rotación titular→fallback de CSV creds rpc_http/rpc_ws (proyección Redis → env) | RunFullSyncCycle F4 | puro | :1-8 | **ACTIVO** |
| `price_oracle.rs` (611) | valoración USD per-token para el spine (3 tiers, fail-honest) — reemplaza BUG-2 | evaluación económica | puro+cache | :1-10 | **ACTIVO** |
| `token_identity.rs` (420) | identidad runtime = (chain_id,address); símbolos = metadata NUNCA gate (ARBX-0018) | anti-spoofing de tokens | puro | :1-9 | **ACTIVO** |
| `tokens.rs` (375) | catálogo estático de tokens (decimales/símbolo) auditado en código | metadata | puro | :1-8 | **ACTIVO** |
<!-- // WO-02d-FIX-GRAN (2026-09-17, fixer gang ronda 1 — obs. 4.1 del cross-exam
     `WO-02d-verify-CROSS-EXAM.md` §4.1): la fila agrupada citaba solo ":1-8 c/u"
     (headers), incumpliendo el gate del board "línea exacta". Desglosada en 4 filas
     con citas concretas re-verificadas contra el fuente + GAP de prueba unitaria
     declarado (0 `#[cfg(test)]` en los 4 archivos). Ver §ERRATA-02. -->
| `metrics.rs` (481) | `init_metrics()` :431-464 fuerza los 30 collectors `Lazy` + `SERVICE_UP.set(1)` :463; `metrics_handler()` :466-481 (REGISTRY.gather → TextEncoder → 200, err→500 :470-475); `record_inclusion()` :419-429; `REGISTRY` :9; familias canónicas: HTTP :11-36, OPPORTUNITIES :38-49, SIMULATIONS :51-62, SIM_FUNDING (SIM-FUND-01) :64-80, EXECUTIONS :82-90, KILLSWITCH gauge :92-96, searcher :104-180, RPC failover (G-RPC-1) :182-261, cred rotation (F4) :263-296, bundle inclusion (N7) :298-334, SIMWIRE-02 PEL :346-409 | Prometheus canónico cross-svc | puro (registries en memoria) | líneas arriba; re-export `lib.rs:42` (`init_metrics, metrics_handler`); terminus lo consume vivo: `relays-client/src/main.rs:45` import, `:118` init, `:455` mount · **GAP prueba unitaria: 0 `#[cfg(test)]`** | **ACTIVO** |
| `db_pool.rs` (89) | `PoolConfig::from_env()` :39-67 (5 env vars, defaults 8/1/5 s/600 s/1800 s), `options_with_timeouts()` :75-82, `connect_pool()` :86-89 | pool PG con timeouts | boot-muta (abre conexiones) | líneas arriba; re-export `lib.rs:38`; consumidores grep: relays-client/searcher-rs/sim-ctl/token-enricher/recon (mains) · **GAP prueba unitaria: 0 `#[cfg(test)]`** · doc-drift :14-16 → §ERRATA-02.2 | **ACTIVO** |
| `health.rs` (58) | `ServiceInfo` :8-26, `build_health_router()` :36-58 (`/health` JSON ok+uptime_s :38-56; `/metrics`→`crate::metrics::metrics_handler` :57) | router /health+/metrics montable | puro | líneas arriba; re-export `lib.rs:39`; terminus: `main.rs:42` import, `:455` mount · **GAP prueba unitaria: 0 `#[cfg(test)]`** | **ACTIVO** |
| `logging.rs` (28) | `init_tracing()` :7-28 (EnvFilter default-env :8; JSON layer :10-18; boot event `service.boot` :26) | tracing JSON estructurado | boot-muta (subscriber global) | líneas arriba; re-export `lib.rs:41`; terminus: `main.rs:44` import, `:117` call · **GAP prueba unitaria: 0 `#[cfg(test)]`** | **ACTIVO** |

## 4. Grafo inverso — quién produce lo que el terminus consume

| Recurso que consume relays-client | Productor (evidencia grep) | Nota de mesa |
|---|---|---|
| `Opportunity` (tipo) | definido shared-rs/contracts.rs:44; instanciado por searcher-rs (scanner) | 02a debe confirmar el punto de construcción |
| Stream `arbx:opps:simulated` | `sim-ctl/src/consumer.rs:49` (`STREAM_OUT`, "if passed → XADD", :18) | 02c: fase simular/seleccionar |
| Key `arbx:validated_plan:<id>` (TTL NO uniforme — // WO-02d-FIX-G2 2026-09-17) | **escritores**: `searcher-rs/src/scanner.rs:2458` (`set_ex(..., 300u64)`, key :2449, fail-SOFT) y `searcher-rs/src/candidate_simulation.rs:585` (`.arg(60)`, pipe atómico :579-592, llamado desde `opportunity_emitter.rs:367`). `sim-ctl/src/canonical_plan_consumer.rs:41` (`CARRIER_KEY_PREFIX`) define el esquema de key y **CONSUME** (GET :81-84); su header :4 cita "TTL 300s" refiriendo al productor scanner — NO escribe | doble productor con TTLs distintos: **300 s (scanner) / 60 s (candidate_simulation)** → ventana detect→broadcast de la ruta candidate = 60 s. Consumidores fail-closed: relays-client `submit_engine.rs:442-484` (:161 GET, :445) y sim-ctl `canonical_plan_consumer::fetch` (mismo key, misma ventana) |
| `ValidatedPlan` (tipo) | crate `prioritization_spine` (fuera del claim 02d; ficha en WO-03/04) | — |
| `capital_usd` (objetivo runtime) | PG `trading_config` vía `TradingConfigClient` (trading_config.rs:183) | ver §5 |
| Catálogo de relays | PG tabla `relays` (migración 013), CRUD en api-server | — |

**Cross-check con par 02a** (publicado durante mi cierre, `WO-02a-DESIGN.md`): su
ficha marca `cartridge/runner.rs:290` como "única ruta que emite Opportunity desde
cartucho" — CONSISTENTE con mi grafo inverso (el terminus consume el tipo canónico
de shared-rs/contracts.rs:44; 02a no reporta otro constructor fuera de scanner).
Sin contradicciones abiertas entre 02a y 02d a la fecha. Su §12 también confirma
el mismo régimen read-only (cero cargo/git).

## 5. objetivo_usd — solo fuentes reales (RULE 00)

- **Fuente runtime 1**: `trading_config.capital_usd` (PG per-chain, hot-reload). El
  terminus live lo usa como `capital_usd / 50` = **cap 2 % del capital por
  oportunidad** (execution_admission.rs:193-201). Valor numérico = dato de operador
  en DB — NO leíble desde el repo (GAP si se pide cifra).
- **Fuente runtime 2**: `ExecutionCfg.max_value_eth` default **1.0 ETH** de principal
  TLS (config.rs:156-158) + override env `ARBX_LIVE_PRINCIPAL_CAP_<chain>_<token>`
  (bundle_builder.rs:90-101) — cap de principal por cadena/token, comparación U256 exacta.
- **Fuente runtime 3**: floor de net profit `trading_config.min_profit_usd`
  (checklist check 4/7) y regla three-gas (execution_admission.rs:185-188).
- **DOCTRINA, NO runtime** (distinguido como ordena el charter): canary §34.5
  "capital en riesgo ≤ $350, principal TLS 5 WETH" vive SOLO en CLAUDE.md §34.5;
  no existe literal $350 ni 5 WETH en relays-client/shared-rs (grep negativo).
  Clasificación: doctrinal. Si el operador quiere el canary como control técnico,
  hoy se materializaría vía `ARBX_LIVE_PRINCIPAL_CAP_*` + `capital_usd` (propuesta en DESIGN).
- **op_origen**: ver F-06 (multi-dex restringido MEV-01 + 3 base). Sin mapping
  operador 1..32→path dentro del terminus (upstream 02a/02b).

## 6. Estado preliminar (board /goal)

- relays-client: **ACTIVO** íntegro (19/19 módulos con ruta+firma+prueba; ninguno MUERTO).
- shared-rs: **ACTIVO** (23/24 archivos src documentados a nivel módulo-tipo en la
  tabla §3; `lib.rs` citado en el preámbulo :11; `tests/oracle_rpc_selection.rs`
  fuera de src, sin ficha). // WO-02d-FIX-G1 (2026-09-17): decía
  "33/33 módulos documentados" — ver §ERRATA-01.
- GAPs honestos:
  1. `docs/EXECUTION_MODES_DOCTRINE.md` (can declarado §34 y en este charter) **NO
     existe** en el checkout (glob `docs/**/*EXECUTION*` y `*MODE*` negativos; solo
     `docs/omega/OMEGA_EXECUTION_BACKLOG.md` y `ANTIGRAVITY_REMOTE_EXECUTION_REPORT.md`). GAP.
  2. `MainnetRefused` no existe como variante Rust (drift doctrina↔código, §1).
  3. Cross-check con 02a/02b/02c pendiente (aún no publicados al escribir esto).

## VERIFICACIÓN (WO-02d-verify · ecc:security-reviewer · 2026-09-17)

> Dictamen: **PASS (con 2 correcciones menores de cita, sin impacto de seguridad)**.
> Método: re-apertura de TODAS las citas file:line del terminus + greps dirigidos.
> CERO cargo/git/VPS (restricción del board WO-02); tests verificados por LECTURA
> de su código, no por ejecución (FAIL-HONEST). Detalle completo:
> `WO-02d-verify-VERIFY.md`.

**Confirmado con re-apertura (muestra crítica):**
- `live_exec_policy.rs` (97 l, leído completo): :3 default Sepolia, :37
  `enabled == Some("true")` exacto, :41-52 assert, :6-11 enum sin `MainnetRefused`,
  tests :59-96 — TODOS EXACTOS.
- Enforcement dual: `main.rs:172-188` (bail :184-186) y `bundle_builder.rs:57-59`
  (primera statement de `build_and_sign`) — EXACTOS.
- `MainnetRefused` grep backend = exactamente las 4 líneas TS citadas
  (`control-board.ts:94,173,742`, `control-board-drift.ts:90`). Drift CONFIRMADO.
- Barrido §32/§33 (superficie firma/broadcast): CERO `send_bundle`/`eth_sendBundle`
  fuera de relays-client; searcher-rs tiene OMEGA SEAL capital-key lockout
  (`searcher-rs/src/main.rs:303-343`: panic si cualquiera de 8 keys de capital está
  poblada) y sed-core declara invariante negativa (`sed-core/src/connectors/mod.rs:20-22`).
  La ficha NO documenta ningún path de firma alcanzable fuera del terminus —
  verificado que no existe en backend.
- Grafo inverso `arbx:validated_plan` (grep backend completo): productores =
  exactamente los 3 listados en §4 (sim-ctl `canonical_plan_consumer.rs:41` +
  searcher `scanner.rs:2449` + `candidate_simulation.rs:582`); consumidor =
  `submit_engine.rs:161,445`. Sin productores no listados.
  **[// WO-02d-FIX-G2 2026-09-17 — precisión: escritores = 2 (scanner.rs:2458 `set_ex 300`
  + candidate_simulation.rs:585 `EX 60`); `canonical_plan_consumer.rs:41` es CONSUMIDOR
  (GET :81-84) que define el prefijo del key — NO escribe. Ver §ERRATA-03.]**
- objetivo_usd RULE 00: grep negativo de `$350`/`5 WETH`/canary en relays-client
  y shared-rs (matches WETH = constantes de dirección; `2500.0` = fixtures de
  test de price_oracle, no canary). Distinción doctrina↔runtime CORRECTA.

**Corrección 1 (cita, §4 cross-check con 02a):** este reporte atribuye a 02a la
frase "única ruta que emite Opportunity desde cartucho" en `cartridge/runner.rs:290`.
Lo que 02a §6.1/§6.3 realmente dice: la ruta de emisión única es
`cartridge_boot.rs:964` (`active_evaluate_and_emit`); `runner.rs:290` es el método
`evaluate` del engine Rhai. Sustancia consistente (una sola ruta de emisión,
mismo tipo canónico), puntero equivocado.

**Corrección 2 (completitud del drift):** `MainnetRefused` TAMBIÉN aparece fuera
de backend, en canon de skills que el operador lee:
`.claude/skills/arbx-live-engineering/references/biblioteca/22-golive-playbook.md:44`,
`20-system-integration-patterns.md:278`, `14-onchain-execution-contracts.md:430`
(+ auditorías antiguas 2026-09-06/08). La corrección documental D1 debería
contemplar la biblioteca, no solo api-server.

**Sin hallazgos que sugieran tocar el default-deny.** Ninguna edición a código
(o ni a este archivo fuera de este adendum).

## ERRATA-01 · conteo "33 archivos shared-rs" → 24 (// WO-02d-FIX-G1 · 2026-09-17)

> Fixer gang ronda 1, gap G-1 del charter (DEFECTO 1 del cross-exam
> `WO-02d-verify-CROSS-EXAM.md` §2). Append-only: el texto original queda registrado
> arriba con el marcador in situ.

- **Claim corregido:** §3 titular "33 archivos" y §6 "33/33 módulos documentados".
  El board (`GOAL-WORKORDERS.md`) propagó "52 fichas: 19 relays-client + 33 shared-rs".
- **Realidad reproducible (recomputada por este fixer, no heredada):**
  - `find backend/shared-rs/src -name "*.rs" | wc -l` = **24** (lista completa:
    candidates, chains, config, contracts, cred_rotation, db_pool, flashloan_math,
    health, killswitch, lib, logging, metrics, oracle_snapshot,
    oracle_snapshot/configured_rpc, paper_mode, pre_execute_checklist, price_oracle,
    risk_ledger, rpc_failover, settlement_risk, sim_taxonomy, token_identity, tokens,
    trading_config).
  - Crate-wide: +1 (`tests/oracle_rpc_selection.rs`, fuera de src) = **25**.
  - Ninguna lectura (src-only, crate-wide, con o sin target/) produce 33.
- **Cobertura de la tabla §3 (post-fix):** 19 filas → 23 de 24 archivos src;
  `lib.rs` citado en el preámbulo (:11) sin fila propia;
  `tests/oracle_rpc_selection.rs` sin ficha. "19 relays-client" verificado correcto
  (`find relays-client/src -name "*.rs" | wc -l` = 19).
- **Origen del 33:** UNKNOWN (hipótesis: confusión con el conteo de módulos
  importados o aritmética 19+33=52 auto-consistente pero sin fuente). FAIL-HONEST:
  no se especula más allá.
- **Impacto:** cero seguridad (ninguna conclusión del §1/§4 cambia); contamina el
  censo del /goal con una cifra de cobertura no reproducible (espíritu RULE 00/R8).
- **Contaminación residual NO tocada (archivos de otros owners, documentada):**
  `WO-02a-verify-VERIFY.md:16` y `:309` ("52 fichas" — owner 02a-verify),
  `WO-02c-CROSS-EXAM.md:68` ("02d fichó los 33 shared-rs" — owner cross-exam 02c).
  `WO-02d-verify-CROSS-EXAM.md:43` cita el 33 como EVIDENCIA del defecto: se deja
  intacto por diseño (es el registro del refutador).
- **Alcance del fix:** SOLO .md (este archivo + `GOAL-WORKORDERS.md`). Cero código,
  cero git, cero cargo/npm, cero VPS, cero HTTP (0/5).
- El DEFECTO 2 del cross-exam (TTL 300 vs 60 s en §4) queda FUERA de este fix
  (gap separado del charter, pendiente de su propio fixer).

## ERRATA-03 · TTL del carrier `arbx:validated_plan:<id>` NO es uniforme 300 s —
## 300 s (scanner) / 60 s (candidate_simulation) (// WO-02d-FIX-G2 · 2026-09-17)

> Fixer gang ronda 1, DEFECTO 2 del cross-exam (`WO-02d-verify-CROSS-EXAM.md` §3,
> gap "agent-fixable" de su tabla §5). Append-only: el claim original queda
> registrado arriba; corrección in situ con marcador en la fila §4 y en el adendum
> del verify (líneas de grafo inverso). Numeración: ERRATA-02 queda reservada al
> fixer paralelo WO-02d-FIX-GRAN (la referencia `→ §ERRATA-02` de :237/:239 no
> estaba en disco al momento de este fix — fail-honest, sin colisión).

- **Claim corregido:** fila §4 "Key `arbx:validated_plan:<id>` (**TTL 300 s**)" con
  columna productor listando 3 entidades (incluida `canonical_plan_consumer.rs:41`).
- **Realidad re-verificada de primera mano (RULE 00, no heredada del cross-exam):**
  - Escritores = exactamente **2** (grep `arbx:validated_plan` en backend completo):
    1. `searcher-rs/src/scanner.rs` — key :2449, `set_ex(..., 300u64)` :2458
       (comentario :2452-2453: "TTL 300s comfortably covers the detect→broadcast
       window (longer than the 180s pending-tx backstop)"), fail-SOFT :2459-2469.
    2. `searcher-rs/src/candidate_simulation.rs` — `persist()` :572-597, pipe
       atómico SET :581-586 con `.arg("EX").arg(60)` :584-585 → **60 s**
       (escribe en el MISMO pipe `arbx:validated_economics:<id>` también EX 60,
       :587-592). Call-site: `opportunity_emitter.rs:367` (ruta de admisión de
       candidates — "plan and economic evidence become visible together",
       doc-comment :570-571).
  - `sim-ctl/src/canonical_plan_consumer.rs` = **CONSUMIDOR, no escritor**:
    `CARRIER_KEY_PREFIX` :41 define el esquema de key, `fetch()` hace GET :81-84
    (`Ok(None)` = carrier ausente/expirado :20-21, :74-75). Su header :4 cita
    "(TTL 300s)" refiriendo al productor scanner — documentación, no escritura.
  - Consumidores del key (ambos fail-closed sobre ausencia): relays-client
    `submit_engine.rs:161, :442-484` y sim-ctl `canonical_plan_consumer::fetch`.
- **Consecuencia material (hereda el cross-exam §3, correcta):** la ventana
  detect→broadcast de los planes escritos por la ruta candidate_simulation es
  **60 s**, no 300 s. Si el terminus queda >60 s detrás sobre ese productor, el
  plan expira y el fail-closed dropea (comportamiento SEGURO, tasa de drop
  esperable mayor a la descrita). El fail-closed del terminus queda INTACTO.
- **Desviación del hint del charter (documentada):** el hint proponía
  "TTL 300 s (scanner, canonical_plan_consumer) / 60 s (candidate_simulation.rs:586)".
  Rechazado en su mitad consumer: listar canonical_plan_consumer como titular del
  TTL 300 reproduciría el mismo error de clase que este fix corrige (productor vs
  consumidor). Precisión de línea adicional: el `60` vive en :585 (la cita ":586"
  del cross-exam apunta a `.ignore()` :586); el `300` de scanner vive en :2458
  (key en :2449).
- **Impacto:** precisión documental; cero cambio de seguridad (default-deny y
  fail-closed intactos). Clasificación: CANONICAL_REPO (citas file:line arriba).
- **Contaminación residual NO tocada (archivos de otros owners, documentada):**
  `WO-02d-verify-CROSS-EXAM.md` §3 (registro del refutador, intacto por diseño;
  su cita ":586" corregida aquí) · **02c-SIM-STACK-SELECTOR.md afirma "TTL 300s"
  en :30, :127, :258 y :326** (:127 acota correctamente el escritor "scanner M2
  carrier-B" pero presenta 300 s como el TTL del key sin registrar el segundo
  escritor EX 60 — mismo defecto de completitud que este ERRATA corrige en 02d;
  :258/:326 verifican la documentación del header canonical_plan_consumer.rs:4,
  exactos como citas de doc). Owner WO-02c: NO pisado, registrado aquí para su
  propio fixer (misma regla de mesa que la contaminación "33 shared-rs" del
  ERRATA-01).
- **Alcance del fix:** SOLO .md (este archivo + `GOAL-WORKORDERS.md` + reporte
  `WO-02d-FIX-G2-20260917.md`). Cero código, cero git, cero cargo/npm/build
  (restricción board), cero VPS, cero HTTP (0/5).

## ERRATA-02 · granularidad de prueba §3 (db_pool/health/logging/metrics) (// WO-02d-FIX-GRAN · 2026-09-17)

> Fixer gang ronda 1. Cierra la observación 4.1 del cross-exam
> (`WO-02d-verify-CROSS-EXAM.md` §4.1): "varias filas citan solo ':1-8' (headers)
> — p.ej. metrics.rs (481 LOC) fichado con prueba ':1-8 c/u'".

### .1 Cambio aplicado (append-only sobre la fila, texto original removido in situ)

- La fila agrupada `db_pool.rs / health.rs / logging.rs / metrics.rs` con prueba
  ":1-8 c/u" fue DESGLOSADA en 4 filas con citas file:line concretas
  re-verificadas contra el fuente (lectura completa de los 4 archivos):
  - `metrics.rs`: `init_metrics()` :431-464, `metrics_handler()` :466-481,
    `record_inclusion()` :419-429, `REGISTRY` :9, 11 bloques de familias citados
    por rango exacto (:11-36 HTTP, :38-49 OPPORTUNITIES, :51-62 SIMULATIONS,
    :64-80 SIM_FUNDING/SIM-FUND-01, :82-90 EXECUTIONS, :92-96 KILLSWITCH,
    :104-180 searcher, :182-261 RPC failover/G-RPC-1, :263-296 cred rotation/F4,
    :298-334 bundle inclusion/N7, :346-409 SIMWIRE-02 PEL).
  - `db_pool.rs`: `PoolConfig::from_env()` :39-67, `options_with_timeouts()`
    :75-82, `connect_pool()` :86-89 (defaults 8/1/5 s/600 s/1800 s).
  - `health.rs`: `ServiceInfo` :8-26, `build_health_router()` :36-58
    (ruta `/health` :38-56; `/metrics` :57 despacha a
    `crate::metrics::metrics_handler`).
  - `logging.rs`: `init_tracing()` :7-28.
  - Respaldos de consumo vivo (hint del cross-examiner confirmado):
    re-exports `shared-rs/src/lib.rs:38` (db_pool), `:39` (health), `:41`
    (logging), `:42` (metrics: `init_metrics, metrics_handler`);
    terminus relays-client `src/main.rs:42,44-45` (imports), `:117`
    (`init_tracing`), `:118` (`init_metrics`), `:455` (`build_health_router`
    merge en el router app).
- **GAP declarado (fail-honest, gate del board "o marcada GAP")**: los 4 módulos
  tienen **0 `#[cfg(test)]`** (`grep -n "cfg(test)" metrics.rs db_pool.rs
  health.rs logging.rs` = 0 hits). La columna Prueba ahora cita líneas de
  misión exactas; la cobertura por test unitario NO existe y queda marcada GAP
  en cada fila. No se fabrica cobertura (RULE 00).

### .2 Hallazgo nuevo colateral — doc-drift `db_pool.rs:14-16`

El doc-comment del módulo afirma: "Logging: on success, emits `db.connected`
[...] On timeout/error, emits `db.connect_failed`". **El módulo NO contiene
ninguna llamada de logging** (0 hits de `tracing`/`info!`/`warn!` en el
archivo). Los eventos `db.connected`/`db.connect_failed` que R6 exige son
emitidos por los CALLERS, no por el helper:
- `relays-client/src/main.rs:276` (`db.connected`) y `:281` (`db.connect_failed`);
- `searcher-rs/src/main.rs:532` y `:544`;
- `sim-ctl/src/main.rs:707-712` emite sus PROPIOS nombres
  (`sim.db_connected`/`sim.db_connect_failed` — drift de nomenclatura
  adicional vs R6);
- `token-enricher/src/main.rs:464` (`enricher.db_connected`).
Impacto: documental (R6 se cumple de facto en relays-client/searcher;
inconsistente en sim-ctl/token-enricher). Corrección del doc-comment de
`db_pool.rs` = operator-gated (requiere tocar .rs; NO aplicada — cero código
en este fix).

### .3 Alcance y no-regresión

- Archivos tocados: `02d-RELAYS-CLIENT-SHARED.md` (fila §3 + esta errata) y
  `GOAL-WORKORDERS.md` (entrada del board). Cero .rs, cero git/cargo/npm,
  cero VPS, cero HTTP (0/5 requests).
- Filas de otros módulos que TAMBIÉN citan solo headers (`cred_rotation.rs`
  :1-8, `tokens.rs` :1-8, y las ":1-N" de módulos puros pequeños) quedan
  FUERA de este charter — documentadas, NO tocadas (cambios quirúrgicos).
- Sin colisión con el fixer paralelo WO-02d-FIX-G2 (TTL §4): ediciones
  disjuntas, marcadores intactos.
