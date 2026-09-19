# WO-02a · Fichas hot-path searcher-rs — DESIGN (2026-09-17)

> Gang Omniscience · rol rust-topology-engineer (rubric ecc:rust-reviewer).
> Método: Read/Grep SOLAMENTE (restricción board — cargo PROHIBIDO, target/ compartido §36.4).
> CERO git commit/push/PR. CERO executor/wallets/broadcast (§32/§33). Lexicon OMEGA.
> Este archivo cumple la misión "02a-SEARCHER-RS.md" (nombre de entrega unificado WO-02a-DESIGN.md).

## 0. Mesa redonda — estado al iniciar

- `audits/first-understand-20260917/GOAL-WORKORDERS.md` leído completo (board, 47 líneas).
- Único reporte previo en el dir: **02-VPS-REMAP-20260917.md** (orquestador). Puntos que
  tocan mi claim: los 3 commits locales (fdb40401, 125b1e0b, dcfe890c) **NO están desplegados**
  en VPS (SHA VPS = a06a968d). Confirmo desde mi mitad: el diff de `route_intent.rs` en
  working tree (PancakeV3→Unknown) pertenece a dcfe890c y solo existe local (evidencia §3.1).
- **02b/02c/02d NO existen aún** — sin pares que contradecir por ahora. Este reporte es la
  primera ficha pública del searcher; 02b (operators+math-engine) heredará mis punteros
  `math_registry`/`OperatorRegistry` (§5.8, §6.3).

### DISCREPANCIA 1 — 01-INVENTARIO.md no existe (CONFIRMADO)
El board marca WO-01 DONE citando `01-INVENTARIO.md`; `ls` del dir (2026-09-17) devuelve
solo `02-VPS-REMAP-20260917.md`, `GOAL-WORKORDERS.md`, `lines-per-file.txt`. Clasificación:
**CANONICAL_REPO (ausencia verificada por listado directo)**. Evidencia LOC usada en su
lugar: `lines-per-file.txt` (como ordenó el charter).

### DISCREPANCIA 2 — conteo "193 archivos" del charter no calibra
Desde `lines-per-file.txt` (paths `./backend/searcher-rs/`, sin worktrees):
- Total crate: **742 archivos, 345,631 LOC** (incluye 264 `.rhai` + JSONs grandes:
  `dirty_trace.fixture.json` 22,929; `strategy_mapping.json` 12,584; `math_map.json` 3,525).
- Solo `src/**/*.rs`: **313 archivos, 150,795 LOC**.
- Mi claim directo (workers + route_discovery + cartridge + route_intent + impact_index +
  entrypoints): **51 archivos, ~36,000 LOC** (wc -l, §evidencias por ficha).
Clasificación: **CANONICAL_REPO** (lines-per-file.txt + wc -l reproducibles).

### DISCREPANCIA 3 — rutas del charter vs disco
- `src/impacto/` **NO existe**. El módulo de impacto real es `src/impact_index.rs` (1,433 LOC).
  Clasificación: CANONICAL_REPO (ls directo).
- `cartridges/runner` → real es `src/cartridge/runner.rs` (singular `cartridge`); `cartridges/`
  (plural, raíz del crate) contiene los ASSETS Rhai (`.rhai` + manifests JSON). Ambos fichados.

## 1. Mapa del hot-path (fase pipeline C-S-E)

```
[COLLECTOR]  scanner.rs (run_chain) ← WsChainClient/block_scanner (mempool real)
   → route_decoder::decode_to_route_intents (scanner.rs:1531)
   → RouteIntent (route_intent.rs) ──→ orchestrator.on_route_intent (scanner.rs:1577)
[STRATEGY]   orchestrator.rs → ImpactIndex.resolve (impact sets)
   → engines natives (Dex/Triangular/Flashloan/Liquidation/Snipe/SpanningTree/CrossChain)
   → cartridge path paralelo (cartridge_boot.active_evaluate_and_emit, Rhai, gated Active)
   → SizeOptimizer + math_engine::OperatorRegistry (evidence) → StrategyCandidate
[EMIT]       opportunity_emitter.rs → PG opportunities + Redis arbx:opps:detected
             hot_path_emitter.rs → arbx:hot:detected / arbx:hot:simulated / arbx:gate:commit
```
Radares laterales (NO tocan `arbx:opps:detected`): `route_discovery` (shadow, default Off),
`route_scanner_worker` (RU-3, default Off), `pool_enumeration_worker` (Block 2, spawn_if_enabled).
Clasificación: CANONICAL_REPO (file:line en cada ficha).

## 2. Fichas — entrypoints

