# ARSE-264-01 — ¿el lote de cartuchos es alcanzable de verdad por el runner?

Medición + decisión. Todo número de este documento tiene su comando y su ruta. Los
comandos se corren desde la raíz del repositorio (el árbol de la rama
`arse-264-01`); los de producción son **solo lectura** (`docker logs`, `redis-cli
GET`), sin escritura, sin reinicio, capital expuesto = 0.

## 1. Conteo reproducible del universo

| Métrica | Valor | Comando / fuente |
|---|---|---|
| `.rhai` en el árbol de la rama | **1341** | `find . -name '*.rhai' -not -path './node_modules/*' -not -path './target/*' -not -path './.git/*' \| wc -l` |
| `.rhai` en el checkout de trabajo (incluye worktrees de otras sesiones) | **14252** | mismo comando en el checkout principal; el `-not -path` NO excluye `.worktrees/` ni `.claude/worktrees/` (ver §6) |
| Lote 264 desplegado | **264** | `ls backend/searcher-rs/cartridges/strategies/*.rhai \| wc -l` |
| Copia generada del lote | **264** | `ls integration/agent-cartridges-v4/generated/cartridges/strategies/*.rhai \| wc -l` |
| Copia de referencia (vendored) | **271** | `find integration/agent-cartridges-v4/reference_repo -name '*.rhai' \| wc -l` |
| Paquete de auditoría | **535** | `find audits/workspace-extreme-audit-2026-09-24/agent_pkg -name '*.rhai' \| wc -l` |
| Registros en el manifiesto | **264** | `strategy_mapping.json` → `total_strategies = 264`, `strategies` = 264 entradas |
| Operadores referenciados por el manifiesto | **31** (IDs 1..31, sin 32) | unión de `primary_operators` ∪ `secondary_operators` ∪ `applicable_operators` ∪ claves de `operator_weights` |
| Cargados + ACTIVE por el runner en producción | **271** | log `cartridge.registry_published` `{"total":271,"active":271}`; clave Redis `arbx:cartridges:registry:1` → `{"active":271,...}` |
| Que pasan por el intérprete real | **271/271** | la carga ejecuta `init_strategy()` en el engine Rhai real (`runner.rs::load_cartridge` → `execute_init_strategy`) y las evaluaciones corren `evaluate_opportunity` |

El lote de 264 es **byte-idéntico** en sus dos ubicaciones
(`backend/searcher-rs/cartridges/strategies/` y
`integration/agent-cartridges-v4/generated/cartridges/strategies/`): 264 hashes
SHA-256 por lado, conjuntos iguales.

## 2. El camino cartucho → oportunidad → card: está COMPLETO hasta el candidato, y se corta en dos gates

Producción medida (2026-10-03, contenedor `arbitragex-v2-searcher-rs-1`,
`ARBX_CARTRIDGE_MODE=active`, `WORKDIR=/app`, 271 `.rhai` presentes en
`/app/cartridges`):

```
cartridge.active_eval_actives  → {"active_count":271}
cartridge.active_eval_pertinent→ {"pertinent_count":269,"source_event":"new_block"}
cartridge.registry_published   → {"total":271,"active":271}
cartridge.active_eval_summary  → {"pertinent":269,"negative":269,"positive":0, ...}
```

**No hay brecha de carga: el lote es alcanzable.** El camino se corta DESPUÉS de
la carga, en dos puntos dentro de `active_evaluate_and_emit`:

1. **Doctrina de despacho del workbook** —
   `backend/searcher-rs/src/cartridge_boot.rs:3050-3056`
   (`signal_tier::mev_id_from_cartridge_id` → `strategy_dispatch_status::disposition(mev_id).may_form_candidate()`):
   bloquea **185/269** del último intent muestreado
   (`dispatch_needs_route_data` 174 · `dispatch_observe_only` 8 ·
   `dispatch_no_compatible_route` 3).
2. **Chequeo económico v4** — el cartucho SÍ evalúa y devuelve
   `is_opportunity=false` con razón `applicable_data_or_constraint_gap`,
   producida en `rhai_agent_bridge.rs:671` (el `reason` del
   `economic_check`) y contabilizada en
   `backend/searcher-rs/src/cartridge_boot.rs:3090-3117`
   (`negative_reasons` + `negative_repairs` = el campo `top_repairs` del summary).
   Bloquea **64/269**, con este histograma de reparaciones:
   `capital_usd::capital_missing_or_cap_exceeded` 64 ·
   `costs.execution_fees::missing_or_invalid_cost` 64 ·
   `costs.financing::mandatory_route_cost_missing` 64 ·
   `operators.22::DATA_GAP` 64 · `operators.21::DATA_GAP` 60 ·
   `protocol_exact_quotes::v3_within_tick_is_hypothesis_not_protocol_verified` 60 ·
   `operators.{16,8,11,13,26,5}::DATA_GAP` 22-32 c/u.
   (Corte LATENTE adicional, no alcanzado en la ventana medida:
   `cartridge_boot.rs:3127-3134` intercepta un proposal v4 **elegible** y lo emite
   como fila REJECTED `agent_v4_<status>_snapshot_store_not_wired` —
   `cartridge.active_v4_intercepted` = 0 ocurrencias, porque ningún cartucho llega
   con `candidate_eligible=true` mientras falten los productores de costos.)
