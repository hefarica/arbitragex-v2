---
name: sop_sandwich_defensive
description: Cuando se discutan sandwich attacks, frontrunning, o protección anti-MEV. Activa con triggers "sandwich attack", "frontrun protection", "anti-MEV", "Flashbots Protect RPC", "MEV Blocker", "victim protection", "sandwich defensive". CRÍTICO — esta skill define la postura ÉTICA del proyecto: ArbitrageX NO implementa sandwich offensive. Solo defensivo.
type: arbx_security
source_section: SOP_ArbitrageX_2026.pdf §8
ethical_constraint: defensive_only
---

# Sandwich Attacks — POSTURA ESTRICTAMENTE DEFENSIVA

## ⚠️ DECLARACIÓN ÉTICA INMUTABLE (§8.1)

**ArbitrageX NO implementa sandwich attacks ofensivamente.** Esto está hardcoded en:
- Strategy catalog DB: row `sandwich_defensive` con `ethical_constraint='defensive_only'`.
- UI: card de Sandwich tiene badge rojo "DEFENSIVE ONLY", switch `enabled` solo controla **activación de protecciones**, NUNCA ejecución ofensiva.
- Skills (este archivo): mantiene conocimiento del mecanismo solo para **proteger nuestras propias operaciones**.

Ningún PR que añada lógica ofensiva de sandwich pasa code review. Punto.

## Qué es un sandwich attack
1. **Front-run**: bot adversario detecta swap pendiente del usuario en mempool.
2. **Buy first**: bot compra el mismo token antes del usuario → infla el precio.
3. **Victim swap**: usuario ejecuta su swap a precio inflado → recibe menos tokens.
4. **Back-run**: bot vende inmediatamente después → captura el spread inflado.

Resultado: bot gana 0.5-5%, usuario pierde proporcionalmente.

## Por qué es predatorio
El usuario es **damnificado directo y no consintió**. Diferencia clave vs arbitraje legítimo:
- **Arbitraje legítimo** (DEX-DEX, JIT): aprovecha ineficiencias estructurales del mercado, beneficia eficiencia agregada.
- **Sandwich attack**: extracción de valor del usuario individual sin compensación.

## Protección DEFENSIVA para nuestras operaciones (§8.2)

### Capa 1: Flashbots Protect RPC
TODAS las txs de ArbitrageX van via `https://rpc.flashbots.net`. **Excluye del mempool público** → invisible para sandwich bots.

### Capa 2: Slippage mínimo
Configurar slippage al **mínimo posible (0.1% o menos)**. Cualquier intento de front-running hace que la tx revierta automáticamente porque excede el slippage tolerado.

### Capa 3: Atomic execution
Bundles atómicos: si sandwich bot intenta insertarse entre nuestras operaciones, **TODO el bundle falla** y ninguno se ejecuta.

### Capa 4: Private mempool alternatives
Además de Flashbots:
- **MEV Blocker** (CoW Protocol).
- **Titan Builder**.
- **Eden Network**.

Todas las alternativas son private (no mempool público).

## Implementación
```rust
// CORRECTO — todas las txs vía Flashbots
let provider = ProviderBuilder::new()
    .on_http("https://rpc.flashbots.net".parse()?);

// INCORRECTO (rechazado en code review)
let provider = ProviderBuilder::new()
    .on_http("https://eth-mainnet.alchemy.com/...".parse()?);
// ↑ Esto va al mempool público → vulnerable a sandwich
```

## Reglas para code review
- ❌ RECHAZAR cualquier PR que añada lógica de "front-run + victim_swap + back-run" en bundle.
- ❌ RECHAZAR PRs que envíen txs a mempool público para arbs propios.
- ❌ RECHAZAR slippage > 1% (debe ser ≤ 0.5% per swap, idealmente 0.1%).
- ✅ APROBAR PRs que añaden private mempool routes alternativas.
- ✅ APROBAR PRs que añaden capas defensivas adicionales.

## Detección de potencial sandwich attack contra nosotros
```rust
async fn detect_sandwich_attempt(our_tx: &Transaction, block: &Block) -> bool {
    // Si en el mismo bloque, antes y después de nuestra tx, hay txs
    // del MISMO sender (hot wallet de un bot conocido) interactuando
    // con el MISMO pool, somos víctimas.
    let txs_in_block = block.transactions();
    let our_idx = txs_in_block.iter().position(|t| t.hash == our_tx.hash)?;
    let before = &txs_in_block[..our_idx];
    let after = &txs_in_block[our_idx+1..];
    // ... lógica de detección por mismo sender + mismo pool
}
```

## Invariantes
- SIEMPRE Flashbots/MEV Blocker/Titan/Eden — NUNCA mempool público.
- SIEMPRE bundle atómico para arbs propios.
- SIEMPRE slippage ≤ 0.5% (idealmente 0.1%).
- NUNCA implementar lógica ofensiva de sandwich (rechazo automático en CR).

## Cross-references
- Detalles Flashbots bundles: `sop_flashbots_bundles`.
- Risk management general: `sop_risk_management`.
- Otras técnicas de detección de scams: `sop_scam_detection`.
