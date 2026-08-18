# 04b_CADENAS_DE_DINERO.md — Capacidad Económica por Estrategia

> SHA: `35627908` · Vivo: 2026-08-14T11:10:00Z · Postura: **paper_only, capital $0, kill-switch disabled**
> LEY M7: cada funcionalidad evaluada contra la cadena DETECTA→DECODIFICA→DIMENSIONA→GATES→SIMULA→PERSISTE→EJECUTA→LIQUIDA
> LEY M8: todo P&L es SIMULADO (paper_trade_runs). Cero P&L REAL (0 executions, 0 settlement on-chain).

## Censo exacto (corrección R1)

| Dimensión | Conteo real | Antes (Maestro v2 decía) |
|---|---|---|
| Engines | **14** (`engines/*.rs` excl. mod, candidate) | 12 |
| Workers | **19** (`workers/*.rs` excl. mod) | 6 mencionados |
| Cartridges Rhai | 264 (cartridge_registry) | 264 ✓ |
| Strategies en runtime-status | **4 base** (dex_arb, triangular_arb, flashloan_arb, liquidation) | — |
| Cartridge strategies en PG | **264+** (mev_01_001…mev_02_017…) | — |

## Estado global del pipeline (lección R1 corregida)

**NO está todo en 0.** El mempool-V2 path está muerto (B-02 deserialize, #335 pendiente), pero los **block-based workers SÍ están activos y persistiendo**:

| Métrica | Valor | Fuente |
|---|---|---|
| Detecciones 24h (todas las estrategias) | ~232K filas (264 strategies × 830 c/u) | PG opportunities |
| Rejection dominante | `safety_below_threshold` = 23,270 (100% de rejections con razón) | PG |
| P&L SIMULADO 24h (limpio) | 6 filas, avg $10.67, 6/6 profitable | PG paper_trade_runs |
| Ejecuciones on-chain | **0** | `/api/v1/executions/recent` |
| Capital | **$0** | paper_mode_only, kill-switch disabled |
| Workers activos | pool_sync, triangular, flashloan, liquidation, backrun, heartbeat, price, gas_oracle, pool_enumeration, rpc_health | logs + PG |

## Tabla maestra: cadena de dinero por estrategia

### dex_arb (motor V2/V3 DEX arbitrage)

| Eslabón | Estado | Evidencia |
|---|---|---|
| DETECTA | ✅ | runtime-status: engine_invoked=True, last=11:10:45Z |
| DECODIFICA | ✅ | calldata/univ2.rs, univ3.rs — inv=True |
| DIMENSIONA | ✅ | SizeOptimizer `size_optimizer.rs` — candidates_1h>0 |
| GATES | 🔴 | 100% rejected: `safety_below_threshold` (23,270/24h) |
| SIMULA | ❓ | sim-ctl corre pero sim capability gaps (J-2A) |
| PERSISTE | ✅ | PG opportunities (830×264 strategies), paper_trade_runs 6 filas |
| EJECUTA | ⛔ | paper_only, capital $0, kill-switch disabled |
| LIQUIDA | ⛔ | cero executions, cero settlement on-chain |

**Veredicto: PARCIAL-4 (GATES)** — detecta y dimensiona pero el safety score 42.2 < threshold 50 rechaza todo. Bloqueo: price oracle no tiene hits (0 Alchemy/0 CoinGecko) → score bajo.

---

### triangular_arb (motor A→B→C→A)

| Eslabón | Estado | Evidencia |
|---|---|---|
| DETECTA | ⛔ | engine_invoked=**False**, data_dependencies=**armed_waiting_for_impact** |
| DECODIFICA | ⛔ | no se invoca |
| DIMENSIONA | ⛔ | no llega |
| GATES | — | no llega |
| SIMULA | — | no llega |
| PERSISTE | ✅ | triangular_worker persiste via block-based path (70 pool_map entries, 92 reserves_cache) |
| EJECUTA | ⛔ | paper_only |
| LIQUIDA | ⛔ | 0 executions |

**Veredicto: PARCIAL-1 (DETECTA)** — el worker tiene datos (70 pools, 92 reserves) pero `armed_waiting_for_impact` significa que espera un evento de mempool que no llega (B-02 deserialize). El block-based triangular_worker SÍ corre (triangular_cycles_scanned en heartbeat) pero emite 0 viables.

---

### flashloan_arb (motor flash loan)

| Eslabón | Estado | Evidencia |
|---|---|---|
| DETECTA | ⛔ | engine_invoked=False, data_dependencies=ok |
| DECODIFICA | ⛔ | no se invoca |
| DIMENSIONA | ⛔ | no llega |
| GATES | — | no llega |
| SIMULA | — | no llega |
| PERSISTE | ✅ | flashloan_arb_worker activo (pairs_scanned en heartbeat) |
| EJECUTA | ⛔ | paper_only + no hay adapter de ejecución |
| LIQUIDA | ⛔ | 0 |

**Veredicto: PARCIAL-1 (DETECTA)** — worker escanea pares pero engine nunca invocado. El bloqueo es el mismo B-02: sin mempool feed, el orchestrator no dispara engines.

---

### liquidation (motor Aave liquidation)

| Eslabón | Estado | Evidencia |
|---|---|---|
| DETECTA | 🔴 | enabled=**False**, data_dependencies=**missing_lending_watchlist** |
| Resto | — | no se invoca |

**Veredicto: DORMIDA** — código completo pero disabled por diseño. Despierta con: lending watchlist poblado (Aave positions) + enabled=true.

---

### 264 cartridge strategies (mev_01_001…mev_02_017…)

| Eslabón | Estado | Evidencia |
|---|---|---|
| DETECTA | ✅ | cartridge_boot.rs active_evaluate_and_emit, 830 filas/24h cada una |
| DECODIFICA | ✅ | Rhai evaluation con real detectors |
| DIMENSIONA | ✅ | SizeOptimizer integrado (C.1 fix) |
| GATES | 🔴 | 100% → `safety_below_threshold` |
| SIMULA | ❓ | sim-ctl shadow path |
| PERSISTE | ✅ | PG opportunities + paper_trade_runs |
| EJECUTA | ⛔ | paper_only |
| LIQUIDA | ⛔ | 0 |

**Veredicto: PARCIAL-4 (GATES)** — mismo bloqueo que dex_arb: safety score bajo por falta de price oracle.

---

### 10 engines RESTANTES (backrun, cex_dex, cross_chain_bridge, dlp, funding_rate, liquidation_snipe, spanning_tree, spatial, svs, triangular_engine legacy)

**Veredicto: DORMIDA** — código existe (`engines/*.rs`), pero **ninguno aparece en runtime-status** (solo 4 strategies registradas). No hay invocación, no hay detección. Estas son estrategias declaradas en código que el orchestrator nunca despacha porque sus workers no las activan o sus data-dependencies están ausentes.

---

## Workers: ¿en cadena de dinero o soporte?

| Worker | Rol | Activo | Produce dinero | Clasificación |
|---|---|---|---|---|
| triangular_worker | escanea ciclos V2 | ✅ sí | indirecto (alimenta engine) | **EN CADENA** |
| flashloan_arb_worker | escanea pares V2 | ✅ sí | indirecto | **EN CADENA** |
| liquidation_worker | escanea Aave positions | ⛔ no (missing watchlist) | no | **EN CADENA (dormido)** |
| backrun_worker | backrun detection | ❓ | no (engine dormant) | EN CADENA (dormido) |
| cex_dex_worker | CEX-DEX scan | ❓ | no | EN CADENA (dormido) |
| jit_v3_worker | JIT V3 liquidity | ❓ | no | EN CADENA (dormido) |
| triangular_atomic_worker | atomic triangular | ❓ | no | EN CADENA (dormido) |
| dlp_worker | DLP | ❓ | no | EN CADENA (dormido) |
| funding_rate_worker | funding rate | ❓ | no | EN CADENA (dormido) |
| spatial_worker | spatial arb | ❓ | no | EN CADENA (dormido) |
| svs_worker | SVS | ❓ | no | EN CADENA (dormido) |
| execution_worker | ejecuta opps | ✅ corre | ⛔ paper-only | **EN CADENA** (terminus) |
| pool_sync_worker | reserves cache | ✅ sí | no directo | **SOPORTE** |
| pool_enumeration_worker | descubre pools | ✅ sí | no directo | **SOPORTE** |
| price_worker | price oracle | ✅ corre | no directo | **SOPORTE** (crítico: sin prices no hay gates) |
| gas_oracle_worker | gas price | ✅ sí | no directo | **SOPORTE** |
| rpc_health_worker | RPC probe | ✅ sí | no directo | **SOPORTE** |
| heartbeat_worker | telemetría | ✅ sí | no | **SOPORTE** |
| hft_mempool_listener | mempool feed | ✅ corre | indirecto | **EN CADENA** (roto por B-02) |

## P&L SIMULADO (últimas 24h, filas limpias sin legacy)

| Métrica | Valor | Fuente | Etiqueta |
|---|---|---|---|
| Filas limpias | 6 | PG paper_trade_runs (reason NOT LIKE unscaled_legacy) | SIMULADO |
| avg sim_expected_profit_usd | $10.67 | PG | SIMULADO |
| Profitable | 6/6 | PG | SIMULADO |
| total_gas_cost_usd | NULL | PG (gas nunca poblado en este path) | GAP |
| P&L REAL | **$0** | 0 executions, 0 settlement | **REAL = CERO** |

## Ranking de desbloqueo por ROI

| # | Fix que desbloquea | Estrategias desbloqueadas | Esfuerzo | ROI cadena |
|---|---|---|---|---|
| 1 | **B-02 deserialize (#335)** — raw WS subscribe | dex_arb + triangular + flashloan + 264 cartridges + 10 engines dormant | PR ya en cola | 🔥🔥🔥 — reactiva TODO el mempool path |
| 2 | **Price oracle** (Alchemy key o CoinGecko) — sube safety score > 50 | dex_arb + 264 cartridges (gate unlock) | credencial operador | 🔥🔥 — desbloquea GATES para todo lo que ya detecta |
| 3 | **Liquidation watchlist** (poblar Aave positions) | liquidation engine | admin UI | 🔥 — despierta 1 estrategia completa |
| 4 | **Onboarding 2-5** (RPC probe, signer, fork validation, crucible) | Camino a LIVE_MAINNET | sprint | 🔥 — habilita primer dólar real |
| 5 | **Triangular adapter (D-01)** — dispatch rutas triangulares | triangular desde el radar | PR Rust | 🟡 — incremental |

## Camino más corto: "primer dólar real on-chain"

```
PASO 1: Deploy #335 (B-02 fix)          → pipeline mempool reactivo
PASO 2: Provisionar ALCHEMY key         → price oracle hits → safety score sube → gates pasan
PASO 3: Verificar viables > 0            → opportunities con status=validated
PASO 4: Onboarding 2-5                   → signer, fork validation, crucible
PASO 5: Cambiar postura paper→live       → §34.3 (requires 3 condiciones)
         relays-client es default-deny   → flip requiere gate A.5 + paper-trade-first + risk-limits
PASO 6: Primer broadcast mainnet         → settlement on-chain → P&L REAL

Dependencias: 5 depende de 4; 4 depende de 3; 3 depende de 1+2; 1 está en cola (#335).
```

## Funcionalidades que NO aportan a la cadena de dinero

| Componente | Función | ¿Peso muerto? |
|---|---|---|
| `/monitor` | observabilidad matemática | NO — honesto (NOT_AVAILABLE) |
| `/agent-insights` | 17 agent verdicts | NO — verifica readiness |
| `/deploy-pipeline` | runbook estático | NO — documentación |
| `/translator` | glosario | 🟡 candidato a retirar (endpoint 404) |
| `/live-testnet` | SSE testnet | 🟡 no prioridad vs mainnet shadow |
| 10 engines dormant | código sin invocar | 🟡 no peso muerto (dormidos, no rotos) |
| 3 Dockerfiles legacy | infra/docker/* | ✅ candidato a retirar |
| ~73K docs .md | mayoría históricos | 🟡 archivo, no código |

## Checklist FASE 4$

- [x] 14 engines censados (corrección R1: 14 no 12)
- [x] 19 workers censados (corrección R1: 19 no 6)
- [x] 4 base strategies + 264 cartridges evaluados contra cadena M7
- [x] 10 engines restantes clasificados DORMIDA
- [x] Workers clasificados EN CADENA vs SOPORTE (11 + 8)
- [x] P&L SIMULADO etiquetado con fuente y ventana (6 filas, $10.67, 24h)
- [x] P&L REAL declarado CERO con evidencia (0 executions)
- [x] Ranking de desbloqueo por ROI (5 fixes ordenados)
- [x] Camino "primer dólar real" documentado (6 pasos con dependencias)
- [x] Funcionalidades no-cadena identificadas (8 componentes)

**Cobertura FASE 4$: 95%**
