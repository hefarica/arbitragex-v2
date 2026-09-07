# WO-05 — VERIFY (adversarial, frontera de capital)

- **Work-order:** WO-05 · kind: verify · fecha 2026-09-07 (sesión 2026-09-06) · rubric ecc:security-reviewer (+ ecc:rust-reviewer).
- **Objeto verificado:** apply de `WO-05-APPLY.md` (Opción C del diseño): eliminación de `src/executor/` (4 archivos, −350 líneas) + wiring de `NonceManager::refresh` (helper `resync_nonce` + 5 call-sites) en `backend/relays-client/`.
- **Método:** NO confío en el reporte del applier — todos los comandos re-ejecutados desde cero (cargo check/clippy/fmt/test con EXIT codes), todos los diffs re-leídos, censo INDEPENDIENTE de puntos de fuga de nonce post-nonce (encontré 8, no 5).
- **Presupuesto dominio:** 0/0 requests (charter = 0). Sin SSH, sin mutación VPS, read-only local. CERO commits/staging generados por este verify.

---

## 0. Veredicto

**PASS-NO-BLOCK.** El apply es fiel al diseño, los 4 gates re-ejecutados son verdes con EXIT=0, §34.3 está intacto byte a byte, los 4 archivos eliminados eran código muerto con 0 referencias, y el estado git es exactamente el pedido (deletions unstaged, 0 commits WO-05).

**PERO** el censo de puntos de fuga del diseño era incompleto: identifico **3 rutas post-nonce de drop SIN resync** (F1/F2/F3). La más grave (F1, `CallBundleDecision::Drop` — revert en re-sim) es la fuga **rutinaria** en LIVE y produce exactamente el defecto que el WO decía eliminar: acumulación silenciosa de nonces huérfanos hasta reinicio. **Cero riesgo de capital** (nonces demasiado altos no son incluibles: los bundles se descartan pre-broadcast; todos los gates fail-closed intactos) e **inalcanzable en PAPER_SHADOW** (modo actual — paper corta antes del paso 5.5), por eso no bloquea. F1 debe aterrizar antes de cualquier consideración LIVE (flip = operador §34.3 de todos modos).

El claim del WO "atasco silencioso eliminado" queda degradado honestamente a: **"reducido — 5 de 8 rutas post-nonce cubiertas; la más común en LIVE sigue descubierta (F1)"**.

---

## 1. Charter (1) — Forense §34.3: PASS

| Afirmación | Evidencia | Resultado |
|---|---|---|
| diff de `live_exec_policy.rs` == vacío | `git diff -- .../live_exec_policy.rs` → salida VACÍA; además `git diff origin/main -- .../live_exec_policy.rs` → VACÍA (**byte-idéntico a origin/main a0bcf29d**) | ✅ |
| default-deny vivo | `live_exec_policy.rs:62` `matches!(enabled, Some("true"))` — solo el string exacto "true" habilita; `:79-81` `NotEnabled` si no; test `non_true_values_are_disabled` (`:159-166`) cubre `"", "false", "1", "TRUE", "yes", "True", " true "` | ✅ |
| `MainnetRefused` intacto | `live_exec_policy.rs:84-86` rechazo INCONDICIONAL de `chain_id==1` incluso si está en el allowlist; tests `:136-145` y `:148-156` (misconfig explícita `"1"` sigue rechazada); mapping en `bundle_builder.rs:475` sin cambios (fuera del hunks WO-04) | ✅ |
| grep `ARBX_LIVE_EXEC_ENABLED` sin cambios | 4 apariciones: `live_exec_policy.rs:12,32,53` (doc/error/from_env) + `main.rs:192` (mensaje de boot). `main.rs` NO está modificado (`git status --porcelain -- backend/relays-client/` no lo lista); live_exec_policy byte-idéntico ⇒ 0 cambios | ✅ |

**Gates arbx-* colaterales** (riesgo-límites): verificados por lectura del diff — kill-switch (`submit_engine.rs:345-349`), signer fail-closed (`:351-354`), ValidatedPlan TTL fail-closed (`:395-435`), value cap (`bundle_builder.rs:123-135`, PRE-nonce), checklist, callBundle fail-closed, EWMA — todos intactos; el diff WO-05 solo AÑADE llamadas resync en ramas terminales + helper; ninguna decisión es sustituida.

## 2. Charter (2) — Compilación/clippy/test re-ejecutados: PASS (EXIT codes propios)

| Gate | Comando (desde `backend/`) | Resultado propio |
|---|---|---|
| Compile | `cargo check -p relays-client` | `Finished dev profile in 2.06s` — **EXIT=0** |
| Lint | `cargo clippy -p relays-client -- -D warnings` | `Finished in 4.30s` — **EXIT=0** (0 warnings) |
| Test | `cargo test -p relays-client` | **77 passed; 0 failed; 1 ignored** — **EXIT=0** |
| Format | `cargo fmt -p relays-client -- --check` | **EXIT=0** (sin diff) |

