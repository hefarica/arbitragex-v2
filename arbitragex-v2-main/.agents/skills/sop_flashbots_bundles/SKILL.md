---
name: sop_flashbots_bundles
description: Cuando se construya, envíe o monitoree bundles MEV vía Flashbots. Activa con triggers "Flashbots bundle", "eth_sendBundle", "MEV-Boost relay", "private RPC", "rpc.flashbots.net", "bundle hash inclusion", "construir bundle atómico", "BundleTransaction can_revert". Trae el código Alloy del SOP §9 con FlashbotsBundle struct + flow de envío + monitoreo.
type: arbx_architecture
source_section: SOP_ArbitrageX_2026.pdf §9
---

# Flashbots y Bundle Construction

## Por qué Flashbots (§9.1)
- Mempool privado: txs nunca aparecen en mempool público → cero front-running.
- Bundles atómicos: secuencia de txs ejecuta atómicamente. Si una falla, ninguna ejecuta.
- Endpoint: `https://rpc.flashbots.net` (RPC) y `https://relay.flashbots.net` (relay para bundles).

Para arbitraje, atomicidad es crítica: combina flash loans + swaps + liquidaciones en una sola operación sin riesgo de ejecución parcial.

## Construcción de bundles con Alloy (§9.2)

```rust
use alloy::providers::{ProviderBuilder, Http};
use alloy::primitives::{Address, Bytes, B256, U256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct FlashbotsBundle {
    txs: Vec<BundleTransaction>,
    block_number: u64,
    min_timestamp: u64,
    max_timestamp: u64,
    reverting_tx_hashes: Vec<B256>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BundleTransaction {
    tx: Bytes,
    can_revert: bool,
}

async fn submit_flashbots_bundle(
    bundle_txs: Vec<BundleTransaction>,
    target_block: u64,
) -> Result<B256, Box<dyn std::error::Error>> {
    let relay_url = "https://relay.flashbots.net";
    let provider = ProviderBuilder::new().on_http(relay_url.parse()?);
    let bundle = FlashbotsBundle {
        txs: bundle_txs,
        block_number: target_block,
        min_timestamp: 0,
        max_timestamp: 0,
        reverting_tx_hashes: vec![],
    };
    let bundle_hash = provider.raw_request::<_, B256>(
        "eth_sendBundle".into(),
        [bundle]
    ).await?;
    tracing::info!("Bundle enviado: hash={:?} block={}", bundle_hash, target_block);
    Ok(bundle_hash)
}

// Bundle de flash-loan arbitraje atómico
async fn build_arb_bundle(
    flash_loan_amount: U256,
    route: Vec<Address>,
    provider: &impl Provider,
) -> Vec<BundleTransaction> {
    let loan_tx = build_flash_loan_tx(flash_loan_amount).await;
    let swap_tx = build_swap_tx(&route, flash_loan_amount).await;
    let repay_tx = build_repay_tx().await;
    vec![
        BundleTransaction { tx: loan_tx, can_revert: false },
        BundleTransaction { tx: swap_tx, can_revert: true },  // swap puede revertir si precio cambió
        BundleTransaction { tx: repay_tx, can_revert: false },
    ]
}
```

## Monitoreo de inclusión (§9.3)
1. Verificar cada nuevo bloque para confirmar inclusión del bundle hash.
2. Rastrear status de la tx (incluso revertida cuenta para el bloque).
3. **Ajustar coinbase payment al builder** si bundles consistentemente no incluidos.
4. Mantener stats de tasa de inclusión por builder → optimizar enrutamiento.

## Builders alternativos (no solo Flashbots)
| Relay | Endpoint | Mecanismo | Costo | Cobertura |
|-------|----------|-----------|-------|-----------|
| Flashbots Protect | rpc.flashbots.net | Private relay → builders | Tips al builder | Ethereum |
| MEV Blocker | mev-blocker.cow.fi | Batch auctions | 0% fee | Ethereum |
| BloxRoute BDN | bloxroute.com | Private tx broadcast | Suscripción mensual | Multi-chain |
| Titan Builder | rpc.titanbuilder.xyz | Builder direct | Gratis | Ethereum |
| Eden Network | api.edennetwork.io | RPC + builder | Suscripción | Ethereum |

## Patrón coinbase payment (tip al builder)
Para bundles de alta competencia (liquidaciones, JIT), pagar tip directo al builder:
```solidity
// Al final del executor contract
block.coinbase.transfer(profit * 90 / 100); // 90% del profit al builder
// Resto va a operator
```
Esto incrementa probabilidad de inclusión sustancialmente.

## Reverting tx hashes
Algunos bundles permiten que ciertas txs reviertan (ej. JIT donde swap del usuario puede no llegar). Marcar:
```rust
reverting_tx_hashes: vec![user_tx_hash], // OK si esta tx revierte
```
Pero las críticas (loan, repay) deben tener `can_revert: false`.

## Invariantes
- TODAS las txs propias vía Flashbots (o alternativa privada). NUNCA mempool público.
- Bundle hash siempre logueado para auditoría.
- Monitor de inclusion rate; si <50%, escalar coinbase payment.
- Loan + repay siempre `can_revert: false` (sino flash loan se devuelve incompleto).
- Min 3 builders configurados (Flashbots + Titan + MEV Blocker es config recomendada).

## Cross-references
- Bundle de JIT: `sop_jit_liquidity` (mint + victim + burn).
- Bundle de liquidación: `sop_liquidations` (flashloan + liquidate + repay).
- Bundle de triangular: `sop_dex_triangular` (loan + 3 swaps + repay).
- Anti-sandwich: `sop_sandwich_defensive` (Flashbots Protect = capa 1 anti-sandwich).
