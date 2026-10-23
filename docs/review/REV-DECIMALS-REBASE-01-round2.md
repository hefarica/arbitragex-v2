# REV-DECIMALS-REBASE-01 — RONDA 2 (t204) — revisión independiente de **#927** tras la reparación de t203

**Objeto:** PR **#927** `DECIMALS-REBASE-01`, rama `w12/decimals-rebase-01`, **head `b4d5bde35693907fea1db01965b1e7ed2ade410d`** (era `b3302cf0…` en la ronda 1), base **`main` = `4077fea5…`**.
**Contexto:** ronda 1 (t202) → **NEEDS_REVISION** por **F1** (bloqueante). t203 reparó. Esta ronda re-mide todo **sobre el head nuevo**.
**Entregable:** este dictamen. **NO se aterriza #927** (`state=OPEN`, `isDraft=true`, `mergedAt=null`).

## VEREDICTO GLOBAL: **PASS** — F1 cerrado y verificado por el CI; el resto del PR sigue en pie y el falsificador vuelve a morder sobre el head reparado

| # | criterio | ronda 1 | **ronda 2** |
|---|---|---|---|
| C1 | Base verificada (ancestro + contraprueba) | PASSED | **PASSED** (+ fast-forward del head) |
| C2 | Diff auditado (12 ficheros) + 3 tests byte-idénticos a #924 | PASSED | **PASSED** |
| C3 | **F1: `Rust CI`** | **FAIL (blocker)** | **PASSED — CERRADO** (fallo→verde en el mismo gate) |
| C4 | Falsificador con mutante PROPIO del revisor | PASSED | **PASSED** (re-hecho sobre el head reparado) |
| C5 | Divergencia cerrada leída en código | PASSED | **PASSED** |
| C6 | Residual del sello, medido | PASSED (F2 bajo) | **PASSED** (+ 1 discrepancia de definición declarada) |
| C7 | Corrección a t197 verificada | PASSED | **PASSED** |
| C8 | Knobs intactos por query propia | PASSED | **PASSED** |
| C9 | NO COMPUTADOS declarados aparte | 3 | **3** (los mismos, con la razón) |
| F3 | `/1e18_f64` fuera del locus | informativo | **confirmado ECONÓMICO + PRE-EXISTENTE en main** ⇒ fuera de #927, tarea propia |

---

## C1 — BASE VERIFICADA (independiente)

```
$ git merge-base --is-ancestor 4077fea5dcca5d29212dc0d76e67f4fc20c66b9c b4d5bde35693907fea1db01965b1e7ed2ade410d
exit=0        # el head NUEVO sigue re-materializado sobre main
$ git merge-base --is-ancestor b3302cf04e34f8eed796c4947642f2dab1e4872a b4d5bde3…
exit=0        # la reparación entró por FAST-FORWARD: no reescribió historia
$ git merge-base --is-ancestor 858b943f origin/main
exit=1        # contraprueba: 858b943f NO es ancestro de main
```
`git log --oneline b3302cf0..b4d5bde3` → **un solo commit** `b4d5bde3 fix(searcher-rs): clippy::cloned_ref_to_slice_refs en orchestrator.rs:3436 (F1-RUST-CI-ROJO)`. **PASSED.**

## C2 — DIFF AUDITADO

```
$ git diff --stat 4077fea5…..b4d5bde3…      ->  12 files changed, 3424 insertions(+), 69 deletions(-)
$ git diff --stat b3302cf0…..b4d5bde3…      ->   1 file changed, 1 insertion(+), 1 deletion(-)
```
**Sigue siendo el conjunto EXACTO**: 5 de código (`orchestrator.rs`, `scanner.rs`, 3 tests) + `docs/loop/DECIMALS-CYCLE-01.md` + 6 artefactos. **La reparación no añadió ni quitó ficheros.** Los 3 tests, **por blob**, siguen **byte-idénticos a #924**:

| fichero | blob en `b4d5bde3` | blob en #924 | idéntico |
|---|---|---|---|
| `tests/cartridge_shadow_replay.rs` | `d69297a843a6…` | `d69297a843a6…` | **SÍ** |
| `tests/orchestrator_parallel_run.rs` | `fc407a238afe…` | `fc407a238afe…` | **SÍ** |
| `tests/v2_shadow_replay.rs` | `87074fc97a74…` | `87074fc97a74…` | **SÍ** |

## C3 — **F1 CERRADO**, y lo cierra el mismo gate que lo abrió (no la palabra del autor)

**La reparación es exactamente lo que el hallazgo pedía** (`orchestrator.rs:3436`, 1 línea):
```diff
-        let got = build_cycle_decimals_map(&[unknown.clone()], &empty_redis, &empty_pg);
+        let got = build_cycle_decimals_map(std::slice::from_ref(&unknown), &empty_redis, &empty_pg);
```
**La medición independiente (CI del repositorio, no el contenedor del autor):**

