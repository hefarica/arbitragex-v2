# GATE-UNIVERSE-01 — Universo correcto de gates de Mainnet live con crosswalk contra `SKILL.md` v2.0.0

- **Rol:** Specification (SSOT documental, fuentes, trazabilidad y contratos de especificación)
- **Tarea:** t62 — GATE-UNIVERSE-01 · Equipo `arbx-publicacion-desbloqueo-02`
- **Sucede a:** `docs/specification/GATE-CRITERIA-TRACE-G1-G8.md` (t59), que probó la infidelidad con severidad ALTA (H-1…H-10).
- **Alcance de esta orden:** **arreglar el UNIVERSO y los CRITERIOS, no los estados.** Re-medir es `t54`/`t61` (SRE); el drill de G7 es `t56` (Reviewer); el canal de G2 es `t60` (Release).
- **No se modificó** el `SKILL.md` ni el tablero de estado: se **emite el universo correcto como artefacto propio**. La sustitución del tablero es decisión del operador.
- **SHA de referencia:** `origin/main` = `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`.

## Autoridad citada

| Artefacto | Ruta | blob git |
|-----------|------|----------|
| **Fuente normativa de los criterios** | `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` | `60e79f908407ce174aa16d91142209c082cc002f` |
| Tablero en disputa (universo paralelo) | `audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md` | `61cc176af1170b9a17424a0c34b932d863a50b81` |
| Dictamen previo con la numeración del SKILL | `audits/real-cycles-audit-20260916/GOAL-WORKORDERS.md` | (L19, WO-05) |

**Regla de autoridad:** el **SKILL.md v2.0.0 es la autoridad de los criterios** (es lo que el tablero declara como fuente, en su L2). El tablero es un **reporte**, no una fuente normativa. Donde discrepan, manda el SKILL.

---

## 0. Por qué esto no es burocracia

El operador quiere llevar la herramienta a **Mainnet live**, y la lista de lo que lo bloquea sale del tablero. El tablero usa la numeración `G1-G8` para **un universo de criterios distinto** del SKILL que gobierna la activación, y **no declara el crosswalk**. Consecuencia medida en t59: **0 de 8** nombres de gate del SKILL aparecen en el tablero.

Dos daños concretos y no formales:

1. **`GATE-1 DATA-INTEGRITY-V3` no está en el tablero** (t59 H-4), pese a que `GOAL-WORKORDERS.md:19` ya lo había dictaminado **`NO PASS`** el 2026-09-16. **Hay un gate de capital en NO PASS del que el tablero no habla.**
2. **El `G3` del tablero funde dos gates distintos** (t59 H-5): `GATE-3 PAPER-4-LAYERS` y `GATE-4 SUBMIT-ENGINE-DURABLE`. Dos criterios con un solo estado: **si uno pasa y el otro no, el tablero no puede decirlo** — y son criterios con modos de fallo independientes (PnL mal calculado vs. doble ejecución).

---

## 1. CROSSWALK explícito — `SKILL.md` v2.0.0 ↔ numeración del tablero

**Veredicto de correspondencia** por par. `MISMO` = mismo criterio; `DISTINTO` = criterio distinto con nombre distinto; `SIN CONTRAPARTE` = no existe del otro lado. `PARCIAL-MERGE` = el tablero colapsa varios gates del SKILL en una fila.

