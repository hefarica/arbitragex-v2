# Sprint 4 — Plan de implementación

Orden lineal. Cada paso compila (o documenta dependency faltante) antes del siguiente.

## Paso 1 — Config + schema

- `configs/app.toml`: sección `[simulation]`.
- `configs/schemas/app.schema.json`: schema para `simulation`.
- `shared-ts/src/config/index.ts`: Zod (opcional S4, sim-ctl es Rust).
- `backend/shared-rs/src/config.rs`: struct `SimulationCfg` + parse.

**Validación**: `python3 automation/tools/validate-config.py`.

## Paso 2 — Docker compose: anvil service

- Añadir `anvil` con `profiles: ["sim"]`, image `ghcr.io/foundry-rs/foundry:latest`.
- Healthcheck via `eth_blockNumber` JSON-RPC.
- `sim-ctl` gana `depends_on: anvil: { condition: service_healthy }` cuando se levanta con profile `sim`.
- `networks: [arbx-net]`.

## Paso 3 — sim-ctl: fork_manager.rs

- `struct ForkManager { provider: Arc<Provider<Http>>, pool: Semaphore<N> }`
- `async fn acquire(&self) -> Result<ForkHandle>` — snapshot + return handle
- `async fn release(&mut self, handle: ForkHandle)` — evm_revert
- `async fn reset_fork(&self) -> Result<()>` — anvil_reset
- `async fn health(&self) -> bool` — eth_blockNumber

## Paso 4 — sim-ctl: tx_builder.rs

- `fn build_probe(opp, chain_id, signer) -> Result<ProbeTx, BuildError>`
- Para UniV2: selector `swapExactTokensForTokens` con `amountOutMin=1`, `path=[in,out]`, `deadline=+60s`.
- Para UniV3: selector `exactInputSingle` con `fee=3000`, `amountOutMinimum=0`.
- Para otros: `BuildError::UnsupportedStrategy`.

## Paso 5 — sim-ctl: sim_engine.rs

- `async fn simulate(opp, handle) -> SimulationResult`
- `provider.call(tx, block)` + `estimate_gas(tx)`.
- Decodifica output UniV2/V3 para obtener `amountOut`.
- Calcula slippage: `(amount_out_min - actual_out) / amount_out_min * 100`.
- Decodifica revert reason si aplica.

## Paso 6 — sim-ctl: persistence.rs

- `async fn insert_simulation(pool, result) -> Result<()>`
- `async fn update_opportunity_after_sim(pool, opp_id, result)` — status transition.

## Paso 7 — sim-ctl: consumer.rs

- Similar a selector-api: XREADGROUP `arbx:opps:validated` group `sim-ctl-g0`.
- Por mensaje: `fork.acquire() → sim_engine.simulate() → fork.release() → persist → publish?`.
- XACK post-persist.
- CB `sim_engine` guard.

## Paso 8 — sim-ctl: http.rs + main.rs

- `/simulate` POST body `{opportunity_id}` o el Opportunity entero: invoca sim_engine en hot-path.
- Si `simulation.provider = "not_implemented"` o sin `ANVIL_URL`: responde 501 con payload canon.
- `main.rs`: spawn HTTP + spawn consumer, graceful shutdown.

## Paso 9 — Tests

- `backend/sim-ctl/tests/tx_builder_test.rs`: UniV2 + UniV3 fixtures, plus unsupported strategy.
- `backend/sim-ctl/tests/sim_engine_mock_test.rs`: con un provider mock retornando revert vs success.
- Unit tests inline en fork_manager (sin anvil real — mock Provider).

## Paso 10 — Dockerfile updates

- `backend/sim-ctl/Dockerfile`: ya existe, solo asegurar que nuevos archivos se incluyan (mismo contexto).

## Paso 11 — Validación

- `cargo metadata --no-deps` → 5 crates OK.
- Python schema validation.
- Commit + push.

## Out-of-scope S4 (explícito)

- Ejecución privada Flashbots (S5).
- Construcción de counter-trade arb (S5).
- Liquidation sim (S4.1).
- Flash loan wrap (S5).