### 2.1 `backend/searcher-rs/src/main.rs` (1,277 LOC)
- Firma: `fn main()` → boot config+tracing+metrics → Redis CM → PgPool opcional →
  `scanner::run_chain` por chain → axum `/health`+/metrics` en SEARCHER_HEALTH_PORT.
- Misión: arranque del Collector/S2 + spawner de workers de soporte.
- op_origen: n/a (infra) — los ops viven en math_engine (§6.3).
- Deps (grep inverso): scanner, workers::*, chain_supervisor, source_supervisor, topology_reload.
- Estado: muta (spawn + env). Prueba: main.rs:429,466,817,846,908,965,1047,1084,1109 (tokio::spawn).
- Gates de workers legacy: `legacy_triangular_worker_enabled()` main.rs:259,
  `legacy_flashloan_arb_worker_enabled()` main.rs:267, `legacy_liquidation_worker_enabled()`
  main.rs:275 — default **OFF** post-Phase-15 (main.rs:924-930: "not spawned; event-driven
  ...Engine active" + contadores LEGACY_WORKER_DISABLED_TOTAL). CANONICAL_REPO.
- Comentario de seguridad clave: main.rs:305-306 — "spawns no executor and never signs nor
  broadcasts (workers/execution_worker.rs is a never-spawned stub)". Alinea §34.3.
- Destino: **ACTIVO**.

### 2.2 `backend/searcher-rs/src/lib.rs` (255 LOC)
- Crate lib para tests/integración (re-expone módulos; aloca constantes duales XLS-QB-02,
  main.rs:178). op_origen: n/a. Estado: puro. Destino: **ACTIVO** (soporte de CI).

### 2.3 `backend/searcher-rs/Cargo.toml`
- Features: `default = ["v2-simulator"]` (REVM sim real, fail-closed si falta RPC por chain);
  `paper-shadow` (sed-core CDC/eigenstate/Pontryagin); `experimental-engines` (workers fantasma).
- Deps críticos: sim-core (default-features=false, gate v2-simulator), math-engine,
  prioritization-spine, simulator-v2 (optional), rhai. CANONICAL_REPO (Cargo.toml:71-82, 84-90).

## 3. Fichas — route_intent.rs (claim directo, 514 LOC)

- Firma: `RouteIntent::new(chain_id, tx_hash, router, router_kind, sender, legs, amount_in,
  min_amount_out, exact_mode, source_event) -> Option<Self>` (route_intent.rs:102-131);
  producto de `route_decoder::decode_to_route_intents` (doc :24-25), consumido por
  `orchestrator::on_route_intent` (:25).
- Misión: Collector→Strategy — representación normalizada del swap detectado en mempool
  (entrada de TODO el fan-out). No es aún evaluación de Asimetría Topológica.
- op_origen: **multi-dex** (struct portador; las Variedades de Liquidez llegan como
  `legs[].protocol_type ∈ {V2,V3,Curve,Balancer,Unknown}`, :217-230).
- Deps: ethers::types, serde. Nadie de math. Grafo inverso: `grep -rln route_intent::` →
  orchestrator, scanner, impact_index, route_scanner_worker, route_discovery/*, engines/*.
- Estado: **puro** (todos los métodos son constructores/lectores; tests :285-514).
- Invariantes R8 (fail-honest): `pool_hint/dex_hint/fee_bps/min_amount_out` quedan `None`
  cuando el decoder no los extrae (:9-11); `legs.len() >= 1` forzado por constructor
  (`new` → `None` con legs vacíos, :114-116, test `zero_legs_returns_none` :361-368);
  `token_path()` devuelve `None` ante geometría discontinua (:65-82) — no compacta rutas.
- **Diff working-tree NO desplegado** (PancakeV3, commit local dcfe890c):
  route_intent.rs:275 `shared_rs::chains::RouterKind::PancakeV3 => RouterKind::Unknown`
  + comentario :264-266 + test :402-404. Política idéntica a OneInch (:263). Es la punta
  searcher del PANCAKE-ROUTER-01 (catálogo mainnet). GAP correlativo: **no existe variante
  local de intent para PancakeV3** — un swap Pancake llega al orchestrator como
  `router_kind=Unknown`, `protocol_type=Unknown` → el ImpactIndex no puede mapear router→
  protocolo y la pierna aporta menos resolución (impact_index.rs:31 `router_to_protocol`).
  Riesgo de cobertura: HUECO de detección hasta que exista `RouterKind::PancakeV3` local.
  Clasificación: CANONICAL_REPO. Coincide con 02-VPS-REMAP: dcfe890c no está en VPS.
- Destino: **ACTIVO**.

## 4. Fichas — src/workers/ (22 archivos, ~16,000 LOC)

Cabecera del módulo ES la sentencia de estado: "only RpcHealthWorker and PoolSyncWorker are
real. RouteDiscoveryWorker and SimulationWorker stubs were deleted because they emitted fake
telemetry" (workers/mod.rs:2-4). CANONICAL_REPO.

### 4.1 `workers/mod.rs` (206 LOC) — WorkerOrchestrator
- Firma: `start_all_multichain(chain_ids, rpc_pools, db, redis) -> Vec<JoinHandle>` (:82);
  `start_all(chain_id, rpc_pool, db, redis)` (:120).
- Misión: soporte per-chain (RPC health 5s, gas oracle, pool sync 12s alineado a bloque).
- Spawn real: RpcHealth :141-143; GasOracle :155-166 (stampea `arbx:gas_price_ts:<chain>`,
  liveness fail-closed para Pre-Execute Check 6); PoolSync :180-187 (requiere db+rpc).
- R8: chain sin RPC pool → warn `chain_skipped/no_rpc_pool` y NO arranca (:97-104). Puro-ish
  (orquesta; los workers mutan Redis/PG). Destino: **ACTIVO**.
- Constantes: DEFAULT_POOL_SYNC_INTERVAL_MS=12_000 (:41-47, corregido de 5s pre-2026-05-07),
  DEFAULT_RPC_HEALTH_INTERVAL_MS=5_000 (:54-57). CANONICAL_REPO.

### 4.2 Workers ACTIVOS (spawn incondicional en main.rs)
| Worker | LOC | Spawn | Misión | Estado | Destino |
|---|---|---|---|---|---|
| rpc_health_worker.rs | 189 | mod.rs:141 | salud RPC failover | muta Redis métricas | ACTIVO |
| gas_oracle_worker.rs | 219 | mod.rs:155 | gas liveness (Check 6) | muta Redis | ACTIVO |
| pool_sync_worker.rs | 1,430 | mod.rs:180 | sync reservas Variedades de Liquidez | muta PG+Redis | ACTIVO |
| price_worker.rs | 1,280 | main.rs:827 | precios token | muta Redis | ACTIVO |
| heartbeat_worker.rs | 387 | main.rs:846 | heartbeat observabilidad | muta Redis/PG | ACTIVO |
| cex_dex_worker.rs | 673 | main.rs:1084 | spread CEX-DEX | lee APIs externas | ACTIVO (ver nota) |

Nota cex_dex: constructor devuelve `Result` (main.rs:1086-1088 `match ... Err`), arranque
fail-honest sin config. CANONICAL_REPO.

### 4.3 Workers LEGACY default-OFF (bypass de dedup si se encienden)
| Worker | LOC | Gate env | Advertencia |
|---|---|---|---|
| triangular_worker.rs | 3,805 | ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER (main.rs:259, spawn :908) | "opportunities bypass orchestrator dedup" (main.rs:906-909) |
| flashloan_arb_worker.rs | 2,068 | ARBX_ENABLE_LEGACY_FLASHLOAN_WORKER (main.rs:267, spawn :965) | idem (main.rs:963-965) |
| liquidation_worker.rs | 1,628 | ARBX_ENABLE_LEGACY_LIQUIDATION_WORKER (main.rs:275, spawn :1047) | idem |

Destino preliminar: **HUEHUFO** (reemplazados por engines event-driven en scanner.rs:
TriangularEngine :432, FlashloanEngine :437, LiquidationEngine :448). Mantener sin encender:
su emisión bypassa el OppDedup del OpportunityEmitter. CANONICAL_REPO.

### 4.4 Workers GATED default-Off
- `route_scanner_worker.rs` (1,318) + `route_scanner_worker/provenance.rs` — RU-3 negative-cycle
  scan per-block. Gate `ARBX_ROUTE_SCANNER_MODE` default Off (route_scanner_worker.rs:27,117-124).
  Presupuesto honesto: p95 250ms/bloque con counters `capped` (:28-31). NO escribe
  `arbx:opps:detected`; solo PUBLISH `arbx:route_discovery:telemetry` + heartbeat CB-02
  `arbx:controlboard:route_scanner:hb` SETEX 75s (:45-49). Consumo: `shadow_evaluate_intent`
  + `spawn_cartridge_eval`. Destino: **ACTIVO-por-gate** (radar, no terminus).
- `pool_enumeration_worker.rs` (653) — Block 2 top-TVL multi-fuente (The Graph/DeFiLlama/
  GeckoTerminal) con circuit breaker por fuente y confirmación on-chain de factory (:1-15).
  Spawn `spawn_if_enabled` scanner.rs:535. Destino: **ACTIVO-por-gate**.

### 4.5 Workers STUB / nunca spawneados
- `execution_worker.rs` (41): loop `sleep(50ms)` con comentarios TODO — **nunca spawneado**
  (main.rs:305-306 lo declara). Guarda `allow_live_execution: bool` (muerto). Contiene
  jerga prohibida (firmar/broadcast) en comentarios — placeholder deprecado (§1.1 ceguera).
  Destino: **MUERTO** (evidencia: workers/mod.rs:7-8 "kept but not spawned" + grep de spawn
  sin resultados). Recomendación mesa: candidato a eliminación en WO de limpieza, NO desde
  esta auditoría (surgical changes).
- `hft_mempool_listener.rs` (27): placeholder XDP/io_uring, no spawneado. Destino: **MUERTO**.
- `jit_v3_worker.rs` (284): scaffold JIT V3 (BE-3.3) bien documentado pero SIN call-site de
  spawn en main/scanner/mod/orchestrator (grep 2026-09-17: solo `pub mod` mod.rs:14).
  Destino: **HUEHUFO** (diseño listo, wiring GAP).
- `backrun/spatial/dlp/funding_rate/svs/triangular_atomic` (60+59+48+72+44+47): solo
  compilados tras `#[cfg(feature = "experimental-engines")]` (mod.rs:6-7,32-41) y la feature
  NO está en default (Cargo.toml). Sin spawn en default build. Destino: **HUEHUFO**
  (compilación condicionada = ni siquiera existen en el binario de producción).

