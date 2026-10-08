# W13-N11-INDEPENDIENTE-01 — Revisión independiente de #884: el gate que deja de gastar fork en los rechazos

**Orden:** t180 (`W13-N11-INDEPENDIENTE-01`, kind: **review**, ronda 1) · **Intento:** `e444ad6e-2c4a-470d-ae0f-5930ca1c9d1a`
**Objeto:** PR **#884**, rama `fix/simctl-verdict-01`, **READY FOR REVIEW** (`draft:false`, `merged:false`, `mergeable:true`)
**Head verificado por el REMOTO:** `gh api repos/hefarica/arbitragex-v2/pulls/884` → `7e059208206e9ef54cdc2aa88e6eb6bd94aa3224`; base `77b42b3dccc001455fda3e8d4437d973c9e98c4b`; `changed_files: 5`; `commits: 1`; `+989/−8`.
**Motor:** Rust **1.91.1** (`cargo`/`rustc`) dentro de la imagen `rust:1.91` del VPS, sobre una COPIA del árbol del head en `/tmp/t180r`. `forge` no se usa: este PR no toca Solidity.
**Modo:** SOLO LECTURA sobre `/opt` y el fork. Ningún cambio de código en el repo; los mutantes viven solo en la copia de `/tmp`. CERO mainnet, firmas, broadcast o escritura en el fork. Paper.

---

## 0. VEREDICTO, por criterio (declarado ANTES de leer el código)

| # | Criterio | Veredicto |
|---|---|---|
| **A** | Diseño del gate: ¿admite lo que debe y deniega lo que debe? | **PASS** |
| **B** | Comportamiento en runtime (E2E del consumer) | **NO COMPUTADO** — declarado con precisión (§3). No se hereda como probado |
| **C** | Orden: la frontera corre ANTES de adquirir el fork | **PASS** (invariante **de colocación**, estructural) |
| **D** | No se filtra ni se rompe nada más (`contracts.rs`, `CANDIDATE_SLOTS`, gate de paper, umbrales) | **PASS** |
| **E** | ¿Puede DENEGAR DE MÁS? (riesgo simétrico, no cubierto por F1) | **PASS con residual declarado** |
| **F** | No reintroducir la liberación de fork como cifra | **CUMPLIDO** — no se cita ninguna cifra de fork liberado |
| **G** | Estado del instrumento nuevo | **NO COMPUTADO** (sin productor en el binario desplegado) |
| **H** | Falsificador F1/F2 re-ejecutado (no citado) | **PASS** — exit 101 en ambos, restauración exacta |
| **I** | Gates del head (merge-base, tree, blobs, suites) | **PASS** — y **una condición de aterrizaje nueva** (§8) |

**Veredicto de la revisión: `pass`** — el cambio es correcto y aterrizable **por su contenido**, con dos condiciones declaradas: (i) el comportamiento en runtime queda **NO VERIFICADO** (§3) y (ii) antes de aterrizar hay que **re-basar y re-medir** porque `main` avanzó 7 commits (§8).

---

## 1. Cadena de custodia: los bytes que revisé son los del head

Los 5 archivos del PR, verificados **por blob** contra la API, y los mismos blobs tras transportar el árbol a `/tmp/t180r`:

| archivo | blob del PR | blob medido por mí | ✔ |
|---|---|---|---|
| `backend/sim-ctl/src/decision_frontier.rs` (added +739) | `1385186a…` | `1385186ac061f991d90b506c0d5b458d0d8a46de` | ✔ |
| `backend/sim-ctl/src/consumer.rs` (+165/−8) | `d06287cb…` | `d06287cb78b14f98d6be6d583d97cbcc23654ea8` | ✔ |
| `backend/shared-rs/src/metrics.rs` (+38) | `bcc3bd99…` | `bcc3bd99159e8fa39f21cdeedeaf97c2ee8bdb20` | ✔ |
| `backend/sim-ctl/src/lib.rs` (+1) | `b8b5f7e2…` | `b8b5f7e23f04b34ccf35bce6774fb1bba35d6555` | ✔ |
| `backend/sim-ctl/src/persistence.rs` (+46) | `c49c4fb7…` | `c49c4fb79b036e01751b860bfa18bfa2c711bd94` | ✔ |

