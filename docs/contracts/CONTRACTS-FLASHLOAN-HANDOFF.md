# Contracts — Handoff: superficie on-chain de flash loans (v1.0.0)

Rol: Contracts (ejecución on-chain). Objetivo de escuadra: "SIGUE".
Base/candidato inspeccionado: rama `fix/perhop-reserves-01`, HEAD `858b943fd80c8b5e1606d220aa87d63b86f4ba15`.
Modo: INSPECCIÓN READ-ONLY. No se modificó ningún archivo. No se ejecutó forge (binario ausente).

## Alcance de esta contribución

El handoff de Quant (GAS-PRICE-ADAPTER-01 / breaker A.6 / GAP-B) es **off-chain**
(TypeScript `risk-circuit-breakers.ts` + ledger `gas_measurement_state`). Ese es el eje
Data/B3, **no** la superficie de Contracts. No lo toqué y no le corresponde a este rol:
el gas-cost USD se computa off-chain; no hay contrato on-chain implicado.

Esta contribución cubre la superficie on-chain que SÍ es de Contracts: el executor de
flash loans y sus adapters, evaluada contra la skill `arbx-flash-loan-discipline` (7 reglas).

## Superficie verificada (artefactos leídos)

- `contracts/src/FlashLoanExecutor.sol` (413 líneas) — wrapper UUPS multi-provider.
- `contracts/src/ArbitrageExecutor.sol` (638 líneas) — core de ruta + gate de profit.
- `contracts/src/flashloans/` — AaveV3FlashAdapter.sol, BalancerFlashAdapter.sol
  (implementados); UniV3FlashAdapter.sol, DyDxFlashAdapter.sol (SKELETON `NotImplemented()`).
- `contracts/src/dexes/` — GMXAdapter.sol, SynthetixAdapter.sol (SKELETON `NotImplemented()`).
- `contracts/test/` — 17 archivos `.t.sol` (FlashLoanExecutor / Invariant / RoundTrip / Adapters, etc.).

## Checklist `arbx-flash-loan-discipline` (7 reglas) aplicada

| # | Regla | Veredicto | Evidencia |
|---|-------|-----------|-----------|
| 1 | Repay-or-revert atómico | VERIFICADO | `FL_RepaymentShortfall` (FLE:50-54, 374, 400); revert del core propaga vía `FL_ArbitrageExecutionFailed` (FLE:364, 391) |
| 2 | `msg.sender == pool` | VERIFICADO | Aave `FL_UnauthorizedCaller` (FLE:315); Balancer (FLE:350). `aavePool` es storage (patrón UUPS, no `immutable`), seteado en `initialize()` (FLE:172-181) |
| 3 | Validar `initiator` | VERIFICADO (Aave) / GAP (Balancer) | Aave `initiator == address(this)` (FLE:316). Balancer sin *session flag* transitorio (ver C2) |
| 4 | Atomicidad sin estado parcial | VERIFICADO | Sin `transfer` a EOA dentro del callback; `forceApprove → call → reset → repay` (FLE:359-377, 385-405) |
| 5 | Profit-floor on-chain antes del repay | GAP (ver C1) | El floor vive en `ArbitrageExecutor._runRoute` (`profit < minProfit → InsufficientProfit`, AE:441-445), **no** en el callback |
| 6 | Fees on-chain, no hardcodear | VERIFICADO | Aave usa `premium` argumento (FLE:310, 399); Balancer `feeAmounts[0]=0` (FLE:357) |
| 7 | No re-entrar al protocolo | VERIFICADO | `nonReentrant` en ambos callbacks (FLE:86-91, 312, 340) |

## Hallazgos (superficie Contracts)

### C1 — MEDIA (doctrinal): el callback no preserva saldo previo ante premium > profit
`FlashLoanExecutor` mide su balance SÓLO como `balanceOf >= amount + premium`
(`FL_RepaymentShortfall`, FLE:373-374 y 399-400). No snapshottea el balance previo al
préstamo ni exige `profit >= premium` on-chain. Si el off-chain codifica
`executeArbitrageFlashFunded` con `minProfit < premium` (mala config), el executor repaga
consumiendo su propio capital preexistente y la operación es perdedora en apariencia de éxito.