3. El resto: `native_domain_solver_required` 12, y colas de 1-3.

**Sobre la observabilidad de estos dos gates:** los eventos por-cartucho
(`cartridge.active_dispatch_blocked`, `cartridge.active_eval_negative`) son
`debug!`, y el contenedor corre con `RUST_LOG=info,searcher_rs::v3_quote_provider=debug,searcher_rs::state_projector=debug`
— por eso su conteo en la ventana es **0** aunque el histograma agregado
(`info!`) reporte 185 y 64. La evidencia válida es el `active_eval_summary`
(agregado, R9/LOGFLOOD-01), no la ausencia de los eventos de debug.

Es decir: **la carga y el intérprete funcionan; lo que falta son PRODUCTORES**
(de capital/costos exactos y de cotizaciones protocolo-verificadas) y la decisión
de doctrina del workbook. `positive_total = 0` no es un fallo del lote.

## 3. Defecto real del runner que SÍ dejaba cartuchos inalcanzables (corregido acá)

**Síntoma medido:** 385 `cartridge.active_eval_error` =
`runtime error: Size of object map too large` en la ventana retenida, repartidos
en 24 cartuchos, 7 de ellos estructurales (~50 cada uno:
`mev_03_001..006_*`, `mev_01_022_non_cyclic_inventory_arbitrage`).

**Causa raíz (evidencia en la dependencia, no en prosa):** el límite de rhai
`set_max_map_size(N)` **no** acota las claves de UN map: acota el **total
ACUMULADO** de entradas de map en todo el árbol de valores.
`rhai-1.25.1/src/eval/data_check.rs:61-93` (`calc_map_sizes`) acumula `mx`
recursivamente por arrays y maps anidados, y `data_check.rs:143-147`
(`throw_on_size`) compara ese total contra `limits.map_size`. El chequeo se aplica
al valor devuelto por la función del script (`rhai-1.25.1/src/func/call.rs:412`).

**Por qué mataba cartuchos legítimos:** `agent_v4_seal`
(`rhai_agent_bridge.rs`) devuelve el proposal sellado COMPLETO — `observations[]`
(una entrada por candidato evaluado, con su ledger de quote y su evidencia por
operador) más el grafo `discovery` entero. Con `MAX_MAP_SIZE = 1024` ese árbol
superaba el total acumulado aunque ningún map individual fuera grande, y TODO el
`evaluate()` fallaba.

**Fix:** `MAX_MAP_SIZE = 8_192` en
`backend/searcher-rs/src/cartridge/runner.rs`, derivado de los propios topes del
contrato v4 (`max_evaluations = 8` × ≈300 entradas por observación + `discovery`
≈480 ≈ 2_900 peor caso ⇒ ~2.8× de holgura; ~1 MB por valor devuelto a ~120 B por
entrada, con `SHADOW_MAX_CONCURRENCY = 16`). La protección contra bucles sigue
siendo `MAX_OPERATIONS = 1_000_000`, que es la que de verdad acota el runaway.
El test `map_size_is_cumulative_over_the_whole_tree` FIJA la semántica: el mismo
payload debe ser rechazado con 1_024 y aceptado con el valor actual.

## 4. Decisión 31 vs 32 (operadores) — escrita y verificada por test

Los dos números son ciertos en ejes distintos y **ambos se mantienen**:

* **RUNTIME = 32** — `backend/math-engine/src/operators/mod.rs`:
  `OPERATOR_COUNT = 32`; el bloque `register!` registra IDs 1..=32; op_32 =
  NSGA-II (`op_32_nsga2/`).
* **CATÁLOGO = 31** — `SOURCE_OPERATOR_COUNT = 31` (mismo archivo). La Master
  Matrix (`strategy_mapping.json`, 264 registros) y el puente de cartuchos
  (`backend/searcher-rs/src/native_operator_adapter.rs:95`
  `.filter(|i| (1..=31).contains(i))`, y su contrato
  `"source_operator_count": 31`) sólo referencian 1..=31. **op_32 queda
  DELIBERADAMENTE desconectado de los cartuchos.**

Motivos (en orden de peso):

