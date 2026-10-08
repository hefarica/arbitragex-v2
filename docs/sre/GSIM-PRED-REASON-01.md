# GSIM-PRED-REASON-01 — razón por muestra para `pred_failed`, y por qué 283 de 305 topologías se descartan

**Alcance (InScope de t127):** `.github/workflows/`, `docs/sre/`. Todo lo demás (`backend/`, `frontend/`,
`contracts/`, `monitoring/`, `docker/`, `docs/data/`, `docs/release/`) se leyó **read-only**.
**Doctrina:** paper por defecto, sin firma, sin broadcast, sin deploy, sin merge. Cero escrituras en producción.

---

## 0. Resumen ejecutivo (4 líneas, cada número con su artefacto)

1. `pred_failed=22/22` **nombraba la categoría, nunca la muestra**: el único artefacto que lo contiene es el
   bloque agregado `skips` del log del harness (`variance-benchmark.log:662` del run `37735084659`), y la
   cadena `sample_id` aparece **0 veces** en las 671 líneas de ese log (conteo propio, §6).
2. El productor de esa razón es **`backend/sim-core/tests/variance_benchmark.rs:483-486`**
   (`hist.pred_failed += 1; continue;`) — **fuera de mi InScope** ⇒ **STOP declarado** (§5) con el parche
   mínimo propuesto, no aplicado.
3. Lo que **sí** es medible hoy sin tocar el harness: la **atribución por muestra contra producción**
   (read-only), implementada como paso nuevo del workflow con códigos cortos, estables y agrupables (§3).
   **Medido en la corrida `37736725663`:** `SAMPLE_REASON_DISTRIBUTION total=22
   sim_failed:sim_signer_funding_slot_unresolved=4 no_simulation_row=18`
   (`sample-reasons.log`, §6.2) — es decir, 4 muestras con simulación fallida por causa nombrada y **18 sin
   ninguna fila de simulación** (no medidas, no "cero").
4. El descarte de **283 de 305** topologías es **del harness, no del productor**:
   `scripts/gsim1_variance_export.sql:62-64` filtra por el allowlist de su propio encoder
   (`<@ '["UniswapV2","SushiSwap"]'`, espejo de `adapter_to_semantic()`), y el driver **lo imprime él mismo**
   en `scripts/gsim1_variance_benchmark.sh:181-185` (§4).

---

## 1. R1 — el estado medido ANTES del cambio

**Artefacto:** log preservado del run `37735084659` (workflow_dispatch, `headSha=8414e51211d0a26d664b7e669af2eacbee8d1dd4`,
descargado con `gh run download 37735084659` → `variance-benchmark.log`, 22.339 B, 671 líneas).

Línea 662 (JSON del harness), verbatim:

```
VARIANCE_BENCH_JSON={...,"samples_labeled":0,...,"pass_reason":"sample-floor",...,"skips":{"attempted":22,"bad_shape":0,"decimals_failed":0,"dedup":0,"encode_failed":0,"obs_failed":0,"pred_failed":22,"stale_timestamp":0,"unsupported_adapter":0,"zero_predicted":0},...}
```

Línea 661 (marcador greppable), verbatim:

```
VARIANCE_BENCH_OUTCOME=FAIL samples=0 mean_abs_drift_pct=n/a p95=n/a max=n/a (min_samples=22, threshold=5%, distinct_exported=22)
```

Lecturas **honestas** de esos bytes:

| Hecho | Artefacto | Lectura |
|---|---|---|
| `pred_failed=22` de `attempted=22` | `variance-benchmark.log:662` | el 100% de las muestras exportadas murió en el paso `pred` |
| `obs_failed=0`, `unsupported_adapter=0`, `stale_timestamp=0`, `bad_shape=0`, `zero_predicted=0`, `encode_failed=0`, `decimals_failed=0`, `dedup=0` | `variance-benchmark.log:662` | el fallo es **de una sola rama** (`pred`), no un problema de export/encode/forma |
| ninguna línea identifica la muestra | `sample_id` aparece **0** veces en 671 líneas | **cero de extracción**, declarado como tal: el log no trae identidad por muestra |
| existe un `harness.log` completo | `variance-benchmark.log:670` (`full harness log: /tmp/gsim1/harness.log`) | ese archivo **no** se preserva como artefacto (no está en el upload); el log preservado ya contiene el JSON íntegro |

**Cómo muere la razón, en el código (leído del objeto git, no del working tree):**

