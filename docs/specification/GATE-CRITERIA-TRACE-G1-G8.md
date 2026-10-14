# GATE-CRITERIA-TRACE-01 — Auditoría de trazabilidad G1-G8: fidelidad de criterios y existencia de artefactos

- **Rol:** Specification (SSOT documental, fuentes, trazabilidad y contratos de especificación)
- **Tarea:** t59 — GATE-CRITERIA-TRACE-01 · Equipo `arbx-publicacion-desbloqueo-02`
- **Naturaleza:** auditoría de **fidelidad y trazabilidad**. **NO** es re-medición de gates (eso es `t54`, SRE).
- **Dictamina, no corrige.** No se modificó el SKILL.md ni el documento de estado (verificación en §8).
- **SHA auditado:** `origin/main` = `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`. Mediciones en worktree aislado en ese commit exacto.

## Insumos auditados (hashes medidos, no citados)

| Insumo | Ruta | blob git | sha256 de contenido |
|--------|------|----------|---------------------|
| **Reporte** (a auditar) | `audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md` | `61cc176af1170b9a17424a0c34b932d863a50b81` | `910794ff88985c36ddd1665ac1febd9b…` |
| **Fuente declarada** de los criterios | `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` | `60e79f908407ce174aa16d91142209c082cc002f` | `6b04826dfb0ca7f7571303255d25ccdf…` |

El reporte declara en L2: *"Fuente de los criterios: `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` (v2.0.0)."* Ambas rutas resuelven en `origin/main` (`git cat-file -e origin/main:<ruta>` → exit 0). **La fuente declarada existe y es localizable: eso no está en discusión. Lo que está en discusión es si el reporte es fiel a ella.**

---

## 0. Veredicto en una línea

**El reporte NO es fiel a su fuente declarada.** Los 8 criterios del SKILL.md v2.0.0 y los 8 criterios de la tabla del estado son **dos conjuntos de criterios distintos con la misma numeración**, sin crosswalk declarado en ninguna de las dos partes; **2 gates del SKILL (GATE-1 DATA-INTEGRITY-V3 y GATE-8 MAINNET-CANARY) no tienen ninguna fila** en el estado; y de los 2 `PASS` de la tabla, **el artefacto de G5 no está en el repo ni en el remoto** y **el de G8 es la circularidad del propio paquete**.

**Severidad global: ALTA.** El documento que dice cumplir §34.5.3 —*"artefactos reproducibles, JAMÁS claims"*— contiene una celda de estado PASS/OK cuyo único sostén es una frase.

---

## 1. Fidelidad de criterios — tabla de trazabilidad

Comando base: `git show origin/main:<ruta>` sobre ambos insumos. `criterion_id` = identificador estable del gate en el reporte.

### 1.1 GATES del SKILL.md v2.0.0 (la fuente declarada)

| Gate (SKILL) | Línea SKILL | Nombre / criterio | Requisito literal |
|--------------|-------------|-------------------|-------------------|
| `GATE-1` | L27–31 | **DATA-INTEGRITY-V3** | Manifest V3 con **318 pools** verificados on-chain, **90 candidatos**; **2 firmas humanas** sobre el hash del manifest + **backup PostgreSQL probado** + **restore verificado**; blocker SSH exit 255 |
| `GATE-2` | L33–37 | **SIMULATION-CYCLIC-C2C3** | S4 simula **ciclos COMPLETOS (2–5 hops)**, no probes de un swap; **10 casos fork-replay <1 % desviación**; blocker `tx_builder.rs:51-79` rechaza ciclos |
| `GATE-3` | L39–42 | **PAPER-4-LAYERS** | PnL de 4 capas (predicted→simulated→observed_market→realized); decimales reales, precio ETH dinámico (no 3500 fijo) |
| `GATE-4` | L44–47 | **SUBMIT-ENGINE-DURABLE** | Máquina de estados **7 fases**; `idempotency_key` determinista, **UNIQUE INDEX PostgreSQL**, sin dedup por ventana de 60 s |
| `GATE-5` | L49–52 | **FORK-REPLAY-10** | **10 bloques mainnet históricos** replayados; **<1 % desviación** en montos, gas y neto; contrastado contra resultados on-chain reales |
| `GATE-6` | L54–57 | **SEPOLIA-LIVE** | `ArbitrageExecutor.sol` deployado, inicializado, **UUPS proxy verificado**; circuito de detección→quote→sim→decide→execute→reconcile con gas real; **circuit breaker probado + pausa de emergencia funcional** |
| `GATE-7` | L59–61 | **A.9-REINFORCED** | Checklist A.9 (G7) con artefactos reproducibles: max notional, max loss, min profit, slippage, gas ceiling, bribe ceiling, expiry blocks, RPC quorum, stale-quote y reorg protection **todos configurados y probados** |
| `GATE-8` | L63–66 | **MAINNET-CANARY** | Primera operación con `max_loss` parametrizado; capital en riesgo **≤ $350**, principal flash **5 WETH**, reserva de gas separada; reconciliación **<1 % desviación** |

