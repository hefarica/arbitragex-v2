# WO-02 — CROSSFIX G5 — gas_price_wei no-cero stringificado en el test del hot-stream

- **Work-order:** WO-02 · **Tipo:** CROSSFIX (fixer del gang, ronda 1) · **Gap fuente:** `WO-02-CROSS.md` §3 G5
- **Charter:** el caso `passed` de `hot_sim_record_maps_outcome_verbatim` nunca ejercitaba un `gas_price_wei` NO-cero stringificado — solo el caso `failed` zeroed asertaba `"0"`; la stringificación U256→String del gas price con valor real quedaba sin assert directo.
- **Fecha de ejecución:** 2026-09-07 (ventana local temprana). Marcadores de diff con la convención de gang `// WO-02 (2026-09-06)` (ID del WO cuyo gap se cierra; fecha del programa — nota fail-honest: la ejecución fue 09-07).
- **Reglas respetadas:** 0 commit/push/PR/deploy (NO-GIT), 0 SSH/VPS, 0 requests HTTP (0/5 presupuesto), edición local + gates SOLAMENTE.

---

## VEREDICTO: **G5 FIXED_VERIFIED** — assert directo sobre la stringificación U256→String de un gas price real (25 gwei + 1 wei), con mutation-check que prueba que el assert tiene dientes.

## 1. El gap (re-verificado antes de tocar)

- Mapper bajo test — `backend/searcher-rs/src/scanner.rs:3233`: `gas_price_wei: outcome.gas_price_wei.to_string()` dentro de `hot_sim_record` (`:3226-3235`), el mapping puro `SimulationOutcome` → registro del stream `arbx:hot:simulated` (`hot_path_emitter.rs:37-42`, `gas_price_wei: String`).
- Test pre-fix — `scanner.rs:3337-3363`: el caso `failed` asertaba `r.gas_price_wei == "0"` (`:3346`); el caso `passed` seteaba `simulated_profit_token_in` (>u128, `:3351-3354`) y `gas_used_total` (`:3355`) pero **jamás** tocaba `gas_price_wei` — quedaba en `U256::zero()` heredado de `SimulationOutcome::failed("unused")` (`round_trip_executor.rs:91`) — y no existía ningún assert sobre `r2.gas_price_wei`. Es decir: el único `.to_string()` de gas price con valor real que consume el XADD wire (`hot_path_emitter.rs:153-180`, WO-02-CROSS §1) pasaba por el test sin ser observado.

## 2. El fix (diff neto +8/−0, todo dentro del bloque WO-02 ya existente)

`backend/searcher-rs/src/scanner.rs`, caso `passed` del test:

- `:3356-3359` — `passed.gas_price_wei = ethers::types::U256::from(25_000_000_001u64);` con marcador `// WO-02 (2026-09-06), cross G5`. 25 gwei + 1 wei: no cabe en u32, no es un gwei redondo, y el `...001` final pinea el último dígito decimal (detectaría cualquier truncamiento/rounding del string).
- `:3367-3369` — `assert_eq!(r2.gas_price_wei, "25000000001");` con marcador ídem. **Literal decimal** (no `passed.gas_price_wei.to_string()`): pinea el formato wire verbatim de forma independiente de `U256::Display`, que un re-derivar con `to_string()` del source no puede falsar.

Se respetó el hint del cross-examiner textualmente (mismo valor, mismo literal esperado).

## 3. Gates (todos re-ejecutados por este fixer, árbol `target/` caliente §36.4)

