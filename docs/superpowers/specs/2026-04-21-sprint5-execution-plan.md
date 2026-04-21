# Sprint 5 — Plan de implementación

Orden lineal. Cada paso compila (o marca dep faltante) antes del siguiente.
Paper mode ON hasta que estés listo para operar real.

## Paso 1 — Config + schema

- `configs/app.toml`: `paper_mode=true`, `max_value_eth=1.0`, `target_block_offset`, `max_inclusion_wait_blocks`, `priority_fee_increment_pct`, `flashbots_submit_timeout_ms`.
- `configs/schemas/app.schema.json`: añadir a `execution` las keys nuevas.
- `relays[].endpoint` opcional (para flashbots = `https://relay.flashbots.net`, resto vacío).
- `shared-ts/src/config/index.ts`: Zod.
- `backend/shared-rs/src/config.rs`: structs extendidos.

## Paso 2 — shared-rs: ExecutionStatus añadir `NotSubmitted`

- `contracts.rs`: enum extendido + serde rename `not_submitted`.
- `configs/schemas/execution_result.schema.json`: enum añade `"not_submitted"`.

## Paso 3 — relays-client: signer.rs

- `struct Signer { wallet: LocalWallet, address: Address }`
- `pub fn from_env() -> Result<Option<Self>>` — lee `FLASHBOTS_SIGNER_KEY`; None si ausente.
- Impl `Drop` para `zeroize` de la memoria interna.
- Método `sign_typed_tx(tx: TypedTransaction) -> Signature`.
- Método `flashbots_auth_header(body: &[u8]) -> String`.

## Paso 4 — relays-client: nonce_manager.rs

- `struct NonceManager { map: RwLock<HashMap<(u64, Address), u64>>, provider: Arc<Provider<Http>> }`
- `async fn next(&self, chain_id, addr) -> u64` — returns + increments atomically with Semaphore per-address.
- `async fn refresh(&self, chain_id, addr) -> u64` — eth_getTransactionCount.

## Paso 5 — relays-client: bundle_builder.rs

- Reutiliza `tx_builder.rs` de sim-ctl (extraído a shared-rs o copiado con comentario "keep synced with sim-ctl").
- Build signed EIP-1559 tx with max_fee/priority_fee from fee history.
- Output: `SignedBundle { txs: Vec<Bytes>, target_block: u64, opportunity_id }`.
- Checks `tx.value > max_value_eth` → `BuildError::ValueExceedsCap`.

## Paso 6 — relays-client: relay_flashbots.rs

- `struct FlashbotsClient { url: String, http: reqwest::Client, signer: Signer }`
- `async fn send_bundle(&self, bundle: &SignedBundle) -> Result<RelayResponse>`
- X-Flashbots-Signature header construction.
- Response parse: `bundleHash`, possible `simError`.

## Paso 7 — relays-client: relay_mev.rs (stub)

- Generic stub matching MEV-Boost relay API. Returns `NotSupportedYet` for bloxroute/eden/beaver/titan in S5.

## Paso 8 — relays-client: submit_engine.rs

- Orchestrator: kill-switch check → paper-mode check → build → parallel submit to enabled relays → track inclusion → persist → publish.
- `pub async fn execute(&self, opp) -> ExecutionResult`

## Paso 9 — relays-client: tracker.rs

- `async fn wait_for_inclusion(bundle_hash, provider, max_blocks) -> InclusionOutcome`
- Polls `eth_getTransactionReceipt` every block up to max_blocks.

## Paso 10 — relays-client: persistence.rs + consumer.rs

- Persistence: INSERT executions, UPDATE opportunities, UPSERT relay_scores. Transactional.
- Consumer: XREADGROUP `arbx:opps:simulated`, group `relays-client-g0`, XACK post-persist.

## Paso 11 — relays-client: http.rs + main.rs

- `/execute` hot-path, same decision tree as consumer, but synchronous response.
- Main spawns HTTP + consumer only if signer loaded; otherwise HTTP stays up returning 501 (same as S1).

## Paso 12 — Métricas

- Añadir a `shared-rs/src/metrics.rs` las 6 nuevas métricas S5.

## Paso 13 — Tests

- `signer_test.rs`: load valid key → address matches; invalid key → error; empty → None.
- `nonce_test.rs`: increment in order, handle refresh.
- `bundle_builder_test.rs`: v2/v3 builds, value cap enforcement, paper-mode mark.
- `submit_engine_test.rs`: paper-mode returns `NotSubmitted`; kill-switch returns `dropped`.

## Paso 14 — Validación + commit + push

- Python schema.
- Fake-data scan.
- Commit + push.

## Out-of-scope S5 (explícito)

- Multi-tx bundles (S6).
- Adaptive relay scoring beyond included/submitted (S6).
- Bloxroute/eden/beaver/titan real clients (S5.1 per-relay).
- MEV-Share (S7+).
- Gas fee estimation ML model (S7+).
