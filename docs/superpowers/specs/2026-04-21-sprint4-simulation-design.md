# ArbitrageX v2 — Sprint 4 "Simulation real" — Design Spec

**Fecha**: 2026-04-21
**Sprint**: 4 de 8
**Depende de**: S1+S2+S3 cerrados. `arbx:opps:validated` publicado por selector-api.
**Nueva credencial**: `ANVIL_FORK_URL` (un RPC con acceso archive, ej. Alchemy free tier sirve).
**Servicio impactado**: `backend/sim-ctl` pasa de `501` a simulación real.

## 0. Propósito real y acotado

Sprint 4 responde **dos preguntas** sobre cada oportunidad validada:

1. **¿La transacción revierte o pasa en el estado actual del fork?** → `passed: boolean`, `fail_reason: string | null`.
2. **¿Qué gas consume y cuál es el slippage observado?** → `gas_estimate_wei`, `slippage_pct`.

S4 **no** calcula profit real ni construye bundle arbitraje — eso es S5 (construcción de tx con priority gas + MEV bundle). S4 **sí** nos dice si la tx observada es físicamente ejecutable en el fork del bloque actual.

**Escenarios cubiertos en S4**:
- `dex_arb` con single swap observado (UniV2/V3 router).
- Resto de `strategy_kind` (`triangular`, `backrun`, `liquidation`, `flashloan_arb`) quedan marcadas como `simulator: 'not_implemented'` con fail_reason explícito. **No se inventa pass/fail.**

## 1. Arquitectura

```
arbx:opps:validated (Redis Stream)
         │
         ▼ consumer group sim-ctl-g0
    sim-ctl consumer
         │
         ▼
    ForkManager.acquire() → snapshot_id (evm_snapshot)
         │
         ▼
    TxBuilder.build(opportunity) → { from, to, data, value, gas_cap }
         │
         ▼
    anvil.eth_call(tx, "latest")  (sin enviar; solo simulación)
    anvil.estimateGas(tx)
         │
         ├─ OK  → measure slippage (decode output)
         └─ revert → capture fail_reason
         │
         ▼
    ForkManager.release(snapshot_id) → evm_revert
         │
         ▼
    Build SimulationResult
         │
         ├─ INSERT simulations (cada intento, pass o fail)
         ├─ UPDATE opportunities SET status='simulated'|'rejected'
         └─ if pass → XADD arbx:opps:simulated
         
    XACK (at-least-once)
```

## 2. Decisiones estructurales

| # | Decisión | Justificación |
|---|---|---|
| 1 | **Anvil persistente** en docker compose profile `sim`. No ephemeral per-request. | Start cost de anvil ~2s; persistente elimina ese costo. `evm_snapshot`/`evm_revert` garantiza aislamiento. |
| 2 | **Fork de `latest` al arrancar**; `anvil_reset` cada N minutos (configurable) para refrescar estado. | Balancea determinismo vs frescura. Entre resets, usamos snapshot/revert. |
| 3 | **Pool de M snapshots en paralelo** (default 4). Locks coarse-grained; cada sim-ctl worker toma un snapshot antes y lo libera después. | Permite simulación concurrente sin state bleed. |
| 4 | **Sólo `dex_arb` simulable en S4**. Resto devuelve `simulator='not_implemented'` con `fail_reason='strategy_not_simulatable_in_s4'`. | Alcance honesto. Probe tx para swaps conocidos es tratable; otros requieren S5 bundle. |
| 5 | **Probe tx en lugar de trade completo**: construimos una swap de `amount_in_wei` usando el router de `dex_a`, con `to = signer_address_dev` (EOA dummy). | No necesitamos construir la arb entera; solo medir si el swap individual pasa y cuánto gas/slippage tiene. |
| 6 | **eth_call, NO eth_sendRawTransaction**. | eth_call no consume nonce ni modifica estado. Más rápido y no requiere signing. |
| 7 | **Timeout duro 3 s por simulación**. Si anvil no responde, CB `sim_engine` cuenta. | Latencia dominante. |
| 8 | **SimulationResult siempre persiste** (incluso fails); solo los `passed=true` publican al stream. | Trazabilidad total en DB. |
| 9 | **Consumer group sim-ctl-g0**, XACK post-persist (at-least-once). | Resiliente a crash. |
| 10 | **Sin ANVIL_FORK_URL configurado**: `/simulate` HTTP responde 501 igual que S1; consumer del stream se mantiene idle con log "no_fork". NUNCA fabrica resultados. | Honestidad. |