Coincide con el reporte del applier (§2) — reproducido de forma independiente. La eliminación NO removió símbolos referenciados (la prueba es el propio `cargo check` verde: si algún archivo vivo importara `executor::*`, fallaría). Nota: `cargo test` reporta "running 78 tests" y `77 passed + 1 ignored` — paridad exacta con el applier.

## 3. Charter (3) — Wiring de refresh: PASS con hallazgos F1/F2/F3

### 3.1 Los 5 call-sites calzan con el diseño (anclas re-leídas en el archivo)

| # | Causa | Ancla verificada | Posición |
|---|---|---|---|
| 1 | `callbundle_abort` | `CallBundleDecision::Abort` | `submit_engine.rs:655-658` — primera sentencia del arm, antes del warn/return `dropped` (:670) ✅ |
| 2 | `all_relays_failed` | `if !broadcast_result.any_success()` | `:688-691` — primera sentencia del bloque ✅ |
| 3 | `inclusion_timeout` | `InclusionOutcome::Dropped` | `:817-820` — arm convertido de expresión a bloque, resync primera sentencia ✅ |
| 4 | `build_error_post_nonce` | arm `Err(e)` genérico de `build_and_sign` | `:467-470` — antes del `return Self::not_submitted(...)` (:471) ✅ |
| 5 | `paper_short_circuit` | bloque `if paper` (paso 5) | `:511-514` — primera sentencia ✅ |

Helper: `submit_engine.rs:926-954`, marcador `:926`, doc-comment fiel al diseño §5. Cuerpo: `self.nonce.as_ref()` (Option, sin panic), `Ok` → `info!(event="nonce.resynced")`, `Err` → `warn!(event="nonce.resync_failed")`. El fetch va por `pool.with_retry` (`nonce_manager.rs:75`) — disciplina HttpRpcPool (circuit breaker + failover EWMA, G-RPC-1) preservada. Marcadores `// WO-05 (2026-09-06)` = exactamente 6 (5 sites :468/:512/:656/:689/:818 + helper :926).

Punto de consumo de nonce verificado: `bundle_builder.rs:155-158` (`nonce_mgr.next()`). Los errores PRE-nonce (LiveExecDenied `:115-117`, UnsupportedStrategy `:119-121`, ValueExceedsCap `:130-135`) ocurren ANTES — su exclusión es correcta; el arm genérico los captura igual y el re-fetch es inocuo-idempotente (declarado en diseño §4, confirmo).

### 3.2 Semántica fail-soft vs la frase del charter ("fail-closed")

El código implementa **fail-SOFT** (diseño §3.3, aplicado fiel): si `refresh` falla → `warn` + el contador local queda adelantado → la EJECUCIÓN SIGUIENTE sí firma con el nonce stale. La frase del charter "resync falla → no firma con nonce stale" NO es lo que hace el código — pero la firma con nonce demasiado alto **no puede gastar capital**: un tx con nonce > nonce de la cuenta no es incluible, se descarta en re-sim/relay pre-broadcast (gas 0). El egress sigue fail-closed; la corrección de caché es fail-soft. No requiere cambio (declarado, no oculto). Adjudico: aceptable — con la salvedad de F1 abajo.

### 3.3 Sin math por-modo (§34.1): PASS

`resync_nonce` no contiene matemática ni branching por modo — es primitiva de corrección de caché idéntica en paper/testnet/live. El site 5 (`if paper`) es wiring de terminus (§34.1.3 permite diferencias de terminus), higiene del contador cuando paper corre con signer. Ningún operador matemático de la Master Matrix tocado.

### 3.4 CENSO ADVERSARIAL INDEPENDIENTE — hallazgos (el diseño censó 5 de 8)

Rutas post-nonce donde el bundle NO aterriza on-chain (fuga de nonce huérfano):

| Ruta | ¿Resync? | Severidad |
|---|---|---|
| `Err(e)` genérico post-nonce de `build_and_sign` | ✅ site 4 | — |
| **`NoSubmitDecision::DropAllSchemasRejected`** (`submit_engine.rs:499-507`) | ❌ **NO** | **F2 MEDIUM** |
| paper short-circuit | ✅ site 5 | — |
| **`CallBundleDecision::Drop`** (re-sim revert) (`submit_engine.rs:576-584`) | ❌ **NO** | **F1 HIGH** |
| `CallBundleDecision::Abort` | ✅ site 1 | — |
| **`multi_relay` == None** (`submit_engine.rs:682-684`) | ❌ **NO** | **F3 LOW** |
| all_relays_failed | ✅ site 2 | — |
| inclusion Dropped | ✅ site 3 | — |
| (Included / Reverted — el nonce SÍ aterrizó on-chain, incluido revertido: consume nonce en cadena) | n/a correcto excluir | — |

