# GATES G1-G8 — ESTADO CON EVIDENCIA (2026-09-17)

> **ENMENDADO 2026-10-06** (t67 / G8-TABLERO-01). La tabla original de 2026-09-17 se conserva
> íntegra abajo, marcada `SUPERSEDED`. El motivo de la enmienda no es cosmético: **este tablero
> usaba el rótulo `G1-G8` para un universo de criterios DISTINTO del que gobierna la activación**,
> y en consecuencia **`GATE-8 MAINNET-CANARY` — el único gate que arriesga capital real — no tenía
> fila**, mientras el rótulo `G8` de este tablero estaba en `✅`. Un lector de «G8 ✅» quedaba
> inducido a creer que el canary había pasado. Ver §Enmienda y §Crosswalk.
> Artefacto de referencia: `docs/specification/GATE-UNIVERSE-v1.md` (t62).

> Fuente de los criterios: `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` (v2.0.0).
> Estándar de evidencia §34.5.3: artefactos reproducibles, JAMÁS claims.

> ⚠️ **REGLA DE CLAVE PRIMARIA (obligatoria — derivada en t62).** Una tabla que cite el
> `SKILL.md` v2.0.0 **DEBE usar los nombres `GATE-n NOMBRE` del SKILL como clave primaria**, y
> puede añadir columnas propias, pero **NO puede reutilizar la numeración `Gn` para otro conjunto
> de criterios**. Sin esta regla, **dos tablas con los mismos nombres miden cosas distintas** — que
> es exactamente lo que ocurrió aquí: `t59` midió **0 de 8** nombres de gate del SKILL presentes en
> este tablero, y `t63` midió además que **el tablero PERMUTA `G5` y `G6`** respecto del SKILL.
> **La numeración `Gn` NO es una clave válida; el par `GATE-n NOMBRE` sí lo es.**
>
> Convención de identificadores adoptada en esta enmienda:
> - **`GATE-n NOMBRE`** → gate normativo del `SKILL.md` v2.0.0. Numeración reservada.
> - **`CTRL-n NOMBRE`** → criterio propio de este tablero **sin contraparte en el SKILL**. No
>   puede usar la numeración reservada `GATE-n`.

## Tabla original (2026-09-17) — `SUPERSEDED`

> Conservada **íntegra** como registro histórico. Sus rótulos `Gn` **no son claves válidas** (ver
> regla de clave primaria arriba): designan el universo propio de este tablero, no los gates del
> SKILL. La versión vigente es §Enmienda.

| Rótulo | Criterio (resumen) | Estado | Evidencia verificada hoy |
|---|---|---|---|
| G1 | Deploy veraz / infra | ⚠️ PARCIAL | VPS `a06a968d` healthy 24/24; PERO 3 commits locales sin deploy (`fdb40401`,`125b1e0b`,`dcfe890c`) |
| G2 | Simulación cíclica (≥1 sim passed) | ❌ FAIL | `SELECT COUNT(*) FROM simulations WHERE passed` = **0** (toda la historia, query 2026-09-17) |
| G3 | Paper→submit engine con ciclo real | ❌ FAIL | `COUNT(*) FROM executions` = **0**; paper ledger 598K runs todos REJECTED (R-0001, ledger) |
| G4 | Net-profit gate on-chain honesto | ⚠️ SIN EJERCITAR | Gate en código (G-ECON cerrado 2026-08: net=0=gas_floor_breach honesto); sin sims passed no hay input |
| G5 | Contratos Sepolia verificados | ✅ (histórico) | Contrato defi verificado (deploy previo); revalidar al SHA nuevo antes del flip |
| G6 | Fork replay + invariants | ⚠️ PARCIAL | anvil-1 healthy; SIM-FUND-01 (125b1e0b) arregla STF del probe — SIN deploy aún |
| G7 | Risk-limits + checklist pre-ejecución | ⚠️ CÓDIGO OK / DRILL FALTA | pre_execute_checklist.rs presente (check 1 = kill-switch); drill trip/untrip NO documentado |
| G8 | Acta + paquete activación | ✅ HOY | Este paquete (ACTA + RUNBOOK + este estado) |

## Cuello único que bloquea todo

`v3_quote_unavailable` → 0 candidatos evaluables → G2 imposible → G3 imposible.
**El trabajo YA está en curso en `fix/v3-slot0-coverage-20260917`** (remedio catálogo fee-tiers + slot0 backfill).
Pipeline vivo mientras tanto: opportunities fluye (28.3M total, última 06:19 UTC hoy).

## Nota anti-regresión

G2 del skill v2.0.0 citó evidencia fabricada una vez (rama codex/567 + commit 9a10350 inexistentes,
verificado 2026-09-15). Todo PASS de esta tabla requiere artefacto reproducible citado.

