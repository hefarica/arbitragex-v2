---
name: sop_csa_architecture
description: Cuando se diseñe, refactorice o evalúe la arquitectura del backend de ArbitrageX. Activa con triggers "arquitectura C-S-E", "Compose-Simulate-Execute", "patrón Paradigm", "estructura del workspace Cargo", "dependencias Alloy", "loop del searcher", "responsabilidades por crate". Sintetiza el cap 2 del SOP_ArbitrageX_2026.pdf con dependencias exactas y código de referencia del searcher.
type: arbx_architecture
source_section: SOP_ArbitrageX_2026.pdf §2
---

# Arquitectura del Sistema — Patrón C-S-E

## Patrón Compose-Simulate-Execute (Paradigm)

### Compose (Componer)
Construir grafo de rutas: nodos=tokens, aristas=pools (con tasa de cambio + costo de gas como peso). Detectar oportunidades vía Bellman-Ford modificado buscando ciclos de peso negativo.

### Simulate (Simular)
Cada ruta candidata simulada localmente con **revm 19.0** usando estado on-chain via `alloy-provider`. Calcular: fees LP, slippage, impacto precio, costo gas. Solo rutas con profit neto positivo avanzan.

### Execute (Ejecutar)
Empaquetar en bundle atómico → Flashbots Protect o MEV-Boost relay. Atomicidad: o todas las txs ejecutan, o ninguna.

## Estructura del Workspace Cargo

```toml
[workspace]
resolver = "2"
members = [
    "crates/searcher-rs",
    "crates/sim-ctl",
    "crates/relays-client",
    "crates/shared-rs",
]

[workspace.dependencies]
alloy = { version = "0.9", features = ["full"] }
alloy-primitives = "0.8"
alloy-sol-types = "0.8"
alloy-provider = { version = "0.9", features = ["ws"] }
alloy-rpc-types = "0.9"
alloy-transport-ws = "0.9"
alloy-network = "0.9"
revm = "19.0"
revm-primitives = "9.0"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
eyre = "0.6"
tracing = "0.1"
dashmap = "6"
```

## Loop principal del searcher (referencia)

```rust
use alloy::providers::{Provider, ProviderBuilder, WsConnect};

const MIN_PROFIT: u128 = 50_000_000_000u128; // 50 GWEI minimum

async fn run_searcher(wss_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ws = WsConnect::new(wss_url);
    let provider = ProviderBuilder::new().on_ws(ws).await?;
    let mut sub = provider.subscribe_pending_transactions().await?;
    while let Some(tx_hash) = sub.next().await {
        let tx = provider.get_transaction_by_hash(tx_hash).await?;
        if let Some(decoded) = decode_swap_tx(&tx) {
            let profit = simulate_arbitrage(&decoded).await?;
            if profit > MIN_PROFIT {
                submit_bundle(vec![tx.into()], profit).await?;
            }
        }
    }
    Ok(())
}
```

## Responsabilidades por Crate (Tabla §2.4)

| Crate | Responsabilidad | Dependencias clave |
|-------|------------------|---------------------|
| `searcher-rs` | Loop principal, mempool sub, decode swaps, orquestación C-S-E, envío bundles | alloy-provider, alloy-transport-ws, tokio, dashmap |
| `sim-ctl` | Simulación determinista revm: estado on-chain, ejec txs, cálculo profit neto | revm, alloy-primitives, alloy-sol-types |
| `relays-client` | Comunicación Flashbots/MEV-Boost: build bundles, envío, monitor inclusión | alloy-provider, reqwest, serde |
| `shared-rs` | Tipos compartidos, config, utilidades decode, métricas | alloy-primitives, alloy-sol-types, serde, tracing |

## Estado actual del repo vs SOP
- ✅ `backend/searcher-rs/` existe (corresponde a `searcher-rs` del SOP)
- ✅ `backend/sim-ctl/` existe pero stub (debe reescribirse en Sprint 4)
- ✅ `backend/relays-client/` existe
- ✅ `backend/shared-rs/` existe
- ⚠ `backend/prioritization-spine/` existe en repo pero NO en SOP — es capa adicional propia. Sprint 4 la complementa con `simulator-v2` (nuevo crate paralelo, no modifica spine).

## Invariantes
- NUNCA `ethers-rs` en código nuevo. Solo Alloy 0.9+.
- NUNCA simular sin estado on-chain real (revm + lazy_db real, no stub).
- NUNCA broadcast sin pasar por bundle atómico vía Flashbots/MEV-Boost.

## Cross-references
- Triangular arb impl: `sop_dex_triangular`
- CEX-DEX impl: `sop_cex_dex`
- Bundle construction: `sop_flashbots_bundles`
- Bellman-Ford code: `sop_atomic_route_construction`