### 1.2 Filas del ESTADO (el reporte) y su correspondencia REAL

| `criterion_id` | Línea ESTADO | Criterio (resumen) | Estado | Evidencia citada | ¿Fuente en el SKILL? |
|----------------|--------------|--------------------|--------|------------------|----------------------|
| `G1` | L7 | Deploy veraz / infra | ⚠️ PARCIAL | commits `fdb40401`,`125b1e0b`,`dcfe890c` | **NINGUNA** — el SKILL no define un gate "deploy veraz / infra" |
| `G2` | L8 | Simulación cíclica (≥1 sim passed) | ❌ FAIL | `SELECT COUNT(*) FROM simulations WHERE passed` = 0 | **PARCIAL** — corresponde al GATE-2 del SKILL, pero **omite** "2–5 hops", "10 fork-replay <1 %" |
| `G3` | L9 | Paper→submit engine con ciclo real | ❌ FAIL | `COUNT(*) FROM executions` = 0; ledger 598K REJECTED | **FUSIÓN de dos gates**: GATE-3 (PAPER-4-LAYERS) + GATE-4 (SUBMIT-ENGINE-DURABLE). Omite 4 capas de PnL, máquina de 7 fases, UNIQUE INDEX |
| `G4` | L10 | Net-profit gate on-chain honesto | ⚠️ SIN EJERCITAR | "Gate en código (G-ECON cerrado 2026-08)" | **NINGUNA** — criterio no definido en el SKILL |
| `G5` | L11 | Contratos Sepolia verificados | ✅ (histórico) | "Contrato defi verificado (deploy previo)" | **PARCIAL** — corresponde al GATE-6, pero **omite** UUPS proxy, circuit breaker probado, pausa de emergencia |
| `G6` | L12 | Fork replay + invariants | ⚠️ PARCIAL | anvil-1 healthy; SIM-FUND-01 (`125b1e0b`) | **PARCIAL** — corresponde al GATE-5, pero **omite** los 10 bloques y el **<1 % de desviación** |
| `G7` | L13 | Risk-limits + checklist pre-ejecución | ⚠️ CÓDIGO OK / DRILL FALTA | `pre_execute_checklist.rs` presente | **PARCIAL** — corresponde al GATE-7, pero reduce 10 controles a "check 1 = kill-switch" |
| `G8` | L14 | Acta + paquete activación | ✅ HOY | "Este paquete (ACTA + RUNBOOK + este estado)" | **NINGUNA** — corresponde a un artefacto de proceso, **no** al GATE-8 del SKILL |

### 1.3 Gates de la fuente declarada que NO tienen fila en el reporte

| Gate del SKILL | Criterio | ¿Fila en el ESTADO? | Consecuencia |
|----------------|----------|---------------------|--------------|
| `GATE-1` | **DATA-INTEGRITY-V3** (318 pools, 90 candidatos, 2 firmas, backup+restore, SSH exit 255) | **NO** | Verificación cruzada (§1.4): **0/11** palabras clave del criterio presentes en el reporte. El gate **desapareció del tablero** |
| `GATE-8` | **MAINNET-CANARY** (≤$350, 5 WETH, max_loss parametrizado, reconciliación <1 %) | **NO** | El `G8` del reporte es "Acta + paquete activación", un criterio **distinto**. El canary real, el gate de mayor consecuencia económica, **no se reporta** |

**Ninguno de los 8 nombres del SKILL** (`DATA-INTEGRITY-V3`, `SIMULATION-CYCLIC-C2C3`, `PAPER-4-LAYERS`, `SUBMIT-ENGINE-DURABLE`, `FORK-REPLAY-10`, `SEPOLIA-LIVE`, `A.9-REINFORCED`, `MAINNET-CANARY`) aparece en el reporte: **0 de 8**.

