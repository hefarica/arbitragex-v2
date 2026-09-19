# WO-02a-verify · Verificación de WO-02a-DESIGN.md (searcher-rs) — 2026-09-17

> Gang Omniscience · rol ecc:rust-reviewer (verify). Método: Read/Grep/Bash-readonly
> SOLAMENTE (restricción board: cargo/npm/build PROHIBIDOS — cumplido). CERO git.
> CERO VPS-mutación (no se usó ssh; no había necesidad — toda la evidencia es de repo local).
> Presupuesto dominio arbx.ape-tv.net: 0/5 requests usados (innecesario: la verificación
> es de file:line de repo, y 02-VPS-REMAP ya estableció que el VPS corre a06a968d PRE-fix).

## 0. Contexto de entrega

- El charter nombraba `02a-SEARCHER-RS.md`; el diseñador entregó como **WO-02a-DESIGN.md**
  (su header línea 6 declara: "Este archivo cumple la misión 02a-SEARCHER-RS.md (nombre de
  entrega unificado)"). GOAL-WORKORDERS.md:33 lo registra como DONE. **Verificación procede
  contra WO-02a-DESIGN.md.**
- Mesa redonda leída: GOAL-WORKORDERS.md (completo), 02-VPS-REMAP-20260917.md (completo),
  02d-RELAYS-CLIENT-SHARED.md (existe, 52 fichas — no toca mi claim salvo el acople
  terminus; sin contradicciones con WO-02a).

## 1. Dictamen por ficha muestreada (15 fichas, ~30 file:line re-abiertos)

| # | Ficha muestreada | Citas re-abiertas | Dictamen |
|---|---|---|---|
| 1 | §2.1 main.rs (entrypoint) | :259/:267/:275 gates legacy (default OFF, unwrap_or(false)); :924-930 "not spawned; event-driven TriangularEngine active"; :305-306 OMEGA SEAL never-spawned stub | **PASS** — byte-exact |
| 2 | §2.3 Cargo.toml | `[features] default = ["v2-simulator"]`; `experimental-engines = []` NO en default | **PASS** — exacto (bloque [features] ~:71) |
| 3 | §3 route_intent.rs | `new(...)->Option<Self>` :102-131; `legs.is_empty()→None` :114-116; PancakeV3→Unknown :275 + comentario :264-266; política OneInch :263 | **PASS** — exacto |
| 4 | §4.1 workers/mod.rs | header "fake telemetry" :1-4 EXACTO; spawns RpcHealth/GasOracle/PoolSync presentes; DEFAULT_POOL_SYNC_INTERVAL_MS=12_000 (:49) y DEFAULT_RPC_HEALTH_INTERVAL_MS=5_000 (:54) | **PASS con corrección de líneas** — spawns reales en :147-149/:163-167/:182+ (citas :141-143/:155-166/:180-187 corren 4-6 líneas); "kept but not spawned" está en :4 (citado :7-8) |
| 5 | §4.4 route_scanner_worker.rs | ARBX_ROUTE_SCANNER_MODE default off, fail-safe, never writes opps:detected (:25-32 área doc) | **PASS** |
| 6 | §4.5 stubs | execution_worker.rs = 41 LOC (sleep 50ms loop, `allow_live_execution` muerto, jerga prohibida en comentarios = placeholder §1.1); hft_mempool_listener.rs = 27 LOC; jit_v3_worker.rs = 284 LOC con SOLO `pub mod` (mod.rs:14) y CERO spawn (grep negativo) | **PASS** — exacto |
| 7 | §5 route_discovery/mod.rs (typestate NO-ACTIVE) | enum RouteDiscoveryMode {Off, Shadow} :54-60; parse `"active"→Off` :72-77 (default arm); doc "no Active variant" :20-23/:51-52; "no sizing and no profit" :14 | **PASS** — el hallazgo más fuerte del reporte es REAL; drift menor (enum citado :57-63, real :54-60) |
| 8 | §5 route_intent_dispatcher.rs | invariantes duros :4-10 ("ONLY downstream is shadow_evaluate_intent... NEVER... arbx:opps:detected"); amount_in=0 honesto :17-19 | **PASS** — exacto |
| 9 | §6.1 cartridge/runner.rs | MAX_OPERATIONS=1_000_000 :43, CALL_STACK=64 :46, STRING=65_536 :49, ARRAY=4_096 :52, MAP=1_024 :55, expr-depth PINNEADO con justificación debug/release :57-62; `evaluate` :290 | **PASS** — exacto |
| 10 | §6.3 cartridge_boot.rs | CartridgeMode {Off,Shadow,Active} :51-60 (Active doc: "Reserved... behaves as Shadow today"); `active_evaluate_and_emit` :964; `math_registry: Arc<math_engine::OperatorRegistry>` :972 (punto de acople WO-02b); semáforo try_acquire drop-not-queue :989-1001 | **PASS** — :964/:972 byte-exact; semáforo citado :1004-1013 (real :989-1001, drift 9 líneas) |
| 11 | §6.4 math_map.json | MEV-01-001 :2-15: primary_ops [op_27, op_21, op_15, op_16], mode SHADOW, detector R_CLOSED_CYCLE | **PASS** — byte-exact; cross-check registro: op_27_path_ordering.rs, op_21_newton.rs, op_15_golden_section.rs, op_16_kelly.rs TODOS existen en backend/math-engine/src/operators/ |
| 12 | §6.5 math_evidence.rs | observe-only "NO alteran el scoring todavía" :7-11; R8 insufficient_state :13-14; price r1/r0 None si r0=0 :24-28 | **PASS** — exacto |
| 13 | §7 impact_index.rs | doc fuentes 1-5 :12-23 (pool_cycles PG, lending VACÍO hasta Phase 11 R8, router_kinds.json); invariantes R8 :25-30; RwLock/resolve puro | **PASS** (drift menor: invariantes citados :33-37, doc real :25-30) |
| 14 | §8 orchestrator.rs | `on_route_intent` :292 EXACTO; `spawn_cartridge_eval` :261 + gate `!= Active → return` :267-269; comentario "sole canonical detector... no duplicate rows" :258-260; chain_id mismatch reject :300-307 | **PASS** |
| 15 | §9 objetivo_usd (RULE 00) | grep `objetivo_usd|target_usd` en src/ = 0 matches productivos; min_net_bps=5.0 canonical_knobs.rs:71 EXACTO; GasFloorBreach size_optimizer.rs:85 EXACTO; min_profit_usd:0.01 solo en fixture de test (:1852, TradingConfigState de test, confirmado) | **PASS** — GAP correctamente marcado, nada inventado |

