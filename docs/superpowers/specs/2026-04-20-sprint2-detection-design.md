# ArbitrageX v2 — Sprint 2 "Detection Real" — Design Spec

**Fecha**: 2026-04-20
**Sprint**: 2 de 8
**Depende de**: S1 cerrado estructuralmente (shared-rs, DB, contratos, kill-switch, observabilidad)
**Nuevas credenciales**: RPC WebSocket real (Alchemy / Infura / self-hosted Geth)
**Servicio principal impactado**: `backend/searcher-rs`

## 0. Objetivo

Reemplazar el scanner idle de S1 por detección real de oportunidades MEV a partir del **mempool público** (y eventualmente feeds privados en S3). El servicio debe:

1. Conectarse a un RPC WebSocket real y suscribirse a `newPendingTransactions`.
2. Decodificar el calldata contra ABIs conocidos de routers DEX (UniswapV2, UniswapV3, Sushi, Curve — los dos primeros en alcance obligatorio de S2).
3. Clasificar cada transacción candidata (`dex_arb` proto-opportunity, resto de `strategy_kind` = S2.1+).
4. Persistir la oportunidad en `opportunities` (status `detected`) **y** publicarla en el stream `arbx:opps:detected`.
5. Respetar el kill-switch.
6. Si no hay RPC configurado, **no fabricar** nada: logear explícitamente el estado y mantener el health OK pero con una gauge `arbx_searcher_backend_state{state="no_rpc"}=1`.

## 1. Decisiones estructurales

| # | Decisión | Razón |
|---|---|---|
| 1 | **`ethers-rs 2.x`** como cliente de chain. Migración a `alloy` en un sprint futuro. | Estable, documentado, soporta WS pendingTransactions nativo. |
| 2 | Detección se limita a **UniswapV2 + UniswapV3** routers en S2. Sushi/Curve/Balancer en S2.1 (no bloqueante). | Cobertura 80/20; los mismos selectores con ABI idéntico cubren forks V2. |
| 3 | Clasificación S2: **sólo `dex_arb` como candidato** (single-swap observado; el análisis de arb path se hace en S3 via selector). "Triangular", "liquidation", "backrun", "flashloan_arb" quedan como detecciones futuras con logging explícito. | No inflar S2; la ventaja real viene de S3 (selector) filtrando. |
| 4 | **Idempotencia por `tx_hash`**: el mismo pending tx no genera dos filas. UNIQUE sobre `(tx_hash)` a nivel de fila lógica (no DB column — `opportunities` no tiene tx_hash; usamos dedup en memoria con TTL + Redis SETNX). | Mempool repite hashes entre suscripción y replay. |
| 5 | **Backoff + reconexión** con jitter exponencial en el WS. No fail-fast — mantener el servicio vivo. | Los WS de providers caen con frecuencia. |
| 6 | **Rate guard**: si el backend recibe > N tx/s que no se logran procesar, drop con contador `arbx_searcher_dropped_total`. | Preservar memoria; arbx no necesita ver el 100%. |
| 7 | **Persistencia Opportunity en DB como status='detected'** + publicación a Redis Stream `arbx:opps:detected`. DB es fuente de verdad; Stream es bus temporal (24 h retention). | DB + Stream redundantes: consumer puede recuperarse de cualquier punto. |
| 8 | Sin parseo de estado on-chain (liquidez de pools) en S2. Sólo decode calldata. Análisis de liquidez vive en S3. | Alcance S2 acotado. |
| 9 | **Feature flag `ARBX_SEARCHER_ENABLED`** independiente del kill-switch: permite operar selector/sim sin detección real. | Debugging y sprints posteriores. |
| 10 | Todo en Rust async/tokio con `ethers::providers::Provider<Ws>`. | Coherente con canon. |

## 2. Flujo técnico

```
[RPC WS] ── newPendingTransactions ──▶ [subscribe loop]
                                          │
                                          ▼
                                  [tx filter by `to == known router`]
                                          │
                                          ▼
                                  [calldata decode by selector]
                                          │
                                          ▼
                                  [pattern matcher → Opportunity]
                                          │
                                          ├─▶ [Postgres: INSERT opportunities status='detected']
                                          └─▶ [Redis: XADD arbx:opps:detected]
```

## 3. Inventario de módulos nuevos/modificados

```
backend/shared-rs/src/
  chains.rs                ← NEW: router catalog, chain metadata
backend/searcher-rs/src/
  main.rs                  ← modified: boot + spawn new tasks
  scanner.rs               ← rewritten: mempool loop (replaces idle)
  publisher.rs             ← rewritten: real XADD + DB insert
  chain_client.rs          ← NEW: ethers-rs Provider<Ws> wrapper with reconnect
  calldata/mod.rs          ← NEW: selector registry + decode dispatcher
  calldata/univ2.rs        ← NEW: UniV2 decoders
  calldata/univ3.rs        ← NEW: UniV3 decoders
  patterns/mod.rs          ← NEW: pattern traits + dispatcher
  patterns/dex_arb.rs      ← NEW: DEX swap → candidate
  persistence.rs           ← NEW: sqlx INSERT opportunities
  dedup.rs                 ← NEW: in-memory LRU + Redis SETNX
backend/searcher-rs/tests/ ← NEW: calldata fixtures + pattern tests
```

