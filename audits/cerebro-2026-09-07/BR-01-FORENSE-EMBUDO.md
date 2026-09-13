# BR-01 — FORENSE DEL EMBUDO COMPLETO stage-by-stage (datos vivos)

> **WO:** BR-01 · **kind:** design · **agente:** ecc:database-reviewer (Gang Omniscience)
> **Fecha/hora de medición:** 2026-09-07 12:51Z → 13:11Z (todas las horas UTC, VPS en UTC)
> **Método:** read-only TOTAL via `ssh arbx`: `psql SELECT` sobre
> opportunities/simulations/route_discovery_outcomes/paper_trade_runs, `redis-cli` read
> (XLEN/XINFO GROUPS), `docker logs` con verificación de ventana R9. HTTP público: **0/5**.
> **SHA desplegado en VPS:** `e65040f1` = Merge PR #555 (HOPS-LIVE-01) — desplegado a las
> **12:46:44Z** (verificado `git rev-parse HEAD` en `/opt/arbitragex-v2`). El board dice
> "PR #555 en CI": **desactualizado — ya está VIVO** (hallazgo BR-01-0).
> **R9 (ventana de logs):** `StartedAt=12:46:44.845Z`, primera línea retenida
> `12:46:45.057Z` → ventana boot-completa, sin gap de rotación (logcfg 5×10m). Las citas de
> logs cubren 12:46→13:03Z (~17 min) — declarado, no extrapolado.

---

## 0. Resumen ejecutivo (para el operador y la mesa)

1. **El embudo está 100% cerrado en la terminal de flujo**: 60,302 oportunidades/24h, **100%
   rejected**; 37,723 simulaciones/24h, **0 passed** (1,020,039 sims históricas, 0 aprobadas);
   `XLEN arbx:opps:simulated = 0` desde el origen de los tiempos; `paper_trade_runs` congelada
   desde 2026-09-01 16:32:06Z.
2. **La cuota de muerte depende de la ERA**. Este 12:46:44Z el deploy de #555 partió la
   historia en dos:
   - **Era pre-#555 (24h):** el asesino #1 es `TokenNotAllowed` (74.6% — AGLD+XEN, flood de
     2 tokens spam amplificado ×~37 por la matriz de cartuchos). El embudo gasta su
     presupuesto en ruido de tokens, no en matemática.
   - **Era post-#555 (17 min):** el asesino #1 es `v3_quote_unavailable` (**65.5%** — el
     oráculo V3/QuoterV2 no responde para las rutas canónicas `dex_arb_v3v3` que ahora
     dominan). La forma del embudo INVERTIÓ: `dex_arb` 70.6% del mix, self-pair 97.5%→29.4%,
     y **nació el multihop** (5,888 rutas de 3+ pools en 25 min).
3. **El grafo YA encuentra Topological Yield**: RU-3 marca 8,263 rutas `is_opportunity=true`
   en 3h (0.18% de 4.58M evaluaciones; 6,076 `closed_cycle_profit`). El #555 conectó las
   top-25/bloque al pipeline canónico (`route_scanner.canonical_dispatch budget=25
   dispatched=25` por bloque). El cuello se movió del grafo al **oráculo (S5)** y a las
   **reserves (S2/S4)**.
4. **BR-00 localizado con precisión quirúrgica**: `sim-ctl/src/tx_builder.rs:45-46` acepta
   EXACTAMENTE un `strategy_kind` (`dex_arb`); todo lo demás → `strategy_not_simulatable_in_s4`
   (96.55% de las sims 24h). El simulador ni siquiera intenta las 39 demás kinds.
5. **Dónde se pierde el Topological Yield real** (pares blue-chip WETH/USDT/USDC, no spam):
   `v3_quote_unavailable` 81.9% del sub-embudo no-self-pair 24h. Sin quote V3 no hay spread,
   sin spread no hay sizing, sin sizing no hay sim, sin sim no hay ledger.

---

## 1. SECCIÓN CANÓNICA NUMERADA DE STAGES — EL CONTRATO BR-10 (v1, 2026-09-07)

Este vocabulario es **el contrato estable** que BR-10 renderizará. Ocho compuertas, ID fijo,
nombre fijo, label de muerte canónico y gate con `file:line` en el código desplegado
(`e65040f1`). NOTA de arquitectura honesta: en el código, S5/S6 son sub-gates internos de S4
(el kernel `SizeOptimizer` hace oráculo→gas después del sizing de ruta), y S8 corre aguas
abajo en `sim-ctl`/`relays-client` después de la emisión a stream. El orden del contrato es
el orden lógico de evaluación de una oportunidad, no el orden de procesos del sistema.

| ID | Stage (nombre canónico) | Qué decide | Gate (file:line desplegado) | Labels de muerte canónicos |
|----|--------------------------|------------|------------------------------|-----------------------------|
| **S1** | `decode` | Ingesta mempool/bloque → intención decodificada | `chain_client.rs:280-302` (modo), `scanner.rs` (heartbeat counters: `pending_received/decoded_ok/decoded_err` en `workers/heartbeat_worker.rs`) | (sin etiqueta PG — contadores de heartbeat; RDO `source_event`) |
| **S2** | `graph` | Universo de pools con reserves frescas + enumeración de ciclos (RU-3) | `workers/route_scanner_worker.rs:449` (`route_scanner.done`: cycles_found/dispatched/shadow_forced), admisión `:163-165`, telemetría en `route_discovery_outcomes` | RDO: `missing_reserves`, `had_reserves=f`, `route_shape_out_of_bounds`, `*_feed_unavailable` |
| **S3** | `engine` | Candidato topológico por motor nativo o cartucho | `engines/dex_engine.rs:203` (`single_pool_no_spread`), `:335-336` (`v3_quote_unavailable`/`spread_zero_equilibrium` — ver S5), `engines/triangular_engine.rs:437-440` (`spot_product_le_one`); cartuchos: `cartridge_boot.rs:1383-1397` + summary `active_eval_summary` | `single_pool_no_spread`, `spot_product_le_one`, `spread_zero_equilibrium`, `*_feed_unavailable` (Rhai, 24 familias) |
| **S4** | `size` | Sizing convexo CFMM del kernel económico (`SizeOptimizer` ES el motor económico) | `size_optimizer.rs:684,844,1099` (`missing_route_legs`), `:704,872,1113` (`missing_reserves_pool_b`), `:728,921,940,1143` (`non_positive_profit`); llamados desde `cartridge_boot.rs:1407` (path cartucho, "C.1 size BEFORE gates") y `orchestrator.rs:903` (path nativo) | `missing_reserves_pool_b`, `missing_route_legs`, `non_positive_profit`, `non_positive_gross_usd`, `non_positive_net_usd`, `zero_reserves` |
| **S5** | `oracle` | Precio/quote para valorar el spread (V2 cascade → V3 QuoterV2 staticcall) | `size_optimizer.rs:393` (`unknown_token_price`); `engines/dex_engine.rs:423-424` (sin projector), `:433-444` (quote de pierna falla), `:446-450` (ambas cero) → rechazo emitido `:333-356`; wiring del oráculo `scanner.rs:332-368` (`scanner.v3_oracle_wired` verificado 12:57:39Z) | `v3_quote_unavailable`, `unknown_token_price`, `no_price_oracle` |
| **S6** | `gas` | Gas floor: net ≥ k × cost-proxy; Kelly de borde no-negativo | `size_optimizer.rs:524-533` (`GasFloorBreach`, net < `kelly_gas_safety_multiplier` × (gross−net)), `:631` (2º sitio), Kelly `:552` | `gas_floor_breach` (con sufijo `:financing_mode` — `is_net_dependent()` `:133-138`), `kelly_negative_edge` |
| **S7** | `spine` (allowlist/estrategia) | Allowlist de tokens addr-keyed + estrategia habilitada (corre DESPUÉS del sizing en el path cartucho) | `prioritization-spine/src/config_aware.rs:604-612` (identity-mode `is_allowed_addr`), `:648-655` (legacy), `prioritization-spine/src/decision.rs:137`; emitido en `cartridge_boot.rs:1649-1658` y `orchestrator.rs:1236-1239` | `TokenNotAllowed:<addr>`, `StrategyDisabled:<kind>`, `StrategyConfigGateBlocked:*`, `NoTradingConfig` (`cartridge_boot.rs:1598-1606`) |
| **S8** | `sim→emit` (simulación y terminus) | Simulabilidad + veredicto on-chain honesto + ledger paper | `sim-ctl/src/tx_builder.rs:45-46` (**solo `dex_arb` pasa**), `sim-ctl/src/sim_engine.rs:44` (`strategy_not_simulatable_in_s4`), `:31-38` (fork ausente); terminus `relays-client/src/consumer.rs:21` (grupo sobre `arbx:opps:simulated`) | `strategy_not_simulatable_in_s4`, `anvil_fork_not_configured*`, `build_error:*`, `reverted:*` (TRANSFER_FROM_FAILED/STF/INSUFFICIENT_OUTPUT), `sim_timeout`, `rpc_error:*` |
| **S9** | `emit` (emisión/persistencia) | Dedup → PG insert → Redis XADD `arbx:opps:detected` | `opportunity_emitter.rs:211` (`emit_accepted`), `:363` (`emit_rejected`), `:294-329` (I/O path: dedup → Gate-C scoring → PG → XADD) | (la emisión vive: 60,302 filas/24h; muere por AUSENCIA de accepted, no por falla del emitter) |

