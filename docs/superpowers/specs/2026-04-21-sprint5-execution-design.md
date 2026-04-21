# ArbitrageX v2 — Sprint 5 "Execution privada vía Flashbots" — Design Spec

**Fecha**: 2026-04-21
**Sprint**: 5 de 8
**Depende de**: S1+S2+S3+S4 cerrados. `arbx:opps:simulated` publicado por sim-ctl.
**Credenciales nuevas**:
- `FLASHBOTS_SIGNER_KEY` — clave privada EOA. En S5 dev: key sin fondos. En prod: key con fondos y custody hardening.
- `RPC_HTTP_1` — RPC HTTP mainnet para lectura de nonce + broadcast eth_sendRawTransaction como backup.
- `FLASHBOTS_RELAY_URL` — default `https://relay.flashbots.net`.

**Servicio impactado**: `backend/relays-client` pasa de 501 estático a bundle builder + multi-relay submitter con **paper-mode obligatorio por defecto**.

## 0. REGLA DE SEGURIDAD ABSOLUTA

S5 es el ÚNICO sprint que toca claves privadas y puede mover capital. Reglas no negociables:

1. **Paper mode ON por default**. `execution.paper_mode = true` en `configs/app.toml` hasta que el operador lo desactive manualmente con revisión.
2. **Sin `FLASHBOTS_SIGNER_KEY` → `/execute` responde 501**. Igual que S1. No fabrica tx_hash.
3. **Kill-switch consultado antes de CADA submit**. Con caché TTL 1s — no 30s.
4. **Max amount hard cap** — `execution.max_value_eth` en config. Si el bundle excede → reject + risk_event severity=critical.
5. **Clave nunca se loguea**. Al cargar, se convierte a `Wallet` y la memoria se marca para zeroize on-drop.
6. **Log explícito al boot**: `signer_address=0x…`, pero NUNCA `signer_key=...`.
7. **Paper-mode log exitoso**: `{"event":"paper_mode.skip_submit","would_submit_to":[...]}` — operador ve qué hubiera pasado.

## 1. Arquitectura

```
arbx:opps:simulated (Redis Stream, por sim-ctl S4)
              │
              ▼ consumer group relays-client-g0
        Consumer
              │
              ▼
        SubmitEngine.execute(opp)
              │
              ├─ kill_switch → reject 503 + risk_event
              ├─ paper_mode? → log + return NotSubmitted
              ├─ signer==None? → reject not_implemented
              │
              ▼
        BundleBuilder.build(opp, signer, nonce_manager, chain_client)
              │
              ├─ resolve target_block = current + offset
              ├─ construct swap tx (same TxBuilder pattern as S4)
              ├─ sign with Wallet
              └─ BundleRequest
              
              │
              ▼
        MultiRelaySubmitter.submit(bundle)
              │  for each enabled relay in config:
              ▼
        {flashbots, bloxroute, eden, beaver, titan}.submit(bundle)
              │
              ▼
        Wait up to max_inclusion_wait_blocks
        (poll via HTTP RPC: eth_getTransactionByHash)
              │
              ├─ included → ExecutionResult{status="included", tx_hash, block_included, gas_used, actual_profit}
              ├─ reverted → ExecutionResult{status="reverted", …}
              ├─ dropped → ExecutionResult{status="dropped", block_included=null}
              └─ timeout → ExecutionResult{status="dropped", error="max_wait_exceeded"}
              
        persist(execution_result) + update opportunity.status + update relay_scores
        XACK
```

## 2. Decisiones estructurales