- `backend/sim-core/tests/variance_benchmark.rs:93-103` — `struct Histogram { …, bad_shape, …, pred_failed, obs_failed, zero_predicted }`: **contadores por categoría**, sin campo de identidad.
- `:483-486` — el sitio exacto:
  ```rust
  if !pred.passed {
      hist.pred_failed += 1;
      continue;              // ← aquí se descarta la identidad de la muestra
  }
  ```
- `:561-572` — el JSON de salida emite **solo** los agregados (`"skips": { "attempted": …, "pred_failed": hist.pred_failed, … })`.

⇒ La información existe por muestra **dentro del bucle** (donde `row` está en scope) y se **colapsa** a un
contador antes de salir. Eso es exactamente lo que R1 pide dejar de hacer.

---

## 2. R2 — lo que NO se movió (el criterio de aprobación está intacto)

| Elemento del criterio | Valor medido antes | Valor después del cambio | Cómo se garantiza |
|---|---|---|---|
| `samples_labeled > 0` | `samples_labeled=0` (`log:662`) | igual (paso nuevo no toca el harness) | el paso sólo hace `SELECT`; no reescribe la fila de registro |
| `pass_reason` | `sample-floor` (`log:662`) | igual | se decide en `variance_benchmark.rs:536-542`, no en el YAML |
| umbral de drift | `threshold_mean_pct=5.0` (`log:662`) | igual | `-e` de entorno del driver, no modificado |
| `min_samples` (piso de muestra) | `min_samples=22` = población (`log:661`) | igual | `gsim1_variance_benchmark.sh:208` (`MIN_SAMPLES=${VARIANCE_MIN_SAMPLES:-$ROWS}`), no modificado |
| fila del registro | `status=failed` (`gsim1-rows.jsonl`, run `37735084659`) | igual | el paso nuevo no escribe en `readiness_evidence` |
| modo `strict` | falla sólo con `driver_rc != 0` | igual | último paso, no modificado |

Además: el paso nuevo es `if: always()` + **`continue-on-error: true`** y toda su ruta de anomalía termina en
`exit 0` con un `::warning::`. Es decir: **no puede enrojecer el gate**. Un instrumento observacional que
alterara el veredicto sería fabricación de métrica; este no lo hace por construcción.

---

## 3. R1 (entrega) — el paso nuevo: atribución por muestra, read-only, advisory

Archivo: `.github/workflows/gsim1-variance-benchmark.yml` (paso `Attribute each exported sample to an
observable cause (read-only, advisory)`, insertado entre la lectura del registro y el `assert-fresh`).

**Qué mide (y qué NO).** Para cada topología exportada (las 22 de `/tmp/gsim1/input.jsonl`, el mismo archivo
que consumió el harness en esa misma corrida) consulta producción y emite un código:

| Código | Significado exacto | Fuente |
|---|---|---|
| `no_simulation_row` | la oportunidad **no tiene ninguna fila** en `simulations` → su simulación **NO ESTÁ MEDIDA** (no es "falló", no es "0") | `LEFT JOIN simulations` → `s.sim  IS NULL`, `opportunities.id` |
| `sim_failed:<fail_reason>` | hay fila(s) y la **última** tiene `passed=false`; el sufijo es el `fail_reason` literal (`unnamed` si es NULL/vacío) | `simulations.passed`, `simulations.fail_reason` |
| `sim_passed` | hay fila(s) y la última tiene `passed=true` → el estado de producción **no explica** el fallo del `pred` del harness | `simulations.passed` |

Salida por muestra (una línea, greppable):

```
sample_reason opportunity_id=<uuid> status=<status> rejection=<rejection_reason|-> pair=<pair_symbol|-> sim_rows=<n> code=<CODE>
```

y una línea agregable:

```
SAMPLE_REASON_DISTRIBUTION total=<T> <code>=<n> ...
```

**Frontera declarada en la propia salida** (R8 — nunca leer una etiqueta como otra cosa):

```
SAMPLE_REASON_HARNESS_INTERNAL=not_observable_here producer=backend/sim-core/tests/variance_benchmark.rs:483-486
```

Esto es un código de **estado de producción de la muestra**, NO el código de error interno del harness
(por qué `sim_at_block` devolvió `passed=false`). Llamarlo "la causa del `pred_failed`" sería exactamente el
modo de fallo (d) de la doctrina: una etiqueta leída como otra cosa. Lo que sí hace es convertir un
`22/22` sin atribución en una **distribución sobre causas observables**, que es medición.

**Higiene del instrumento** (defectos de contrato ya pagados en esta sesión, evitados aquí):

- primera línea del ledger remoto: `tool_docker=$(command -v docker || echo MISSING) input=<ruta>` — la
  ausencia de herramienta se ve como literal, no se infiere de un `grep -c = 0`;
- si `/tmp/gsim1/input.jsonl` no existe o está vacío → `SAMPLE_REASON_UNAVAILABLE reason=input_jsonl_absent_or_empty`
  y `exit 0` (explícito, no silencioso);
- si la extracción de UUIDs devuelve 0 → `SAMPLE_REASON_UNAVAILABLE reason=no_opportunity_id_extracted …`
  con la nota `(extraction zero, not an absence of samples)`;
- `psql_rc=` y `rows_returned=` se imprimen **antes** de interpretar nada;
- el SQL no usa `stdin`: se pasa por `-c` inline (evita la pelea `bash -s` vs `docker exec -i` ya documentada);
- `-t -A -F'|'` + `array_agg(... ORDER BY simulated_at DESC NULLS LAST)[1]` para "la última simulación",
  declarado como tal (no "la simulación representativa").

---

## 4. R3 — el descarte de 283 de 305: **es del harness**, con archivo:línea

**Artefacto primario:** `scripts/gsim1_variance_export.sql:57-73` (el `WHERE` del export), en particular:

```sql
L58: WHERE o.chain_id = 1
L61:   AND o.route_metadata ? 'dex_adapters'
L62:   AND jsonb_array_length(o.route_metadata->'dex_adapters') = 2
L63:   -- A.3.a encoder-supported legs only (mirror of adapter_to_semantic()).
L64:   AND (o.route_metadata->'dex_adapters') <@ '["UniswapV2","SushiSwap"]'::jsonb
```

La línea **L64** es el descarte: contiene (`<@`) el array de adapters dentro de `{UniswapV2, SushiSwap}`.
El comentario de la propia línea dice la razón: **espejo de `adapter_to_semantic()`** — la función del
harness que traduce un adapter a la semántica que el encoder A.3.a sabe ejecutar
(`backend/sim-core/tests/variance_benchmark.rs:111 fn adapter_to_semantic(label: &str)`).

Y el encabezado del mismo archivo (`scripts/gsim1_variance_export.sql:12-20`, según su texto) declara que
las patas V3/PancakeSwap son *honestamente UNSUPPORTED* por ese encoder, de modo que exportarlas solo
inflaría el contador `unsupported_adapter` con filas que el harness **nunca podría etiquetar**.

**Contra-verificación (el driver mide la brecha él mismo), `scripts/gsim1_variance_benchmark.sh:181-185`:**

```bash
L182:  "SELECT count(*) FROM (SELECT DISTINCT dex_a, token_in, token_out, route_metadata->'pool_addresses'
        FROM opportunities WHERE chain_id = 1 AND route_metadata ? 'dex_adapters'
        AND jsonb_array_length(route_metadata->'dex_adapters') = 2 AND detected_at > now() - interval '2 hours') t;"
L184:  if [ "$TOTAL_2LEG" -gt "$ROWS" ]; then UNENCODABLE=$(( TOTAL_2LEG - ROWS )); else UNENCODABLE=0; fi
L185:  echo "distinct 2-leg topologies in window (all adapters): $TOTAL_2LEG (not A.3.a-encodable: $UNENCODABLE)"
```

y el log del run `37735084659` lo imprime en sus **dos primeras líneas**:

```
L1: exported 22 distinct labelable topologies (A.3.a-encodable 2-leg, chain 1, 2h window)
L2: distinct 2-leg topologies in window (all adapters): 305 (not A.3.a-encodable: 283)
```

**Veredicto R3 (sin ambigüedad):**

- **Es del harness.** Las 283 topologías **existen** en `opportunities` (el propio driver las cuenta con el
  mismo `DISTINCT` sobre la misma ventana de 2 h), y se descartan porque el **encoder del benchmark no sabe
  codificar sus patas**, no porque el productor las haya descartado.
- **Cuantificación:** 22/305 = **7,2%** de las topologías 2-leg de la ventana son etiquetables por el
  benchmark (22 y 305 medidos en `variance-benchmark.log:1-2`). El techo de cobertura del benchmark es su
  propio encoder.
- **No está escondido:** el descarte está comentado en el SQL y **medido e impreso** por el driver. Es una
  limitación declarada, no un silencio.

*Nota de alcance:* el encabezado del SQL y `adapter_to_semantic()` están en `scripts/` y `backend/`, ambos
**fuera** de mi InScope: los cito, no los modifico.

---

## 5. R5 — **STOP declarado**: el parche que R1 pediría en el harness, y por qué no lo aplico

R1 pide que el benchmark **emita una razón por muestra**. El punto donde esa razón existe y se tira es
`backend/sim-core/tests/variance_benchmark.rs:483-486`, y `backend/` está **explícitamente fuera** del
InScope de t127 (read-only). Por lo tanto:

> **No aplico este cambio. Lo declaro como propuesta para el dueño del harness.**

Parche mínimo propuesto (texto, **no aplicado**), en el mismo estilo del archivo — el `row` ya está en scope
y el valor devuelto por `sim_at_block` (firma en `:256`) ya expone `passed` (`:483`) y
`simulated_profit_token_in` (`:493`):

```rust
        let pred = sim_at_block(&rpc, block_b, &ctx, &exec_cfg);
        if !pred.passed {
            hist.pred_failed += 1;
            // GSIM-PRED-REASON-01: la identidad de la muestra no debe morir aquí.
            eprintln!(
                "VARIANCE_BENCH_SKIP={{\"opportunity_id\":\"{}\",\"code\":\"pred_failed\"}}",
                row.opportunity_id
            );
            continue;
        }
```

Análogo en `:488-491` (`obs_failed`) y `:495-499` (`zero_predicted`), con `code` respectivo. Dos precisiones
que **no puedo cerrar sin leer el tipo de retorno** (y que el dueño debe verificar antes de wirear):

1. Si el valor de `sim_at_block` expone un campo de error/motivo, el `detail` debe salir **de ese campo**, no
   de un texto inventado por el parche;
2. el consumo del marcador nuevo debe hacerse donde ya se parsea `VARIANCE_BENCH_JSON`
   (`scripts/gsim1_variance_benchmark.sh:228-229`), y ese archivo también está fuera de mi InScope.

**Qué queda entonces pendiente y qué no:** la *causa interna del harness* queda **no medida** (declarada,
no disfrazada). La *atribución observable por muestra* queda **medida** (§3, resultados en §6).
Ninguna de las dos se llama con el nombre de la otra.

---

## 6. R4 — corrida REAL en el runtime vivo

| Campo | Valor | Artefacto |
|---|---|---|
| workflow | `G-SIM-1 Evidence — variance benchmark (scheduled, G-SIM1-AUTOREFRESH)` | `.github/workflows/gsim1-variance-benchmark.yml:1` |
| run id | `37736725663` | `gh run list --workflow gsim1-variance-benchmark.yml --branch sre/gsim-pred-reason-01` |
| `headSha` | `ae7967fc576b9426d3d1a0b97ee3fc7b1faf1756` | idem + `git rev-parse HEAD` en el clon aislado |
| evento | `workflow_dispatch` | idem |
| rama | `sre/gsim-pred-reason-01` (base `origin/main`) | `git branch --show-current` |

### 6.1 Los artefactos de esta corrida (medidos, no citados)

`gh run download 37736725663` → 3 archivos: `variance-benchmark.log` (22.339 B, 671 líneas),
`gsim1-rows.jsonl` (4.802 B, 7 líneas), **`sample-reasons.log` (4.208 B, 27 líneas)** ← el artefacto nuevo.

Embudo de esta corrida (`variance-benchmark.log:1-3`):

```
L1: exported 22 distinct labelable topologies (A.3.a-encodable 2-leg, chain 1, 2h window)
L2: distinct 2-leg topologies in window (all adapters): 307 (not A.3.a-encodable: 285)
L3: sample floor for this run: 22 (population 22)
```

El marcador del harness en esta corrida (`variance-benchmark.log:661-662`) es **idéntico** al del run base
en todo lo que decide el veredicto: `VARIANCE_BENCH_OUTCOME=FAIL samples=0 … (min_samples=22, threshold=5%,
distinct_exported=22)` y `"samples_labeled":0, "pass_reason":"sample-floor", "skips":{…,"pred_failed":22,…}`.

### 6.2 La distribución medida (R1, ANTES `22/22` sin causa → AHORA atribuido)

`sample-reasons.log`, verbatim (cabecera + cierre):

```
tool_docker=/usr/bin/docker input=/tmp/gsim1/input.jsonl
exported_rows=22
psql_rc=0 rows_returned=22
…
SAMPLE_REASON_DISTRIBUTION total=22 sim_failed:sim_signer_funding_slot_unresolved=4 no_simulation_row=18
SAMPLE_REASON_HARNESS_INTERNAL=not_observable_here producer=backend/sim-core/tests/variance_benchmark.rs:483-486
```

**`total=22` con `psql_rc=0`: las 22 muestras del export quedaron atribuidas, ninguna se perdió.**

Las 4 muestras con fila de simulación (las únicas que **sí** fueron simuladas, y todas por la misma causa):

```
sample_reason opportunity_id=0249f64a-47de-4275-8eed-626f606cb82e status=rejected rejection=single_pool_no_spread pair=a606d4…/c02aaa… sim_rows=1 code=sim_failed:sim_signer_funding_slot_unresolved
sample_reason opportunity_id=082e835f-d80a-4b04-a35b-1b2ca52bb356 status=rejected rejection=non_positive_profit pair=dac17f…/c02aaa… sim_rows=1 code=sim_failed:sim_signer_funding_slot_unresolved
sample_reason opportunity_id=ae15b172-d3b8-4a44-9a9a-4d1fb4cae32f status=rejected rejection=negative_net_profit  pair=c02aaa…/1f9840… sim_rows=1 code=sim_failed:sim_signer_funding_slot_unresolved
sample_reason opportunity_id=31ec95dc-dbcb-430a-a6f5-8182d9d160c0 status=rejected rejection=non_positive_profit pair=c02aaa…/970b9b… sim_rows=1 code=sim_failed:sim_signer_funding_slot_unresolved
```

Las otras 18 (listadas completas en el artefacto) son todas `code=no_simulation_row`, `sim_rows=0`.

**Lectura honesta de ese `18`:** NO significa "18 fallaron la simulación". Significa **18 no tienen ninguna
fila en `simulations`**: su simulación **no está medida** (R8: `None` ≠ `0.0`). Por eso el código se llama
`no_simulation_row` y no `sim_missing` ni nada que suene a fallo.

### 6.3 Composición de la población (re-medida independientemente en esta corrida)

Contando los `rejection=` de las 22 líneas de `sample-reasons.log`:

| `rejection_reason` | muestras | `status` |
|---|---|---|
| `non_positive_profit` | 14 | `rejected` (22/22) |
| `single_pool_no_spread` | 7 | `rejected` |
| `negative_net_profit` | 1 | `rejected` |

**14/7/1 sobre 22, todas `rejected`.** Esto **reproduce exactamente** la composición que t118 midió por otro
camino de consulta: dos instrumentos independientes, mismo número ⇒ la medición es estable y no un artefacto
de una query. (Y corrobora el 18/4 de t118: 4 con fila, 18 sin fila — mismos 4, mismas 18.)

### 6.4 R2 — probado por medición, no por afirmación

Comparación **campo por campo** de la fila `variance_benchmark` del registro entre el run base
(`37735084659`) y esta corrida (`37736725663`), leyendo ambos `gsim1-rows.jsonl`:

| Campo | base | con el paso nuevo | veredicto |
|---|---|---|---|
| `status` | `failed` | `failed` | **igual** |
| `pass_reason` | `sample-floor` | `sample-floor` | **igual** |
| `samples_labeled` | `0` | `0` | **igual** |
| `min_samples_required` | `22` | `22` | **igual** |
| `threshold_mean_pct` | `5` | `5` | **igual** |
| `coverage` | `0` | `0` | **igual** |
| `p95_estimable` | `True` | `True` | **igual** |
| `mean/p95/max_abs_drift_pct` | `null` ×3 | `null` ×3 | **igual** |
| `skips` | `{…,"pred_failed":22,…}` | `{…,"pred_failed":22,…}` | **byte-idéntico** |
| `distinct_topologies_exported` / `population_size` | `22` / `22` | `22` / `22` | **igual** |
| `harness_rc` | `0` | `0` | **igual** |
| `distinct_all_adapters` | `305` | `307` | **cambia** — ventana de mercado de 2 h |
| `tip_block` | `26145751` | `26145833` | **cambia** — cabeza de cadena |

Los dos únicos campos que cambian son **entradas de mercado**, no criterios: la ventana de 2 h y la altura
de bloque avanzaron entre las dos corridas. **Ningún campo que decida el veredicto se movió**, y el `skips`
del registro sigue diciendo `pred_failed=22` sin que mi paso lo haya tocado.

Pasos 9-10 de la corrida nueva, ambos `success` (la atribución no rompió el gate):

```
step 9  success  Assert the producer actually delivered in THIS run
step 10 success  Annotate the honest checklist state
step 11 success  Job summary        ← incluye el bloque per-sample
step 12 success  Preserve the harness log   ← ahora también sample-reasons.log
step 14 skipped  Enforce strict mode (manual dispatch only)
```

Y el log del driver cierra igual que el base (`variance-benchmark.log:667-671`):
`benchmark outcome: FAIL → registry status: failed`, `RECORDED: status=failed population=22 min_samples=22`,
`G-SIM-1 item variance_benchmark stays PENDING — the recorded row carries the measured reason.`


---

## 7. R7 — evidencia read-only y fronteras

- VPS: **sólo lectura**. `docker exec arbitragex-v2-postgres-1 psql … -c "SELECT …"` (el paso nuevo), sin
  `INSERT/UPDATE/DELETE/DDL`, sin firma, sin broadcast, sin capital, sin reinicios, sin tocar `.env`.
- Repo ajeno/harness: **sólo lectura** (`git grep`, `git show`, `git ls-tree` sobre el objeto git — nunca
  sobre el working tree compartido, cuyo `frontend/` está vacío según el aviso del capitán).
- Escrituras realizadas: `.github/workflows/gsim1-variance-benchmark.yml` (en mi rama) y este documento.
- Sin merge, sin push a `main` (un push a `main` = deploy por `auto-deploy-vps.yml`).

Comandos clave y su resultado (los que sostienen las afirmaciones de arriba):

| Comando | Resultado |
|---|---|
| `gh run download 37735084659` | `gsim1-rows.jsonl` (4.802 B, 7 líneas) + `variance-benchmark.log` (22.339 B, 671 líneas) |
| conteo de `sample_id` en ese log | **0** ocurrencias sobre 671 líneas |
| `git ls-tree -r --name-only HEAD -- backend/sim-core/tests` | `backend/sim-core/tests/variance_benchmark.rs` |
| `git grep -n -E "pred_failed\|obs_failed\|..." HEAD -- backend/sim-core/tests/variance_benchmark.rs` | incrementos en `:354`, `:425`, `:484`, `:489`, `:497`; campos en `:97-103`; JSON en `:561-572` |
| `git show HEAD:backend/sim-core/tests/variance_benchmark.rs` | 638 líneas (blob) |
| `python -c "yaml.safe_load(...)"` sobre el workflow | `YAML OK: steps=13`, el paso nuevo con `if=always()` |
| bytes del workflow | `CR=0 LF=299` (LF-puro: el heredoc remoto `bash -s` no puede recibir un `\r`) |

---

## 8. R6 — contexto que este cambio NO arregla (y hay que decir)

La población exportada son **22 muestras ya rechazadas** por el pipeline (composición **re-medida en esta
corrida**, contada de `sample-reasons.log`: 14 `non_positive_profit`, 7 `single_pool_no_spread`,
1 `negative_net_profit`, §6.3). Un benchmark que sólo puede
etiquetar rutas que el pipeline **ya descartó** no puede atestiguar fidelidad económica: mide el error de
predicción sobre el subconjunto que el productor consideró no viable.

La atribución por muestra **hace visible esa composición** (cada línea lleva `status` y `rejection_reason`),
pero **no la cambia**. Cambiarla requiere ampliar lo etiquetable — es decir, el encoder del harness (§4) —
y eso está fuera de este InScope.

---

## 9. Límites declarados (lo NO medido)

- **No medido:** la rama interna exacta por la que `sim_at_block` devolvió `passed=false` para cada muestra.
  Productor identificado (`variance_benchmark.rs:483-486`), alcance ajeno, §5.
- **No preservado:** `/tmp/gsim1/harness.log` (citado en `variance-benchmark.log:670`) no forma parte del
  upload de artefactos; el log preservado sí contiene el JSON íntegro. No lo doy por leído.
- **No medido aquí:** por qué el 85,7% de las oportunidades de producción traen
  `sim_signer_funding_slot_unresolved` (medición de t118, no re-medida en esta tarea).
- **`0` vs "no computado":** `SAMPLE_REASON_DISTRIBUTION total=0` significaría **fallo de medición**, no
  "ninguna muestra falló". Los códigos de `exit 0` con `SAMPLE_REASON_UNAVAILABLE` existen justamente para
  no dejar ese cero sin dueño.
