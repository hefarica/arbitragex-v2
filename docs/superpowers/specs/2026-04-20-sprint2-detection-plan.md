# Sprint 2 — Plan de implementación

Ejecución en 7 pasos acumulativos. Cada paso **debe** compilar o validar antes del siguiente.

## Paso 1 — Shared-rs: catálogo de chains + routers

- `backend/shared-rs/src/chains.rs`
  - `enum RouterKind { UniswapV2, UniswapV3, Sushi, Curve, Unknown }`
  - `struct RouterEntry { chain_id: u64, name: String, kind: RouterKind, address: [u8;20] }`
  - Catálogo estático para chain_id=1 (UniV2 router + UniV3 SwapRouter).
  - `pub fn routers_for_chain(chain_id: u64) -> &'static [RouterEntry]`
- Export desde `lib.rs`.
- **Validación**: compile lib → ok.

## Paso 2 — Dependencias

- `backend/searcher-rs/Cargo.toml`: añadir `ethers = "2"`, `sqlx = { workspace = true }`, `futures-util`, `lru`, `hex`.
- Workspace `backend/Cargo.toml`: añadir `ethers` y `futures-util` a `workspace.dependencies`.
- **Validación**: `cargo metadata` → 5 packages, todos con deps resueltas.

## Paso 3 — `chain_client.rs`

- Wrapper `WsChainClient { provider: Arc<Provider<Ws>>, chain_id: u64 }`.
- `pub async fn connect(url: &str) -> anyhow::Result<Self>` — con timeout 10 s.
- `pub async fn subscribe_pending(&self) -> Result<impl Stream<Item = H256>>` — usa `provider.subscribe_pending_txs()`.
- `pub async fn get_tx(&self, hash: H256) -> Result<Option<Transaction>>` — `getTransactionByHash`.
- **Validación**: compile only. No runtime sin RPC real.

## Paso 4 — `calldata/`

- `calldata/mod.rs`: `pub fn decode(input: &[u8], router: RouterKind) -> Option<DecodedSwap>`
- `calldata/univ2.rs`: selectores conocidos (`swapExactTokensForTokens 0x38ed1739`, `swapTokensForExactTokens 0x8803dbee`, `swapExactETHForTokens 0x7ff36ab5`, `swapExactTokensForETH 0x18cbafe5`, `swapTokensForExactETH 0x4a25d94a`, `swapETHForExactTokens 0xfb3bdb41`). Decode con ABI embebido.
- `calldata/univ3.rs`: selectores (`exactInputSingle 0x414bf389`, `exactInput 0xc04b8d59`, `exactOutputSingle 0xdb3e2198`, `exactOutput 0xf28c0498`).
- `DecodedSwap` implementa `Serialize`.
- **Validación**: cargo test con ≥ 4 fixtures de calldata reales por router (de etherscan).

## Paso 5 — `patterns/dex_arb.rs`

- `pub fn build_candidate(tx: &Transaction, swap: &DecodedSwap, chain_id: u64) -> Opportunity`
- Mapea `DecodedSwap` → `Opportunity { strategy_kind: DexArb, dex_a: "uniswap-v2|v3", token_in, token_out, amount_in_wei, expected_profit_usd: 0.0, roi_pct: None, ... }`
- Generates `id = uuid::Uuid::new_v4()`, `trace_id = uuid::Uuid::new_v4()`, `detected_at = Utc::now()`.
- **Validación**: unit test con swap sintético.

## Paso 6 — `persistence.rs`

- `pub async fn insert_opportunity(pool: &PgPool, o: &Opportunity) -> Result<()>` — `INSERT INTO opportunities (...) VALUES ($1,...) ON CONFLICT (id) DO NOTHING`.
- Conecta con `DATABASE_URL`. Si env ausente, skip DB con log warn.
- **Validación**: compile + (opcional) integration test con DB.

## Paso 7 — `scanner.rs` + `publisher.rs` + `main.rs` + `dedup.rs`

- `dedup.rs`: LRU<H256, ()> con capacidad configurable + Redis SETNX con TTL.
- `scanner.rs`:
  - Entry `run(cfg, killswitch, redis, pg).await` reemplaza completo lo actual.
  - Lee `RPC_WS_<chain_id>` por cada chain habilitada en config.
  - Si ausente: gauge state=no_rpc, idle loop 5 s.
  - Si presente: conectar `chain_client`, subscribe pending, consumir stream, filtrar por `tx.to ∈ routers`, decode, match pattern, dedup, publish + persist.
  - Cada 5 s: revisar kill-switch.
- `publisher.rs`:
  - `pub async fn publish(opp: &Opportunity, redis: &mut ConnectionManager) -> Result<()>` con `XADD arbx:opps:detected MAXLEN ~ 10000 *` + field `json`.
  - Métricas.
- `main.rs`:
  - Spawna una `scanner::run` por chain habilitada (tokio::spawn) + HTTP server.
  - Shutdown limpio con `tokio::signal::ctrl_c`.

## Paso 8 — Tests embebidos

- `backend/searcher-rs/tests/calldata_test.rs`: 8 fixtures (4 UniV2 + 4 UniV3) con calldata real de mainnet (hex strings) → assert DecodedSwap fields.
- `backend/searcher-rs/src/patterns/dex_arb.rs::tests`: `build_candidate` produce Opportunity con campos correctos.

## Paso 9 — Validación final

- `cargo check -p searcher-rs`
- `cargo test -p searcher-rs`
- Python sanity: JSON schemas siguen válidos, app.toml sigue parseando.
- Smoke test conceptual (sin RPC): servicio sube, `/health` OK, gauge `state=no_rpc`, cero rows en DB.

## Out-of-scope S2 (documentado, no implementado)

- Sushi, Curve, Balancer decoders — stubs `Unknown` con counter `undecoded_total{reason="unknown_router"}`.
- Triangular / liquidation / backrun / flashloan patterns — no se intenta; logging explícito.
- Priority gas / MEV-Share / SUAVE — S5+.
- Block-level events (AccessList, inclusion race) — S3+.