Transporte: `git archive` del head → `head.tar.gz` (3 496 686 B, sha256 `d73c82e78faf0fd40ef9a1cc83cba636a5e48c9ed578d8c9873d8c37f051b8c9`, 1 300 archivos) → `scp` → extracción en `/tmp/t180r/head`. **`/opt` no se tocó**: la copia se monta dentro del contenedor en `/opt/arbitragex-v2/backend` **desde `/tmp`**, y el `/opt` del host queda fuera.

---

## 2. ★ A — DISEÑO DEL GATE

### 2.1 `read_verdict` — total y fail-closed

`decision_frontier.rs:137-165` lee **solo** dos claves de un envelope propio (`verdict`, `verdict_reason`, `#[serde(default)]`, claves desconocidas ignoradas ⇒ el read es **estrictamente aditivo**: no puede rechazar un payload que el parse tipado habría aceptado). Mapea a `Verdict`:

- `"accept"` → `Accept` (la **única** ruta a `EligibleSimulations`).
- `"reject"` → `Reject { reason }`, con `reason = None` si `verdict_reason` es null/ausente/vacío (familia propia, **no** un string vacío).
- **ausente o `null` → `Absent`** (fail-closed, y **no** es `Accept`: declarado en el doc del enum).
- string desconocido → `Unknown { raw }` con `raw` **acotado a 64 bytes** (`RAW_CAP`); tipo no-string → `Malformed { kind }`.
- un payload que no parsea como objeto JSON → `Absent` (fail-closed) y **el parse tipado conserva la propiedad del mensaje** (responde `invalid_msg_parse`).

**Nunca falla y es total**: cada entrada cae en exactamente una variante.

### 2.2 `admission` — total, y con UNA sola ruta admitida

`admission(&Verdict) -> Admission` (`:209-238`) mapea los 5 estados a **exactamente dos conjuntos**: `Accept → EligibleSimulations`; **todo lo demás** → `DecisionLog { reason, family }` con un `reason` prefijado (`producer_verdict:` + `reject:<reason>` / `envelope_missing` / `verdict_unknown:<raw>` / `verdict_malformed:<kind>`). `Admission::EligibleSimulations` se documenta como *«el ÚNICO conjunto que puede consumir fork RPC — y el único cuyas entradas pueden producir `passed = true`»*, y `DecisionLog` como *«explicado, registrado y ACKeado; no adquiere permiso de rate/in-flight y no llega a ningún simulador»*.

### 2.3 `ReasonFamily` — vocabulario ACOTADO, y COMPLETO contra el productor

`ReasonFamily` (`:247-264`) es una lista cerrada de 15 valores, y `PRODUCER_REASONS` (`:269-281`) mapea **8** razones exactas. **El productor está en el mismo árbol y lo leí** (`backend/selector-api/src`): `engine.ts` es una **unión de tipos cerrada** —

```ts
export type Decision =
  | { kind: "accept"; score: number; reason: null }
  | { kind: "reject"; score: number | null; reason: string; severity: RejectSeverity };
```

y `consumer.ts:451-452` estampa cada payload publicado:

```ts
verdict: decision.kind,
verdict_reason: decision.kind === "reject" ? decision.reason : null,
```

Los `reason` que el motor puede emitir son **exactamente los 8 del allowlist**, con su línea medida en `backend/selector-api/src/policy/engine.ts` (blob del head): `kill_switch_on` **:48**, `producer_rejected` **:57**, `blacklist_hit` **:65** (el literal es `bl.reason ?? "blacklist_hit"`), `token_safety_circuit_open` **:74**, `safety_below_threshold` **:96**, `simulation_failed` **:104**, `revert_risk_too_high` **:112**, `score_below_min` **:120**; la unión cerrada está en **:17-18**. ⇒ **la etiqueta de la métrica no puede desbordarse con el productor actual**, y el estado `Other` queda como bucket honesto para lo que aparezca en el futuro (señal de extender la lista, no escondite) mientras la razón cruda se **persiste verbatim** en la fila de `simulations`.

### 2.4 El acoplamiento con el clasificador NO invierte la propiedad del productor

`persistence.rs` suma **un brazo** a `is_sim_capability_gap`: `fail_reason.starts_with(sim_ctl::decision_frontier::SKIP_REASON_PREFIX)` — el prefijo se lee del módulo (una sola fuente de verdad). Sin ese brazo, el `UPDATE` habría re-marcado la oportunidad con una razón **sintética** y **pisado el `rejection_reason` que `selector-api` ya escribió** en la misma transacción. El test que lo acompaña lleva **control en la dirección opuesta**: `revert`, `gas_exceeded`, `insufficient_profit` **siguen rechazando** ⇒ el brazo no se traga veredictos de mercado.

