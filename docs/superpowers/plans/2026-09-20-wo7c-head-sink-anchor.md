# WO-7c — Cartridge Head-Sink Anchor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminar las filas `block_number = NULL` del cartridge-layer (100% de emisiones cartridge, forense post-deploy #600) manteniendo los atomics de head del `HostContext` actualizados en modo `ARBX_MEMPOOL_MODE=auto|mempool`.

**Architecture:** Un loop minimal `head_sink_loop` en `block_scanner.rs` (fuera de la lista de congelación §37) que suscribe `newHeads` y hace `store` SOLO en los atomics `GasBlockSink` (head + base_fee milli-gwei). Sin orchestrator, sin impact index, sin `eth_getLogs` — no consume intents (route_discovery/route_scanner_worker intocados). Spawn en el path mempool/auto de `scanner.rs` cuando `cartridge_runner` exista. En modo `Block` no se duplica (ese path ya cablea `block_detection_loop`).

**Tech Stack:** Rust (tokio, ethers Provider<Ws>, tracing), searcher-rs crate.

## Root cause (evidencia cerrada)

1. Cartridge path construye su propio `Opportunity` en `cartridge_boot.rs` (~1206) — nunca pasa por el loop de anclaje del orchestrator (`orchestrator.rs:873` `.or(intent.observed_block())`, que explica el 0.06% NULL del path engines).
2. El fallback de #599 (`intent.observed_block().or_else(|| host head)`) lee `host_block_number_handle()` cuyo atomic SOLO lo escribe `block_detection_loop` vía `GasBlockSink`.
3. `GasBlockSink` SOLO se cablea si `MempoolMode::Block` (`scanner.rs:1076-1081`); el VPS corre `auto` → loop jamás spawn-ea (0 logs `block_scanner.connected`) → head=0 → `(head > 0).then_some(head)` = `None`.
4. Resultado: 3864/3864 filas cartridge NULL (100%) vs engines 34/54791 (0.06%).

## Global Constraints

- §37 freeze Nivel 1: `route-discovery` / `route_scanner_worker` INTOCADOS. El cambio vive en `block_scanner.rs` + el spawn en `scanner.rs` (path mempool/auto).
- RULE 00 / R8: fail-honest. Si no hay WS endpoints → warn UNA vez + idle (head queda 0, anchor sigue None — nunca fabricar head).
- Mode-invariant (§34.1): la matemática no cambia; esto solo corrige la observabilidad del anchor de contexto de cadena.
- Coste aceptado: +1 suscripción WS newHeads por chain en modo auto/mempool (documentado, revisado por -61).
- CI gate: `cargo clippy -p searcher-rs --locked --all-targets -- -D warnings` + `cargo test -p searcher-rs` (1317 tests).
- Rama limpia `fix/wo7c-cartridge-head-anchor` desde `origin/main` (d458f557). §36: verificar `git branch --show-current` antes de commitear. Aviso SendMessage a -61 ANTES del branch-switch.
- Commits: solo archivos propios (block_scanner.rs, scanner.rs, este plan). `Co-Authored-By: Claude Code <noreply@anthropic.com>`. PR con `🤖 Generated with [Claude Code](https://github.com/claude-code)`. Merge authority: -61.

---

### Task 1: Helper `publish_head` + tests

**Files:**
- Modify: `backend/searcher-rs/src/block_scanner.rs` (nuevo helper + módulo de tests)

**Interfaces:**
- Produces: `fn publish_head(sink: &GasBlockSink, block_number: u64, base_fee_wei: Option<ethers::types::U256>)` — pub(crate) implícito (misma crate, módulo privado OK).

- [ ] **Step 1: Write the failing tests** (añadir al bloque `#[cfg(test)]` existente de block_scanner.rs; si no existe al final del archivo, crearlo)

