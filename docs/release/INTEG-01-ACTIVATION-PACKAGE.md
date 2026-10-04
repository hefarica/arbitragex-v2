# INTEG-01 — Paquete de activación (NO ejecutado)

> **Estado**: listo para revisión del OPERADOR. **Este documento no despliega nada.**
> La activación (deploy a `/opt/arbitragex-v2`) es acto del operador (§34.5). Aquí solo está
> el changeset ensamblado, auditado y verificado, y la verificación post-deploy que decide si
> la activación sirvió.
>
> Fecha de ensamblado: 2026-10-04 · Autor: `integrador` (INTEG-01 / t5)
> Protocolo de entrada aplicado: §32/§33 audit/scaffold/shadow/READ-ONLY. Capital expuesto = 0.

---

## 1. Veredicto del integrador

**El changeset se ensambla, compila y typechequea; sus tests pasan. NO está verde al 100%:
un check de CI queda ROJO por la implementación A.8 y otro ROJO viene heredado de `main`.**

| # | Check | Estado | ¿Causado por este changeset? |
|---|-------|--------|------------------------------|
| 1 | `Rust CI` — cargo fmt/check/clippy/test | **VERDE** (run 37171449284) | — |
| 2 | `TypeScript CI` — tsc all workspaces + tests | **VERDE** (run 37171449228) | — |
| 3 | `opportunities-fidelity-gate`, `unit-tests`, `Frontend Build`, `Semiotic-Bridge`, `Security Scans` | **VERDE** | — |
| 4 | `TypeScript integration (live Postgres + Redis)` | **ROJO** | **SÍ — introducido por A.8 (t2)**. Evidencia en §5. |
| 5 | `Rust integration (live Postgres + Redis)` | **ROJO** | **NO — pre-existente en `main` 697dc472** (t13 OP21-CONTRACT-01). Evidencia en §6. |

Consecuencia operativa: el check 4 **no invalida el deploy** (es una carencia del *fixture* de
tests de integración, no un defecto del camino de producción — ver §5.2), pero **impide declarar
el changeset "verificado al 100%"**, y su reparación es de una línea. La decisión de activar con
ese check rojo es del operador; el integrador no la toma.

---

## 2. Qué se activa

Base: `origin/main` @ **697dc472** (`git merge-base origin/main HEAD` = 697dc472;
`git rev-list --count HEAD..origin/main` = 0).