## 3. Componentes nuevos / modificados

```
backend/sim-ctl/src/
  main.rs                ← modificado: spawn consumer + fork_manager init
  fork_manager.rs        ← NEW: ethers Provider<Http> + snapshot/revert pool
  tx_builder.rs          ← NEW: Opportunity → probe tx (UniV2/V3)
  sim_engine.rs          ← NEW: orquestador (fork → build → call → result)
  consumer.rs            ← NEW: XREADGROUP arbx:opps:validated
  persistence.rs         ← NEW: INSERT simulations + UPDATE opportunities
  http.rs                ← NEW: /simulate handler (delega a sim_engine)

backend/shared-rs/src/
  contracts.rs           ← adición: SimulationResult already exists, no change
  chains.rs              ← adición: router ABI selectors helpers (para TxBuilder)

docker/docker-compose.prod-like.yml:
  anvil service (profile: sim)

configs/app.toml:
  [simulation] section

configs/schemas/app.schema.json:
  simulation schema
```

## 4. Config (additive)

```toml
[simulation]
provider = "anvil"           # "anvil" | "tenderly" | "not_implemented"
fork_block = "latest"        # "latest" or <number>
snapshot_pool_size = 4
sim_timeout_ms = 3000
reset_interval_s = 600       # anvil_reset cada 10 min
gas_limit_safety_factor = 1.3
max_slippage_for_pass_pct = 5.0
probe_amount_fraction = 1.0  # 1.0 = use amount_in_wei completo; <1 = porción
```

JSON Schema sigue el mismo patrón.

## 5. Config env vars

- `ANVIL_URL` — URL del anvil persistente (default `http://anvil:8545` dentro de compose).
- `ANVIL_FORK_URL` — RPC upstream de donde anvil hace fork.
- `SIM_SIGNER_ADDRESS` — dirección EOA para probe tx (no signer real, solo `from` para eth_call).

## 6. Anvil en docker compose

```yaml
anvil:
  image: ghcr.io/foundry-rs/foundry:latest
  profiles: ["sim"]
  command: >
    anvil
    --host 0.0.0.0
    --fork-url ${ANVIL_FORK_URL}
    --fork-block-number latest
    --block-time 1
    --accounts 10
    --balance 10000
  ports: ["8545:8545"]
  networks: [arbx-net]
  healthcheck:
    test: ["CMD","sh","-c","curl -s -X POST -H 'Content-Type: application/json' --data '{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"id\":1}' http://localhost:8545 | grep -q result"]
    interval: 5s
    timeout: 3s
    retries: 10
```

`sim-ctl` añade `depends_on: anvil: { condition: service_healthy }` cuando el profile `sim` está activo. Sin profile, `anvil` no arranca y `sim-ctl` sigue respondiendo 501.

## 7. TxBuilder (S4 alcance)

```rust
pub fn build_probe(opp: &Opportunity, chain_id: u64, signer_addr: Address)
    -> Result<ProbeTx, BuildError>;

pub enum BuildError {
    UnsupportedStrategy,   // triangular, liquidation, etc.
    UnknownRouter,
    InvalidAmount,
}

pub struct ProbeTx {
    pub from: Address,
    pub to: Address,        // router dex_a
    pub value: U256,        // 0 para tokens ERC20; amount para ETH-in
    pub data: Bytes,
    pub gas_cap: u64,
}
```

**Para `dex_arb` con UniV2 router**:
- selector `swapExactTokensForTokens(uint256 amountIn, uint256 amountOutMin=1, address[] path=[token_in,token_out], address to=signer, uint256 deadline=now+60)`

**Para `dex_arb` con UniV3 router**:
- selector `exactInputSingle(ExactInputSingleParams{tokenIn, tokenOut, fee=3000, recipient=signer, deadline=now+60, amountIn, amountOutMinimum=0, sqrtPriceLimitX96=0})`

