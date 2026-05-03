---
name: sop_scam_detection
description: Cuando se evalúe seguridad de un token nuevo, se detecte honeypots, rug pulls, o se diseñe el filtro pre-trade de seguridad. Activa con triggers "honeypot detection", "rug pull check", "fee on transfer", "blacklist token", "tax compra venta", "Token Sniffer", "is_token_safe", "verificación seguridad ERC20". Trae el código Alloy del SOP §11 con is_token_safe() implementación.
type: arbx_security
source_section: SOP_ArbitrageX_2026.pdf §11
---

# Detección de Estafas y Protección Pre-Trade

## Tipos de ataques (§11)

### Honeypots (§11.1)
Token que se puede comprar pero NO vender. Capital queda atrapado. Detección:
- Análisis bytecode → patrones sospechosos en `transfer()`.
- **Simulación de venta** con revm → si revierte = honeypot.
- Análisis blacklist/mint functions.
- Cálculo de impuesto transferencia (tax compra vs tax venta).

### Rug Pulls (§11.2)
Desarrolladores retiran liquidez o transfieren fondos. Indicadores:
- **Liquidez NO bloqueada** (sin timelock).
- **Allocation excesiva al equipo** (>20% del supply).
- **Contratos actualizables** (owner con privilegios upgrade).
- **Holders top-10 concentrando >50% del supply**.

## Implementación: `is_token_safe()` (§11.4)

```rust
use alloy::primitives::Address;
use alloy::providers::Provider;

const MAX_ACCEPTABLE_TAX: f64 = 0.05; // 5% max transfer tax

async fn is_token_safe(
    provider: &impl Provider,
    token: Address,
) -> bool {
    // Step 1: Check contract exists
    let code = provider.get_code_at(token).await.unwrap_or_default();
    if code.is_empty() {
        tracing::warn!("Token {:?} no tiene código", token);
        return false;
    }

    // Step 2: Check for honeypot - can we sell?
    let owner = get_token_owner(provider, token).await;
    if is_blacklisted(provider, token, owner).await {
        tracing::warn!("Token {:?} tiene blacklist activo", token);
        return false;
    }

    // Step 3: Check transfer tax (compra vs venta asimétrico)
    let tax = estimate_sell_tax(provider, token).await;
    if tax > MAX_ACCEPTABLE_TAX {
        tracing::warn!("Token {:?} tax de venta {:.2}% excede máximo", token, tax * 100.0);
        return false;
    }

    // Step 4: Check liquidity lock
    let liq_locked = is_liquidity_locked(provider, token).await;
    if !liq_locked {
        tracing::warn!("Token {:?} liquidez no bloqueada", token);
        return false;
    }

    // Step 5: Check for unrestricted mint
    let has_mint = has_unrestricted_mint(provider, token).await;
    if has_mint {
        tracing::warn!("Token {:?} tiene mint sin restricción", token);
        return false;
    }

    tracing::info!("Token {:?} pasó todas las verificaciones", token);
    liq_locked && tax < MAX_ACCEPTABLE_TAX && !has_mint
}
```

## Checklist completo

| Verificación | Si falla | Herramienta | Acción |
|--------------|----------|-------------|--------|
| No mint infinito | RECHAZAR | Token Sniffer API | Excluir del grafo |
| Sin blacklist/pausable | RECHAZAR | Etherscan verify | Excluir del grafo |
| Liquidez bloqueada >6M | ADVERTENCIA | De.Fi Scanner | Reducir max amount |
| Holder distribution OK (<50% top10) | ADVERTENCIA | Etherscan holders | Reducir exposure |
| Auditoría (OpenZeppelin/Trail) | INFO | DefiSafety.com | Priorizar auditados |
| No fee on transfer >1% | RECHAZAR | eth_call test | Excluir del grafo |
| Sin upgradability sin timelock | ADVERTENCIA | proxy detection | Reducir max amount |

## Herramientas externas integrables

| Servicio | Función | API |
|----------|---------|-----|
| Token Sniffer | Honeypot detection + risk score | https://tokensniffer.com/api/v2/ |
| De.Fi Scanner | Smart contract audit | https://de.fi/scanner/api |
| Honeypot.is | Simulación buy+sell | https://api.honeypot.is/v2/IsHoneypot |
| GoPlus Security | Risk score multi-chain | https://api.gopluslabs.io/api/v1/ |
| DefiSafety | Audit transparency score | https://defisafety.com/api/ |

## Estrategia recomendada
Combinar **3 herramientas mínimo** (defense in depth):
1. Token Sniffer (rápida, gratis con rate limit).
2. eth_call test buy + sell con revm (más lento, pero ground truth).
3. GoPlus Security (cross-chain).

Si **2 de 3 marcan riesgo** → rechazar.

## Detección de fee on transfer
```rust
async fn detect_fee_on_transfer(provider: &impl Provider, token: Address) -> Option<f64> {
    let amount = U256::from(1_000_000u64);
    let balance_before = balanceOf(token, recipient).call().await?;
    transfer(token, recipient, amount).simulate().await?;
    let balance_after = balanceOf(token, recipient).call().await?;
    let received = balance_after - balance_before;
    let fee = (amount.saturating_sub(received)).to::<u128>() as f64 / amount.to::<u128>() as f64;
    Some(fee)
}
```

## Invariantes
- TODO token nuevo pasa por `is_token_safe()` antes de añadirse al grafo de detección.
- Fee on transfer > 1% → rechazo automático.
- Tax compra ≠ tax venta (asimetría >2%) → rechazo (señal de honeypot).
- Liquidez no bloqueada → reducir max trade amount al 1% de pool TVL.
- Holders top-10 > 50% → reducir max trade al 0.5% de pool TVL.

## Cross-references
- Selección de tokens previa: `sop_token_pool_selection`.
- Honeypot detection en mev_index: `token-risk-and-asset-safety-filter`.
- Risk management general: `sop_risk_management`.