| ID | Implementación | Rama origen (PR) | Head de origen | CI de la rama de origen |
|----|----------------|------------------|----------------|--------------------------|
| t1 | V3-QUOTE-01 + CATALOG-HYGIENE-01 + CATALOG-ADMISSION-WIRE-01 + QUOTE-TRUTH-ADMISSION-01 | `ci/cargo-gate-v3quote-01` (#786) | `a71b1e3b` | verde |
| t3 | ARSE-264-01 (alcanzabilidad del lote 264, coherencia 31 vs 32) | `arse-264-01` (#785) | `f3557633` | verde |
| t2 | A8-CONF-01 (wire de confidence scoring, gate A.8) | `ci/a8-conf-gate-01` (#788) | `eedb96f1` | ver §5 |

Rama de integración: **`integ/arbx-oportunidades-vivas-01`** · PR **#789**
· SHA del changeset de código: **`bbbe52482eb4453a6b6dca994e434ec6d7d4dbcb`**

Método: `git fetch origin main` → worktree aislado → `cherry-pick -x` de los **11 commits** de
las tres ramas (5 de t1, 5 de t3, 1 de t2), en orden t1 → t3 → t2. **Sin conflictos**: los tres
conjuntos de archivos son disjuntos. Las ramas de origen quedan intactas como evidencia de cada
implementación; el operador decide su cierre.

---

## 3. Diff auditado (`git diff --numstat origin/main HEAD`)

**17 archivos, 0 ajenos. Método obligatorio del contrato: acotado al set declarado; nunca
`git diff --stat` crudo ni `git add -A`.**

```
98      0   backend/api-server/src/routes/opportunities-live.test.ts
138     1   backend/api-server/src/routes/opportunities-live.ts
38      1   backend/math-engine/src/operators/mod.rs
156     0   backend/math-engine/src/operators/real_ops_tests.rs
745    14   backend/searcher-rs/src/amm_math.rs
229     0   backend/searcher-rs/src/cartridge/ARSE-264-01_REACHABILITY.md
319   114   backend/searcher-rs/src/cartridge/runner.rs
27      0   backend/searcher-rs/src/metrics.rs
33      3   backend/searcher-rs/src/size_optimizer.rs
518     3   backend/searcher-rs/src/state_projector.rs
648     9   backend/searcher-rs/src/v3_fee_catalog.rs
284    65   backend/searcher-rs/src/v3_quote_provider.rs
83     39   frontend/components/XRayCard.tsx
82     34   frontend/components/__tests__/XRayCard.test.tsx
128     9   frontend/lib/__tests__/home-opportunity.test.ts
276    23   frontend/lib/home-opportunity.ts
96      5   frontend/lib/schemas.ts
```

Cada línea coincide **exactamente** con la suma de los diffs de las tres ramas de origen:
los 6 de t1 (745/14, 27/0, 33/3, 518/3, 648/9, 284/65), los 4 de t3 (38/1, 156/0, 229/0, 319/114)
y los 7 de t2 (98/0, 138/1, 83/39, 82/34, 128/9, 276/23, 96/5). El cherry-pick no añadió ni perdió
nada. Auditoría de artefactos: **cero** rutas `node_modules/`, `dist/`, `.next`, `target/` o `*.log`
en el diff (`git diff --name-only` filtrado — vacío).

### 3.1 Líneas locales vs `origin/main` (frontera del port)

Las cifras del port se midieron **antes y después**. `Líneas` = `Get-Content | Measure-Object`
(líneas físicas, incluidas vacías); `non-empty` = la forma del comando del contrato
(`git show origin/main:<path> | Measure-Object -Line`), que descarta líneas vacías.

| Archivo | `origin/main` (físicas / non-empty) | Integración (físicas) | Δ físicas |
|---|---|---|---|
| `backend/api-server/src/routes/opportunities-live.ts` | 1425 / **1358** | 1562 | +137 |
| `backend/api-server/src/routes/opportunities-live.test.ts` | 759 / — | 857 | +98 |
| `backend/math-engine/src/operators/mod.rs` | 233 / — | 270 | +37 |
| `backend/math-engine/src/operators/real_ops_tests.rs` | 695 / — | 851 | +156 |
| `backend/searcher-rs/src/amm_math.rs` | 1239 / — | 1970 | +731 |
| `backend/searcher-rs/src/cartridge/ARSE-264-01_REACHABILITY.md` | 1 / — | 229 | +228 |
| `backend/searcher-rs/src/cartridge/runner.rs` | 1097 / — | 1302 | +205 |
| `backend/searcher-rs/src/metrics.rs` | 863 / — | 890 | +27 |
| `backend/searcher-rs/src/size_optimizer.rs` | 8420 / — | 8450 | +30 |
| `backend/searcher-rs/src/state_projector.rs` | 1502 / — | 2017 | +515 |
| `backend/searcher-rs/src/v3_fee_catalog.rs` | 411 / — | 1050 | +639 |
| `backend/searcher-rs/src/v3_quote_provider.rs` | 956 / — | 1175 | +219 |
| `frontend/components/XRayCard.tsx` | 105 / — | 149 | +44 |
| `frontend/components/__tests__/XRayCard.test.tsx` | 79 / — | 127 | +48 |
| `frontend/lib/__tests__/home-opportunity.test.ts` | 41 / — | 160 | +119 |
| `frontend/lib/home-opportunity.ts` | 39 / — | 292 | +253 |
| `frontend/lib/schemas.ts` | 1347 / — | 1438 | +91 |

El anclaje existe para los 17: `git show origin/main:<path>` devuelve contenido en todos
(exit 0). Los 1358 del comando del contrato coinciden con la medición del capitán, lo que
confirma que su cifra usaba la forma `Measure-Object -Line` (no-vacías) y la mía, líneas físicas:
**no son contradictorias, son dos unidades distintas.** Igual para `home-opportunity.ts`:
37 non-empty / 39 físicas en `origin/main` → 292 tras el port (+253), consistente con el
~235–253 declarado.

---

## 4. Puertas de verificación

### 4.1 CI (por SHA, no por rama)

| Workflow | Run | Evento | Resultado |
|---|---|---|---|
| `Rust CI` | [37171449284](https://github.com/hefarica/arbitragex-v2/actions/runs/37171449284) | `pull_request` | **success** |
| `TypeScript CI` | [37171449228](https://github.com/hefarica/arbitragex-v2/actions/runs/37171449228) | `pull_request` | **success** |
| `opportunities-fidelity-gate` | 37171449198 | `pull_request` | success |
| `unit-tests` | 37171449227 | `pull_request` | success |
| `Frontend Build` | 37171449229 | `pull_request` | success |
| `Semiotic-Bridge CI/CD` | 37171449197 | `pull_request` | success |
| `Security Scans` | 37171449212 | `pull_request` | success |
| `integration-tests` | 37171449206 | `pull_request` | **failure** (ver §5 y §6) |

`Rust CI` verde con **todos** los steps en success: `cargo fmt (workspace)` ·
`cargo check --workspace --locked` · `clippy gating -p searcher-rs -p relays-client --all-targets -- -D warnings` ·
`cargo fmt (searcher-rs + relays-client)` · `clippy workspace` · `cargo test --lib (workspace)`.

`cargo test --lib (workspace)`: **2577 passed, 0 failed, 7 ignored** (suma de los 12 `test result: ok`
del log; ninguno con `FAILED`).

Tests load-bearing, uno por uno, del log del run 37171449284:

```
test v3_fee_catalog::tests::canonical_tier_wins_over_legacy_regardless_of_ingest_order ... ok
test state_projector::tests::pool_revert_and_transport_failure_get_distinct_labels ... ok
test amm_math::v3_tests::valid_metadata_with_reverting_quote_is_never_admitted ... ok
test amm_math::v3_tests::tier_mismatch_still_wins_over_an_observed_revert ... ok
test cartridge::runner::tests::map_size_is_cumulative_over_the_whole_tree ... ok
```

### 4.2 Verificación local (independiente) — typecheck y tests DESPUÉS del port

Ejecutada en el worktree de integración con `node_modules` espejado (§9) y en un **control**
idéntico sobre `origin/main` sin changeset, para poder atribuir cada diferencia.

| Workspace | Control (`origin/main` 697dc472) | Integración | Δ |
|---|---|---|---|
| `shared-ts` | 7 archivos / 44 tests | 7 / 44 | 0 |
| `backend/selector-api` | 9 / 84 | 9 / 84 | 0 |
| `backend/api-server` | 73 / 970 | 73 / **975** | **+5** |
| `edge/worker` | 2 / 12 | 2 / 12 | 0 |
| `edge/dev-local` | 4 / 37 | 4 / 37 | 0 |
| `frontend` | 150 / 1595 | 150 / **1610** | **+15** |
| **TOTAL** | **2742** | **2762** | **+20** |

- `npm run typecheck --workspaces --if-present` → **exit 0** en los 6 workspaces (control **y**
  integración). El mismo comando en CI (`tsc --noEmit (all workspaces)`) = success.
- `npm test --workspaces --if-present -- --testTimeout=120000` → **exit 0**, 0 fallos, en los dos.
- **No hay regresión: el changeset AÑADE 20 tests y no rompe ninguno.**
- Nota de honestidad sobre la línea base de t2 (140 archivos / 1426 tests frontend; 70 / 903
  api-server): fue medida sobre la base stale `858b943f`. Sobre `origin/main` (control) las
  cifras ya son 150/1595 y 73/970: la comparación válida es contra el control, no contra el dossier.

---

## 5. ROJO #1 — `TypeScript integration (live Postgres + Redis)`: **introducido por A.8**

Run [37171449206](https://github.com/hefarica/arbitragex-v2/actions/runs/37171449206), job
`TypeScript integration (live Postgres + Redis)`. Salida cruda:

```
FAIL test/opportunities-live.test.ts > GET /api/v1/opportunities/live — token enrichment > token_in_info surfaces UNVERIFIED when no tokens row exists
Error: expected 200 "OK", got 503 "Service Unavailable"     ❯ test/opportunities-live.test.ts:198:8
FAIL ... token_in_info is populated when tokens row exists  ❯ :231:8
FAIL ... expected_profit_usd is null (not 0) when not set   ❯ :257:8
FAIL ... cross-chain row: chain_id_out present, bridge/bridge_fee_usd null ❯ :297:8
FAIL ... viable_only=true excludes rejected rows            (5º fallo)
✓    GET /api/v1/opportunities/live — db_unavailable > returns HTTP 503 with db_unavailable when pool is null
 Test Files  1 failed | 6 passed (7)
      Tests  5 failed | 84 passed (89)
npm error Lifecycle script `test:integration` failed with error code 1
```

### 5.1 Atribución (no es conjetura, es comparación de ramas)

| Rama | `TypeScript integration` |
|---|---|
| `main` @ 697dc472 | **success** |
| #785 (t3, solo Rust) | success |
| #786 (t1, solo Rust) | success |
| #788 (t2/A.8) | **failure** |
| #789 (integración, contiene A.8) | **failure** |

El único cambio de la unión que toca `backend/api-server` es A.8 → el rojo entra por t2, y se
reproduce de forma independiente en su propia rama.

### 5.2 Mecanismo medido

El camino `GET /api/v1/opportunities/live` de A.8 añade a `LIVE_QUERY`:

```sql
LEFT JOIN LATERAL ( ... FROM scored_opportunities s ... )   -- opportunities-live.ts:431-434
```

`scored_opportunities` la crea la migración **`database/migrations/097_scored_opportunities_gate_c.sql`**
(el archivo existe, verificado). El test de integración
`backend/api-server/test/opportunities-live.test.ts` **no aplica migraciones automáticas**: carga
una lista explícita y ordenada, y **097 no está en esa lista** (`grep 097` sobre el archivo = 0
ocurrencias; `grep scored_opportunities` = 0 ocurrencias). Lista real que carga:

```
001_roles.sql · 003_opportunities.sql · 021_defi_registries.sql ·
033_opportunities_fail_honest_and_cross_chain_slots.sql · 034_tokens_table.sql ·
049_h2_net_expected_profit.sql · 072_reconcile_tokens_schema.sql ·
099_opportunities_route_metadata.sql · 102_opportunities_cartridge_id.sql ·
121_opportunities_detector_id_pipeline_latency.sql · 126_opportunities_economics_computation.sql
```

Sin la tabla, la consulta falla y la ruta responde 503 `db_unavailable` — el mismo camino que el
test que sí pasa (`when pool is null`) demuestra que existe. El propio archivo documenta esta
clase de fallo, tres veces, para migraciones anteriores:

> *"099 adds opportunities.route_metadata (JSONB): the live route SELECTs it … without this
> migration the testcontainer schema lacks the column and EVERY GET returns 503 query_failed.
> This was the persistent 'flaky' third member of the CI trio — not flaky at all: deterministic
> breakage latent since the route started selecting the column"* (misma familia citada para 102 y 121)

A.8 es el cuarto miembro de esa familia: añadió una dependencia de esquema y no añadió su
migración al fixture.

### 5.3 Por qué NO invalida el deploy (pero sí impide el "verde")

`scored_opportunities` **existe y está viva en producción**: `SELECT count(*), max(created_at)
FROM scored_opportunities` → **10.945.442 filas, max = 2026-10-04 02:39:50Z** (medido read-only,
2026-10-04). El 503 es del *entorno de test* (contenedor sin migración 097), no del camino de
producción. Aun así el check queda rojo y la afirmación "verificado al 100%" sería falsa.

### 5.4 Fix requerido (una línea, fuera del alcance del integrador → `backend/`)

Añadir `"097_scored_opportunities_gate_c.sql"` a la lista de migraciones de
`backend/api-server/test/opportunities-live.test.ts` (en orden numérico, entre `072` y `099`).
Alternativa doctrinal (R10, más profunda y decisión del operador): degradar el JOIN a
`not_computed` cuando la tabla no existe (patrón `to_regclass` que ya usa
`backend/api-server/src/routes/scoring-status.ts:162`) en vez de convertir la ausencia de un
productor de *scoring* en un 503 de todo el feed de oportunidades.

---

## 6. ROJO #2 — `Rust integration (live Postgres + Redis)`: **pre-existente en `main`**

`integration-tests.yml` sobre **`main`**: `37131476018` @ 697dc472 → failure (también
`37131265316`, `37131245972`, `37130375907`; los runs de 14:32Z eran success → el rojo entra con
un merge posterior). Falla en `searcher-rs/tests/native_operator_adapter_contract_test.rs`:

```
test hardcoded_defaults_are_declared_instead_of_faked_as_sourced ... FAILED
  panicked at searcher-rs/tests/native_operator_adapter_contract_test.rs:300:5:
  assertion `left == right` failed: op_21 computa con sus defaults declarados (op_21_newton.rs:109-121)
test features_present_unlock_the_feature_keyed_operators ... FAILED
  panicked at ...:224:9: op 21: ... "status": "DATA_GAP" ... "defaulted_inputs": ["features.break_even_target"]
test result: FAILED. 7 passed; 2 failed
```

Es **el mismo rojo que el tablero ya tiene ruteado como t13 OP21-CONTRACT-01** ("main lleva rojo
desde 9cf42cbd6"). Ni t1/t3 (Rust) ni t2 (TS) lo empeoran ni lo arreglan: los tres PR de origen y
la integración lo heredan. **No es una regresión de este changeset.**

---

## 7. Trampas de NULL (preservadas)

El contrato exige que el `null-check` del mapper vaya **antes** del clamp, porque
`GREATEST(0, LEAST(1, posterior_prob))*10000` en Postgres daría 10000 bps (100 %) con
`posterior_prob` NULL (LEAST/GREATEST ignoran NULL).

Verificado en el port:

- `GREATEST`/`LEAST` en el TS: **0 ocurrencias** (la trampa no se reproduce por SQL).
- `backend/api-server/src/routes/opportunities-live.ts:838-839`:
  `const posterior = row.scored_posterior_prob;` → `if (posterior == null || !Number.isFinite(posterior)) { return { confidence_score_bps: null, …, confidence_state: "not_computed", confidence_reason: "no_scored_row_for_opportunity" } }`
  **y solo después** `const clamped = Math.min(1, Math.max(0, posterior)); const bps = Math.round(clamped * 10_000);`
- Frontend (`frontend/lib/home-opportunity.ts:102-116`, `XRayCard.tsx:9-24,60`): tres hechos
  distintos — no computado + reason / **cero exacto** ("0% conf") / valor real — con el
  `posterior_prob` sin redondear viajando en el wire para distinguir "exactamente 0" de
  "redondeado a 0". Nada de esto se perdió en el port.

## 8. El wire de A.8 es NUEVO en `origin/main` (port aditivo, sin merge de historia)

| Métrica en `backend/api-server/src/routes/opportunities-live.ts` | `origin/main` | Integración |
|---|---|---|
| `scored_opportunities` | **0** | 7 |
| `LEFT JOIN LATERAL` | **0** | 2 |
| `confidence_score_bps` | **0** | 7 |
| `a8ScoringFromRow` | **0** | 4 |
| `LIVE_QUERY` (anclajes preexistentes) | 5 | **5** |
| `route_metadata` (anclajes preexistentes) | 17 | **17** |

Los anclajes no cambian: el port añade, no reescribe historia.

## 9. `node_modules` y mutaciones del árbol compartido

- El changeset **no contiene** `node_modules/` ni artefactos de build (§3); `node_modules/`,
  `dist/`, `.next` están en `.gitignore`.
- **No se usó `npm ci`** ni se escribió en el árbol compartido: la verificación local corrió en un
  worktree temporal con `node_modules` **espejado por junctions** desde el árbol principal
  (junctions creadas desde el árbol principal, nunca hacia él; los enlaces `@arbx/*` se
  re-apuntaron a los workspaces del worktree, que es lo que evita resolver el código stale).
  Ese espejo se elimina con el worktree.
- **Dependencias completadas por t2** (documentadas por exigencia del contrato, R11):
  `frontend/node_modules/@rainbow-me/rainbowkit` = **2.2.11** y
  `frontend/node_modules/@gemini-wallet/core` = **0.3.2** (transitiva declarada por wagmi).
  **Ambas coinciden exactamente con `package-lock.json`** (`frontend/node_modules/@rainbow-me/rainbowkit`
  → 2.2.11, `resolved` del registry; `frontend/node_modules/@gemini-wallet/core` → 0.3.2; wagmi
  declara `"@gemini-wallet/core": "0.3.2"`). Consecuencia: `npm ci` en CI resuelve las mismas
  versiones y los conteos locales son comparables a los de CI. Riesgo remanente: son mutaciones
  de un árbol compartido por varias sesiones — no entran en ningún commit (gitignored) y no
  afectan al VPS, que instala desde el lockfile.

## 10. Procedimiento de activación (acto del OPERADOR — NO ejecutado por el integrador)

Ruta y servicios según el dossier y `docker/compose.dev.yml`. Servicios afectados por este
changeset: **`searcher-rs`** (t1 + t3: también arrastra `math-engine`), **`api-server`** (t2) y
**`frontend`** (t2). `selector-api` y `edge` **no** se tocan.

```bash
# 1) en el VPS (read-only para el integrador; lo ejecuta el operador)
ssh arbx
cd /opt/arbitragex-v2
git fetch origin && git checkout bbbe5248    # SHA del changeset; o el merge a main si el operador lo integra

# 2) build SIN caché y con env explícito (RULE 03/04 — sin --env-file se hornean localhost)
docker compose --env-file .env -f docker/compose.dev.yml build --no-cache searcher-rs api-server frontend
docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs api-server frontend
```

Verificación de arranque (§ skill `arbx-vps-verification-runbook`): `docker ps` con `Up …` (no
`Restarting`), ausencia de pánicos en `docker logs <servicio> --since 10m`, y `curl` interno en
`localhost` comparado con el endpoint público del edge (un 502 del proxy no debe pasar por
"deploy OK").

## 11. Verificación POST-DEPLOY — lo que decide si la activación sirvió

El criterio de éxito **no es "compila"**: es que el histograma de `rejection_reason` cambie de
forma medible respecto de una línea base medida, y que el wire de A.8 emita confianza.

**Línea base medida (read-only, 2026-10-04 ≈02:40Z, ventana 1 h, 69.917 filas):**

| `rejection_reason` | Filas/h | Cuota |
|---|---|---|
| `v3_quote_unavailable` | 27.965 | 40 % |
| `spread_zero_equilibrium` | 27.335 | 39 % |
| `non_positive_profit` | 10.512 | 15 % |
| `v3_multileg_budget_exhausted` | 1.801 | 2,6 % |
| `single_pool_no_spread` | 1.498 | 2,1 % |
| `no_tradable_size` | 900 | 1,3 % |
| **`status='emitted'`** | **0** | **0 %** |

```sql
-- A) histograma (la métrica del contrato)
SELECT rejection_reason, count(*)
FROM opportunities
WHERE detected_at >= now() - interval '1 hour'
GROUP BY 1 ORDER BY 2 DESC;

-- B) el resultado que importa: ¿alguna oportunidad emitida?
SELECT count(*) FILTER (WHERE status='emitted') AS emitted, count(*) AS total
FROM opportunities WHERE detected_at >= now() - interval '1 hour';

-- C) el wire de A.8 (debe traer claves de confianza, no su ausencia)
--    curl -s localhost:8080/api/v1/opportunities/live | jq '.[0] | {confidence_score_bps, confidence_state, confidence_source}'
```

**Qué puede mover este changeset y qué no** (para no leer un no-cambio como fracaso de otra cosa):

- **Sí**: el canal `v3_quote_unavailable` (tier canónico vs legacy, admisión por cotización real,
  `QuoteReverted` con evidencia) y los rechazos por presupuesto/catálogo.
- **No**: `spread_zero_equilibrium` (39 % del total). Ninguna de las tres implementaciones lo
  toca — su diagnóstico es t15 SPREAD-ZERO-01. **Si ese bucket no cambia, NO es un fallo de esta
  activación.**
- **A.8**: `scored_opportunities` ya tiene 10.945.442 filas con `max(created_at)` a segundos del
  presente, así que el join debe encontrar fila; si el payload sigue sin las claves de confianza,
  el fallo es del wire, no de la fuente.

Interpretación honesta (R8/R10): un `emitted=0` después de la ventana de observación **no se
explica como "mercado"** sin antes mostrar que los buckets objetivo se movieron. Si el histograma
queda idéntico al de §11, la activación no cambió nada medible y eso se reporta como tal.

## 12. Rollback

SHA desplegado antes de esta activación: **`6d38a7c6fce7ab3c7511e7a027adbdd85829d33b`**
(`main`, 2026-10-03 04:15 -05). Reversión = `git checkout 6d38a7c6` en el VPS + rebuild sin caché
de los mismos tres servicios. No hay migración de base de datos en este changeset, así que el
rollback es solo de imagen (y las tablas que A.8 *lee* ya existían: 097 en producción).

## 13. Qué queda FUERA de este changeset, y por qué

| Fuera | Motivo | Estado |
|---|---|---|
| t12 V4-SNAPSHOT-PRODUCERS-01 (#787) | trabajo en curso, no es una de las tres implementaciones | in_progress |
| t13 OP21-CONTRACT-01 | es la causa del rojo **pre-existente** de `main` (§6) | pending |
| t15 SPREAD-ZERO-01 | canal del 39 % que nadie tocó; diagnóstico en curso | in_progress |
| t16 CATALOG-CANONICAL-CONFLICT-01 | conflicto canón-canónico remanente (`CANON_CANON_CONFLICTS=1`) | pending |
| Fix de §5.4 (migración 097 en el fixture) | vive en `backend/api-server/test/` → fuera del alcance del integrador | **bloqueador abierto** |

**Residuales del árbol compartido** (declarados, no silenciados): el checkout principal está en
`fix/perhop-reserves-01` @ `858b943f` con 153 commits de atraso y trabajo de otras sesiones
(7 modificaciones rastreadas de `backend/searcher-rs/src` + 1 archivo sin seguimiento). **Dos de
esos artefactos NO forman parte de este changeset y por tanto no pasaron por ningún gate**:
`backend/searcher-rs/src/snapshot_services.rs` (+179/−0) y
`backend/searcher-rs/tests/cartridge_r_closed_cycle_test.rs` (sin seguimiento). No contienen
ninguna de las pruebas load-bearing. Su pertenencia a t1/t6/t7/t9 **no está establecida** por
ninguna rama: si alguno debía viajar, este paquete no lo cubre.

## 14. Limitaciones del entorno de verificación (fail-honest)

- `cargo` **no puede** correr en este host (AppControl 4551 / proc-macro E0463): el veredicto de
  compilación de los ~2.500 líneas Rust es el de CI (run 37171449284), no local.
- La verificación local de TS corrió con **Node v24.12.0**; CI usa **node 20**. Los conteos
  locales y los de CI coinciden en "0 fallos"; no se comparó test-por-test.
- El `node_modules` local es un **espejo por junctions** del árbol compartido (no `npm ci`); las
  dos dependencias completadas por t2 coinciden con el lockfile (§9), que es lo que hace
  comparables los conteos.
- Las cifras de 1 hora de PG son **ventana móvil**: cada medición lleva su timestamp; ninguna se
  presenta como constante.