**Para resto de strategy_kind**: retorna `BuildError::UnsupportedStrategy` → sim result `not_implemented`.

## 8. Métricas nuevas

| Métrica | Tipo | Labels |
|---|---|---|
| `arbx_sim_runs_total` | counter | `simulator`, `result` (pass/fail/not_implemented/error) |
| `arbx_sim_duration_seconds` | histogram | `simulator` |
| `arbx_sim_fork_resets_total` | counter | — |
| `arbx_sim_fork_errors_total` | counter | `reason` |
| `arbx_sim_snapshot_pool_active` | gauge | — |

## 9. Decision rules (post-sim)

| Resultado anvil | SimulationResult.passed | opportunities.status next |
|---|---|---|
| eth_call OK + slippage ≤ max | `true` | `simulated` (published a stream) |
| eth_call OK + slippage > max | `false`, fail_reason=`slippage_too_high` | `rejected` |
| eth_call revert | `false`, fail_reason=decoded revert reason | `rejected` |
| anvil timeout / error | `false`, fail_reason=`sim_timeout` / `fork_error` | estado NO cambia (retry vía no-XACK) |
| TxBuilder UnsupportedStrategy | `false`, simulator=`not_implemented` | `rejected` con reason=`strategy_not_simulatable_in_s4` |

## 10. Fallos / comportamiento

| Condición | Comportamiento |
|---|---|
| `ANVIL_FORK_URL` absent | HTTP `/simulate` responde 501 (mismo que S1). Consumer loop idle 60s con log `no_fork`. |
| `anvil` container down (profile activo) | healthcheck falla → sim-ctl no arranca (depends_on). Si anvil cae en runtime, CB `sim_engine` trips, consumer pausa. |
| `ANVIL_FORK_URL` RPC rate-limited | CB `sim_engine` cuenta 429s. Al trip, anvil se reinicia (podría). Documentado: necesita Alchemy o self-host. |
| Kill-switch ON | consumer pausa igual que selector. |
| revert con razón desconocida | `fail_reason = "revert_unknown_selector"` + hex dump (64 bytes). No silencia. |
| Multiple chains habilitados | por S4 solo soportamos chain_id=1. Otros chains → `simulator: not_implemented` con reason `chain_not_supported_in_s4`. |

## 11. Honesty rules

- `sim-ctl /simulate` devuelve **501 con NotImplementedPayload** si: no hay `ANVIL_URL` accesible, o si strategy_kind no es `dex_arb`, o si chain no es 1. Payload incluye `requires` y `sprint`.
- Nunca marca `passed=true` sin haber recibido respuesta exitosa de anvil.
- Nunca inventa `gas_estimate_wei` — si anvil no reportó, el campo queda `null`.
- `slippage_pct` se calcula del output real de `eth_call`, no del `amount_out_minimum` del calldata original.

## 12. Criterios de aceptación S4

- [ ] `cargo test -p sim-ctl` pasa (≥ 5 tests: fork_manager mock, tx_builder v2/v3, unsupported strategy, dispatcher).
- [ ] Sin `ANVIL_FORK_URL`: servicio sube, `/health` OK, `/simulate` responde 501 con payload estructurado.
- [ ] Con anvil real + fork Alchemy free tier: simulación de WETH→USDC UniV3 eth_call retorna gas estimate > 50k y < 300k.
- [ ] Consumer procesa un fixture XADD a `arbx:opps:validated` en < 5 s p95; DB tiene fila en `simulations`.
- [ ] Revert case: construye un swap deliberadamente malo (amount enorme), anvil lo rechaza, `fail_reason` contiene la razón (p.ej. "INSUFFICIENT_LIQUIDITY").

## 13. Fuera de scope S4 (explícito)

- **Construcción del bundle arbitraje completo** (path multi-hop, counter-trade) → S5.
- **Flashbots priority gas estimation** → S5.
- **Liquidation simulation** (necesita health-factor query + triggerable call) → S4.1.
- **Cross-chain atomic sim** → out-of-roadmap.
- **Back-test histórico** → S6.
