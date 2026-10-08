# REV-DECIMALS-REBASE-01 (RONDA 2) — Segunda revisión independiente de #927 sobre el head reparado: F1 cerrado, veredicto por criterio

**Orden:** t207 (`review-round-2`, kind **review**) · **Intento:** 1 · **`attempt_id`:** `73e8eabd-7567-484a-a632-a4c8f68c36f3`
**Tarea revisada:** **t206** (repair-round-2) · **PR:** **#927**, rama `w12/decimals-rebase-01`
**Head revisado en ronda 1 (t202):** `b3302cf04e34f8eed796c4947642f2dab1e4872a` → **needs_revision** (1 blocker: CI rojo por clippy).
**Head revisado AHORA (ronda 2):** **`b4d5bde35693907fea1db01965b1e7ed2ade410d`** → commit de reparación `fix(searcher-rs): clippy::cloned_ref_to_slice_refs en orchestrator.rs:3436 (F1-RUST-CI-ROJO)`.
**Estado por el REMOTO:** `state=OPEN`, **`draft=True`**, `mergedAt=''`, base `4077fea5dcca5d29212dc0d76e67f4fc20c66b9c` (sin cambios), 12 ficheros, `+3424/−69`, **`mergeStateStatus: CLEAN`** (en ronda 1 era `BLOCKED`).

> **NO SE ATERRIZA.** El entregable es el dictamen. Cero merge, cero push a main, cero firma, cero broadcast, cero escritura on-chain; el VPS sólo recibió `tar`/`scp` a `/tmp/t202r`, `docker run` y lecturas `SELECT`/`GET`.

---

## 0. VEREDICTO, por criterio (declarado ANTES de medir)

| # | Criterio | Veredicto |
|---|---|---|
| **R1** | BASE re-verificada de forma independiente sobre el head NUEVO | **PASS** |
| **R2** | DIFF = 12 ficheros exactos; 3 tests byte-idénticos a #924; **reparación acotada** | **PASS** |
| **R3** | ★ **F1 (el blocker de t202): ¿CERRADO?** | **PASS — cerrado, con clippy PROPIO en los dos sentidos** |
| **R4** | FALSIFICADOR re-mordido sobre el head NUEVO con **mi** mutante | **PASS** |
| **R5** | DIVERGENCIA `row_token_decimals` sigue cerrada en el código | **PASS** |
| **R6** | RESIDUAL mapa-vs-sello: re-medido, y declaración publicada | **PASS** (severidad baja) |
| **R7** | Corrección a t197 (fixture / alcance del árbol) | **PASS** |
| **R8** | Knobs por query propia | **PASS** |
| **R9** | NO COMPUTADOS declarados por separado | **declarados** (§9) |
| **R10** | El PR NO se aterriza en esta revisión | **CUMPLIDO** |

### ⇒ VEREDICTO GLOBAL: **`pass`**

El único blocker de la ronda 1 está **cerrado y lo verifiqué con mi propio instrumento en los dos sentidos** (§3): `cargo clippy … -- -D warnings` **exit 0** sobre el head reparado, **exit 101** sobre la línea pre-fix restaurada, y **0** otra vez al restaurar. El falsificador vuelve a morder sobre el head nuevo (§4), la base sigue siendo real (§1), el diff sigue siendo el conjunto exacto (§2) y la divergencia sigue cerrada (§5). Los criterios no medidos quedan declarados (§9), no redondeados.

---

## 1. ★ R1 — BASE VERIFICADA (independiente, sobre el head NUEVO)