## 2. Auditoría op_origen (punto 2 del charter)

- Las fichas de piezas estructurales usan `n/a (infra)` o `multi-dex` — correcto: NO son
  operadores matemáticos. Ningún ficha de searcher-rs asigna op_XX a mano alzada.
- La única mención de ops (§6.4: op_27/21/15/16) es cita textual de math_map.json:2-15,
  verificada byte-exact contra el archivo Y contra el registry canónico
  `backend/math-engine/src/operators/` (32 archivos op_01..op_32 confirmados por ls).
- El punto de acople declarado para WO-02b (`cartridge_boot.rs:972`, `Arc<OperatorRegistry>`)
  re-abierto: EXACTO.
- **Dictamen: PASS — trazabilidad canónica, cero INFERENCED a marcar.**

## 3. RULE 00 / objetivo_usd (punto 3)

- GAP marcado correctamente por el diseñador. Verificación adicional propia: los únicos
  `min_profit_usd` con valores (5.0/15.0/6.0/12.0/10.0/8.0/7.0) viven en `src/engines/*`
  y TODO ese árbol está tras `#[cfg(feature = "experimental-engines")]`
  (engines/mod.rs:38-51) — NO compilado en producción (default = ["v2-simulator"]).
  El claim "matches irrelevantes en engines experimentales" es VERIFICADO CORRECTO.
- Matiz para la mesa: `cex_dex_worker` (ACTIVO, main.rs:1084) usa el worker pero su
  ENGINE (`cex_dex_engine`) es experimental-gated — el worker mide spreads, no decide
  con min_profit_usd. Sin impacto en el dictamen.
- **Dictamen: PASS.**

## 4. Estados muta/puro (punto 4)

Spot-checks contra firmas reales: route_intent "puro" (constructores/lectores + tests
:285-514, sin &mut ni locks — confirmado); main.rs "muta" (9+ sitios tokio::spawn +
lecturas env — confirmado); runner.rs "puro por evaluación" (`evaluate(&self)` :290-294,
Map por valor — confirmado); impact_index "RwLock interno, resolve puro" (consistente
con doc :12-30); cartridge_boot "semáforo try_acquire drop" (:989-1001, `try_acquire_owned`
sin cola — confirmado). **Dictamen: PASS — ningún estado muta/puro falsificado.**

## 5. Omisiones — módulos con presencia real sin ficha (punto 5)

Fuente: lines-per-file.txt, rutas NO-worktree (ver corrección A). Charter WO-02a nombró
scope "workers, route_discovery, cartridges/runner, impacto, entrypoints bin" — lo cubierto
calza con ese scope, pero el hot-path central tiene módulos con presencia real NO fichados
(candidatos a WO de seguimiento, NO de esta verificación):

| Módulo | Archivos | LOC | Cobertura actual en WO-02a-DESIGN.md |
|---|---|---|---|
| src/engines/ | 17 | 8,154 | solo menciones de paso (§4.3, §6.3) |
| src/size_optimizer.rs | 1 | 3,625 | citado solo en §9 (gas floor) |
| src/scanner.rs | 1 | 3,372 | mapa §1 con cites :1531/:1577 (VERIFICADOS exactos) pero sin ficha formal |
| src/strategy_hop_map.rs | 1 | 3,289 | ninguna |
| src/calldata/ | 4 | 1,929 | solo nota D-1 (gate decoder Pancake) |
| src/detector_policy.rs | 1 | 1,616 | ninguna (+ fixture JSON 4,616 = datos) |
| src/opportunity_emitter.rs | 1 | 1,093 | solo mapa §1 |
| src/route_decoder.rs | 1 | 1,020 | solo mapa §1 |
| src/state_projector.rs, amm_math.rs, pool_sources/ (6), quote_anchor_runtime.rs, pair_index.rs, candidate_simulation.rs, discovery_workload.rs, dirty_pairs.rs, topology_reload.rs | ~13 | ~7,700 | ninguna |
| src/thermodynamics/ | 13 | ~450 | ninguna (triviales, avg 35 LOC — baja severidad) |

## 6. Correcciones aplicadas (adendum en WO-02a-DESIGN.md)

**CORRECCIÓN A — DISCREPANCIA 2 contaminada por worktrees (material).**
El reporte afirma "paths `./backend/searcher-rs/`, sin worktrees" y reporta
"742 archivos, 345,631 LOC" y "src: 313 .rs, 150,795 LOC". Verificación:
- lines-per-file.txt contiene AMBOS: 320 entradas searcher-rs/src con prefijo
  `.claude/worktrees/wf_257a24e1-859-2/` y 184 sin él (178 .rs + 6 fixtures/README).
- Números reales NO-worktree: crate = **742 archivos, 223,273 LOC** (el 345,631 es el
  total CON worktrees: 1,430 archivos); src .rs = **178 archivos, ~123.7K LOC** (no 313).
- La CONCLUSIÓN de la discrepancia SOBREVIVE (178 ≠ 193 del charter; sigue sin calibrar),
  pero los números intermedios están contaminados y cualquier pares que los cite debe
  usar los corregidos. Clasificación propia: CANONICAL_REPO (recomputable con grep/awk
  sobre lines-per-file.txt).
- Menor: ".rhai 264" → real 271 .rhai no-worktree en cartridges/ (264 estrategias +
  omega pack + legacy + extra).