| head | workflow "Rust CI" (job `cargo check + clippy + test`) |
|---|---|
| `b3302cf0` (pre-fix) | run **37852377663** → **conclusion=FAILURE (2m30s)** · `Process completed with exit code 101` |
| **`b4d5bde3` (post-fix)** | run **37856088020** → **conclusion=success (3m21s)** |
| `main` (control de fondo) | 37783695176 success (77b42b3d) · 37728772313 success (8414e512) |

⇒ **el fallo desaparece con el cambio que el dictamen anterior exigió, sobre ese gate y ese head.** **CERRADO.** **PASSED.**

## C4 — FALSIFICADOR RE-MEDIDO SOBRE EL HEAD REPARADO, con mutante PROPIO (ronda 2)

**Mutante del revisor, regenerado sobre `b4d5bde3`** (mismo criterio que en la ronda 1: la rama `None` de `build_cycle_decimals_map` vuelve a insertar el default fabricado):
```diff
+                // MUTANTE DEL REVISOR (t204, ronda 2): restaura el default fabricado de 18
+                // en el locus del MAPA. NO ES EL FIX y NO se mergea.
+                map.insert(lc.clone(), 18);
```
Ejecutor: **el CI**, vía PR **#930** (DRAFT *NO MERGE*, **cerrado** al leer el resultado).

| lado | job / run | target searcher-rs (lib) | conclusion |
|---|---|---|---|
| **FIX `b4d5bde3`** | `Rust tests` run **37856088082** / job 113580492469 | `running 1720 tests` → **`ok. 1714 passed; 0 failed; 6 ignored`** | **pass (1m27s)** ⇒ FOCAL_EXIT=**0** |
| **MUTANTE `7a95204b`** | `Rust tests` run **37856868033** / job 113583043852 | `running 1720 tests` → **`FAILED. 1712 passed; 2 failed; 6 ignored`** | **fail (1m36s)** ⇒ FOCAL_EXIT=**101** |

```
MUTANTE — fallos por nombre y aserción (log crudo):
 test orchestrator::tests::decimals_cycle_source_precedence_and_route_order ... FAILED
 test orchestrator::tests::decimals_cycle_unknown_token_is_not_computed_never_18 ... FAILED
 thread '…source_precedence_and_route_order' panicked at searcher-rs/src/orchestrator.rs:3483:
   assertion `left == right` failed: sin fuente no hay entrada
 thread '…unknown_token_is_not_computed_never_18' panicked at searcher-rs/src/orchestrator.rs:3441
```
*(Las líneas del panic son del **árbol del mutante**, que lleva 3 líneas insertadas por el revisor por encima; en el **árbol del PR** las aserciones están en `orchestrator.rs:3480` —`assert_eq!(got.map.map.get(&c), None, "sin fuente no hay entrada")`— y `:3445` —`"el token sin fuente no puede aparecer en el mapa"`—, verificadas por `git grep` en `b4d5bde3`.)*

- **Conteo TOTAL idéntico en ambos lados: `running 1720 tests`** (1714+0+6 y 1712+2+6 = **1720**) ⇒ el mutante cambia una implementación, **no añade ni quita tests**.
- **El mutante COMPILA** ⇒ `CHECK_EXIT=0`: el fallo es un `assert` **dentro** del binario de test ya construido (que reporta `1712 passed`), no un error de compilación. Lo confirma por otra vía el otro job del mismo head, que **compiló lib y bin-test** antes de fallar por lint (o de pasar, en el fix).
- **Los números del autor NO se citan como propios**: el `lib 1714` lo **re-medí** yo en el log del CI de este head; el **`bin 1701`** y las **19 suites** quedan **NO MEDIDOS** por mí (ver §C9).

**PASSED.**

## C5 — DIVERGENCIA CERRADA (leída en el código de `b4d5bde3`)

