# MATRIZ DE ACEPTACIÓN §15 — 264 identidades MEV, fees por protocolo, búsqueda autónoma, sizing y streaming

**Tarea T-E** · consolidación de los informes A (búsqueda §6/§7), B (sizing §6), C (fees §5) y D (streaming §11)
sobre la base CORRECTA `origin/main` = `9202561e` (2026-10-03, SHADOW-CANONICAL-01 #772).

| | |
|---|---|
| Worktree | `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\.worktrees\omegasearch-autonomous` |
| Rama | `fix/omegasearch-autonomous-01` |
| Base | `9202561e` (= `origin/main`) |
| HEAD al empezar T-E | `3fef1e5b` (commit de la tarea A: `OMEGA-SEARCH-01`) |
| Alcance de escritura | `docs/omegasearch/**` (único árbol tocado por esta tarea) |
| Naturaleza | **Documento de consolidación**: no cambia código, no activa capital, no firma, no transmite |

> **Regla de lectura.** Este documento separa tres cosas que no se pueden mezclar:
> (a) **cobertura** (código/tests presentes), (b) **verificación con datos reales** (evaluación productiva),
> (c) **rentabilidad** (dinero). Un `PASS`, un test verde o una fila de matriz **no** son ganancia.
> Lo que no se pudo medir se escribe **AUSENTE** con su razón, nunca como cero.

---

## 1. Método y disciplina de evidencia

1. **Toda cifra de este documento es una medición**, no una copia de un informe previo: se re-derivó en este
   worktree con los comandos de §10 y su salida se cita al lado.
2. **Premisas de la oleada 1 verificadas una por una** (§2). Una de ellas **no reprodujo** y se corrige aquí
   con la medición que la contradice (no se hereda un dato falso por venir de un informe).
3. **Estados separados** (§8) con criterio explícito por estado y conteo por identidad.
4. **Fail-honest**: las ausencias se declaran `AUSENTE` con la razón exacta y, cuando existe, el requisito
   externo que las desbloquearía (§12).

---

## 2. Verificación de las premisas de la oleada 1

| # | Premisa recibida | Veredicto T-E | Evidencia medida |
|---|---|---|---|
| P1 | `origin/main` = `9202561e` (2026-10-03 02:32:01), "SHADOW-CANONICAL-01 (#772)" | **CONFIRMADA** | `git log --oneline -12` en el worktree: `9202561e feat(cartridge): SHADOW-CANONICAL-01 … (#772)` inmediatamente bajo `3fef1e5b` |
| P2 | `aef131b2` (`RECEIPT-CONTRACT-01`, #762) ya está mergeado en `main` | **CONFIRMADA** | `9202561e` lo tiene como ancestro; aparece en el `git log` del worktree con `(#762)` |
| P3 | El árbol principal está en `fix/perhop-reserves-01` = `858b943f`, 110 commits por detrás, con 220 cambios sin commitear (fósil) | **NO RE-VERIFICADA en T-E** | T-E no tocó el árbol del operador (regla 4). Se acepta como hallazgo de la oleada 1 y se actúa en consecuencia: todos los worktrees de esta oleada parten de `9202561e` |
| P4 | 264 identidades `MEV_ID`, 11 familias con conteos 36/17/31/31/14/30/30/25/20/18/12 | **CONFIRMADA** | `docs/excel_strategies_extracted.json`: `strategies` = 264, `MEV_ID` distintos = 264, duplicados = 0; `Grupo` 1..11 con esos conteos exactos; `264 = 36+17+31+31+14+30+30+25+20+18+12` |
| P5 | 60 detectores distintos | **CONFIRMADA** | `Detector_ID` distintos en las 264 filas = **60**; `docs/excel_detectors_extracted.json` `detectors` = 60; `spec/detector_contracts.json` = 60 entradas |
| P6 | 31 operadores `1..31` | **CONFIRMADA** | `docs/excel_operators_extracted.json`: 33 filas = 1 encabezado + 31 ids + 1 "Interpretación". En `strategy_mapping.json` los 31 ids aparecen (`1..31`) |
| P7 | 8.184 relaciones 264×31, **aritméticas, NO materializadas** | **CONFIRMADA con matiz cuantitativo** | Grid = 264×31 = **8184** celdas. **Materializado**: 1716 pares no-cero (`docs/coverage_manifest.json` → `strategy_operator_links.count = 1716`, `expected = 8184`; y `artifacts/excel_coverage.md` línea 4: *"Matrix links (non-zero): 1716 of 8184 grid cells"*). Además `cartridges/strategy_mapping.json` declara `applicable_operators` (2810 referencias) y `operator_weights` (3960 entradas). **Ninguna de esas tres cifras es 8184** |
| P8 | 7 cartuchos raíz con **SHA256 idénticos 7/7** | **NO REPRODUCE — CORREGIDA** | Medido: `backend/searcher-rs/cartridges/*.rhai` son **7 archivos con 7 SHA256 distintos**. Ver §2.1 |
| P9 | 264 cartuchos de librería en `strategies/` | **CONFIRMADA** | `backend/searcher-rs/cartridges/strategies/`: 264 archivos `.rhai`, 264 `mev_id` distintos, 60 `detector_id` distintos |
| P10 | 16 ramas `fix/*` en vuelo; trabajar sobre `858b943f` está prohibido; los 11 archivos de zona caliente no se tocan | **RESPETADA** | T-E escribió **solo** `docs/omegasearch/**`. No se tocó ningún archivo de la lista caliente (§10.1) |

### 2.1 Corrección P8 — los 7 cartuchos raíz NO son idénticos

```
$ Get-ChildItem backend/searcher-rs/cartridges -File -Filter *.rhai |
    % { "{0,-38} {1,8}  {2}" -f $_.Name, $_.Length, (Get-FileHash $_.FullName -Algorithm SHA256).Hash }
backrun.rhai                    12050  F730DA7459AE129E00012D57A40605C50AFD39757884C908629035D03198DF72
dex_arb.rhai                    40109  2F55AB3B3C727D8A2A63E41568C0A9588BA6E56B51EA563FD910083B8C653BFF
funding_rate_arbitrage.rhai     14451  A6A7DA733D468FAE2619255DBB162EF9B13A4A606FAE2DFAF0AC61BFCA63EA0E
liquidation.rhai                10175  9067EE3A129FAFA66D371A67F1DA37147DB2678CD70B98F26E889F00D26EF019
mean_reversion_arbitrage.rhai   13082  B649D17FCC17D4076A5DFF5CF9EFCAB9EA2261F45CEA06380DB902687A6D5DA3
omega_strategy_pack.rhai        23757  402891C98536870EC566CE782AA575356D3A97EDB3A912F789DF27FD71527131
triangular_arb.rhai              9390  0D8FB05CD0D91D3BCD89DC8B49757B197805E952EE10DCE918A9A3B310B08CF9
```

Contraste con las copias del mismo nombre en el árbol de auditoría
(`audits/workspace-extreme-audit-2026-09-24/agent_pkg/ARBX_CARTUCHOS_AGENTE/reference_repo/backend/searcher-rs/cartridges`
y `integration/agent-cartridges-v4/reference_repo/backend/searcher-rs/cartridges`, idénticas entre sí):

* **3 de 7** copias byte-idénticas al árbol vivo: `backrun.rhai`, `omega_strategy_pack.rhai`, `triangular_arb.rhai`.
* **4 de 7 divergen**: `dex_arb.rhai`, `funding_rate_arbitrage.rhai`, `liquidation.rhai`, `mean_reversion_arbitrage.rhai`.
* Las dos carpetas `generated/cartridges` **no tienen ningún `.rhai` raíz** (solo `strategies/`).

**Lectura honesta**: la frase "7/7 idénticos" no describe el árbol vivo. Lo que sí es verificable es
*"3 de 7 raíces coinciden entre el árbol vivo y las copias de auditoría; 7 de 7 son distintas entre sí"*.
Ninguna decisión de esta matriz depende de P8, así que la corrección no altera ningún conteo de §8.

---

## 3. Censo canónico verificado (§2 del prompt)

### 3.1 Excel canónicos: existencia, hash y procedencia

Medidos en `C:\Users\HFRC\Downloads` (el directorio que declara `docs/excel_ingestion_manifest.json`):

| Libro (nombre citado por el prompt) | Existe | Bytes | SHA256 | Observación |
|---|---|---|---|---|
| `ArbitrageX_264_Cartridge_Math_Architecture.xlsx` | **SÍ** | 113 198 | `93E0807F5BAA9DC0…EA47C4` | `(1)` y `(2)` presentes y **byte-idénticos** entre sí y al base |
| `ArbitrageX_Master_264x31_LIVE_First.xlsx` | **SÍ** | 1 451 281 | `54FCDCA909886884…723ABA` | `(1)` presente e idéntico. **`(2)` AUSENTE** (el nombre `…LIVE_First (2).xlsx` no existe) |
| `ArbitrageX_SOP_264_Implementacion_Rentabilidad.xlsx` | **SÍ** | 853 675 | `866AAECE1E752573…FF9E0A` | mtime 2026-10-01; copia `(1)` idéntica |
| `ArbitrageX_SOP_264_Math_Software_GO_NOGO.xlsx` | **SÍ** | 523 242 | `E69EA9D552E66305…458121` | mtime 2026-10-01 |
| `ArbitrageX_Integrated_2_to_7_Hops_Flash_Atomic_Model.xlsx` | **SÍ** | 232 298 | `456B0329FE596AE9…5CB864` | copia `(1)` idéntica |
| `ArbitrageX_Route_Strategy_Optimizer_264_ULTRA.xlsx` | **SÍ** | 313 971 | `362BA8762EDEA602…ADC6D` | es el libro que consumió la ingesta canónica (coincide con `artifacts/strategy_registry.json → workbook.sha256`) |
| `ArbitrageX_Master_264x31_B_then_A.xlsx` | **SÍ** | 1 261 869 | `504BA65926BFB988…500E6` | citado en el manifiesto de ingesta |
| `ArbitrageX_Dynamic_QuoteBase_Route_Manual_264.xlsx` | **SÍ (dos variantes)** | 268 929 / 258 262 | `88EA467DA6F2B2BC…6C483` / `2A66E7449CE9C700…58C5FA` | **Discrepancia de fuente**: la copia base (2026-08-28) y las copias `(1)/(2)` (2026-08-23) **tienen hashes distintos**. La ingesta (`docs/coverage_manifest.json`) usó la copia base. Se documenta la precedencia; no se elige en silencio |

Manifiesto de ingesta (`docs/excel_ingestion_manifest.json`) — hojas y celdas por libro:
`ULTRA` 21 hojas, `MASTER_LIVE` 11, `MASTER_BA` 9, `CART_MATH` 6; `docs/coverage_manifest.json` añade
`QUOTEBASE` 17 hojas → **64 hojas, 577 785 celdas, 33 071 fórmulas** escaneadas.
Las hojas clave para el censo: `11_STRATEGY_CATALOG` (265 filas = 1 cabecera + 264), `13_STRAT_OP_MATRIX`
(269 filas), `07_MASTER_264x31` (**A1:AJ8185 → 8184 filas de datos**, 245 603 celdas no vacías, 24 552 fórmulas).

### 3.2 Emparejamiento workbook ↔ repositorio (igualdad de conjuntos)

| Conjunto | Cardinalidad | Resultado |
|---|---|---|
| `MEV_ID` workbook canónico | 264 | — |
| `MEV_ID` en `backend/searcher-rs/cartridges/strategies/*.rhai` | 264 | **igualdad exacta** (0 ausentes, 0 extra) |
| `MEV_ID` en `artifacts/strategy_registry.json` | 264 | **igualdad exacta**; `repo_verification = VERIFIED` en 264/264 |
| Nombres de estrategia distintos en el workbook | **263** | **1 nombre duplicado** entre dos `MEV_ID` distintos: la identidad se sostiene por `MEV_ID`, no por el nombre |
| `Detector_ID` distintos | 60 | en workbook y en cartuchos: **60 = 60** |
| Filas del workbook `11_STRATEGY_CATALOG` | 265 (+1 cabecera) | `docs/coverage_manifest.json` reporta `strategies.count = 267` (contando también `13_STRAT_OP_MATRIX`/otras hojas) → `coverage_pct = 101.1`. **No es un 264 falso**: es una suma de dos fuentes; el conjunto canónico es 264 |
| Relaciones 264×31 | 8184 celdas | **1716 materializadas** (§2 P7) |
| `cartridges/strategy_mapping.json` | 264 entradas, `total_strategies = 264` | 2810 referencias `applicable_operators` + 3960 pesos; 31 operadores distintos |

### 3.3 Requisitos de detector: 93 nombres, 1970 slots

Extraído de `fn detector_requirements()` de los 264 cartuchos (parse robusto, 264/264 parseados):

* **93 nombres distintos** de requisito.
* **1970 slots** (suma de requisitos declarados por cartucho).
* **8 identidades declaran lista VACÍA** — y son exactamente las 8 `OBSERVE_ONLY` / detector `OBSERVE`:
  `MEV-03-029`, `MEV-03-030`, `MEV-04-031`, `MEV-09-019`, `MEV-09-020`, `MEV-11-009`, `MEV-11-010`, `MEV-11-011`.
  Ausencia de contrato de requisitos **no es PASS**: su cierre se juzga con criterios de observación (§8.4).
* Requisitos más frecuentes: `native_risk_and_impact_policy` 256, `strategy_specific_note_verified` 256,
  `settlement_executable` 218, `protocol_exact_quotes` 68, `same_economic_asset` 46, `same_snapshot` 40,
  `closed_token_cycle` 38, `component_quotes_firm` 38, `conversion_contract_valid` 38, `delay_costed` 38,
  `token_continuity` 38, `finite_horizon` 35, `funding_schedule` 35, `hedged_position` 35, `margin_sufficient` 35.
* La lista completa de los 93 con su frecuencia y su verificador está en §7.

---

## 4. Matriz de aceptación por familia (§2 exige: archivo, detector, lógica económica, atomicidad, fuentes, adapters, operadores, restricciones, fees, búsqueda, sizing, encoder, ejecutor, tests, estado)

Detalle por identidad: **anexo `docs/omegasearch/ACCEPTANCE_MATRIX_264_ANNEX.csv`** (264 filas, 37 columnas),
generado desde `artifacts/strategy_registry.json` + mediciones de §3, §5, §6. Las primeras 20 identidades se
detallan en §4.2.

### 4.1 Agregado por familia (11 filas, con el denominador real)

| Familia | n | Módulo / Superficie | Detectores | Links | Reqs (slots) | Tests comportamiento | Live-shadow | Status workbook | Fees (§5) |
|---|---:|---|---:|---:|---:|---:|---:|---|---|
| **mev_01** Arbitrajes spot DEX misma cadena | 36 | `route_graph_engine` / `DEX_AMM` | 6 | 273 | 227 | 25 (piloto) + 11 solo-compila | 36 | ROUTE_READY 36 | V2/V3/V4+Curve+gas+builder+funding **con productor** |
| **mev_02** Arbitrajes según curva del AMM | 17 | `amm_curve_engine` / `DEX_AMM` | 13 | 94 | 118 | **0** (solo compila) | 16 | ROUTE_READY 16 · NO_COMPATIBLE_ROUTE 1 | V2/V3 con productor; stableswap/weighted/bins/PMM/bond/constant-sum **sin productor** |
| **mev_03** Disparados por tx/cambio de estado | 31 | `state_event_engine` / `DEX_STATE` | 6 | 198 | 186 | 31 | 27 | ROUTE_READY 27 · NO_COMPATIBLE_ROUTE 2 · OBSERVE_ONLY 2 | Swap V2/V3 con productor; oráculo/CEX **sin productor** |
| **mev_04** Equivalencia, paridad, redención | 31 | `parity_redemption_engine` / `PARITY_REDEMPTION` | 8 | 175 | 231 | 31 | 0 | NEEDS_ROUTE_DATA 30 · OBSERVE_ONLY 1 | Recibo `redemption_within_limits` **real**; `delay_costed` FAIL-honesto; mint/redeem fee **sin productor** |
| **mev_05** CEX–DEX y mercados externos | 14 | `cex_external_engine` / `CEX_DEX` | 3 | 110 | 102 | 14 | 0 | NEEDS_ROUTE_DATA 14 | **SIN PRODUCTOR** (tier/maker-taker/profundidad/lot size) |
| **mev_06** Cross-chain / cross-domain | 30 | `cross_domain_engine` / `CROSS_CHAIN` | 3 | 222 | 300 | 30 | 0 | NEEDS_ROUTE_DATA 30 | **SIN PRODUCTOR** (bridge/relayer/finalidad/desenlaces parciales) |
| **mev_07** Derivados y volatilidad | 30 | `derivatives_engine` / `DERIVATIVES` | 5 | 195 | 250 | 30 | 0 | NEEDS_ROUTE_DATA 30 | **SIN PRODUCTOR** (funding por horizonte, margen, cierre) |
| **mev_08** Lending, crédito, liquidaciones | 25 | `credit_liquidation_engine` / `LENDING` | 5 | 167 | 208 | 25 | 0 | NEEDS_ROUTE_DATA 25 | Funding flash **con productor** (Aave `FLASHLOAN_PREMIUM_TOTAL`); interés/close-factor/penalty **sin productor** |
| **mev_09** Intents, solvers, subastas | 20 | `intents_solver_engine` / `INTENT_AUCTION` | 5 | 123 | 131 | 20 | 0 | NEEDS_ROUTE_DATA 18 · OBSERVE_ONLY 2 | **SIN PRODUCTOR** |
| **mev_10** NFT, juegos, no fungibles | 18 | `nft_engine` / `NFT` | 5 | 101 | 135 | 18 | 0 | NEEDS_ROUTE_DATA 18 | **SIN PRODUCTOR** (royalties/marketplace) |
| **mev_11** Prediction markets | 12 | `prediction_engine` / `PREDICTION` | 5 | 58 | 82 | 12 | 0 | NEEDS_ROUTE_DATA 9 · OBSERVE_ONLY 3 | **SIN PRODUCTOR** |
| **TOTAL** | **264** | 11 módulos / 10 superficies | **60** | **1716** | **1970** | **236** | **79** | — | — |

Fuente de las columnas: `artifacts/strategy_registry.json` (identidad, estructura, operadores, math, estado),
`backend/searcher-rs/cartridges/strategies/*.rhai` (requisitos), tests de `backend/searcher-rs/tests/` (columna de tests),
PostgreSQL `route_discovery_outcomes_p20261002/p20261003` (columna live-shadow), §5 (columna de fees).

**Atomicidad y dependencias (medido sobre las 264):**

* `Atomic_Possible`: **Sí 144 · Condicional 72 · No 48**. Los 48 no atómicos se concentran en
  mev_06 (28), mev_05 (14), mev_03 (2), mev_04 (2), mev_08 (1), mev_10 (1).
* `Bridge_Dep = Sí` 33 · `Oracle_Dep = Sí` 14 · `External_Dep = Sí` 14 ·
  **unión de (external ∪ bridge ∪ oracle) = 60 identidades**.
* `Min_Legs/Max_Legs` declarados por identidad: 2–8 en mev_01/mev_03, y rangos propios por familia
  (columna `min_legs`/`max_legs` del anexo). El rango 2–7 del §6 del prompt **no** se impone aquí como regla
  universal: el anexo conserva el rango que la especificación vigente declara por identidad.
* Operadores: `primary_ops`/`secondary_ops` por identidad en el anexo (texto del workbook) y
  `matrix_links` = nº de pares no-cero de esa identidad en la matriz 264×31.

### 4.2 Detalle real de las primeras 20 identidades (mev_01 completo hasta 020)

| MEV_ID | Cartucho (archivo) | Detector | Exec class | Legs | Links | Reqs | Status | Test | Live-shadow | `IMPLEMENTED` | `TESTED` | `LIVE_DATA_VERIFIED` | `SETTLED`/`RECONCILED` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| MEV-01-001 | `mev_01_001_dex_dex_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí (0 opps) | sí | sí | parcial (evaluada, 0 opp) | no / no |
| MEV-01-002 | `mev_01_002_cross_pool_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-003 | `mev_01_003_cross_protocol_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-004 | `mev_01_004_cross_version_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-005 | `mev_01_005_cross_fork_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-006 | `mev_01_006_cross_fee_tier_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-007 | `mev_01_007_cross_liquidity_tier_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-008 | `mev_01_008_amm_amm_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-009 | `mev_01_009_amm_clob_arbitrage.rhai` | R_ORDERBOOK | EXTERNAL_DATA_REQUIRED | 2–8 | 7 | 7 | ROUTE_READY | **sólo compila** | sí | sí | parcial (compila) | parcial | no / no |
| MEV-01-010 | `mev_01_010_clob_clob_arbitrage.rhai` | R_ORDERBOOK | EXTERNAL_DATA_REQUIRED | 2–8 | 7 | 7 | ROUTE_READY | **sólo compila** | sí | sí | parcial (compila) | parcial | no / no |
| MEV-01-011 | `mev_01_011_amm_rfq_arbitrage.rhai` | R_ORDERBOOK | EXTERNAL_DATA_REQUIRED | 2–8 | 7 | 7 | ROUTE_READY | **sólo compila** | sí | sí | parcial (compila) | parcial | no / no |
| MEV-01-012 | `mev_01_012_rfq_rfq_arbitrage.rhai` | R_ORDERBOOK | EXTERNAL_DATA_REQUIRED | 2–8 | 7 | 7 | ROUTE_READY | **sólo compila** | sí | sí | parcial (compila) | parcial | no / no |
| MEV-01-013 | `mev_01_013_aggregator_dex_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-014 | `mev_01_014_router_pool_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–8 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-015 | `mev_01_015_two_leg_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 2–2 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-016 | `mev_01_016_triangular_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 3–3 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-017 | `mev_01_017_quadrangular_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 4–4 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-018 | `mev_01_018_n_leg_cyclic_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 3–16 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-019 | `mev_01_019_multi_hop_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 3–16 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |
| MEV-01-020 | `mev_01_020_multi_dex_cyclic_arbitrage.rhai` | R_CLOSED_CYCLE | DETERMINISTIC_EXECUTABLE | 3–16 | 8 | 6 | ROUTE_READY | piloto | sí | sí | sí | parcial | no / no |

**Columnas §2 y de dónde salen, para las 264:**

| Columna §2 | Fuente medida |
|---|---|
| archivo | `backend/searcher-rs/cartridges/strategies/mev_XX_YYY_*.rhai` (264/264 existen) |
| detector | `init_strategy().detector_id` (264/264 presentes; 60 distintos) |
| lógica económica | `Discovery_Equation` + `Detector_Math` por identidad (`artifacts/strategy_registry.json`) y `detector_contracts.json → logic` |
| atomicidad | `Atomicity_Type` / `NonAtomic_Type` / `Atomic_Possible` (anexo) |
| fuentes | `Source_1..3` por identidad + `Fuente 1..3` del detector (`spec/detector_contracts.json`) |
| adapters | `Backend_Module` + `Required_Surface` + `quote_semantics`/`implementation_boundary` del detector |
| operadores | `primary_ops`/`secondary_ops` + `matrix_links` (1716 totales) |
| restricciones | `Frontend_Config` + `Gate_LIVE` + los 1970 requisitos declarados |
| fees | §5 (productores de coste) con su ámbito por detector; **sólo V2/V3/V4/Curve/gas/builder/funding tienen productor** |
| búsqueda | §6.1 (crate `omega-search`, commit `3fef1e5b`) |
| sizing | §6.2 (`joint_sizing`, commit `d9154311`) |
| encoder | `rhai_agent_bridge` + `SnapshotServices::build_payload` (`snapshot_services.rs:1210`; sin payload canónico → `canonical_simulation_and_encoding_missing`) |
| ejecutor | `backend/relays-client/**` (`live_exec_policy.rs`, `submit_engine.rs`, `settlement_accounting.rs`); política default-deny (§8.6) |
| tests | §6.4 (236/264 comportamiento + 264/264 compilación) |
| estado | §8 |

---

## 5. Fees por protocolo, financiación y otros dominios (§5)

La resolución común de costes con **productores trazables** vive en el módulo nuevo de la tarea C
(rama `fix/omegasearch-fees-01`, commits `c8a06992` → `98871072`, 11 archivos, **+5850/−0**, 40 tests en
`cost_producers/tests.rs`), registrado desde `economics.rs` (no desde `lib.rs`, que es zona caliente).
Matriz completa y archivo:línea: `docs/verification/COST-PRODUCERS-01-matrix.md` (rama C) + informe C.

| Ámbito §5 | Productor real | Evidencia | Estado |
|---|---|---|---|
| V2 y forks — tarifa y denominador efectivos | `V2FeeTerms::from_read` (par num/den del deployment); `documented_terms` sólo contraste | `cost_producers/univ2.rs:66`, `:46` | IMPLEMENTED·TESTED |
| V2 — cantidades/redondeo | `univ2::amount_out` (floor, overflow, frontera de reserva) | `univ2.rs:196` | IMPLEMENTED·TESTED |
| V2 — comisión retenida | `univ2::fee_taken_raw` + `lp_fee_component` (`embedded`) | `univ2.rs:236`, `:251` | IMPLEMENTED·TESTED |
| V3 y concentrada | `V3FeeTerms::resolve` (fee tier real) | `univ3.rs:88` | IMPLEMENTED·TESTED |
| V3 — protocol fee | `univ3::protocol_fee_component` (corte de la comisión LP; cero sin cruce de tick) | `univ3.rs:186` | IMPLEMENTED·TESTED |
| V3 — ticks completos o quoter | `univ3::full_traversal_task` → **tarea** con recibo `full_tick_traversal_or_protocol_quoter` | `univ3.rs:238/242` | IMPLEMENTED (tarea de resolución)·TESTED |
| V4 — PoolKey, LP fee estática/dinámica | `PoolKey::static_lp_fee_pips` (bit 23), `V4FeeTerms::resolve` | `univ4.rs:70`, `:79`, `:137` | IMPLEMENTED·TESTED |
| V4 — protocol fee y hook fees | `protocol_fee_component` (carril uint8 ×100), `hook_fee_component`, `hook_rebate_component` (flujo con signo) | `univ4.rs:313`, `:333`, `:354` | IMPLEMENTED·TESTED |
| V4 — hook state | `univ4::hook_state_task` → **tarea** `dynamic_parameters_and_hook_state` | `univ4.rs:392` | IMPLEMENTED (tarea)·TESTED |
| Curve — invariante/versión, base de fee, reparto admin, fee variable, ramped A | `CurveInvariant::detect`, `scaled_fee_rate`, `fee_on_output`, `admin_split`, `cryptoswap_dynamic_fee`, `ramped_a` | `curve.rs:62`, `:97`, `:137`, `:181`, `:210`, `:241` | IMPLEMENTED·TESTED |
| Curve — oráculo y rama de inventario | `curve::oracle_task` → **tarea** `oracle_round_and_inventory_branch` | `curve.rs:325/326/330` | IMPLEMENTED (tarea)·TESTED |
| Gas — payload real, base/priority, L1/L2, blobs, receipt | `gas::intrinsic_gas`, `estimate_from_payload`, `effective_gas_price`, `L2DataFee`, `blob_fee`, `cost_from_receipt` + `GasReconciliation` | `gas.rs:113`, `:136`, `:161`, `:186/204`, `:271/339`, `:333/483` | IMPLEMENTED·TESTED |
| Builder/relay | `BuilderPaymentMode::treatment`, `bid_component`, `rebate_component` (flujo con signo) | `builder.rs:57`, `:113`, `:181` | IMPLEMENTED·TESTED |
| Financiación — Aave | `funding::resolve` + `aave_premium_raw` desde `FLASHLOAN_PREMIUM_TOTAL()` (**prohibido 5/9 bps de fixture**) | `funding.rs:182`, `:217` | IMPLEMENTED·TESTED |
| Financiación — semántica de método y capacidad | `FundingMethod::callback`, `premium_in_bps`, `premium_read`, `cheapest_admissible` (compara capital propio y proveedores) | `funding.rs:74`, `:96`, `:120`, `:473` | IMPLEMENTED·TESTED |
| Contrato común + auditoría de doble conteo | `CostResolution::to_cost_lines` (agrega por categoría) y `double_count_audit` (8 códigos) | `cost_producers.rs:811/961`, `:1130` | IMPLEMENTED·TESTED |
| **CEX/derivados** (tier, maker/taker, profundidad, lot size, funding por horizonte, margen) | — | — | **AUSENTE — sin productor en esta oleada** |
| **Cross-domain** (bridge, relayer, mensajería, retiro, demora, finalidad) | — | — | **AUSENTE — sin productor** |
| **Otros dominios** (mint/redeem, unwrap, vault, subasta, liquidación, marketplace, royalties) | — | — | **AUSENTE — sin productor** (las categorías `cex_trading_fee`, `bridge_cost`, `mint_redeem_fee` sí están declaradas en `bridge_kind` para que su productor futuro no colisione) |

**Tres dobles conteos ya bloqueados con test** (detalle en la matriz de C §2):
priority tip dentro de gas, pago al builder vía priority fee declarado `external`, y comisión de swap en un
`atomic_quote` (se declara `embedded`, no se resta otra vez).

**Lectura honesta de cobertura de fees**: de las 10 superficies del catálogo, **3** (DEX_AMM, DEX_STATE parcial,
LENDING sólo en su parte de financiación flash) tienen productor trazable. Las familias **mev_05 (14),
mev_06 (30), mev_07 (30), mev_09 (20), mev_10 (18), mev_11 (12) = 124 identidades** y las partes
mint/redeem, interés y penalización de **mev_04 (31)** y **mev_08 (25)** siguen **sin productor**. Eso es
cobertura de costes **incompleta**, y por §15 **no se cuenta como verificación**, ni se compensa con el hecho
de que los tests estén en verde.

---

## 6. Búsqueda, sizing y streaming: lo que la oleada 2 añadió (§6/§7/§11)

### 6.1 Búsqueda autónoma (§6/§7) — tarea A

* Crate nuevo y aislado `backend/omega-search/` (8 archivos, **4178 líneas**, cero dependencias externas),
  commit **`3fef1e5b`** en `fix/omegasearch-autonomous-01` (base `9202561e`).
* Cubre: multigrafo dirigido por (pool, dirección, versión) con allowlists/suelo de liquidez/máscara interior;
  enumeración acotada con presupuesto y **poda contada por regla nombrada** + `enumerate_exhaustive` (oráculo);
  cotización entera con floor y evaluación conjunta de splits; generaciones + invalidación de candidatos
  caducados; separación de máximo histórico vs mejor vigente.
* **Verificado por T-E ejecutando la suite en este worktree** (§10.3): **32 tests unitarios + 16 de integración
  = 48 pasan, 0 fallan, exit 0**, incluidos `bounded_engine_matches_exhaustive_oracle_on_a_small_graph`,
  `generation_turnover_invalidates_a_stale_positive_result`,
  `net_positive_below_execution_minimum_is_reported_but_not_authorized`,
  `all_candidates_negative_terminates_and_keeps_the_sign`, `break_even_is_a_credited_zero_not_a_loss_and_not_absence`.

### 6.2 Sizing (§6) — tarea B

* `fix/omegasearch-sizing-01`, commit **`d9154311`**, base `9202561e`; 3 archivos, +4809 líneas;
  motor en `backend/searcher-rs/src/size_optimizer/joint_sizing.rs` (2949 líneas) + `joint_sizing/tests.rs`
  (**37 tests**, 1847 líneas). Sólo se añade `pub mod joint_sizing;` a `size_optimizer.rs` (fuera de zona caliente).
* Dominio de tamaño con `Bound::{Known(value,source) | Absent(reason)}`: **rechaza el dominio entero si no hay
  cota superior conocida**; exploración multiescala + refinamiento de intervalos/fronteras/saltos de tick;
  redondeo a unidades mínimas y **re-cotización**; comparación de financiación.
* No reporta "óptimo global": reporta *mejor encontrado en el alcance evaluado*.

### 6.3 Streaming de card (§11) — tarea D

* `fix/omegasearch-stream-01`, commit **`727f1f3e`**, base `9202561e`; 16 archivos, +3961/−20.
* Corrige el defecto que el propio código documentaba (*"OUT-OF-ORDER: last ARRIVAL wins the CONTENT"*):
  una sola decisión (`stream-contract.decideUpsert`, `frontend/lib/store/stream-contract.ts`) ordena por el
  reloj de vigencia de la fila; una observación estrictamente más vieja se rechaza **y se cuenta**; una fila
  sin fecha se acepta declarando la falta de autoridad de orden.
* Contrato de eventos en `backend/api-server/src/stream/opportunity-stream.ts`
  (`schema_version, event_id, seq, strategy_key, plan_key, snapshot_id, emitted_at, kind, payload, progress`),
  con claves estables `chain|strategy_kind` y `estrategia#ruta@notional`.
* Estado de búsqueda visible y honesto en `frontend/lib/store/search-state.ts`:
  `ProgressCell` con `ABSENT_REASONS`, `planNet`, `selectBestPlans`, `lifecycleOf`, `deriveStrategySearchState`,
  `trackImprovement` (vigencia por defecto 5 min), `CappedInventory`.
* Tests nuevos medidos: `stream-contract.test.ts` 20 · `search-state.test.ts` 29 · `ws-ingest-buffer.test.ts` 11 ·
  `opportunity-stream.test.ts` 14 · `OpportunitySearchStatePanel.test.tsx` 14. **T-E no re-ejecutó la suite de
  Node** (vitest no estaba disponible en este worktree); los conteos son de lectura del archivo, no de ejecución
  → estado `TESTED (declarado por D)`, no re-verificado por T-E (ver §12 brecha B-7).

### 6.4 Tests por identidad de estrategia (medido en este worktree)

| Cobertura | Identidades | Evidencia |
|---|---:|---|
| Compilación con el engine de release (todos los `.rhai` de `strategies/`) | **264 / 264** | `tests/cartridge_e2e_test.rs:604 test_all_strategy_cartridges_compile` (asserta `checked >= 200` y `failures.is_empty()`) |
| Evaluación de comportamiento **por identidad** | **236 / 264** | wave B 62 (`cartridge_wave_b_test.rs`), wave C 74 (`cartridge_wave_c_test.rs`), wave DE 75 (`cartridge_wave_de_test.rs`), piloto 25 (`cartridge_r_closed_cycle_test.rs`, filtro `R_CLOSED_CYCLE`+`DETERMINISTIC_EXECUTABLE`) |
| Sólo compilación (sin test de comportamiento por identidad) | **28 / 264** | mev_01: 11 identidades · **mev_02: 17 identidades (familia entera)** |
| Evidencia de operadores (los 264 × ops declarados contra el registro de 31) | 264 | `tests/evidence_bench_31_test.rs::measured_evidence_cost_and_coverage_for_declared_ops` (asserta `declared_sets.len() == 264` y `compute_rate > 0.5`) |
| `#[test]` totales en `backend/searcher-rs/src/**` | 838 | conteo directo |
| Tests del verificador de requisitos | 4 | `snapshot_services.rs:1439`, `:1505`, `:1551`, `:1642` (incluye `broken_leg_yields_fail_receipt_not_fabricated_pass`) |

**No** hay 264 PASS ejecutando 264 veces el mismo test de metadata: hay 236 identidades con test de
comportamiento propio y 264 con compilación real. Y **28 identidades, incluida toda la familia mev_02
(AMM curve), no tienen test de comportamiento por identidad** — se declara como brecha, no como verde.

---

## 7. Mapa de requisitos y verificadores (§9)

### 7.1 Universo de requisitos (tres conjuntos distintos, que NO se pueden mezclar)

| Universo | Cardinalidad | Dónde vive | Qué mide |
|---|---:|---|---|
| **R1 — requisitos de detector** | **93 nombres / 1970 slots** | `fn detector_requirements()` de los 264 cartuchos; contrato en `integration/agent-cartridges-v4/spec/detector_contracts.json` (`required_checks` de 60 detectores = 93 nombres) | la condición semántica que cada detector exige |
| R2 — requisitos de workbook (celdas) | 441 | `artifacts/excel_requirements.json` (17 257 líneas) + `artifacts/excel_coverage.md` | anclaje hoja/celda→repo; **441/441 VERIFIED** según la ingesta canónica de la oleada 1 |
| R3 — recibos de plan | 27 verificadores reales | `backend/searcher-rs/src/snapshot_services.rs` | lo que hoy puede emitirse con evidencia |

R2 **no** es cobertura de R1: los 15 nombres que el prompt §9 enumera aparecen **0 veces** en
`artifacts/excel_requirements.json`. Confundirlos sería exactamente el "falso cierre" que §3 prohíbe.

### 7.2 Verificadores reales hoy (27), todos en `snapshot_services.rs`

`closed_token_cycle`, `token_continuity`, `same_snapshot`, `protocol_exact_quotes`,
`strategy_specific_note_verified`, `settlement_executable`, `post_state_bound`, `confirmed_transition`,
`same_economic_asset`, `balance_conservation`, `matched_quantity`, `identical_input`, `identical_output_asset`,
`nonnegative_allocations`, `input_allocation_conserved`, `firm_depth`, `inventory_available`,
`component_quotes_firm`, `firm_baseline`, `firm_unsplit_baseline`, `shared_pool_state_consistent`,
`conversion_contract_valid`, `firm_unwind`, `execution_before_quote_expiry`, `redemption_within_limits`,
`delay_costed` (**FAIL-honesto estructural**), `native_risk_and_impact_policy`.

* `receipt(...)` con nombre literal en el código: **27 nombres** (`snapshot_services.rs`, líneas 483–808).
* De los **93** nombres de R1, **28 aparecen en fuentes Rust** de `backend/searcher-rs/src`; de esos 28,
  **27 son verificadores** `receipt(...)` y el restante (`finality_model`) aparece sólo como referencia, **no
  como verificador**. Los **65 restantes no aparecen en absoluto**.
* Comportamiento declarado y medido para un nombre sin verificador (`verify_requirements`,
  `snapshot_services.rs:1161-1209`): se emite **exactamente un recibo** con `status = "FAIL"` y
  `reason = "no_verifier_at_discovery_layer_for:<nombre>"`, jamás un PASS fabricado. Es decir: ausencia de
  verificador **es observable y se distingue** de fallo de condición (R8/§9).
* Los 8 `OBSERVE_ONLY` no declaran requisitos, así que ni siquiera entran en este bucle: su cierre es de observación.

### 7.3 Los 15 nombres que el prompt enumera — estado uno por uno

| # | Requisito | Detector(es) que lo exigen | Slots | Verificador real | Estado |
|---|---|---|---:|---|---|
| 1 | `full_tick_traversal_or_protocol_quoter` | `CF_CLAMM` | 1 | **No** en Rust. Tarea de resolución con ese nombre de recibo: `cost_producers/univ3.rs:238/242` (rama C) | **parcial**: productor de la tarea (declara `factory.getPool` + `QuoterV2`), sin verificador que emita PASS |
| 2 | `exact_invariant_version_and_rates` | `CF_STABLESWAP` | 1 | **No** en Rust. Declaración con nombre en `cost_producers.rs:799-800` + `curve.rs:118 version_task` | **parcial** |
| 3 | `normalized_weights_and_scaling` | `CF_WEIGHTED` | 1 | **NO EXISTE** ni verificador ni productor declarado | **AUSENTE** |
| 4 | `all_crossed_bins_and_variable_fees` | `CF_LB` | 1 | **NO EXISTE** | **AUSENTE** |
| 5 | `oracle_round_and_inventory_branch` | `CF_PMM` | 1 | **No** en Rust. `curve.rs:325/326/330 oracle_task` con ese nombre de recibo | **parcial** |
| 6 | `dynamic_parameters_and_hook_state` | `CF_DYNAMIC` | 4 | **No** en Rust. `cost_producers/univ4.rs:25/392 hook_state_task` | **parcial** |
| 7 | `exact_integrated_cost_curve` | `CF_BOND` | 1 | **No** en Rust. `cost_producers/univ2.rs:363` (`receipt: Some(...)`) | **parcial** |
| 8 | `reserve_boundary_respected` | `CF_CONSTANT_SUM` | 1 | **No** en Rust. `cost_producers.rs:795` | **parcial** |
| 9 | `confirmed_transition` | `CF_TWAMM`, `E_LATENCY`, `E_ORACLE`, `E_POST`, `E_STATE` | 30 | **SÍ** — `snapshot_services.rs` (recibo con condición `same_snapshot && !q.legs.is_empty()`) | **REAL** |
| 10 | `post_state_bound` | idem | 30 | **SÍ** — exige `amount_out_raw` no vacío y `snapshot_id` por pierna | **REAL** |
| 11 | `settlement_executable` | 48 detectores | 218 | **SÍ** — `closed && continuity && same_snapshot && !legs.is_empty()` | **REAL** (viabilidad de settlement, **no** settlement realizado) |
| 12 | `virtual_orders_advanced_to_snapshot` | `CF_TWAMM` | 1 | **NO EXISTE** | **AUSENTE** |
| 13 | `oracle_round_verified` | `E_ORACLE` | 2 | **NO EXISTE** | **AUSENTE** |
| 14 | `independent_timestamps` | `E_LATENCY` | 6 | **No** en Rust. `cost_producers.rs:775` (nombre de recibo en la declaración de coste) | **parcial** |
| 15 | `execution_before_quote_expiry` | `E_LATENCY` | 6 | **SÍ** — `self.data.valid_until_ms > now_ms()` | **REAL** |

**Resumen exacto de los 15**: **4 reales** (#9, #10, #11, #15) · **7 parciales** (nombre declarado como tarea
de resolución en el módulo de costes de C, sin verificador que pruebe la condición) · **4 ausentes**
(#3, #4, #12, #13).

> La relación de §9 **no autoriza a omitir los nombres no enumerados**: los 93 están en el anexo
> (columna `requirements` por identidad) y su frecuencia se midió completa. Los 65 sin verificador
> reciben en runtime el recibo FAIL con `no_verifier_at_discovery_layer_for:<nombre>`.

---

## 8. Estados separados (§15) — por identidad y por familia

### 8.1 Definición operativa del criterio (para que el conteo sea reproducible)

| Estado | Criterio usado | Fuente del veredicto |
|---|---|---|
| `SPECIFIED` | identidad + detector + ecuación + procedencia libro/hoja/celda | `artifacts/strategy_registry.json`, `docs/excel_*.json` |
| `IMPLEMENTED` | artefacto ejecutable presente y admitido: `.rhai` + canon `STRATEGY.json` + digest de manifiesto + compila | 264/264 de ambos; `test_all_strategy_cartridges_compile` |
| `TESTED` | (a) compilación con el engine de release; (b) test de comportamiento propio por identidad | §6.4 |
| `FORK_VERIFIED` | ejecución del camino real contra estado de fork, **con artefacto registrado** por identidad | §8.5 → **0 evidenciado** |
| `LIVE_DATA_VERIFIED` | evaluación contra datos productivos con resultado registrado **por identidad** | `route_discovery_outcomes_*` (§8.5) |
| `EXECUTION_AUTHORIZED` | permiso operativo vigente para comprometer fondos en esa identidad | §8.6 → **0** |
| `SETTLED` | liquidación on-chain real | `executions` = 0 filas |
| `RECONCILED` | beneficio conciliado desde balances/fills reales | `profit_reconciliation` = 0 filas |
| `BLOCKED_EXTERNAL` | dependencia externa exacta que impide cerrar; **no cuenta como verificación** | §12 |

### 8.2 Conteo por estado (denominador: 264 identidades)

| Estado | Cumplen | Detalle |
|---|---:|---|
| `SPECIFIED` | **264 / 264** | 264 `MEV_ID` distintos, 0 duplicados; `repo_verification = VERIFIED` 264/264; cartridge y canon existen 264/264 |
| `IMPLEMENTED` | **264 / 264** | 264 `.rhai` + 264 `skills/arbitragex-ultra/strategies/MEV-XX-YYY/STRATEGY.json` presentes; 60 detectores; 1970 requisitos declarados |
| `TESTED` (compilación) | **264 / 264** | `cartridge_e2e_test.rs:604` |
| `TESTED` (comportamiento por identidad) | **236 / 264** | 211 wave B/C/DE + 25 piloto. **28 sin test propio** (mev_01 ×11, mev_02 ×17) |
| `FORK_VERIFIED` | **0 / 264 evidenciado** | `multistep_fork.rs:149-150` es `#[ignore]` y requiere `RPC_HTTP_1` + `EXECUTOR_1` + nodo archivo; `sim-fork-evidence.yml` es `workflow_dispatch`; **sin artefactos de fork registrados** en el repo |
| `LIVE_DATA_VERIFIED` (evaluación shadow) | **79 / 264** | 20 605 616 evaluaciones en `route_discovery_outcomes_p20261002` + `p20261003`, 85 `cartridge_id` distintos de los que **79 son `mev_*`**; **`is_opportunity = 0` en las dos particiones**, `max(estimated_profit) = 0` |
| `LIVE_DATA_VERIFIED` (con oportunidad) | **0 / 264** | 0 filas con `is_opportunity = true` |
| `EXECUTION_AUTHORIZED` | **0 / 264** | `live_exec_policy.rs:3` `DEFAULT_LIVE_CHAINS = [11155111]`, `:19 from_env`, `:41 assert_broadcast_allowed`; sin `ARBX_LIVE_EXEC_ENABLED=true` verificado en el entorno productivo; la autorización de gasto es acto del operador (§34.3/§34.5) |
| `SETTLED` | **0** | `SELECT count(*) FROM executions` = **0** |
| `RECONCILED` | **0** | `SELECT count(*) FROM profit_reconciliation` = **0**; `paper_trade_runs` = 0; `simulations` = 0 |
| `BLOCKED_EXTERNAL` (identidades con dependencia externa declarada) | **60** | unión `External_Dep ∪ Bridge_Dep ∪ Oracle_Dep` = 60; de ellas mev_05+mev_06+mev_07 = 74 identidades de venue externo (solape parcial con la unión); **48 no atómicas** |

### 8.3 Advertencia de no-mezcla (§15, textual)

* **Cobertura ≠ rentabilidad.** `TESTED` = 236/264 **no** implica que alguna identidad gane dinero:
  el mejor neto cotizado observado es **USD 0.0538** y el realizado es **USD 0** (§9).
* **`BLOCKED_EXTERNAL` no es verificación completa**: las 60 identidades con dependencia externa
  **no** se cuentan en `TESTED`, `FORK_VERIFIED` ni `LIVE_DATA_VERIFIED` como cierre.
* Una identidad puede estar `IMPLEMENTED` y `TESTED` y aun así tener **cero** verificadores para todos sus
  requisitos (p. ej. toda la familia mev_02, cuyos requisitos `*_invariant/weights/bins` están AUSENTES en §7.3).

### 8.4 Las 8 identidades `observe_only` (criterio de cierre propio, sin settlement inventado)

`MEV-03-029`, `MEV-03-030`, `MEV-04-031`, `MEV-09-019`, `MEV-09-020`, `MEV-11-009`, `MEV-11-010`, `MEV-11-011`
(detector `OBSERVE`, `execution_class = OBSERVE_ONLY`, `status_excel = OBSERVE_ONLY`).

* Declaran **lista de requisitos VACÍA** (0/1970 slots) → no hay contrato de recibos que cumplir.
* Criterio de cierre aplicado: (a) cartucho presente y compilable (264/264 incluye estas 8);
  (b) su salida es una señal, **no** una ganancia; (c) `monetizar` la señal exigiría especificar y demostrar una
  operación ejecutable vinculada — **no existe** en esta oleada.
* Por tanto: `SPECIFIED` sí, `IMPLEMENTED` sí, `TESTED` sí (comportamiento: 6 de las 8 en wave C/DE;
  MEV-03-029/030 en wave B), `LIVE_DATA_VERIFIED` parcial (MEV-03-029/030 aparecen en el set de 85 del shadow),
  y **`SETTLED`/`RECONCILED` = NO APLICA** (no se inventa settlement para observación).

### 8.5 `FORK_VERIFIED` y `LIVE_DATA_VERIFIED`: evidencia exacta

* **Fork**: `backend/searcher-rs/tests/multistep_fork.rs:149-150`
  `#[tokio::test] #[ignore = "requires RPC_HTTP_1 + EXECUTOR_1 + archive node…"]`
  `multistep_fork_round_trip_weth_usdc`; lectura de `RPC_HTTP_1/EXECUTOR_1/FLASHLOAN_EXECUTOR_1` en `:80-83`.
  `.github/workflows/sim-fork-evidence.yml` es `workflow_dispatch` (manual) y exige el marcador
  `FORK_SUITE_OUTCOME=PASS`. **No hay artefacto de fork registrado** en este worktree.
* **Live-data (shadow)**: PostgreSQL productivo, `route_discovery_outcomes_p20261002`
  (13 412 755 filas, ts 2026-10-02T00:00:00Z → 23:59:53Z) y `p20261003`
  (7 192 861 filas, 2026-10-03T00:00:02Z → 09:35:28Z): **85 `cartridge_id` distintos** en la unión,
  `mode = shadow`, **0 filas `is_opportunity = true`**, `max(estimated_profit) = 0`.
  Motivos dominantes (top): `applicable_data_or_constraint_gap` 4 932 314 ·
  `native_domain_solver_required` 1 253 051 · `search_completed_within_declared_scope` 534 491 ·
  `insufficient_price_history` 87 367 · `not_confirmed_zero_victim_guard` 87 364 ·
  `no_funding_rates_data` 87 357 · `native_size_schedule_missing` 44 388.
* **Métrica honesta**: 79 identidades tienen **evaluación productiva**; **ninguna** tiene oportunidad.
  Llamar "verificado" a eso sería el falso cierre que §3 prohíbe; por eso la etiqueta es
  `LIVE_DATA_VERIFIED = parcial: evaluada en shadow, 0 oportunidades`.

### 8.6 `EXECUTION_AUTHORIZED`: dónde vive el interruptor y qué falta

`backend/relays-client/src/live_exec_policy.rs` (único binario que puede firmar/broadcast):

```
  3: pub const DEFAULT_LIVE_CHAINS: &[u64] = &[11_155_111];
 19: pub fn from_env() -> Self {   // ARBX_LIVE_EXEC_ENABLED / ARBX_LIVE_EXEC_CHAINS
 41: pub fn assert_broadcast_allowed(&self, chain_id: u64) -> Result<(), LiveExecDenied>
 59: fn defaults_are_disabled()
 71: fn explicit_mainnet_is_supported()
 78: fn invalid_allowlist_never_partially_activates()
```

Sin `ARBX_LIVE_EXEC_ENABLED=true` el deny es total. T-E **no** verificó el valor de esa variable en el VPS
(no se dispone de esa lectura en este worktree) → se declara **AUSENTE**, no "false". Además, aun con el
switch técnico en verde, la autorización de comprometer capital es decisión del operador (§34.3/§34.5) y
**no** se infiere de flags.

---

## 9. Contabilidad honesta del mejor neto vigente observado (§15, textual)

Medido contra PostgreSQL productivo (`SELECT` de sólo lectura) el 2026-10-03 en la ventana de 6 h
(`detected_at > now() - interval '6 hours'`, cierre de datos 2026-10-03 09:32:22Z):

| Métrica | Valor medido |
|---|---|
| Filas en la ventana | 771 487 |
| Estado de esas filas | **`rejected` 771 487** (0 en cualquier otro estado) |
| Con `net_expected_profit_usd > 0` | **2** |
| **Mejor neto cotizado vigente** | **USD 0.053802** (`dex_arb`, `pair bbbbca…/c02aaa…`, `status = rejected`) |
| Mejor bruto de la ventana | USD 0.73486039 |
| Mejor neto histórico en la tabla completa (9 341 338 filas, desde 2026-09-30) | **USD 1.927164** (363 filas positivas = **0.0039 %**) |
| **Beneficio realizado / conciliado** | **USD 0 — sin datos**: `executions` = 0 filas, `profit_reconciliation` = 0 filas, `paper_trade_runs` = 0, `simulations` = 0 |
| Mínimo de ejecución vigente (chain 1) | `trading_config.min_profit_usd = 50.0000`, `enabled = t`, 8 `enabled_strategies` |
| Otras cadenas | chain 42161/56/10/8453/137 con `enabled = f` |

**Lectura sin adornos**: no hay beneficio real que reportar. El mejor número **cotizado** vigente
(USD 0.0538) es **1 000 veces menor** que el mínimo de ejecución vigente (USD 50), está en estado
`rejected`, y **no existe** ninguna liquidación ni conciliación. No se sustituye por proyecciones, por
"neto esperado" de fixtures ni por el caso de la captura del usuario (≈ −7,91 USD frente a objetivo 50 USD).

Motivos de rechazo en la misma ventana (los cinco primeros son el 99.9 % de la población):
`spread_zero_equilibrium` 521 029 · `v3_quote_unavailable` 104 559 · `non_positive_profit` 101 410 ·
`single_pool_no_spread` 38 844 · `no_tradable_size` 3 853 · `v3_multileg_budget_exhausted` 1 877 ·
`no_price_oracle` 50 · `v3_pair_no_pools` 15 · `anomalous_math` 6 · `TokenNotAllowed` 3.

---

## 10. Evidencia reproducible (comandos, salida, exit code, tiempos, denominadores)

Todos los comandos se ejecutaron en
`C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\.worktrees\omegasearch-autonomous` (salvo los marcados como VPS/SQL).

### 10.1 Alcance de escritura y zona caliente

```
# Gate de rutas de la skill cordis-plugin-safe-workflow, ANTES de escribir:
& "<preset>\arbx-arb\skills\cordis-plugin-safe-workflow\scripts\assert-safe-write-scope.ps1" `
  -Workspace "<worktree>" -Paths "…\docs\omegasearch\ACCEPTANCE_MATRIX_264.md","…\ACCEPTANCE_MATRIX_264_ANNEX.csv"
CORDIS_SAFE_WRITE_SCOPE=OK
GATE EXIT CODE = 0
```

Archivos modificados por T-E: `git status --porcelain` → sólo `docs/omegasearch/…` (2 archivos).
Ninguno de los 11 archivos de la zona caliente fue tocado (`git diff --name-only` no los incluye).

### 10.2 Censo y conjunto

```
$ (Get-Content docs\excel_strategies_extracted.json -Raw | ConvertFrom-Json).strategies.Count      → 264
$ distinct MEV_ID                                                                                  → 264   (duplicados 0)
$ por Grupo                                                                                        → 36,17,31,31,14,30,30,25,20,18,12 = 264
$ distinct Detector_ID                                                                             → 60
$ docs\excel_detectors_extracted.json .detectors.Count                                             → 60
$ spec\detector_contracts.json entradas                                                            → 60 ; required_checks distintos → 93
$ cartridges\strategies\*.rhai                                                                     → 264 (264 mev_id, 60 detector_id, 0 sin mev_id)
$ cartridges\*.rhai (raíz)                                                                         → 7 con 7 SHA256 distintos
$ requisitos: fn detector_requirements() parseado en 264/264 cartuchos                             → 93 nombres, 1970 slots, 8 listas vacías
$ strategy_mapping.json: total_strategies                                                          → 264 ; applicable_operators 2810 ; operator_weights 3960 ; 31 ops
$ docs\coverage_manifest.json: strategy_operator_links                                             → 1716 de 8184
$ artifacts\strategy_registry.json: repo_verification=VERIFIED                                     → 264/264 ; cannon JSON existente 264/264 ; cartridge existente 264/264
```

### 10.3 Tests ejecutados realmente por T-E

```
$ cd backend\omega-search ; cargo test --offline
   Unit:        32 passed; 0 failed; 0 ignored
   engine_contract: 16 passed; 0 failed; 0 ignored
   Doc-tests:    0 passed; 0 failed
   → exit code 0
```

Éste es el **único** suite que T-E ejecutó (el crate `omega-search` no tiene dependencias externas y compila
en este worktree). Conteos de tests de B/C/D y del árbol `searcher-rs` son **lectura de archivo**
(nº de `#[test]`/`it(`), no ejecución: se declaran como tales (§12, brecha B-7).

### 10.4 Estado productivo (PostgreSQL, sólo lectura)

```
SELECT count(*), max(detected_at) FROM opportunities;                    → 9337377 | 2026-10-03 09:32:22.185005+00
SELECT status, count(*), count(*) FILTER (WHERE net_expected_profit_usd>0), max(net_expected_profit_usd)
  FROM opportunities WHERE detected_at > now() - interval '6 hours' GROUP BY status;
                                                                          → rejected | 771487 | 2 | 0.053802
SELECT max(net_expected_profit_usd), count(*) FILTER (WHERE net_expected_profit_usd>0), count(*) FROM opportunities;
                                                                          → 1.927164 | 363 | 9341338
SELECT count(*) FROM executions;                                          → 0
SELECT count(*) FROM profit_reconciliation;                               → 0
SELECT count(*) FROM paper_trade_runs;                                    → 0
SELECT count(*) FROM simulations;                                         → 0
SELECT count(*) FROM cartridge_control;                                   → 0
SELECT count(*) FROM cartridge_registry;                                  → 4   (todas con file_path "-- loaded from filesystem at boot --", status pending_boot_load)
SELECT count(*) FROM rpc_endpoints;                                       → 11
SELECT count(*), count(DISTINCT cartridge_id), count(*) FILTER (WHERE is_opportunity), max(estimated_profit)
  FROM route_discovery_outcomes_p20261002;                                → 13412755 | 85 | 0 | 0
  … p20261003                                                            → 7192861  | 85 | 0 | 0
  unión de ambas: 85 cartridge_id, de los que 79 son mev_*
```

**Nota de lectura (R10/E2E):** `cartridge_registry` con 4 filas y `cartridge_control` con 0 filas
**no** significa "0 cartuchos": el cargador real es el sistema de archivos en boot
(`cartridge_boot.rs:304 let loaded = results.iter().filter(|r| r.success).count();`), y el registro en BD
declara literalmente `-- loaded from filesystem at boot --`. Contar la tabla como inventario sería un falso
cero por ausencia de productor.

---

## 11. Registro de trabajo (permite continuar sin repetir auditorías)

| ID | Tarea | Rama | Commit (SHA) | Archivos | Estado | Qué falta |
|---|---|---|---|---|---|---|
| **A** | Motor de búsqueda autónoma (§6/§7) | `fix/omegasearch-autonomous-01` | **`3fef1e5b`** | `backend/omega-search/**` (8 archivos, +4178) | IMPLEMENTED · TESTED (48 tests, re-ejecutados por T-E, exit 0) | integración con el pipeline real (hoy es crate aislado); cotización V3/Curve/bins reales en `quote`; reparto justo conectado al scheduler |
| **B** | Joint sizing (§6) | `fix/omegasearch-sizing-01` | **`d9154311`** | `size_optimizer.rs` (+13), `size_optimizer/joint_sizing.rs` (+2949), `joint_sizing/tests.rs` (+1847) | IMPLEMENTED · TESTED (37 tests declarados) | verificación de compilación/tests en árbol con `target/` caliente; wiring al orchestrator |
| **C** | Productores de coste/fee (§5) | `fix/omegasearch-fees-01` | **`c8a06992`** → **`98871072`** | 11 archivos, +5850/−0; matriz en `docs/verification/COST-PRODUCERS-01-matrix.md` | IMPLEMENTED · TESTED (40 tests declarados) | CEX/derivados, cross-domain y otros dominios (**124+ identidades** con coste sin productor); conectar `to_cost_lines` al bridge |
| **D** | Streaming/secuenciación de card (§11) | `fix/omegasearch-stream-01` | **`727f1f3e`** | 16 archivos, +3961/−20; contrato en `frontend/lib/store/stream-contract.ts` y `search-state.ts` | IMPLEMENTED · TESTED (88 casos declarados en 5 archivos) | re-ejecutar vitest fuera de este worktree; reconexión/resync E2E contra el API server |
| **E** | **Matriz de aceptación y consolidación (§15)** | `fix/omegasearch-autonomous-01` | **este commit** (ver §13) | `docs/omegasearch/ACCEPTANCE_MATRIX_264.md` + `ACCEPTANCE_MATRIX_264_ANNEX.csv` | documento + anexo 264 filas | §12 |

**Hallazgo de árbol**: el anexo `strategy_registry.json` + `docs/excel_*.json` + `docs/coverage_manifest.json`
**ya viven en `origin/main`** (commit `659656b5`, XLS-CANON-01). La oleada 2 **no** repitió la ingesta: la
**midió y la usó como fuente primaria**, dejando constancia de hashes y de la discrepancia del QuoteBase.

---

## 12. Brechas restantes y requisito externo exacto de cada bloqueo

| ID | Brecha | Alcance | Requisito externo exacto |
|---|---|---|---|
| **B-1** | 65 de los 93 requisitos de detector **sin verificador** en la capa de descubrimiento | 264 identidades (todas declaran ≥1 de los 65) | no es externo: es implementación pendiente por nombre (los 4 AUSENTES de §7.3 primero: `normalized_weights_and_scaling`, `all_crossed_bins_and_variable_fees`, `virtual_orders_advanced_to_snapshot`, `oracle_round_verified`) |
| **B-2** | Fees sin productor para CEX/derivados, cross-domain, mint/redeem, vault, subasta, marketplace, royalties | 124 identidades (mev_05/06/07/09/10/11) + partes de mev_04/mev_08 | no es externo en su mayoría: falta el productor. Para **CEX** el requisito es la **credencial de API de la cuenta y su tier real** (maker/taker, profundidad, lot/tick) |
| **B-3** | `FORK_VERIFIED` = 0 evidenciado | 264 | `RPC_HTTP_1` (nodo **archivo**, URL única cruda) + `EXECUTOR_1` (+ `FLASHLOAN_EXECUTOR_1` si aplica) y ejecutar `cargo test -p searcher-rs --test multistep_fork -- --ignored`; opcionalmente `ARBX_READINESS_EVIDENCE_URL` + `ARBX_ADMIN_TOKEN` para registrar la evidencia |
| **B-4** | Toda la familia **mev_02 (17 identidades, AMM curve)** sin test de comportamiento por identidad | 17 | decisión de ingeniería (escribir los tests), no bloqueo externo |
| **B-5** | `mev_01`: 11 identidades con sólo compilación | 11 | idem |
| **B-6** | `LIVE_DATA_VERIFIED` con oportunidad = 0 | 264 | depende del mercado y de cerrar B-1/B-2; **no** se puede fabricar |
| **B-7** | Suites de Node (vitest) y de `searcher-rs` **no ejecutadas por T-E** | A/B/C/D | entorno con dependencias instaladas (`node_modules`) y `target/` caliente; en este worktree `cargo check` de `searcher-rs` no es viable por `target/` frío (nota §36.4 del repo, Windows AppControl os error 4551) |
| **B-8** | `EXECUTION_AUTHORIZED` = 0 | 264 | (a) valor real de `ARBX_LIVE_EXEC_ENABLED`/`ARBX_LIVE_EXEC_CHAINS` en el VPS — **AUSENTE en esta evidencia**; (b) autorización operativa de gasto del operador (§34.3/§34.5) |
| **B-9** | `SETTLED`/`RECONCILED` = 0 | 264 | capacidad de ejecución autorizada (B-8) + inventario/capital; **es el resultado honesto actual**, no un defecto del motor |
| **B-10** | QuoteBase con dos hashes distintos en `Downloads` | fuente documental | decidir precedencia explícita (base 2026-08-28 vs copias 2026-08-23) y registrarla; la ingesta usó la base |
| **B-11** | `…LIVE_First (2).xlsx` AUSENTE | fuente documental | si existe en otra ubicación, aportarla; el manifiesto de ingesta usó el nombre base |
| **B-12** | 1 nombre de estrategia duplicado entre dos `MEV_ID` | 2 identidades | corregir el nombre en el workbook origen o declarar la excepción; la identidad válida es `MEV_ID` |
| **B-13** | 8 `OBSERVE_ONLY` sin contrato de requisitos | 8 | decidir si su contrato de observación debe declarar requisitos propios (hoy `[]` es honesto pero silencioso) |

---

## 13. Trazabilidad de este entregable

| | |
|---|---|
| Documento | `docs/omegasearch/ACCEPTANCE_MATRIX_264.md` (este archivo) |
| Anexo máquina-legible | `docs/omegasearch/ACCEPTANCE_MATRIX_264_ANNEX.csv` — 264 filas × 37 columnas, con `requirements` por identidad y los 8 estados por identidad |
| Commit | el de esta tarea en `fix/omegasearch-autonomous-01` (SHA en el informe final de T-E) |
| Base | `9202561e` (`origin/main`) — verificado ancestro del commit de T-E |
| Autoría | tarea **T-E**, oleada 2 del prompt `PROMPT_ARBITRAGEX_BUSQUEDA_AUTONOMA_FEES_264.md` |
| Reproducibilidad | §10 completo: comandos, salidas, exit codes y denominadores |
| Lo que este documento **no** hace | no cambia código, no activa capital, no firma, no transmite, no toca zona caliente, no toca el árbol del operador, no deshabilita tests |