### 1.4 El desmentido documental: el reporte contradice a un informe previo del propio repo

`audits/real-cycles-audit-20260916/GOAL-WORKORDERS.md:19` (WO-05, estado `DONE`) **sí** usa la numeración del SKILL y ya había dictaminado los 8 gates:

> `G1 data-integrity: #573 mergeado+deployado PERO 0 firmas en manifest verificadas, 0 restore-test (artifacts/ sin attestation) → NO PASS. G2 cíclica: fix escrito pero NO merged → NO PASS. G3 paper-4-capas: no ejercitada (passed=0) → NO PASS. G4 submit-engine: executions=0, jamás exercised → NO PASS. G5 fork-replay-10: sin artefactos → NO PASS. G6 Sepolia: sin evidencia on-chain verificada esta sesión → NO PASS. G7 A.9: dashboard dice "sign-off pending" → NO PASS. G8 canary: bloqueado por G1-G7.`

Ese informe cubre **los 8 gates del SKILL**, con veredicto por gate, y declara `G1 data-integrity → NO PASS`. **El reporte auditado (fechado un día después, 2026-09-17) no contiene G1 data-integrity en absoluto.** El criterio no fue "simplificado de más": fue **sustituido** por otro con el mismo número.

---

## 2. Existencia de artefactos — ¿algún PASS vive sin artefacto?

### 2.1 Conteo de referencias resolubles en toda la tabla

Medición sobre el reporte completo (`git show origin/main:…GATES-G1-G8-ESTADO.md`):

| Clase de referencia | ¿Presente en el reporte? |
|---------------------|--------------------------|
| Dirección `0x…` (40 hex) | **NO** |
| tx hash `0x…` (64 hex) | **NO** |
| URL `http(s)://` | **NO** |
| Ruta `archivo.ext:línea` | **NO** |
| SHA de commit de 8 hex | **SÍ** (`a06a968d`, `fdb40401`, `125b1e0b`, `dcfe890c`) |
| Nombre de archivo suelto | **SÍ** (`pre_execute_checklist.rs`) |
| Consulta SQL | **SÍ** (2) |

**Clases de artefacto citadas por celda:** G1=[sha] · G2=[query] · G3=[query] · **G4=[NINGUNA]** · **G5=[NINGUNA]** · G6=[sha] · G7=[ruta] · **G8=[NINGUNA]**.

### 2.2 Los 2 `PASS` de la tabla — verificación medida

El contrato de la tarea identifica los PASS como **G5** (`✅ (histórico)`) y **G8** (`✅ HOY`). Dictamen por gate:

| Gate | Declarado | Artefacto citado | ¿Existe en repo/remoto? | Veredicto |
|------|-----------|------------------|-------------------------|-----------|
| **G5** | ✅ (histórico) | *"Contrato defi verificado (deploy previo); revalidar al SHA nuevo antes del flip"* | **NO** | **PASS SIN ARTEFACTO — HALLAZGO ALTO** |
| **G8** | ✅ HOY | *"Este paquete (ACTA + RUNBOOK + este estado)"* | 3/3 archivos existen, pero **la evidencia es el propio paquete** | **PASS CIRCULAR — HALLAZGO ALTO** |

**G5 — detalle de la medición.** La celda **no cita SHA, ni dirección, ni URL, ni tx hash, ni ruta**. Búsqueda ejecutada en el worktree:
- artefactos de deploy Sepolia (`*sepolia*.{json,md,txt,log}`): **2 hallados**, ambos documentación (`docs/m5-sepolia-runbook.md`, `docs/superpowers/plans/2026-07-17-smoke-test-sepolia-e2e.md`), **ninguno es attestation de deploy del contrato del proyecto**;
- directorios `broadcast/` de Foundry (raw tx de deploy): **0**;
- direcciones `0x…40hex` asociadas a Sepolia en `contracts/ docs/ backend/`: **todas son de terceros** (Aave V3 Pool `0x6Ae43d…`, WETH Sepolia, QuoterV2) — **ninguna es el `ArbitrageExecutor` del proyecto**.

**No existe, en el repo ni en el remoto, un artefacto que sostenga el PASS de G5.** El propio paquete lo admite por omisión: `ACTA-CAPACIDAD-LIVE.md` dice *"Contrato defi deployado + verificado"* — otra vez una frase, sin dirección ni tx.

