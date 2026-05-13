# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Sprint 5 â€” Plan de implementaciÃ³n

Orden lineal. Cada paso compila (o marca dep faltante) antes del siguiente.
Paper mode ON hasta que estÃ©s listo para operar real.

## Paso 1 â€” Config + schema

- `configs/app.toml`: `paper_mode=true`, `max_value_eth=1.0`, `target_block_offset`, `max_inclusion_wait_blocks`, `priority_fee_increment_pct`, `flashbots_submit_timeout_ms`.
- `configs/schemas/app.schema.json`: aÃ±adir a `execution` las keys nuevas.
- `relays[].endpoint` opcional (para flashbots = `https://relay.flashbots.net`, resto vacÃ­o).
- `shared-ts/src/config/index.ts`: Zod.
- `backend/shared-rs/src/config.rs`: structs extendidos.

## Paso 2 â€” shared-rs: ExecutionStatus aÃ±adir `NotSubmitted`

- `contracts.rs`: enum extendido + serde rename `not_submitted`.
- `configs/schemas/execution_result.schema.json`: enum aÃ±ade `"not_submitted"`.

## Paso 3 â€” relays-client: signer.rs

- `struct Signer { wallet: LocalWallet, address: Address }`
- `pub fn from_env() -> Result<Option<Self>>` â€” lee `FLASHBOTS_SIGNER_KEY`; None si ausente.
- Impl `Drop` para `zeroize` de la memoria interna.
- MÃ©todo `sign_typed_tx(tx: TypedTransaction) -> Signature`.
- MÃ©todo `flashbots_auth_header(body: &[u8]) -> String`.

## Paso 4 â€” relays-client: nonce_manager.rs

- `struct NonceManager { map: RwLock<HashMap<(u64, Address), u64>>, provider: Arc<Provider<Http>> }`
- `async fn next(&self, chain_id, addr) -> u64` â€” returns + increments atomically with Semaphore per-address.
- `async fn refresh(&self, chain_id, addr) -> u64` â€” eth_getTransactionCount.

## Paso 5 â€” relays-client: bundle_builder.rs

- Reutiliza `tx_builder.rs` de sim-ctl (extraÃ­do a shared-rs o copiado con comentario "keep synced with sim-ctl").
- Build signed EIP-1559 tx with max_fee/priority_fee from fee history.
- Output: `SignedBundle { txs: Vec<Bytes>, target_block: u64, opportunity_id }`.
- Checks `tx.value > max_value_eth` â†’ `BuildError::ValueExceedsCap`.

## Paso 6 â€” relays-client: relay_flashbots.rs

- `struct FlashbotsClient { url: String, http: reqwest::Client, signer: Signer }`
- `async fn send_bundle(&self, bundle: &SignedBundle) -> Result<RelayResponse>`
- X-Flashbots-Signature header construction.
- Response parse: `bundleHash`, possible `simError`.

## Paso 7 â€” relays-client: relay_mev.rs (stub)

- Generic stub matching MEV-Boost relay API. Returns `NotSupportedYet` for bloxroute/eden/beaver/titan in S5.

## Paso 8 â€” relays-client: submit_engine.rs

- Orchestrator: kill-switch check â†’ paper-mode check â†’ build â†’ parallel submit to enabled relays â†’ track inclusion â†’ persist â†’ publish.
- `pub async fn execute(&self, opp) -> ExecutionResult`

## Paso 9 â€” relays-client: tracker.rs

- `async fn wait_for_inclusion(bundle_hash, provider, max_blocks) -> InclusionOutcome`
- Polls `eth_getTransactionReceipt` every block up to max_blocks.

## Paso 10 â€” relays-client: persistence.rs + consumer.rs

- Persistence: INSERT executions, UPDATE opportunities, UPSERT relay_scores. Transactional.
- Consumer: XREADGROUP `arbx:opps:simulated`, group `relays-client-g0`, XACK post-persist.

## Paso 11 â€” relays-client: http.rs + main.rs

- `/execute` hot-path, same decision tree as consumer, but synchronous response.
- Main spawns HTTP + consumer only if signer loaded; otherwise HTTP stays up returning 501 (same as S1).

## Paso 12 â€” MÃ©tricas

- AÃ±adir a `shared-rs/src/metrics.rs` las 6 nuevas mÃ©tricas S5.

## Paso 13 â€” Tests

- `signer_test.rs`: load valid key â†’ address matches; invalid key â†’ error; empty â†’ None.
- `nonce_test.rs`: increment in order, handle refresh.
- `bundle_builder_test.rs`: v2/v3 builds, value cap enforcement, paper-mode mark.
- `submit_engine_test.rs`: paper-mode returns `NotSubmitted`; kill-switch returns `dropped`.

## Paso 14 â€” ValidaciÃ³n + commit + push

- Python schema.
- Fake-data scan.
- Commit + push.

## Out-of-scope S5 (explÃ­cito)

- Multi-tx bundles (S6).
- Adaptive relay scoring beyond included/submitted (S6).
- Bloxroute/eden/beaver/titan real clients (S5.1 per-relay).
- MEV-Share (S7+).
- Gas fee estimation ML model (S7+).