## 5. Fichas — src/route_discovery/ (17 archivos, ~9,300 LOC)

- Firma módulo: `RouteGraphBuilder → UniqueRouteFinder (DFS 2-3 hops) → RouteCanonicalizer →
  StrategyApplicabilityEngine → RouteIntentDispatcher → shadow_evaluate_intent → telemetry`
  (mod.rs:8-11). Entrada: grafo de pools (PG/Redis); salida: `arbx:route_discovery:telemetry`.
- Misión: **capa discovery** de la doctrina ROUTES_CROWN_JEWEL (discovery≠evaluation):
  enumeración de topología de Holonomic Loop Resolution candidates SIN sizing ni Topological
  Yield — "Phase 1 measures topology: it carries no sizing and no profit" (mod.rs:14-16).
- **Invariante estructural NO-ACTIVE**: `RouteDiscoveryMode ∈ {Off, Shadow}` — no existe
  variante Active EN EL TIPO (mod.rs:57-63); `"active"` parsea a `Off` (:77-79, fail-safe).
  Default `ARBX_ROUTE_DISCOVERY_MODE=Off` → nada se spawnea (:20-22). Prueba adicional:
  `guarantees.rs` (test-only, mod.rs:41). CANONICAL_REPO — este es el patrón de gate más
  fuerte del repo (typestate de seguridad).
- Spawn: scanner.rs:945 `spawn_route_discovery(...)`. op_origen: **multi-dex** (grafía
  multi-Variedad); los ops por estrategia están en math_map.json (§6.4).
- `route_intent_dispatcher.rs` (304): invariante duro documentado :8-13 — único downstream es
  `shadow_evaluate_intent`; NUNCA llama on_route_intent/emitter/arbx:opps:detected; solo
  dispatch con cartucho existente (`dex_arb`, `triangular_arb`); Phase 1 emite intents
  SIN sizing (`amount_in = 0`, :21-24 — honesto, no fabricación). `tx_hash` = route_hash
  keccak real, router/sender zero declarado no-sentinel (:59-61).
- `lat_candidates.rs` (251): wire per-candidate latency (ARBX-FE-EMIT-09/FE-0037). Semántica
  de ausencia R8 ejemplar: `reprice_us` AUSENTE (no 0) cuando la ruta no atravesó el adapter
  (:26-30); atribución `reprice` = upper-bound declarada (:33-35); cap top-K con bloque meta
  `truncated/dropped` (LOGFLOOD, :37-40).
- Per-file: graph_builder 1,306 (construye TokenGraph), unique_route_finder 1,096 (DFS
  acotado con counters honestos capped/dropped_for_cap — route_scanner_worker.rs:28-29),
  triangular_adapter 1,045, strategy_applicability 1,606, cycle_enumerator 485 (RU-1,
  llena `pool_cycles` PG — fuente 1 del ImpactIndex), canonicalizer 415, dense_view 468,
  multi_hop_search 706, telemetry 369, types 320.
- Estado: puro-cómputo + escritura SOLO telemetría. Destino: **ACTIVO-por-gate (Shadow)**.

## 6. Fichas — runtime de cartuchos (FASE OMEGA)

### 6.1 `src/cartridge/runner.rs` (977)
- Firma: `CartridgeRunner` — Engine Rhai sandboxeado; `compile` valida AST vs contrato
  universal; `evaluate` (:290) corre con Scope fresco. Entrada: pool_data (host bindings);
  salida: `CartridgeEvalResult`.
- Sandboxing CANÓNICO: MAX_OPERATIONS=1_000_000, MAX_CALL_STACK=64, string 64KiB, array
  4,096, map 1,024, expr-depth PINNADO a valores release para determinismo debug/release
  (runner.rs:40-66 — decisión de PhD correcta: sin pin, validación diverge por perfil).
- Misión: Strategy (detección 264 cartuchos). op_origen: **multi-dex + op_XX vía
  math_registry** (ver 6.4). Estado: puro por evaluación; HashMap concurrente de ASTs.
- "A misbehaving cartridge NEVER crashes the host — marked Failed and excluded" (:23-24).
  Destino: **ACTIVO** (núcleo canónico de detección).

### 6.2 resto `src/cartridge/`
- `host_bindings.rs` (1,181): periféricos get_reserves/get_token_meta/simulate_swap/
  get_base_fee/log_quantum/emit_signal (mod.rs doc :28-40); :446 expone valores del
  registry de 31 ops. Muta Redis/RPC por binding. **ACTIVO**.
- `contract.rs` (233): validador contrato universal (init_strategy/evaluate_opportunity/
  build_payload). Puro. **ACTIVO**.
- `subscriber.rs` (299): hot-reload por PubSub. **ACTIVO**.
- `types.rs` (134) + `manifest_test.rs` (163): tipos + tests del manifest 264. Soporte.

### 6.3 `src/cartridge_boot.rs` (2,465) — el dispatcher de modos
- `CartridgeMode ∈ {Off, Shadow, Active}` desde `ARBX_CARTRIDGE_MODE`, default Off
  (cartridge_boot.rs:51-66). `spawn_cartridge_runtime` (:108) crea el runner (:161).
- `shadow_evaluate_intent` (:825) — observe-only. `active_evaluate_and_emit` (:964) —
  única ruta que emite Opportunity desde cartucho; recibe `Arc<math_engine::OperatorRegistry>`
  (:972) — **punto de acople con WO-02b**; bound de concurrencia con semáforo
  try_acquire (drop, no queue, :1004-1013). `spawn_cartridge_eval` en orchestrator.rs:261
  exige `CartridgeMode::Active` (:271-273) — no-op en Shadow/Off. Paper, capital=0 (:259-260).
- Destino: **ACTIVO** (es EL switch de la detección canónica por cartuchos).

