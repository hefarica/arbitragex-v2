# FASE 1 — Base git + mapa del pipeline (T0)

Fecha: 2026-10-23 · Autor: integrador (t1, attempt f7c2c1da) · Repo: hefarica/arbitragex-v2

## 1. Base git

| Ítem | Valor | Comando de verificación |
|---|---|---|
| Clone | `C:\Users\HFRC\Desktop\repos\arbitragex-v2` (completo, NO shallow) | `git rev-parse --is-shallow-repository` → `false` |
| SHA base | `f27bdb33de026c0f6fa5edcbc1b94438b20af663` | `git rev-parse HEAD`; coincide con `origin/main` tras `git fetch origin main` ejecutado INMEDIATAMENTE antes de branchar (regla 2 de `arbx-ci-cd-prolijo`) |
| PRs vivos por los 5 problemas P0 | NINGUNO | `gh pr list --state open --limit 50` (50 abiertos, ninguno con título/rama de TWO-LEG-01, OPTIMAL-SIZE-01, POBLACION-FILTER-01, REVERT-TRACE-01, ATOMIC-CAUSE-01); búsqueda `--state all` por cada keyword: sin match exacto. Historial RELACIONADO (no duplicado): #700 `V3-MULTILEG-SIZING-01` (MERGED), #694 `HOPS-EMIT-01` (MERGED), #690 per-hop ledger (CLOSED), #975 `SIM-ORACLE-01` (OPEN, sim oracle, distinto problema) |
| Skill cargada | `arbx-ci-cd-prolijo` | cargada antes de branchar |

### Worktrees (todos en `f27bdb33`, desde `origin/main` fresco)

| Rama | Worktree |
|---|---|
| `feat/two-leg-01` | `C:\Users\HFRC\Desktop\repos\arbx-wt\two-leg-01` |
| `feat/optimal-size-01` | `C:\Users\HFRC\Desktop\repos\arbx-wt\optimal-size-01` |
| `feat/poblacion-filter-01` | `C:\Users\HFRC\Desktop\repos\arbx-wt\poblacion-filter-01` |
| `feat/revert-trace-01` | `C:\Users\HFRC\Desktop\repos\arbx-wt\revert-trace-01` |
| `feat/atomic-cause-01` | `C:\Users\HFRC\Desktop\repos\arbx-wt\atomic-cause-01` |
| `docs/rea-base` | `C:\Users\HFRC\Desktop\repos\arbx-wt\docs-rea-base` |

Verificación: `git worktree list` → 7 entradas, todas en `f27bdb33`.

## 2. Mapa del pipeline (rutas + líneas exactas, sobre `f27bdb33`)

### 2.1 Emisor de rutas del searcher → INSERT (TWO-LEG-01)

**El embudo único**: `backend/searcher-rs/src/persistence.rs`
- `insert_opportunity_with_route(pool, o, route)` — L80. Único funil de TODA escritura de `opportunities` del crate (comentario L205-213 lo declara: "single funnel for every opportunities write").
- `INSERT INTO opportunities (...)` — **L148-166**. Columnas planas: `dex_a, dex_b, pair_symbol, token_in, token_out` + `route_metadata` JSONB (L153) + `status` derivado por `status_from_rejection_reason` (L144, CARDS-MIRROR-01) + `economics` (L192-201, ALWAYS-COMPUTE).
- `build_route_metadata_from_plan` — L233: topología completa (token_addresses = hops+1, pool_addresses ≤ hops, dex_adapters = hops; `decimals` queda VACÍO — fail-honest declarado L226-230).
- Gate estructural L109-110: `token_addresses.len() == hops+1 && pool_addresses.len() <= hops`, si falla persiste `{}`.