```rust
// orchestrator.rs:2261-2268
fn row_token_decimals(address: &str,
    provider: Option<&Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>>,
    chain_id: u64) -> Option<u8> {
    let addr = address.parse::<Address>().ok()?;
    provider?.decimals(chain_id, &addr)        // 2267 — None si el provider no tiene fila, NUNCA 18
}
```
- **Los dos fallbacks `1e18` del locus, eliminados**: `git grep -n '1e18_f64' b4d5bde3 -- …/orchestrator.rs` → **cero coincidencias** (en `main` estaban en 2196 y 2204).
- **NO COMPUTADO con la reason**: `event = "orchestrator.sizing_units_unresolved"` en **2352** (notional → `f64::NAN`) y **2373** (salida medida).
- **El test que AFIRMABA el 1e18 fue REESCRITO, no borrado** (`orchestrator.rs:2807-2822`): misma fixture (`legacy_seed`, `three_units`), ahora `stamp_sized_figures(…, no_provider.as_ref(), 1)` + `assert!(amount_in.is_nan(), "without a resolved unit the notional is NOT COMPUTED (NaN)…")` + `assert_ne!(amount_in, 3.0, "the 1e18 fallback IS the divergence t201 closes — it must be gone")`.
- **Los 4 tests del ciclo** siguen presentes: `row_decimals_resolves_through_the_provider_never_to_18` (2851), `decimals_cycle_t187_eight_cases_resolve_to_the_real_unit_not_18` (**3390**, el focal de los 8 tokens), `decimals_cycle_unknown_token_is_not_computed_never_18` (3432), `decimals_cycle_source_precedence_and_route_order` (3460).

**PASSED** (con la salvedad de alcance de F3, §Hallazgos).

## C6 — RESIDUAL DEL SELLO: re-medido en la ronda 2, y **una discrepancia de definición declarada**

**Construcción:** mapa = **Redis primero, PG después** (`orchestrator.rs:222`); sello = **solo `PgTokenDecimalsProvider`** (`main.rs:606-610`). Para un token **solo-Redis**: el mapa resuelve y el sello cae en **NO COMPUTADO declarado** (reason + `NaN`), nunca en 18.

**Medición propia (chain 1, ronda 2):**

| medida | valor |
|---|---|
| catálogo Redis `arbx:tokens:1:*` con `decimals` | **2503** |
| `tokens` PG chain 1: filas / con `decimals` no nulo | **6759 / 6758** |
| en AMBAS fuentes | **2503** |
| **solo en Redis** | **0** |
| solo en PG | **4255** |
| **solo-Redis ∩ pools** (radio de acción del residual) | **0** |
| tokens de pools activos **sin** fila PG con decimals | **0** |

**★ Discrepancia con el número de t203, resuelta y declarada:** t203 reporta **"tokens de pools 3747"**; yo mido **914**. **No es un error de ninguno de los dos: es la definición.** Cacé la variante:
```
a) distinct tokens en pools chain1 ACTIVOS          =   914     <- mi número
b) distinct tokens en pools chain1 (activos o no)   =  3747     <- el de t203
c) distinct tokens en pools TODAS las chains        =  3747
d) filas de pools chain1 activos                    =  1363
e) filas de pools TODAS las chains                  =  4288
```
**Lo decisivo es robusto a la definición:** como **solo-Redis = 0**, la intersección con *cualquier* conjunto de pools es **0** ⇒ **el residual F2 no tiene radio de acción medible hoy**, en los dos alcances. Y su consecuencia sería **declarada** (NO COMPUTADO con reason), no silenciosa. **F2 se mantiene en severidad BAJA / aceptado.** (El `6759` de t203 vs mi `6758` es la fila de `tokens` con `decimals` NULL: filas vs no-nulos. Declarado.)

## C7 — CORRECCIÓN A t197, RE-VERIFICADA en el head nuevo

| afirmación | verificación propia | resultado |
|---|---|---|
| la fixture está commiteada | `git ls-tree -r b4d5bde3 -- frontend/lib/apex/schemas/__tests__/fixtures/route-discovery-tick/` → `full.json` blob `6c5833c5…` (+ `knobs-off.json`) | **SOSTENIDA** |
| `golden_fixture_dir()` la busca **fuera de `backend/`** | `route_discovery_worker.rs:2597-2601`: `CARGO_MANIFEST_DIR` + `../../frontend/lib/apex/schemas/__tests__/fixtures/route-discovery-tick` | **SOSTENIDA** |
| el fallo es artefacto de **ALCANCE del árbol** (`git archive -- backend`) | con ese `archive` la ruta `../../frontend/…` no puede existir, y el test hace `panic!("golden {name} missing …")` (`:2694`) | **SOSTENIDA** |
| contraste con el CI **en el mismo head** | **`Rust tests` = pass (1m27s)** en `b4d5bde3` (run 37856088082) — medido por mí | **CONFIRMA** |

**PASSED.**

## C8 — KNOBS INTACTOS (query propia, ronda 2)

```
$ docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
  "SELECT 'capital_usd='||capital_usd||' min_profit_usd='||min_profit_usd||' spread_sanity_mult='||spread_sanity_mult FROM trading_config WHERE chain_id=1"
capital_usd=1000.00 min_profit_usd=50.0000 spread_sanity_mult=3.0000      exit=0
```
**PASSED.** Esta revisión **no escribió** en PG ni en Redis (solo SELECT/scan/mget) y **no tocó el VPS**.

