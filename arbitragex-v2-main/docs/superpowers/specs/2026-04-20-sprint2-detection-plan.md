# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Sprint 2 â€” Plan de implementaciÃ³n

EjecuciÃ³n en 7 pasos acumulativos. Cada paso **debe** compilar o validar antes del siguiente.

## Paso 1 â€” Shared-rs: catÃ¡logo de chains + routers

- `backend/shared-rs/src/chains.rs`
  - `enum RouterKind { UniswapV2, UniswapV3, Sushi, Curve, Unknown }`
  - `struct RouterEntry { chain_id: u64, name: String, kind: RouterKind, address: [u8;20] }`
  - CatÃ¡logo estÃ¡tico para chain_id=1 (UniV2 router + UniV3 SwapRouter).
  - `pub fn routers_for_chain(chain_id: u64) -> &'static [RouterEntry]`
- Export desde `lib.rs`.
- **ValidaciÃ³n**: compile lib â†’ ok.

## Paso 2 â€” Dependencias

- `backend/searcher-rs/Cargo.toml`: aÃ±adir `ethers = "2"`, `sqlx = { workspace = true }`, `futures-util`, `lru`, `hex`.
- Workspace `backend/Cargo.toml`: aÃ±adir `ethers` y `futures-util` a `workspace.dependencies`.
- **ValidaciÃ³n**: `cargo metadata` â†’ 5 packages, todos con deps resueltas.

## Paso 3 â€” `chain_client.rs`

- Wrapper `WsChainClient { provider: Arc<Provider<Ws>>, chain_id: u64 }`.
- `pub async fn connect(url: &str) -> anyhow::Result<Self>` â€” con timeout 10 s.
- `pub async fn subscribe_pending(&self) -> Result<impl Stream<Item = H256>>` â€” usa `provider.subscribe_pending_txs()`.
- `pub async fn get_tx(&self, hash: H256) -> Result<Option<Transaction>>` â€” `getTransactionByHash`.
- **ValidaciÃ³n**: compile only. No runtime sin RPC real.

## Paso 4 â€” `calldata/`

- `calldata/mod.rs`: `pub fn decode(input: &[u8], router: RouterKind) -> Option<DecodedSwap>`
- `calldata/univ2.rs`: selectores conocidos (`swapExactTokensForTokens 0x38ed1739`, `swapTokensForExactTokens 0x8803dbee`, `swapExactETHForTokens 0x7ff36ab5`, `swapExactTokensForETH 0x18cbafe5`, `swapTokensForExactETH 0x4a25d94a`, `swapETHForExactTokens 0xfb3bdb41`). Decode con ABI embebido.
- `calldata/univ3.rs`: selectores (`exactInputSingle 0x414bf389`, `exactInput 0xc04b8d59`, `exactOutputSingle 0xdb3e2198`, `exactOutput 0xf28c0498`).
- `DecodedSwap` implementa `Serialize`.
- **ValidaciÃ³n**: cargo test con â‰¥ 4 fixtures de calldata reales por router (de etherscan).

## Paso 5 â€” `patterns/dex_arb.rs`

- `pub fn build_candidate(tx: &Transaction, swap: &DecodedSwap, chain_id: u64) -> Opportunity`
- Mapea `DecodedSwap` â†’ `Opportunity { strategy_kind: DexArb, dex_a: "uniswap-v2|v3", token_in, token_out, amount_in_wei, expected_profit_usd: 0.0, roi_pct: None, ... }`
- Generates `id = uuid::Uuid::new_v4()`, `trace_id = uuid::Uuid::new_v4()`, `detected_at = Utc::now()`.
- **ValidaciÃ³n**: unit test con swap sintÃ©tico.

## Paso 6 â€” `persistence.rs`

- `pub async fn insert_opportunity(pool: &PgPool, o: &Opportunity) -> Result<()>` â€” `INSERT INTO opportunities (...) VALUES ($1,...) ON CONFLICT (id) DO NOTHING`.
- Conecta con `DATABASE_URL`. Si env ausente, skip DB con log warn.
- **ValidaciÃ³n**: compile + (opcional) integration test con DB.

## Paso 7 â€” `scanner.rs` + `publisher.rs` + `main.rs` + `dedup.rs`

- `dedup.rs`: LRU<H256, ()> con capacidad configurable + Redis SETNX con TTL.
- `scanner.rs`:
  - Entry `run(cfg, killswitch, redis, pg).await` reemplaza completo lo actual.
  - Lee `RPC_WS_<chain_id>` por cada chain habilitada en config.
  - Si ausente: gauge state=no_rpc, idle loop 5 s.
  - Si presente: conectar `chain_client`, subscribe pending, consumir stream, filtrar por `tx.to âˆˆ routers`, decode, match pattern, dedup, publish + persist.
  - Cada 5 s: revisar kill-switch.
- `publisher.rs`:
  - `pub async fn publish(opp: &Opportunity, redis: &mut ConnectionManager) -> Result<()>` con `XADD arbx:opps:detected MAXLEN ~ 10000 *` + field `json`.
  - MÃ©tricas.
- `main.rs`:
  - Spawna una `scanner::run` por chain habilitada (tokio::spawn) + HTTP server.
  - Shutdown limpio con `tokio::signal::ctrl_c`.

## Paso 8 â€” Tests embebidos

- `backend/searcher-rs/tests/calldata_test.rs`: 8 fixtures (4 UniV2 + 4 UniV3) con calldata real de mainnet (hex strings) â†’ assert DecodedSwap fields.
- `backend/searcher-rs/src/patterns/dex_arb.rs::tests`: `build_candidate` produce Opportunity con campos correctos.

## Paso 9 â€” ValidaciÃ³n final

- `cargo check -p searcher-rs`
- `cargo test -p searcher-rs`
- Python sanity: JSON schemas siguen vÃ¡lidos, app.toml sigue parseando.
- Smoke test conceptual (sin RPC): servicio sube, `/health` OK, gauge `state=no_rpc`, cero rows en DB.

## Out-of-scope S2 (documentado, no implementado)

- Sushi, Curve, Balancer decoders â€” stubs `Unknown` con counter `undecoded_total{reason="unknown_router"}`.
- Triangular / liquidation / backrun / flashloan patterns â€” no se intenta; logging explÃ­cito.
- Priority gas / MEV-Share / SUAVE â€” S5+.
- Block-level events (AccessList, inclusion race) â€” S3+.