### 6.4 `cartridges/` (assets)
- `strategies/*.rhai` (264 × 202 LOC) + `omega_strategy_pack.rhai` + 4 .rhai legacy.
- `manifests/math_map.json` (3,525 LOC, 264 entradas): mapeo CANÓNICO mev_id →
  {detector_id, primary_ops [op_XX], equation, data_bindings, frontend_toggle, mode}.
  Ejemplo MEV-01-001: primary_ops [op_27 path_ordering, op_21 newton, op_15 golden_section,
  op_16 kelly], mode SHADOW (math_map.json:2-15). **Este es el op_origen de partida para
  WO-02b — NO re-derivar** (board WO-02 line 15). CANONICAL_REPO.
- `strategy_mapping.json` (12,584) + `liquidity_classification.json` (3,016): config de
  estrategias/clasificación. `scripts/gen_math_manifest.py` genera el manifest (mod.rs:69-70).

### 6.5 `src/math_evidence.rs` — cableado Fix B observe-only
- Construye `MarketState` real desde ReservesCache (precio=r1/r0, None si r0=0, :22-25) y
  evalúa ops recomendados por `RegimeRouter`. **"NO alteran el scoring todavía"** (:9-11,
  doctrina anti-reincidencia: nunca cablear matemática no validada al hot-path de decisión).
  R8: sin reservas → evidencia `insufficient_state`, nunca MarketState fabricado (:13-14).
  Destino: **ACTIVO (observe-only)**. Punto crítico para WO-02b: los 31-32 ops están hoy
  EN OBSERVACIÓN, no en la decisión — cualquier afirmación de que "los ops deciden" es falsa.

## 7. Fichas — impacto (impact_index.rs, 1,433)

- Firma: `ImpactIndex::resolve(&RouteIntent) -> ImpactSet` — "which triangular cycles, DEX
  pools, and lending positions are affected?" (:7-9). Entrada: RouteIntent; salida: conjunto
  mínimo (pools, cycles, lending positions).
- Fuentes de construcción (:14-31): 1) `pool_cycles` PG (universo dinámico del
  cycle_enumerator RU-1) con fallback cold-boot a MVP_CYCLES vía Redis pool index;
  2) `token_pair_to_pools` desde tabla pools (refresh por pool_sync add_pool); 3)
  `token_to_lending_positions` **VACÍO hasta LendingPositionIndexer (Phase 11)** — R8
  fail-honest, nada fabricado; 4) router_to_protocol estático de configs/router_kinds.json;
  5) selector_to_decoder compile-time.
- Invariantes R8 (:33-37): legs vacíos → ImpactSet::default() (no error); pair ausente →
  esa pierna no aporta; dedup de cycles/pools. **GAP estructural**: la fuente 3 vacía
  significa que la detección de liquidations por impacto está incompleta BY DESIGN hasta
  Phase 11 — reportado, no maquillado.
- Estado: RwLock interno (read en hot-path orchestrator.rs:330); resolve puro.
- op_origen: multi-dex (resolución de Variedades afectadas). Destino: **ACTIVO**.

## 8. Fichas — orchestrator.rs (2,098; hub del claim aunque no listado explícito)

- `Orchestrator::on_route_intent` (:292): valida chain_id (cross-chain leak → reject+counter,
  :300-311), métrica decoded_intents_total, `ImpactIndex.resolve` (:330), fan-out a engines,
  evaluate_with_route_plan, emisión vía OpportunityEmitter (doc :240-259).
- `spawn_cartridge_eval` (:261): ruta canónica para cycles de route_discovery — "cartridges
  are the sole canonical detector for discovered cycles (no duplicate rows)" (:263-266);
  exige CartridgeMode::Active (:271-273). Err de Redis publish se propaga (reconnect);
  errores de evaluación/gate/PG se tragan con counter (fail-honest por candidato, :251-255).
- Destino: **ACTIVO** (el corazón del hot-path). op_origen: multi-dex.

## 9. objetivo_usd — FAIL-HONEST

- **GAP general**: NO existe `objetivo_usd`/`target_usd` como fuente real en searcher-rs
  (grep 2026-09-17: solo matches irrelevantes en engines experimentales). PROHIBIDO inventar.
- Fuentes REALES más cercanas (bps/relativos, NO USD):
  - `canonical_knobs.rs:71` `min_net_bps: f64 = 5.0` — "Min_Net_bps: gate mínimo beneficio
    neto" heredado de QUOTEBASE-264 01_CONFIG (canonical_knobs.rs:44,186,294 env
    `ARBX_KNOB_MIN_NET_BPS`; validación >=0 :428-429). Gate en bps del Topological Yield neto.
  - `size_optimizer.rs:85` gas-safety floor: `net_usd < gas_usd × kelly_gas_safety_multiplier`
    (fricción termodinámica como piso). `min_profit_usd: 0.01` aparece SOLO en tests
    (size_optimizer.rs:1852) — no es producción.
- Conclusión para el board: el sistema opera con umbrales RELATIVOS (bps y múltiplos de
  gas), no con un blanco USD absoluto por estrategia. Clasificación CANONICAL_REPO.

### 9.1 ADENDUM (WO-02a-FIX-R1, 2026-09-17) — knobs USD declared-only + consumidor real de min_net_bps + reconciliación 02d

> Append-only, enmienda del gap §9 del cross-exam (WO-02a-verify-CROSS-EXAM.md G1/G2 y
> hint del orquestador). No borra texto previo; corrige el framing "solo RELATIVOS".

**A. Los 4 knobs USD del workbook son DECLARED-ONLY (0 consumidores hot-path)** — mismo
precedente declarativo que `beam_k` (canonical_knobs.rs:74-82). El §9 original los omitió:

| Knob USD | Default | Declaración (struct/default/env/validate) |
|---|---|---|
| `min_pool_liquidity_usd` | 150_000 | canonical_knobs.rs:57 / :176 / :278-281 / :417-419 |
| `max_gas_usd` | 120 | canonical_knobs.rs:60 / :179 / :287 / :414-416 |
| `min_size_usd` | 10_000 | canonical_knobs.rs:63 / :182 / :290 / :414-416 |
| `min_ev_usd` | 25 | canonical_knobs.rs:65 / :183 / :291 / :414-416 |

Grep evidencia (2026-09-17, `backend/ --include=*.rs`, field-access
`\.(min_ev_usd|max_gas_usd|min_size_usd|min_pool_liquidity_usd)\b` fuera de
canonical_knobs.rs): **0 hits**. Único wiring: main.rs:367 `from_env()` + publicación boot
a Redis `arbx:config:canonical_knobs` (main.rs:373-386) y el GET api-server
`/api/v1/config/canonical-knobs` (canonical-knobs.ts) — AMBOS observabilidad, jamás gate.

Nota anti-colisión de nombres (3 capas distintas que NO son este knob):
- `trading_config.max_gas_usd: Option<f64>` per-estrategia (shared-rs/src/trading_config.rs:164,
  superficie operador PG, ver 02d) — field-access Rust: 0 hits también.
- `max_gas_usd` schema-param de cartuchos .rhai (mev_01_*.rhai:44, default 50.0, editable
  desde panel) — capa cartucho, default DISTINTO (50 ≠ 120).