**G8 — detalle de la medición.** Los 3 archivos del paquete existen en `origin/main` (`EXISTE` por `git cat-file -e`). Pero la celda de evidencia de G8 apunta al propio paquete que la contiene. **La circularidad no es un artefacto de verificación**: un paquete que se declara completo a sí mismo no acredita ninguna capacidad técnica. Y el mismo paquete lo desmiente: `RUNBOOK-ACTIVACION-MAINNET.md:4` dice **"Estado actual: NO PASS (ver GATES-G1-G8-ESTADO.md)"** — es decir, el RUNBOOK lee la tabla del ESTADO y concluye **NO PASS**, mientras la propia tabla marca G8 como `✅ HOY`.

### 2.3 Referencias citadas: resolución individual

Comando: `git ls-remote origin <ref>` y `git cat-file -t <sha>`.

| Referencia | Citada por | Medición | Veredicto |
|------------|-----------|----------|-----------|
| `a06a968d` | ESTADO G1 | `cat-file -t` → `commit`; `2026-09-16 21:51:05`; *Merge PR #576* | **RESUELVE** |
| `fdb40401` | ESTADO G1 | `commit`; *SEL-GATE-01*; punta de `refs/heads/fix/sel-gate01-20260917` | **RESUELVE** |
| `125b1e0b` | ESTADO G1, G6 | `commit`; *SIM-FUND-01*; punta de `refs/heads/fix/sim-fund01-20260917` | **RESUELVE** |
| `dcfe890c` | ESTADO G1 | `commit`; *PANCAKE-ROUTER-01* | **RESUELVE** |
| `b99c834` | **SKILL** L22 (`Current HEAD`) | `commit`; `2026-09-14 23:38:58`; *Merge PR #569* | **RESUELVE** |
| `fix/v3-slot0-coverage-20260917` | **ESTADO L19** | `git ls-remote origin 'refs/heads/fix/v3-slot0-coverage-20260917'` → **vacío**; ningún ref con `slot0` | **NO RESUELVE — HALLAZGO ALTO** |
| `codex/567-canonical-plan-simulation` | **SKILL** L34 (precedente) | `ls-remote` exacto → vacío; glob `codex/567*` → vacío; ningún ref `codex/*` con `567` | **NO RESUELVE** (es el precedente, correctamente documentado) |
| `9a10350` | **SKILL** (precedente) | `cat-file -t` → `fatal: Not a valid object name` | **NO RESUELVE** (es el precedente) |
| `0e72a7cc` | WO-01 (contexto) | `commit` | RESUELVE |
| `verify-pipeline-567` (workflow) | **SKILL** L10 | `origin/main:.github/workflows/verify-pipeline-567.yml` → `fatal: path … does not exist` | **NO RESUELVE — HALLAZGO MEDIO** |

**Nota de honestidad (R8).** `fix/v3-slot0-coverage-20260917` **no resuelve en el remoto consultado**, y tampoco existe un ref alternativo con `slot0`. Esto **no prueba por sí solo** que la rama nunca existió: prueba que **hoy no es resoluble** en `origin`, y por tanto que la afirmación *"El trabajo YA está en curso en `fix/v3-slot0-coverage-20260917`"* **no es verificable desde el repo**. La distinción entre "borrada", "nunca empujada" y "empujada a otro remoto" **requiere un artefacto que no tengo**; no la resuelvo por prosa.

### 2.4 Rutas citadas por el SKILL

| Ruta | ¿Existe en `origin/main`? |
|------|---------------------------|
| `backend/relays-client/src/submit_engine.rs` | EXISTE |
| `backend/api-server/src/paper/executor.ts` | EXISTE |
| `contracts/script/DeploySepolia.s.sol` | EXISTE |
| `contracts/script/DeployMainnet.s.sol` | EXISTE |
| `backend/sim-ctl/src/tx_builder.rs` | EXISTE |
| `backend/sim-ctl/src/sim_engine.rs` | EXISTE |
| `backend/sim-ctl/src/canonical_plan_consumer.rs` | EXISTE |
| `backend/shared-rs/src/pre_execute_checklist.rs` | EXISTE |
| `backend/sim-ctl/src/submit_engine.rs` | **AUSENTE** (el SKILL lo lista bajo FIX-4; el real vive en `relays-client/`) |
| `scripts/00-preflight-checks/` `01-deploy-contracts/` `02-execute-canary/` | **AUSENTES** (el SKILL L11 ya lo declara: *"NO existen aún"*) |