**A: PASS.** El gate admite `accept` y sólo `accept`; deniega todo lo demás **explicándolo**; el vocabulario coincide con el del productor por construcción de tipos; la etiqueta está acotada y la evidencia no.

---

## 3. ★★ B — EL HUECO DECLARADO POR EL AUTOR: el runtime queda **NO VERIFICADO**

**No lo heredo como probado, y no lo doy por probado yo: lo declaro con precisión.**

- El `consumer` **no corre E2E** en esta revisión: requiere Redis + Postgres + fork, y el binario **desplegado** es **anterior** al PR (verificado: `docker exec arbitragex-v2-sim-ctl-1` no conoce `validated_frontier` — **0 ocurrencias** en su salida de métricas — y el contenedor no expone endpoint de métricas propio). Montar un harness E2E del consumer sería **construir un artefacto nuevo**, no revisar el PR.
- Por lo tanto **el comportamiento en runtime de la frontera NO ESTÁ MEDIDO**: no observé una sola entrada moviéndose de `eligible` a `decision_log` en ejecución real.
- Lo que **sí** sostiene el cambio, y lo que **no**: **(1)** el **invariante de colocación es estructural** (§4) — un `include_str!` sobre el fuente, no una observación de runtime; **(2)** el **falsificador** prueba que los tests *muerden* (§6), pero también en un árbol de fuentes, no en runtime; **(3)** los **tests puros** de la frontera (el target **lib**, 23 tests) prueban la clasificación de los 5 estados, en unidad.
- **Evidencia de runtime que SÍ aporto, y es sobre la ENTRADA, no sobre el gate**: el censo del canal vivo (§7) muestra que **el 100 % de lo que la frontera va a ver hoy es `reject` con familia conocida**. Es la distribución de entrada del gate, no su efecto.
- Sobre los **tests puros** que sí existen: la frontera vive en el target **lib** de `sim-ctl` (`lib.rs:12`), y ese target corre **23 tests** — es exactamente el conjunto donde muerden mis dos mutantes (**23 = 20+3** en F1; **23 = 22+1** en F2; baseline `--lib` = **23 passed / 0 failed**). Prueban la clasificación de los 5 estados **en unidad**, no el runtime.
- **Lo que cerraría el hueco** (evidencia concreta): desplegar el binario y medir `arbx_sim_validated_frontier_total` con tráfico real observando el delta `decision_log` crecer **y** `eligible_simulations` quedarse en 0 mientras el stream es reject-only — con `docker inspect StartedAt` como frontera pre/post y corroborando con una serie que **sí** crece (p. ej. `cache_hit`), nunca deduciendo de una ausencia.

---

## 4. ★ C — EL ORDEN: la frontera corre ANTES de adquirir el fork

**Path + línea + blob, todo medido en la copia del head** (`backend/sim-ctl/src/consumer.rs`, blob `d06287cb78b14f98d6be6d583d97cbcc23654ea8`):

| línea | qué es |
|---:|---|
| **895** | `let admission = decision_frontier::admission(&decision_frontier::read_verdict(&json));` ← **la frontera** |
| 924 | `let permit = in_flight.clone().try_acquire_owned().ok();` ← permiso de in-flight |
| 929 | `if !bound.try_admit(now) {` ← presupuesto de tasa |
| 959 | `Some(b2c) => match self.simulate_b2c(b2c, &opportunity, &id).await {` |
| 1006 | `.simulate_with_route(&opportunity, &route_path)` |

⇒ **895 < 924 < 929 < 959 < 1006**: la admisión se resuelve **antes** del permiso, del presupuesto y de los dos simuladores. Y el brazo denegado **no se descarta**: `return self.finish(&id, &opportunity, skip).await;` — llega a la cola compartida de persistencia/XACK con su razón tipada.

**El pin existe y pinnea lo correcto.** `decision_frontier.rs:674-729` (`frontier_runs_before_the_fork_budget_and_the_simulators`) lee `include_str!("consumer.rs")`, recorta `async fn process_message(` … `async fn finish(`, y exige: `frontier < try_acquire_owned()`, `frontier < bound.try_admit(now)`, `frontier < self.simulate_b2c(`, `frontier < .simulate_with_route(`, **exactamente un dispatch de cada simulador** (dos serían un bypass), y que el brazo `DecisionLog` **retorne por `finish(` antes** de cualquier simulación. Compara **offsets absolutos** (convierte los relativos), que es justo el detalle que haría pasar la aserción en silencio si se comparara relativo contra absoluto.