| Gate SKILL (autoridad) | Línea SKILL | Fila del tablero | Línea tablero | **Correspondencia** | Qué se pierde en el tablero |
|------------------------|-------------|------------------|---------------|---------------------|------------------------------|
| `GATE-1 DATA-INTEGRITY-V3` | **L27–31** | *(ninguna)* | — | **SIN CONTRAPARTE** | **TODO el gate.** 318 pools, 90 candidatos, 2 firmas, backup+restore, SSH exit 255 |
| `GATE-2 SIMULATION-CYCLIC-C2C3` | **L33–37** | `G2` | L8 | **MISMO** (parcial) | Omite "2–5 hops" y los "10 fork-replay <1 %" |
| `GATE-3 PAPER-4-LAYERS` | **L39–42** | `G3` | L9 | **PARCIAL-MERGE** | Las 4 capas de PnL; ETH dinámico vs 3500 |
| `GATE-4 SUBMIT-ENGINE-DURABLE` | **L44–47** | `G3` (compartida) | L9 | **PARCIAL-MERGE** | Máquina de 7 fases; `idempotency_key`; UNIQUE INDEX |
| `GATE-5 FORK-REPLAY-10` | **L49–52** | `G6` | L12 | **MISMO** (parcial) | Los 10 bloques; el `<1 %` de desviación |
| `GATE-6 SEPOLIA-LIVE` | **L54–57** | `G5` | L11 | **MISMO** (parcial) | UUPS proxy verificado; circuit breaker probado; pausa de emergencia |
| `GATE-7 A.9-REINFORCED` | **L59–61** | `G7` | L13 | **MISMO** (parcial) | 10 controles reducidos a "check 1 = kill-switch" |
| `GATE-8 MAINNET-CANARY` | **L63–66** | *(ninguna)* | — | **SIN CONTRAPARTE** | **TODO el gate.** ≤$350, 5 WETH, `max_loss`, reconciliación <1 % |
| *(no existe en el SKILL)* | — | `G1` | L7 | **SIN CONTRAPARTE** | Criterio del tablero sin fuente normativa: "Deploy veraz / infra" |
| *(no existe en el SKILL)* | — | `G4` | L10 | **SIN CONTRAPARTE** | Criterio del tablero sin fuente normativa: "Net-profit gate on-chain honesto" |
| *(no existe en el SKILL)* | — | `G8` | L14 | **SIN CONTRAPARTE** | Criterio del tablero sin fuente: "Acta + paquete activación" (no es el canary) |

### 1.1 El error de raíz, dicho sin rodeos

**8 gates del SKILL ↔ 8 filas del tablero NO es una correspondencia 1:1, es una coincidencia de conteo.**

- **2 gates del SKILL no tienen fila:** `GATE-1` y `GATE-8`.
- **2 gates del SKILL están fundidos en una fila:** `GATE-3` + `GATE-4` → `G3`.
- **3 filas del tablero no tienen fuente normativa:** `G1`, `G4`, `G8`.

Es decir: **el "G1-G8" del tablero y el "G1-G8" del SKILL son dos universos de 8 elementos cada uno que comparten el rótulo y comparten sólo 6 criterios, uno de ellos fundido.** Con la numeración coincidente, cualquiera que lea "G1-G8 PASS" en el tablero creerá que el `GATE-8 MAINNET-CANARY` pasó cuando en realidad pasó un criterio documental distinto.

**Regla derivada (obligatoria para cualquier tabla futura):** *una tabla de gates que cite el `SKILL.md` v2.0.0 DEBE usar los nombres `GATE-n NOMBRE` del SKILL como clave primaria, y puede añadir columnas propias, pero NO puede reutilizar la numeración `Gn` para otro conjunto de criterios.* Dos tablas con los mismos nombres miden cosas distintas.

---

## 2. Universo CORRECTO — 8 gates, criterio textual citado por línea

Numeración **normativa del SKILL**. Cada gate declara su criterio textual, el estado **declarado** por las fuentes existentes (no re-medido) y el artefacto — o su ausencia, medida.

### `GATE-1` — DATA-INTEGRITY-V3 · **RESTITUIDO**

**Criterio textual (SKILL L27–31):**
> **L27** `GATE-1: DATA-INTEGRITY-V3`
> **L28** `- Evidence: Manifest V3 with 318 pools verified on-chain, 90 candidates identified`
> **L29** `- Requirement: 2 human signatures on manifest hash + PostgreSQL backup tested + restore verified`
> **L30** `- Blocker: SSH access to VPS (exit 255) must be resolved or alternative backup path established`
> **L31** `- Output: SQL UPDATE plan generated, signed, ready for execution`

**Criterio atómico, descompuesto (5 sub-criterios, uno por línea del SKILL):**