**El bloque de artefactos del SKILL resuelve bien** (8/12 rutas; las 4 ausentes están declaradas como ausentes por el propio SKILL). El problema de trazabilidad **no está en la fuente: está en el reporte.**

### 2.5 Artefactos que SÍ existen (crédito donde corresponde)

Para no convertir esto en un descarte indiscriminado: **G1, G6 y G7 citan referencias reales y resolubles.**
- Los 4 commits existen, son `commit` válidos, y **los 4 son ancestros de `origin/main`**.
- `pre_execute_checklist.rs` **existe en `origin/main`** y **contiene** el kill-switch que la celda dice: L90 `/// Kill-switch is armed in Redis`, L91 `#[error("kill_switch active")]`, L15 `1-2 : Operator intent (kill_switch, paper_mode)`.
- El paquete G8 existe íntegro (3/3 archivos).

El defecto **no es que el reporte invente referencias** — de hecho no inventó ninguna. El defecto es **qué cuentan como evidencia**: presencia de un commit y presencia de un archivo no acreditan que un gate PASE.

---

## 3. G2 frente a su precedente

**Precedente (SKILL L34, citado por el propio reporte en L24-25):** *"G2 del skill v2.0.0 citó evidencia fabricada una vez (rama codex/567 + commit 9a10350 inexistentes, verificado 2026-09-15)."*

**Dictamen de Specification — el precedente se reproduce por medición independiente:**

| Referencia del precedente | Medición | Resultado |
|---------------------------|----------|-----------|
| rama `codex/567-canonical-plan-simulation` | `git ls-remote origin 'refs/heads/codex/567*'` y filtro de todos los refs `codex/*` | **∅ vacío** — no resuelve |
| commit `9a10350` | `git cat-file -t 9a10350` | `fatal: Not a valid object name` |
| rama hermana `fix/567-canonical-plan-consumer` | `git ls-remote` | **RESUELVE** → `99153e1a…` |

**Conclusión:** la rama fabricada **no existe en el remoto** y el commit fabricado **no es un objeto válido**. **La afirmación del precedente es verdadera y verificable.** Además, existe una rama hermana real (`fix/567-canonical-plan-consumer`) que hace el episodio plausible como confusión de nombre con un objeto genuino — sin que eso excuse haberlo citado como evidencia.

**¿El `FAIL` actual de G2 se sostiene con artefacto?**

**SÍ, con reserva cuantificada.** La celda declara: *"`SELECT COUNT(*) FROM simulations WHERE passed` = **0** (toda la historia, query 2026-09-17)"*.

- **Lo que la sostiene:** es una consulta SQL reproducible, con predicado explícito y ventana declarada ("toda la historia"). Su dirección de error es la correcta: un conteo que da `0` es un **FAIL honesto**, no un PASS fabricado. Un FAIL no necesita tanta evidencia como un PASS (no hay capital que autorizar).
- **La reserva:** el reporte **no adjunta el resultado crudo** ni un id de consulta. *"`COUNT(*)` = 0"* como frase **no es** el artefacto; el artefacto sería la salida de la query. Sin ella, la cifra es **no re-verificable desde el reporte**.
- **Severidad: MEDIA**, no alta. La asimetría es deliberada y correcta en principio: el modo de fallo peligroso es un PASS sin artefacto (§2.2), no un FAIL sin artefacto.

**El `FAIL` de G2 está bien orientado y su método es reproducible, pero su evidencia no está adjunta.** Que el gate haya citado evidencia fabricada una vez hace que esta reserva no sea un tecnicismo: el mismo G2, el mismo número, la misma tabla.

---

## 4. Hallazgos