**B. Consumidor real de `min_net_bps` = net_bps_ranking.rs (no canonical_knobs).**
El contrato canónico Net_bps (07_INEFFICIENCY, ARBX-0009) vive en net_bps_ranking.rs:19
(fórmula PASS `AND(T>0, U>=01_CONFIG!$B$13)` → `passes(min_net_bps)` :141-143) y :37-38
(`DEFAULT_MIN_NET_BPS = 5.0` cross-pinned a `CanonicalKnobs::min_net_bps`, test :347-351).
El hot-path consume la MÉTRICA net_bps como clave de ORDEN: `rank_by_net_bps`
(orchestrator.rs:1019 — sort del sized_batch; discovery_workload.rs:358).
Matiz fail-honest MÁS estricto que el framing del cross-exam: `passes(min_net_bps)` tiene
**0 call-sites de producción** — solo tests (net_bps_ranking.rs:286-316, tras `#[cfg(test)]`
:188). Es decir: el RANKING por net_bps está vivo; el GATE V≥B13 del workbook aún no se
invoca fuera de tests. El piso económico efectivo en producción sigue siendo el del sizing
kernel (`GasFloorBreach`, size_optimizer.rs:85). Clasificación: CANONICAL_REPO.

**C. Reconciliación con 02d — la fuente USD REAL del terminus es trading_config (PG runtime).**
02d-RELAYS-CLIENT-SHARED.md:212 + §5 (:245-258): `trading_config.capital_usd`
(shared-rs/src/trading_config.rs:183; cap 2% por oportunidad, execution_admission.rs:193-201),
`min_profit_usd` (:260, gate REAL; override por estrategia :622) y `simulation_capital_usd`
(:166). Coincide con R1 del cross-exam (scanner.rs:2568 `min_profit_threshold`). Consume
además 02b:148 (G1: `principal_usd/expected_profit_usd` son OBSERVADOS, no objetivos).