| Sub | Criterio | Estado declarado | Artefacto citado / medido |
|-----|----------|------------------|----------------------------|
| 1a | Manifest V3 con **318 pools** verificados on-chain | **NO PASS** (WO-05) | — no localizado |
| 1b | **90 candidatos** identificados | **NO PASS** (WO-05) | — no localizado |
| 1c | **2 firmas humanas** sobre el hash del manifest | **NO PASS** (WO-05) | **AUSENTE**, medido |
| 1d | **Backup PostgreSQL probado + restore verificado** | **NO PASS** (WO-05) | **AUSENTE**, medido |
| 1e | **SSH exit 255** resuelto o ruta alterna establecida | **NO PASS** (WO-05) | — |

**Artefacto: su AUSENCIA, medida.** `GOAL-WORKORDERS.md:19` dice literalmente *"0 firmas en manifest verificadas, 0 restore-test (**artifacts/ sin attestation**)"*. Mediciones propias que lo confirman:

- `git ls-tree -r --name-only origin/main -- artifacts` → **6 entradas, ninguna es attestation**: `artifacts/excel_coverage.json`, `artifacts/excel_coverage.md`, `artifacts/excel_requirements.json`, `artifacts/excel_ultra_raw.json`, `artifacts/source_field_map.json`, `artifacts/strategy_registry.json`.
- Archivos de firma (`.sig`/`.asc`/`.gpg`/`.minisig`, o `signature`/`firma` en el nombre): **0** relevantes. El único impacto, `docs/contracts/src/utils/SignatureValidator.sol`, es un utilitario de contratos sin relación con firmar un manifest.
- `cargo run -p data-integrity --bin verify-v3-fees` (SKILL L193): **la crate `data-integrity` y el binario `verify-v3-fees` NO existen** en `origin/main`. Sí existen módulos de fees (`backend/searcher-rs/src/v3_fee_catalog.rs`, `backend/searcher-rs/src/pool_discovery/v3_fee.rs`, `.github/workflows/v3-fee-data-integrity.yml`), pero **no son el generador de manifest que el gate exige**.

**Dictamen:** **`NO PASS`**, sostenido. Es una **regresión de criterio sobre un gate de capital**: el gate estaba dictaminado en NO PASS el 09-16 y **el tablero del 09-17 no lo menciona en absoluto**. Un lector del tablero concluiría que la integridad de datos V3 no es un bloqueo de mainnet. **Lo es, y es el primero de la lista del SKILL.**

### `GATE-2` — SIMULATION-CYCLIC-C2C3

**Criterio textual (SKILL L33–37):**
> **L33** `GATE-2: SIMULATION-CYCLIC-C2C3`
> **L34** `- Evidence: Issue #567 closed with PR merged (codex/567-canonical-plan-simulation)`
> **L35** `- Requirement: S4 consumer simulates FULL cycles (2-5 hops), not single-swap probes`
> **L36** `- Validation: 10 fork-replay cases with <1% PnL deviation vs on-chain reality`
> **L37** `- Blocker: Current tx_builder.rs:51-79 rejects cyclic routes with CyclicRouteNotRepresentable`

**Estado declarado:** `FAIL` (tablero `G2`, L8: `SELECT COUNT(*) FROM simulations WHERE passed` = 0).

**Verificaciones de trazabilidad que sí corresponden a esta orden:**
- **El blocker de L37 es real y localizable:** `backend/sim-ctl/src/tx_builder.rs:77` → `return Err(BuildError::CyclicRouteNotRepresentable(` — dentro del rango L51-79 que el SKILL declara. 4 ocurrencias del símbolo. **La cita del SKILL se sostiene.**
- **`canonical_plan_consumer.rs` EXISTE** en `origin/main` y en la rama `fix/567-canonical-plan-consumer` (`99153e1a…`). El SKILL L11 lo declaraba *"NO existe (FIX-2 ordena crearlo)"*: **esa afirmación del SKILL quedó obsoleta**, y se registra como tal (contexto, no criterio).
- **La evidencia que el SKILL propone está podrida:** `codex/567-canonical-plan-simulation` **no resuelve** en `origin` (medido en t59 §3). La rama real es `fix/567-canonical-plan-consumer`. **El propio criterio cita una referencia inexistente** — es el precedente del 2026-09-15, aún sin corregir en el SKILL.

**No es de esta orden** re-medir el conteo: eso es `t54`/`t61`. Aquí consta que el `FAIL` declarado tiene método reproducible y que su *evidencia propuesta por el SKILL* no resuelve.