**F1 (HIGH, no bloquea) — `CallBundleDecision::Drop` sin resync.** Este es el drop RUTINARIO en LIVE: el propio comentario del código (`submit_engine.rs:552-555`) dice que la re-sim atrapa "route reverts, slippage exceeded which are the majority of preventable wasted-gas losses". Cada disparo orfaniza un nonce. Cascada: con `flashbots_for_callbundle` configurado (config estándar live), los bundles posteriores firman nonces demasiado altos → `eth_callBundle` devuelve error per-tx → clasificador `Drop` de nuevo (`relay_flashbots.rs:154-163` parsea `error`/`revert` per-tx; `any_failed()` `:194-197`; clasificador `submit_engine.rs:971-980`) → **sin resync, loop sin recuperación**: divergencia local vs cadena crece +1 por ejecución, terminus inerte hasta reinicio/coincidencia (Abort por outage de endpoint, o `flashbots_for_callbundle=None` → broadcast → `all_relays_failed` → resync → sanado). Es EXACTAMENTE el "atasco silencioso hasta reinicio del proceso" que el diseño §4 decía eliminar — por eso el claim del WO se degrada. **Cero riesgo de capital**: nonces demasiado altos no son incluibles ni reemplazan txs (replacement exige MISMO nonce); los gates fail-closed están intactos. **Inalcanzable en PAPER_SHADOW actual** (paper retorna en paso 5, antes de 5.5; y en paper la clasificación 4.5 es `LogOnly` — `classify_no_submit` `:1002-1008`). **Remedio (pre-LIVE, 3 líneas, mismo patrón):** `resync_nonce(opp.chain_id, signer.address, "callbundle_reverted")` como primera sentencia del arm `Drop` (:577), + su marcador WO. Recomiendo enrutarlo vía el PR del operador o un WO-05b; NO lo apliqué yo (mi rol es verify; los archivos están bajo claim del WO-05 pero el apply está cerrado y el charter me pide verificar, no extender).

**F2 (MEDIUM) — `DropAllSchemasRejected` sin resync** (`:499-507`). Post-nonce (el bundle ya firmó en paso 4), drop pre-egress. Disparo raro (rechazo de forma por las 3 schemas wire en no-paper; los bundles de `build_and_sign` son well-formed) — pero cuando dispara, entra en la misma cascada de F1. Remedio idéntico (primera sentencia del arm, cause `"relay_no_submit_all_schemas_rejected"`).

**F3 (LOW) — `multi_relay_not_configured` post-nonce sin resync** (`:682-684`). Si `multi_relay` es None, NADA puede broadcastarse nunca (config estática del boot); el contador queda creciendo +1/ejecución pero es irrelevante hasta que un reinicio (que resetea el caché de nonce de todos modos) reconfigure el relay. Cosmético.

**Recuento honesto: 5/8 rutas de fuga cubiertas.** Las 5 del diseño calzan; las 3 faltantes son el gap del CENSO del diseño (no infidelidad del applier — el apply implementó exactamente lo diseñado).

## 4. Charter (4) — Los 4 archivos eliminados eran solo código muerto: PASS

| Grep (en `backend/`, `--include="*.rs"`) | Resultado |
|---|---|
| `mod executor` | **0 hits** (la declaración jamás existió — consistente con diseño §1.2 `git log --all -S` vacío) |
| `LiveTestnetExecutor \| check_or_insert \| IdempotencyChecker` | **0 hits** |
| `ExecutionReceipt \| ExecutionState` (símbolos de mod.rs) | **0 hits** fuera de `target/` |
| `src/executor` (rutas) | **0 hits** |
| `executor::` | único hit: `bundle_builder.rs:284` `prioritization_spine::round_trip_executor::RoundTripContext` — falso positivo (módulo de otro crate, entidad distinta; predicho por el diseño §1.2) |
| `GasOracle` | solo entidades homónimas de OTROS crates: `sed-core/src/connectors/gas_oracle.rs:33` y `searcher-rs` `GasOracleWorker` — ninguna referencia al `executor/gas_oracle.rs` eliminado |

Directorio físicamente inexistente (`ls src/ | grep -c executor` → 0). Y la prueba estructural: `cargo check -p relays-client` EXIT=0 post-delete — si algo vivo lo referenciara, no compilaría.

## 5. Charter (5) — Estado git: PASS