## C9 — CRITERIOS **NO COMPUTADOS** (declarados aparte; no se redondean a passed ni a failed)

1. **Compilación EN LOCAL: NO COMPUTADO.** El VPS no tiene `cargo`/`rustc` (`which` exit 1; `cargo --version` → 127), no hay `target/` caliente (`/opt/arbitragex-v2/backend/target` no existe, exit 2) y el disco está al **93 % con 11 GB libres** con `postgres_data` de **66 GB** ⇒ compilar ahí ponía en riesgo producción. **Razón exacta**; el criterio se cumplió por el **CI del repositorio como ejecutor independiente** (C3 y C4), que es *más* independiente que mi shell.
2. **Conteo de la suite `bin` (el "1701" del autor) y las "19 suites": NO MEDIDOS por mí.** El job del CI que usé corre `cargo test --lib`. **No se citan como propios.**
3. **Los números de la tabla del autor sobre SU árbol** (lib 1713+1 failed, bin 1700+1 failed): **no reproducidos por mí**; mi medición es la del CI (**1714 / 0 failed / 6 ignored**). Se declaran **no verificados** en su árbol.

## HALLAZGOS — estado en la ronda 2

| id | ronda 1 | ronda 2 |
|---|---|---|
| **F1-RUST-CI-ROJO** | **BLOCKER** (`Rust CI` rojo por línea del propio PR) | **CERRADO** — el mismo gate pasó a **success** con la línea reparada (C3) |
| **F2-ASIMETRIA-MAPA-SELLO** | low (latente) | **ACEPTADO con medición**: solo-Redis = 0 ⇒ radio de acción 0 en los dos alcances (C6); consecuencia declarada, nunca 18 |
| **F3-1e18-FUERA-DEL-LOCUS** | informativo | **CONFIRMADO ECONÓMICO y PRE-EXISTENTE EN `main`** ⇒ **fuera de #927**, tarea propia (ver abajo) |

**F3, precisado (no bloquea #927):** `triangular_engine.rs:578-580` no es cosmético —
```rust
let amount_in_f64 = amount_in_wei.map(|w| u256_to_f64(&w) / 1e18_f64).unwrap_or(0.0);   // 578-580
… Some(p) if p > 0.0 => amount_in_f64 + gross_profit_usd.unwrap_or(0.0) / p,             // 618
… amount_in: amount_in_f64,                                                              // 631 (Opportunity)
… amount_in: Some(amount_in_f64),                                                        // 653 (OpportunityCandidate)
```
alimenta `amount_in` en **las dos capas** del candidato ⇒ **la misma clase de error que #927 cierra para el 2-hop sigue viva en la ruta triangular**, con factor `10^(18−d)` para un token de entrada de `d ≠ 18` decimales. **Pero está en `main` desde antes** (`git grep` en `4077fea5` → presente, exit 0) y **fuera del locus declarado de #927** ⇒ **no es defecto de este PR**: es **tarea propia** (y con severidad **media** como item independiente, no baja, porque toca el económico).

---

## CIERRE

- **#927 NO se aterriza en esta revisión**: `state=OPEN`, `isDraft=true`, `mergedAt=null`, head `b4d5bde3`. **Cero merge, cero push a `main`, cero firma, cero broadcast, cero escritura on-chain, cero reinicio.**
- El único push de esta revisión fue la **rama del mutante** (`review/t204-mutant-18-map`), publicada como **PR #930 DRAFT "NO MERGE"** y **CERRADA** al leer el CI (`state=CLOSED`, `mergedAt=null`). El PR de la ronda 1 (#928, mismo propósito) también quedó **CLOSED**.
- `inScope` respetado: este dictamen vive en `docs/review/`.

```bash
git hash-object docs/review/REV-DECIMALS-REBASE-01-round2.md
git rev-parse HEAD:docs/review/REV-DECIMALS-REBASE-01-round2.md
```

---

*Ronda 2: **PASS**. El bloqueante de la ronda 1 se cerró con la línea exacta que el dictamen pedía y **lo confirmó el mismo gate que lo abrió** (Rust CI: failure→success sobre el head reparado), el falsificador **vuelve a morder** sobre el head nuevo con un mutante mío ejecutado por el CI —**mismo total 1720, mutante que compila**—, la divergencia sigue cerrada en el código con el test reescrito y no borrado, el residual queda medido (0 casos, en los dos alcances) y la corrección a t197 se sostiene. Lo único que queda abierto —el `/1e18_f64` de `triangular_engine`, que **sí** alimenta el económico— **ya estaba en `main`** y pide su propia tarea, no una ampliación de este PR.*
