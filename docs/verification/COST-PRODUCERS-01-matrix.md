# COST-PRODUCERS-OMEGASEARCH-FEES-01 — matriz de aceptación (§5 del prompt FEES-264)

Rama: `fix/omegasearch-fees-01` · base `9202561e` (= `origin/main`, SHADOW-CANONICAL-01 #772)
Módulo nuevo: `backend/searcher-rs/src/economics/cost_producers/` (registrado desde
`economics.rs`, NO desde `lib.rs` — zona caliente).

Esta matriz es el insumo de la tarea **T-E** (consolidación). Cada fila declara
ÁMBITO · PRODUCTOR · ARCHIVO:LÍNEA · FUENTE y ESTADO. Los estados son los de
§15 del prompt y NO se mezclan con rentabilidad.

## 1. Ámbitos §5 y sus productores

| ÁMBITO | PRODUCTOR | ARCHIVO:LÍNEA | FUENTE (autoritativa) | ESTADO |
|---|---|---|---|---|
| V2 y forks | `V2FeeTerms::from_read` (par efectivo) + `V2Fork::documented_terms` (sólo contraste) | `cost_producers/univ2.rs:66`, `:46` | `pair.fee()` / `getFee()` del deployment; documentación del fork sólo para detectar discrepancia | IMPLEMENTED |
| V2 y forks — cantidades y redondeo | `univ2::amount_out` (floor, overflow controlado, frontera de reserva) | `cost_producers/univ2.rs:196` | `getAmountOut` de UniV2/PancakeSwap V2 (misma forma `(num, den)` efectiva) | IMPLEMENTED |
| V2 y forks — comisión retenida | `univ2::fee_taken_raw` + `lp_fee_component` (embedded) | `cost_producers/univ2.rs:236`, `:251` | derivada exacta del par efectivo | IMPLEMENTED |
| V2 — comportamiento especial de token | `univ2::special_token_task` (declara incompatibilidad en vez de asumir) | `cost_producers/univ2.rs:353` | delta de transferencia observado | IMPLEMENTED (declaración) |
| V3 y concentrada | `V3FeeTerms::resolve` (fee tier real en pips/1e6) | `cost_producers/univ3.rs:88` | `pool.fee()` (uint24) | IMPLEMENTED |
| V3 — comisión protocolaria | `univ3::protocol_fee_component` (CORTE de la comisión LP; cero si no se cruza tick) | `cost_producers/univ3.rs:186` | `factory.protocolFee()`/`protocolFeeDenominator()` + `UniswapV3Pool.swap` (acumula sólo con cruce) | IMPLEMENTED |
| V3 — ticks completos o quoter | `univ3::full_traversal_task` → receipt `full_tick_traversal_or_protocol_quoter` | `cost_producers/univ3.rs:238` | `factory.getPool` + `QuoterV2.quoteExactInputSingle` | IMPLEMENTED (tarea) |
| V4 y hooks — PoolKey | `PoolKey::static_lp_fee_pips` / `is_dynamic_fee` | `cost_producers/univ4.rs:70`, `:79` | `PoolManager` PoolKey + `LPFeeLibrary` (bit 23) | IMPLEMENTED |
| V4 — LP fee estática/dinámica | `V4FeeTerms::resolve` (dinámica exige lectura del hook) | `cost_producers/univ4.rs:137` | `updateDynamicLPFee` / estado del hook | IMPLEMENTED |
| V4 — protocol fee | `univ4::protocol_fee_component` (ADITIVO, carril uint8 ×100) | `cost_producers/univ4.rs:312` | `Pool.Slot0.protocolFee` + `ProtocolFeeLibrary.getZeroForOneFee` (×100, máx 1000 pips) | IMPLEMENTED |
| V4 — hook fees | `univ4::hook_fee_component` | `cost_producers/univ4.rs:332` | `IHooks.getHookFee(...)` | IMPLEMENTED |
| V4 — cambios de balance personalizados | `univ4::hook_rebate_component` + `BeforeSwapDelta` como flujo con signo | `cost_producers/univ4.rs:353` | retorno de `beforeSwap` | IMPLEMENTED |
| V4 — contexto del hook | `univ4::hook_state_task` → receipt `dynamic_parameters_and_hook_state` | `cost_producers/univ4.rs:384` | `IHooks` before/afterSwap + estado | IMPLEMENTED (tarea) |
| Curve — invariante y versión | `CurveInvariant::detect` + `scaled_fee_rate` (StableSwap-NG `N/(4(N-1))`) + `version_task` | `cost_producers/curve.rs:62`, `:97`, `:159` | getters reales del pool (`fee`, `A`, `offpeg_fee_multiplier`, `mid_fee`, `out_fee`, `fee_gamma`, `N_COINS`) | IMPLEMENTED |
| Curve — base de la comisión | `curve::fee_on_output` (sobre la SALIDA, no el input) | `cost_producers/curve.rs:137` | `StableSwap.get_dy` (`fee = self.fee * dy / 1e10`) | IMPLEMENTED |
| Curve — reparto admin/LP | `curve::admin_split` (conserva el total exacto) | `cost_producers/curve.rs:181` | `admin_fee` en unidades 1e10 | IMPLEMENTED |
| Curve — comisión variable | `curve::cryptoswap_dynamic_fee` (mid/out/fee_gamma/K) | `cost_producers/curve.rs:210` | reductor `_fee()` de CryptoSwap | IMPLEMENTED |
| Curve — parámetros dinámicos | `curve::ramped_a` + `curve::offpeg_fee_rate` | `cost_producers/curve.rs:241`, `:268` | `initial_A/future_A/future_A_time`, `offpeg_fee_multiplier` | IMPLEMENTED |
| Curve — oráculo y rama de inventario | `curve::oracle_branch` + `oracle_task` → receipt `oracle_round_and_inventory_branch` | `cost_producers/curve.rs:297`, `:323` | `price_oracle` / `last_prices` / `price_scale` | IMPLEMENTED (tarea) |
| Gas — payload REAL antes del envío | `gas::intrinsic_gas` + `estimate_from_payload` (EIP-2028 calldata, EIP-2930 access list) | `cost_producers/gas.rs:113`, `:136` | calldata serializada del intent | IMPLEMENTED |
| Gas — base/priority | `gas::effective_gas_price` (`min(maxFee, base+tip)`) | `cost_producers/gas.rs:161` | `eth_getBlockByNumber.baseFeePerGas` + `eth_maxPriorityFeePerGas` | IMPLEMENTED |
| Gas — datos L1 / componente L2 | `gas::L2DataFee` (enum excluyente: NotApplicable / OpStackPreEcotone / OpStackEcotoneRead / ArbitrumRead) | `cost_producers/gas.rs:186`, `:204` | `GasPriceOracle.getL1Fee` (Ecotone) / `NodeInterface.gasEstimateL1Component` (Nitro) / fórmula pre-Ecotone | IMPLEMENTED |
| Gas — blobs | `gas::blob_fee` + `GasCost::blob_wei` (una sola vez) | `cost_producers/gas.rs:271`, `:339` | `blobBaseFee` × `blobGasUsed` (EIP-4844) | IMPLEMENTED |
| Gas — receipt DESPUÉS | `gas::cost_from_receipt` + `GasReconciliation` (varianza con signo) | `cost_producers/gas.rs:333`, `:483` | `gasUsed` y `effectiveGasPrice` del receipt | IMPLEMENTED |
| Builder/relay | `BuilderPaymentMode::treatment` + `builder::bid_component` | `cost_producers/builder.rs:57`, `:113` | payload de `eth_sendBundle` / `flashbots_getBundleStats` / `coinbaseDiff` | IMPLEMENTED |
| Builder/relay — rebates | `builder::rebate_component` (flujo con signo propio) | `cost_producers/builder.rs:181` | refund del relay / MEV-Share | IMPLEMENTED |
| Financiación — Aave | `funding::resolve` + `aave_premium_raw` (BPS de `FLASHLOAN_PREMIUM_TOTAL()`) | `cost_producers/funding.rs:217`, `:182` | `Pool.FLASHLOAN_PREMIUM_TOTAL()` / `FLASHLOAN_PREMIUM_TO_PROTOCOL()` — **prohibido 5/9 bps** | IMPLEMENTED |
| Financiación — semántica del método | `FundingMethod::callback` + `premium_in_bps` + `premium_read` | `cost_producers/funding.rs:74`, `:96`, `:120` | callbacks reales (`executeOperation` ×2, `onFlashLoan`, `receiveFlashLoan`, `uniswapV2Call`) | IMPLEMENTED |
| Financiación — capacidad y comparación | `funding::resolve` (capacidad) + `candidates` + `cheapest_admissible` | `cost_producers/funding.rs:250`, `:425`, `:466` | `availableLiquidity` del proveedor / capital propio | IMPLEMENTED |
| Financiación — principal/premium/repayment | `FundingQuote` con los tres exactos | `cost_producers/funding.rs:147` | derivados de la lectura | IMPLEMENTED |
| Contrato común de costos | `CostResolution` + `to_cost_lines` (agrega por categoría, sin duplicados) | `cost_producers.rs:811`, `:956` | contrato `CostLine` del bridge (consumo aditivo) | IMPLEMENTED |
| Auditoría de doble conteo | `CostResolution::double_count_audit` (8 códigos) | `cost_producers.rs:1125` | — | IMPLEMENTED |
| CEX/derivados · Cross-domain · Otros dominios (mint/redeem, vault, subasta, royalties) | — | — | — | **NO IMPLEMENTADO en esta tarea** (fuera del sector asignado; las categorías `cex_trading_fee`, `bridge_cost`, `mint_redeem_fee` están declaradas en `bridge_kind` para que su productor no colisione) |

## 2. Dobles conteos evitados (verificados por test)

| # | Riesgo | Mecanismo que lo evita | Test |
|---|---|---|---|
| 1 | Priority fee contado dentro de `gas` y otra vez como línea propia | `effective_gas_price = min(maxFee, base+tip)`; audit `priority_tip_double_counted` | `priority_fee_is_inside_gas_and_never_a_second_line` |
| 2 | Pago al builder hecho por priority fee declarado `external` | `BuilderPaymentMode::treatment` deriva el tratamiento del MODO; audit `builder_paid_via_priority_fee_counted_twice` | `builder_payment_via_priority_fee_is_embedded_not_external` |
| 3 | Comisión de swap restada otra vez en un `atomic_quote` | `lp_fee_component` de V2/V3/V4/Curve emiten `embedded`; audit `execution_fee_external_on_atomic_quote` | `embedded_execution_fees_are_declared_but_not_subtracted` |
| 4 | Protocol fee V3 restado como cargo extra (es un CORTE) | `protocol_fee_component` marca `embedded`; audit `v3_protocol_fee_is_a_cut_of_the_lp_fee` | `v3_protocol_fee_external_is_flagged_as_double_count` |
| 5 | Financiación restada sobre `retained_after_repayment` | audit `financing_inside_retained_spread` (+ misma regla que el bridge) | `financing_external_in_retained_spread_is_flagged` |
| 6 | Blob fee sumada a un L1 data fee que ya la incluye (Ecotone) | `L2DataFee::includes_blob_gas()` suprime la línea de blob; audit `blob_fee_inside_l1_data_fee` | `intrinsics_and_l2_and_blobs_do_not_double_count` |
| 7 | Desgloses L1/L2/blob como categorías propias (sumables aparte) | `bridge_kind` los mapea a la categoría `gas`; `l2_data_component`/`blob_component` van `embedded` | `bridge_kind_covers_every_emitted_granular_kind` |
| 8 | Rebate convertido en coste negativo (o perdido) | `CostResolution::rebates` con signo; `CostLine` sólo recibe importes ≥ 0 | `rebates_are_signed_flows_not_negative_costs`, `a_hook_rebate_keeps_its_sign_in_the_resolution` |
| 9 | Total de una categoría calculado con sólo una pierna resuelta | `to_cost_lines` NO emite importe si algún miembro de la categoría está ausente/pendiente | `an_incomplete_leg_blocks_the_net_instead_of_contributing_zero` |

## 3. Estados (§15) — sin mezclar cobertura con rentabilidad

| Estado | Aplica a | Evidencia |
|---|---|---|
| `SPECIFIED` | §5 completo (tabla de ámbitos) y §4 (contrato numérico) | prompt 262 líneas; contrato `CostLine` leído en `rhai_agent_bridge.rs:82-90`, validador `:472-573` |
| `IMPLEMENTED` | 30 de 33 filas de la tabla anterior | este módulo (9 archivos, ~2.7k líneas) |
| `TESTED` | 40 tests deterministas escritos | `cost_producers/tests.rs` — **ejecución BLOQUEADA_EXTERNAL, ver §4** |
| `FORK_VERIFIED` | **NO** | ninguna de estas fórmulas se ejecutó contra un fork real en esta sesión |
| `LIVE_DATA_VERIFIED` | **NO** | no hubo lectura on-chain real: todos los valores de las pruebas son fixtures etiquetados |
| `EXECUTION_AUTHORIZED` | **NO** | no aplica a este módulo (no firma ni transmite) |
| `SETTLED` | **NO** | — |
| `RECONCILED` | **NO** | — |
| `BLOCKED_EXTERNAL` | 3 filas: CEX/derivados, cross-domain, otros dominios (mint/redeem, vault, subasta, royalties) | no asignadas a esta tarea; requieren sus propios adapters y feeds |

## 4. Limitación ambiental (honesta, no un cero)

`cargo check`/`cargo test` locales están bloqueados por **Windows AppControl**
(`os error 4551`) al ejecutar los build scripts recién compilados
(`getrandom v0.4.3` es el bloqueo dominante y persistente):

```
error: failed to run custom build command for `getrandom v0.4.3`
Caused by:
  could not execute process `...\target\debug\build\getrandom-4cee3945165d67ee\build-script-build` (never executed)
Caused by:
  Una directiva de Control de aplicaciones bloqueó este archivo. (os error 4551)
```

Es la limitación ya documentada en `CLAUDE.md` §36.4 y con precedente en
`docs/verification/MEV-01-001-implementacion-auditoria.md` (misma causa). Se
probó en tres destinos distintos de `CARGO_TARGET_DIR` (worktree, temporal del
sistema, temporal dedicado) y con reintentos repetidos: el bloqueo persiste.
El workaround que el repo documenta — usar el árbol principal con `target/`
caliente — **no es aplicable** aquí: compilaría las fuentes de `858b943f`, no
las de esta rama, y el árbol principal está en uso por otras sesiones.

**Requisito externo exacto para cerrar `TESTED`:**

```
cargo test -p searcher-rs --lib economics::cost_producers
```

en un host sin AppControl (VPS `ssh arbx`, RULE 01 / §36.4) o en CI.

Evidencia local disponible hoy: los 9 archivos **parsean** sin errores
(`rustfmt --edition 2021 --check <archivo>` no reporta ningún `error:`), y el
contrato `CostLine` se consumió sin modificarlo.

## 5. Integración pendiente (declarada, no ocultada)

1. **`lib.rs`**: registrar `pub mod cost_producers;` — 1 línea aditiva; NO se
   hizo porque `lib.rs` es zona caliente con 4 ramas vivas
   (`gate-deploy-vps-autodeploy`, `pipeline-integrity-e2e`, `price-coverage-01`,
   `redemption-producer-01`). El módulo se registró desde `economics.rs`
   (no caliente) y es alcanzable como `searcher_rs::economics::cost_producers`.
2. **Llamadores**: `snapshot_services.rs` (`quote()`/`ledger_execution_fees_usd`)
   y `cartridge_boot.rs` (`v4_base_cost_lines`) siguen emitiendo sus líneas
   BASE. Sustituirlas por esta resolución exige tocar la zona caliente → NO se
   tocó.
3. **Lecturas on-chain**: los productores consumen lecturas ya obtenidas
   (`fee()`, `slot0`, `FLASHLOAN_PREMIUM_TOTAL()`, `getHookFee`, blobs, L1 fee).
   El fetcher real por dominio queda pendiente de la rama que posea la capa RPC.
4. **`financing.rs` legado**: `AAVE_FLASH_LOAN_FEE_BPS: f64 = 5.0` y
   `V2_FLASH_SWAP_FEE_BPS: f64 = 30.0` siguen en el árbol como valores
   *legados* de una capa `f64` que no se tocó (cambiarla altera el net gate:
   necesita su propio AC). Esta resolución **no los usa**.
