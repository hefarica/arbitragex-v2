# REV-DECIMALS-REBASE-01 (t202) — revisión independiente de **#927** con el listón de #884

**Objeto:** PR **#927** `DECIMALS-REBASE-01`, rama `w12/decimals-rebase-01`, **head `b3302cf04e34f8eed796c4947642f2dab1e4872a`**, base **`main`**.
**Rol del revisor:** independiente (no soy el autor de #927 ni de sus artefactos). Ningún criterio se aprueba por prosa.
**Entregable:** este dictamen. **NO se aterriza #927** (sigue `OPEN` + `DRAFT`, `mergedAt=null`).

## VEREDICTO GLOBAL: **NEEDS_REVISION** — 1 hallazgo bloqueante de CI, el resto del PR se sostiene

| # | criterio | veredicto |
|---|---|---|
| C1 | Base verificada de forma independiente (ancestro + contraprueba) | **PASSED** |
| C2 | Diff auditado contra los 12 ficheros intencionales; 3 tests byte-idénticos a #924 | **PASSED** |
| C3 | Falsificador re-mordido con **mutante PROPIO del revisor**, sobre la base nueva | **PASSED** |
| C4 | Divergencia `row_token_decimals` cerrada (leída en código, no en el informe) | **PASSED**, con la salvedad literal de F3 |
| C5 | Residual del sello sync (mapa-vs-sello) revisado y declarado | **PASSED**, con hallazgo F2 (bajo) |
| C6 | Corrección a t197 verificada de forma independiente (fixture + alcance + CI) | **PASSED** |
| C7 | Knobs intactos por query propia | **PASSED** |
| C8 | El PR no se aterriza en esta revisión | **DECLARADO** |
| C9 | Criterios NO COMPUTADOS, declarados aparte | **3 declarados** (ver §C9) |
| **F1** | **`Rust CI` ROJO en el head, verde en main, por una línea que el PR AGREGA** | **BLOCKER** |

---

## C1 — BASE VERIFICADA (independiente, no la afirmación del autor)

```
$ git merge-base --is-ancestor 4077fea5dcca5d29212dc0d76e67f4fc20c66b9c b3302cf04e34f8eed796c4947642f2dab1e4872a
exit=0                                  # SÍ es ancestro: el PR está re-materializado sobre main
$ git merge-base --is-ancestor 858b943f origin/main
exit=1                                  # contraprueba: 858b943f NO es ancestro de main
$ git merge-base --is-ancestor 858b943f b3302cf04e34f8eed796c4947642f2dab1e4872a
exit=1                                  # ni del head del PR
```
**Y la prueba de que NO sigue siendo una UNIÓN sobre `fix/perhop-reserves-01`:**
```
$ git diff --name-only 4077fea5dcca5d29212dc0d76e67f4fc20c66b9c..b3302cf0… | wc -l   ->  12
$ git diff --name-only 858b943f..b3302cf0… | wc -l                                  ->  404
```
Si el PR siguiera apilado sobre la rama vieja, el diff contra `858b943f` sería **casi vacío**; son **404 ficheros** ⇒ el contenido del PR es un **delta limpio sobre `main`**, no una unión. **PASSED.**

## C2 — DIFF AUDITADO CONTRA EL CONJUNTO INTENCIONAL

```
$ git diff --stat 4077fea5…..b3302cf0…     (12 files changed, 3424 insertions(+), 69 deletions(-))
 backend/searcher-rs/src/orchestrator.rs                      |  589 +-
 backend/searcher-rs/src/scanner.rs                           |   10 +
 backend/searcher-rs/tests/cartridge_shadow_replay.rs         |    1 +
 backend/searcher-rs/tests/orchestrator_parallel_run.rs       |    1 +
 backend/searcher-rs/tests/v2_shadow_replay.rs                |    1 +
 docs/loop/DECIMALS-CYCLE-01.md                               |  380 +
 docs/loop/decimals-cycle-01/DECIMALS-CYCLE-01.patch          | 1855 +
 docs/loop/decimals-cycle-01/apply-decimals-cycle-01.py       |  458 +
 docs/loop/decimals-cycle-01/chain.sh                         |   24 +
 docs/loop/decimals-cycle-01/fixup-dec01-tests.py             |   42 +
 docs/loop/decimals-cycle-01/mutate-decimals-cycle-01.py      |   68 +
 docs/loop/decimals-cycle-01/verify-run.sh                    |   64 +
```
**Cotejo con el conjunto declarado: 5 de código** (`orchestrator.rs`, `scanner.rs` + los 3 tests) **+ 1 doc** (`docs/loop/DECIMALS-CYCLE-01.md`) **+ 6 artefactos** (`.patch`, `apply-*.py`, `chain.sh`, `fixup-*.py`, `mutate-*.py`, `verify-run.sh`) **= 12**. **Cero ficheros de más, cero de menos.**

**Los 3 ficheros de test, por BLOB (no por inspección visual), contra #924 (head `4872d583…`) — que sigue CLOSED con base `fix/perhop-reserves-01`:**

| fichero | blob en #927 = blob en #924 | blob en main | idéntico a #924 |
|---|---|---|---|
| `backend/searcher-rs/tests/cartridge_shadow_replay.rs` | `d69297a843a6ce78d564e218ea96581a6a292447` | `d0d601f2…` | **SÍ** |
| `backend/searcher-rs/tests/orchestrator_parallel_run.rs` | `fc407a238afe86391f53e55a43b0d41c59cd9f70` | `0bf323ae…` | **SÍ** |
| `backend/searcher-rs/tests/v2_shadow_replay.rs` | `87074fc97a74758c1cb750be9288c4444b29df57` | `a7b9e04e…` | **SÍ** |

**PASSED.**

## C3 — FALSIFICADOR RE-MORDIDO CON MUTANTE **PROPIO DEL REVISOR**

**No usé los mutantes del autor.** Construí el mío: en `build_cycle_decimals_map` (el locus del MAPA) la rama `None` vuelve a insertar el default fabricado:

```
$ git diff   (rama review/t202-mutant-18-map, 803e10a1)
+                // MUTANTE DEL REVISOR (t202): restaura el default fabricado de 18 …
+                map.insert(lc.clone(), 18);
```
**Ejecutor:** el CI del repositorio (runner de GitHub), **no** mi shell y **no** el VPS. Para dispararlo abrí el PR **#928** (DRAFT, *NO MERGE*), y lo **cerré al leer el resultado**.

| lado | job | resultado del target `searcher-rs` (lib) | conclusion |
|---|---|---|---|
| **FIX** `b3302cf0` | `Rust tests` — run `37852377771` / job `113568279802` | `running 1720 tests` → **`test result: ok. 1714 passed; 0 failed; 6 ignored`** | **pass (1m33s)** ⇒ FOCAL_EXIT=**0** |
| **MUTANTE** `803e10a1` | `Rust tests` — run `37853398385` / job `113571705448` | `running 1720 tests` → **`test result: FAILED. 1712 passed; 2 failed; 6 ignored`** | **fail (1m25s)** ⇒ FOCAL_EXIT=**101** |

```
MUTANTE — fallos por nombre (log crudo del job):
 test orchestrator::tests::decimals_cycle_unknown_token_is_not_computed_never_18 ... FAILED
 test orchestrator::tests::decimals_cycle_source_precedence_and_route_order ... FAILED
 failures:
 thread 'orchestrator::tests::decimals_cycle_source_precedence_and_route_order' panicked at
   searcher-rs/src/orchestrator.rs:3484: assertion `left == right` failed: sin fuente no hay entrada
 thread 'orchestrator::tests::decimals_cycle_unknown_token_is_not_computed_never_18' panicked at
   searcher-rs/src/orchestrator.rs:3442
```

- **El conteo TOTAL es idéntico en ambos lados: `running 1720 tests`** (1714+0+6 y 1712+2+6 = **1720**) ⇒ el mutante **no añade ni quita tests**, cambia una implementación.
- **El mutante COMPILA**: el fallo es un `assert` **dentro** del binario de test ya construido (que reporta `1712 passed`), no un error de compilación; y el otro job (`cargo check + clippy + test`) **compiló lib y bin-test** antes de fallar por un *lint*, lo que lo prueba por una segunda vía.
- **Los números del autor (lib 1714 / bin 1701 / 19 suites) NO se citan como propios**: el 1714 lo **re-medí** en el CI (coincide); **el de `bin` no lo medí ⇒ NO COMPUTADO** (el job del CI corre `cargo test --lib`; ver §C9).

**PASSED** — el falsificador muerde sobre la **base nueva**, con mutante mío y ejecutor independiente.

## C4 — DIVERGENCIA CERRADA (leída en el código del head, no en el informe)

**(a) `row_token_decimals` resuelve por el provider y devuelve `None`, nunca 18:**
```rust
// orchestrator.rs:2261-2268  (head b3302cf0)
fn row_token_decimals(address: &str,
    provider: Option<&Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>>,
    chain_id: u64) -> Option<u8> {
    let addr = address.parse::<Address>().ok()?;
    provider?.decimals(chain_id, &addr)      // None si el provider no tiene fila
}
```

**(b) Los DOS fallbacks `1e18` del locus reparado, ELIMINADOS** (presentes en la base, ausentes en el head):
```
$ git grep -n '1e18_f64' origin/main -- backend/searcher-rs/src
origin/main:…/orchestrator.rs:2196:  None => …u256_to_f64_lossy(ledger.amount_in_wei) / 1e18_f64,
origin/main:…/orchestrator.rs:2204:      None => …u256_to_f64_lossy(out_wei) / 1e18_f64,
$ git grep -n '1e18_f64' b3302cf0 -- backend/searcher-rs/src
b3302cf0:…/engines/dex_engine.rs:1272:  // The blanket `/ 1e18_f64` was not "wrong as a constant" …   (COMENTARIO)
b3302cf0:…/engines/triangular_engine.rs:579: .map(|w| u256_to_f64(&w) / 1e18_f64)        (CÓDIGO VIVO, otro locus)
```
**★ El comando literal del contrato `grep -rn '1e18_f64' backend/searcher-rs/src` NO da cero: da 2 líneas.** Lo sustantivo —**los dos fallbacks del sizing en `orchestrator.rs`**— **sí está eliminado** (la base los tiene en 2196/2204 y el head no). Hallazgo **F3** (abajo). El autor acota su propio grep a `orchestrator.rs` (`docs/loop/DECIMALS-CYCLE-01.md:343`), donde efectivamente es cero; el contrato lo generalizó a todo `src/`.

**(c) Sin unidad resuelta ⇒ NOT COMPUTED con la reason, y jamás 18:**
```rust
// orchestrator.rs:2350-2358 y 2372-2377
None => { warn!(event = "orchestrator.sizing_units_unresolved", chain_id,
                reason = "decimals_not_resolved_by_provider_for_sizing_units", …);
          f64::NAN }
…
None => warn!(event = "orchestrator.sizing_units_unresolved", chain_id,
              reason = "decimals_not_resolved_by_provider_for_sizing_output", …)
```
Los `warn!` con `event="orchestrator.sizing_units_unresolved"` están en **2** call sites (2352, 2373), con **reasons distintas** para entrada y salida.

**(d) El test que AFIRMABA el 1e18 fue REESCRITO, no borrado:**
```
BASE (4077fea5)  orchestrator.rs:2636-2640
   stamp_sized_figures(&mut legacy, &legacy_ledger);
   assert_eq!(legacy.candidate.amount_in, 3.0,
              "without decimals the legacy 1e18 rule applies to the SIZED amount");
HEAD (b3302cf0)  orchestrator.rs:2807-2822   (MISMO test, MISMO fixture legacy_seed/three_units)
   let no_provider: … = None;
   stamp_sized_figures(&mut legacy, &legacy_ledger, no_provider.as_ref(), 1);
   assert!(legacy.candidate.amount_in.is_nan(), "without a resolved unit the notional is NOT COMPUTED (NaN)…");
   assert_ne!(legacy.candidate.amount_in, 3.0,
              "the 1e18 fallback IS the divergence t201 closes — it must be gone");
```
La aserción está **invertida sobre el mismo cuerpo**, no eliminada. **PASSED** (con F3 declarado).

## C5 — RESIDUAL DEL SELLO SYNC: **medido**, no supuesto

**Construcción (código):** el **MAPA** resuelve **`redis_decimals` primero y `pg_decimals` después** (`orchestrator.rs:222: redis_decimals.get(&lc).or_else(|| pg_decimals.get(&lc))`, poblados en `resolve_cycle_decimals`, 1653-1696). El **SELLO** (`row_token_decimals`) resuelve **solo por `PgTokenDecimalsProvider`** (`main.rs:606-610`, "PostgreSQL-backed"). ⇒ **la asimetría existe por construcción: un token que estuviera SOLO en el catálogo Redis lo resolvería el mapa y NO el sello.**

**¿Puede producir una discrepancia SILENCIOSA? Medido en producción (chain 1, hoy):**

| medida | valor |
|---|---|
| claves del catálogo Redis `arbx:tokens:1:*` | **2.503** (valor `string` JSON `{"symbol":…,"decimals":…,"is_stablecoin":…}`) |
| `tokens.decimals` no nulo en PG (chain 1) | **6.758** |
| presente en AMBAS fuentes | **2.503** |
| **solo en Redis** (el caso del residual) | **0** |
| **discrepancias de VALOR** (mismo token, distinto `decimals`) | **0** |
| solo en PG | 4.255 |

**Dictamen del residual:** la asimetría **existe** y podría materializarse si un token se escribiera en el catálogo Redis **antes** de tener fila en PG (ventana transitoria del scanner↔PG). **Hoy no ocurre: 0 tokens solo-Redis y 0 discrepancias de valor** (Redis ⊂ PG con valores idénticos). Y su consecuencia **no sería silenciosa**: el sello emite `orchestrator.sizing_units_unresolved` con reason y escribe `NaN` (declarado, y **conservador**: NO COMPUTADO en vez de una unidad inventada). ⇒ **hallazgo F2, severidad BAJA.** No bloquea el aterrizaje.

## C6 — CORRECCIÓN A t197, VERIFICADA DE FORMA INDEPENDIENTE

| afirmación del autor | verificación propia | resultado |
|---|---|---|
| la fixture **sí está commiteada** | `git ls-tree -r b3302cf0 -- frontend/lib/apex/schemas/__tests__/fixtures/route-discovery-tick/` → `100644 blob 6c5833c5eeb93abe8828a9800bf9bb2e487a3e9d full.json` (+`knobs-off.json`) | **SOSTENIDA** |
| `golden_fixture_dir()` la busca **fuera de `backend/`** | `route_discovery_worker.rs:2597-2601`: `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../frontend/lib/apex/schemas/__tests__/fixtures/route-discovery-tick")` — de `backend/searcher-rs` sube 2 ⇒ **raíz del repo** | **SOSTENIDA** |
| por eso el fallo es **artefacto de ALCANCE del árbol** (`git archive -- backend`) | con `git archive -- backend` la ruta `../../frontend/…` **no puede existir** en el árbol extraído; el propio test hace `panic!("golden {name} missing …")` (`:2694`) | **SOSTENIDA** |
| contraste con el CI en el mismo head | **`Rust tests` = PASS (1m33s)** en `b3302cf0` (run 37852377771) — medido por mí, no citado del informe | **CONFIRMA** |

⇒ la corrección a t197 **se sostiene**. **PASSED.**

## C7 — KNOBS INTACTOS (query propia)

```
$ docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
  "SELECT 'capital_usd='||capital_usd||' min_profit_usd='||min_profit_usd||' spread_sanity_mult='||spread_sanity_mult \
   FROM trading_config WHERE chain_id=1"
capital_usd=1000.00 min_profit_usd=50.0000 spread_sanity_mult=3.0000      exit=0
```
**PASSED** — y esta revisión **no escribió** en PG, ni tocó el VPS (solo lecturas: `psql` SELECT, `redis-cli` scan/mget, `git`, `gh`).

## C8 — EL PR NO SE ATERRIZA

`gh pr view 927` → `state=OPEN`, `isDraft=true`, `mergedAt=null`, `head=b3302cf0`. **Cero merge, cero push a `main`, cero firma, cero broadcast, cero escritura on-chain, cero reinicio de servicios.** El único push de esta revisión fue una **rama de mutante** (`review/t202-mutant-18-map`) para que el CI ejecutara, **abierta como PR #928 DRAFT "NO MERGE" y CERRADA al leer el resultado** (`state=CLOSED`, `mergedAt=null`).

---

## HALLAZGOS

### **F1 — BLOCKER: el PR deja `Rust CI` en ROJO, y es su propia línea de test**

```
workflow "Rust CI" (job: cargo check + clippy + test) en el head b3302cf0  -> conclusion=FAILURE (2m30s)
   error: this call to `clone` can be replaced with `std::slice::from_ref`
     --> searcher-rs/src/orchestrator.rs:3436:44
        let got = build_cycle_decimals_map(&[unknown.clone()], &empty_redis, &empty_pg);
        help: try: `std::slice::from_ref(&unknown)`
     = note: `-D clippy::cloned-ref-to-slice-refs` implied by `-D warnings`
   error: could not compile `searcher-rs` (lib test) due to 1 previous error
   Process completed with exit code 101
misma workflow sobre main: 37783695176 success (77b42b3d) · 37728772313 success (8414e512) · 37722666339 success (80e2f86c)
```
**Atribución (no por proximidad, por historia):** `build_cycle_decimals_map` **no existe** en la base (`git grep` en `4077fea5` → sin resultados, exit 1) y la línea 3436 aparece como **línea agregada** en `git diff 4077fea5..b3302cf0 -- …/orchestrator.rs`. **El PR introduce el fallo.**

**Por qué el autor no lo vio:** su tabla de evidencia (§ del doc, líneas 196-199/320-324) corre `cargo fmt --check`, **`cargo check -Dwarnings`** (exit 0 en los tres árboles) y `cargo test`. **`cargo check -Dwarnings` NO ejecuta los lints de clippy**; el gate del CI es `cargo clippy … --all-targets -- -D warnings`, que sí incluye el target de test donde vive la línea nueva. **Hueco exacto entre la evidencia del autor y el gate que decide.**

**Fix requerido:** `std::slice::from_ref(&unknown)` (o `&[unknown.clone()]` → referencia, según el `help` del propio lint) en `orchestrator.rs:3436`, y volver a correr **`cargo clippy -p searcher-rs -p relays-client --locked --all-targets -- -D warnings`** como parte de la evidencia, no solo `cargo check`.
**No bloquea el fondo del PR** (los tests pasan y el falsificador muerde); **bloquea el aterrizaje** tal como está.

### **F2 — BAJO: asimetría mapa-vs-sello (mapa Redis-primero; sello solo-PG)**
Latente, medida como **0 casos hoy** (§C5). Consecuencia futura posible: NO COMPUTED declarado (conservador), nunca una unidad inventada. Recomendación: si se quiere cerrar del todo, que el sello consulte **la misma** fuente compuesta que el mapa (o que el catálogo Redis y `tokens.decimals` se escriban en la misma transacción). **No bloquea.**

### **F3 — INFORMATIVO: el `grep` literal del contrato no da cero**
`grep -rn '1e18_f64' backend/searcher-rs/src` → **2 coincidencias**: un comentario (`dex_engine.rs:1272`) y **código vivo** (`triangular_engine.rs:579: .map(|w| u256_to_f64(&w) / 1e18_f64)`). **Los dos fallbacks que la aceptación nombra sí están eliminados.** El de `triangular_engine.rs` es una suposición de 18 decimales **de la misma clase** en **otro locus**, fuera del alcance declarado de #927 ⇒ **candidato a tarea propia**, no defecto de este PR.

---

## C9 — CRITERIOS NO COMPUTADOS (declarados aparte, nunca redondeados)

1. **Compilación EJECUTADA EN LOCAL: NO COMPUTADO.** El VPS **no tiene `cargo`/`rustc`** (`which` exit 1; `cargo --version` → 127) y **no hay `target/` caliente** (`/opt/arbitragex-v2/backend/target`: no existe, exit 2); el disco está al **93 % con 11 GB libres** y `postgres_data` ocupa **66 GB** ⇒ compilar el workspace de `searcher-rs` ahí ponía en riesgo el volumen de producción. **Razón exacta, no una excusa: el criterio se cumplió por otra vía — el CI del repositorio como ejecutor independiente** (C3), que es **más** independiente que mi shell, no menos.
2. **Conteo de la suite `bin` (el "1701" del autor): NO MEDIDO.** El job del CI que usé corre `cargo test --lib`; el lado `--bins` no lo ejecuté. **No se cita como propio.**
3. **Los números de la tabla del autor sobre SU árbol** (lib 1713+1 failed, bin 1700+1 failed) **no se reproducen aquí**: mi medición es la del CI (1714/0/6). Se declaran **no verificados por mí** en su árbol.

**Y nada de lo anterior se redondea a `passed`.**

## PERMISOS Y ALCANCE

Solo lecturas: `git`/`gh` (local), `psql` SELECT y `redis-cli` scan/mget (VPS, sin escrituras), logs del CI. **`inScope` respetado: este dictamen vive en `docs/review/`.** El PR #927 **no se modificó** (ni una línea, ni un comentario en su rama).

```bash
git hash-object docs/review/REV-DECIMALS-REBASE-01.md
git rev-parse HEAD:docs/review/REV-DECIMALS-REBASE-01.md
```

---

# APÉNDICE (t202, intento 2) — EJECUCIÓN PROPIA: el falsificador, corrido EN MI MANO

> **Qué agrega y qué NO cambia.** Este apéndice es **append-only**: agrega la ejecución que el cuerpo declaró **NO COMPUTADA** (§C9.1: *«compilación ejecutada en local: NO COMPUTADO»*) y **no toca la conclusión**. El veredicto sigue siendo **`needs_revision`** con el blocker F1 intacto. Nada del cuerpo se borra ni se reinterpreta: lo que decía sigue dicho.

## A.1 Entorno y CUSTODIA del árbol medido
`rust:1.91` (cargo/rustc 1.91.1) sobre una **COPIA** del árbol del head en `/tmp/t202r/head` del VPS (el `/opt` productivo no se tocó; cero escrituras en el VPS: sólo `tar`/`scp` a `/tmp` y `docker run`). Custodia por `git hash-object` **dentro del árbol extraído**:
```
BLOB_orchestrator=cdd9ad253a842c44d936987debb96b03e724fbb7   # == el del commit b3302cf0
BLOB_scanner     =10bb5dfa9dd7fae724da0e379aa6d05a0f5455b1   # == el del commit b3302cf0
BLOB_fixture     =6c5833c5eeb93abe8828a9800bf9bb2e487a3e9d   # == el de main y del head
PRISTINE_sha256  =04f4a25aaa4cc9ba7dcc697a614e3a7ca56782c4d89a0d8cf62a752c31cfe9d3
```
⇒ compilé **el head**, no otra cosa. (Ese era el punto que el cuerpo no podía afirmar.)

## A.2 ★ MUTANTES PROPIOS (dos, construidos por mí, no los del autor)
Reemplazo determinista sobre el bloque reparado, verificado `OCCURRENCES=1` y sha256 antes/después. `-p searcher-rs --lib --locked`, sin `-D warnings` (por eso el mutante compila aunque el árbol prístino tenga el lint de clippy de F1).

| estado | blob del fichero mutado | CHECK (`--no-run`) | **FOCAL** (`decimals_cycle`) | TOTAL (`--no-fail-fast`) |
|---|---|---|---|---|
| **FIX** `b3302cf0` | `04f4a25aaa4cc9ba7dcc697a614e3a7ca56782c4d89a0d8cf62a752c31cfe9d3` | **0** | **0** — `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1717 filtered out` | `FAILED. 1713 passed; 1 failed; 6 ignored` |
| **M1** (mío) | `73df1d419ca0f1d48495484bfb3f743679c7504307fc8f325fce679443be053d` | **0** | **101** — `FAILED. 0 passed; 3 failed; …; 1717 filtered out` | `FAILED. 1710 passed; 4 failed; 6 ignored` |
| **M2** (mío) | `1973ce3aafd22354ee12278682ae65c250b7c248065abcc1b10eb8a6fa08335a` | **0** | **101** — `FAILED. 1 passed; 2 failed; …; 1717 filtered out` | `FAILED. 1712 passed; 2 failed; 6 ignored` |
| **restaurado** | `04f4a25aaa4cc9ba7dcc697a614e3a7ca56782c4d89a0d8cf62a752c31cfe9d3` ⟵ **== prístino** | — | (vuelve a 0) | — |

- **M1 = «restaurar el default de 18 en el locus reparado»**, tal como el contrato lo pide: la resolución vuelve a la tabla canónica con su `_ => 18` (comportamiento PRE-fix):
  `let _ = (redis_decimals, pg_decimals); let d = crate::engines::dex_engine::canonical_token_decimals_str(addr); map.insert(lc, d);`
- **M2 = la variante mínima**: fuentes reales intactas + `unwrap_or(&18)` — para separar «ignora las fuentes» de «sólo añade el 18».

**Pánicos nombrados de M1 (los tres del ciclo, con línea):**
```
orchestrator::tests::decimals_cycle_t187_eight_cases_resolve_to_the_real_unit_not_18
  panicked at searcher-rs/src/orchestrator.rs:3400:13:
  assertion `left == right` failed: EURC 0x1abaea1f7c830bd89acc67ec4af516284b1bc33c (redis+pg)
  debe entrar al mapa con su unidad real 6
orchestrator::tests::decimals_cycle_unknown_token_is_not_computed_never_18
  panicked at …:3431:9: sin fuente no hay unidad: el mapa no puede llevar NINGUNA entrada (ni 18)
orchestrator::tests::decimals_cycle_source_precedence_and_route_order
  panicked at …:3467:9: assertion `left == right` failed: Redis manda sobre PG
```
El **4.º fallo del total de M1** es `sizing_ledger_and_cycle_map_agree_on_the_same_unit`: **el test que fija la divergencia que este PR cierra cae en cuanto el 18 vuelve** ⇒ la reparación está **pinneada por su propio test**, no sólo por un caso de ejemplo.

## A.3 Las cuatro condiciones del contrato, cerradas **por mi ejecución**
1. **`FOCAL_EXIT` del fix = 0** ✅ (`ok. 3 passed; 0 failed`)
2. **`FOCAL_EXIT` del mutante = 101** ✅ (`0 passed; 3 failed`), con el **test focal de los 8 tokens** entre los que caen
3. **El mutante COMPILA**: `CHECK_EXIT = 0` en M1 y en M2, con **0 líneas `^error`** en el log de compilación ✅
4. **Conteo TOTAL idéntico en ambos lados**: `1713+1+6 = 1720`, `1710+4+6 = 1720`, `1712+2+6 = 1720` ✅ — el mutante **no añade ni quita tests**
5. **Restauración exacta**: `FINAL_sha256 == PRISTINE_sha256` ✅

**Y corrobora lo que el CI ya había mostrado (no lo sustituye):** mi total **1720** coincide con el `running 1720 tests` del job `Rust tests` de #929/#928, y mi **1714 ejecutado** (1713 passed + 1 failed) coincide con el `1714 passed` del CI **y** con el `1714` que el autor declaró. **`bin 1701` y `19 suites` siguen NO COMPUTADOS** por mí (corrí `--lib`): no se citan como propios.

## A.4 ★ El único fallo prístino, y su causa medida (refuerza §C6)
`TOTAL_FIX_EXIT=101` por **un** test: `route_discovery::route_discovery_worker::tests::xlang_golden_tick_contract`, con
`panicked at …:2694:33: golden knobs-off.json missing (No such file or directory (os error 2))`.
**Causa medida, no inferida:** yo había enviado **sólo `full.json`**; el test pide **dos** fixtures (`assert_golden("full.json", …)` :2706 y `assert_golden("knobs-off.json", …)` :2710). Enviado **el directorio completo**:
```
FIXTURES=full.json knobs-off.json
GOLDEN_EXIT=0      test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1719 filtered out
```
⇒ **reproducción propia del artefacto de ALCANCE** que t197 sufrió y que el autor corrigió: un árbol `git archive -- backend` **no puede** satisfacer ese test porque `golden_fixture_dir()` resuelve `CARGO_MANIFEST_DIR/../../frontend/…`, **fuera de `backend/`**. La corrección del autor a t197 **queda confirmada por mi propia ejecución**, en el fichero hermano (`knobs-off.json`), lo que la hace independiente del caso que t197 citaba.

## A.5 ★ CAUSA RAÍZ de F1 — por qué el arnés del autor no lo vio
El `verify-run.sh` que el propio PR incluye corre: `cargo fmt --all -- --check` · `cargo fmt -p searcher-rs -- --check` · **`RUSTFLAGS=-Dwarnings cargo check --manifest-path searcher-rs/Cargo.toml`** · `cargo test … --lib … decimals_cycle` · `cargo test … --no-fail-fast`. **Nunca `cargo clippy`.** El job **gating** del CI es **clippy** con `-D warnings` (conjunto **estrictamente mayor**), y el lint que cae (`clippy::cloned_ref_to_slice_refs`) **sólo existe en clippy**. Es un **hueco de cobertura del arnés**, no un dato oculto: explica que su `fmt=0` y su `-Dwarnings check=0` sean ciertos **y** el CI esté rojo a la vez.
**Fix (de una línea, no lo ejecuto: soy revisor):** `std::slice::from_ref(&unknown)` en `orchestrator.rs:3436` y volver a pedir revisión con **`cargo clippy -p searcher-rs --all-targets -- -D warnings` en la mano**.

## A.6 Estado del CI en el head (mi propia consulta, con hora)
`gh api …/commits/b3302cf0…/check-runs` → **37 checks**: **32 `success`** · **3 `failure`** (`cargo check + clippy + test`, `lint-and-test-rust`, `ci-gate` paraguas) · **1 neutral** (`CodeQL`: *1 configuration not found*) · **1 `in_progress`** (`analyze (rust)`, al momento de consultar).
**Control en `main` (`4077fea5`):** `lint-and-test-rust :: success` (2026-10-08 21:32:19) y `ci-gate :: success` ⇒ **el rojo lo introduce el PR**. (`main` tiene 1 fallo pre-existente, `Deploy to VPS`, sin relación con este PR.)

## A.7 Residual del sello — mis números (§C5, ahora con conteos)
Query propia, solo lectura, chain 1: catálogo Redis `arbx:tokens:1:*` vs PG `tokens`.
```
REDIS_DEC_ROWS=2503   PG_ROWS=6759
REDIS_ONLY=0          EN_AMBAS=2503          PG_ONLY=4256
DISCREPANCIAS_DECIMALS=0 / 2503
```
⇒ **Hoy no es alcanzable** ni el caso de cobertura (hace falta un token sólo-Redis: **0**) ni el **numérico silencioso** (mismo token con unidad distinta en Redis y PG: **0 de 2503**). Y la degradación es un `warn!` con evento propio + `NaN`, **nunca 18**. Condición de disparo declarada: un catálogo Redis que discrepe de `tokens.decimals` de PG, o un token sólo-Redis. Severidad: **baja**, como en §F2.

## A.8 Knobs (mi propia query)
`SELECT chain_id, capital_usd, min_profit_usd, spread_sanity_mult, updated_by FROM trading_config` → chain 1: **`1000.00` / `50.0000` / `3.0000`**, `updated_by=admin`. Las otras 5 cadenas: `capital_usd=0.00` (`migration_046`). Redis `arbx:config:canonical_knobs`: `"execution_mode":"PAPER_SHADOW"`.

## A.9 Lo que este apéndice **cierra** de §C9
- **§C9.1 (compilación/ejecución local: NO COMPUTADO) → MEDIDO.** El falsificador corrió **en mi mano** (A.2/A.3), no sólo en el CI.
- **§C9.2 (`bin` 1701) y §C9.3 (números del autor en su árbol) → siguen NO COMPUTADOS.** No se redondean.
- **La conclusión no cambia: `needs_revision`.** Los hallazgos del cuerpo —**F1 (BLOCKER)**, **F2 (bajo)** y **F3 (informativo)**— quedan exactamente como están ahí: el PR **no se aterriza** en esta revisión. Este apéndice sólo agrega medición; no reescribe su veredicto.

---

*El fondo del PR se sostiene: la base está re-materializada, la divergencia está cerrada en el código, el test del 1e18 fue reescrito y **el falsificador muerde sobre la base nueva con un mutante mío, ejecutado por el CI —y ahora también por mí, en `rust:1.91`, con `CHECK_EXIT=0`, `FOCAL_EXIT` 0 vs 101 y 1720 tests en los tres estados—**. Lo que impide aterrizarlo es una línea: **`clippy --all-targets -D warnings` cae en el propio test nuevo del PR, y `main` está verde en esa misma workflow**. Se devuelve como **NEEDS_REVISION** con el fix exacto, sin tocar el PR.*