### `GATE-3` — PAPER-4-LAYERS · **DES-FUNDIDO**

**Criterio textual (SKILL L39–42):**
> **L39** `GATE-3: PAPER-4-LAYERS`
> **L40** `- Evidence: 4-layer PnL implemented (predicted → simulated → observed_market → realized)`
> **L41** `- Requirement: USD calculations use real decimals, dynamic ETH price (not 3500 fixed)`
> **L42** `- Validation: predicted ≠ observed for all trades; realized only for LIVE`

**Sub-criterios que el `G3` fundido perdía:**

| Sub | Criterio | Cómo se comprueba |
|-----|----------|-------------------|
| 3a | Las **4 capas** existen y están etiquetadas: `predicted → simulated → observed_market → realized` | Presencia de los 4 campos por trade |
| 3b | Cálculos USD con **decimales reales** y **precio ETH dinámico** — `3500` fijo PROHIBIDO | Ausencia del literal; precio con fuente y timestamp |
| 3c | **`predicted ≠ observed`** en todos los trades | Desigualdad verificada por trade |
| 3d | **`realized` sólo en LIVE** | Ausencia de `realized` en PAPER/SHADOW |

**Estado declarado:** `FAIL` — el tablero lo reporta dentro de `G3` como *"paper ledger 598K runs todos REJECTED (R-0001, ledger)"*. **Ese estado cubre el flujo paper, no la corrección de las 4 capas**: con el gate fundido es imposible saber si las capas existen y están bien calculadas aunque el flujo esté rechazado, y viceversa.

**Conexión con la prohibición:** SKILL **L240** `3. NEVER use hardcoded USD prices (3500 or any other)` y **L241** `4. NEVER allow predicted_pnl == observed_pnl (must be calculated separately)`. Son **prohibiciones de abort inmediato**, no recomendaciones.

### `GATE-4` — SUBMIT-ENGINE-DURABLE · **DES-FUNDIDO**

**Criterio textual (SKILL L44–47):**
> **L44** `GATE-4: SUBMIT-ENGINE-DURABLE`
> **L45** `- Evidence: State machine 7-phase operational (CREATED→VALIDATED→AUTHORIZED→SUBMITTING→SUBMITTED→INCLUDED/REVERTED/EXPIRED→RECONCILED)`
> **L46** `- Requirement: Idempotency_key deterministic, UNIQUE INDEX PostgreSQL, no 60s window dedup`
> **L47** `- Blocker: Duplicate execution prevention must be server-side, not UI-only`

**Sub-criterios que el `G3` fundido perdía:**

| Sub | Criterio | Medición de existencia en `origin/main` |
|-----|----------|------------------------------------------|
| 4a | **Máquina de estados de 7 fases** operativa | **NO LOCALIZADA.** `git grep 'ExecutionState' origin/main -- '*.rs'` → **0**; `submit_engine.rs` (1540 líneas) → **0** ocurrencias de `ExecutionState`, `idempotency`, `Idempotency`, `UNIQUE`, `RECONCILED`, `EXPIRED` |
| 4b | **`idempotency_key` determinista** | **PARCIAL.** El símbolo existe en el repo, pero en otra ruta: `backend/searcher-rs/src/config_reload_omni.rs:45,61,211`. **No en el submit engine.** SKILL **L161** define la receta `keccak256(chain_id + opportunity_id + target_block + route_hash + amount_in + mode)` |
| 4c | **UNIQUE INDEX PostgreSQL** sobre `execution_intents(idempotency_key)` | **AUSENTE como se especifica.** SKILL **L164** ordena `CREATE UNIQUE INDEX idx_idempotency ON execution_intents(idempotency_key);`. `git grep 'execution_intents' origin/main` → **1 sola coincidencia, y es el propio SKILL.md L164**: **la tabla `execution_intents` no existe en ninguna migración**. Hay índices únicos de idempotencia en **otras** tablas: `066_omni_entity_registries.sql:199,206` (`idempotency_key TEXT NOT NULL` + `UNIQUE (idempotency_key)`), `113_simulations_revm_idempotency.sql:27` (`simulations_revm_idempotency_uq`). **Ninguno es el que el gate exige** |
| 4d | **Sin dedup por ventana de 60 s**; prevención **server-side**, no sólo UI | Sin medición propia en esta orden |