- `git status --porcelain -- backend/relays-client/`: ` D` ×4 (executor/, **unstaged** — espacio-D, no D-espacio), ` M` nonce_manager.rs, ` M` submit_engine.rs, ` M` bundle_builder.rs (drift WO-04). `git diff --cached` → **VACÍO** (nada staged). ✅
- **0 commits WO-05**: `git log origin/main..HEAD` contiene 3 commits — `6d9d7830` (.gitleaksignore), `b7ca27f6` (WO-15 hotfix websocket.ts), `f7ed4cdb` (migración 119) — `git diff --name-only origin/main..HEAD` confirma que tocan SOLO `backend/api-server/src/websocket*.ts` + `database/migrations/119_*.sql`. **Ninguno toca relays-client** (verificado con grep `wo-05` en log → vacío). Esos 3 commits son trabajo de otros WOs/operador, fuera de mi claim — los reporto como observación O-3.
- Branch actual: `fix/wo15-xinfo-shape` (§36.2 — árbol compartido conmutado por otro agente; el PR futuro del operador debe partir de `git branch --show-current` verificado en ese momento, como ya declara el applier §5.2).

## 6. Drift coexistente WO-04 (cuarentena verificada)

`git diff` de `submit_engine.rs` contiene EXACTAMENTE 1 línea WO-04 (`:446` `self.cfg.execution.priority_fee_gwei, // WO-04 (2026-09-06)`) + los 5 hunks WO-05; `bundle_builder.rs` es 100% WO-04 (5 hunks: 8º parámetro + priority_fee desde config, barrera LiveExecPolicy `:110-117` intacta como contexto no-modificado). Coincide con la declaración del applier §5.1. Sin contaminación cruzada.

## 7. R8 — No computado / declarado honesto

- **Runtime del resync SIN ejercicio**: ningún test cubre el camino runtime de `resync_nonce` (grep `resync` en tests → 0; requiere seam RPC o anvil-fork — parked en diseño §5). El estado `APPLIED` con runtime-unverified del applier se sostiene. Señales de observabilidad vivas: `nonce.resynced` (info) / `nonce.resync_failed` (warn).
- **Tests NO añadidos por el WO**: los 77 tests son pre-existentes (classifiers puros). Aceptable bajo el charter del diseño (§3.3 declaró el límite), pero el PR del operador para F1/F2 debería idealmente añadir un test del clasificador Drop→resync-ordering si se extrae a función pura.
- **Presupuesto dominio**: 0 requests ejecutados (charter 0). Sin SSH a VPS.

## 8. Tabla resumen de defectos

| ID | Severidad | Bloquea | Descripción | Remedio |
|---|---|---|---|---|
| F1 | **HIGH** | NO (liveness, no capital; inalcanzable en paper; flip LIVE = operador §34.3) | `CallBundleDecision::Drop` (revert de re-sim, fuga RUTINARIA en LIVE) sin resync → cascada de nonces demasiado altos sin recuperación hasta reinicio | 3 líneas en arm Drop `:577` — pre-LIVE obligatorio, PR operador o WO-05b |
| F2 | MEDIUM | NO | `DropAllSchemasRejected` (`:499`) post-nonce sin resync (disparo raro) | ídem, arm A7 |
| F3 | LOW | NO | `multi_relay_not_configured` (`:682`) post-nonce sin resync (config-estático; reinicio reseta caché) | opcional |
| O-1 | observación | — | Charter pedía "fail-closed" del resync; implementado fail-soft (como el diseño §3.3 declaró). Firma con nonce stale NO puede gastar capital (no-incluible). Sin cambio requerido | documentar |
| O-2 | observación | — | Drift WO-04 coexistente correctamente cuarentenado (1 línea submit_engine + bundle_builder) | PR WO-04 aparte |
| O-3 | observación | — | 3 commits locales más allá de origin/main (WO-15/migr-119/gitleaks) — NO tocan relays-client, NO son WO-05; disciplina §36 a cargo del operador | operador |
| O-4 | observación | — | Runtime del resync sin test (anclaba ya en diseño §5 "Qué NO se hace") | follow-up anvil-fork |

## 9. Estado final

- **WO-05: PASS-NO-BLOCK** — apply fiel al diseño Opción C, gates 4/4 reproducidos (EXIT=0 ×4), §34.3 byte-intacto, código muerto eliminado con 0 referencias, git limpio (unstaged, 0 commits).
- **Claim a degradar**: "atasco silencioso eliminado" → "reducido (5/8 rutas)"; F1 (arm Drop de re-sim) es la fuga rutinaria en LIVE y debe cerrarse ANTES de cualquier consideración de flip (que es operador-only §34.3 de cualquier forma).
- Capital expuesto por este verify: 0. Requests a dominio: 0. Mutaciones git/VPS: 0.