```rust
#[cfg(test)]
mod head_sink_tests {
    use super::*;
    use ethers::types::U256;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    fn sink() -> GasBlockSink {
        GasBlockSink {
            block_number: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            base_fee_milligwei: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    #[test]
    fn publish_head_stores_block_number() {
        let s = sink();
        publish_head(&s, 12_345, None);
        assert_eq!(s.block_number.load(Ordering::Acquire), 12_345);
    }

    #[test]
    fn publish_head_stores_base_fee_milligwei() {
        // 1.5 gwei = 1_500_000_000 wei → 1500 milli-gwei (get_base_fee decodifica ÷1000).
        let s = sink();
        publish_head(&s, 1, Some(U256::from(1_500_000_000u64)));
        assert_eq!(s.base_fee_milligwei.load(Ordering::Acquire), 1500);
    }

    #[test]
    fn publish_head_none_base_fee_keeps_previous() {
        // Cadena pre-EIP-1559 (sin base fee): no debe resetear el valor previo.
        let s = sink();
        publish_head(&s, 1, Some(U256::from(2_000_000_000u64)));
        publish_head(&s, 2, None);
        assert_eq!(s.base_fee_milligwei.load(Ordering::Acquire), 2000);
        assert_eq!(s.block_number.load(Ordering::Acquire), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p searcher-rs --lib head_sink_tests`
Expected: FAIL — `cannot find function publish_head`.

- [ ] **Step 3: Implement helper** (justo debajo de `pub struct GasBlockSink`)

```rust
/// WO-7c — publish chain context (head + base fee) into the cartridge
/// `HostContext` atomics. Shared by the block-mode subscription and the
/// mempool/auto `head_sink_loop` so both writers keep identical semantics.
pub(crate) fn publish_head(sink: &GasBlockSink, block_number: u64, base_fee_wei: Option<U256>) {
    sink.block_number
        .store(block_number, std::sync::atomic::Ordering::Release);
    if let Some(base_fee_wei) = base_fee_wei {
        // get_base_fee() decodes the atomic as gwei×1000 (milli-gwei); wei / 1e6
        // = milli-gwei. Saturating: pre-EIP-1559 chains have no base fee → skipped.
        let milligwei = (base_fee_wei / U256::from(1_000_000u64)).as_u64();
        sink.base_fee_milligwei
            .store(milligwei, std::sync::atomic::Ordering::Release);
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p searcher-rs --lib head_sink_tests`
Expected: PASS 3/3.

- [ ] **Step 5: Refactor `run_block_subscription` para reutilizar el helper** (reemplaza el bloque inline 218-228)

```rust
                if let Some(sink) = gas_sink {
                    publish_head(sink, block_num.as_u64(), block.base_fee_per_gas);
                }
```

(Comportamiento idéntico: mismos stores, mismas orderings, mismo skip de base fee `None`. Verificar con el test existente de block_scanner si aplica + compilar.)

- [ ] **Step 6: Commit**

