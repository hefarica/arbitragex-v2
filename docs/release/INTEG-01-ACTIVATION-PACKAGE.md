# INTEG-01 — Paquete de activación (NO ejecutado)

> **Estado**: listo para revisión del OPERADOR. **Este documento no despliega nada.**
> La activación (deploy a `/opt/arbitragex-v2`) es acto del operador (§34.5). Aquí solo está
> el changeset ensamblado, auditado y verificado, y la verificación post-deploy que decide si
> la activación sirvió.
>
> Fecha: 2026-10-04 · Autor: `integrador` (INTEG-01 / t5, attempt 2)
> Protocolo de entrada aplicado: §32/§33 audit/scaffold/shadow/READ-ONLY. Capital expuesto = 0.

---

## 1. Veredicto del integrador

**El changeset se ensambla, compila, typechequea, pasa sus tests unitarios y pasa la integración
TypeScript contra Postgres+Redis vivos. Queda UN check rojo, y es heredado de `main`.**

| # | Check | Estado (SHA `e87eb410`) | ¿Causado por este changeset? |
|---|-------|--------------------------|------------------------------|
| 1 | `Rust CI` — cargo fmt/check/clippy/test | **VERDE** — run [37174029248](https://github.com/hefarica/arbitragex-v2/actions/runs/37174029248) | — |
| 2 | `TypeScript CI` — tsc all workspaces + unit tests | **VERDE** — run [37174029218](https://github.com/hefarica/arbitragex-v2/actions/runs/37174029218) | — |
| 3 | `TypeScript integration (live Postgres + Redis)` | **VERDE** — run [37174029245](https://github.com/hefarica/arbitragex-v2/actions/runs/37174029245) | **F1 CERRADO** (era mi bloqueador; ver §5) |
| 4 | `opportunities-fidelity-gate`, `unit-tests`, `Frontend Build`, `Semiotic-Bridge`, `Security Scans`, `Dockerfile Audit`, `V3 Fee Boundary Regression`, `Deployment regression gates`, `no-hardcode`, gates OMEGA, `ethics-guard` | **VERDE** | — |
| 5 | `Rust integration (live Postgres + Redis)` | **ROJO** | **NO — pre-existente en `main` 697dc472** (2 asserts de op_21, t13). Ver §6. |

Consecuencia operativa: el único rojo **ya está rojo en `main`** y su reparación existe y está
verde en un PR independiente (**#790**, `op21-contract-01` @ `b0ef4b41`: `integration-tests` run
37171636474 = SUCCESS). Este paquete **no lo incluye** (no es una de las tres implementaciones, §13);
si el operador quiere el árbol sin ese rojo, debe mergear #790 junto con —o después de— este changeset.

---

## 2. Qué se activa

Base: `origin/main` @ **697dc472** (`git merge-base origin/main HEAD` = 697dc472;
`git rev-list --count HEAD..origin/main` = 0).

| ID | Implementación | Rama origen (PR) | Head de origen |
|----|----------------|------------------|----------------|
| t1 | V3-QUOTE-01 + CATALOG-HYGIENE-01 + CATALOG-ADMISSION-WIRE-01 + QUOTE-TRUTH-ADMISSION-01 · y el cierre de la F1 de t4: **CATALOG-CANONICAL-CONFLICT-01** (t16) | `ci/cargo-gate-v3quote-01` (#786) | **72aabffa** |
| t3 | ARSE-264-01 (alcanzabilidad del lote 264, coherencia 31 vs 32) | `arse-264-01` (#785) | f3557633 |
| t2 | A8-CONF-01 (wire de confidence scoring, gate A.8) + `el sink ausente degrada el campo, no tira el feed` (defecto hallado por el gate de CI) | `ci/a8-conf-gate-01` (#788) | **70706c0b** |

Rama de integración: **`integ/arbx-oportunidades-vivas-01`** · PR **#789**
· SHA del changeset de código: **`e87eb410691f553fcce02900aca8fc8739ff231b`**

Método: `git fetch origin main` → worktree aislado → `cherry-pick -x` de **14 commits** en orden
t1 (7) → t3 (5) → t2 (2). **Sin conflictos**: los tres conjuntos de archivos son disjuntos.
Las ramas de origen quedan intactas como evidencia; el operador decide su cierre.

---

## 3. Diff auditado (`git diff --numstat origin/main HEAD`)

**18 archivos = 17 de código + este documento. Cero ajenos.** Auditoría acotada al set declarado
(nunca `git diff --stat` crudo, nunca `git add -A`).

```
169    10   backend/api-server/src/routes/opportunities-live.test.ts
222     3   backend/api-server/src/routes/opportunities-live.ts
 38     1   backend/math-engine/src/operators/mod.rs
156     0   backend/math-engine/src/operators/real_ops_tests.rs
745    14   backend/searcher-rs/src/amm_math.rs
229     0   backend/searcher-rs/src/cartridge/ARSE-264-01_REACHABILITY.md
319   114   backend/searcher-rs/src/cartridge/runner.rs
 27     0   backend/searcher-rs/src/metrics.rs
 33     3   backend/searcher-rs/src/size_optimizer.rs
518     3   backend/searcher-rs/src/state_projector.rs
964    16   backend/searcher-rs/src/v3_fee_catalog.rs
284    65   backend/searcher-rs/src/v3_quote_provider.rs
435     0   docs/release/INTEG-01-ACTIVATION-PACKAGE.md
 83    39   frontend/components/XRayCard.tsx
 82    34   frontend/components/__tests__/XRayCard.test.tsx
128     9   frontend/lib/__tests__/home-opportunity.test.ts
276    23   frontend/lib/home-opportunity.ts
 96     5   frontend/lib/schemas.ts
```

Verificación de completitud: los números son **idénticos** a la unión de los tres diffs de origen
medidos por separado contra `origin/main` — #786: 745/14, 27/0, 33/3, 518/3, **964/16**, 284/65;
#785: 38/1, 156/0, 229/0, 319/114; #788: **169/10, 222/3**, 83/39, 82/34, 128/9, 276/23, 96/5.
El cherry-pick no añadió ni perdió nada. Cero rutas `node_modules/`, `dist/`, `.next`, `target/`
o `*.log` en el diff.

### 3.1 Líneas locales vs `origin/main` (frontera del port)

`físicas` = `Get-Content | Measure-Object`; `non-empty` = la forma del comando del contrato
(`git show origin/main:<path> | Measure-Object -Line`), que descarta líneas vacías.

| Archivo | `origin/main` | Integración | Δ |
|---|---|---|---|
| `backend/api-server/src/routes/opportunities-live.ts` | 1425 (**1358** non-empty) | 1644 | +219 |
| `backend/api-server/src/routes/opportunities-live.test.ts` | 759 | 918 | +159 |
| `backend/math-engine/src/operators/mod.rs` | 233 | 270 | +37 |
| `backend/math-engine/src/operators/real_ops_tests.rs` | 695 | 851 | +156 |
| `backend/searcher-rs/src/amm_math.rs` | 1239 | 1970 | +731 |
| `backend/searcher-rs/src/cartridge/ARSE-264-01_REACHABILITY.md` | 1 | 229 | +228 |
| `backend/searcher-rs/src/cartridge/runner.rs` | 1097 | 1302 | +205 |
| `backend/searcher-rs/src/metrics.rs` | 863 | 890 | +27 |
| `backend/searcher-rs/src/size_optimizer.rs` | 8420 | 8450 | +30 |
| `backend/searcher-rs/src/state_projector.rs` | 1502 | 2017 | +515 |
| `backend/searcher-rs/src/v3_fee_catalog.rs` | 411 | 1359 | +948 |
| `backend/searcher-rs/src/v3_quote_provider.rs` | 956 | 1175 | +219 |
| `frontend/components/XRayCard.tsx` | 105 | 149 | +44 |
| `frontend/components/__tests__/XRayCard.test.tsx` | 79 | 127 | +48 |
| `frontend/lib/__tests__/home-opportunity.test.ts` | 41 | 160 | +119 |
| `frontend/lib/home-opportunity.ts` | 39 | 292 | +253 |
| `frontend/lib/schemas.ts` | 1347 | 1438 | +91 |

El anclaje existe para los 17: `git show origin/main:<path>` devuelve contenido (exit 0) en todos.
La cifra **1358** reproduce exactamente la medición del capitán con su misma forma; su "1187
local" era el árbol STALE con otra unidad (líneas físicas) — **no se contradicen, son dos
unidades sobre dos árboles distintos**. `home-opportunity.ts`: 39 físicas / 37 non-empty en
`origin/main` → 292 tras el port.

---

## 4. Puertas de verificación

### 4.1 CI sobre el SHA `e87eb410` (por SHA, no por rama)

| Workflow | Run | Resultado |
|---|---|---|
| `Rust CI` | [37174029248](https://github.com/hefarica/arbitragex-v2/actions/runs/37174029248) | **success** (15 steps) |
| `TypeScript CI` | [37174029218](https://github.com/hefarica/arbitragex-v2/actions/runs/37174029218) | **success** |
| `integration-tests` | [37174029245](https://github.com/hefarica/arbitragex-v2/actions/runs/37174029245) | `TypeScript integration` **success** · `Rust integration` **failure** (§6) |

`Rust CI` verde con **todos** los steps en success: `cargo fmt (workspace)` ·
`cargo check --workspace --locked` · `clippy gating -p searcher-rs -p relays-client --all-targets -- -D warnings` ·
`cargo fmt (searcher-rs + relays-client)` · `clippy workspace` · `cargo test --lib (workspace)`.

`cargo test --lib (workspace)`: **2580 passed, 0 failed, 7 ignored** (suma de los 12
`test result: ok` del log; cero `FAILED`). Tests load-bearing, del log del run:

```
test v3_fee_catalog::tests::canonical_tier_wins_over_legacy_regardless_of_ingest_order ... ok
test state_projector::tests::pool_revert_and_transport_failure_get_distinct_labels ... ok
test amm_math::v3_tests::valid_metadata_with_reverting_quote_is_never_admitted ... ok
test amm_math::v3_tests::tier_mismatch_still_wins_over_an_observed_revert ... ok
test cartridge::runner::tests::map_size_is_cumulative_over_the_whole_tree ... ok
test v3_fee_catalog::tests::canonical_canonical_conflict_is_resolved_by_the_legacy_witness_in_both_orders ... ok
test v3_fee_catalog::tests::canonical_canonical_conflict_without_witness_is_still_order_invariant ... ok
```

### 4.2 Verificación local (independiente) — typecheck y tests DESPUÉS del port

Ejecutada en el worktree de integración con `node_modules` espejado (§9); el CONTROL es un
worktree idéntico sobre `origin/main` sin changeset.

| Workspace | Control (`origin/main` 697dc472) | Integración | Δ |
|---|---|---|---|
| `shared-ts` | 7 archivos / 44 tests | 7 / 44 | 0 |
| `backend/selector-api` | 9 / 84 | 9 / 84 | 0 |
| `backend/api-server` | 73 / 970 | 73 / **977** | **+7** |
| `edge/worker` | 2 / 12 | 2 / 12 | 0 |
| `edge/dev-local` | 4 / 37 | 4 / 37 | 0 |
| `frontend` | 150 / 1595 | 150 / **1610** | **+15** |
| **TOTAL** | **2742** | **2764** | **+22** |

- `npm run typecheck --workspaces --if-present` → **exit 0** en los 6 workspaces (control e integración).
- `npm test --workspaces --if-present -- --testTimeout=120000` → **exit 0**, 0 fallos, en los dos.
- **No hay regresión: el changeset AÑADE 22 tests y no rompe ninguno.**
- Nota de honestidad: la línea base que cita el contrato (140 archivos / 1426 tests frontend;
  70 / 903 api-server) fue medida por t2 sobre la base stale `858b943f`. Sobre `origin/main` el
  control ya da 150/1595 y 73/970: la comparación válida es contra el control.

---

## 5. F1 CERRADO — `TypeScript integration` era rojo por A.8 y ya no lo es

**Estado previo (attempt 1 de esta tarea, SHA `bbbe5248`)**: 5 de 6 tests de
`backend/api-server/test/opportunities-live.test.ts` devolvían `expected 200 "OK", got 503
"Service Unavailable"` (exit 1). Diagnóstico: A.8 nombraba `scored_opportunities` en `LIVE_QUERY`
(migración 097) y el testcontainer carga una lista explícita de 11 migraciones **sin la 097**;
en un Postgres sin esa tabla, nombrar la relación hace fallar toda la ventana live y el endpoint
responde 503. Cuarta aparición de la familia que el propio fixture documenta para 099/102/121/126.

**Resolución (por el dueño de A.8, no por el integrador)**: commit
**`70706c0b`** — *"el sink de A.8 ausente degrada el campo, no tira el feed entero (CI-GATE-01)"*:
se sondea el catálogo con `to_regclass` (cacheado por mount) y, si el sink no existe, la query
**omite el join**, las columnas degradan a NULL literales y `a8ScoringFromRow()` declara
`NOT COMPUTED` con reason **`scored_opportunities_sink_absent`** (R8/R10). Es la alternativa
doctrinal que este paquete recomendaba en su §5.4, y **no confunde** las dos ausencias:
`sink_absent` (el productor no existe en esta base) ≠ `no_scored_row_for_opportunity` (existe y
aún no tiene fila). Añade 2 tests.

**Medición del cierre, sobre el SHA integrado `e87eb410`**:

- `integration-tests` run 37174029245 → `TypeScript integration (live Postgres + Redis)` = **SUCCESS**.
- Independiente, en la rama de origen: PR #788 @ `70706c0b` → mismo check = **SUCCESS**.
- Local: api-server pasó de 970 (control) a **977** tests, 0 fallos.

---

## 6. ROJO #5 — `Rust integration (live Postgres + Redis)`: pre-existente en `main`

Sobre el SHA integrado, el log del run 37174029245 muestra **exactamente** las dos aserciones de
op_21, idénticas a las de `main`:

```
thread 'hardcoded_defaults_are_declared_instead_of_faked_as_sourced' panicked at
  searcher-rs/tests/native_operator_adapter_contract_test.rs:300:5:
  assertion `left == right` failed: op_21 computa con sus defaults declarados (op_21_newton.rs:109-121)
thread 'features_present_unlock_the_feature_keyed_operators' panicked at ...:224:9:
  op 21: ... "status": "DATA_GAP" ... "defaulted_inputs": ["features.break_even_target"]
test result: FAILED. 7 passed; 2 failed
```

`integration-tests` sobre **`main`** ya estaba rojo por esto (runs 37130375907, 37131245972,
37131265316, 37131476018 @697dc472). Ni t1/t3 (Rust) ni t2 (TS) lo empeoran ni lo arreglan:
**no es una regresión de este changeset**. Su reparación es el PR **#790** (`op21-contract-01` @
`b0ef4b41`, t13, `integration-tests` 37171636474 = SUCCESS), **fuera** de este paquete (§13).

---

## 7. Trampas de NULL (preservadas) y la nueva degradación honesta

El contrato exige que el `null-check` del mapper vaya **antes** del clamp, porque
`GREATEST(0, LEAST(1, posterior_prob))*10000` en Postgres daría 10000 bps (100 %) con
`posterior_prob` NULL (LEAST/GREATEST ignoran NULL).

- `GREATEST`/`LEAST` en el TS: **0 ocurrencias**.
- `backend/api-server/src/routes/opportunities-live.ts`: `const posterior = row.scored_posterior_prob;`
  → `if (posterior == null || !Number.isFinite(posterior)) { return { confidence_score_bps: null, …,
  confidence_state: "not_computed", confidence_reason: "no_scored_row_for_opportunity" } }`
  **y solo después** `const clamped = Math.min(1, Math.max(0, posterior)); const bps = Math.round(clamped * 10_000);`
- Tercera ausencia, ahora explícita: **sink ausente** ⇒ `confidence_reason =
  "scored_opportunities_sink_absent"` (§5). Tres estados distintos, cero `—` mudo y cero 0 disfrazado.
- Frontend (`frontend/lib/home-opportunity.ts`, `XRayCard.tsx`): tres hechos separados — no
  computado + reason / **cero exacto** ("0% conf") / valor real — con `posterior_prob` sin redondear
  viajando en el wire para distinguir "exactamente 0" de "redondeado a 0". Nada de esto se perdió.

## 8. El wire de A.8 sigue siendo NUEVO en `origin/main` (port aditivo)

| Métrica en `backend/api-server/src/routes/opportunities-live.ts` | `origin/main` | Integración |
|---|---|---|
| `scored_opportunities` | **0** | 8 |
| `LEFT JOIN LATERAL` | **0** | 2 |
| `confidence_score_bps` | **0** | 7 |
| `a8ScoringFromRow` | **0** | 4 |
| `LIVE_QUERY` (anclaje preexistente) | 5 | **5** |
| `route_metadata` (anclaje preexistente) | 17 | **17** |

Los anclajes no cambian: el port añade, no reescribe historia.

## 9. `node_modules` y mutaciones del árbol compartido

- El changeset **no contiene** `node_modules/` ni artefactos de build (§3); `node_modules/`,
  `dist/`, `.next` están en `.gitignore`.
- **No se usó `npm ci`** ni se escribió en el árbol compartido: la verificación local corrió en un
  worktree temporal con `node_modules` **espejado por junctions** creadas DESDE el árbol principal
  (nunca hacia él), con los enlaces `@arbx/*` re-apuntados a los workspaces del worktree para no
  resolver código stale. Al terminar se desenlazaron y se verificó que el árbol compartido quedó
  intacto (`node_modules` 658 entradas antes y después; `@arbx/shared` → árbol principal; `react`
  y `rainbowkit` presentes).
- **Dependencias completadas por t2** (documentadas por exigencia del contrato, R11):
  `frontend/node_modules/@rainbow-me/rainbowkit` = **2.2.11** y
  `frontend/node_modules/@gemini-wallet/core` = **0.3.2** (transitiva declarada por wagmi).
  **Ambas coinciden exactamente con `package-lock.json`** (`frontend/node_modules/@rainbow-me/rainbowkit`
  → 2.2.11, `resolved` del registry; `frontend/node_modules/@gemini-wallet/core` → 0.3.2; wagmi
  declara `"@gemini-wallet/core": "0.3.2"`). `npm ci` en CI resuelve las mismas versiones, así que
  los conteos locales son comparables a los de CI. Riesgo remanente: son mutaciones de un árbol
  compartido — no entran en ningún commit (gitignored) y no afectan al VPS, que instala desde el
  lockfile.

## 10. Procedimiento de activación (acto del OPERADOR — NO ejecutado por el integrador)

Ruta y servicios según el dossier y `docker/compose.dev.yml`. Servicios afectados por este
changeset: **`searcher-rs`** (t1 + t3; arrastra `math-engine`), **`api-server`** (t2) y
**`frontend`** (t2). `selector-api` y `edge` **no** se tocan.

```bash
# 1) en el VPS (el integrador no escribe en el VPS)
ssh arbx
cd /opt/arbitragex-v2
git fetch origin && git checkout e87eb410    # SHA del changeset; o el merge a main si el operador lo integra

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

**Línea base medida (read-only, 2026-10-04T03:26Z, ventana 1 h, 69.302 filas):**

| `rejection_reason` | Filas/h | Cuota |
|---|---|---|
| `v3_quote_unavailable` | 29.409 | 42 % |
| `spread_zero_equilibrium` | 26.319 | 38 % |
| `non_positive_profit` | 8.954 | 13 % |
| `single_pool_no_spread` | 2.484 | 3,6 % |
| `v3_multileg_budget_exhausted` | 1.009 | 1,5 % |
| `no_tradable_size` | 901 | 1,3 % |
| `negative_net_profit` | 250 | 0,4 % |
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

-- C) el wire de A.8 (debe traer claves de confianza y su estado honesto)
--    curl -s localhost:8080/api/v1/opportunities/live | jq '.[0] | {confidence_score_bps, confidence_state, confidence_source, confidence_reason}'
```

**Qué puede mover este changeset y qué no** (para no leer un no-cambio como fracaso de otra cosa):

- **Sí**: `v3_quote_unavailable` (tier canónico vs legacy, admisión por cotización real,
  `QuoteReverted` con evidencia) y los rechazos por catálogo/conflicto canónico-canónico (t16).
- **No**: `spread_zero_equilibrium` (38 % del total). **Tampoco lo toca este changeset**: es de
  t15/t18 (diagnóstico SPREAD-ZERO-01 y su reparación, PR #792, fuera de este paquete). Si ese
  bucket no cambia, NO es un fallo de esta activación.
- **A.8**: `scored_opportunities` está viva (10,9 M de filas, `max(created_at)` a segundos del
  presente), así que el join debe encontrar fila; si el payload no trae las claves de confianza,
  el fallo es del wire, no de la fuente.

Interpretación honesta (R8/R10): un `emitted = 0` después de la ventana de observación **no se
explica como "mercado"** sin antes mostrar que los buckets objetivo se movieron. Si el histograma
queda idéntico al de §11, la activación no cambió nada medible y eso se reporta como tal.

## 12. Rollback

SHA desplegado antes de esta activación: **`6d38a7c6fce7ab3c7511e7a027adbdd85829d33b`** (`main`,
2026-10-03 04:15 -05). Reversión = `git checkout 6d38a7c6` en el VPS + rebuild sin caché de los
mismos tres servicios. **Este changeset no incluye ninguna migración de base de datos**, así que
el rollback es solo de imagen (las tablas que A.8 *lee* ya existían en producción: 097).

## 13. Qué queda FUERA de este changeset, y por qué

| Fuera | Motivo | Estado |
|---|---|---|
| **#790** `op21-contract-01` (t13) | es la causa del **rojo heredado** de `main` (§6); no es una de las tres implementaciones | OPEN, verde |
| #787 `v4-snapshot-producers-01` (t12) | changeset distinto (cartuchos) | OPEN, verde |
| #791 `price-coverage-01` (t17) | changeset distinto (price_worker) | OPEN, verde |
| #792 `spread-signed-delta-01` (t18) | changeset distinto (canal del 38 %) | OPEN, verde |
| t19 COST-FEATURES-WIRE-01, t21 ECON-AMOUNT-DENOM-01 | trabajo en curso | in_progress |

**Residuales del árbol compartido** (declarados, no silenciados): el checkout principal sigue en
`fix/perhop-reserves-01` @ `858b943f` con 153 commits de atraso y trabajo de otras sesiones. Dos
artefactos NO forman parte de este changeset y por tanto **no pasaron por ningún gate**:
`backend/searcher-rs/src/snapshot_services.rs` (+179/−0) y
`backend/searcher-rs/tests/cartridge_r_closed_cycle_test.rs` (sin seguimiento). Ninguno contiene
las pruebas load-bearing; su pertenencia a t1/t6/t7/t9 **no está establecida** por ninguna rama.

## 14. Limitaciones del entorno de verificación (fail-honest)

- `cargo` **no puede** correr en este host (AppControl 4551 / proc-macro E0463): el veredicto de
  compilación del Rust es el de CI (run 37174029248), no local.
- La verificación local de TS corrió con **Node v24.12.0**; CI usa **node 20**. Los conteos
  coinciden en "0 fallos"; no se comparó test-por-test.
- El `node_modules` local es un **espejo por junctions** del árbol compartido (no `npm ci`); las
  dos dependencias completadas por t2 coinciden con el lockfile (§9), que es lo que hace
  comparables los conteos.
- Las cifras de 1 hora de PG son **ventana móvil**: cada medición lleva su timestamp; ninguna se
  presenta como constante.
- **Historial de este paquete**: la versión del attempt 1 (SHA `bbbe5248`) declaraba dos rojos —
  el de A.8 (F1) y el heredado de `main`. El de A.8 quedó **cerrado por medición** en `e87eb410`
  (§5); el heredado sigue y su reparación vive en el PR #790, fuera de alcance.