---

## Enmienda 2026-10-06 — universo correcto (clave = `GATE-n NOMBRE`)

**Fuente normativa:** `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` v2.0.0 · blob
`60e79f908407ce174aa16d91142209c082cc002f` · **NO modificado** por esta enmienda.

### A. Gates normativos del SKILL

`estado` = lo dictaminado por las fuentes de estado existentes. **Esta enmienda NO re-mide estados**
(eso es de SRE); corrige el universo, los criterios y el error de estado.

| Clave primaria | Criterio textual (SKILL, archivo:línea) | Estado | Artefacto probatorio |
|---|---|---|---|
| **`GATE-1 DATA-INTEGRITY-V3`** | `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md:27-31` — *"Manifest V3 with 318 pools verified on-chain, 90 candidates identified"*; *"2 human signatures on manifest hash + PostgreSQL backup tested + restore verified"*; *"SSH access to VPS (exit 255) must be resolved"* | **NO PASS** | **`NO COMPUTADO`** — sin attestation. Medido: `artifacts/` tiene 6 entradas y ninguna es attestation; 0 archivos de firma; la crate `data-integrity`/binario `verify-v3-fees` no existen |
| `GATE-2 SIMULATION-CYCLIC-C2C3` | `SKILL.md:33-37` | **FAIL** (rótulo `G2` original) | `SELECT COUNT(*) FROM simulations WHERE passed` = 0 — query no adjunta |
| `GATE-3 PAPER-4-LAYERS` | `SKILL.md:39-42` | **NO COMPUTADO** como gate propio | Estaba **fundido** en el rótulo `G3` original, que medía `COUNT(*) FROM executions` — un contador que **no** evalúa las 4 capas de PnL. Al des-fundirlo, **no hay medición que lo sostenga** |
| `GATE-4 SUBMIT-ENGINE-DURABLE` | `SKILL.md:44-47` | **NO COMPUTADO** como gate propio | Ídem: `executions = 0` **no** evalúa la máquina de 7 fases ni el UNIQUE INDEX. Medido además: `execution_intents` (la tabla que `SKILL.md:164` exige) **no existe en ninguna migración** |
| `GATE-5 FORK-REPLAY-10` | `SKILL.md:49-52` | **NO PASS** | **`NO COMPUTADO`** — `GOAL-WORKORDERS.md:19`: *"G5 fork-replay-10: sin artefactos"* |
| `GATE-6 SEPOLIA-LIVE` | `SKILL.md:54-57` | **NO PASS** | **`NO COMPUTADO`** — `GOAL-WORKORDERS.md:19`: *"G6 Sepolia: sin evidencia on-chain verificada esta sesión"*. El `✅` original **no citaba dirección, tx hash, URL ni SHA** (t59 H-1) |
| `GATE-7 A.9-REINFORCED` | `SKILL.md:59-61` | **NO PASS** | `backend/shared-rs/src/pre_execute_checklist.rs` existe, pero cubre **4 de los 10** controles que el criterio exige (`min_profit`, `slippage`, `bribe`, `stale`); `max_notional`, `max_loss`, `gas_ceiling`, `expiry`, `quorum`, `reorg` = **0 ocurrencias** en el artefacto citado |
| **`GATE-8 MAINNET-CANARY`** | `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md:63-66` — *"First trade with max_loss parameterized, not seeking "profit" but "correct measurement""*; *"Capital at risk ≤ $350, flash principal 5 WETH, gas reserve separate"*; *"Reconciliation shows expected vs actual with <1% deviation"* | **NO PASS** — bloqueado por `GATE-1`…`GATE-7` | **`NO COMPUTADO`** — **no existe la primera operación canary**: sin receipt, sin reconciliación expected-vs-actual. No hay artefacto que acredite ninguno de los 3 sub-criterios (≤$350 / 5 WETH / <1 %). Dice `GOAL-WORKORDERS.md:19`: *"G8 canary: bloqueado por G1-G7"* |

> **`GATE-8 MAINNET-CANARY` es el único gate que arriesga capital real.** Su estado **no puede ser
> `✅` sin artefacto reproducible**: un gate de capital sin artefacto es **`NO COMPUTADO`**, nunca
> `✅`. El `✅` que esta enmienda retira pertenecía a un criterio documental distinto (ver §B).

### B. Criterios propios de este tablero, SIN contraparte en el SKILL

**Ninguna de estas 3 filas se borra.** Se les **retira la numeración reservada** `GATE-n` y quedan
declaradas como criterios propios, con la clave `CTRL-n`. Se conserva su estado original.