```bash
git add backend/searcher-rs/src/block_scanner.rs
git commit -m "refactor(searcher): extract publish_head helper (WO-7c prep)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

### Task 2: `head_sink_loop` + suscripción anchor-only

**Files:**
- Modify: `backend/searcher-rs/src/block_scanner.rs`

**Interfaces:**
- Produces: `pub async fn head_sink_loop(chain_id: u64, ws_urls: Vec<String>, sink: GasBlockSink, cancel: CancellationToken)` — consumido por scanner.rs (Task 3).

- [ ] **Step 1: Write the failing test** (añadir a `head_sink_tests`)

```rust
    #[tokio::test]
    async fn head_sink_loop_empty_urls_idles_honestly() {
        // R8: sin WS endpoints NO fabricamos head — warn + idle hasta cancel.
        let s = sink();
        let cancel = tokio_util::sync::CancellationToken::new();
        let c = cancel.clone();
        let task = tokio::spawn(head_sink_loop(1, vec![], s, cancel));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        c.cancel();
        let _ = task.await; // debe terminar limpio, sin panic
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p searcher-rs --lib head_sink_tests`
Expected: FAIL — `cannot find function head_sink_loop`.

- [ ] **Step 3: Implementar `head_sink_loop` + `run_head_subscription`** (después de `run_block_subscription`, mismo patrón de reconnect/backoff que `block_detection_loop`)

```rust
/// WO-7c — minimal head-only sink loop for mempool/auto mode: subscribe to
/// `newHeads` and publish chain context (block height + base fee) into the
/// cartridge `HostContext` atomics. In `MempoolMode::Block` the full
/// `block_detection_loop` already does this; in `auto`/`mempool` nothing wrote
/// the atomics, so the #599 anchor fallback (`host_block_number_handle`)
/// always read 0 and every cartridge-layer opportunity was persisted with
/// `block_number: NULL` (100% of cartridge rows, forensics 2026-09-20).
/// This loop deliberately does NOT consume intents — no orchestrator, no
/// impact index, no getLogs — it only keeps the head atomics honest (R8).
pub async fn head_sink_loop(
    chain_id: u64,
    ws_urls: Vec<String>,
    sink: GasBlockSink,
    cancel: CancellationToken,
) {
    if ws_urls.is_empty() {
        warn!(
            event = "head_sink.no_ws",
            chain_id,
            "no WS endpoints; cartridge head anchor stays 0 (R8 honest)"
        );
        cancel.cancelled().await;
        return;
    }
    info!(
        event = "head_sink.start",
        chain_id,
        endpoints = ws_urls.len(),
        "anchor-only head sink starting (cartridge block_number context)"
    );
    let mut backoff_ms: u64 = 1_000;
    let max_backoff_ms: u64 = 30_000;
    let mut url_idx = 0usize;
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let url = &ws_urls[url_idx % ws_urls.len()];
        match run_head_subscription(chain_id, url, &sink, &cancel).await {
            Ok(()) => return, // clean exit (cancelled)
            Err(e) => {
                warn!(
                    event = "head_sink.reconnect",
                    chain_id,
                    error = %e,
                    backoff_ms,
                    next_endpoint = (url_idx + 1) % ws_urls.len(),
                    "head subscription dropped; rotating endpoint + backing off"
                );
                url_idx = url_idx.wrapping_add(1);
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(max_backoff_ms);
            }
        }
    }
}