**Estado declarado:** `FAIL` — el tablero lo reporta dentro de `G3` como `COUNT(*) FROM executions` = 0. **Ese estado mide ejecuciones, no durabilidad del engine**: un sistema sin ejecuciones tiene `executions = 0` **aunque la máquina de estados esté perfectamente implementada**. El contador no puede distinguir "no se ejecutó nada" de "el engine no protege contra dobles ejecuciones". **Ése es exactamente el daño de fundir los dos gates.**

**Conexión con la prohibición:** SKILL **L245** `8. NEVER enable automatic execution without idempotency_key` — abort inmediato. Y **L247** fija el techo de capital del canary.

### `GATE-5` — FORK-REPLAY-10

**Criterio textual (SKILL L49–52):**
> **L49** `GATE-5: FORK-REPLAY-10`
> **L50** `- Evidence: 10 historical mainnet blocks replayed with full simulation`
> **L51** `- Requirement: <1% deviation in output amounts, gas costs, and net PnL`
> **L52** `- Validation: Matched against actual on-chain results`

**Estado declarado:** el tablero lo reporta como `G6` `⚠️ PARCIAL` con evidencia *"anvil-1 healthy; SIM-FUND-01 (125b1e0b) arregla STF del probe"*.
**Lo que el tablero pierde:** los **10 bloques** concretos (el SKILL L229 los enumera: `20499995…20500004`) y el umbral **`<1 %`**. "anvil-1 healthy" es salud de infraestructura; **el gate pide 10 replays con desviación acotada**. `GOAL-WORKORDERS.md` lo dictaminó **`NO PASS`** (*"G5 fork-replay-10: sin artefactos"*).
**Prohibición asociada:** SKILL **L246** `9. NEVER skip fork replay before Sepolia`.

### `GATE-6` — SEPOLIA-LIVE

**Criterio textual (SKILL L54–57):**
> **L54** `GATE-6: SEPOLIA-LIVE`
> **L55** `- Evidence: ArbitrageExecutor.sol deployed, initialized, UUPS proxy verified`
> **L56** `- Requirement: Complete operation (detect→quote→simulate→decide→execute→reconcile) with real gas`
> **L57** `- Validation: Circuit breaker tested, emergency pause functional`

**Estado declarado:** el tablero lo reporta como `G5` `✅ (histórico)`, evidencia *"Contrato defi verificado (deploy previo)"*.
**Veredicto de trazabilidad (de t59 H-1, no repetido aquí):** **PASS sin artefacto**. La celda no cita dirección, tx hash, SHA ni URL.
**Lo que el tablero pierde:** **UUPS proxy verificado** (L55), **circuit breaker probado** y **pausa de emergencia funcional** (L57) — dos de los tres sub-criterios, y precisamente los dos que tienen que ver con contención de pérdida.
`GOAL-WORKORDERS.md` lo dictaminó **`NO PASS`** (*"G6 Sepolia: sin evidencia on-chain verificada esta sesión"*).

### `GATE-7` — A.9-REINFORCED

**Criterio textual (SKILL L59–61):**
> **L59** `GATE-7: A.9-REINFORCED`
> **L60** `- Evidence: A.9 checklist (G7) with verified reproducible artifacts (§34.5.3 — las "2 firmas físicas" quedan sustituidas por evidencia verificada, órdenes 2026-09-15/17)`
> **L61** `- Requirement: Max notional, max loss, min profit, slippage, gas ceiling, bribe ceiling, expiry blocks, RPC quorum, stale quote protection, reorg protection ALL configured and tested`

**Los 10 controles de L61, uno por uno, contra `backend/shared-rs/src/pre_execute_checklist.rs` (1219 líneas, existe en `origin/main`):**

| # | Control (L61) | Ocurrencias del símbolo en el checklist |
|---|---------------|------------------------------------------|
| 1 | Max notional | **0** |
| 2 | Max loss | **0** |
| 3 | Min profit | **15** |
| 4 | Slippage | **44** |
| 5 | Gas ceiling | **0** |
| 6 | Bribe ceiling | **2** |
| 7 | Expiry blocks | **0** |
| 8 | RPC quorum | **0** |
| 9 | Stale quote protection | **7** |
| 10 | Reorg protection | **0** |