| ID | Sev. | Hallazgo | Evidencia medida | Fix requerido |
|----|------|----------|------------------|---------------|
| **H-1** | **ALTA** | **G5 `✅` sin artefacto alguno.** La celda no cita SHA, dirección, URL, tx hash ni ruta; no existe attestation de deploy Sepolia en repo ni remoto | §2.2: 0 `broadcast/`, 0 direcciones del proyecto, celda con `[NINGUNA]` clase de artefacto | Adjuntar dirección + tx + link de verificación, o degradar el estado a `SIN ARTEFACTO` |
| **H-2** | **ALTA** | **G8 `✅` circular, y contradicho por su propio paquete.** La evidencia es el paquete que la contiene; `RUNBOOK-ACTIVACION-MAINNET.md:4` dice "NO PASS" | §2.2; `git show origin/main:…/RUNBOOK-ACTIVACION-MAINNET.md` L4 | G8 no puede declararse por auto-referencia. Exigir artefacto externo o retirar el `✅` |
| **H-3** | **ALTA** | **El reporte no es fiel a su fuente declarada: 0/8 nombres del SKILL presentes; numeración distinta sin crosswalk.** Consecuencia: **GATE-1 DATA-INTEGRITY-V3 y GATE-8 MAINNET-CANARY desaparecen del tablero** | §1.1–1.3: 0/8 nombres; `GATE-1` no aparece; 0/11 palabras clave de DATA-INTEGRITY | Declarar el crosswalk explícito o rotular la tabla como universo propio con nombre distinto |
| **H-4** | **ALTA** | **Regresión de criterio sobre un gate de capital.** `GOAL-WORKORDERS.md` (09-16) registra `G1 data-integrity → NO PASS` (0 firmas, 0 restore-test). El reporte (09-17) **no contiene ese gate** | §1.4 | Restaurar GATE-1 DATA-INTEGRITY-V3 como fila propia con su veredicto |
| **H-5** | **ALTA** | **Fusión de dos gates en uno con pérdida de criterio.** `G3` funde GATE-3 (PAPER-4-LAYERS) y GATE-4 (SUBMIT-ENGINE-DURABLE); se pierden las 4 capas de PnL, la máquina de 7 fases y el UNIQUE INDEX | §1.2 | Desdoblar G3 en dos filas |
| **H-6** | **ALTA** | **Referencia no resoluble: la rama que el reporte cita como "trabajo en curso" no existe en el remoto** | §2.3: `ls-remote` vacío, sin ref alternativo con `slot0` | Adjuntar el SHA de la rama, o corregir la afirmación |
| **H-7** | MEDIA | **Los 2 FAIL (G2, G3) sostienen su veredicto en consultas SQL no adjuntas.** Método reproducible, artefacto ausente | §3 | Adjuntar salida cruda de las queries con timestamp |
| **H-8** | MEDIA | **Criterio no definido en la fuente: `G4` "Net-profit gate on-chain honesto"** no existe en el SKILL; su evidencia ("G-ECON cerrado 2026-08") no apunta a ningún artefacto localizable | §1.2: clase `[NINGUNA]` | Citar el artefacto del cierre G-ECON o declarar el origen real del criterio |
| **H-9** | MEDIA | **El workflow `verify-pipeline-567` que el SKILL cita como verificación "en curso" no existe** en `origin/main` | §2.3: `fatal: path … does not exist in 'HEAD'` | Retirar la cita o reponer el workflow |
| **H-10** | BAJA | `backend/sim-ctl/src/submit_engine.rs` citado por el SKILL no existe; el real está en `relays-client/` | §2.4 | Corregir la ruta en el SKILL |

**Sin hallazgo:** los 4 commits de G1, el hash de G6, la ruta de G7 y los 3 archivos del paquete G8 **resuelven correctamente**. El reporte **no fabricó referencias**; el defecto es de **autoridad de lo citado**, no de existencia.

---

## 5. Lo que esta auditoría NO declara

- **No re-mido los gates.** No consulté PostgreSQL, no corrí `simulations`/`executions`, no verifiqué on-chain. Eso es `t54` (SRE). Aquí se audita la **relación** criterio↔reporte y la **existencia** de lo citado.
- **No declaro que G5 sea FALSO**, declaro que **no está sostenido por artefacto**. Son afirmaciones distintas: "no encontré el artefacto" ≠ "el deploy no ocurrió".
- **No declaro que la rama `fix/v3-slot0-coverage-20260917` nunca existió**, declaro que **no resuelve hoy en `origin`** (H-6).
- **No corrijo** el SKILL.md ni el reporte. Se dictamina.
- **No evalúo** si el estado `PARCIAL`/`SIN EJERCITAR` es correcto: eso requiere re-medición.

---

## 6. Efecto sobre P/N — declaración explícita

**Esta auditoría NO mueve P/N.**

- `N = 115` permanece **congelado** en `scope_version = arbx-scope-1.0.0` (`docs/specification/ACCEPTANCE-MATRIX-v1.md` §0).
- `aceptados = 0` permanece **sin cambio**. Los 8 gates G1-G8 son un **universo de criterios distinto** del censo `AC-USR-*`/`AC-FND-*`/`AC-FRT-*`. **No se suman a N** (regla R5: no mezclar naturalezas).
- Los hallazgos H-1…H-10 son **hallazgos de trazabilidad documental**, no unidades del censo de aceptación. Ninguna fila de la matriz se re-clasificó.
- **Ningún porcentaje se recalcula**: recalcular un porcentaje con un denominador nuevo es exactamente el autoengaño que la matriz existe para impedir.

