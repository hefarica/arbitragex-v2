# GSIM-COVERAGE-CEILING-01 — el descarte es un dato CONTADO Y NOMBRADO, y el veredicto de cobertura DEGRADA a NO COMPUTADO

**Rama:** `sre/gsim1-coverage-ceiling-01` · **PR:** [#885](https://github.com/hefarica/arbitragex-v2/pull/885) — `state=OPEN`, `isDraft=true`, `mergedAt=null`, `changedFiles=8`, `headRefOid=49b232e45e9d53167ea763eb502295c69c5a20a1`, `base=main`.

**Entrega:** rama + PR en **DRAFT**. Sin merge, sin deploy, sin push a `main`, sin mainnet, sin firma, sin broadcast, sin préstamos, sin transferencias, sin rotación de claves. Redis, configuración, motor de simulación y umbrales de rentabilidad: **intactos**.

**Base:** el trabajo se inició en `main = 901eb947ff3bec359a3021a8db5b074e7080b79f` (verificado por `git ls-remote` antes de clonar). `main` **avanzó durante el trabajo** a `77b42b3dccc001455fda3e8d4437d973c9e98c4b` (3 commits: `fix(sim-ctl): FUND-READ-CALLDATA-01` + 2 merges). El rango toca **solo** `backend/sim-ctl/src/signer_funding.rs` y `docs/backend/FUND-READ-CALLDATA-01.md` — **cero solapamiento** con esta superficie de cambio — así que la rama se rebaseó sobre la punta, como pide la consigna. Tras el rebase, los 8 blobs son byte-idénticos a los probados (ver §7).

**Clon aislado:** `%TEMP%\arbx-gsim1-coverage-01`. El checkout compartido (`858b943f`, rama `fix/perhop-reserves-01`) **no se tocó**: todo se leyó/escribió en el clon. Evidencia de que no hubo contaminación: una llamada `[System.IO.File]::ReadAllBytes` con ruta relativa resolvió contra el checkout compartido y **falló con «file not found»** (no leyó ni escribió); desde entonces todas las rutas son absolutas.

---

## 1. El defecto, medido

El benchmark G-SIM-1 sólo puede etiquetar las topologías que su **propio encoder A.3.a** sabe codificar.

### 1.1 La foto de referencia (una sola sentencia SQL, un solo `now()`)

Artefacto: `scripts/ci/fixtures/gsim1_coverage_window.json` — capturado read-only de producción dentro de **una transacción** (`snapshot_now = 2026-10-08 13:24:46.406955 UTC`), con `raw_repr_sha256 = c3a62b767861554f8317ecc597f6521d665cb274ecde5859d46406537f9afac1` y 70 buckets. La sentencia es `scripts/ci/fixtures/gsim1_ceiling_snapshot.sql`.

| | conteo |
|---|---|
| topologías distintas en la ventana (chain 1, con `dex_adapters`, 2 h rodantes) | **977** |
| etiquetables por el encoder del benchmark | **13** (1,33 %) |
| **descartadas por el encoder** | **964** (98,67 %) |

`named_total = 977 = window_total`: la partición **cuadra**, que es la forma aritmética de «ningún descarte quedó sin nombre».

**Por qué hace falta una sola sentencia:** la ventana es una rebanada rodante de una tabla viva. Consultas separadas divergen en segundos — medido: 952 topologías y 962 topologías a cuatro minutos de distancia (el conteo por `legs` pasó de `2|184, 3|237, 4|340, 5|186, 6|9` a `2|184, 3|241, 4|342, 5|186, 6|9`). Con varias consultas la partición no cuadra y el «techo» sería un artefacto de la ventana, no un dato.

### 1.2 Los tres lugares donde el descarte era un agregado

- `scripts/gsim1_variance_export.sql:62-64` — el `WHERE` lo tira:
  ```sql
  L62:   AND jsonb_array_length(o.route_metadata->'dex_adapters') = 2
  L63:   -- A.3.a encoder-supported legs only (mirror of adapter_to_semantic()).
  L64:   AND (o.route_metadata->'dex_adapters') <@ '["UniswapV2","SushiSwap"]'::jsonb
  ```
- `scripts/gsim1_variance_benchmark.sh:181-185` — el driver imprimía **una** línea de brecha total (`not A.3.a-encodable: <N>`).
- `scripts/gsim1_variance_benchmark.sh:269-274` — la fila del registro calcula
  `detail["coverage"] = round(labeled / detail["population_size"], 4)`, donde `population_size` es el export **ya filtrado por el encoder**. Es decir: `coverage = 1.0` significa «etiquetamos todo lo que el encoder permite», y eso se lee como un resultado del mercado. **Ése es el defecto R10.**

Y el descarte interno del harness moría en `backend/sim-core/tests/variance_benchmark.rs:483-486` (`hist.pred_failed += 1; continue;`) y sus hermanos: la identidad de la muestra estaba en scope y se colapsaba a un contador.

---

## 2. Qué cambia

### 2.1 El descarte deja de ser un agregado

`scripts/ci/gsim1_coverage_ceiling.py` (nuevo, **SELECT-only**) parte la ventana en buckets `(razón, kind, adapter-set)` y emite **una línea por bucket** más un total por código:

```
COVERAGE_DISCARD_CODE_TOTAL code=<razón nombrada> count=<n>
coverage_discard code=<razón> kind=<kind> adapters=<json> count=<n>
```

Tres decisiones que son la diferencia entre nombrar y aparentar nombrar:

1. **La clave del bucket es el tuple completo `(code, kind, adapters)`, nunca el código solo.** Con la clave en el código, `["UniswapV2","UniswapV3"]`, `["SushiSwap","UniswapV3"]` y `["UniswapV3","UniswapV2"]` colapsarían en una línea `unsupported_adapter:UniswapV3` que imprime los adapters del **primero** contra el conteo **sumado** de los tres: una razón nombrada adosada a muestras que no le pertenecen. `test_bucket_never_merges_two_adapter_sets` falla si eso vuelve.
2. **Los labels de adapter se usan VERBATIM.** `PancakeSwap V3` conserva el espacio; `uniswap-v3` conserva las minúsculas. Normalizarlos escondería exactamente el desajuste que el instrumento existe para exponer: ambos scopes del encoder matchean por spelling literal.
3. **El roll-up de una línea es space-safe.** Un `awk` ingenuo sobre `unsupported_adapter:PancakeSwap V3=14` inventa un término fantasma `V3`. Cada término va `shlex.quote`ado y el artefacto autoritativo es la línea por código (delimitador ` count=`). Esto lo encontró un test, no una revisión.

### 2.2 El veredicto DEGRADA a NO COMPUTADO

```
COVERAGE_VERDICT=NO_COMPUTADO reason=encoder_scope: supported_adapters=UniswapV2/SushiSwap route_shape=legs==2 excludes 964 of 977 window topologies (98.7%); the readings describe the encoder, not the window
COVERAGE_DEGRADED_FIELD field=coverage reads_as=market_result measures=the_encoder_not_the_market verdict=NO_COMPUTADO source=scripts/gsim1_variance_benchmark.sh:269-274 (coverage = samples_labeled / population_size)
```

El criterio es **estructural**: `discarded == 0` ⇒ `COMPUTADO`; cualquier descarte ⇒ `NO_COMPUTADO`. **No es un umbral nuevo**: no se introdujo ninguna constante de corte, y la prueba `test_verdict_is_structural_not_a_threshold` muestra que **una** topología descartada entre 1001 (0,1 %) ya degrada. `window_total == 0` también degrada (`empty_window`), porque un cero de población no puede leerse como cobertura plena.

### 2.3 El harness nombra sus propios descartes

`backend/sim-core/tests/variance_benchmark.rs`:

- `enum SkipCode` (L162) — 11 variantes, cada una con su causa. `code()` (L~180) nunca devuelve una categoría pelada; `aggregate_field()` (**L203**) es el **único** lugar que mapea una razón nombrada a su contador histórico.
- `skip_marker_line()` (**L221**) — pura, para que los tests ejerciten el camino real de emisión (stdout no es capturable desde `#[test]`, y un test que re-deriva la línea que dice verificar no verifica nada).
- `emit_skip()` (**L238**) — imprime `VARIANCE_BENCH_SKIP={"opportunity_id":…,"code":"…"}` y acumula `skip_code_distribution`.
- `pre_simulation_gate()` (**L247**) — las tres compuertas pre-simulación en una función pura: testeable sin RPC y sin poder divergir de la decisión que toma el loop.
- `VARIANCE_BENCH_JSON` gana la clave **nueva** `skip_code_distribution` (**L755**). El driver postea ese JSON verbatim como `detail` de la fila del registro, así que **la fila misma deja de ser un agregado**.

**Paridad de contadores:** `skips` conserva nombres de campo **y valores**. Los sitios de incremento son literalmente los mismos; lo único que se agregó es la línea nombrada al lado.

### 2.4 Cableado, incapaz de enrojecer el gate

`.github/workflows/gsim1-variance-benchmark.yml` **L230-284**: paso nuevo `Count and NAME the coverage ceiling`, con `if: always()` (L~262), `continue-on-error: true` (**L264**) y `exit 0` **explícito** (**L284**) — doblemente incapaz de enrojecer el gate, incluso si el instrumento crashea. Toda ruta de anomalía (`ssh_credentials_absent`, `query_failed_rc_<rc>`, `response_unparseable`, `fixture_unreadable`) emite un `::warning::` y **`COVERAGE_VERDICT=NO_COMPUTADO reason=not_measured:…`** — una medición ausente jamás es un veredicto de cobertura aprobado. `coverage-ceiling.log` y `coverage-ceiling.json` se preservan como artefactos (L347-348) y el bloque entra al job summary (L330).

`.github/workflows/ci.yml`: job hermético `gsim1-coverage-ceiling-regression` (4 pasos) registrado en `ci-gate` (`needs` + el set `required` del script del gate) — la cadena de checks requeridos se actualizó **consistentemente en los dos lugares**, o el gate del `ci-gate` fallaría por set mismatch.

---

## 3. Qué son las 964 que no se pueden etiquetar — contado, no impresionado

### 3.1 Comando y salida cruda

```
$ python scripts/ci/gsim1_coverage_ceiling.py --from-fixture scripts/ci/fixtures/gsim1_coverage_window.json
COVERAGE_CEILING_ENCODER harness_accepted_adapters=UniswapV2,uniswap-v2,uniswapv2,SushiSwap,sushi,sushiswap export_literal_adapters=UniswapV2,SushiSwap supported_route_shape=legs==2 harness_source=backend/sim-core/tests/variance_benchmark.rs:111 adapter_to_semantic() export_filter_source=scripts/gsim1_variance_export.sql:64 ((dex_adapters) <@ '["UniswapV2","SushiSwap"]')
coverage_discard code=unsupported_route_shape:legs=4 kind=triangular adapters=["uniswap-v3","uniswap-v3","uniswap-v3","uniswap-v3"] count=172
coverage_discard code=unsupported_route_shape:legs=3 kind=triangular adapters=["uniswap-v3","uniswap-v3","uniswap-v3"] count=149
coverage_discard code=unsupported_adapter:UniswapV3+UniswapV3 kind=dex_arb adapters=["UniswapV3","UniswapV3"] count=46
coverage_discard code=unsupported_adapter:UniswapV3 kind=dex_arb adapters=["UniswapV2","UniswapV3"] count=43
coverage_discard code=unsupported_adapter:PancakeSwap V3 kind=dex_arb adapters=["UniswapV2","PancakeSwap V3"] count=10
coverage_discard code=unsupported_adapter:PancakeSwap V3+UniswapV3 kind=dex_arb adapters=["PancakeSwap V3","UniswapV3"] count=7
coverage_discard code=unsupported_adapter:PancakeSwap V3+PancakeSwap V3 kind=dex_arb adapters=["PancakeSwap V3","PancakeSwap V3"] count=5
coverage_discard code=zero_amount_in kind=dex_arb adapters=["SushiSwap","UniswapV2"] count=1
coverage_discard code=zero_amount_in kind=dex_arb adapters=["UniswapV2","SushiSwap"] count=1
   … (70 buckets en total en el artefacto)
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_route_shape:legs=4 count=352
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_route_shape:legs=3 count=248
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_route_shape:legs=5 count=184
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_adapter:UniswapV3 count=77
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_adapter:UniswapV3+UniswapV3 count=46
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_adapter:UniswapV3+PancakeSwap V3 count=20
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_adapter:PancakeSwap V3 count=14
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_route_shape:legs=6 count=9
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_adapter:PancakeSwap V3+UniswapV3 count=7
COVERAGE_DISCARD_CODE_TOTAL code=unsupported_adapter:PancakeSwap V3+PancakeSwap V3 count=5
COVERAGE_DISCARD_CODE_TOTAL code=zero_amount_in count=2
COVERAGE_CEILING window_total=977 labelable=13 discarded=964 named_total=977 share_labelable=0.013306 share_discarded=0.986694 shape_breakdown={"legs=3":248,"legs=4":352,"legs=5":184,"legs=6":9}
COVERAGE_VERDICT=NO_COMPUTADO reason=encoder_scope: supported_adapters=UniswapV2/SushiSwap route_shape=legs==2 excludes 964 of 977 window topologies (98.7%); the readings describe the encoder, not the window
```

### 3.2 Los dos ejes, medidos por separado y **nunca mezclados**

| Eje | Qué mide | Fuente |
|---|---|---|
| **Encoder** | por qué el benchmark no puede codificar la muestra | las líneas de arriba |
| **Estado de producción** | qué hizo el pipeline con esa misma fila | `opportunities.status` / `rejection_reason` |

Estado de producción de las muestras descartadas de 2 patas (snapshot distinto, declarado: el de la consulta `G` de `measure.sh`, que contó 182 topologías de 2 patas y 169 descartadas):

```
=== G (status + rejection_reason de las descartadas)
rejected|v3_quote_unavailable|163
rejected|single_pool_no_spread|6
rc=0
```

**Lectura honesta, sin venderlo:** 163 de 169 ya habían sido rechazadas por producción con `v3_quote_unavailable`. Es decir: `unsupported_adapter:<label>` es la limitación **del encoder del benchmark**, y **NO** es evidencia de que el mercado tuviera ahí una ruta rentable. Por eso el instrumento emite los dos ejes en campos distintos y el paso de atribución por muestra (ya existente) sigue midiendo el otro. Fusionarlos sería el modo de fallo (d) de la doctrina: una etiqueta leída como otra cosa.

### 3.3 El eje `kind`

De la misma foto (§1.1): **184** topologías son `dex_arb` de 2 patas y **793** son `triangular` con 3-6 patas (248 + 352 + 184 + 9). Las 793 no son «2 patas con otro adapter»: el encoder A.3.a **rechaza estructuralmente** cualquier cosa que no sea un round trip de 2 patas — texto literal del encoder, `sim_encoder.rs`:
> `A.3.a supports 2-leg round trip only; candidate has {n} legs`

Por eso hay un código propio `unsupported_route_shape:legs=<n>` y **no** se cuenta como adapter no soportado.

### 3.4 Un detalle que el instrumento no esconde: dos scopes distintos

`adapter_to_semantic()` acepta 6 literales (`UniswapV2`, `uniswap-v2`, `uniswapv2`, `SushiSwap`, `sushi`, `sushiswap`) y es **case-sensitive** (sólo `.trim()`); el filtro del export usa **containment jsonb contra 2 literales**. Y los productores escriben distinto — medido: las filas `dex_arb` traen `UniswapV3`/`PancakeSwap V3`, las `triangular` traen `uniswap-v3` en minúsculas. Una topología que el harness **sí** podría codificar pero el export descarta por comparación literal se reporta con su propio código, `adapter_label_spelling:export_literal_mismatch`, jamás bajo `unsupported_adapter`. Atribuirle al encoder un desajuste del export sería exactamente el error que este trabajo corrige. (En la foto de referencia ese código tiene **0** filas: las 2 patas usan el spelling mixto.)

---

## 4. ¿El arreglo CAMBIA lo que el benchmark puede acreditar? — **NO**

Declaración explícita, sin venderlo como cobertura nueva:

- **Hace VISIBLE el techo; NO lo amplía.** Las 13 topologías etiquetables siguen siendo 13. No se tocó el `WHERE` del export, ni el allowlist del encoder, ni el encoder.
- **Población etiquetable:** idéntica. **`min_samples`:** idéntico. **Umbral de drift:** idéntico. **`skips`:** idéntico.
- Lo que **sí** cambia es lo que el benchmark puede **acreditar**: ahora puede acreditar — y publica — que su cobertura del mercado es `13/977`, con los 964 descartes desglosados y con nombre. Antes publicaba un `coverage` que se leía como resultado del mercado. **Se corrigió la lectura, no el alcance.**

### Cero movimiento de umbrales — citado antes y después

| Umbral | Antes (blob base) | Después | Cómo se garantiza |
|---|---|---|---|
| `min_samples` | `MIN_SAMPLES="${VARIANCE_MIN_SAMPLES:-$ROWS}"` (`gsim1_variance_benchmark.sh:208`) | **idéntico** — el archivo no se tocó | el driver no está en la superficie de cambio |
| Umbral de drift | `VARIANCE_MAX_MEAN_DRIFT_PCT` default 5.0 (`variance_benchmark.rs:516` `max_mean_drift`, no modificado) | **idéntico** | misma expresión, mismos valores |
| `skips` | `{attempted,…,pred_failed,obs_failed,zero_predicted}` | **idéntico** | los sitios de incremento no se movieron; `skip_code_distribution` es clave NUEVA; `skip_codes_keep_their_historical_aggregate` lo prueba variante por variante |
| Criterio de PASS/FAIL del benchmark | `hist.labeled >= min_samples && mean_abs < max_mean_drift` | **idéntico** | sin cambios en esa expresión |

---

## 5. ¿El encoder sigue sin soportar V3/PancakeSwap DESPUÉS del cambio? — **SÍ** (y el costo, estimado y declarado)

**Respuesta medida:** sí. El allowlist efectivo sigue siendo `{UniswapV2, SushiSwap}` y la forma soportada sigue siendo `legs==2`. Ninguna línea de este PR agrega un encoder. Lo prueban dos tests, no una promesa:

- `test_harness_adapter_literals_match_adapter_to_semantic` lee el fuente real de `adapter_to_semantic()` y exige que el set de literales sea exactamente los 6 de arriba; si alguien añade un brazo V3, **el test falla** y obliga a re-medir la cobertura en vez de ampliar el espejo en silencio.
- `test_v3_and_pancake_are_still_unsupported_after_this_change` afirma que `UniswapV3` y `PancakeSwap V3` siguen clasificándose como descartados.
- Del lado Rust, el propio encoder lo declara fail-closed en su test `parse_dex_kind_fails_closed_with_original_label`, que enumera `["uniswap-v3", "UniswapV3", "PancakeSwapV3", "curve", ""]` como `UnsupportedDexKind` preservando el label original.

### Costo de soportarlos — ESTIMACIÓN DECLARADA, **no** implementación

Esto es una **estimación**, con el bloqueo medido donde se pudo medir y declarado como no-medido donde no. **No se implementó nada de esto.**

**Bloqueo 1 — la forma de ruta (el más grande).** `build_round_trip_context_from_candidate` rechaza `dex_adapters.len() != 2` con `SimEncoderError::UnsupportedRouteShape`. Afecta a **793 de las 977** topologías de la ventana (`legs` 3-6, todas `triangular`). Soportarlas **no** es «añadir un adapter»: es un encoder multi-pata.

**Bloqueo 2 — el router.** `parse_dex_kind` sólo conoce `uniswapv2` y `sushi|sushiswap|sushiv2`. Haría falta una variante en `RouterKind` y entradas por cadena en el catálogo `routers_for_chain()` (`resolve_router_address` devuelve `MissingRouterAddress` sin entrada — fail-closed, sin fallback hardcodeado).

**Bloqueo 3 — el fee tier por pata.**
- `OpportunityCandidate` no lleva fee tier (`dex_adapters: Vec<String>`, `pool_addresses`, `token_addresses`, `amount_in`, `expected_amount_out`, `gross_profit`).
- **Medido** sobre `route_metadata` de las filas con `UniswapV3` en la ventana: las claves presentes son exactamente
  ```
  decimals | dex_adapters | economics_basis | pool_addresses | token_addresses
  ```
  **Ninguna es un fee tier.** `pool_addresses` trae las direcciones de pool y `token_addresses` los 3 tokens; para las 2 patas V3: `legs=2, pools=2, tokens=3`.
- **NO MEDIDO (declarado):** 17 117 **filas** de esa población tienen algún texto que matchea `%fee%` dentro del JSON. Eso **no** es una clave de fee tier y no lo uso como si lo fuera: queda como pregunta abierta — «¿existe un fee tier recuperable en alguna parte de `route_metadata`?» — que hay que resolver **antes** de estimar el trabajo de codificación. No lo cierro porque resolverlo exige abrir el esquema de esos payloads y eso ya es trabajo de implementación, fuera de este alcance.

**Estimación declarada (no medición):** soportar V3/PancakeSwap en el camino de 2 patas toca `RouterKind` + catálogo de routers + propagación de fee tier al candidato/export + un encoder de calldata V3-aware (`exactInputSingle` con fee, o `exactInput` con path de fee tiers) + la cobertura REVM correspondiente. El bloqueo 1 (multi-pata) es un encoder **distinto** y mucho mayor. Ninguna de las dos cosas está implementada ni medida en horas; cualquier cifra de esfuerzo que yo diera sería inventada, así que no la doy.

---

## 6. Tests y falsificador

### 6.1 Tests — con su efecto observable

**Python (25 tests, herméticos: sin DB, sin red, sin Docker).** Comando y salida:

```
$ python -m unittest discover -s scripts/ci -p 'test_gsim1_coverage_ceiling.py' -v
...
Ran 25 tests in 0.023s

OK
```

Los dos tests que son el contrato de esta tarea, y su efecto:

| Test | Efecto observable |
|---|---|
| `test_unsupported_adapter_discard_carries_its_named_reason` | con una muestra de adapter no soportado, la salida contiene `code=unsupported_adapter:UniswapV3+UniswapV3 kind=dex_arb adapters=["UniswapV3","UniswapV3"] count=46` y `code=unsupported_adapter:PancakeSwap V3+PancakeSwap V3` (spelling verbatim, con espacio) |
| `test_no_emitted_code_is_a_bare_category_name` | todo código emitido contiene `:` (o es `zero_amount_in`, una razón completa); un `code=unsupported_adapter` pelado es el fallo |
| `test_bucket_never_merges_two_adapter_sets` | dos adapter-sets distintos no comparten una línea de bucket |
| `test_every_discard_is_named_and_the_buckets_reconcile` | `named_total == window_total` y la suma de `code_totals` == descartadas |
| `test_verdict_degrades_to_no_computado_on_the_real_window` | `COVERAGE_VERDICT=NO_COMPUTADO` + `COVERAGE_DEGRADED_FIELD field=coverage reads_as=market_result` |
| `test_verdict_is_computado_when_the_encoder_covers_everything` | una ventana totalmente encodable **sí** da `COMPUTADO` → la degradación es condicional, no hardcodeada |
| `test_verdict_is_structural_not_a_threshold` | 1 descarte entre 1001 (0,1 %) ya degrada |
| `test_degraded_field_is_the_one_the_driver_actually_emits` | lee `gsim1_variance_benchmark.sh` y exige que `detail["coverage"]` siga siendo `round(labeled / detail["population_size"], 4)` |
| `test_the_only_statement_is_a_single_read_only_select` | la única SQL es un `WITH`/`SELECT`, un solo `;`, sin `INSERT/UPDATE/DELETE/DROP/ALTER/CREATE/TRUNCATE/GRANT/COPY`, y una sola invocación `psql -U postgres` en el módulo |
| `test_real_window_reports_the_measured_ceiling` | sobre la foto real: `window_total=977`, `labelable=13`, `discarded=964` |

Además, tres tests de ruta de anomalía prueban que `main()` devuelve **0** y emite `::warning::` + `COVERAGE_VERDICT=NO_COMPUTADO reason=not_measured:…` cuando faltan credenciales, cuando el fixture es ilegible y cuando la respuesta es inparseable.

**Rust (10 pasan, 0 fallan, 1 ignorado).** Comando y salida cruda (WSL2, toolchain 1.91.0 del `rust-toolchain.toml`, `cc` = `/home/hfrc/.local/bin/cc`, `CARGO_TARGET_DIR` propio para no bloquear la sesión concurrente):

```
$ cargo test -p sim-core --test variance_benchmark -- --nocapture
   Compiling sim-core v0.1.0 (/home/hfrc/arbx-cc-01/backend/sim-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 25s
     Running tests/variance_benchmark.rs (/home/hfrc/arbx-cc-01-target/debug/deps/variance_benchmark-0910124c977a10b1)
running 11 tests
VARIANCE_BENCH_SKIP={"code":"unsupported_adapter:PancakeSwap V3","opportunity_id":"0f0f0f0f-1111-2222-3333-444444444444"}
test every_skip_code_is_named ... ok
test skip_codes_keep_their_historical_aggregate ... ok
test pre_simulation_gate_keeps_the_historical_check_order ... ok
test unsupported_adapter_discard_is_named_per_sample ... ok
test skip_line_carries_identity_and_reason_on_one_line ... ok
test skip_marker_is_greppable_and_escaped ... ok
test compact_bounds_and_flattens_reasons ... ok
test adapter_mapping_is_truthful_about_supported_dexes ... ok
test route_hash_is_deterministic ... ok
test wei_to_tokens_converts_with_decimals ... ok
test variance_benchmark_predicted_vs_settled_block ... ignored, requires RPC_HTTP_1 + …
test result: ok. 10 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**El efecto observable, no el exit code:** la línea `VARIANCE_BENCH_SKIP={"code":"unsupported_adapter:PancakeSwap V3",…}` aparece impresa por el binario de test — es la emisión real del camino que antes hacía `hist.pred_failed += 1; continue;`. **«Compiling sim-core» presente y cero `Fresh sim-core`**: el `grep -E 'Compiling sim-core|Fresh sim-core'` del log devolvió `Compiling sim-core v0.1.0 (…)` y ninguna línea `Fresh sim-core`, así que los tests corrieron sobre la fuente modificada y no sobre un artefacto cacheado. Y los **bytes probados son los del repo**: `sha256` = `5e06f7fbbd352463167c88c2eb9b9b86b08f8e005f5e59a1333a563c1be57268` en ambos lados (Windows y WSL), verificado tras el `rebase`.

**Lints bloqueantes de CI** (los dos pasos que enrojecerían `lint-and-test-rust`):

```
$ cargo fmt -- --check                                    → fmt_rc=0   (y CR=0 en el archivo del repo)
$ cargo clippy -p sim-core --all-targets --locked -- -D warnings → clippy_rc=0, 507 líneas Compiling/Checking
```

`cargo fmt -- --check` **falló en el primer intento** (rustfmt quería reenvolver 146 líneas). Se aplicó `cargo fmt`, el diff fue puramente de formato, se copió de vuelta al repo con `cp` (bytes, LF preservado, `CR=0`) y **se re-corrieron los tests sobre la fuente ya formateada** para demostrar que es semánticamente neutro en vez de asumirlo: `test_rc=0`, 10 pasan. Si no lo hubiera corrido, el PR habría enrojecido el gate — es exactamente la clase de defecto que la consigna prohíbe introducir.

### 6.2 FALSIFICADOR — `mutar el arreglo y el test debe caer`

Artefacto: `scripts/ci/falsify_gsim1_coverage_ceiling.py` (commiteado, reproducible; **deliberadamente NO cableado a CI** porque muta el árbol de trabajo — un falsificador que muta el árbol dentro de un pipeline compartido sería un defecto nuevo).

Dos mutaciones independientes, porque la tarea nombra dos propiedades independientes:

- **M1** neutraliza la **razón nombrada**: `classify()` vuelve a devolver el agregado pelado `unsupported_adapter`.
- **M2** neutraliza la **degradación**: `compute_ceiling()` fuerza `verdict = "COMPUTADO"`.

Salida completa, verbatim (`falsifier-run.txt`):

```
BLOB A (pristine) = 53aed0a89be006fe2c6423ce24aa1c7a0acfa082
[RUN 1 | blob A] exit=0  (expect 0)   … Ran 25 tests … OK
[RUN 2 | blob B1 = 8688783fe357deb4d8ec8572a13e9421f96f6dff] M1: `unsupported_adapter` loses its named reason
  exit=1  (expect != 0)
  AssertionError: ':' not found in 'unsupported_adapter' : discard code 'unsupported_adapter' names a category, not a reason
  AssertionError: 'code=unsupported_adapter:UniswapV3+UniswapV3 kind=dex_arb adapters=["UniswapV3","UniswapV3"] count=46' not found in …
  AssertionError: 0 not greater than or equal to 2 : expected several distinct adapter sets sharing the UniswapV3 code in the real window
  Ran 25 tests … FAILED (failures=3)
  failing tests: ['FAIL: test_bucket_never_merges_two_adapter_sets …', 'FAIL: test_no_emitted_code_is_a_bare_category_name …', 'FAIL: test_unsupported_adapter_discard_carries_its_named_reason …']
[RUN 3 | blob B2 = b19eefc2590ab3c5f195f461c0b1bf652ccb265d] M2: the coverage verdict no longer degrades
  exit=1  (expect != 0)
  AssertionError: 'COMPUTADO' != 'NO_COMPUTADO' : 13 of 977 labelable topologies must not produce a market verdict
  AssertionError: 'COMPUTADO' != 'NO_COMPUTADO' : a 0.1% discard must still degrade: the criterion is structural
  Ran 25 tests … FAILED (failures=3)
  failing tests: ['FAIL: test_successful_fixture_run_exits_zero_and_warns_on_degradation …', 'FAIL: test_verdict_degrades_to_no_computado_on_the_real_window …', 'FAIL: test_verdict_is_structural_not_a_threshold …']
[RUN 4 | blob A restored = 53aed0a89be006fe2c6423ce24aa1c7a0acfa082] exit=0  (expect 0)   … Ran 25 tests … OK
  byte-identical to the pristine blob A: True
FALSIFIER SUMMARY
  run 1  blob A  = 53aed0a89be006fe2c6423ce24aa1c7a0acfa082  exit=0  (green)
  mutate M1 named reason neutralized   blob B = 8688783fe357deb4d8ec8572a13e9421f96f6dff  exit=1  (RED)
  mutate M2 degradation neutralized    blob B = b19eefc2590ab3c5f195f461c0b1bf652ccb265d  exit=1  (RED)
  run 4  blob A  = 53aed0a89be006fe2c6423ce24aa1c7a0acfa082  exit=0  (green)  restored=True
FALSIFIER PASS
```

Las tres corridas con sus blobs y exit codes, arriba. **Y lo que prueban no es el exit code**: prueban que con la razón nombrada removida, la salida deja de contener la cadena `code=unsupported_adapter:UniswapV3+UniswapV3` y pasa a contener `code=unsupported_adapter` pelado (AssertionError de RUN 2 lo imprime literal), y que con la degradación removida el veredicto pasa a `COMPUTADO` en una ventana de 977 topologías de las cuales 13 son etiquetables (AssertionError de RUN 3).

---

## 7. Blobs antes y después (`git hash-object`)

Ninguno se produjo con `Out-File` de PowerShell (re-codifica y mete CRLF): los archivos se escribieron con la herramienta de archivos (LF) y se verificó `CR=0` en los 7.

| Archivo | Antes | Después |
|---|---|---|
| `backend/sim-core/tests/variance_benchmark.rs` | `2bf3c3a160ba00c590485459099b390d4a30161a` | `8987cb75be9c2ae0d682be54238d9161723746c5` |
| `.github/workflows/gsim1-variance-benchmark.yml` | `b693d39afb6f3aba5539c4a4c0043b03f6ee60a1` | `54cfb069bb798a681cc1fa89a6fed6c9d2672b18` |
| `.github/workflows/ci.yml` | `d4ab97c565a876a62ce275984ffe7925bde2de06` | `d21261ab252e5f3718f9718ae62a518503a63479` |
| `scripts/ci/gsim1_coverage_ceiling.py` | (nuevo) | `53aed0a89be006fe2c6423ce24aa1c7a0acfa082` |
| `scripts/ci/test_gsim1_coverage_ceiling.py` | (nuevo) | `4e7cf18355d45c885bcdda4cb5a5b5513723a973` |
| `scripts/ci/falsify_gsim1_coverage_ceiling.py` | (nuevo) | `6bdf0cde562799172a905870311e5b43751f708b` |
| `scripts/ci/fixtures/gsim1_coverage_window.json` | (nuevo) | `4c92a8320486e0374f3f6716cdc40eeefcd89269` |
| `scripts/ci/fixtures/gsim1_ceiling_snapshot.sql` | (nuevo) | `bd7835fc8c75a924a87d4fba806a82a86e65eeb8` |

**Los 8 blobs son idénticos antes y después del rebase sobre `77b42b3d`** — en particular el blob Rust probado sigue siendo `8987cb75…`, así que la evidencia de compilación sigue siendo válida para el árbol entregado.

## 8. Líneas exactas del cambio

**`backend/sim-core/tests/variance_benchmark.rs`** (466 insertadas, 19 borradas; líneas post-`rustfmt`, que es el blob entregado): `skip_code_distribution` en `Histogram` **L110**; `enum SkipCode` **L162**; `SkipCode::code()` **L178**; `is_named()` **L197**; `aggregate_field()` **L203**; `skip_marker_line()` **L221**; `emit_skip()` **L238**; `pre_simulation_gate()` **L247**; `"skip_code_distribution"` en el JSON **L755**; los sitios de descarte ahora con `emit_skip` (dedup **L499**, compuerta pre-simulación **L514**, `stale_timestamp` block-resolve **L533** y out-of-window **L544**, `decimals_failed` **L572**, `bad_shape` de amount **L590**, `encode_failed` **L629**, `pred_failed` **L659**, `obs_failed` **L669**, `zero_predicted` **L682**); tests nuevos **L862-L1065** (`unsupported_adapter_discard_is_named_per_sample` **L862**, `every_skip_code_is_named` **L897**, `skip_codes_keep_their_historical_aggregate` **L932**, `pre_simulation_gate_keeps_the_historical_check_order` **L978**, `compact_bounds_and_flattens_reasons` **L1005**, `skip_line_carries_identity_and_reason_on_one_line` **L1022**, `skip_marker_is_greppable_and_escaped` **L1049**).

**`scripts/ci/gsim1_coverage_ceiling.py`** (nuevo, 617 líneas): espejos del contrato **L81** (`HARNESS_ACCEPTED_ADAPTERS`), **L92** (`EXPORT_LITERAL_ADAPTERS`), **L97** (`SUPPORTED_ROUTE_SHAPE`); códigos **L103-L109**; `classify()` **L112**; `build_window_sql()` **L139** (la única SQL); `compute_ceiling()` **L202** con las tres ramas del veredicto **L245 / L248 / L251**; `render_lines()` **L330**; `_unavailable()` **L491**.

**`.github/workflows/gsim1-variance-benchmark.yml`** (64 insertadas): paso nuevo **L230-284** — `if: always()`, `continue-on-error: true` **L264**, invocación con `--job-summary`/`--out-json` **L276**, `::warning::` **L281**, `exit 0` **L284**; bloque del summary **L330**; artefactos **L347-348**.

**`.github/workflows/ci.yml`** (65 insertadas, 2 borradas): job `gsim1-coverage-ceiling-regression` (4 pasos: unittest, marcadores sobre el fixture real, y el contrato de que el paso del benchmark sigue siendo advisory); registrado en `ci-gate` → `needs` y en el set `required` del script del gate.

---

## 9. Lo que NO pude hacer, y por qué

1. **Resolver si existe un fee tier recuperable para V3.** Declarado como **no medido**, no como ausente (§5, bloqueo 3). Medí que las claves de `route_metadata` para filas V3 son exactamente `decimals | dex_adapters | economics_basis | pool_addresses | token_addresses` — ninguna es fee tier — y que 17 117 **filas** tienen algún texto que matchea `%fee%` **sin** resolverlo a una clave. Resolverlo ya es trabajo de implementación, fuera del alcance de esta tarea. **No doy una cifra de esfuerzo** para soportar V3/PancakeSwap: sería inventada.
2. **No corrí el harness con RPC.** El test `variance_benchmark_predicted_vs_settled_block` sigue `#[ignore]` (necesita `RPC_HTTP_1`, `ARBITRAGE_EXECUTOR`, `FLASHLOAN_EXECUTOR_1`, `GAS_PRICE_WEI`, `VARIANCE_INPUT`), y no lo ejecuté: no tengo esas variables ni autorización para correr una simulación contra mainnet. Por lo tanto **no medí** el efecto del cambio en `pred_failed` de una corrida real end-to-end; lo que medí es la emisión de la línea nombrada en el test unitario y la composición de la ventana en la base.
3. **No corrí el workflow de GitHub Actions.** No lancé `workflow_dispatch` del benchmark. La evidencia del paso nuevo es: (a) los tests herméticos que lo recorren, (b) el job de contrato que asserta `continue-on-error` + `exit 0` sobre el YAML, (c) una corrida **real** del camino live del instrumento contra producción (`ssh-target arbx`, `ssh_and_psql_rc=0`, 70 buckets, `window_total=986`, `labelable=13`, `named_total=986`). Lo que **no** está probado es el paso YAML end-to-end dentro del runner.
4. **No corrí `cargo clippy --workspace`** (el paso completo de CI): corrí `cargo clippy -p sim-core --all-targets --locked -- -D warnings`, que es **más estricto** para el target que toco (incluye tests, que el comando de CI no incluye). El clippy del workspace completo no lo corrí porque su resultado no depende de mi cambio y cuesta compilar todo el workspace.
5. **Los números de la ventana son de una ventana rodante.** 977 es la foto de `2026-10-08 13:24:46.406955Z` (fixture commiteado); 962/986 son otras fotos del mismo instrumento, con su instante declarado. `labelable=13` se mantuvo en las tres. Ninguno de esos números es extrapolable a otra hora sin re-medir — el instrumento existe precisamente para eso.

## 10. Regla de forma y límites de lo afirmado

Ningún exit code se cita como prueba del hecho. Cada afirmación cuantitativa lleva su artefacto en la misma cláusula: el fixture con su `sha256`, el `sha256` del archivo Rust probado (`5e06f7fb…` en ambos lados), la línea `Compiling sim-core` del log (`/home/hfrc/cc01-test.log`), la cadena `VARIANCE_BENCH_SKIP={"code":"unsupported_adapter:PancakeSwap V3",…}` impresa por el binario, las líneas exactas de cada cambio, y la salida cruda de cada consulta con su `rc=`.

Donde no hay artefacto, se dice: fee tier de V3 recuperable (**no medido**), corrida end-to-end del harness con RPC (**no corrida**), `workflow_dispatch` del benchmark (**no lanzado**), clippy del workspace completo (**no corrido**). Y `COVERAGE_CEILING_UNAVAILABLE reason=…` está en el código justamente para que una medición ausente nunca se lea como «no hay problema de cobertura»: `COVERAGE_DISCARD_DISTRIBUTION total=0` significaría fallo de medición, no «nada se descartó».