**Dictamen de criterio:** el criterio de L61 exige los 10 **"ALL configured and tested"**. La medición de existencia muestra **4 controles con presencia textual** (min profit, slippage, bribe, stale) y **6 sin ocurrencia del símbolo** en el archivo que el tablero cita como evidencia. **No declaro que los 6 estén ausentes del sistema** — pueden implementarse en otro módulo; declaro que **no están en el artefacto que el tablero cita**, y por tanto **la evidencia citada no cubre el criterio**.

**Estado declarado:** el tablero lo reporta como `G7` `⚠️ CÓDIGO OK / DRILL FALTA`, reduciendo 10 controles a *"check 1 = kill-switch"*. `GOAL-WORKORDERS.md`: **`NO PASS`** (*"dashboard dice sign-off pending"*).

### `GATE-8` — MAINNET-CANARY · **RESTITUIDO**

**Criterio textual (SKILL L63–66):**
> **L63** `GATE-8: MAINNET-CANARY`
> **L64** `- Evidence: First trade with max_loss parameterized, not seeking "profit" but "correct measurement"`
> **L65** `- Requirement: Capital at risk ≤ $350, flash principal 5 WETH, gas reserve separate`
> **L66** `- Validation: Reconciliation shows expected vs actual with <1% deviation`

**Sub-criterios:**

| Sub | Criterio | Referencia cruzada |
|-----|----------|--------------------|
| 8a | Primera operación con **`max_loss` parametrizado** | SKILL L293 `--max-loss 350` |
| 8b | **Capital en riesgo ≤ $350** | SKILL **L247** `10. NEVER use LIVE capital > $350 for canary`; SKILL L274 `Capital at risk: ≤ $350` |
| 8c | **Principal flash 5 WETH**, reserva de gas **separada** | SKILL L275 `Flash principal: 5 WETH` |
| 8d | Reconciliación **expected vs actual <1 %** | SKILL L277 `Reconciliation shows <1% deviation` |

**Estado declarado:** **`NO PASS`.** `GOAL-WORKORDERS.md:19` lo dictamina: *"G8 canary: bloqueado por G1-G7"*.
**Lo que el tablero pierde:** la fila `G8` del tablero (`L14`) se llama *"Acta + paquete activación"* con estado `✅ HOY` — **un criterio documental, no el canary**. **El gate de mayor consecuencia económica de todo el universo — el único que arriesga capital real — no tiene fila en el tablero.** Peor: el rótulo `G8` del tablero está en `✅`, de modo que quien lea "G8 ✅" creerá que el canary pasó.

**Regla de activación asociada:** SKILL **L239** `2. NEVER enable LIVE mode before G1-G8 pass with verified reproducible evidence (§34.5.3, órdenes 2026-09-15/17)`. Y la cabecera L9: los gates se cumplen **con artefactos reproducibles**, no con claims. **Un gate ausente del tablero no puede pasar: simplemente no se evalúa.**

---

## 3. Criterios del tablero que NO existen en la fuente (t59 H-8)

El tablero usa 3 criterios que **no tienen contrapartida en el `SKILL.md` v2.0.0**. No se declaran falsos: se declaran **huérfanos de fuente normativa**.

| Fila tablero | Criterio | Línea | Situación |
|--------------|----------|-------|-----------|
| `G1` | Deploy veraz / infra | L7 | **Sin fuente.** El SKILL no define un gate de "deploy veraz". El concepto de verificar el SHA desplegado es legítimo, pero **no es uno de los 8 gates del SKILL** |
| `G4` | Net-profit gate on-chain honesto | L10 | **Sin fuente.** Evidencia citada: *"Gate en código (G-ECON cerrado 2026-08)"* — **sin artefacto localizable** |
| `G8` | Acta + paquete activación | L14 | **Sin fuente, y ocupa el rótulo del canary.** Es un entregable documental, no un gate técnico |