**Framing §9 corregido (reemplaza la lectura literal "el sistema opera con umbrales
RELATIVOS, no con un blanco USD absoluto"):** el hot-path de searcher-rs opera hoy con
(1) umbrales RELATIVOS vivos (net_bps como clave de ranking; gas-safety floor del sizing),
(2) un piso USD absoluto operador-configurable vivo (`trading_config.min_profit_usd`, PG
runtime, consumido en scanner y terminus), y (3) 4 knobs USD del workbook DECLARED-ONLY
en canonical_knobs.rs:57-65 (0 consumidores, precedente beam_k). El **GAP objetivo_usd POR
ESTRATEGIA en searcher-rs SOBREVIVE** intacto: ningún `objetivo_usd`/`target_usd` existe
como fuente real; lo más cercano canónico es `simulation_target_profit_usd`
(trading_config.rs:249, filtro UI stores-but-not-gates, R8).

*No se tocó WO-02a-verify-VERIFY.md (archivo del verificador; su adendum G1/G2 queda a
decisión de su owner/orquestador). Evidencia recomputable: greps arriba, re-ejecutables.*

### 9.2 PRECISIÓN (WO-02a-FIX-G1, 2026-09-17) — el piso USD vivo NO gatea vía PrioritizationEngine // WO-02a (2026-09-17)

> Append-only, fixer gang ronda 1 (charter G1 de la TABLA del cross-exam). No modifica
> §9 ni §9.1. Espejo del adendum `// WO-02a-FIX-G1` en WO-02a-verify-VERIFY.md.

El §9.1-C alineó el piso absoluto con scanner.rs:2568 (R1.2 del cross-exam:
"`min_profit_threshold: cfg.min_profit_usd` ... alimenta PrioritizationEngine").
Re-verificación de este fixer (precedente "aserciones de agentes ≠ facts"):

- `PrioritizationEngine::score` (prioritization-spine/src/scoring.rs:15-46) **NO lee
  `min_profit_threshold`** en ninguna rama — declarado :14, poblado en scanner.rs:2568,
  y su ÚNICO gate es `net_expected <= 0.0 → NegativeProfit` (:25-26). Grep
  `min_profit_threshold` en backend/ (--include=*.rs, sin target/): exactamente 2 hits
  (declaración + población). **Cero lectores**: wiring populated-but-unconsumed, mismo
  patrón declarativo que `beam_k` y los 4 knobs USD del §9.1-A.
- El piso USD absoluto VIVO del hot-path gatea por **otro cable**: dentro de
  `decode_and_score_tx` (scanner.rs:1488), `ConfigAwareEvaluator::with_cache`
  (scanner.rs:2125) → `policy_from_config` (config_aware.rs:148-151:
  `RiskPolicy.min_net_profit_usd = cfg.min_profit_usd`, instanciado :1053) →
  `validate_opportunity_risk` (config_aware.rs:1085) → **math-engine/src/risk_engine.rs:47-48**
  (`net_profit_usd <= policy.min_net_profit_usd → NegativeNetProfit`). Además el override
  por estrategia: `strategy_config_gate.rs:302-308` (`Reject(StrategyConfigBelowMinProfit)`)
  consumido en scanner.rs:2312-2319 con persistencia fail-honest (`rejection_reason`
  poblado, patrón R8). `cfg` = `trading_config.state(chain_id)` (scanner.rs:1637) — PG operador.

**Efecto**: el framing corregido de §9.1 (umbrales relativos vivos + piso USD absoluto
operador-configurable vivo + knobs declared-only) NO cambia; solo se precisa la CITA del
mecanismo. Quien consuma esta conclusión (gates 1-8, WO-06) debe citar
`config_aware.rs:150 → risk_engine.rs:47 (+ strategy_config_gate.rs:302 → scanner.rs:2312)`
y NO scanner.rs:2568 como gate. Clasificación: CANONICAL_REPO. **GAP objetivo_usd POR
ESTRATEGIA: intacto** (re-confirmado: ningún `objetivo_usd`/`target_usd` productivo;
lo más cercano sigue siendo `simulation_target_profit_usd`, trading_config.rs:249,
stores-not-gates R8).

## 10. Diseño — recomendaciones con diffs exactos (NO aplicados, gated operador)

> Ningún diff fue escrito a código de producción (restricción WO kind:design). Todos
> marcados con el ID del WO.

### D-1 · Cerrar el hueco PancakeV3 en RouteIntent (continuación PANCAKE-ROUTER-01)
- Invariante: un swap Pancake V3 decodificado conserva identidad de protocolo end-to-end
  (decoder→intent→impact_index), sin sentinel R8.
- Diff propuesto en `backend/searcher-rs/src/route_intent.rs`:
```rust
// WO-02a (2026-09-17): PancakeV3 deja de degradar a Unknown — SwapRouter02-style
// encoding, precio V3 concentrated (protocol_type=V3).
 pub enum RouterKind {
     UniswapV2,
     UniswapV3,
     Sushi,
     Curve,
     Balancer,
     UniversalRouter,
     OneInch,
+    PancakeV3,
     Unknown,
 }
...
-            shared_rs::chains::RouterKind::PancakeV3 => RouterKind::Unknown,
+            shared_rs::chains::RouterKind::PancakeV3 => RouterKind::PancakeV3,
```
  + propagar `RouterKind::PancakeV3 → ProtocolType::V3` donde router_kind→protocol_type
  se resuelva (impact_index `router_to_protocol`, fuente 4 estática:
  configs/router_kinds.json ya tiene la dirección por dcfe890c).
- Gate: test round-trip existente (:395-414) actualizando el caso PancakeV3; gate de
  impact_index (test de resolve con leg PancakeV3 → pool V3 en ImpactSet).
- NOTA: dcfe890c eligió Unknown como R8-safe temporal; este diff es la promoción, misma
  política que siguió UniversalRouter. Requiere el decoder calldata activo para el router
  Pancake en `calldata/` — verificar antes (posible GAP de decoder: HYPOTHESIS).

### D-2 · Eliminación de stubs muertos (execution_worker, hft_mempool_listener)
- Invariante: ningún módulo compilado sugiera capacidad de firma/broadcast que no existe
  (§32 audit/scaffold; main.rs:305 documenta que nunca se spawnea).
- Diff: borrar `src/workers/execution_worker.rs`, `src/workers/hft_mempool_listener.rs`,
  sus `pub mod` en workers/mod.rs:7-8 y el comentario de main.rs:305-306.
- Gate: `cargo check -p searcher-rs` (orquestador, WO-05) + grep cero referencias.
- Es limpieza de superficies de seguridad, NO cambio de comportamiento. Gated operador
  (hardening §37: un PR = un ID de anomalía).

### D-3 · Documentar el presupuesto de workers experimentales en el board
- Invariante: la mesa sabe que `experimental-engines` NO está en default build (Cargo.toml),
  por lo que 6 workers (+ ~8 engines en src/engines/) son código no-compilado en producción.
- Sin diff de código: entrada de board. Gate: cita Cargo.toml:71 (default = ["v2-simulator"]).

## 11. Destino preliminar consolidado (solo con evidencia)

| Pieza | Destino | Evidencia clave |
|---|---|---|
| main.rs, scanner.rs, orchestrator.rs, route_intent.rs, impact_index.rs | ACTIVO | scanner.rs:1531,1577; orchestrator.rs:292; impact_index.rs:7-9 |
| cartridge/{runner,host_bindings,contract,subscriber} + cartridge_boot.rs | ACTIVO | cartridge_boot.rs:108,825,964; runner.rs:290 |
| workers reales (rpc_health, gas_oracle, pool_sync, price, heartbeat, cex_dex) | ACTIVO | mod.rs:141,155,180; main.rs:827,846,1084 |
| route_discovery/* (Shadow) + route_scanner_worker | ACTIVO-por-gate (radar) | mod.rs:57-63 NO-ACTIVE typestate; rs_worker:117-124 |
| triangular/flashloan_arb/liquidation workers legacy | HUEHUFO | main.rs:259-275, default-off-post-phase-15 |
| jit_v3_worker | HUEHUFO (wiring GAP) | sin spawn (grep 2026-09-17) |
| 6 workers experimental-engines | HUEHUFO (no compilados en prod) | Cargo.toml default |
| execution_worker, hft_mempool_listener | MUERTO | mod.rs:7-8; main.rs:305-306 |

## 12. Verificación y honestidad

- Análisis 100% Read/Grep/Bash-readonly. CERO cargo/npm/build/tsc (cumplido).
- CERO commits (cumplido; `git status` muestra solo los diffs preexistentes del operador).
- FAIL-HONEST: 02b/02c/02d ausentes al momento de escribir — sin síntesis cross-crate del
  stack sim/selector/relays desde esta mitad; el punto de acople queda marcado (§6.3).
- Pendiente para verificadores: (1) confirmar decoder Pancake en calldata/ antes de D-1;
  (2) WO-02b debe fichar los 32 ops contra math_map.json SIN re-derivar; (3) el VPS corre
  a06a968d sin los 3 commits locales — cualquier verificación en vivo ve el estado PRE-fix.

## VERIFICACIÓN (WO-02a-verify, 2026-09-17) — adendum append-only del verificador

> Dictamen completo: `WO-02a-verify-VERIFY.md`. Resumen: 15 fichas muestreadas, 15/15 PASS
> en sustancia (~30 citas re-abiertas, 0 contenido falso). op_origen PASS (trazable a
> math_map.json:2-15 + backend/math-engine/src/operators/, cero mano alzada). objetivo_usd
> GAP PASS (engines con min_profit_usd verificados experimental-gated, engines/mod.rs:38-51).
> muta/puro PASS. D-1..D-3 confirmados NO aplicados. Dos correcciones:

**CORRECCIÓN A (material) — conteos de DISCREPANCIA 2 contaminados por worktrees.**
lines-per-file.txt contiene 320 entradas searcher-rs/src CON prefijo worktree
(`.claude/worktrees/wf_257a24e1-859-2/`) y 184 sin él. Números reales NO-worktree:
crate = **742 archivos, 223,273 LOC** (los 345,631 reportados son el total CON worktrees,
1,430 archivos); src .rs = **178 archivos** (no 313). La conclusión (193 del charter no
calibra) SOBREVIVE. Menor: .rhai real no-worktree = 271 (no 264).

**CORRECCIÓN B (menor) — drift de líneas en ~10 citas (2-9 líneas).** Spawns reales
workers/mod.rs:147-149/:163-167 (citados :141-143/:155-166); enum RouteDiscoveryMode
:54-60 (citado :57-63); semáforo cartridge_boot :989-1001 (citado :1004-1013);
invariantes impact_index :25-30 (citados :33-37). En todos los casos el contenido citado
existe y dice lo afirmado.

**Omisiones (módulos con presencia real sin ficha, para WO de seguimiento):** engines/
(17 arch, 8,154 LOC), size_optimizer.rs (3,625), scanner.rs (3,372, solo mapa §1 sin
ficha formal), strategy_hop_map.rs (3,289), calldata/ (1,929), detector_policy.rs (1,616),
opportunity_emitter.rs (1,093), route_decoder.rs (1,020), y ~10 más menores
(state_projector, amm_math, pool_sources/, quote_anchor_runtime, pair_index,
candidate_simulation, discovery_workload, dirty_pairs, topology_reload); thermodynamics/
(13 arch, ~450 LOC, triviales). Verificado por el charter de WO-02a: el scope nombrado
("workers, route_discovery, cartridges/runner, impacto, entrypoints bin") SÍ fue cubierto;
las omisiones son piezas hot-path fuera del scope nombrado pero dentro del interés del board.

## ERRATA DE PROCEDENCIA (ERRATA-PROC-WO-02a, 2026-09-17) — fixer respawn-A, append-only

> Origen: errata material del cross-examiner de procedencia (charter del orquestador;
> registrada en GOAL-WORKORDERS.md §ERRATA-WO-02a). Método: git read-only recomputado
> por este fixer (`git show --stat`, `git log -- <path>`, `git status --porcelain`,
> `git diff`). CERO commit/push/PR (NO-GIT cumplido — git usado SOLO en modo lectura),
> CERO cargo, CERO VPS, CERO HTTP. Esta errata NO borra nada: el texto previo queda
> arriba como historial; para citas futuras manda esta sección.

### E-1 (MATERIAL) — El diff working-tree de route_intent.rs NO pertenece a dcfe890c

Afirmaciones refutadas de ESTE archivo:
- §0 (líneas 13-14): "el diff de `route_intent.rs` en working tree (PancakeV3→Unknown)
  **pertenece a dcfe890c** y solo existe local (evidencia §3.1)" — FALSO. (Menor: el
  puntero "§3.1" también está mal; la ficha es §3.)
- §3 (líneas 101-104): "**Diff working-tree NO desplegado** (PancakeV3, **commit local
  dcfe890c**)" — FALSO en la atribución.
- §10 D-1 NOTA (línea 396): "**dcfe890c eligió Unknown** como R8-safe temporal" — FALSO:
  dcfe890c no tocó route_intent.rs, luego no eligió nada ahí.

Evidencia recomputable (ejecutada 2026-09-17 ~01:41 -0500):
- `git show dcfe890c --stat` → SOLO `backend/shared-rs/src/chains.rs` (+21) y
  `backend/sim-ctl/src/tx_builder.rs` (+84). 2 archivos, 105 inserciones.
  `route_intent.rs` NO aparece.
- `git log -- backend/searcher-rs/src/route_intent.rs` → último commit = `a37bcbfc`
  ("close V3 wire, fee boundary...", 2026-09-15 06:05:57 -0500). Ningún commit reciente
  toca el archivo.
- `git status --porcelain -- backend/searcher-rs/src/route_intent.rs` → ` M` (modificado,
  NO commiteado). `git diff --stat` → **+7/−1** (doc-comment ~:264-266 + mapeo
  `PancakeV3 => RouterKind::Unknown` :275 + test ~:402-404).

**Reclasificación canónica de la mesa** (idéntica a la del board §ERRATA-WO-02a y a la
ya usada por WO-05): el diff es un **huérfano de working-tree, NO commiteado, autoría
SIN confirmar** (hipótesis razonable pero NO verificada: operador, follow-up de
PANCAKE-ROUTER-01 — el propio comentario interno del test del diff dice
"PANCAKE-ROUTER-01 follow-up: no local intent variant yet — R8 safe default is Unknown
(same policy as OneInch)", lo que explica cómo este reporte y el verify/cross-exam
lo atribuyeron al commit por asociación). El verificador (WO-02a-verify §7) y el
cross-examiner (WO-02a-verify-CROSS-EXAM :33-35) repitieron la atribución sin correr
git — esa "corrección" fue de humo; esta errata la corrige con los comandos arriba.

Nota de honestidad (mtime): el charter de errata citaba mtime 00:31; el mtime medido
por este fixer es **2026-09-17 01:36:02 -0500** (archivo tocado a las 01:36 por proceso
no identificado — mismo minuto que la última escritura de ESTE archivo). El CONTENIDO
del diff (+7/−1, mismas tres hunk-áreas) es idéntico al descrito por verify (01:07) y
cross-exam (01:13), así que el contenido es estable desde antes de ambos reportes; solo
el mtime se movió. No hay forma read-only de determinar quién tocó el archivo a las 01:36.

### E-2 (SECUNDARIO, mismo tipo) — §10 D-1 línea 393: router_kinds.json NO existe

Afirmación refutada: "fuente 4 estática: `configs/router_kinds.json` **ya tiene la
dirección por dcfe890c**" — DOBLEMENTE falsa:
1. El archivo NO existe: `find . -name "router_kinds.json"` (excl. worktrees/node_modules)
   = 0 resultados; `git ls-files | grep -i router_kind` = vacío; el directorio
   `backend/searcher-rs/configs/` no existe en disco. La única mención es el doc-comment
   de impact_index.rs:21 ("static config from `configs/router_kinds.json` plus runtime
   detection (future)") — o sea, una fuente FUTURA documentada, no materializada.
2. dcfe890c jamás tocó ese path (ver E-1 stat). La dirección PANCAKE_SMART_ROUTER_MAINNET
   (0x13f4EA83D0bd40E75C8222255bc855a974568Dd4) sí fue agregada por dcfe890c, pero en
   **backend/shared-rs/src/chains.rs** (+21).
Consecuencia para D-1: la propagación `RouterKind::PancakeV3 → ProtocolType::V3` en
impact_index NO puede apoyarse en un JSON inexistente — vive en código (tabla estática
o chains.rs). D-1 queda condicionado a esto (HYPOTHESIS a re-verificar al ejecutar D-1).

### Lo que SOBREVIVE de §0/§3/§10 (la sustancia técnica no cae)

- El diff ES local-only y NO desplegado: VPS corre a06a968d (02-VPS-REMAP) y un diff
  sin commit no puede estar en ningún deploy — por definición.
- El GAP correlativo de §3 es REAL y queda intacto: NO existe variante local
  `RouterKind::PancakeV3` de intent → swap Pancake llega como Unknown → HUECO de
  resolución en `router_to_protocol` (impact_index.rs:31). La detección del hueco no
  dependía de la autoría del diff.
- La descripción del CONTENIDO del diff (:275 mapeo, :264-266 comentario, test) era
  exacta (verify §1 ficha 3 la re-abrió byte-exact).
- §12 "git status muestra solo los diffs preexistentes del operador": la parte factual
  (preexistente, no escrito por el diseñador) SOBREVIVE; la palabra "del operador" queda
  como autoría SIN confirmar (ver reclasificación E-1).

*(Errata aplicada por fixer respawn-A del gang 2026-09-17 — mitad A del charter de
procedencia: DESIGN.md + punto §7 del verify. Entrada de board y errata numérica
E-a/E-b/E-c: fixer par respawn-B, no tocadas aquí. Contaminación residual documentada
en el board y NO tocada por pertenecer a otros owners: INFORME.md:63 (REPARO-1) y
WO-02a-verify-CROSS-EXAM.md:35.)*

## ERRATA MATERIAL (WO-02a-FIX-R2, 2026-09-17) — fuente fantasma `configs/router_kinds.json` en §7 y punto de propagación inexistente en D-1

> Append-only (ronda 1 del gang; R1 ya tomado por el fixer del §9 — `WO-02a-FIX-S9-20260917.md`,
> adendum §9.1). Esta errata corrige §7 (fuente 4), el mecanismo citado en §3 y REESCRIBE D-1
> (el texto original queda arriba intacto; queda SUPERSEDO por esta sección). Cierra parte de
> las "erratas de detalle PENDIENTES" del board (entrada ERRATA-WO-02a) y responde al
> cross-exam de la errata de fuente fantasma. Reporte del fixer: `WO-02a-FIX-R2-20260917.md`.

### E-1 · §7 fuente 4 = MUERTA/NO-IMPLEMENTADA (falso doble)

El §7 ficha como fuente de construcción del ImpactIndex: *"4) router_to_protocol estático
de configs/router_kinds.json"*. REFUTADO en ambas mitades:

1. **`configs/router_kinds.json` NO existe en el repo** — `find . -name "router_kinds.json"`
   (raíz del repo, 2026-09-17) = 0 resultados. La ÚNICA mención del archivo en todo el árbol
   es el doc-comment de impact_index.rs:21-22 — un comentario que describe una fuente que
   nunca se implementó (fuente fantasma documental, el origen de esta errata).
2. **`router_to_protocol` es un campo muerto**: grep repo `router_to_protocol` = exactamente
   3 hits — doc :21, declaración :222 bajo `#[allow(dead_code)]` (:221), e inicialización
   vacía `HashMap::new()` en `empty()` (:244). CERO escritores con datos, CERO lectores.
   `resolve()` (:452-473) y `resolve_leg()` (:476-519) NUNCA consultan router ni
   `router_kind`: la resolución de protocolo del ImpactSet sale del registro de Variedades
   de Liquidez (`pool_ref.protocol_type`, :503), no de un mapa router→protocolo.

Clasificación: CANONICAL_REPO (find/grep/lectura directa, recomputables).
Corrección de cita del §3: el puntero "impact_index.rs:31 `router_to_protocol`" apunta en
realidad a la línea del `use crate::route_intent::{...}` (:31-32) — el campo vive en :221-222.

**Hallazgo nuevo para la mesa (borrador de ficha)**: la fuente 5 (`selector_to_decoder`,
compile-time, :226-227) está TAMBIÉN muerta — `#[allow(dead_code)]`, poblada por
`build_selector_table()` (:245, :529-564) y con 0 lectores. Es decir, de las 5 "fuentes de
boot" del doc-comment :12-23, la 3 es vacía-by-design (R8 Phase 11) y la 4 y la 5 son
código muerto `#[allow(dead_code)]`. El doc-comment sobrevende la construcción real del
índice (= fuentes 1 y 2: `pool_cycles` PG + tabla `pools`). Recomendación (GATED operador,
NO aplicada en esta oleada read-only): corregir el doc-comment de impact_index.rs:21-22
para marcar la fuente 4 como no-implementada — es la raíz que hizo tropezar a WO-02a y al
verify (precedente "aserciones de agentes ≠ facts"; aquí la aserción era del propio código).

### E-2 · Mecanismo del hueco Pancake (corrige §3): el swap muere en el DECODER, no llega degradado al orchestrator

§3 afirmaba: *"un swap Pancake llega al orchestrator como `router_kind=Unknown`,
`protocol_type=Unknown` → el ImpactIndex no puede mapear router→protocolo"*. IMPRECISO:
con la entrada de catálogo de dcfe890c (chains.rs:415-416) el tx SÍ se identifica como
dirigido al router Pancake, pero muere UNA FASE ANTES:

- route_decoder.rs:83-84 pasa el kind del catálogo a `decode_swaps` → `calldata::decode`.
- calldata/mod.rs:96-109 despacha POR `RouterKind`: solo UniswapV2|Sushi, UniswapV3 y
  UniversalRouter tienen brazo (:102-106). `PancakeV3` cae en `_ => Err(DecodeFailReason::
  UnknownRouter)` (:107).
- route_decoder.rs:84-87: `Err(UnknownRouter)` → `None` → `Ok(vec![])` — **NO se produce
  RouteIntent alguno** (ni con Unknown). El gap real de cobertura es de DECODER, no de
  resolución router→protocolo.

### E-3 · D-1 REESCRITO (D-1-rev2) — el punto de propagación propuesto NO existe; la resolución real vive en el decoder

El D-1 original ordenaba: *"propagar `RouterKind::PancakeV3 → ProtocolType::V3` donde
router_kind→protocol_type se resuelva (impact_index `router_to_protocol`, fuente 4 estática:
configs/router_kinds.json ya tiene la dirección por dcfe890c)"*. **Falso doble**: el
archivo no existe (E-1.1) y dcfe890c NO tocó ninguna config de routers — `git show
--stat dcfe890c` = solo `backend/shared-rs/src/chains.rs` + `backend/sim-ctl/src/tx_builder.rs`
(0 hits route_intent/router_kinds; consistente con la errata de procedencia del board).
Además el `From` actual (route_intent.rs:275, diff huérfano de working-tree) ya mapea
`PancakeV3 => Unknown`, con lo que la pierna ni siquiera se construiría.

**Cadena REAL de resolución router→protocolo** (la que D-1 debe tocar):

```
shared_rs::chains catálogo (chains.rs:415-416, PANCAKE-ROUTER-01)
  → route_decoder.rs:84 decode_swaps
  → calldata::decode(input, RouterKind) — dispatch calldata/mod.rs:101-108
  → sub-decoder asigna DecodedSwap.protocol_type (calldata/mod.rs:74;
    univ3.rs:116 lo hace para UniV3)                      ← AQUÍ vive la resolución
  → route_decoder.rs:264 build_legs_from_decoded (`protocol_type: decoded.protocol_type`)
  → RouteIntentLeg.protocol_type (consumidores: candidate_simulation.rs:189
    parse_dex_kind, cartridge_boot.rs:240/290-325 runtime Rhai)
```

**D-1-rev2 (reemplaza el D-1 original; diffs NO aplicados, gated operador):**
1. `calldata/pancake.rs` (nuevo): decoder del encoding SwapRouter02-style del Smart Router
   — el ESPEJO del encoder ya canónico en sim-ctl/tx_builder.rs (dcfe890c): 7-tuple SIN
   deadline (tokenIn, tokenOut, fee, recipient, amountIn, amountOutMinimum,
   sqrtPriceLimitX96 uint160), selector COMPUTADO de la firma canónica. Debe asignar
   `protocol_type: ProtocolType::V3` y `path_fees_bps` en pips raw (convención
   calldata/mod.rs:57-68: NO normalizar a bps).
2. `calldata/mod.rs::decode`: agregar el brazo `RouterKind::PancakeV3 => pancake::decode(...)`
   (:101-108). Sin este brazo, nada de lo demás produce efecto (E-2).
3. La promoción de enum de route_intent.rs (diff original D-1: variante `PancakeV3` +
   `From` directo) SIGUE siendo necesaria para fidelidad de `router_kind` end-to-end.
4. **impact_index: SIN cambios** — `router_to_protocol` está muerto (E-1.2) y `resolve`
   nunca lo necesitó; los pools V3 de Pancake ya aportan `impacted_protocols` vía el
   registro de Variedades (:503). El "gate de impact_index" del D-1 original queda OBSOLETO.

**Gates de verificación (se mantiene el gate del decoder en calldata/, como exige el
cross-examiner):** tests en calldata/pancake.rs asserting `decoded.protocol_type ==
ProtocolType::V3`, fees en pips raw y decode del 7-tuple sin deadline (análogos a
univ3.rs:415-482); round-trip en route_decoder.rs (legs[0].protocol_type == V3 y
router_kind local == PancakeV3, análogo a :501/:630); correlativo del encoder
tx_builder.rs como vector de prueba (mismo selector computado). Prerequisito confirmado
por esta errata: NO existe decoder Pancake hoy (calldata/ = univ2, univ3,
universal_router solamente) — el "posible GAP de decoder: HYPOTHESIS" del D-1 original
queda PROMOVIDO a GAP CONFIRMADO (ls calldata/ + mod.rs:101-108).