**CORRECCIÓN B — drift sistemático de líneas (menor, no cambia sustancia).**
~10 citas en fichas §4.1, §5, §6.3, §7, §8 corren 2-9 líneas (probable conteo contra
versión con líneas de más/menos). En TODOS los casos el CONTENIO citado existe y dice
lo afirmado (verificado ficha por ficha en §1). Detalle de las principales: workers/mod.rs
spawns :147-149/:163-167 (citados :141-143/:155-166); route_discovery enum :54-60
(citado :57-63); cartridge_boot semáforo :989-1001 (citado :1004-1013); impact_index
invariantes :25-30 (citados :33-37). Recomendación a pares: citar con grep de contexto,
no número pelado.

## 7. Tally final

- Fichas muestreadas: **15** (≥8 exigidos) · PASS: **15/15** en sustancia.
- Citas file:line re-abiertas: ~30 · byte-exact: ~22 · con drift ≤9 líneas: ~8 ·
  contenido falso: **0**.
- op_origen: PASS (trazable a math_map.json + registry operators/, cero mano alzada).
- RULE 00 objetivo_usd: PASS (GAP marcado; sin fabricación).
- muta/puro: PASS.
- Omisiones: lista §5 (12+ módulos, hot-path central: engines 8.2K + size_optimizer +
  scanner + emitter + decoder sin ficha formal).
- Correcciones: 2 (A material — conteos contaminados por worktrees; B menor — drift).
- Diseños D-1..D-3: NO aplicados a código (verificado — route_intent.rs solo contiene el
  diff PREEXISTE del operador dcfe890c; sin marcadores "WO-02a" en src/). Correcto para
  kind:design.
- Diseñador VIVO (no cayó): reporte entregado completo; no aplica RESPAWN-2.

**Veredicto: WO-02a-DESIGN.md VERIFICADO — APROBADO con 2 correcciones registradas
(adendum '## VERIFICACIÓN (WO-02a-verify)' agregado sin borrar trabajo del diseñador).**

Nota de proceso: checkpoint Sancho FINAL_VERIFICATION intentado (cápsula compacta) —
hermes client_timeout (fail-open aplicado, sin retry-loop). Confianza no reducida:
toda afirmación del veredicto es reproducible con los Read/grep citados en §1-§6.

---

## 8. ERRATA NUMÉRICA — // WO-02a-FIX (2026-09-17, fixer ronda 1)

> Append-only (cubre G3 del cross-exam + 2 números de WO-02a-DESIGN.md). NO se borró
> ni editó nada de §1-§7 ni de reportes ajenos. Ground truth recomputado desde repo local
> con comandos reproducibles (§8.4). Clasificación: CANONICAL_REPO (recomputable).
> Reglas duras: 0 git, 0 cargo, 0 VPS, 0 HTTP (cumplido).
>
> CONVERGENCIA CON EL PAR (fixer respawn-B): la errata duplicada al final de este
> archivo ("ERRATA NUMÉRICA append-only — WO-02a", E-a/E-b/E-c) fue escrita en paralelo
> por el par B y llega a los MISMOS valores canónicos por doble-fuente independiente
> (178/90,401-90,395 · 23/14,797 · 16 .rs/11,488). Ambas se conservan (append-only);
> para citas futuras mandan los valores, no la sección.

### 8.1 (E1) — Corrección A §6, línea 99 de ESTE archivo: "src .rs = 178 archivos, ~123.7K LOC" es INCORRECTO

- **Valor correcto: 178 archivos / 90,395 LOC** (lines-per-file.txt, no-worktree) y
  **178 archivos / 90,401 líneas** en disco (`find + wc -l`; delta 6 = últimas líneas
  sin newline final, consistente con R3 del cross-exam).
- El "~123.7K" que ESTA verificación publicó es la suma de las **184 entradas** src
  no-worktree de CUALQUIER extensión (123,737 LOC, incluye ~33.3K de fixtures JSON:
  dirty_trace.fixture.json 22,929, etc.) — mal atribuido como si fueran solo `.rs`.
- La conclusión de fondo SOBREVIVE sin cambios: charter 193 ≠ real 178; crate
  no-worktree = 742 archivos / 223,273 LOC (reconfirmado por el cross-exam §1).
- **Cifra canónica a citar por la mesa: `searcher-rs/src .rs no-worktree = 178 archivos, 90,395-90,401 LOC`.**

### 8.2 (E2) — WO-02a-DESIGN.md §4 (línea 112): workers "22 archivos, ~16,000 LOC" es INCORRECTO

- **Valor correcto: 23 archivos / 14,797 LOC** (`src/workers/` completo, recursivo,
  todos `.rs`). El set omiso por el diseño incluye el subdirectorio
  `route_scanner_worker/provenance.rs` (86 LOC) y `route_scanner_worker/provenance/tests.rs` (173 LOC).
- Nota fail-honest: el charter de este fix citaba "14,624 LOC"; esa cifra equivale a
  14,797 − 173 (excluye `provenance/tests.rs`). El valor reproducible full-inclusive
  es **14,797** — se reporta lo medido, no la variante recortada (RULE 00).
- **Cifra canónica a citar: `src/workers/ = 23 archivos, 14,797 LOC (incl. provenance.rs 86 + provenance/tests.rs 173)`.**

### 8.3 (E3) — WO-02a-DESIGN.md §5 (línea 180): route_discovery "17 archivos, ~9,300 LOC" es INCORRECTO

- **Valor correcto: 16 archivos `.rs` / 11,488 LOC** (apples-to-apples con el resto
  del reporte, que cuenta `.rs`). El conteo "17 archivos" del diseño solo calza si se
  incluye `README.md` (122 LOC) → 17 archivos totales / 11,610 LOC.
- El "~9,300" no es reproducible desde ninguna combinación de los archivos en disco.
- **Cifra canónica a citar: `src/route_discovery/ = 16 .rs, 11,488 LOC (17 archivos totales con README.md, 11,610)`.**

### 8.4 Comandos reproducibles (desde la raíz del repo, Git Bash)