**Recomendación de gobierno:** estos 3 criterios pueden conservarse en una tabla **con otra clave** (p. ej. `CTRL-n`) o como columnas adicionales de los gates del SKILL, pero **no deben reutilizar la numeración `Gn`** reservada a los gates normativos. Reutilizarla es lo que produjo H-3.

---

## 4. Tabla consolidada del universo correcto

Clave primaria = nombre del SKILL. `estado_declarado` = lo que dicen las fuentes (`GFW` = `GOAL-WORKORDERS.md:19`; `TB` = tablero). **Ningún estado fue re-medido por esta orden.**

| Gate (SKILL) | Línea | Nombre | Estado declarado | Fuente del estado | Artefacto | ¿En repo/remoto? |
|--------------|-------|--------|------------------|-------------------|-----------|------------------|
| GATE-1 | L27–31 | DATA-INTEGRITY-V3 | **NO PASS** | GFW | Firmas del manifest, restore-test, attestation | **AUSENTE** (medido) |
| GATE-2 | L33–37 | SIMULATION-CYCLIC-C2C3 | **NO PASS** | GFW / TB `G2` FAIL | `SELECT COUNT(*) … passed` = 0 | Query no adjunta |
| GATE-3 | L39–42 | PAPER-4-LAYERS | **NO PASS** | GFW | 4 capas de PnL + decimales + ETH dinámico | Parcial (módulos de fee existen) |
| GATE-4 | L44–47 | SUBMIT-ENGINE-DURABLE | **NO PASS** | GFW / TB `G3` FAIL | Máquina 7 fases + UNIQUE INDEX | **Parcial/otra tabla** (medido) |
| GATE-5 | L49–52 | FORK-REPLAY-10 | **NO PASS** | GFW | 10 bloques replayados <1 % | **AUSENTE** |
| GATE-6 | L54–57 | SEPOLIA-LIVE | **NO PASS** | GFW (TB dice ✅) | Dirección + tx + UUPS verificado | **AUSENTE** (t59 H-1) |
| GATE-7 | L59–61 | A.9-REINFORCED | **NO PASS** | GFW | 10 controles configurados + probados | Artefacto citado cubre 4/10 |
| GATE-8 | L63–66 | MAINNET-CANARY | **NO PASS** (bloqueado por G1-G7) | GFW | Receipt + reconciliación <1 % | **AUSENTE** |

**Los 8 gates están en `NO PASS`** según la fuente de estado disponible (`GOAL-WORKORDERS.md`, 2026-09-16, con la numeración del SKILL). El tablero del 09-17 reporta 2 en `✅`. **Esa discrepancia es el hallazgo, y su resolución es de SRE (`t54`/`t61`), no de esta orden.**

---

## 5. Declaraciones obligatorias

### 5.1 Esto NO mueve P/N

- `N = 115` permanece **congelado** en `scope_version = arbx-scope-1.0.0` (`docs/specification/ACCEPTANCE-MATRIX-v1.md` §0).
- Los gates G1-G8 son un **universo DISTINTO** del censo `AC-USR-*`/`AC-FND-*`/`AC-FRT-*` y **NO se suman a N** (regla R5: no mezclar naturalezas). 8 gates nuevos no convierten `N = 115` en `N = 123`.
- `aceptados = 0` sin cambio. Ninguna fila de la matriz se re-clasificó.
- **Ningún porcentaje se recalcula.**

### 5.2 No se modificó ningún artefacto auditado

| Artefacto | blob en `origin/main` | ¿Modificado? |
|-----------|----------------------|--------------|
| `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` | `60e79f908407ce174aa16d91142209c082cc002f` | **NO** — fuera de alcance |
| `audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md` | `61cc176af1170b9a17424a0c34b932d863a50b81` | **NO** — fuera de alcance |
| `audits/real-cycles-audit-20260916/GOAL-WORKORDERS.md` | (sólo leído) | **NO** |
| `backend/shared-rs/src/pre_execute_checklist.rs` | (sólo leído) | **NO** |
| `backend/sim-ctl/src/tx_builder.rs` | (sólo leído) | **NO** |
| Tableros, ACTA, RUNBOOK | (sólo leídos) | **NO** |

**Todo el trabajo de escritura ocurrió en `docs/specification/`** (único ámbito en alcance), sobre la rama `docs/gate-universe-01` en worktree aislado, con base `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`.