**Regla de conservación del contrato (para BR-10):** toda fila de `opportunities` mapea a
exactamente UN bucket S3-S7 (por `rejection_reason`) o a `PASS` (NULL). La suma de cuotas ==
total emitido. Ver invariante INV-BR01-1 en §7.

---

## 2. El embudo en cifras (pirámide de throughput, ventanas declaradas)

```
S1/S2  RDO (graph, 3h)          4,584,930 evaluaciones  (mode=shadow, RU-3, ~424/s)
S2     RDO is_opportunity (3h)       8,263  rutas con profit (0.18%)  ← Topological Yield hallado
S2     RDO had_reserves=f (3h)   2,552,541  (55.7% sin reserves frescas)
S9     opportunities (24h)          60,302  (100.0% rejected, frescura max(detected_at)=16s)
S8     simulations (24h)            37,723  (100.0% passed=f)
S8     simulator=anvil (24h)         1,301  (3.45% — los demás ni lo intentan)
S8     passed (24h + historia)           0  (1,020,039 sims totales, 0 aprobadas JAMÁS)
S8     XLEN arbx:opps:simulated          0  (stream vacío desde el origen)
T      paper_trade_runs 7d               1  (congelada desde 2026-09-01 16:32:06Z)
```

Cadencia 24h (por hora): continua, 1,000-3,948/h, sin huecos >1h (R7 satisfecho: el loop
nunca muere; lo que muere es la economía). Saltos visibles solo en redeploys.

**Hueco opps→sims:** 22,774 de 60,302 (37.8%) no reciben fila de simulación. Las sims cubren
TODAS las familias de rechazo (incluidas TokenNotAllowed 23,044) → no hay filtro por razón;
es rate/dedup del consumo del stream `arbx:opps:validated` (sim-ctl-g0: 58 consumers, lag
8,119 < XLEN 10,003, sin pérdida por trim aún). NO adjudicado a un gate: el invitation
filter exacto queda como UNKNOWN honesto (§8).

---

## 3. CUOTA DE MUERTE EXACTA POR COMPUERTA (tablas maestras)

### 3.1 Era pre-#555 — ventana 24h (2026-09-06 12:51Z → 2026-09-07 12:51Z), snapshot 12:51:06Z

Total: **60,302** · status=rejected: 60,302 (100.0%) · rejection_reason NULL: **0**.

| Stage | Razón de muerte | Filas | % del total | Gate (file:line) |
|-------|-----------------|-------|-------------|-------------------|
| S7 spine | `TokenNotAllowed:0x3235…(AGLD)` 22,960 + `0x0645…(XEN)` 22,064 | 44,968 | **74.57%** | `config_aware.rs:604-612` → `cartridge_boot.rs:1649-1658` |
| S4 size | `missing_reserves_pool_b` | 10,852 | 18.00% | `size_optimizer.rs:704,872,1113` |
| S4 size | `non_positive_profit` | 1,517 | 2.52% | `size_optimizer.rs:728,921,940,1143` |
| S5 oracle | `v3_quote_unavailable` | 1,244 | 2.06% | `dex_engine.rs:423-450` → `:335` |
| S4 size | `missing_route_legs` | 575 | 0.95% | `size_optimizer.rs:684,844,1099` |
| S6 gas | `gas_floor_breach` | 420 | 0.70% | `size_optimizer.rs:524-533,631` |
| S5 oracle | `unknown_token_price` | 280 | 0.46% | `size_optimizer.rs:393` |
| S3 engine | `single_pool_no_spread` | 248 | 0.41% | `dex_engine.rs:203` |
| S3 engine | `spot_product_le_one` | 152 | 0.25% | `triangular_engine.rs:437-440` |
| S7 spine | `StrategyDisabled` | 40 | 0.07% | `config_aware.rs` → `cartridge_boot.rs:1659-1666` |
| S3 engine | `spread_zero_equilibrium` | 6 | 0.01% | `dex_engine.rs:336` |
| **Σ** | | **60,302** | **100.00%** | |

**Cuota agregada por stage (24h):** S7 45,008 (74.64%) · S4 12,944 (21.46%) · S5 1,524
(2.53%) · S6 420 (0.70%) · S3 406 (0.67%). Conservación exacta.

**Matriz familia × razón (24h, hallazgo estructural):** el embudo pre-#555 son TRES embudos
superpuestos con asesinos distintos:

| Familia | Asesino dominante | Lectura |
|---------|-------------------|---------|
| `mev_01_*` (AMM arb) | TokenNotAllowed 33,978/34,251 (99.2%) | tokens spam (AGLD), sizing PAGADO antes del rechazo |
| `mev_02_*` | TokenNotAllowed 11,326/11,417 (99.2%) | ídem |
| `mev_04_*` (wrapper/vault/LST) | `missing_reserves_pool_b` 11,392/13,727 (83.0%) | **reserves no cacheadas para el pool B** (BR-02) |
| `dex_arb` (nativo) | `v3_quote_unavailable` 1,244/1,575 (79.0%) | pares blue-chip REALES (WETH/USDT 609, USDC/WETH 603) sin quote V3 |
| `triangular` (nativo) | `spot_product_le_one` 152/152 (100%) | matemática honesta: el ciclo no tiene spread |

Multiplicador de cartucho: ≥15 kinds emiten **exactamente 1,623 c/u** en 24h (40 kinds
distintos) — cada evento disparador se amplifica ~×37 antes de PG/Redis. Es el
`arbx_emission_multiplier` que pidió N4, medido.

### 3.2 Era post-#555 — desde 12:46:44Z (deploy #555), snapshots 13:0xZ (~17-25 min)

> **[SUPERSEDED 17:45Z por §10.1]** — esta tabla fue capturada con 17 min de era-2. La
> cuota MADURA (4.98h, 828,548 filas) está en §10.1: `v3_quote_unavailable` subió de 65.5%
> a **77.86%**. Esta tabla se conserva como registro de la captura temprana, no como cifra
> de referencia — usar §10.1 para BR-10 y para gates.

Total razonado: 11,031 (snapshot de razones; el total de filas 9,283 fue capturado minutos
antes — deriva de ventana declarada, §8). **Sigue 100% rejected.**

| Stage | Razón | Filas | % | Nota |
|-------|-------|-------|---|------|
| S5 oracle | `v3_quote_unavailable` | 7,230 | **65.5%** | strategy `dex_arb_v3v3` (logs `v2.emitter.input`, 13,429 eventos en 17 min); oráculo WIRED (`scanner.v3_oracle_wired` 12:57:39Z) pero el staticcall QuoterV2 falla en runtime |
| S3 engine | `spot_product_le_one` | 1,063 | 9.6% | multihop 3+ pools: ciclo cerrado sin spread — **matemática honesta, no falla** |
| S7 spine | `TokenNotAllowed` | 980 | 8.9% | flood residual (mempool) |
| S4 size | `missing_reserves_pool_b` | 947 | 8.6% | el otro asesino del multihop (BR-02) |
| S4 size | `non_positive_profit` | 355 | 3.2% | |
| S3 engine | `single_pool_no_spread` | 405 | 3.7% | |
| S4 size | `missing_route_legs` | 40 | 0.4% | |
| S3 engine | `spread_zero_equilibrium` | 11 | 0.1% | |

**Mix post-deploy (14 min):** `dex_arb` 6,554 (70.6%) + `triangular` 850 (9.2%) dominan;
mev_01/mev_02 DESAPARECIERON del top (el flood era mempool-driven; post-restart el path
dominante es block-driven). Self-pair: 2,729/9,283 = **29.4%** (era 97.5%). Pools por ruta:
2 pools 81%, **3 pools 850, 5 pools 897, 4 pools 10 → multihop VIVO**. Pares multihop top:
`WETH(triangular)` 853, `0xc02aaa…/0xc02aaa…` (ciclos WETH→X→WETH por pools distintos) 820,
`USDC(triangular)` 210.

### 3.3 S8 sim (24h): el embudo de la simulación

| Sub-gate | Filas | % de 37,723 | file:line |
|-----------|-------|-------------|-----------|
| `strategy_not_simulatable_in_s4` | 36,422 | **96.55%** | `sim-ctl/src/tx_builder.rs:45-46` → `sim_engine.rs:44` |
| `reverted: TransferHelper TRANSFER_FROM_FAILED` | 681 | 1.81% | runtime anvil (probe revierte on-chain) |
| `sim_timeout` | 270 | 0.72% | `sim_engine.rs` timeout |
| `reverted: STF` | 258 | 0.68% | runtime anvil |
| `build_error: router not in catalog… PancakeSwap V3` | 60 | 0.16% | `tx_builder.rs:53` (catálogo de routers) |
| `reverted: INSUFFICIENT_OUTPUT_AMOUNT` | 7 | 0.02% | runtime anvil |
| `rpc_error` (free-plan 408) | ~8 | ~0.02% | RPC externo del fork |

Por columna `simulator`: `not_implemented` 36,658 (97.2%) vs `anvil` 1,301 (3.4%). **El
catálogo S4 soporta UNA kind (`dex_arb`) de las 40 emitidas** — censo BR-00 cerrado desde
datos vivos. De los 1,301 que llegan a anvil: 0 pasan (52.3% TRANSFER_FROM_FAILED, 20.8%
timeout, 19.8% STF — el probe revierte: la economics que el detector creyó ver no sobrevive
el fork, o el from-address del probe no tiene balance/approval).

### 3.4 S2 graph (RDO, 3h): la boca del embudo

4,584,930 outcomes (100% mode=shadow — el telemetría del RU-3; la pierna canónica de #555 es
un dispatch aparte, no cambia el `mode` de RDO). Top razones: `missing_reserves` 688,204
(15.0%) · `bridge_state_unavailable` 528,375 · `funding_feed_unavailable` 528,360 ·
`nft_floor_feed_unavailable` 306,788 · `intent/cex_feed_unavailable` 238,614 c/u ·
`lending/prediction_market` ~153,4xx c/u · `route_shape_out_of_bounds` 104,937. Es decir: la
mitad de las muertes del grafo son **feeds declarados ausentes** (cartuchos de universos que
no tienen fuente de datos cableada) y la otra mitad **reserves/estado no fresco**
(`had_reserves=f` 55.7%).

Por bloque (logs `route_scanner.done` 12:59Z): `cycles_found=500` (cap), `dispatched=319-383`,
`shadow_forced=117-181`, `capped=true`, `enumeration_ms=8-61`, `elapsed_ms=102-155`. Y
`route_scanner.canonical_dispatch budget=25 dispatched=25` (bloques 25925631-2, cada ~12s) —
**la mano del grafo ya está dentro del pipeline canónico**.

---

## 4. DÓNDE SE PIERDE EL TOPOLOGICAL YIELD (el mapa económico)

Tres estratos, tres diagnósticos distintos:

1. **Yield ya hallado y no consumido (pre-#555):** 8,263 rutas/3h marcadas
   `is_opportunity=true` por RU-3 en shadow. Desde las 12:46Z las top-25/bloque entran al
   canónico (#555) — el resto (≈2.7K/h) sigue siendo telemetría pura. El techo de consumo
   es un knob (`route_scanner_worker.rs:218-227`, `canonical_per_block=25`), no un límite
   matemático.
2. **Yield real que muere en el oráculo (post-#555, CRÍTICO ahora):** los pares blue-chip
   (WETH/USDT, USDC/WETH — 81.9% del sub-embudo no-self-pair 24h y 65.5% del total
   post-deploy) mueren en `v3_quote_unavailable` ANTES de poder demostrar spread. El oráculo
   V3 está wired pero el quote runtime falla (RpcCircuitBreaker del pool — N5 midió 5/9
   providers vivos, 2 FIRING). **Sin S5 no hay S6 ni S8**: esto es hoy el cuello de botella
   #1 del sistema post-#555 y conecta directo con BR-03 (cascada de oráculos) y BR-08.
3. **Yield que sobrevive detección y muere en simulación (BR-00):** de lo poco que llega,
   96.55% muere porque el S4 solo sabe construir probes para `dex_arb`
   (`tx_builder.rs:45`). El `mev_04_*` (missing_reserves 83%) ni siquiera llega a sim:
   el sizing lo mata antes por reserves ausentes (BR-02, carrera de reserves — coherente
   con `had_reserves=f` 55.7% en RDO).

**Lectura de asignación de presupuesto del cerebro:** en la era pre-#555, 74.6% del gasto
(tokens spam sizing+emisión+persistencia incluidos, ×37 de multiplicador) fue invertido en
candidatos que el allowlist iba a rechazar de todos modos — el sizing corre ANTES del gate
(`cartridge_boot.rs:1399-1404` "C.1: run SizeOptimizer BEFORE the gates"). Reordenar
(allowlist antes de sizing) o filtrar en S2 por liquidez (BR-04/WO-06) libera ~3/4 del
presupuesto computacional sin tocar la matemática.

---

## 5. Citas y refutaciones al prior art (por archivo)

### A `audits/omniscience-integration-2026-09-06/04-searcher-pipeline-CROSS.md` (N4)

- **CONFIRMO** (re-medición viva): 100% rejected sostenido; multiplicador de cartucho (ahora
  1,623 exactos por kind, ≥15 kinds — tu "~28×" era 09-06; hoy ~×37); rotación de tokens
  spam (tu PEPE 25.8% DESAPARECIÓ hoy; solo AGLD+XEN quedan); `arbx:hot:*` XLEN=0; churn de
  consumers (75+66+58 orphans vs tus 63+59+51).
- **REFUTO el detalle de tu caracterización estructural** ("self-pair … matemáticamente
  no-ops (swap X→X)", línea 26): el 99.5% de los self-pairs 2-pool del flood usan **pools
  DISTINTOS** (46,618 distinct vs 248 same-pool, 24h pre-deploy). Un ciclo cerrado A→B→A por
  P1≠P2 es la topología canónica de arb 2-hop — no un no-op de forma. La basura es el TOKEN
  (AGLD/XEN sin liquidez real, correctly killed por el allowlist), no la forma de la ruta.
  Importa para BR-04: el filtro anti-spam correcto es por **token/liquidez** (dinámico), NO
  por forma `token_in==token_out` — un filtro por forma mataría también los ciclos cerrados
  legítimos WETH→X→WETH que #555 acaba de empezar a traer (820 en 14 min).
- **ACTUALIZO** tu cifra heredada: 57,981 → **60,302**/24h (mi ventana) y tu nota de que
  "cada recreate = feed público 0 por 5-7 min" sigue vigente (deploy 12:46Z visible).

### A `audits/omniscience-integration-2026-09-06/05-simulator-family-CROSS.md` (N5)

- **CONFIRMO todo tu §1 hoy**: `XLEN arbx:opps:simulated`=0 (12:51Z), sims passed=0
  (37,723/24h), `simulator=not_implemented` dominante, paper_trade_runs congelada
  09-01 16:32:06Z.
- **CIERRO tu pregunta 5 a N4 con dato nuevo**: la invitación a sim NO filtra por razón
  (TokenNotAllowed también se simula: 23,044 filas con sim). El 37.8% sin sim es
  rate/dedup del consumo del stream, no un gate declarado (§2).
- **PRECISO tu D-7** ("el catálogo no cubre lo que emite el searcher"): la causa raíz es UNA
  línea — `sim-ctl/src/tx_builder.rs:45-46` rechaza todo kind ≠ `dex_arb`. No es un catálogo
  parcial: es un catálogo de exactamente 1 entrada vs 40 kinds emitidas. BR-00 tiene
  objetivo único y medible.
- Tu secuencia propuesta (fix alertas → flip revm → primer passed) sigue siendo la
  correcta; agrego que #555 ya cambió el mix de entrada del simulador (más `dex_arb`
  70.6% — la ÚNICA kind que S4 sabe simular — y menos spam): **el flip revm/BR-00 ahora
  tiene más masa útil que cuando escribiste tu reporte.**

### A la memoria `arbx-rejection-taxonomy-2026-09-06` (48.4K/24h, XEN+AGLD 78%)

- **ACTUALIZO**: 60,302/24h; AGLD+XEN = 44,968 TokenNotAllowed = 74.6% (los dos tokens
  suman TODO el TokenNotAllowed; el resto del flood murió en otros gates). La taxonomía
  completa de 24h está en §3.1 con los 11 labels exactos.

### Al board `GOAL-WORKORDERS.md` (líneas 14-17, evidencia forense acumulada)

- "59,140 opps/24h TODAS legs=1" → **matiz**: 60,302 opps; el 77.7% son rutas 2-pool (2
  piernas) cerradas — no "legs=1"; y ya no "todas": ver siguiente.
- "0 multihop en 7 días" → **REFUTADO con histograma diario** (rutas 3+ pools):
  08-30: 741 · 08-31: 1,366 · 09-01: 1,298 · 09-02: 191 · 09-03: 66 · 09-04: 80 ·
  09-05: 4 · **09-06: 7,110** (RU-3 ON 12:13Z) · **09-07: 11,538 y contando** (#555
  12:46Z). Tu claim era cierto SOLO en la ventana 09-04→09-05; RU-3 ya lo había roto el
  09-06 y #555 lo multiplicó. La métrica "multihop/día" es ahora un indicador vivo del
  efecto #555.
- "missing_reserves = 79% del sink multihop (196,927/h)" → **mismo orden, cifras mías**:
  RDO `missing_reserves` = 688,204/3h ≈ 229K/h (tu 196.9K/h mediste otra ventana); en el
  sink multihop de `opportunities` post-#555: `missing_reserves_pool_b` = 947/2,050 = 46.2%
  (el resto muere en `spot_product_le_one` honesto). La carrera de reserves (BR-02) sigue
  siendo el asesino #1 del graph stage (`had_reserves=f` 55.7%).
- "RU-3 ya ON (500 ciclos/bloque, 272-380 despachados)" → **CONFIRMADO** (319-383 hoy) y
  **EXTENDIDO**: + `canonical_dispatch budget=25 dispatched=25`/bloque VIVO desde 12:46Z
  (#555 deployado, no "en CI").
- "PR #555 … en CI" → **desactualizado**: `git rev-parse HEAD` = e65040f1 (merge #555) en
  el VPS; flota recreada 12:46:44Z.

### A `audits/control-board-2026-09-07/CB-05-PROPUESTA-B.md`

- CONFIRMO tu hallazgo del día desde mi superficie: searcher recreado (tú viste 12:13:00Z;
  la flota completa volvió a recrearse 12:46:44Z con #555 — churn de deploy que documenta
  N8). Tu inventario Clase B me sirvió de mapa para `ARBX_NATIVE_ENGINES`/`ARBX_MEMPOOL_MODE`.

---

## 6. Streams Redis y terminus (estado vivo 12:51Z)

| Stream | XLEN | Grupo | Consumers | Lag | Veredicto |
|--------|------|-------|-----------|-----|-----------|
| `arbx:opps:detected` | 10,000 (MAXLEN) | enricher | 1 | 11,228 | **lag > XLEN → pérdida por trim estructural** (N4/N7 vigente) |
| | | paper-archiver-g0 | 75 | 11,292 | ídem + orphans +N/restart |
| | | selector-g0 | 66 | 11,292 | ídem |
| `arbx:opps:validated` | 10,003 | sim-ctl-g0 | 58 | 8,119 | lag < XLEN: sin pérdida AÚN |
| `arbx:opps:simulated` | **0** | relays-client-g0 | (consumer vivo, esperando) | — | **jamás un mensaje en la historia** |
| `arbx:hot:detected` / `arbx:hot:simulated` | 0 / 0 | — | — | — | cable muerto confirmado por 3ª vez |

Entradas históricas `detected`: entries-added 546,976 (vista de grupo) — el sistema emite
~0.7/s sostenido. La pérdida por trim (~1.2K entradas/recorte) es un hueco de trazabilidad
del ledger paper, NO la causa de la sequía (la causa es 0 passed, arriba).

---

## 7. DESIGN (kind: design) — telemetría del embudo para el cerebro

Produce el diseño con diffs exactos + invariantes + gate. **NO se editó código de producción**
(NO-GIT; los diffs son el artefacto de diseño para que el orquestador decida PR con ID P-∅).

### D1 — Vista SQL de cuota por stage (el corazón de BR-10)

```sql
-- WO BR-01 (2026-09-07) — DESIGN ONLY. Aplica el orquestador vía PR con ID (P-∅ §37).
CREATE OR REPLACE VIEW v_br01_funnel_quota AS
SELECT
  date_trunc('hour', detected_at)                          AS hour_utc,
  CASE
    WHEN rejection_reason LIKE 'TokenNotAllowed%'
      OR rejection_reason LIKE 'StrategyDisabled%'
      OR rejection_reason = 'NoTradingConfig'
      OR rejection_reason LIKE 'StrategyConfigGateBlocked%'        THEN 'S7_spine'
    WHEN rejection_reason IN ('missing_reserves_pool_a','missing_reserves_pool_b',
                              'zero_reserves','missing_route_legs',
                              'missing_pool_address')             THEN 'S4_size_route'
    WHEN rejection_reason IN ('non_positive_profit','non_positive_gross_usd',
                              'non_positive_net_usd')             THEN 'S4_size_profit'
    WHEN rejection_reason IN ('v3_quote_unavailable','unknown_token_price',
                              'no_price_oracle')                  THEN 'S5_oracle'
    WHEN rejection_reason LIKE '%gas_floor_breach%'
      OR rejection_reason = 'kelly_negative_edge'                 THEN 'S6_gas_kelly'
    WHEN rejection_reason IN ('spot_product_le_one','single_pool_no_spread',
                              'spread_zero_equilibrium')          THEN 'S3_engine'
    WHEN rejection_reason IS NULL                                 THEN 'PASS'
    ELSE 'S9_unmapped'
  END                                                       AS stage,
  count(*)                                                  AS rows
FROM opportunities
WHERE detected_at >= now() - interval '48 hours'
GROUP BY 1, 2;
```

### D2 — Query de conservación (gate de la vista)

```sql
-- WO BR-01 (2026-09-07) — gate: S9_unmapped debe ser 0 y la suma debe calzar.
SELECT stage, sum(rows) FROM v_br01_funnel_quota
WHERE hour_utc >= date_trunc('hour', now()) - interval '24 hours'
GROUP BY stage HAVING stage = 'S9_unmapped';
-- Expected: 0 filas. Cualquier S9_unmapped > 0 = taxonomy drift → PR con ID.
```

### D3 — Diffs propuestos (marcados, para el PR futuro del orquestador)

1. `backend/searcher-rs/src/cartridge_boot.rs` — mover la invocación del evaluator de
   allowlist ANTES de `size_optimizer.optimize_with_reason` (hoy `:1399-1477` sizing →
   `:1617` spine). Diferencia esperada: ~45K sizings/día ahorrados (74.6% del gasto del
   kernel en candidatos que el spine mataría igual). Marcador `// BR-01 (2026-09-07)`.
   **Cuidado (R5-style):** el spine consume `price_snapshot` y el sizing produce
   `leg_ledger`; el reorder exige snapshot antes — diseño detallado lo hace BR-04/WO-06
   (anti-spam tiering), que ya existe. BR-01 solo aporta la cifra que lo justifica.
2. `monitoring/…` o api-server: exponer `v_br01_funnel_quota` como
   `arbx_funnel_stage_quota` gauge por stage — BR-10 lo renderiza. Marcador
   `// BR-01 (2026-09-07)`.

### Invariantes

- **INV-BR01-1 (conservación):** para toda ventana, `Σ rows(stage) == COUNT(*)` de
  `opportunities` en la ventana, y `S9_unmapped == 0`. El embudo no pierde filas por
  clasificación — si pierde, es taxonomy drift visible, no silencioso.
- **INV-BR01-2 (fail-honest R8):** la vista NO inventa buckets: razón desconocida →
  `S9_unmapped` visible (nunca reclasificado a ojo). `PASS` solo con `rejection_reason IS
  NULL` (hoy: 0 en 24h — y esa ES la verdad).
- **INV-BR01-3 (ventanas UTC declaradas):** toda cuota cita su ventana; las dos eras
  (pre/post 12:46:44Z #555) NUNCA se mezclan en un solo número agregado — el deploy partió
  la distribución y promediarlas fabrica un embudo que no existe.
- **INV-BR01-4 (read-only):** BR-01 no mutó VPS alguno (solo SELECT/XLEN/XINFO/logs); los
  diffs de D3 quedan como diseño para PR con ID del orquestador.

### Gate de verificación (re-ejecutable por cualquiera)

```bash
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F'|' -c \
  \"SELECT COALESCE(split_part(rejection_reason,':',1),'(PASS)'), count(*) FROM opportunities \
   WHERE detected_at >= now() - interval '24 hours' GROUP BY 1 ORDER BY 2 DESC;\""
# Σ debe == COUNT(*) de la misma ventana; y (hasta que BR-00/BR-03 aterricen):
# XLEN arbx:opps:simulated == 0 sigue siendo la verdad del terminus.
```

---

## 8. Fail-honest (lo que NO pude verificar / declaro UNKNOWN)

1. **Ventana de logs corta (R9 satisfecho pero angosta):** contenedor nuevo 12:46:44Z →
   17 min de logs. El `pending_received=0` en 5 heartbeats consecutivos vs RDO
   `public_mempool=18,830/3h` (que incluye era pre-restart) NO adjudica si el feed de
   pendings está vivo post-restart: los counters del heartbeat (V1 path) están
   desacoplados de la emisión activa (`pg_period_inserted` 2,089-2,823/min en los MISMOS
   heartbeats). UNKNOWN honesto; requiere ventana ≥1h post-boot.
   **[RESUELTO 18:16Z en §10.3-F4:** ventana post-boot cumplida; counters V1 siguen en 0
   sostenido con emisión a ~50/s → el heartbeat mide un path muerto, no un feed caído.]
2. **Deriva entre snapshots:** cada query corrió en instante distinto (12:51→13:11Z); las
   cifras 24h son del snapshot 12:51:06Z y las post-deploy de 13:0xZ. Diferencias de ±1-2%
   entre totales de queries vecinas son deriva de ventana, no inconsistencia.
3. **Filtro de invitación a sim (22,774 sin sim):** demostrado que NO es por razón de
   rechazo; el mecanismo exacto (dedup por trace/rate del sim-ctl) no está adjudicado a
   file:line en esta pasada.
4. **Causa runtime exacta de `v3_quote_unavailable`:** el oráculo está wired
   (`scanner.v3_oracle_wired`); la falla está en el staticcall QuoterV2 (provider
   `v3_quote_provider.rs:93-111` — "rpc failover exhausted"). La atribución fina al pool
   degradado (5/9, breakers FIRING — evidencia de N5, no mía) queda INFERRED, citada.
5. **`simulator='not_implemented'` 36,658 vs `strategy_not_simulatable_in_s4` 36,422:** el
   delta 236 corresponde a otras razones not_implemented (p.ej. chain no soportada) — no
   desglosado en esta pasada.

---

## 9. Entrega a la mesa (downstreams)

- **BR-00 (P0):** tu censo está cerrado — §3.3 + `tx_builder.rs:45-46`. La métrica de éxito
  (98.2% → <50%) debe medirse sobre `simulations.fail_reason` con la ventana INV-BR01-3;
  ojo que el mix post-#555 ya subió el share `dex_arb` (la única kind simulable) a 70.6%
  del input — tu porcentaje puede mejorar SOLO por cambio de mix: mide también el
  conteo absoluto por kind.
- **BR-02 (reserves):** tu blanco tiene dos cifras de referencia — `had_reserves=f` 55.7%
  (RDO 3h) y `missing_reserves_pool_b` 947/2,050 (46.2% del sink multihop post-#555;
  83.0% de la familia mev_04 en 24h).
- **BR-03 (oráculos):** `v3_quote_unavailable` es ahora el #1 global (65.5% post-#555).
  Tu diseño debe contemplar que el V2-cascade también muere (`unknown_token_price` 280 +
  `no_price_oracle` residual).
- **BR-04 (anti-spam):** §5/N4 — filtra por TOKEN/liquidez dinámica, NO por forma
  token_in==token_out (mataría los ciclos cerrados legítimos de #555). El ahorro medido
  del reorder sizing→spine: ~74.6% del gasto del kernel en 24h pre-#555.
- **BR-08 (latencia):** `route_scanner.done elapsed_ms=102-155`/bloque con
  `enumeration_ms=8-61` — el grafo no es el cuello; tu p95 764ms vive más arriba en el
  I/O de emisión (dedup→PG→XADD, `opportunity_emitter.rs:294-329`).
- **BR-10 (UI):** §1 es tu contrato (S1-S9, labels, file:line, queries D1/D2). Renderiza
  las DOS eras por separado (INV-BR01-3) con el corte 2026-09-07 12:46:44Z marcado.

**Veredicto BR-01:** el embudo es estructuralmente vivo (cadencia continua, emisión y
persistencia sanas, grafo productivo desde #555) y económicamente mudo en el terminus (0
passed en 1,020,039 sims históricas, ledger congelado 6 días). La muerte se reparte:
pre-#555 era ruido de tokens (S7 74.6%); post-#555 es oráculo (S5 65.5%) + reserves (S4/S2)
+ cobertura de simulación (S8 96.6%). Ninguna de las tres es el mercado: gas 0.087 gwei y
6,076 ciclos con profit/3h dicen que el mercado está; lo que falta es instrumentación
(S5/S2) y cobertura (S8). El cerebro ya tiene qué optimizar — ahora lo puede VER.

---

# 10. ACTUALIZACIÓN 17:41Z→18:20Z — ERA-2 MADURA (4.98h): el embudo consolidó su forma definitiva

> **Segunda pasada del mismo WO (RESPAWN).** Motivo: §3.2 se capturó con 17 min de era-2 y
> §8.1 exigía ventana ≥1h post-boot. Ahora la era-2 tiene 4.98h (12:46:44Z→17:45:25Z).
> **SHA sin mover:** `e65040f1` verificado 17:41:38Z (BR-00-APPLY sigue sin aterrizar —
> coherente con BR-00-VERIFY §0 BLOCKED). Flota "Up 4 hours"; searcher `StartedAt=
> 13:20:06.373Z` (segunda recreación post-#555, +33.6 min tras el deploy 12:46:44Z;
> `RestartCount=0`, `OOMKilled=false`, `exit=0` — causa de la recreación UNKNOWN).
> Método idéntico: read-only total, HTTP público 0/5.

## 10.1 Cuota de muerte era-2 MADURA — snapshot 17:45Z (828,548 filas razonadas, 100% rejected)

Snapshot de la query de taxonomía (corre ~416 filas por encima del conteo de estatus
tomado segundos antes — deriva de tabla viva, §8.2). Ventana 12:46:44Z→17:45Z.

| Stage | Razón | Filas | % | vs 17-min (§3.2) |
|-------|-------|-------|---|------------------|
| **S5 oracle** | `v3_quote_unavailable` | 645,087 | **77.86%** | ↑ de 65.5% — dominancia CRECIÓ |
| **S3 engine** | `spot_product_le_one` | 95,000 | 11.47% | ↑ de 9.6% (todo `triangular`, honesto) |
| **S4 size** | `non_positive_profit` | 37,350 | 4.51% | ↑ de 3.2% |
| **S3 engine** | `single_pool_no_spread` | 32,919 | 3.97% | ↑ de 3.7% |
| **S7 spine** | `TokenNotAllowed` | 11,508 | 1.39% | ↓ de 8.9% — el spam MURIÓ de forma |
| **S4 size** | `missing_reserves_pool_b` | 5,248 | 0.63% | ↓ de 8.6% |
| **S6 gas** | `gas_floor_breach` | 1,112 | 0.13% | ↓ de 0.70% era-1 (solo cartuchos; 1,112 al snapshot 17:45Z y 1,680 a las 18:1xZ — el gate sigue activo; sufijo financing vacío en era-2) |
| **S4 size** | `missing_route_legs` | 278 | 0.03% | |
| **S5 oracle** | `no_price_oracle` + `unknown_token_price` | 31 + 4 | ~0% | |
| **S3 engine** | `spread_zero_equilibrium` | 11 | ~0% | |
| **Σ** | | **828,548** | **100.00%** | conservación exacta |

**Cuota por stage (era-2 madura):** **S5 77.87%** · S3 15.44% · S4 5.18% · S7 1.39% ·
S6 0.13%. **S9 0% por razón** (pero ver F-1: pérdida en el emitter, fuera de esta tabla).
El embudo invirtió de forma PERMANENTE: era-1 era S7 74.6%; era-2 es **S5 77.9%**.

**Cross-tab familia × razón (top, era-2):** `dex_arb|v3_quote` 660,008 ·
`triangular|spot_product_le_one` 97,199 · `dex_arb|non_positive_profit` 37,836 ·
`dex_arb|single_pool_no_spread` 34,095 · `mev_01|TokenNotAllowed` 8,736 ·
`mev_04|missing_reserves_pool_b` 5,318 · `mev_02|TokenNotAllowed` 2,912. El nativo
`dex_arb` es 86.3% del mix (715,062) y muere 92.3% en v3_quote (660,008, snapshot posterior).

**Perfil de la víctima S5 (los pares REALES):** WETH(`c02aaa…`)/USDT(`dac17f…`) con
`UniswapV3` presente en TODAS las combinaciones top (UniV3×PancakeV3 74,640; UniV2×UniV3
37,320; Sushi×UniV3 37,320; UniV3×UniV3 37,320), USDC(`a0b869…`)/USDT 36,7xx, USDC/WETH
34,850, `698250…`/WETH 31,236. **Solo 31 pares distintos** generan las 828K filas. No es
spam de tokens: es el universo blue-chip muriendo por falta de quote V3.

**Ritmo:** ~166K/h sostenido (buckets de 10 min: 24K-34K; rampa 12:50→13:40 de 7K→30K; sin
huecos). XADD confirmado por el contador de Redis: `entries-added` 546,976 (12:51Z) →
1,454,018 (17:54Z) = **+907,042 en 5.05h ≈ 49.9/s**. La era pre-#555 era ~2.5K/h:
**el throughput multiplicó ~66×**. Multihop 3+ pools: **100,655 (12.1%) — VIVO y masivo**
(vs "0 multihop en 7 días" del board para la era previa). Self-pair: 13.7% (era 97.5%).
`gas_floor_breach` SIN sufijo de financing (1,680/1,680 con sufijo vacío — ver S6 §1).

**CORRECCIÓN a una atribución tentativa (honestidad conmigo mismo):** el volumen NO es
`25/bloque × bloques/h` (eso da ~6.9K/h = ~4.2% del total; bloques ~13s, verificados
25927171→25927172 en 13s). `canonical_dispatch budget=25 dispatched=25`/bloque sigue vivo
pero es minoritario: **~95% del volumen es event-driven** (path S1→S3 por evento de
mempool/bloque, `v2.emitter.input`; INFERRED del mix dex_arb 86.3% + censo de eventos; el
split exacto por fuente queda UNKNOWN).

## 10.2 El mapa económico CERRADO: cero Topological Yield neto real computado en era-2 (R8)

1. **Las columnas `expected_profit_usd` / `net_expected_profit_usd` NO están en USD** —
   guardan unidades crudas sin normalizar por decimals (avg de filas "positivas" =
   1,607,017 unidades crudas ≈ dust en wei). **RULE 00 / colusión de nombres:** BR-10 y
   BR-11 NO deben renderizarlas como dólares sin dividir por 10^decimals del token.
2. **Cuota de "yield positivo" en era-2:** 42,491 filas con `expected_profit_usd > 0`, de
   las cuales 29,975 son `non_positive_profit` (contradicción aparente que resuelve el
   punto 1: el check del gate usa otra magnitud — net/gross real — y la columna cruda queda
   positiva en dust). Con `net_expected_profit_usd > 0`: **8,400 filas, TODAS
   `TokenNotAllowed`, valores 3-4 unidades crudas** (tokens de 0 decimals tipo XEN):
   económicamente $0. **Con umbral >2,143,721 unidades crudas: 0 filas.**
3. **Conclusión fail-honest (R8):** en era-2, NINGUNA oportunidad con Topological Yield
   neto real fue computada y luego matada por un gate aguas abajo (S6/S7 no destruyen nada
   de valor: S7 mata spam, S6 mata dust). El Yield se pierde en DOS puntos y solo dos:
   - **ANTES de computarse — S5 (77.9%):** el grafo señala 9,866 rutas `is_opportunity`
     /3h pero el QuoterV2 no puede valorarlas → `expected_profit_usd = NULL` para 645K
     filas. La Variedad de Liquidez está; el instrumento para medirla, no.
   - **Computado y honestamente ≤0 — S3/S4 (20.6%):** `spot_product_le_one` + `single_pool_no_spread`
     + `non_positive_profit` son matemática correcta diciendo "aquí no hay nada".
4. **Implicación para el board:** la frase "dónde se pierde el Topological Yield" tiene
   respuesta única hoy: **en S5, sin precio**. BR-03 (cascada de oráculos) no es un WO más
   del roadmap — es EL cuello del sistema post-#555.

## 10.3 Hallazgos nuevos F-1..F-7 (evidencia viva 17:41→18:20Z)

**F-1 (CRITICAL, S9 ya no está limpio) — el emitter PIERDE filas en PG.**
`opportunity_emitter.db_error` × **442 en 20 min**: `"PG insert failed; opportunity
published to Redis stream only"`, `error:"insert opportunity"` (la causa del insert NO
viaja en el log — cadena truncada, UNKNOWN). Ritmo: ~22/min vs `pg_period_inserted`
2,284/min (heartbeat 18:16Z) ≈ **~1% de los inserts caen** → PG SUBCONTA la emisión real;
Redis es el contador de verdad (F-6). Esto INVIERTE mi nota §1/S9 ("muere por AUSENCIA de
accepted, no por falla del emitter"): ahora también falla la persistencia, degrada
invariantes INV-BR01-1 (Σ PG ≠ emitido real) y exige reconciliación (INV-BR01-5, DESIGN).

**F-2 — sim-ctl también pierde persistencia.** `sim_consumer.persist_err` `"insert
simulation"` vivo a 18:17:54Z + `pel_observed pending=1 oldest_pending_ms=1,222,139` (PEL
de 20 min). Sims por cuarto de hora (15:15→18:15Z): 125-718/15min y A LA BAJA — el
consumer procesa ~0.3/s contra 50/s de emisión.

**F-3 (raíz operativa del S5) — hambre de RPC medida: Alchemy 429.**
`price_worker.alchemy_failed` × **783 en 20 min**: `HTTP 429 Too Many Requests` contra
`api.g.alchemy.com/prices/v1/…/tokens/by-address` (chunks 1-5), `"will try Coingecko
fallback"`. El heartbeat marca `price_alchemy_hits=0` en el período. La cuenta Alchemy
está rate-limited (la memoria del proyecto ya recomendó Free→PAYG al 80% del trigger).
Cadena exacta `429 → QuoterV2 staticcall fail` queda **INFERRED** (el staticcall es
eth_call del pool RPC, no la Prices API — pero comparten salud de cuenta/provider; N5
midió 5/9 providers vivos, 2 breakers FIRING). En la ventana retenida de logs NO aparece
ningún error del `v3_quote_provider` — la falla S5 es hoy SILENCIOSA en INFO.

**F-4 (RESUELVE §8.1) — el heartbeat V1 mide un path muerto.** Heartbeat 18:16Z:
`pending_received=0, decoded_ok=0, decoded_err=0`, TODOS los `gate_*=0`, `passed_all_gates=0`
— sostenido — mientras `pg_period_inserted=2,284/min` (¡38/s!) en el MISMO heartbeat.
Peor: `redis_stream_delta=2/min` contra XADD real ~3,000/min, porque lee el XLEN del
stream **MAXLEN-capped** (10,003) en vez de `entries-added` (1,454,018). **BR-10 NO puede
usar el heartbeat como fuente de verdad** — hay que rewirearlo (DESIGN D4).

**F-5 (R9 CRÍTICO — LOGFLOOD-01 reincidente).** `StartedAt=13:20:06Z` pero primera línea
retenida **17:44:13Z** → los 50MB (5×10m) retienen **SOLO ~16 min** a esta verbosidad
(`v2.emitter.input` por-evento en INFO × ~50/s, más `cartridge.active_eval_enter` 37,792/20min).
Toda conclusión de "ausencia" con ventana >16 min es artefacto de rotación. Aplica R9.3:
per-ítem a `debug!` + summary a `info!` — el propio CLAUDE.md §R9 lo exige.

**F-6 — inanición downstream estructural (todos los consumers trim-starved).** Lags a
17:54Z: enricher **18,022** · paper-archiver-g0 **18,086** · selector-g0 **9,922** ·
sim-ctl-g0 **9,996** — contra XLEN ~10,000. **lag ≥ XLEN ⇒ pérdida por trim estructural en
toda la cadena.** Sims era-2: **7,797** (0.94% de las opps; ~96% del flujo JAMÁS simulado;
la brecha opps→sims pasó de 37.8% a ~99%). El terminus intacto en su muerte: `XLEN
arbx:opps:simulated = 0`, `passed = 0`, `paper_trade_runs` congelada **2026-09-01
16:32:06Z** (6.1 días).

**F-7 — S8 era-2: el mix-shift de BR-00-VERIFY siguió su curso.** Sims era-2 por
simulador: `anvil` 4,708 (60.4%) vs `not_implemented` 3,093 (39.6%). Dentro del total:
`reverted` 3,204 (41.1%, #1 — probe revierte: TRANSFER_FROM/STF del signer sin
balance/approval), `strategy_not_simulatable_in_s4` 3,093 (37.4% — era 96.55% en 24h mixta
y 66.47% en la 2h de BR-00-VERIFY), `build_error` 692, `sim_timeout` 684, `rpc_error` 124,
`fork_acquire_failed` 4. **El gate de éxito BR-00 "<50%" está a ~12pp de cruzarse por
drift de mix SIN ningún fix** — el protocolo §1.4 de BR-00-VERIFY (era-separada + conteo
absoluto) es ahora obligatorio, no opcional.

**RDO 3h (14:41→17:41Z, 4,269,568 outcomes, ~100% shadow):** `is_opportunity` 9,866
(0.23%) · `NOT had_reserves` 2,411,316 (**56.5%** — estable vs 55.7% de la pasada) · top
razones: `missing_reserves` 15.12%, `funding_feed_unavailable` 11.52%, `bridge_state_unavailable`
11.52%, `nft_floor_feed_unavailable` 6.69%. BR-02 sigue vigente con las mismas cifras.
`route_scanner.done` por bloque: `cycles_found=500 (capped), dispatched=254-291,
shadow_forced=209-246, enumeration_ms=98-192, elapsed_ms=292-315` — el grafo NO es el
cuello de latencia (BR-08: mira el emitter I/O y los 429).

## 10.4 Delta de entregas a la mesa (sobre §9)

- **BR-00 (P0):** F-7 — tu métrica va a cruzar <50% sola por mix. Los `reverted` anvil
  (41.1% de sims era-2) son el próximo cuello que tu apply DEBE declarar (probe sin
  balance/approval del signer). Cita BR-00-VERIFY §2.4.
- **BR-02:** sin cambio material (56.5% had_reserves=f, 15.1% missing_reserves).
- **BR-03:** **eres EL cuello del sistema** — S5 77.87% + F-3 (429 Alchemy) + fail-silent
  del provider en INFO. La cascada debe incluir la salud de la cuenta RPC, no solo el
  catálogo de oráculos.
- **BR-04:** el flood MUTÓ: ya no es XEN/AGLD (S7 cayó a 1.39%); es ruido v3_quote de 31
  pares blue-chip a 50/s. El anti-spam por token NO lo toca — se necesita
  dedup/oracle-circuit/cooldown de reevaluación por par. La cifra que justificaba el
  reorder sizing→spine (74.6%) ya NO aplica en era-2 (S7 es 1.4%): el ahorro ahora está en
  NO emitir/persistir lo que murió sin precio (645K filas NULL-profit en PG + Redis + PG
  retención: ~4M filas/día proyectadas).
- **BR-08:** grafo 292-315ms/bloque SANO; tu p95 764ms vive en emitter I/O (F-1) y en la
  cola RPC (F-3), no en enumeración.
- **BR-10:** (a) fuente de verdad = vista D1 + `entries-added` de Redis (NO heartbeat,
  F-4); (b) NUNCA renderizar `expected_profit_usd` como USD (§10.2-1); (c) las dos eras
  con corte 12:46:44Z (INV-BR01-3) y ahora un SEGUNDO corte menor 13:20:06Z (recreación
  del searcher, sin cambio de SHA — no altera taxonomías, solo cadencia).
- **BR-11:** tus tarjetas multihop ya tienen masa (100,655 filas 3+ pools en 4.98h) — la
  desaparición con USD que reportas NO es falta de materia prima.

## 10.5 Fail-honest delta de esta pasada

1. **Causa raíz del insert failure (F-1/F-2):** UNKNOWN — el log trunca la cadena del
   error (`error:"insert opportunity"`); requiere reproducir con nivel de error completo o
   leer el código del path de insert (queda para el apply del diseño D5).
2. **429 → QuoterV2-staticcall-fail:** INFERRED (comparten cuenta/provider; no hay línea
   de error del quote provider en la ventana retenida — F-5 limita la observabilidad).
3. **Causa de la segunda recreación 13:20:06Z del searcher:** UNKNOWN (sin OOM, exit 0;
   presumiblemente compose recreate del deploy; BR-00-VERIFY la registró igual).
4. **Split exacto del volumen por fuente (canonical_dispatch vs event-driven):**
   canonical ~4.2% verificado; el resto event-driven INFERRED (mix dex_arb + census);
   split fino UNKNOWN.
5. **Ventana de logs ~16 min (F-5):** los conteos de eventos de §10.3 (442/783/37,792)
   corresponden a la ventana retenida 17:44→18:20Z declarada, no a la era completa.

**Veredicto BR-01 actualizado (era-2 madura):** el sistema post-#555 es un grafo sano que
señala Topological Yield (9,866 rutas/3h) y una capa de valuación que no puede medirlo
(S5 77.9% sin quote, hambre de RPC 429 en la cuenta), con un terminus intacto pero sin
alimento (96% del flujo jamás simulado; 0 passed en la historia) y un S9 que empieza a
perder filas (~1%). La optimización del cerebro tiene UN solo blanco económico hoy:
**S5**. Todo lo demás es honestidad matemática funcionando.