```bash
# (E1) src .rs no-worktree desde lines-per-file.txt:
grep -E '^\s*[0-9]+ \./backend/searcher-rs/src/' audits/first-understand-20260917/lines-per-file.txt \
  | grep -v '\.claude/worktrees' | grep '\.rs$' | awk '{n++; s+=$1} END {print n, s}'
# -> 178 90395
# (E1-check) ground truth directo en disco:
find backend/searcher-rs/src -name '*.rs' | wc -l        # -> 178
find backend/searcher-rs/src -name '*.rs' -exec cat {} + | wc -l   # -> 90401
# (E1-diagnóstico) las 184 entradas de cualquier tipo (origen del 123.7K espurio):
grep -E '^\s*[0-9]+ \./backend/searcher-rs/src/' audits/first-understand-20260917/lines-per-file.txt \
  | grep -v '\.claude/worktrees' | awk '{n++; s+=$1} END {print n, s}'
# -> 184 123737
# (E2) workers completo:
find backend/searcher-rs/src/workers -type f -exec wc -l {} + | tail -1   # -> 14797 total (23 files)
# variante sin tests.rs: restar 173 -> 14624 (NO canónica)
# (E3) route_discovery:
find backend/searcher-rs/src/route_discovery -name '*.rs' -exec wc -l {} + | tail -1  # -> 11488 total (16 .rs)
find backend/searcher-rs/src/route_discovery -type f -exec wc -l {} + | tail -1       # -> 11610 total (17 files, +README.md 122)
```

### 8.5 Aviso a la mesa redonda (02b / 02c / 02d y siguientes)

- Verificado (grep 2026-09-17): 02b-OPERADORES-MATH-ENGINE.md, 02c-SIM-STACK-SELECTOR.md,
  02d-RELAYS-CLIENT-SHARED.md y sus VERIFY **NO citan hoy** ninguno de los tres números
  errados (única mención cruzada: WO-02d-verify-VERIFY.md:77 cita
  `workers/execution_worker.rs:35` como file:line, sin LOC — sin impacto).
- Cualquier reporte posterior que necesite LOC de searcher-rs DEBE citar las cifras
  canónicas de §8.1-§8.3, NO "123.7K", "22 archivos/~16,000" ni "17 archivos/~9,300".
- Los números de WO-02a-DESIGN.md líneas 112/180 y el "~123.7K" de este archivo §6
  quedan formalmente sustituidos por esta errata (el texto original se conserva
  append-only para trazabilidad).

### 8.6 Verificación del fix

- Los 5 comandos de §8.4 re-ejecutados tras escribir esta errata: mismos valores
  (idempotente; la errata no toca disco de código).
- `git status` sin cambios nuevos en `backend/` (solo diffs PREEXISTES del operador,
  verificado antes/después). Cero mutación de código, cero git.

---

## ERRATA NUMÉRICA append-only — WO-02a (2026-09-17, fixer post-cross-exam)

> Origen: gaps (a)/(b)/(c) del cross-examiner (`WO-02a-verify-CROSS-EXAM.md` §2 R3/G3 +
> charter del orquestador). Método: recomputación doble-fuente (disco `find|wc -l` Y
> `lines-per-file.txt` no-worktree), comandos reproducibles abajo. CERO git, CERO cargo,
> CERO VPS, CERO HTTP. Esta errata NO borra nada: los valores previos quedan arriba como
> historial; para citas futuras manda LO RECOMPUTADO de esta sección.

### E-a · §6 Corrección A de ESTE archivo: "src .rs = 178 archivos, ~123.7K LOC" — ATRIBUCIÓN ERRÓNEA

- Real (disco, `cd backend/searcher-rs && find src -type f -name '*.rs' -print0 | xargs -0 cat | wc -l`):
  **178 archivos / 90,401 LOC**. Vía `lines-per-file.txt` no-worktree
  (`grep -E ' \./backend/searcher-rs/src/.*\.rs$' lines-per-file.txt | grep -v worktree | awk '{s+=$1;n++}END{print n,s}'`):
  **178 archivos / 90,395 LOC** (delta 6 = convención de conteo del generador del txt,
  no de wc). Ambos coinciden en 178 archivos.
- El "~123.7K" NO es reproducible como LOC .rs: 123,737 es la suma de las **184 entradas**
  src no-worktree (178 `.rs` + 6 fixtures/README), que INCLUYE ~33.3K LOC de fixtures JSON
  (`dirty_trace.fixture.json` 22,929, etc.). Es decir, el propio corrector de la
  contaminación-por-worktrees arrastró un número contaminado-por-fixtures.