### 5.3 Sustitución del tablero = decisión del operador

Este artefacto **emite el universo correcto**. **No reemplaza** `GATES-G1-G8-ESTADO.md`, no lo edita y no lo deprecia. La decisión de sustituirlo, renombrarlo o añadirle el crosswalk como encabezado **es del operador**, y es la acción concreta que cerraría H-3 en el tablero mismo.

---

## 6. Fronteras que entrego

| Frontera | A quién | Por qué no es mía |
|----------|---------|-------------------|
| **Re-medir** los 8 estados | **SRE (`t54`/`t61`)** | Requiere PG/RPC/runtime, no documentación. Aquí se declara, no se re-mide |
| **Drill trip/untrip** de GATE-7 | **Reviewer (`t56`)** | Es ejecución de un procedimiento, no trazabilidad documental |
| **Canal de GATE-2** | **Release (`t60`)** | |
| **Decidir la sustitución del tablero** y añadirle el crosswalk | **Operador** | §5.3. Cambia un artefacto de gobernanza que el operador usa para decidir capital |
| **Corregir las 3 referencias podridas del SKILL** (`codex/567-canonical-plan-simulation` L34; `verify-pipeline-567` L10; `sim-ctl/src/submit_engine.rs` L152) | **Autor del SKILL / operador** | Está fuera de alcance (`.claude/skills/`) y el SKILL no se toca |
| **Reponer/retirar `fix/v3-slot0-coverage-20260917`** | **Repo admin** | Estado real de la rama |

---

## 7. Comandos de reproducción

```bash
# Autoridad de los criterios, línea por línea
git show origin/main:.claude/skills/arbitragex-v2-mainnet-live/SKILL.md | sed -n '27,66p'

# GATE-1: la ausencia de artefacto, medida
git ls-tree -r --name-only origin/main -- artifacts          # 6 entradas, ninguna attestation
git grep -n 'data-integrity' origin/main -- backend          # sin crate verify-v3-fees

# GATE-4: la tabla que el criterio nombra no existe
git grep -n 'execution_intents' origin/main                  # 1 sola: SKILL.md L164
git grep -n 'ExecutionState' origin/main -- '*.rs'           # 0
git grep -n -i 'unique.*idempotency' origin/main -- 'database/migrations/*.sql'
#   -> 066_omni_entity_registries.sql (otra tabla), 113_simulations_revm_idempotency.sql (otra tabla)

# GATE-2: el blocker del SKILL L37 es real
git show origin/main:backend/sim-ctl/src/tx_builder.rs | sed -n '77p'

# GATE-7: los 10 controles contra el artefacto citado
git show origin/main:backend/shared-rs/src/pre_execute_checklist.rs | grep -cE 'max_notional|max_loss|gas_ceiling|expiry|quorum|reorg'

# Existencia de cada artefacto citado
git cat-file -e origin/main:backend/shared-rs/src/pre_execute_checklist.rs   # exit 0
git cat-file -e origin/main:artifacts                                        # exit 0 (6 entradas)
```

---

## 8. Cierre

- El universo correcto son **los 8 gates del `SKILL.md` v2.0.0**, con sus nombres como clave primaria y su índice `SKILL-<1..8>` como único rótulo admisible en tablas que lo citen.
- **`GATE-1 DATA-INTEGRITY-V3` y `GATE-8 MAINNET-CANARY` quedan restituidos**, ambos en **`NO PASS`** según la fuente de estado disponible, con su artefacto declarado y su ausencia medida.
- **`G3` queda des-fundido** en `GATE-3 PAPER-4-LAYERS` y `GATE-4 SUBMIT-ENGINE-DURABLE`, cada uno con su criterio y su estado.
- **3 criterios del tablero quedan declarados huérfanos de fuente normativa** (`G1`, `G4`, `G8` del tablero): no se borran, se les retira la numeración reservada.
- **`N = 115` y `aceptados = 0` sin cambio.** Los gates son otro universo y no se suman a N.

*Un tablero que reutiliza la numeración de su fuente para otro conjunto de criterios no mide menos: mide otra cosa, con la misma etiqueta. Este artefacto restituye qué se está midiendo.*