```
$ git merge-base --is-ancestor 4077fea5dcca5d29212dc0d76e67f4fc20c66b9c b4d5bde35693907fea1db01965b1e7ed2ade410d
exit=0        # la base ES ancestro del head nuevo
$ git merge-base --is-ancestor 858b943f origin/main      →  exit=1   # contrapueba del contrato
$ git merge-base --is-ancestor 858b943f b4d5bde35693907fea1db01965b1e7ed2ade410d  →  exit=1   # la UNION sigue MUERTA
$ git rev-parse origin/main                              →  4077fea5dcca5d29212dc0d76e67f4fc20c66b9c  # base == main actual
```
Sin deuda `behind_by` (a diferencia de #884): la base del PR **es** el `main` de hoy. **PASS.**

## 2. ★ R2 — DIFF: los 12 exactos, y la reparación ACOTADA

```
$ git diff --stat 4077fea5..b4d5bde3   →  12 files changed, 3424 insertions(+), 69 deletions(-)
```
Los 3 ficheros de test, **por blob** contra #924 (`4872d583…`): `d69297a8…` / `fc407a23…` / `87074fc9…` → **IDÉNTICOS** ✔. Cero ficheros extra, cero faltantes.

**La reparación no metió nada más — probado a nivel de BYTE:**
```
$ git show --stat b4d5bde3   →  1 file changed, 1 insertion(+), 1 deletion(-)
-        let got = build_cycle_decimals_map(&[unknown.clone()], &empty_redis, &empty_pg);
+        let got = build_cycle_decimals_map(std::slice::from_ref(&unknown), &empty_redis, &empty_pg);
```
`orchestrator.rs`: blob `cdd9ad25…` → **`9f1c59b4aa4d5fa0c9ce3d19432c44f077f75831`** (sha256 `980c1128d484389c3035cad13f4ab9588fcd5c881767f5c3df8c956de007adde`).
**Prueba extra mía:** revertir **esa única línea** en el fichero del head nuevo produce un fichero cuyo sha256 es `04f4a25aaa4cc9ba7dcc697a614e3a7ca56782c4d89a0d8cf62a752c31cfe9d3` — **exactamente el sha256 del `orchestrator.rs` del head anterior (`b3302cf0`)** ⇒ el commit de reparación **cambió esa línea y sólo esa línea**, en todo el fichero. **PASS.**

## 3. ★★ R3 — F1 CERRADO (y lo medí yo, no sólo el CI)

### 3.1 Por el REMOTO: el CI del head nuevo está ENTERO verde
`gh api repos/hefarica/arbitragex-v2/commits/b4d5bde3…/check-runs` → **38 checks, 38 `completed/success`, 0 no-success**:
```
ci-gate                      :: completed/success :: sha=b4d5bde3
lint-and-test-rust           :: completed/success :: sha=b4d5bde3      <- rojo en b3302cf0
cargo check + clippy + test  :: completed/success :: sha=b4d5bde3      <- rojo en b3302cf0
Rust tests                   :: completed/success :: sha=b4d5bde3
rust-check / rust-unit / Rust integration (live Postgres + Redis) :: success
analyze (rust)               :: completed/success
cargo audit (Rust advisories):: completed/success
```
Contraste con la ronda 1 (mismo PR, head viejo): **32 success / 3 failure / 1 neutral / 1 in_progress**, con los rojos `ci-gate`, `lint-and-test-rust` y `cargo check + clippy + test`. **PASS por el remoto.**

### 3.2 ★ Con MI PROPIO clippy, en los dos sentidos (lo que no hizo la ronda 1)
Entorno: `rust:1.91`, árbol del head en `/tmp/t202r/head` (custodia por `git hash-object`: `orchestrator=9f1c59b4aa4d5fa0c9ce3d19432c44f077f75831` == el del commit). Comando literal:
```
cargo clippy -p searcher-rs -p relays-client --locked --all-targets -- -D warnings
```
| estado del fichero | sha256 | **CLIPPY (mío)** | salida |
|---|---|---:|---|
| **FIX** (`b4d5bde3`) | `980c1128…` | **0** | `Finished dev profile … in 37.43s` (INSTALL_EXIT=0) |
| **M3 = línea pre-fix restaurada** | `04f4a25a…` (**== el viejo**) | **101** | `error: this call to clone can be replaced with std::slice::from_ref` → `--> searcher-rs/src/orchestrator.rs:3436:44` · `= note: -D clippy::cloned-ref-to-slice-refs implied by -D warnings` · `error: could not compile searcher-rs (lib test)` · `(bin "searcher-rs" test)` |
| **restaurado** | `980c1128…` (== head) | **0** | `Finished dev profile … in 40.47s` |

⇒ **El lint mordía exactamente en esa línea y en ninguna otra, y la reparación lo apaga.** No es una afirmación del autor: es mi corrida, con los tres exit codes y los tres sha256 del fichero.

**Defecto de mi propio instrumento, declarado:** en el primer intento (job3) los dos últimos `cargo clippy` salieron **exit 1** con `error: 'cargo-clippy' is not installed for the toolchain` — el componente instalado en un `docker run` **no sobrevive al contenedor**. **Un exit 1 ahí NO es el lint** (el lint da **101**); por eso repetí los tres casos instalando el componente **dentro del mismo contenedor** (job4, §3.2) y **no leí el exit 1 como un fallo del PR**. La lección se registra: *exit 1 ≠ lint denegado*.

**PASS.** El blocker de t202 queda **cerrado**.

## 4. ★ R4 — FALSIFICADOR RE-MOVIDO SOBRE EL HEAD NUEVO (mutante MÍO, no el del autor)

No se heredan los números de t202 (eran de `b3302cf0`): **todo re-medido sobre `b4d5bde3`**. Mi mutante **M1** restaura el default de 18 en el locus reparado (`build_cycle_decimals_map` → tabla canónica con su `_ => 18`), con reemplazo determinista (`OCCURRENCES=1`) y custodia por sha256.

| estado | blob/sha256 del fichero | CHECK (`--no-run`) | **FOCAL** (`decimals_cycle`) | TOTAL (`--no-fail-fast`) |
|---|---|---|---|---|
| **FIX** | `980c1128…` | **0** | **0** — `ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1717 filtered out` | **0** — `ok. 1714 passed; 0 failed; 6 ignored` |
| **M1 (mío)** | `52d986bb478cde52f985b8f107ab8d7050583187cc2c19ba7156547d02b4b7f3` | **0** (0 líneas `^error`) | **101** — `FAILED. 0 passed; 3 failed; …; 1717 filtered out` | **101** — `FAILED. 1710 passed; 4 failed; 6 ignored` |
| **restaurado** | `980c1128…` | — | **0** (`R2_FOCAL_RESTORED_EXIT=0`) | — |

- **Los 4 fallos de M1**: `decimals_cycle_source_precedence_and_route_order`, `decimals_cycle_t187_eight_cases_resolve_to_the_real_unit_not_18`, `decimals_cycle_unknown_token_is_not_computed_never_18` y **`sizing_ledger_and_cycle_map_agree_on_the_same_unit`** — el test que fija **la divergencia que este PR cierra**: con el 18 de vuelta, cae.
- **Conteo TOTAL idéntico**: 1714+0+6 = **1720** (fix) vs 1710+4+6 = **1720** (M1) ⇒ el mutante no añade ni quita tests.
- **Restauración**: sha256 == el del head, y el focal vuelve a **0**.
- **Re-medición del total (no cito los números del autor):** mi `--lib` del head nuevo ejecuta **1714** (1714 passed + 0 failed) + **6 ignorados** = **1720**; coincide con el `1714` que el autor declaró. **`bin 1701` y `19 suites`: NO COMPUTADOS por mí** (corrí sólo `--lib`).
- **Mejora respecto a la ronda 1 (y cierra R7 por ejecución):** el total del FIX pasó de `1713 passed; 1 failed` (ronda 1, copia sin la fixture completa) a **`1714 passed; 0 failed`** — porque en esta ronda el árbol que compilé **incluye las dos fixtures** del golden.

**PASS.**

## 5. ★ R5 — LA DIVERGENCIA SIGUE CERRADA (código, no informe)

En el head nuevo (la reparación sólo tocó una línea de un test, así que el locus está intacto — y lo verifiqué de nuevo):
- `row_token_decimals(address, provider, chain_id)` (`orchestrator.rs:2261-2268`) → `provider?.decimals(chain_id, &addr)` → **`None`**, nunca `Some(18)`.
- `stamp_sized_figures` recibe el **mismo tipo** de provider (call site de producción `:1501-1505` = `self.ctx.token_decimals_provider`) y sin unidad → `warn!(event = "orchestrator.sizing_units_unresolved", reason = "decimals_not_resolved_by_provider_for_sizing_units")` + **`f64::NAN`**.
- **Los dos `1e18_f64` del locus están FUERA**: `git grep -c '1e18_f64' b4d5bde3 -- backend/searcher-rs/src/orchestrator.rs` → **0**. Y el comando del contrato (`grep -rn '1e18_f64' backend/searcher-rs/src`) **sigue dando 2** — las MISMAS dos de la ronda 1, pre-existentes y ajenas: un comentario en `engines/dex_engine.rs:1272` y **código vivo** en `engines/triangular_engine.rs:579`. (Ese `1e18` del motor triangular ya está en su propia tarea: **PR #932**, verificado abajo.)
- El test que **afirmaba** el 18 sigue **reescrito** (no borrado) y afirmando `amount_in.is_nan()`.

**PASS** (con el mismo aviso declarado de la ronda 1 sobre el comando literal del contrato).

## 6. ★ R6 — RESIDUAL DEL SELLO: re-medido, y la declaración está publicada

**Medición propia fresca** (2026-10-08T23:19:44Z → 23:20:09Z, chain 1, sólo lectura):
```
REDIS_DEC_ROWS=2503   PG_ROWS=6761   EN_AMBAS=2503   BOTH_ROWS=2503
REDIS_ONLY=0                           # ningún token existe SÓLO en el catálogo Redis
DISCREPANCIAS_DECIMALS=0 de 2503       # la intersección coincide en TODOS
PG_ONLY=4258
```
⇒ Sigue **no alcanzable** hoy: ni el caso de cobertura (mapa resuelve / sello NO COMPUTED) ni el **numérico silencioso**; y la degradación es un `warn!` propio + `NaN`, **nunca 18**. Severidad: **baja**, con la condición de disparo declarada. (PG pasó de 6759 a 6761 filas entre rondas: el scanner sigue escribiendo; la conclusión no cambia.)

**Y la declaración del autor está PUBLICADA, verificada por mí en el cuerpo de #927** (no en su informe):
```
| claves `arbx:tokens:1:*` (catálogo Redis)      | 2503 |
| filas `tokens` en PG (chain 1)                  | 6759 |
| tokens referenciados por pools (chain 1)        | 3747 |
| **tokens SOLO en Redis**                        | **0** |
| tokens solo en PG                               | 4256 |
| **tokens SOLO en Redis y además en pools**      | **0** |
```
**PASS.**

## 7. ★ R7 — CORRECCIÓN A t197: sigue en pie, y ahora la cierro por ejecución

- Fixtures en el head nuevo: `full.json` blob `6c5833c5eeb93abe8828a9800bf9bb2e487a3e9d` (4844 B) y `knobs-off.json` blob `79d86792ac4bd1ce42d7ceb70a2f41edf1eca71c` (3831 B) — **los dos commiteados** (y presentes también en `main`).
- `golden_fixture_dir()` (`route_discovery/route_discovery_worker.rs:2597-2601`) = `CARGO_MANIFEST_DIR/../../frontend/lib/apex/schemas/__tests__/fixtures/route-discovery-tick` ⇒ **fuera de `backend/`**; el test pide **ambas** (`:2706` y `:2710`).
- **Ejecución propia:** con el directorio de fixtures **completo** en mi árbol, la suite `--lib` del head nuevo da **`1714 passed; 0 failed; 6 ignored`** ⇒ el golden **pasa**; el único fallo de la ronda 1 era `golden knobs-off.json missing` por mi recorte de alcance. **PASS.**
- CI del mismo head: `Rust tests` = `completed/success`.

## 8. ★ R8 — KNOBS (query propia)

`SELECT chain_id, capital_usd, min_profit_usd, spread_sanity_mult, updated_by, updated_at FROM trading_config WHERE chain_id = 1;`
```
1 | 1000.00 | 50.0000 | 3.0000 | admin | 2026-10-08 23:20:01.871743+00
```
**1000 / 50 / 3 intactos** ✔ (el `updated_at` es ~1 min anterior a mi consulta: los valores son el estado **en el momento de medir**; no lo interpreto como otra cosa).

---

## 9. R9 — NO COMPUTADOS (por separado, con su razón)

1. **`bin 1701` y `19 suites`** (números del autor, y del mensaje del commit de reparación): **NO COMPUTADOS por mí** — corrí `-p searcher-rs --lib`. Lo que sí re-medí: `1714` ejecutados + `6` ignorados = **1720**.
2. **Runtime/E2E del ciclo** (searcher → Redis → PG → sim-ctl) con el fix desplegado: **NO COMPUTADO** — el binario desplegado es pre-PR.
3. **Efecto económico del cierre**: **NO COMPUTADO y no afirmado**.
4. **Cadenas distintas de chain 1** en la comparación Redis-vs-PG: **NO COMPUTADO** (medí el catálogo completo de chain 1).
5. **El `fmt --all --check` del autor**: **NO COMPUTADO por mí** (no lo corrí; sí corrí clippy, que es el que bloqueaba).
6. **Los dos `cargo clippy` de mi job3** (M3 y restaurado): **NO COMPUTADOS como evidencia** — salieron `exit 1` por el componente efímero (`cargo-clippy is not installed`), no por el lint. **Sustituidos por mi job4**, que sí los mide con el componente presente (§3.2). *Un exit 1 no es un lint denegado.*

## 10. R10 — EL PR NO SE ATERRIZA

`gh pr view 927` → `state=OPEN`, **`draft=True`**, **`mergedAt=''`**, base `4077fea5` (sin cambios), head `b4d5bde3`. **Cero merge, cero push a main, cero firma, cero broadcast, cero escritura on-chain.** El PR **no se tocó** (ni una línea, ni un comentario en su rama).

---

## 11. Revisión de **t206** (la tarea que se me pidió revisar)

Sus tres afirmaciones, comprobadas por mí contra el REMOTO y los datos:

| afirmación de t206 | verificación independiente | |
|---|---|---|
| «F1 CERRADO en `b4d5bde3`» | CI **38/38 success** en ese head + **mi propio clippy 0 / 101 / 0** (§3) | **CONFIRMADA** |
| «F2 aceptado, declaración publicada en el cuerpo de #927» | las 6 filas están en el cuerpo, leídas por mí (§6) | **CONFIRMADA** |
| «F3 = t205 → PR #932» | `#932 draft=True OPEN head=247f370d files=1` — *«TRIANGULAR-1E18-01: el notional triangular usa la unidad resuelta, no un 1e18 fabricado (camino no-sized)»* | **CONFIRMADA** |
| «decidí NO reparar» | coherente: `sourceFindingIds` = los tres hallazgos ya dispuestos, ninguno nuevo; no tengo evidencia en contra | **ACEPTABLE** |

**Hallazgo de PROCESO (no del PR), con su artefacto — este contrato:**
`In scope:` **vacío** y `Out of scope:` **vacío**, `Verify:` **vacío**, y el `Objective` **fija el head VIEJO** (`…rama w12/decimals-rebase-01 = b3302cf04e34f8eed796c4947642f2dab1e4872a…`) mientras el head real es `b4d5bde3…` (medido: `gh pr view 927 --json headRefOid`). Un revisor que siguiera el contrato al pie de la letra **revisaría un objeto obsoleto y aprobaría un head que ya no existe**. Sumado al truncamiento que t206 declaró en su propio `acceptance` (`Rep Nin Aco`) y al `Verify` de t203 que listaba los comandos de t201, van **tres instancias en rondas consecutivas**: defecto del generador de contratos, no anécdota. **No lo arreglo por mi cuenta; lo declaro con el texto exacto.**

---

## 12. Respuesta directa

- **¿Está re-materializado sobre `main` y no es una unión?** **Sí**: `4077fea5 → head` = exit **0**; `858b943f → head` = exit **1**; `858b943f → origin/main` = exit **1**; base == `origin/main`. (`R1: PASS`)
- **¿El diff es el conjunto exacto de 12 y la reparación está acotada?** **Sí**: 12 ficheros, `+3424/−69`, los 3 tests byte-idénticos a #924 por blob, y el commit de reparación = 1 fichero, `+1/−1`, con la prueba de byte de que **no cambió nada más**. (`R2: PASS`)
- **¿Sigue mordiendo el falsificador sobre la base nueva?** **Sí, con mi mutante**: `CHECK=0` (compila, 0 errores), focal **0 → 101**, total **1720 == 1720**, y el 4.º fallo es el test que pinnea la divergencia; restauración por sha256 y focal de vuelta en **0**. (`R4: PASS`)
- **¿El BLOCKER de la ronda 1 está cerrado?** **Sí, y lo medí yo**: CI **38/38 success** en el head nuevo (contra 3 rojos en el viejo) y **mi propio `cargo clippy --all-targets -- -D warnings`**: **0** en el fix, **101** con la línea pre-fix restaurada (`orchestrator.rs:3436:44`, `cloned_ref_to_slice_refs`), **0** al restaurar. (`R3: PASS`)
- **¿Sigue cerrada la divergencia?** **Sí**: mismo provider, `None`, cero `1e18_f64` en `orchestrator.rs`, `NaN` + evento `orchestrator.sizing_units_unresolved`, test reescrito a `is_nan()`. (`R5: PASS`)
- **¿Se rompió algo más al reparar?** **No**: el diff total no cambió (+3424/−69), los 3 tests siguen idénticos, la suite da **1714/0/6** y no quedan rojos en el CI.

**Dictamen: `pass` sobre el head `b4d5bde35693907fea1db01965b1e7ed2ade410d`.** El PR queda **apto** para el siguiente paso de gobierno, **sin aterrizarlo aquí**. Los NO COMPUTADOS de §9 siguen siendo NO COMPUTADOS.

---

*Revisión independiente (revisor ≠ autor). Árbol medido: COPIA del head en `/tmp/t202r/head` (custodia: `orchestrator=9f1c59b4aa4d5fa0c9ce3d19432c44f077f75831`, `scanner=10bb5dfa9dd7fae724da0e379aa6d05a0f5455b1`). No mergea, no despliega, no firma, no escribe en el fork ni en el VPS.*