## 4. Contratos / Tipos nuevos

### `RouterKind`

```rust
pub enum RouterKind {
    UniswapV2,
    UniswapV3,
    Unknown,
}
```

### `DecodedSwap`

```rust
pub struct DecodedSwap {
    pub router: RouterKind,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub min_amount_out: U256,
    pub path_len: u32,
    pub deadline: U256,
    pub recipient: Address,
    pub raw_selector: [u8; 4],
}
```

## 5. Config (additive to `configs/app.toml`)

```toml
[searcher]
enabled = true
rpc_ws_primary = ""     # resolved from env RPC_WS_<chain_id>
max_pending_buffer = 10000
decode_timeout_ms = 50
pattern_timeout_ms = 20
dedup_ttl_seconds = 60

[[searcher.routers]]
chain_id = 1
name = "uniswap-v2"
kind = "UniswapV2"
address = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"

[[searcher.routers]]
chain_id = 1
name = "uniswap-v3"
kind = "UniswapV3"
address = "0xE592427A0AEce92De3Edee1F18E0157C05861564"
```

Schema update in `configs/schemas/app.schema.json` (addition-only; S1 config remains valid).

## 6. Env vars nuevas (required when `searcher.enabled=true`)

- `RPC_WS_1` (per-chain WebSocket URL). Required to actually detect; absent → service stays healthy but no detection.
- `ARBX_SEARCHER_DEDUP_REDIS_DB` (optional; default 1).

## 7. Métricas nuevas

| Name | Type | Labels | Meaning |
|---|---|---|---|
| `arbx_searcher_backend_state` | gauge | `state` ∈ `{running, reconnecting, no_rpc, killswitch_on}` | One-hot via label value. |
| `arbx_searcher_pending_total` | counter | `chain_id` | pending txs received from subscription |
| `arbx_searcher_decoded_total` | counter | `chain_id`, `router` | txs successfully decoded |
| `arbx_searcher_undecoded_total` | counter | `chain_id`, `reason` | reason ∈ `{unknown_router, unsupported_selector, abi_decode_error, timeout}` |
| `arbx_searcher_candidates_total` | counter | `chain_id`, `strategy_kind` | opportunities persisted |
| `arbx_searcher_dropped_total` | counter | `chain_id`, `reason` | reason ∈ `{buffer_full, dedup_hit, killswitch}` |
| `arbx_searcher_ws_reconnects_total` | counter | `chain_id` | reconnect attempts |

## 8. Fallos esperados y comportamiento

| Condición | Comportamiento |
|---|---|
| `RPC_WS_1` ausente | Log warn, set gauge `state=no_rpc`, no detectar, **no hacer 501** (el servicio es worker, no HTTP). `/health` = 200. |
| WS cae | Backoff exponencial 1–30 s con jitter; reconnects_total++. Scanner no bloquea el proceso. |
| Kill-switch ON | Gauge `state=killswitch_on`, pausar subscription (evita gasto de cuota al provider). |
| ABI decode error | `undecoded_total++` con reason, continuar. |
| DB transient down | Retry 3 veces con backoff; si persiste, drop con `reason=persistence_down`; **no bloquear** ingestion. |

## 9. Pruebas S2

- **Unit**: decoders UniV2/UniV3 contra al menos 4 calldata fixtures cada uno (incluyendo un caso inválido que debe fallar graciosamente).
- **Unit**: pattern matcher con DecodedSwap sintético → Opportunity con campos correctos.
- **Unit**: dedup LRU (mismo tx_hash dos veces → segunda rechaza).
- **Integration** (opcional, requiere DB): insertar opportunity + leer stream; fuera de ejecución automatizada en S2 (validación manual).

## 10. Criterios de aceptación S2

- [ ] `cargo test -p searcher-rs` pasa
- [ ] Con `RPC_WS_1` no configurado: servicio sube, health 200, logs `state=no_rpc`, gauge correcta, **cero filas insertadas**.
- [ ] Con `RPC_WS_1` real (Alchemy free tier): ≥ 1 oportunidad persistida por minuto (dependiente del chain activity), logs estructurados con trace_id, métrica `arbx_searcher_candidates_total > 0`.
- [ ] Kill-switch toggle desde `api-server` **pausa** detección en < 2 s (TTL 1 s + iteración del loop).
- [ ] `smoke-test.sh` sigue pasando 100%.

## 11. Fuera de scope S2

- Análisis de ruta arb (DEX ↔ DEX con cálculo de profit) — **S3**.
- Triangular, liquidaciones, backrun, flashloan_arb — **S2.1 / S3**.
- Token safety check antes de persistir — **S3**.
- Simulación pre-persist — **S4**.
- Construcción de bundle — **S5**.