**Naturaleza del invariante: DE COLOCACIÓN (ESTRUCTURAL), no de comportamiento medido.** Es un análisis de texto sobre el fuente — el propio test lo declara («el consumer completo no se puede ejecutar en este gate: necesita Redis, PG y un fork»). Confirmo la clasificación del autor **y** su límite: un `include_str!` no puede detectar que el runtime tome **otra** rama.

---

## 5. ★ D — NO SE FILTRA NI SE ROMPE NADA MÁS

- **`shared-rs/src/contracts.rs` NO está en el PR** y su blob en el árbol del head es **`0a70fa5608bcfe39697c6d10dd523b4985c903b7`** = el exigido `0a70fa56…` ⇒ **no ganó campos**. La razón está documentada en el propio cambio (el `verdict` se lee de la **JSON cruda**, nunca de `Opportunity`, que queda byte-idéntico; añadirle campos rompería los literales `Opportunity {` de fuera de la crate). **`contracts.rs` intacto ⇒ no es `reject`.**
- **`CANDIDATE_SLOTS` intacto, por bytes**: vive en `sim-ctl/src/signer_funding.rs`, cuyo blob en el head es **`6b5fb633f4cdd7547a8bb9ae31f2386cb4d2d734`** — **idéntico al de `main`** ⇒ esa ruta no se tocó (y de paso prueba que el contenido de #879 está **dentro** del árbol del head).
- **Gate de paper y umbrales**: los 5 archivos son `metrics.rs`, `consumer.rs`, `decision_frontier.rs` (nuevo), `lib.rs`, `persistence.rs`. Ninguno es config, umbral, `sizing`, `risk`, ni gate de paper. El único cambio de comportamiento fuera del gate es **aditivo** (un brazo de clasificación con su control en contra, §2.4).
- `lib.rs:12 pub mod decision_frontier;` va al target **lib** a propósito, para que sus tests corran bajo el `cargo test --workspace --locked --lib` bloqueante del CI.

**D: PASS.**

---

## 6. ★ H — EL FALSIFICADOR, RE-EJECUTADO CON MUTANTES MÍOS (no citado)

El contrato exige reproducir F1/F2. **No heredé los mutantes del autor: construí los míos con la misma intención** (por eso mis blobs mutantes difieren de los suyos — y por eso el resultado es más fuerte: dos construcciones independientes del mismo fallo).

| brazo | mutación (mía) | blob mutante | resultado | restauración |
|---|---|---|---|---|
| **F1** — el filtro deja de filtrar | en `admission()`, `Verdict::Reject` → `Admission::EligibleSimulations` | `5e3df48e34d084d58e473fc96318966a93511724` | **exit 101** · `FAILED. 20 passed; 3 failed` · fallan `accept_is_admitted_and_reject_is_not`, `reject_without_a_reason_gets_its_own_family`, `every_observed_state_lands_in_exactly_one_set` | `1385186a…` **exacto** |
| **F2** — la frontera pierde su colocación | la llamada deja de contener el needle del pin (`df::admission(...)`) | `36d818fef0bb0d9934c43319572f80b2eb31751b` | **exit 101** · `FAILED. 22 passed; 1 failed` · falla `frontier_runs_before_the_fork_budget_and_the_simulators` · **pánico en `sim-ctl/src/decision_frontier.rs:688:21`** | `d06287cb…` **exacto** |

**Los dos mutantes dan EXACTAMENTE los conteos que el autor declaró (20/3 y 22/1) y el pánico cae en la MISMA línea (`688:21`)** — dos construcciones independientes, el mismo par de números. **Un mutante con exit 0 habría sido `needs_revision`; los dos son exit 101.** Baseline sin mutar (`--lib`): **23 passed / 0 failed**.

---

## 7. ★ E — ¿PUEDE DENEGAR DE MÁS? (el riesgo simétrico que F1 no cubre)

Enumeré **los caminos a `DecisionLog`** y busqué si alguno es alcanzable por una oportunidad legítima:

| camino | ¿alcanzable por un payload legítimo? |
|---|---|
| `reject` | **Sí, y es el propósito**: el productor ya rechazó ese mensaje; el gate deja de gastar fork en diagnosticarlo |
| `Absent` (sin `verdict`, `null`, o payload no-objeto) | **No con el productor actual**: `publishValidated` estampa `verdict` en **cada** payload. **Residual declarado** (§7.2) |
| `Unknown { raw }` | **No**: `decision.kind` es una unión cerrada (`"accept"`/`"reject"`) |
| `Malformed { kind }` | **No**: el tipo impide un `verdict` no-string |

### 7.1 Censo del canal VIVO (dos extremos de la ventana)

`XLEN arbx:opps:validated` = **10 001**. Muestreé **1 000 entradas de cada extremo** (`XRANGE` más antiguas / `XREVRANGE` más recientes):

| medida | extremo antiguo | extremo nuevo |
|---|---:|---:|
| `"verdict":"reject"` | **1 000** | **1 000** |
| `"verdict":"accept"` | **0** | **0** |
| valores distintos de `verdict` | **1** (`"reject"`) | **1** (`"reject"`) |
| `verdict_reason` = `producer_rejected` | 1 000 | — |
| control (`rejection_reason` presente) | **1 000** | **1 000** |

⇒ En 2 000 entradas muestreadas (20 % de la ventana) **no hay un solo caso de estado de envelope**: nada legítimo está siendo denegado hoy por la frontera, y la razón entra siempre por el allowlist conocido.

### 7.2 Residual declarado (el único que queda vivo)

**Un productor anterior al contrato `verdict` (o uno que lo viole) publicaría mensajes SIN la clave** ⇒ la frontera los deniega a **todos** (fail-closed). Es un **riesgo de versionado**, no de oportunidad legítima actual: el censo muestra 100 % con veredicto reconocido. **Lo que lo cerraría**: contar, en una ventana más larga del stream, las entradas **sin** `verdict` (y no sólo samplear dos extremos), y — si el canal admitiera varios productores — exigir el veredicto por contrato en el punto de publicación.

**E: PASS con el residual de §7.2 declarado.**

---

## 8. ★ I — GATES DEL HEAD, y una CONDICIÓN DE ATERRIZAJE NUEVA

- **Tree del head = `dbe76d83da3902a6bbe8d4e18c10f3f590cde304`** — **idéntico** al que el autor declaró (§1). Medido con `git rev-parse 'FETCH_HEAD^{tree}'` sobre el fetch del ref del remoto.
- **Re-parento REAL, no tautología**: `merge-base(head, main) = 77b42b3dccc001455fda3e8d4437d973c9e98c4b` con `ahead_by: 1` y `behind_by: 0` respecto de **ese** punto. El ataque «los hashes coinciden por construcción» muere por dos vías independientes: (a) el merge-base es el tip que `main` tenía al re-parentar, no el tip actual; (b) `signer_funding.rs` tiene el **mismo blob** en el head y en `main` (`6b5fb633…`) ⇒ el contenido de #879 **está dentro** del árbol del head, y eso es una coincidencia **medida**, no derivada.
- **Suites sobre el head** (cargo 1.91.1, `--locked`, `out`/`cache` no aplican aquí: es cargo, y el build fue fresco sobre la copia):
  - `cargo test -p sim-ctl --locked` → **118 passed / 0 failed / 2 ignored** (agregado de 6 targets) ⇒ coincide con lo declarado (`118/2i/0`).
  - `cargo test -p shared-rs --lib --locked` → **284 passed / 0 failed** ⇒ coincide con lo declarado.
- **CI de la plataforma sobre el head** (`/commits/7e059208…/check-runs`): **34 checks, los 34 `completed/success`, todos con `head_sha = 7e059208…`** — incluidos `cargo check + clippy + test`, `lint-and-test-rust`, `Rust tests`, `rust-check`. **Declarado como evidencia de PLATAFORMA, no como reproducción mía**: no corrí `clippy` ni `fmt` (no están en el comando de `verify` de esta orden).

### 8.1 ★ HALLAZGO NUEVO (condición de aterrizaje): `main` avanzó 7 commits

`main` está hoy en **`d5114eddf59ac1f9ae81be58edc22fffd75b65ee`**, y `compare(main, head)` da `merge_base = 77b42b3d`, `status = diverged`, **`behind_by: 7`**, `ahead_by: 1`.

⇒ **El head está 7 commits por detrás de `main`.** Todo lo medido (tree, suites, CI, falsificador) es sobre **ese head**, que es lo correcto para juzgar el cambio — pero **el resultado del merge no es lo medido**: al re-basar, el árbol cambia y **hay que re-correr los gates y el falsificador** antes de aterrizar. No es un defecto del PR (el re-parento era correcto cuando se hizo); es una **precondición de aterrizaje** que corresponde a t166. **Y es exactamente el tipo de cosa que convierte un verde en un verde falso si se aterriza sin re-medir.**

---

## 9. ★ F y G — Lo que NO se dice, y el instrumento

**F — no se reintroduce la cifra de liberación de fork.** Esta revisión **no cita ningún número** de fork, RPC o capacidad liberada. Lo único que declara es lo que los bytes y las líneas sostienen: **un rechazo no adquiere permiso de in-flight, no consume presupuesto de tasa y no alcanza ningún simulador** (líneas 895 < 924 < 929 < 959/1006 y la aserción del pin). Que eso libere X fork/minuto **no lo medí y no lo afirmo** — el autor lo corrigió y el capitán retiró su propia afirmación; esta acta no la resucita.

**G — el instrumento nuevo: NO COMPUTADO, no cero.** `SIM_VALIDATED_FRONTIER_TOTAL` (`arbx_sim_validated_frontier_total{outcome,reason}`) está **registrado en el código** (`backend/shared-rs/src/metrics.rs`, +38: `pub static … Lazy<IntCounterVec>` en **:477**, el nombre de la serie `"arbx_sim_validated_frontier_total"` en **:480**, y el toque de registro en `init_metrics` en **:547**). **Pero no tiene productor en producción**:

- La consulta por loopback (`ssh` + `http://localhost:9090/api/v1/query?query=arbx_sim_validated_frontier_total`) → **`{"result":[]}`** = la serie **no existe**.
- **Control positivo del mismo canal**: `up{job=~"sim-ctl.*"}` → `1` para `instance=sim-ctl:3003` ⇒ el canal **mide**; el vacío es real, no un instrumento mudo.
- **Control negativo del defecto del capitán**: el `:9090` **externo** (`http://195.201.235.70:9090/...`) → **`external_9090_http_code=000`** ⇒ por ahí no se llega; el loopback por `ssh` era la única vía.
- **El binario desplegado no conoce la métrica**: `grep -c validated_frontier` en su salida de métricas ⇒ **0**. Es el estado **pre-PR**, coherente con que el PR no aterrizó.
- **Frontera pre/post** para cualquier medición futura: `docker inspect arbitragex-v2-sim-ctl-1 --format '{{.State.StartedAt}}'` → **`2026-10-08T14:16:07.1195963Z`**.
- **Y la lección de t175, aplicada**: el contador de fondeo es **acumulativo** y **el deploy reinicia `sim-ctl`** ⇒ cualquier delta dice **de qué arranque viene**, y se corrobora con una serie que **crece** (p. ej. `cache_hit`), **nunca deduciendo de una ausencia**.

**Un contador sin tráfico es NO COMPUTADO, no cero.** Estado del instrumento: **registrado en código, sin productor en el binario servido.**

---

## 10. Defectos de MI instrumento (declarados antes de que otro los encuentre)

1. **`Set-Content -AsByteStream` dejó el `git archive` en 0 bytes** (el pipe de PowerShell re-encodea/escribe mal los binarios). Corregido con `git archive -o <archivo>` directo: 19 025 920 B → `gzip` 3 496 686 B.
2. **`bash -lc` dentro del contenedor perdió `RUSTUP_HOME`/`PATH`** ⇒ `cargo` corría pero **no encontraba `rustc`** (`could not execute process rustc -vV`), y con `sh -c` se perdía `PIPESTATUS` (`Bad substitution`). Corregido: invocar `cargo` **como comando directo** de la imagen (`docker run … rust:1.91 cargo …`), sin shell intermedio.
3. **Paths de `cp` mal armados en job2** (omití el segmento `sim-ctl`): los mutantes **sí** se aplicaron (python con ruta absoluta) pero las copias de respaldo y la restauración fallaron ⇒ **re-extraje el árbol desde el tar** y verifiqué los blobs prístinos antes de continuar. Nada de eso tocó el repo ni `/opt`.
4. **Un control mío quedó mal elegido** en el primer censo del stream: grepeé `"json"` como control positivo y dio 0 porque las entradas del stream son planas, no envueltas en `json`. El control válido es `rejection_reason` (1 000/1 000) ✔.
5. **Primer `gh api` de los diffs** falló por comillas mal escapadas en `--jq` (`accepts 1 arg(s), received 2`); re-hecho con filtro por `test(...)`.

---

## 11. Lo que NO queda establecido (fail-honest)

1. **Comportamiento en runtime del gate**: **NO COMPUTADO** en esta revisión (§3), con la evidencia que lo cerraría.
2. **`clippy -D warnings` y `fmt`**: **no los corrí** (no están en el `verify` de esta orden). La CI los reporta verdes sobre el head (`cargo check + clippy + test`), declarado como **evidencia de plataforma**.
3. **La liberación de fork como cifra**: **no medida y no afirmada** (§9).
4. **El universal de §7.2**: muestreé 2 extremos (2 000 entradas); **no** barrí la ventana completa ni varias ventanas para descartar un productor legado.
5. **La equivalencia entre el árbol del head y el resultado del merge**: el merge no existe (no está aterrizado) y `main` avanzó 7 commits (§8.1).
6. **`Other` como familia alcanzable**: si `blacklist.ts` devolviera un `reason` distinto de `blacklist_hit` (el `?? "blacklist_hit"` de `engine.ts:65` sugiere que existe esa posibilidad), la familia sería `Other`; **no verifiqué el vocabulario de `blacklist.ts`**. No es un defecto (bucket honesto + razón verbatim) pero **no lo medí**.

---

## 12. Respuesta directa

**¿El gate admite lo que debe admitir y deniega lo que debe denegar?** **Sí.** La única ruta a simulación es `verdict == "accept"`; todo lo demás se deniega **explicado** (razón tipada persistida, contada, ACKeada, sin permiso ni simulador). El vocabulario está **acotado** (allowlist de 8 + 3 estados de envelope) y **completo contra el productor**, que es una **unión de tipos cerrada**. (`A: PASS`)

**¿El hueco del E2E?** **Declarado NO VERIFICADO en runtime**, con lo que lo cierra (§3). El cambio se sostiene sobre **(1)** el invariante **estructural** de colocación y **(2)** el falsificador — y esta acta **no hereda** el runtime como probado. (`B: NO COMPUTADO`)

**¿El orden?** **La frontera corre antes de adquirir el fork**: `consumer.rs:895` < `924` < `929` < `959`/`1006`, blob `d06287cb…`. Invariante **de colocación (estructural)**, y el pin de `decision_frontier.rs:688:21` **sigue ahí y pinnea el orden correcto** (verificado por el mutante F2, que hace paniquear **esa línea exacta**). (`C: PASS`)

**¿Denegar de más?** **No con el productor actual**: 2 000 entradas muestreadas del canal vivo, **100 % `reject` con familia conocida, 0 casos de envelope**. Residual declarado: un productor sin el contrato `verdict` quedaría denegado en bloque (fail-closed, riesgo de versionado). (`E: PASS con residual`)

**¿Se filtra o rompe algo más?** **No**: `contracts.rs` **no está en el PR** y conserva `0a70fa56…`; `signer_funding.rs` (donde vive `CANDIDATE_SLOTS`) tiene el mismo blob que `main`; ningún archivo de config, umbral o gate de paper está entre los 5. (`D: PASS`)

**¿Se reproduce lo que el autor midió?** **Sí, todo**: tree `dbe76d83…`, `118/2i/0`, `284/0`, y el falsificador con **mutantes míos** dando **20/3** (F1) y **22/1 con pánico en `688:21`** (F2), con restauración exacta. (`H: PASS`)

**¿Y la cifra de liberación de fork?** **No se cita**: esta acta no la reintroduce. (`F: CUMPLIDO`)

**¿F25/N11 cerrado?** Este PR **no cierra nada por sí solo**: cierra cuando el binario con la frontera esté **sirviendo en producción** y el instrumento muestre tráfico. Hoy la serie **no existe** y el binario desplegado no conoce la métrica. (`G: NO COMPUTADO`)

---

*Revisión independiente. No mergea, no despliega, no firma. Un aterrizaje por ciclo: t166 queda detrás de esta acta y debe **re-basar y re-medir** antes de aterrizar (§8.1).*