/// One WS connection for the anchor-only sink: subscribe to blocks, publish
/// head + base fee on each. Returns `Ok(())` on cancellation, `Err` on
/// disconnect (caller reconnects). Mirrors `run_block_subscription` minus the
/// orchestrator/getLogs machinery.
async fn run_head_subscription(
    chain_id: u64,
    url: &str,
    sink: &GasBlockSink,
    cancel: &CancellationToken,
) -> anyhow::Result<()> {
    let client = WsChainClient::connect(chain_id, url).await?;
    let mut blocks = client.subscribe_blocks().await?;
    info!(
        event = "head_sink.connected",
        chain_id, "subscribed to newHeads (anchor-only)"
    );
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            blk = blocks.next() => {
                let Some(block) = blk else {
                    return Err(anyhow::anyhow!("newHeads stream ended"));
                };
                let Some(block_num) = block.number else { continue };
                publish_head(sink, block_num.as_u64(), block.base_fee_per_gas);
            }
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p searcher-rs --lib head_sink_tests`
Expected: PASS 4/4 (incluye idle-honest).

- [ ] **Step 5: Commit**

```bash
git add backend/searcher-rs/src/block_scanner.rs
git commit -m "feat(searcher): anchor-only head_sink_loop (WO-7c)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

### Task 3: Wiring en scanner.rs (path mempool/auto)

**Files:**
- Modify: `backend/searcher-rs/src/scanner.rs:1092-1095` (inmediatamente DESPUÉS del early-return del modo Block, antes del comentario "Phase 16")

**Interfaces:**
- Consumes: `crate::block_scanner::head_sink_loop`, `crate::block_scanner::GasBlockSink`, `runner.host_block_number_handle()`, `runner.host_base_fee_handle()`.

- [ ] **Step 1: Añadir el spawn** (después de la línea del `return Ok(ScannerHandle { chain_id });` del branch Block — línea ~1091 — para que NUNCA corra duplicado en modo Block)

```rust
    // WO-7c — keep the cartridge head/base-fee atomics honest in mempool/auto
    // mode (block mode already gets them from block_detection_loop above).
    // Without this the #599 anchor fallback reads head=0 and cartridge-layer
    // opportunities persist block_number=NULL (100% of cartridge rows).
    if let Some(runner) = cartridge_runner.as_ref() {
        let ws_urls: Vec<String> = pool.endpoints.iter().map(|e| e.url.clone()).collect();
        let gas_sink = crate::block_scanner::GasBlockSink {
            block_number: runner.host_block_number_handle(),
            base_fee_milligwei: runner.host_base_fee_handle(),
        };
        let head_cancel = cancel.clone();
        tokio::spawn(crate::block_scanner::head_sink_loop(
            chain_id,
            ws_urls,
            gas_sink,
            head_cancel,
        ));
    }
```

Notas de ownership: `cartridge_runner` sigue disponible (los spawns previos usan `.clone()`); `cancel` se mueve al `detection_loop` más abajo (línea ~1139) → usar `cancel.clone()`; `pool.endpoints` se consume por `detection_loop(chain_id, pool.endpoints, …)` → el `iter().map().collect()` de arriba NO lo mueve.

- [ ] **Step 2: Compilar + clippy**

Run: `cargo clippy -p searcher-rs --locked --all-targets -- -D warnings`
Expected: 0 warnings.

Run: `cargo test -p searcher-rs --locked`
Expected: 1317+ tests PASS (todos los pre-existentes + 4 nuevos).

- [ ] **Step 3: fmt**

Run: `cargo fmt -p searcher-rs`
Expected: sin cambios (o aplicarlos).

- [ ] **Step 4: Commit**

```bash
git add backend/searcher-rs/src/scanner.rs
git commit -m "feat(searcher): spawn head_sink_loop in mempool/auto path (WO-7c)

Cartridge-layer block_number NULL 100% -> anchored via real observed head.

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

### Task 4: Plan + PR

- [ ] **Step 1: Commit del plan**

```bash
git add docs/superpowers/plans/2026-09-20-wo7c-head-sink-anchor.md
git commit -m "docs: WO-7c head-sink anchor plan

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

- [ ] **Step 2: Push + PR**

```bash
git push origin fix/wo7c-cartridge-head-anchor
```

PR title: `fix(searcher): WO-7c — cartridge head-sink anchor (block_number NULL → 0)`
PR body: root cause + evidencia + diff + gate (clippy/tests). Footer `🤖 Generated with [Claude Code](https://github.com/claude-code)`. CI: 34 checks required.

- [ ] **Step 3: BOARD update + aviso a -61** (SendMessage): estado WO-7c en `audits/perf-stack-2026-09-20/GOAL-WORKORDERS.md` + link del PR. Merge authority -61. Post-deploy gate: PG `SELECT count(*) FROM opportunities WHERE block_number IS NULL AND detected_at > <deploy_ts>` para filas cartridge → 0.

## Rollback

Revert del merge commit (§37 Parte 5). El cambio es additive: en modo Block no hay doble escritura (early return), y el loop solo escribe atomics advisory (Ordering::Release sobre valores de contexto, no gates económicos).

## Verificación post-deploy (gate BOARD)

1. `docker logs arbitragex-v2-searcher-rs-1 | grep head_sink.connected` → presente.
2. PG: filas cartridge con `block_number IS NULL` detectadas DESPUÉS del deploy → 0.
3. Tarjeta §30 QUARANTINED por missing block en /opportunities → desaparece para eventos nuevos.