---

## 7. Fronteras que entrego

| Frontera | A quién | Por qué no es mía |
|----------|---------|-------------------|
| Re-medir G1-G8 (sims, executions, on-chain, fork replay) | **SRE (`t54`)** | Requiere acceso a PG/RPC/runtime, no a documentación |
| Decidir si el gate G1-G8 se **re-congela como universo propio** de la matriz | **Planner + operador** | Cambia el alcance del censo; N sólo cambia por bump de `scope_version` |
| Autorizar o denegar el flip a `LIVE_MAINNET` | **Operador** | §34.5. Ningún PASS de esta tabla lo habilita, con o sin artefacto |
| Reponer/retirar la rama `fix/v3-slot0-coverage-20260917` | **Repo admin / SRE** | Estado real de la rama, no trazabilidad documental |
| Adjuntar los artefactos faltantes (G5, G8, queries) | **Autor del reporte / SRE** | Specification dictamina: no produce evidencia de runtime |

---

## 8. Declaración de no-modificación de los artefactos auditados

**Ningún artefacto auditado fue modificado.** Verificación:

| Artefacto | Blob en `origin/main` | ¿Modificado por esta tarea? |
|-----------|----------------------|----------------------------|
| `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` | `60e79f908407ce174aa16d91142209c082cc002f` | **NO** — fuera de alcance (`.claude/skills/`) |
| `audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md` | `61cc176af1170b9a17424a0c34b932d863a50b81` | **NO** — fuera de alcance (`audits/`) |
| `audits/real-cycles-audit-20260916/GOAL-WORKORDERS.md` | (sólo leído) | **NO** |
| `audits/live-activation-package-20260917/ACTA-CAPACIDAD-LIVE.md` | (sólo leído) | **NO** |
| `audits/live-activation-package-20260917/RUNBOOK-ACTIVACION-MAINNET.md` | (sólo leído) | **NO** |

**Todo el trabajo de escritura ocurrió en `docs/specification/`** (el único ámbito en alcance), en un **worktree aislado** en `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`, sobre la rama `docs/gate-criteria-trace-01`. **No hubo merge. No hubo push a `main`.**

**Guardia §22.20 respetada.** Un run de `auto-deploy-vps.yml` sobre `main` estaba **`in_progress`** durante toda la ejecución de esta auditoría — medido dos veces, la segunda como re-verificación:

```
createdAt 2026-10-06T02:48:51Z  headSha c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e  status in_progress
```

Con la guardia viva, **no se empujó nada a `main`**. El resultado se propone por rama + PR y **no se mergea**.

---

## 9. Comandos de reproducción

```bash
# Identidad del SHA auditado
git rev-parse origin/main                                    # c89d21a3…

# Los dos insumos, hasheados
git rev-parse origin/main:audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md
git rev-parse origin/main:.claude/skills/arbitragex-v2-mainnet-live/SKILL.md

# Existencia de referencias citadas
git cat-file -t a06a968d fdb40401 125b1e0b dcfe890c b99c834   # todos: commit
git cat-file -t 9a10350                                        # fatal: Not a valid object name
git ls-remote origin 'refs/heads/codex/567*'                   # vacío
git ls-remote origin 'refs/heads/fix/v3-slot0-coverage-20260917'  # vacío
git ls-remote origin 'refs/heads/fix/567-canonical-plan-consumer' # 99153e1a…

# Existencia de rutas citadas
git cat-file -e origin/main:backend/shared-rs/src/pre_execute_checklist.rs   # exit 0
git cat-file -e origin/main:.github/workflows/verify-pipeline-567.yml        # exit 128
git cat-file -e origin/main:backend/sim-ctl/src/submit_engine.rs             # exit 128

# Alcance de la tabla de estados
git show origin/main:audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md \
  | grep -cE '0x[0-9a-fA-F]{40}|https?://'                     # 0
```

---

*Auditoría de fidelidad, no de corrección. `N = 115` sin cambio; `aceptados = 0` sin cambio. Los artefactos auditados se dictaminan, no se editan.*