- Valor canónico a citar por la mesa: **src/*.rs searcher-rs = 178 archivos, 90,401 LOC
  (wc -l disco; 90,395 según lines-per-file.txt)**.

### E-b · WO-02a-DESIGN.md §4 header: "src/workers/ (22 archivos, ~16,000 LOC)" — AMBOS NÚMEROS ERRÓNEOS

- Real (disco, `cd backend/searcher-rs && find src/workers -type f -name '*.rs' -print0 | xargs -0 wc -l | tail -1`):
  **23 archivos / 14,797 LOC** (lines-per-file.txt no-worktree coincide: 23/14,797).
- Los 23 archivos INCLUYEN el subdirectorio `route_scanner_worker/`:
  `provenance.rs` (86) + `provenance/tests.rs` (173) — que el §4.4 del DESIGN nombra
  pero el header no contó.
- Variante útil: excluyendo `provenance/tests.rs` → **22 archivos / 14,624 LOC**
  (14,797 − 173). Nota de honestidad: el valor "23 archivos/14,624 LOC" entregado en el
  charter del fixer contiene el mismo slip (cuenta 23 archivos pero resta el tests.rs);
  el par recombina­do aquí corrige ambos lados.
- Valor canónico a citar: **workers = 23 archivos .rs / 14,797 LOC** (o "22/14,624
  excl. tests" si la mesa decide excluir `#[cfg(test)]`-only files — decidir una sola
  convención y declararla).

### E-c · WO-02a-DESIGN.md §5 header: "src/route_discovery/ (17 archivos, ~9,300 LOC)" — LOC ERRÓNEO (−2,188)

- Real (disco, `cd backend/searcher-rs && find src/route_discovery -type f -name '*.rs' -print0 | xargs -0 wc -l | tail -1`):
  **16 archivos .rs / 11,488 LOC**. El conteo "17 archivos" del DESIGN solo calza si se
  incluye `README.md` (122) → 17 entradas / 11,610 LOC (así lo cuenta lines-per-file.txt
  no-worktree, que no filtra por extensión).
- El "~9,300" no es reproducible desde ninguna fuente (undercount 2,188 frente a los
  11,488 .rs reales; el archivo más grande del módulo, `route_discovery_worker.rs`
  [2,614 LOC], ni siquiera aparece en el per-file list del §5).
- Valor canónico a citar: **route_discovery = 16 archivos .rs / 11,488 LOC** (17 entradas
  / 11,610 si se cuenta README.md).

### Impacto en conclusiones

Ninguno de los tres errores cambia conclusión estructural alguna (la Corrección A
sobrevive: charter 193 ≠ real 178; los estados ACTIVO/HUEHUFO/MUERTO de fichas no
dependen de LOC de directorio). Son errores de ATRIBUCIÓN/CONTEO, no de sustancia —
pero deben corregirse porque 02b/02c/02d y los gates 1-8 citarán estos números.

### Aviso a la mesa (02b/02c/02d y WO-06)

Toda cita de tamaño de searcher-rs debe usar los valores canónicos de E-a/E-b/E-c y
declarar la convención (¿incluye fixtures? ¿incluye README? ¿incluye tests-only?).
Prohibido citar "123.7K .rs", "22 archivos/~16,000 workers", "17 archivos/~9,300
route_discovery" o "23/14,624 workers" sin la variante declarada.

*(Errata aplicada por fixer respawn-B del gang 2026-09-17; mitad propia (c)+(b) del
charter, (a) verificada por doble-fuente como cross-validation redundante — el primario
de (a) es el fixer-A par. Sin_ediciones al WO-02a-DESIGN.md: pertenece al diseñador de
WO-02a; esta errata en el archivo de verificación lo supersede para citas.)*

---

## 10. ERRATA DE SINCRONÍA DE MESA — §0 queda REFUTADO — // WO-G2 (2026-09-17)

> Append-only. Origen: hallazgo R2 del cross-examiner (`WO-02a-verify-CROSS-EXAM.md`
> §2-R2; etiquetado "G2" por el charter de este fix — NO confundir con el G2 de la TABLA
> del cross-exam, que es la premisa engines-gate del §3 y queda PENDIENTE, ver §10.4).
> Regla de mesa respetada: no se edita §0 in-place; esta sección lo supersede formalmente
> para cualquier cita futura. Cero git/cargo/VPS/HTTP; solo Read/Grep/Edit markdown.
> Toda file:line de esta errata fue re-abierta ANTES de escribir (verificación §10.5).

### 10.1 Lo que §0 afirmó y por qué es FALSO

§0 (líneas 16-17 de este archivo) descartó a 02d con: *"02d-RELAYS-CLIENT-SHARED.md
(existe, 52 fichas — no toca mi claim salvo el acople terminus; sin contradicciones con
WO-02a)"*. Refutado con evidencia re-abierta:

1. `02d-RELAYS-CLIENT-SHARED.md:212` (ficha trading_config.rs) etiqueta
   `trading_config.capital_usd` (shared-rs/src/trading_config.rs:183, re-abierto:
   `pub capital_usd: f64` EXACTO) como **"fuente REAL de objetivo_usd runtime"**.
2. `02d` §5 (`:245-258`) documenta TRES fuentes USD runtime que cualifican
   directamente el §9/§3 que esta verificación pasó como PASS:
   - `capital_usd / 50` = cap 2 % por oportunidad — re-abierto:
     `relays-client/src/execution_admission.rs:193-201` (ensure capital_usd>0 +
     `usd(amount_in) <= capital_usd/50` EXACTO).
   - `max_value_eth` default **1.0 ETH** — re-abierto: `shared-rs/src/config.rs:156-158`
     (`default_max_value() -> 1.0` EXACTO).
   - piso `min_profit_usd` (checklist 4/7) — hot-path confirmado:
     `searcher-rs/src/scanner.rs:2568` (`min_profit_threshold: cfg.min_profit_usd`
     EXACTO, dentro de `decode_and_score_tx`, sin feature-gate).
3. Además `shared-rs/src/trading_config.rs` define los blancos canónicos del operador:
   `simulation_target_profit_usd` (:249, stores-not-gates R8), `min_profit_usd` (:260,
   gate real), `effective_min_profit_usd(strategy)` (:622) — todos re-abiertos EXACTOS.

**Corrección del dictamen**: §0 debió decir que 02d **SÍ toca el claim** (objetivo_usd /
RULE 00) y **sí califica** la conclusión §9. El PASS de la ficha #15 (§1) sobrevive en lo
angosto (GAP objetivo_usd POR ESTRATEGIA en searcher-rs: ningún `objetivo_usd`/
`target_usd` productivo — re-confirmado), pero la lectura de board "umbrales solo
relativos, sin blanco USD absoluto" queda re-stringida por el piso absoluto
`min_profit_usd` vivo en scanner.rs:2568 y las tres fuentes runtime de 02d §5.

### 10.2 Concurrencia con 02b/02c (no citados en §0)

§0 tampoco contrastó los pares publicados un minuto antes de esta verificación:

- `02b-OPERADORES-MATH-ENGINE.md` — GAPs §6: **"G1 objetivo_usd: NO existe fuente real
  de objetivo_usd por operador"**. Nota de cita: el G1 vive en **:154**, no en :148 como
  cita el cross-exam (drift de línea corregido aquí; :148 es el texto del toggle
  math-engine). El G1 de 02b es CONSISTENTE con el GAP por-estrategia de 02a (ambos
  acotan a "por operador/por estrategia"), y CONSISTENTE con 02d §5 (ambos distinguen
  blancos globales runtime reales de objetivo_usd inexistente).
- `02c-SIM-STACK-SELECTOR.md:209-211` (§7): **"objetivo_usd (RULE 00) — GAP en los 4
  crates"** (sim/selector). CONSISTENTE con 02a §9 en lo angosto; sin contradicción
  abierta PERO tampoco fue nombrado.
- Timestamps observados (ls del dir): 02d 01:02 · 02b 01:06 · 02c 01:06 · esta
  verificación ~01:07 (observación del cross-exam a las 01:13; el mtime actual del
  archivo ya no lo conserva porque las erratas §8/append-only lo reescribieron).
  **Falla de proceso**: 02b/02c estaban publicados ANTES de cerrar el dictamen §7 y §0
  no los leyó ni citó. Mínimo exigible que se incumplió: nota de concurrencia o lectura
  post-escritura antes del veredicto.

### 10.3 Reconciliación canónica de la mesa (lo que los gates 1-8 deben consumir)

Los cuatro reportes son COMPATIBLES una vez acotados sus dominios — la contradicción era
aparente, producto de que esta verificación no contrastó a sus pares:

| Par | Claim | Acotación de dominio | Compatible con |
|---|---|---|---|
| 02a §9 (+ FIX-R1 §9.1) | GAP objetivo_usd **por estrategia** en searcher-rs; piso absoluto `min_profit_usd` VIVO (scanner.rs:2568); 4 knobs USD workbook declared-only | searcher-rs | 02d §5, 02b G1 |
| 02b :154 (G1) | GAP objetivo_usd **por operador**; único flujo USD = RankingInput OBSERVADO (no objetivo) | operadores/math-engine | 02a §9, 02c §7 |
| 02c :209-211 (§7) | GAP objetivo_usd en los 4 crates sim/selector; solo gates (min_accept_score, SIM_MIN_PROFIT_WEI 0) | sim stack/selector | 02b G1, 02a §9 |
| 02d :212 + §5:245-258 | objetivo_usd **runtime GLOBAL existe** (capital_usd :183 cap 2 %, max_value_eth 1.0 ETH, min_profit_usd floor) | terminus/shared-rs | 02a §9 re-stringido, FIX-R1 §9.1 |

Síntesis: **no existe objetivo_usd por estrategia ni por operador (GAP en 02a/02b/02c),
pero SÍ existen blancos/caps USD runtime globales del operador (02d §5), y un piso USD
absoluto vivo en el hot-path del scanner** (FIX-R1 §9.1 de `WO-02a-DESIGN.md`, que ya
reconcilió esto en el lado del diseñador — esta errata cierra el lado del verificador,
que era la mitad faltante).

### 10.4 Pendiente declarado (fuera del charter de este fix)

- El **G2 de la TABLA del cross-exam** (premisa falsa "TODO engines/ tras
  experimental-gate", §3 líneas 54-55 de este archivo): al escribir esta errata estaba
  SIN corregir y se declaró pendiente; **RESUELTO en curso por el par concurrente**
  (`WO-02a-FIX-G3`, sección :403 de este archivo — corrige la premisa con inventario
  exhaustivo: 7 engines ungated, 7 gated :38-52, el único valor ungated es
  `#[cfg(test)]`). Verificado por lectura propia: `engines/mod.rs:25-28` compila
  `dex_engine`, `flashloan_engine`, `liquidation_engine`, `triangular_engine` SIN gate
  (+3 "Task 3" :31-33), y son **7** (no 8) los módulos gated :38-52 — el miscount "8"
  era del propio cross-exam y el par G3 también lo corrigió. La CONCLUSIÓN de valores
  sobrevive (el 0.01 está en `#[cfg(test)]`, triangular_engine.rs:871 dentro de :843-845).
- Contaminación residual conocida y NO tocada aquí (archivo de otro owner):
  `WO-02a-verify-CROSS-EXAM.md:87` cita "02b (:148, G1)" — el drift :148→:154 queda
  corregido solo en esta errata (§10.2).

### 10.5 Verificación del fix (re-ejecutada tras escribir)

- Marcador presente: grep `WO-G2 (2026-09-17)` en este archivo = 2 hits (cabecera
  del §10 + esta auto-referencia — cuenta exacta, no maquillada).
- Citas re-abiertas post-escritura: trading_config.rs:183/:249/:260/:622,
  execution_admission.rs:193-201, config.rs:156-158, scanner.rs:2568,
  engines/mod.rs:25-28/:38+, 02d:212/:245-258, 02b:154 (y :148 para el drift),
  02c:209-211 — todas byte-exact contra disco.
- Append-only verificado: §0-§9 intactos (sin ediciones in-place); diff limitado a este
  apéndice. `git status`: sin cambios en `backend/` ni en src/ (markdown-only).
- Regla de proceso adoptada para la mesa desde este fix: **contra-citar pares por
  archivo:línea antes de declarar "sin contradicciones"** — una declaración de
  no-contradicción sin citas nombradas por archivo:línea se considera NO EMITIDA.

---

## // WO-02a-FIX-G3 (2026-09-17) — ERRATA DE PREMISA §3 + FUENTE DE REQUISITO §7 (append-only)

> Fixer gang ronda 1, charter G3 (CROSS-EXAM R3+G2+R4). Corrige DOS defectos de ESTE
> archivo que las erratas numéricas previas (§8 / E-a..E-c) NO cubrieron. Append-only:
> nada de §1-§7 ni de las erratas previas fue modificado. Reglas duras: 0 git, 0 cargo
> (board WO-02 los prohíbe), 0 VPS, 0 HTTP, 0 mocks. Clasificación: CANONICAL_REPO
> (toda cita re-abierta con Read/grep/wc antes de escribir).

### G3-1 · §3 de ESTE archivo: premisa "TODO el árbol engines/ está tras experimental-gate" es FALSA

Texto original (:53-56): *"los únicos min_profit_usd con valores... viven en `src/engines/*`
y TODO ese árbol está tras `#[cfg(feature = "experimental-engines")]` (engines/mod.rs:38-51)"*.

**Corrección de premisa (verificada leyendo engines/mod.rs completo):**
- `engines/mod.rs:25-28` compila SIN gate los 4 engines estables: `dex_engine`,
  `flashloan_engine`, `liquidation_engine`, `triangular_engine` (doc del propio módulo
  :37: "The production orchestrator uses the four stable engines above").
- `engines/mod.rs:31-33` compila SIN gate 3 engines adicionales ("New engines for Task 3"):
  `cross_chain_bridge_engine`, `liquidation_snipe_engine`, `spanning_tree_engine`.
- SOLO `:38-52` (7 módulos gated: backrun :39, cex_dex :41, spatial :43, dlp :46,
  funding_rate :48, svs :50, triangular_atomic :52 — cada `pub mod` precedido de su
  `#[cfg]` en la línea inmediata anterior) está tras
  `#[cfg(feature = "experimental-engines")]`. La cita original ":38-51" corrige a :38-52.
  Nota adicional: el cross-exam (§2 R1.1) dijo "los 8 motores de :38-52" — miscount
  menor del propio cross-examiner (son 7; los 7 `#[cfg]` ocupan :38-52). Corregido aquí
  para no propagar el error.

**La CONCLUSIÓN de valores SOBREVIVE (inventario exhaustivo, 22 matches grep
`min_profit_usd` en src/engines/):**
- 21 matches con valores por defecto (5.0/15.0/6.0/12.0/10.0/8.0/7.0) viven TODOS en los
  7 engines gated: backrun_engine.rs:40, cex_dex_engine.rs:37, dlp_engine.rs:75,
  funding_rate_engine.rs:45, spatial_engine.rs:37, svs_engine.rs:39,
  triangular_atomic_engine.rs:52 (+ sus campos/usos `:29/:66/...`).
- El único match NO-gated es `triangular_engine.rs:871` (`min_profit_usd: 0.01`) — dentro
  de `#[cfg(test)] mod tests` (bloque abre en :843-845, verificado por lectura directa).
  NO se compila en build de producción.
- Los 7 módulos ungated (dex/flashloan/liquidation/triangular + 3 "Task 3") tienen
  **CERO** matches de `min_profit_usd` (grep por archivo: 0/0/0/0/0/0/0).

Dictamen corregido: el PASS de RULE 00 §3 se mantiene, pero la justificación correcta NO
es "todo el árbol está gated" sino "cero `min_profit_usd` productivo fuera de los engines
gated; el único ungated con valor es código `#[cfg(test)]`". Nota: la refutación más
profunda del §3 (piso USD absoluto `min_profit_usd` del scanner hot-path, scanner.rs:2568)
fue registrada por el cross-examiner como G1 — NO es parte de esta errata G3.

### G3-2 · §7 de ESTE archivo: "15 fichas (≥8 exigidos)" — requisito SIN FUENTE

Texto original (:118): *"Fichas muestreadas: 15 (≥8 exigidos)"*. Verificación de fuente
(grep `exigido|≥8` en todo audits/first-understand-20260917 + lectura completa del board
GOAL-WORKORDERS.md): NINGÚN charter visible ni regla del board fija un mínimo de 8 fichas
por muestreo. El "≥8 exigidos" es un tamaño de muestra AUTOIMPUESTO por el verificador
presentado como requisito externo — afirmación de requisito sin fuente (práctica R8:
declarar lo medido, no adjuntarle autoridad inexistente).

**Corrección**: leer "15 fichas muestreadas (muestreo autoimpuesto; sin mínimo exigido
por board/charter visible)". El PASS 15/15 en sustancia NO cambia — más muestras sigue
siendo mejor y ninguna ficha depende del número 8.

**Contaminación residual de la misma fórmula (archivos de otros owners — NO tocados,
documentados para sus fixers)**: `02c-SIM-STACK-SELECTOR.md:241` ("requisito ≥8
cumplido") y `:363` ("req ≥8"), `WO-02c-verify-VERIFY.md:17` ("requisito ≥8 → ejecutado
18"). Misma falta de fuente; mismos archivos pertenecen a la mitad WO-02c.

### G3-3 · Componente numérico del charter: YA CORREGIDO por pares, re-verificado aquí

El desliz aritmético de la Corrección A ("~123.7K LOC .rs" = 184 entradas con fixtures)
fue corregido por DOS fixers previos (§8 y E-a arriba + bullets del board). Verificación
independiente de este fixer (2026-09-17, Git Bash desde backend/searcher-rs):
`find src -type f -name '*.rs' | wc -l` → **178**; `find src -type f -name '*.rs'
-print0 | xargs -0 cat | wc -l` → **90,401**. Consistente con lo publicado. Nada más
que corregir en este punto — confirmado sin acción adicional.

### Verificación del fix

- Marcador: grep `WO-02a-FIX-G3` en este archivo = esta sección (única).
- Append puro: §1-§7, §8 y E-a..E-c intactos (diff de estructura verificado pre/post).
- Cero toques a src/, cero git, cero builds (restricción board WO-02 cumplida).

---

## // WO-02a-FIX-G1 (2026-09-17) — CIERRE DE G1 + PRECISIÓN DEL MECANISMO DEL PISO USD (append-only)

> Fixer gang ronda 1, charter G1 de la TABLA del cross-exam (WO-02a-verify-CROSS-EXAM.md
> §4-G1): corregir la conclusión "umbrales solo relativos" reconociendo el piso USD
> absoluto + blancos canónicos, con adendum en WO-02a-DESIGN.md y ESTE archivo; NO tocar
> el GAP por-estrategia. Reglas duras: 0 commit/push/PR/deploy, 0 cargo/build (board
> WO-02 los prohíbe), 0 VPS, 0 HTTP, 0 mocks. Git READ-ONLY únicamente como evidencia
> (mismo precedente que ERRATA-PROC/FIX-R2). Clasificación: CANONICAL_REPO — toda cita
> re-abierta ANTES de escribir.

### G1-A · Estado de G1 al llegar este fixer: YA ATERRIZADO por pares concurrentes

G1 estaba resuelto en sus dos mitades por fixers paralelos:
- Lado DESIGN: `WO-02a-DESIGN.md §9.1 ADENDUM (WO-02a-FIX-R1)` — framing corregido
  (umbrales relativos vivos + piso USD absoluto `trading_config.min_profit_usd` + 4 knobs
  workbook declared-only), citando scanner.rs:2568 y trading_config.rs:183/:260/:622/:166/:249.
- Lado VERIFY: `§10 // WO-G2` de ESTE archivo (10.1 "Corrección del dictamen" + 10.3 tabla
  de reconciliación 02a/02b/02c/02d).

Este fixer NO duplica esas secciones: re-verificó sus citas (todas byte-exact contra
disco) y declara **G1 CERRADO en lo sustantivo**. Lo que sigue es una PRECISIÓN que
ningún par verificó y que corrige la cita que el propio cross-exam sembró.

### G1-B · PRECISIÓN MATERIAL — scanner.rs:2568 NO es el gate: `min_profit_threshold` se puebla pero `score()` jamás lo lee

El cross-exam (R1.2) y las erratas heredadas (§10.1-3 de ESTE archivo, §9.1-C del DESIGN)
citan scanner.rs:2568 (`min_profit_threshold: cfg.min_profit_usd` → PrioritizationEngine)
como evidencia del piso absoluto "vivo" del scanner. Verificación propia:

1. `prioritization-spine/src/scoring.rs:13-14` declara `min_profit_threshold: f64`;
   `score()` (:15-46) NO lo consulta en ninguna rama — su ÚNICO gate es
   `net_expected <= 0.0 → NegativeProfit` (:25-26). Grep `min_profit_threshold` en
   backend/ completo (--include=*.rs, sin target/): EXACTAMENTE 2 hits = la declaración
   (scoring.rs:14) y la población (scanner.rs:2568). **Cero lectores**: wiring
   populated-but-unconsumed (mismo patrón declarativo que `beam_k` / knobs USD del
   FIX-R1 §9.1-A).
2. **El piso absoluto SÍ está vivo en el MISMO hot-path, por otro cable**:
   `decode_and_score_tx` (scanner.rs:1488) construye `ConfigAwareEvaluator::with_cache`
   (scanner.rs:2125) → `policy_from_config` (config_aware.rs:148-151:
   `RiskPolicy.min_net_profit_usd = cfg.min_profit_usd`) invocada en :1053 →
   `validate_opportunity_risk` (config_aware.rs:1085, import :33-34) →
   **math-engine/src/risk_engine.rs:47-48** (`profile.net_profit_usd <=
   policy.min_net_profit_usd → NegativeNetProfit`). Y el override por estrategia:
   `strategy_config_gate.rs:302-308` (`GateOutcome::Reject(StrategyConfigBelowMinProfit)`)
   consumido en scanner.rs:2312-2319 con persistencia fail-honest (`rejection_reason`
   poblado — patrón R8). `cfg` proviene de `trading_config.state(chain_id)`
   (scanner.rs:1637) — superficie operador PG.
3. **Corrección de cita para la mesa**: quien consuma la conclusión G1 (gates 1-8, WO-06,
   WO-04) debe citar el piso como `config_aware.rs:150 → risk_engine.rs:47 (+
   strategy_config_gate.rs:302 → scanner.rs:2312)`, NO como scanner.rs:2568 — ese punto
   es población de un campo que `score()` no lee. La conclusión corregida de board
   (§10.3 de ESTE archivo; §9.1 del DESIGN) NO cambia: el piso USD absoluto
   operador-configurable está VIVO; solo cambia el file:line del mecanismo que lo exige.

Blancos canónicos re-verificados (parte del charter G1): `simulation_capital_usd`
(trading_config.rs:166), `simulation_target_profit_usd` (:249, doc :242-248
"Simulation UI filter — ... backend STORES this value but does NOT gate on it (R8
fail-honest)"), `min_profit_usd` (:260, gate real vía risk_engine.rs:47) y
`effective_min_profit_usd(strategy)` (:622, override por estrategia consumido por
strategy_config_gate.rs:302).

### G1-C · BONUS (cierra pendiente ERRATA-2 del board): §7 de ESTE archivo atribuye el diff de route_intent.rs a "dcfe890c"

§7 (:127-129) dice: *"route_intent.rs solo contiene el diff PREEXISTE del operador
dcfe890c"*. Refutado por el board (ERRATA-PROC-WO-02a) y re-verificado ahora con git
read-only: `git show dcfe890c --stat` toca SOLO `backend/shared-rs/src/chains.rs` +
`backend/sim-ctl/src/tx_builder.rs` (105 insertions, 2 files); `git log --
backend/searcher-rs/src/route_intent.rs` último commit = `a37bcbfc`; `git status
--porcelain -- backend/searcher-rs/src/route_intent.rs` = ` M` (working-tree, NO
commiteado). Lectura canónica vigente: **diff huérfano de working-tree, autoría SIN
confirmar (hipótesis: operador follow-up PANCAKE-ROUTER-01)** — ver ERRATA-PROC en
WO-02a-DESIGN.md y entrada correspondiente del board. La conclusión de §7 ("D-1..D-3 NO
aplicados a código") SOBREVIVE intacta. Pendiente restante de ERRATA-2, fuera de este
charter y de su owner: corrección del doc-comment impact_index.rs:21-22 (recomendación
E-1, gated operador).

### Verificación del fix

- Marcadores: grep `WO-02a-FIX-G1` en ESTE archivo = esta sección (única); en
  WO-02a-DESIGN.md = 1 hit (§9.2, espejo). Reporte del fixer: WO-02a-FIX-G1-20260917.md.
- Append-only: §0-§10 y `// WO-02a-FIX-G3` intactos (sin ediciones in-place).
- Citas re-abiertas post-escritura, byte-exact contra disco (2026-09-17): scoring.rs:13-14/
  :25-26, scanner.rs:1488/:1637/:2125/:2312/:2568, config_aware.rs:33-34/:148-151/:1053/:1085,
  risk_engine.rs:47-48, strategy_config_gate.rs:302-308, trading_config.rs:166/:242-249/:260/:622,
  engines/mod.rs:25-28/:38-52, git show/status/log de dcfe890c + route_intent.rs.
- **GAP objetivo_usd POR ESTRATEGIA en searcher-rs: NO tocado — SOBREVIVE**
  (re-confirmado: ningún `objetivo_usd`/`target_usd` productivo; lo más cercano sigue
  siendo `simulation_target_profit_usd`, trading_config.rs:249, stores-not-gates R8).
