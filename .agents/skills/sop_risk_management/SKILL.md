---
name: sop_risk_management
description: Cuando se diseñe gestión de riesgos del bot, position sizing, stop-loss automático, o circuit breakers. Activa con triggers "risk management", "tamaño de posición", "stop-loss", "circuit breaker", "max 2% capital", "umbral profit gas 3x", "slippage 0.5%", "pérdida diaria 0.5%", "private mempool obligatorio". Trae las 5 capas de protección del SOP §15.
type: arbx_security
source_section: SOP_ArbitrageX_2026.pdf §15
---

# Gestión de Riesgos — 5 Capas Inmutables

## Principios fundamentales (§15.1)

> "La gestión de riesgos es el pilar que diferencia un sistema MEV rentable de uno que pierde dinero."

**Múltiples capas de protección operan de forma autónoma y NO pueden ser desactivadas, ni siquiera manualmente.**

## Capa 1: Tamaño de Posición (§15.2)

- **Nunca arriesgar > 2% del capital total en una sola operación.**
- Para flash loans: límite del 2% del **profit potencial** (no del préstamo, ya que el capital es prestado).
- Cálculo automático del tamaño óptimo basado en:
  - Profundidad del pool (no exceder 1% de TVL).
  - Volatilidad reciente (reducir size en alta volatilidad).
  - Ratio beneficio/riesgo histórico de la ruta (Kelly criterion adaptado).

```rust
fn position_size(capital_usd: f64, pool_tvl_usd: f64, route_history: &RouteStats) -> f64 {
    let max_by_capital = capital_usd * 0.02;  // 2% capital
    let max_by_pool = pool_tvl_usd * 0.01;    // 1% TVL pool
    let kelly = kelly_criterion(route_history.win_rate, route_history.avg_profit, route_history.avg_loss);
    let adjusted_kelly = kelly * 0.5;  // half-Kelly por seguridad
    let max_by_kelly = capital_usd * adjusted_kelly;

    max_by_capital.min(max_by_pool).min(max_by_kelly)
}
```

## Capa 2: Protección Costos de Gas (§15.3)

- **Umbral mínimo de beneficio**: profit_neto ≥ **3× costo_gas** estimado.
- Multiplier ajusta dinámicamente según congestión:
  - Gas < 30 gwei → 3× (default).
  - Gas 30-100 gwei → 4×.
  - Gas > 100 gwei → 5× (o pausar operaciones).
- **Gas price oracle**: rechazar ops cuando gas_price > umbral_max configurado.
- **Gas estimation precisa** con `provider.estimate_gas()` antes de cada op.

```rust
const GAS_PROFIT_MULTIPLIER: u128 = 3;
const MAX_GAS_PRICE_GWEI: u128 = 100;

fn should_execute(profit_wei: U256, gas_units: u64, gas_price: u128) -> bool {
    if gas_price > MAX_GAS_PRICE_GWEI * 1e9 as u128 {
        return false;
    }
    let gas_cost = U256::from(gas_units) * U256::from(gas_price);
    profit_wei > gas_cost * GAS_PROFIT_MULTIPLIER
}
```

## Capa 3: Protección de Slippage (§15.4)

- **Slippage máximo 0.5% por swap** individual.
- Implementado a nivel de smart contract con `amountOutMin` calculado dinámicamente.
- Si precio se mueve > 0.5% entre simulación y ejecución → tx revierte automáticamente.

```rust
fn calculate_amount_out_min(expected_out: U256, slippage_bps: u64) -> U256 {
    expected_out * U256::from(10000 - slippage_bps) / U256::from(10000)
    // 0.5% slippage = 50 bps → min = expected × 0.995
}
```

## Capa 4: Stop-Loss Automático (§15.5)

### 4a. Abort por simulación negativa
Si `revm` simula pérdida → cancelar inmediatamente. Sin excepciones.

### 4b. Límite diario de pérdida
- **Default: 0.5% del capital** acumulado en 1 hora.
- Si supera → sistema entra en **modo de protección**: solo ejecuta micro-beneficios (riesgo virtualmente cero).
- Reset automático cada UTC midnight.

```rust
async fn check_daily_loss_limit(losses_1h: f64, capital: f64) -> Mode {
    let loss_pct = losses_1h / capital;
    if loss_pct > 0.005 {  // 0.5%
        Mode::Protected  // solo micro-arb
    } else {
        Mode::Normal
    }
}
```

### 4c. Protección contra reentrancia
Todos los contratos propios usan:
- Patrón **check-effects-interactions**.
- Modificador **`nonReentrant`** de OpenZeppelin.

## Capa 5: Private Mempool USAGE (§15.6)

**TODAS** las txs de ArbitrageX van por mempool privado:
- Flashbots Protect (primary).
- MEV Blocker (secondary).
- Titan Builder (tertiary).
- Eden Network (quaternary).

**NUNCA mempool público** para arbs propios. Esto elimina:
- Front-running.
- Sandwich attacks contra nosotros.
- Copia de operaciones.

## Tabla resumen de parámetros

| Parámetro | Default | Rango seguro | Activación |
|-----------|---------|---------------|------------|
| Position size max | 2% capital | 1-5% | Hard reject |
| Position size pool | 1% TVL | 0.5-2% | Hard reject |
| Profit / gas | 3× | 2-5× | Hard reject |
| Max gas price | 100 gwei | 50-200 gwei | Hard reject |
| Slippage per swap | 0.5% | 0.1-1% | Smart contract revert |
| Daily loss limit | 0.5% capital | 0.3-1% | Mode switch to Protected |
| Operation timeout | 3 blocks | 1-5 blocks | Cancel tx |
| Min concurrent per block | 1 | 1-3 | Queue delay |
| Max concurrent per block | 5 | 3-10 | Queue delay |

## Invariantes inmutables
- 5 capas SIEMPRE activas. No hay flag para desactivar.
- Override solo via PR + code review humano (no programáticamente).
- Mode::Protected al exceder daily_loss → bot solo ejecuta micro-arbs.
- Slippage 0.5% es techo. Default config 0.3% (más conservador).
- Mempool público nunca, jamás.

## Cross-references
- Capital y mode config: Tab 1 del Strategy Panel.
- Slippage por estrategia: ver cada `sop_<strategy>` (típicamente menor en stables, mayor en volátiles).
- Private mempool detalles: `sop_flashbots_bundles`.
- Anti-sandwich (capa 1+5 combinadas): `sop_sandwich_defensive`.