Esto es la regla 5 de `arbx-flash-loan-discipline` (finalBal >= amount+fee+minProfitFloor) y
mi mandato #9 ("proteger fondos preexistentes frente a repayment"). Es un **gap conocido y
documentado**: `ArbitrageExecutor` declara "covering it [premium] is the CALLER's
responsibility, so minProfit SHOULD be set >= premium" (AE:290-297). La mitigación real
descansa en que premium Aave = 5 bps y el floor operativo net≥3×gas ≫ premium.

**Fix path (requiere sesión dedicada + config del operador):** snapshot pre-loan balance en el
callback y exigir `postBalance >= preBalance + amount + premium` (o validar `profit >= premium`);
nueva variable storage APPEND-ONLY (tras `_reentrancyStatus`) + error `FL_*` + tests fork
(profit<premium revert, profit≥premium success). No-hardcode: el floor viene de config.

### C2 — BAJA (defense-in-depth): `receiveFlashLoan` sin *session flag* transitorio
Regla 3 fallback: Balancer no expone `initiator`, y la skill exige flag transitorio seteado
antes de `flashLoan(` y validado en el callback. Aquí se sustituye por auth de 3 capas
(`balancerVault != 0` → `msg.sender == balancerVault` → `flashLoanProvider != 0`,
FLE:342-353), el trust anchor documentado del audit A4 (FLE:220-223). Residual: cualquier
actor puede forzar al Vault canónico a invocar el callback con sus tokens + `userData`
arbitrario; el blast radius queda acotado por los gates propios de `ArbitrageExecutor`
(`onlyExecutor`, `whenNotPaused`, `nonReentrant`, allowlist de routers/selectors).

### C3 — INFO (capacidad declarada no implementada): adapters SKELETON
`UniV3FlashAdapter`, `DyDxFlashAdapter`, `GMXAdapter`, `SynthetixAdapter` revierten
`NotImplemented()` en todas sus funciones; sus tests usan `vm.skip(true)`
(`FlashLoanAdapters.t.sol:320-338, 616-633`). El `@title` de `FlashLoanExecutor` declara
"soporta Balancer (0%), dYdX y UniV3 via adapter pattern" (FLE:69) — **sobredeclarado**: los
adapters dYdX/UniV3 revierten en todo call. Mandato #10: la solución a capacidad faltante es
completar el soporte, no presentarlo como soportado. El header de UniV3 lista el trabajo
requerido (pool por call, `uniswapV3FlashCallback` en FLE, fee ceil `(amount0·feeTier/1e6)+1`).

### Observación menor (fidelidad de tests): fee Aave hardcodeado en tests
`FlashLoanInvariant.t.sol` usa `premium = amount * 9 / 10000` (0.09%) (líneas 78, 108, 469,
485, 645, 657, 742). El contrato lee `premium` on-chain (correcto); los tests simulan la fee
histórica de 9 bps, hoy 5 bps (`FLASHLOAN_PREMIUM_TOTAL()`). No es defecto del contrato; es
stale de fixtures.

## Estado y fronteras

- **Tests: NO_VERIFICADO.** `forge` no está instalado en este equipo (Windows; `forge.exe`
  → `ResourceUnavailable`). Ningún `.t.sol` fue ejecutado aquí. El checklist arriba es
  inspección estática de bytecode fuente, no resultado de `forge test`.
- **Sin cambios de archivo** en esta contribución (solo lectura + este handoff).
- **Sin firma/broadcast/deploy.** Los hallazgos C1-C3 son propuestas de reparación para
  sesión dedicada + revisión de Security; no se aplicaron.
- **GAP-B no es de este rol** (Data/B3). Handoff de Quant conservado como dependencia.

## Siguiente acción recomendada (para Planner)
1. Asignar a Contracts sesión dedicada para C1 (fix de preservación de saldo previo), con
   valor de `minProfitFloor` del operador y revisión de Security.
2. C3: decidir si los adapters SKELETON se implementan o se retiran del `@title`/docstring
   para no declarar capacidad inexistente (evidencia: revert `NotImplemented()`).
3. Corregir fixture de tests a fee Aave actual (5 bps) o parametrizarla.