**Productores que llaman al INSERT**:
- `opportunity_emitter.rs:770` — `persistence::insert_opportunity_with_route` en el emitter del orquestador (entry points: `emit_accepted` L241, `emit_accepted_with_plan` L251, `emit_rejected` L483; latencia estampada L406/L537).
- `scanner.rs` — L2497, L2705, L2751, L2804, L3145.
- `hop_cycle_bridge.rs:1595` — puente discovery→emission (ciclos 3..=7 hops, heredado de #694 HOPS-EMIT-01).
- `flashloan_arb_worker.rs:1128`, `liquidation_worker.rs:1118`, `triangular_worker.rs:1670/2099`.

**Dónde se pierde la segunda pata en las columnas planas**:
- `route_discovery/hop_cycle_bridge.rs:737-744` — la `Opportunity` del ciclo N-hop lleva SOLO `dex_a = dex_adapters[0]`, `dex_b = dex_adapters.get(1)` (comentario L741-742: "A cycle has no single second counter-venue: the first two hops are the two venues"). El resto de las patas vive únicamente en `route_metadata`.
- `engines/dex_engine.rs:1302-1328` — el engine 2-pool sí llena `dex_a = pool_a.dex_name` (L1306) y `dex_b = pool_b.dex_name` (L1307); `token_in == token_out` = token base del ciclo cerrado (L1282-1283, CANDIDATE-POSTSIZE-SYNC-01).
- Estrategias no-cíclicas dejan `dex_b: None` (e.g. `triangular_engine.rs:602`, `flashloan_engine.rs:404`, `liquidation_worker.rs:1069` usa `dex_b` como etiqueta `user:<addr>` L1044-1069).

### 2.2 Sizing con fallback dustUsd (OPTIMAL-SIZE-01)

Todo en `backend/searcher-rs/src/size_optimizer.rs` (8451 líneas):
- `const DUST_NOTIONAL_USD: f64 = 0.01;` — **L115** (DUST-NOTIONAL-02, medido post-deploy: 362 filas/3 min con principal $8.7e-13).
- Kernel 2-leg V2: `golden_section_search_2leg(x_lo, x_hi, ...)` — llamada **L2753**, definición **L3650**; bracket inferior = dust floor (**L1936-1948**: `lo = (DUST_NOTIONAL_USD * wei_per_usd).ceil()`, si `cap < lo` → rechazo `NoTradableSize` L1948-1949).
- **Reprice-at-band (el "fallback dust")**: **L2763-2801** — si `usd_of(x_star_raw) < DUST_NOTIONAL_USD`: si también `usd_of(x_hi) < DUST` → `Rejected(NoTradableSize)` L2777-2784; si no → republica en `x_hi` (banda capital/slippage) con evento `two_leg_dust_optimum_repriced_at_band` L2789-2797.
- Curva negativa: **L2803-2870** — `profit_wei <= 0` → `RejectedComputed(NonPositiveProfit)` publicado en el extremo de capital `x_hi`, NO en el argmax dust (DUST-NOTIONAL-01: 4087 filas/20 min corregidas).
- Grid N-leg V3: bracket log-espaciado **L3131-3160** (16 puntos, `V3_MULTILEG_LOCAL_POINTS` L108); presupuesto por bloque L185-194.
- Joint sizing: `size_optimizer/joint_sizing.rs:2626` (`dust_floor_usd: Decimal` en el request) y L2722-2731 (conversión USD→base units del floor).
- Kelly: `kelly_sizing.rs:56` (`kelly_fraction`), L90 (`fractional_kelly`), L119 (`compute_position_size`).
- **Hueco REA**: `amm_math.rs` NO tiene input óptimo en forma cerrada — sólo `v2_amount_out` (L96), `v3_amount_out_single_tick` (L159), `mul_div` (L127). El repo depende enteramente de golden-section/grid. OPTIMAL-SIZE-01 porta la forma cerrada upstream (con su vector numérico como test).

### 2.3 Export de población del sim (POBLACION-FILTER-01)

**Productor**: `scripts/gsim1_variance_export.sql` (86 líneas) — corre EN el VPS:
`docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At < scripts/gsim1_variance_export.sql > /tmp/gsim1/input.jsonl`
- Query **L39-86**: `SELECT DISTINCT ON (dex_a, token_in, token_out, route_metadata->'pool_addresses')` la detección MÁS FRESCA por topología.
- **Precondiciones (L58-80), todas son filtros de FORMA, no de valor** (RULE 00, L8-25):
  1. `chain_id = 1` (L58) — el harness corre el path wrapped-flash mainnet.
  2. `route_metadata` NO vacío con `dex_adapters` presente (L59-61).
  3. `jsonb_array_length(dex_adapters) = 2` (L62) — SOLO rutas 2-pata (el encoder A.3.a sólo codifica round-trips de 2).
  4. Ambas patas ∈ {uniswapv2, uniswap-v2, sushi, sushiswap} case-insensitive (L74-77) — el conjunto EXACTO de `adapter_to_semantic()` en variance_benchmark.rs (L182-188). ADAPTER-CASE-01 (L64-73): el filtro original era case-sensitive exact-match CamelCase y el scanner persiste kebab-case → instrumento que mentía "0 topologías".
  5. `amount_in_wei IS NOT NULL AND <> '0'` (L78-79).
  6. `detected_at > now() - interval '2 hours'` (L80) — ventana que el harness puede pinear (tip − 1100 bloques ≈ 3.7h); ampliarla sólo infla dedup/stale (L21-25).
  7. `LIMIT 500` (L86) — válvula sobre la población DISTINCT, no un corte de muestreo (G-SIM1-AUTOREFRESH, L27-37: 800 crudos → 779 dedup → 21 distinct).
- **Consumidor**: `backend/sim-core/tests/variance_benchmark.rs` — lee `VARIANCE_INPUT` (JSONL, L17/L358), `RPC_HTTP_1`/`ALCHEMY_HTTP_URL` (L344-346), `ARBITRAGE_EXECUTOR` + `FLASHLOAN_EXECUTOR_1` + `GAS_PRICE_WEI` (L351-356), `VARIANCE_MIN_SAMPLES` (default 100, L21), `VARIANCE_MAX_MEAN_DRIFT_PCT` (default 5.0, L22). Driver: `scripts/gsim1_variance_benchmark.sh`. Identity por muestra: G-SIM1-SAMPLE-IDENTITY-01 (L110-151, `SampleFate`).

### 2.4 Manejo del revert en sim/executor (REVERT-TRACE-01 + ATOMIC-CAUSE-01)

**El revert ANÓNIMO está en el CONTRATO**:
- `contracts/src/ArbitrageExecutor.sol:508-513` — `_boundedRouterCall`: `(bool success,) = router.call(pld); if (!success) revert SwapFailed();` — **el returndata (2do elemento del tuple) se DESCARTA**: la causa real del router (`Error(string)`, selector de error custom, Panic) jamás sube.
- `contracts/src/FlashLoanExecutor.sol:392-393 y 419-420` — mismo patrón: `(bool success,) = arbitrageExecutor.call(userData); if (!success) revert FL_ArbitrageExecutionFailed();`.
- Consecuencia medible: cuando el sim revplaya `executeArbitrage`, el `reason` que ve REVM es `SwapFailed` (el error del WRAPPER), no `TransferHelper: INSUFFICIENT_OUTPUT` de la pata real.

**Dónde el sim SÍ nombra el revert**:
- REVM multistep (path producción paper): `backend/sim-core/src/sim_multistep.rs:832-842` — `CallOutcome::Reverted { reason }` → `fail_reason = "multistep_call_revert:{reason}"`; `CallOutcome::Halted` → `multistep_call_halt:{reason}`. Infra → `multistep_call_infra:{tag}` L824-830.
- Tracer frame-a-frame: `backend/simulator-v2/src/sequence_runner.rs:287-298` — `RevertTracer` EXISTE pero es **test-only, feature-gated** (`#[cfg(feature = "revert-trace")]`, feature en `simulator-v2/Cargo.toml:37` y `sim-core/Cargo.toml:38`); el default prod usa `NoopInspector` (sim_multistep.rs:631-642). REVERT-TRACE-01 = llevar el nombre del frame que falló al fail_reason de producción sin forkear el path (L631-633 declara esa intención).
- sim-ctl anvil: `sim_engine.rs:333-342` — `extract_revert_reason` matchea strings sobre `ProviderError`; sin marker `"reverted:"` TODO queda como `rpc_error: <truncado>` (el sink anónimo del lado eth_call). `revm_backend.rs:258-267` — `SimError::Reverted(reason)` → `revm_reverted: {reason}` (truncado 200 chars L313).
- **Clasificador gap vs veredicto**: `backend/sim-ctl/src/persistence.rs:130-244` — `is_sim_capability_gap` (SIN catch-all): `execution_reverted` y `multistep_call_revert:*` QUEDAN como rechazo (test L355-369, L399-411); las familias `funding_*` de harness son gaps con prefijo namespaced (L231-243, test L432-457). Si REVERT-TRACE-01 agrega familias nuevas de fail_reason, DEBEN también clasificarse acá o un gap se vuelve rechazo silencioso (F-01, L136-157).
- relays-client: `tracker.rs:23` (`InclusionOutcome::Reverted{block}`), `submit_engine.rs:885-901` (→ `ExecutionStatus::Reverted`, `error_message = "on_chain_revert"` — mensaje genérico, sin causa del receipt), `persistence.rs:49` (mapeo a 'failed'). Pre-broadcast: `relay_flashbots.rs:195-216` (`eth_callBundle` tx-level `revert: Option<String>` — la causa SÍ existe acá y se propaga en `Drop` reason `submit_engine.rs:1064-1067`).

### 2.5 Contratos executor + bindings (ATOMIC-CAUSE-01)

- `contracts/src/ArbitrageExecutor.sol` (638 líneas): custom errors SC-3 L37-108 (`SwapFailed` L50, `InsufficientProfit` L67, `ZeroIntermediate` L87, `UnsupportedRouteLength` L92, `AliasedTwoLegRoute` L108, `TokenOutRetentionViolation` L101). Entrypoints: `executeArbitrage` **L252** (7 args), `executeArbitrageFlashFunded` **L305**. Gate de forma de ruta **L375**: `routers.length > 2 → revert UnsupportedRouteLength` (2 patas máximo on-chain). Validación **L360-387** (LengthMismatch, TokenNotApproved fail-closed L366-367, AliasedTwoLegRoute L384).
- `contracts/src/FlashLoanExecutor.sol`: entrypoint `requestFlashLoan` (selector 0x5107d61e, verified en encoder L64-69); llamadas anidadas L392/L419.
- Tests de referencia: `contracts/test/ArbitrageExecutor.t.sol`, `ArbitrageExecutorFuzz.t.sol`, `ArbitrageExecutorCrossDexRepro.t.sol`, `ExecutorInvariant.t.sol`.
- **Bindings (single source of truth)**: `backend/prioritization-spine/src/execute_arbitrage_encoder.rs` — `EXECUTE_ARBITRAGE_SELECTOR = 0x76d81cdf` (**L55**), `EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR = 0xdde0bf51` (**L62**), `REQUEST_FLASH_LOAN_SELECTOR = 0x5107d61e` (**L69**); ambos selectores comparten UN body de 7 args (L71-80). Consumidores del encoder: `sim_multistep.rs`, `round_trip_executor.rs`, `relays-client/src/bundle_builder.rs`, `mcp-sim-engine`, `sim-core/sim_prefund.rs`.
- NOTA para ATOMIC-CAUSE-01: no hay ABI JSON generado ni bindings ethers `abigen` para el executor — los bindings son los encoders a mano anteriores. Si el PR agrega decode de returndata, el enum de custom errors L37-108 es el catálogo a decodificar.

## 3. Esquema DB real (PG del VPS, medido hoy)

DB: `arbitragex` en `arbitragex-v2-postgres-1` (VPS). Medido con las herramientas sql del harness contra la DB viva.

| Tabla | Filas (medidas/estimadas) | Columnas clave |
|---|---|---|
| `opportunities` | **4,809,001** (medido COUNT) | `status` text NOT NULL, `dex_a` text NOT NULL, **`dex_b` text NULL**, `rejection_reason` text NULL, `route_metadata` jsonb NOT NULL, `cartridge_id` text NULL |
| `simulations` | 510,111 (estimado pg_stat) | `opportunity_id` uuid NOT NULL, `fail_reason` text NULL |
| `executions` | **0** | `status` text NOT NULL, `opportunity_id`, `error_message`, `raw_receipt` jsonb |

- `opportunities_status_check` (CHECK vivo): `status IN ('detected','validated','simulated','scored','executing','executed','reconciled','rejected','failed')`.
- **Distribución ACTUAL de `opportunities.status`: 100% `rejected` (4,809,001 filas, GROUP BY devolvió UN solo grupo).**
- `dex_b` en `opportunities`: 4,595 NULL vs 4,804,006 pobladas (0.095% NULL) — `dex_b` poblado NO es la excepción; la segunda pata EXISTE como texto plano, lo que falta es la topología por pata (TWO-LEG-01).
- `executions_status_check` (CHECK vivo): `status IN ('submitted','included','reverted','dropped','replaced','not_implemented')` — **NO existe el valor 'executed' en `executions`**.
- **PUERTA DE FASE (conclusión del esquema)**: la fila `status=executed` de la puerta vive en **`opportunities`** (`WHERE status='executed'`, hoy 0 filas). `SELECT COUNT(*) FROM opportunities WHERE status='executed';` es el comando de la puerta. Quien instrumente la puerta NO debe escribirla en `executions` (el CHECK lo rechazaría).
- `route_metadata` jsonb NOT NULL con 4,809,001/4,809,001 pobladas (medido FILTER `::text <> '{}'`).

## 4. Restricciones respetadas

- Sin cambios de código de producción: este PR agrega únicamente `docs/rea/FASE1_BASE.md`.
- `configs/app.toml`, `backend/relays-client/src/live_exec_policy.rs`, `backend/relays-client/src/bundle_builder.rs`: NO tocados (`git diff origin/main` sólo muestra este archivo).
- Worktrees listos para los 5 PRs; cada sesión dueña conserva su árbol.

## 5. Comandos y salidas (fail-honest)

| # | Comando | Salida clave |
|---|---|---|
| 1 | `git rev-parse HEAD` (clone) | `f27bdb33de026c0f6fa5edcbc1b94438b20af663` |
| 2 | `git rev-parse --is-shallow-repository` | `false` |
| 3 | `git fetch origin main && git rev-parse origin/main` | `f27bdb33…` == HEAD (SAME) |
| 4 | `gh pr list --state open --limit 50` | 50 abiertos, 0 por los 5 P0 |
| 5 | `git worktree add -b feat/<x> … origin/main` ×5 | HEAD `f27bdb33` ×5 |
| 6 | `git worktree list` | 7 worktrees, todos `f27bdb33` |
| 7 | sql_tables (harness) | 94 tablas; opportunities 4,806,635 est., executions 0 est. |
| 8 | `SELECT status, COUNT(*) FROM opportunities GROUP BY status` | `rejected|4809001` (único grupo) |
| 9 | pg_constraint executions | CHECK sin 'executed' |
| 10 | pg_constraint opportunities | CHECK con 'executed' |
| 11 | dex_b/route_metadata COUNT FILTER | 4595 NULL dex_b; 4809001 route_metadata ≠ '{}' |

Huecos declarados (no computado ≠ cero):
- Filas exactas de `simulations` NO contadas con COUNT (sólo estimación 510,111 de pg_stat_user_tables) — no fue necesario para la puerta.
- `paper_trade_runs` estimadas en 0 filas por pg_stat (no verificadas con COUNT).
- No se verificó si algún job/CI externo escribe `opportunities.status='executed'` en el futuro pipeline S5+ — el mapa muestra que hoy NADIE lo escribe (todo rejected).