| # | Decisión | Justificación |
|---|---|---|
| 1 | **`ethers-rs` wallet** para signing (no `secp256k1` directo). | Ya en deps; maneja EIP-1559 + legacy + typed txs. |
| 2 | **Flashbots client via HTTP JSON-RPC** directo (no SDK — Rust SDK oficial no existe maduro). Firmar con `X-Flashbots-Signature` header. | Control total; Rust `reqwest` + `ethers::utils::hash_message`. |
| 3 | **Paper-mode hard-coded safe**: método `should_submit(cfg) → bool` revisa tanto `execution.paper_mode` como `ARBX_PAPER_MODE` env. Si cualquiera true → no submit. | Defensa en profundidad. Config puede ser modificada por error; env tiene precedencia. |
| 4 | **Nonce manager** en memoria per `(chain_id, address)`, refresca de RPC on-demand + tras cada submit. Semaphore serial por address para evitar race en el incremento. | Evita gap de nonces. Single-instance por address suficiente para S5. |
| 5 | **Multi-relay fan-out**: bundle idéntico enviado paralelo a todos relays `enabled=true` en config. Primer relay en incluir gana; otros detectarán `nonce_used` y la tx será no-op. | Redundancia; el tx on-chain solo puede aplicarse una vez. |
| 6 | **Relay scoring** actualizado: submitted_count, included_count por relay + avg_latency_ms. Escrito a `relay_scores` tras cada submit. | Datos para S6 learning loop. |
| 7 | **Retry strategy**: si no incluido en `target_block`, resubmit al `target_block+1` con mismo nonce pero **priority fee incrementado 10%**. Limite `retry_limit`. | Previene replay issues. |
| 8 | **Hard cap value**: si `tx.value > execution.max_value_eth * 1e18` → reject con risk_event severity=critical + stop pipeline. | Anti-grief + anti-leak catastrófico. |
| 9 | **Consumer pausa si**: kill_switch ON, CB open, signer missing, rpc unreachable. Cualquier otra condición = error por item, continúa el loop. | Evita tormenta de errores silenciosos. |
| 10 | **PersistDecision transaccional**: INSERT executions + UPDATE opportunities.status + UPSERT relay_scores bajo misma BEGIN/COMMIT. | Consistencia cross-tabla. |

## 3. Componentes

```
backend/relays-client/src/
  main.rs
  signer.rs             — load FLASHBOTS_SIGNER_KEY → Wallet, zeroize
  nonce_manager.rs      — per-(chain,addr) with Semaphore
  bundle_builder.rs     — ExecutionRequest → signed BundleRequest
  relay_flashbots.rs    — flashbots-specific client (signed RPC)
  relay_mev.rs          — generic mev-boost style relay client (stub for bloxroute/eden/beaver/titan)
  submit_engine.rs      — orchestrator
  tracker.rs            — poll eth_getTransactionReceipt for inclusion status
  consumer.rs           — XREADGROUP arbx:opps:simulated
  persistence.rs        — executions + opportunities + relay_scores
  http.rs               — /execute handler (hot-path, paper-mode aware)
```

## 4. Config (additive)

```toml
[execution]
private_only = true
paper_mode = true                       # DEFAULT SAFE — flip false ONLY after operator review
max_parallel_executions = 8
retry_limit = 2
target_block_offset = 1
max_inclusion_wait_blocks = 5
max_value_eth = 1.0                     # hard cap per-bundle
flashbots_submit_timeout_ms = 5000
priority_fee_increment_pct = 10

[[relays]]
name = "flashbots"
enabled = true
chains = [1]
endpoint = "https://relay.flashbots.net"

[[relays]]
name = "bloxroute"
enabled = false
chains = [1]
endpoint = ""

# ... eden, beaver, titan same shape
```

`relay` schema gets new optional `endpoint` field.

## 5. Env vars

- `FLASHBOTS_SIGNER_KEY` — hex private key. If absent: service degrades to 501 + consumer idle.
- `FLASHBOTS_RELAY_URL` — override `relays[name=flashbots].endpoint` (documented as priority).
- `RPC_HTTP_1` — for nonce fetching + inclusion tracking.
- `ARBX_PAPER_MODE` (optional) — if `true`, forces paper-mode regardless of config.

## 6. Contracts cambios