| Gate | Resultado | Nota |
|---|---|---|
| `cargo fmt -p searcher-rs -- --check` | **EXIT=0** | sin diffs de formato |
| `cargo check -p searcher-rs --quiet` | **EXIT=0** | cero warnings |
| `cargo clippy -p searcher-rs --quiet -- -D warnings` | **EXIT=0** | (los huérfanos WO-10 de publisher.rs reportados por APPLY ya no están — cerrados por su propio respawn) |
| `cargo test -p searcher-rs --bin searcher-rs hot_sim_record` | **1 passed; 1144 filtered out** — EXIT=0 | `test scanner::tests::hot_sim_record_maps_outcome_verbatim ... ok` |
| **Mutation-check (teeth)** | **FAILED como se esperaba** — EXIT=101 | literal mutado `"25000000001"→"25000000002"` ⇒ `assertion left == right failed · left: "25000000001" · right: "25000000002"` — el string real que produce el mapper ES exactamente `25000000001`; el assert no es vacuo. Literal restaurado inmediatamente. |
| No-regresión bin target (`cargo test -p searcher-rs --bin searcher-rs`) | **1142 passed · 0 failed · 3 ignored** (1145 total) — EXIT=0 | idéntico al tally de WO-02-APPLY §5 |
| No-regresión lib target (`cargo test -p searcher-rs --lib`) | **1150 passed · 0 failed · 3 ignored** — EXIT=0 | idéntico al baseline APPLY |

## 4. Disciplina de claims de archivo

- El diff NO toca nada fuera del bloque de test WO-02 (`scanner.rs:3334-3370`).
- **Convivencia documentada, cero conflicto:** el árbol tenía UN hunk residual sin commitear de **WO-04** (`scanner.rs:443-453`, knob `resolve_gas_cost_usd()` del liquidation engine) — intacto, sin tocar. Tras mi fix, `git diff HEAD -- scanner.rs` = hunk WO-04 (7 líneas) + hunk G5 mío (8 líneas), separables por marcador para el promote-batch.
- Árbol/branch al momento del fix: branch `fix/wo15-xinfo-shape` (HEAD `f7ed4cdb` — el árbol avanzó desde el snapshot `a6-cbprom-01`/f7db6867 del arranque de sesión; el test WO-02 ya está commiteado en HEAD vía el gang-batch, mi G5 queda como edición local sin commitear conforme a NO-GIT).
- Anclas WO-10 en scanner.rs (`:1505/1547/2044/2243/2288/2338/2643`): ninguna dentro del span editado.

## 5. Compliance doctrinal

- **RULE 00 / no-hardcode:** PASS — el literal `25_000_000_001` es un vector de regresión dentro de `cfg(test)` (mismo criterio ya aceptado por WO-06-VERIFY: "addresses solo en cfg(test)"), no un valor de operador ni dato de producción; el mapper bajo test sigue trazando 1:1 a `SimulationOutcome` real, sin datos fabricados.
- **R8 fail-honest:** PASS — el test ahora distingue explícitamente los tres registros: failed ⇒ `"0"` (zeroing real del sim que no corrió), passed ⇒ decimal verbatim de lo medido (`"25000000001"`), y el profit >u128 conserva precisión completa. Nada de Some(0.0)-semántica introducida.
- **§32/§33/§34.3:** intactos — cero VPS, cero executor/capital/firma/broadcast; `live_exec_policy.rs` no tocado.
- **NO-GIT:** cero commit/push/PR/deploy. Edición local + gates.

## 6. Presupuesto

0 requests HTTP al dominio público (0/5). 0 SSH. 0 escrituras fuera de este reporte y del diff de código marcado.

## 7. Pendiente para el orquestador (fuera de mi claim)

- Volcar este cierre al board row de WO-02 (G5 de la tabla de gaps del CROSS pasa a FIXED; G1/G3 siguen operator-gated/agent-fixable respectivamente).
- El promote-batch debe separar por marcador los dos hunks coexistentes en scanner.rs (WO-04 `:443-453` vs WO-02-cross-G5 `:3356-3359`+`:3367-3369`).

---

*WO-02 CROSSFIX G5 — 2026-09-07. Fail-honest: gates re-ejecutados desde cero por este fixer (ninguno heredado); mutation-check documenta que el assert nuevo es capaz de fallar; el gap G5 queda cerrado con evidencia file:line.*