1. **Forma incompatible.** op_32 devuelve un FRENTE de Pareto: `matrix_result` con
   el conjunto no dominado, `vector_result` con los ÍNDICES de los supervivientes
   y `scalar_value` = cardinalidad del frente. El puente construye un receipt de
   evidencia por operador comparable con los 31 escalares/vectoriales; un frente
   multi-objetivo no es una figura ponderable contra ellos.
2. **La matriz de datos no tiene columna 32** (medido: unión = {1..31}). Inventarla
   para "hacer coincidir los números" está prohibido: la matriz es dato del
   operador y el código se adapta a ella, nunca al revés.
3. **Cablearlo no hace alcanzable nada**: ningún cartucho declara op_32, así que
   ampliar la admisión a 1..=32 agregaría un camino sin llamadores y debilitaría
   una frontera explícita.

Invariante ejecutable (falla CI si driftea) — `real_ops_tests::source_catalog_boundary`:

* `runtime_registry_is_exactly_operator_count`
* `source_catalog_is_31_and_op_32_is_the_single_extra_id`
* `op_32_is_the_only_multiobjective_operator_and_returns_a_front`

Ningún registro de `strategy_mapping.json` se tocó.

## 5. Contrato v4 en uso (y el lector v3 f64)

* `runner.rs::parse_eval_result` detecta PRIMERO `contract_version ==
  "arbx.cartridge.agent/4"`, valida con `crate::proposal_contract::ProposalV4::parse`
  y **falla cerrado** (`RuntimeError("invalid_v4_proposal: …")`) ante un envelope
  v4 malformado. Un envelope v4 no puede degradarse a un candidato de beneficio
  cero.
* El lector legacy de f64 (`estimated_profit` / `confidence`, con `unwrap_or(0.0)`)
  sigue existiendo SOLO para cartuchos v3 y ahora se marca explícitamente en
  `metadata["v3_f64_authoritative"]` (false para v4): un `0.0` en un cartucho v4 es
  una AUSENCIA, nunca un cero computado (R8).
* `build_payload` mantiene `approved_for_execution = false`; el propio
  `ProposalV4::parse` rechaza `approved_for_execution = true`
  (`cartridge_cannot_self_authorize_execution`). Tests:
  `v4_envelope_goes_through_proposal_v4_parse`,
  `legacy_v3_result_stays_authoritative_for_v3_cartridges`.

## 6. Validación estática de patrones prohibidos de Rhai 1.25.1 — NO es ejecución

Barrido sobre TODOS los `.rhai` del árbol (`--include='*.rhai'`):

| Patrón prohibido | Hits reales | Detalle |
|---|---|---|
| destructuring en `let` (`let (a,b) = …`) | **0** | el único match es el comentario `backend/searcher-rs/cartridges/dex_arb.rhai:395` ("Rhai no soporta `let (a, b) = …`") |
| tuplas en `return` (`return (a,b)`) | **0** | — |
| optional chaining (`?.`) | **0** | — |
| spread (`...`) | **0** | 520 matches crudos; **todos** son prosa de comentario o el texto `...` DENTRO de strings de documentación (p. ej. `"equation": "Q_R(x)=q_n(...q_2(q_1(x)))"`, `strikes: [K1,...]`). Cero fuera de comentario-y-string |
| archivos afectados (patrón combinado crudo) | 0 reales / 309 falsos positivos | 309 = archivos con `...` en string/comentario |

El comando literal del contrato incluye `cartridges/` (no existe en la raíz; el
directorio real es `backend/searcher-rs/cartridges/`) y barre `.rs`, por lo que su
salida cruda se llena de destructuring de RUST (`let (r0, r1) = …`) y de `x?.y` de
Rust. El barrido acotado a `.rhai` es el que responde la pregunta del intérprete.

**Esto NO es ejecución.** Que un archivo pase el barrido estático no dice que el
intérprete Rhai lo acepte: el lote 264 se generó con validación estática solamente
y ya hubo un incidente (cartucho triangular con dos errores de sintaxis y un fallo
funcional). La única prueba válida es el intérprete real — que en este host no
corre (§7) y en producción sí (271/271 cargados + evaluados).

## 7. Gate de compilación

`cargo check -p math-engine` en este host **NO puede pasar**: Windows AppControl
bloquea la ejecución de los build scripts (`os error 4551`, exit 101) — los
crates `libm`, `serde`, `icu_normalizer_data` ni siquiera llegan a ejecutarse.
El gate se acredita con la corrida de CI (`rust.yml`/`ci.yml`) sobre esta rama
única `arse-264-01`; el resultado crudo del run se adjunta en el reporte de la
tarea, y si la corrida sale en rojo se reporta en rojo, nunca como verde.