`ExecutionStatus` (already in shared-rs):
```rust
Submitted, Included, Reverted, Dropped, Replaced, NotImplemented
```
Add `NotSubmitted` for paper-mode outputs:
```rust
NotSubmitted,   // paper-mode, dry-run, or pre-submit reject (e.g., value cap)
```

Schema update in `execution_result.schema.json`: add `not_submitted` to enum.

## 7. Flashbots auth

Flashbots requires `X-Flashbots-Signature: <signer_address>:<signature_of_body_hash>` on every RPC call. The signer here is NOT the same as the tx signer necessarily — it's a **reputation-building identity**. For S5 we use the same key for simplicity; S6 can split.

## 8. Métricas nuevas

| Métrica | Tipo | Labels |
|---|---|---|
| `arbx_execution_total` | counter | `relay, status, chain_id` (ya declarada S1) |
| `arbx_execution_bundles_submitted_total` | counter | `relay, chain_id` |
| `arbx_execution_bundles_included_total` | counter | `relay, chain_id` |
| `arbx_execution_submit_duration_seconds` | histogram | `relay` |
| `arbx_execution_nonce_mismatches_total` | counter | `chain_id` |
| `arbx_execution_paper_mode_gauge` | gauge | `chain_id` (1 if paper mode) |
| `arbx_execution_value_exceeded_cap_total` | counter | `chain_id` |

## 9. Fallos esperados

| Condición | Comportamiento |
|---|---|
| `FLASHBOTS_SIGNER_KEY` absent | HTTP /execute 501; consumer logs idle 60s. |
| Paper mode ON | Construye y firma bundle; responde `status="not_submitted"` con `would_submit_to=[relays]`. |
| `max_value_eth` exceeded | Reject con severity=critical + risk_event; kill-switch candidato manual. |
| Relay 5xx/timeout | CB `relay_{name}` cuenta. Si trip: esa relay se skipea hasta cooldown. |
| Nonce mismatch on submit | Refresh nonce from RPC + retry una vez. Segundo fail → drop this opp. |
| Kill-switch ON durante submit | Abort (no submit). Mark `status="dropped"` reason=`kill_switch_during_submit`. |
| Inclusion timeout | Mark `status="dropped"`, record en `executions` con `block_included=null`. |

## 10. Criterios de aceptación S5

- [ ] `cargo test -p relays-client` pasa (≥ 6 tests: signer loads, nonce inc, bundle builder v2/v3, value cap, paper-mode).
- [ ] Sin `FLASHBOTS_SIGNER_KEY`: servicio sube, `/execute` 501 con payload, consumer idle.
- [ ] Con key + `paper_mode=true`: consumer procesa msg de `arbx:opps:simulated`, construye+firma bundle, responde `not_submitted` en DB, NO hay tx real.
- [ ] Con `paper_mode=false` + key de testnet fondeada: bundle se submite a relay de testnet, DB refleja `submitted`, tracker observa inclusión o drop. **Este test requiere testnet manual.**
- [ ] `value > max_value_eth` en cualquier opp → reject con risk_event critical, counter incrementa.

## 11. Fuera de scope S5

- **Multi-tx bundles** (backrun+arb en mismo bundle) → S6.
- **Priority gas estimation model** → S6.
- **MEV-Share / SUAVE** → S7+.
- **MEV-Boost direct builder connection** → S7+.
- **Signer hardware (Ledger/YubiHSM)** → S7 cuando llevemos secrets a Vault.
- **Gas arbitrage** (buying blockspace) → out of roadmap.

## 12. Honestidad y seguridad

- Nunca se loguea la clave privada.
- `tx_hash` nunca se fabrica — solo se setea tras respuesta de relay con hash válido.
- `paper_mode=true` siempre se respeta; no hay `if (override)` paths hidden.
- `max_value_eth` es una guardia — no es un límite de profit, es un límite de exposición.
- Relay responses que no coincidan con JSON-RPC spec → CB + log structured; no se inventan resultados.