| Clave | Criterio (resumen) | Estado original | Contraparte en el SKILL |
|---|---|---|---|
| `CTRL-1` | Deploy veraz / infra — VPS `a06a968d` healthy 24/24; PERO 3 commits locales sin deploy (`fdb40401`,`125b1e0b`,`dcfe890c`) | ⚠️ PARCIAL | **NINGUNA.** El SKILL no define un gate de "deploy veraz" |
| `CTRL-4` | Net-profit gate on-chain honesto — *"Gate en código (G-ECON cerrado 2026-08: net=0=gas_floor_breach honesto); sin sims passed no hay input"* | ⚠️ SIN EJERCITAR | **NINGUNA.** Criterio sin fuente normativa; su evidencia (`G-ECON cerrado 2026-08`) **no apunta a artefacto localizable** |
| `CTRL-8` | Acta + paquete activación — *"Este paquete (ACTA + RUNBOOK + este estado)"* | ✅ HOY | **NINGUNA.** Es un entregable documental, **no** `GATE-8 MAINNET-CANARY` |

> **`CTRL-8` conserva su `✅` porque es cierto para lo que realmente mide** — el paquete de
> activación (ACTA + RUNBOOK + este estado) existe en este directorio. Lo que se corrige es que
> **deja de ocupar el rótulo `G8`**, que un lector podía confundir con el gate de canary. El `✅`
> sigue siendo válido para `CTRL-8`; **no lo es, ni lo era, para `GATE-8 MAINNET-CANARY`.**

### C. Crosswalk `SKILL.md` ↔ tablero (par por par)

| Gate SKILL | Línea SKILL | Rótulo tablero | Línea tablero | Correspondencia |
|---|---|---|---|---|
| `GATE-1 DATA-INTEGRITY-V3` | L27–31 | *(ninguna)* | — | **SIN CONTRAPARTE** |
| `GATE-2 SIMULATION-CYCLIC-C2C3` | L33–37 | `G2` | L8 | **MISMO** (parcial) |
| `GATE-3 PAPER-4-LAYERS` | L39–42 | `G3` | L9 | **PARCIAL-MERGE** |
| `GATE-4 SUBMIT-ENGINE-DURABLE` | L44–47 | `G3` (compartida) | L9 | **PARCIAL-MERGE** |
| `GATE-5 FORK-REPLAY-10` | L49–52 | `G6` | L12 | **MISMO — PERMUTADO** |
| `GATE-6 SEPOLIA-LIVE` | L54–57 | `G5` | L11 | **MISMO — PERMUTADO** |
| `GATE-7 A.9-REINFORCED` | L59–61 | `G7` | L13 | **MISMO** (parcial) |
| `GATE-8 MAINNET-CANARY` | L63–66 | *(ninguna)* | — | **SIN CONTRAPARTE** |
| *(no existe en el SKILL)* | — | `G1` → `CTRL-1` | L7 | **SIN CONTRAPARTE** |
| *(no existe en el SKILL)* | — | `G4` → `CTRL-4` | L10 | **SIN CONTRAPARTE** |
| *(no existe en el SKILL)* | — | `G8` → `CTRL-8` | L14 | **SIN CONTRAPARTE** |

**La PERMUTA `G5`/`G6` (medida por `t63`):** el tablero asigna
`G5` → *"Contratos Sepolia verificados"* (L11) mientras el SKILL define
`GATE-5 FORK-REPLAY-10` (L49); y asigna `G6` → *"Fork replay + invariants"* (L12) mientras el SKILL
define `GATE-6 SEPOLIA-LIVE` (L54). **El mismo número designa cosas distintas en cada documento**,
lo que **descarta cualquier crosswalk implícito por numeración**: los pares `(G5, GATE-5)` y
`(G6, GATE-6)` son **inversos**. Sólo el par `GATE-n NOMBRE` identifica un criterio sin ambigüedad.

### D. Efecto sobre el censo

- **`N = 115` sin cambio.** Los gates `GATE-1`…`GATE-8` son un **universo DISTINTO** del censo
  `AC-USR-*`/`AC-FND-*`/`AC-FRT-*` y **NO se suman a N** (regla R5: no mezclar naturalezas).
  **8 gates nuevos NO convierten `N = 115` en `N = 123`.**
- **`aceptados = 0` sin cambio.** Ninguna fila de la matriz de aceptación se re-clasificó.
- Ningún porcentaje se recalcula. `NO COMPUTADO` **no es un cero ni un conteo**: es una etiqueta de
  ausencia de computación, y por eso **no entra en ningún denominador**.
- Artefacto de referencia: `docs/specification/ACCEPTANCE-MATRIX-v1.md`.
