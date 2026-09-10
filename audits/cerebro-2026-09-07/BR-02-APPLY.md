# BR-02 — APPLY: matar la carrera de reserves (backfill-at-discovery + coherencia de frescura)

- **WO**: BR-02 · kind: **apply** · **Agente**: ecc:rust-reviewer (BR-02, Gang Omniscience, IA OMEGA)
- **Fecha**: 2026-09-07/08 · **NO-GIT** (0 commits/push/PR/deploy) · **VPS read-only** (2 SSH de medición, 0 mutación).
- **Charter**: "Matar la carrera de reserves — el asesino #1 del multihop" (GOAL-WORKORDERS.md BR-02:
  missing_reserves=79% del sink, 196.927/h; cobertura 114/235 pools, TTL ~30s, carrera por
  legs-frescas-simultaneas). TTL/backfill coherentes con el tick de evaluación; grafo y evaluación
  ven LA MISMA realidad (ROUTES_CROWN_JEWEL); fail-honest R8 (None, jamás 0). RULE 00 (cero hardcode:
  knobs declarativos `ARBX_KNOB_*`).
- **Lexicon**: Variedad de Liquidez (pool) · Topological Yield · Decoherencia de Estado.

## 0. Veredicto: **APPLIED + VERIFICADO local (check/clippy/fmt/tests)** — deploy = orquestador

| Entregable | Estado | Evidencia |
|---|---|---|
| Backfill-at-discovery V2 (reserves reales a Redis en t=0) | ✅ | pool_discovery.rs §3.1 (`// BR-02 (2026-09-07)` ×6) |
| Backfill-at-discovery V3 (slot0+liquidity que se DESCARTABAN) | ✅ | pool_discovery.rs §3.2 |
| R8: parse-failure ≠ (0,0) fabricado | ✅ | pool_discovery.rs §3.3 (remueve `unwrap_or_default()`) |
| Knobs canónicos de coherencia (2 nuevos, ARBX_KNOB_*) | ✅ | canonical_knobs.rs §3.4 (53→55 knobs, snapshot 56) |
| Grafo lee el MISMO presupuesto de frescura que la evaluación | ✅ | route_scanner_worker.rs §3.5 (max_age_secs del knob; era default 120) |
| Doc TTL STALE corregido (tick 5s→12s real) | ✅ | reserves.rs §3.6 |
| cargo check / clippy / fmt / tests | ✅ | §5 (fmt: territorio propio limpio; diffs residuales SOLO en runtime_knobs.rs = claim ajeno CB-02, intocado) |

## 1. Forense que definió el diseño (leído ANTES de diseñar, como exigía el charter)

### 1.1 La cifra del charter contra la realidad del VPS (read-only, 2026-09-07)

`ssh arbx` medición directa: `arbx:pool_reserves:1:*` = **114 keys** · `arbx:v3_slot0:1:*` = **119** ·
PG universo activo = **235** (V2 114 + V3 121). Ticks del writer: cadencia ~12s, V2 **114/114 ok**,
V3 119/121 ok (2 fails persistentes). **Steady-state = 233/235 (99.1%) del working set cubierto.**

⇒ El "114/235" del charter eran las 114 keys **V2** sobre el universo TOTAL (incl. 121 V3).
pool_sync SÍ cubre su working set completo cada tick. **La carrera no es coverage del working set:
es la ventana discovery→reload (~12 min) y las dos capas que leen realidades distintas.** El 79% del
sink multihop es la muerte por *ausencia en el instante correcto*, no por ausencia absoluta.

### 1.2 Las cuatro causas raíz (file:line, SHA de trabajo = árbol feat/hops-live-01 @ e65040f1+)

1. **Carrera discovery→Redis (la del título).** `pool_discovery.rs` `hydrate_and_persist_pool`:
   V2 hacía getReserves ON-CHAIN REAL y lo insertaba SOLO en el cache in-proc (línea ~553 pre-edit);
   Redis NUNCA. El pool entra al ImpactIndex ya, pero el grafo (graph_builder lee Redis) no ve sus
   reserves hasta el siguiente reload de pool_sync: `reload_every=60 ticks × 12s = ~12 min` de
   aristas muertas como `missing_reserves`.
2. **V3 peor: descarte directo.** La misma función, rama V3 (~563-599): fetchea slot0+liquidity
   reales y los **descarta** ("we just fetched it successfully to prove it exists"). Un pool V3
   recién descubierto no existía en NINGUNA capa por ~12 min.
3. **Dos capas, dos realidades (ROUTES_CROWN_JEWEL).** El in-proc `ReservesCache`
   (triangular_engine.rs:84-100) guarda solo `(U256,U256)` — SIN ts/blk — y
   `hydrate_from_redis` (:119-223) es **merge-upsert sin evicción** (:212 `self.insert`). Cuando la
   key Redis expira (TTL 30s = 2 ticks fallidos), el grafo ve `missing_reserves` pero la evaluación
   (SizeOptimizer vía state_projector) conserva los reserves PARA SIEMPRE. Expiración Redis ≠
   expiración in-proc: las capas divergen por construcción. NOTA: triangular_engine.rs NO está bajo
   claim BR-02 — divergencia residual documentada en §6.
4. **Incoherencia TTL vs presupuestos.** TTL writer 30s (`pool_sync_worker.rs` consts
   `RESERVES_TTL_SECS`/`V3_SLOT0_TTL_SECS`) < presupuesto evaluación (triangular_adapter
   `MAX_RESERVE_LAG_BLOCKS=5` ≈ 60s) < graph_builder `max_age_secs` default 120 — el TTL es la
   restricción vinculante en todas partes: las keys EXPIRAN antes de poder ser legalmente "stale";
   `stale_reserves` era inalcanzable y `missing_reserves` absorbía todo.
5. **Artefacto de medición had_reserves (fuera de claim, documentado).** `cartridge/runner.rs:427`
   `read_pool_reserves` lee SOLO la key V2 `arbx:pool_reserves:*` → toda primera pierna V3 reporta
   `had_reserves=f`. RDO 2h (query read-only): las 14 familias × 6,277 filas = **100% f, cero t**
   (uniforme por familia = misma cohorte de escaneo). El 56.5% de BR-01 es en parte este artefacto.

## 2. Diseño aplicado (una sola definición de "fresco" para las dos capas)

> Doctrina citada (docs/ROUTES_CROWN_JEWEL_DOCTRINE.md §Reglas 1 y 6, textuales): "**Dos capas**:
> DISCOVERY (enumerar topología) ≠ EVALUATION (gates + sizing + EV). Nunca mezclar." / "**El grafo
> es dinámico**: reservas via `ReservesCache`/`ImpactIndex` sync (ref. 16)". BR-02 NO mezcla las
> capas: hace que ambas LEAN el mismo hecho Redis con el mismo presupuesto declarado de frescura.

**Principio:** no perseguir el writer — publicar en el ORIGEN. La discovery YA observó reserves
reales on-chain: escribirlas a Redis en t=0 con el MISMO TTL que las capas declaran como "frescura
legal". Una variable (`reserves_freshness_budget_s`) define cuándo algo es stale para grafo,
sizing y backfill.

```
discovery (getReserves/slot0 REALES on-chain)
   │  t=0  ── BR-02 backfill ──▶ Redis arbx:pool_reserves / arbx:v3_slot0   (TTL = budget 60s)
   │                                   ▲                              ▲
   │  t≈0-12s  pool_sync tick ─────────┘ (toma el relevo tras reload;   │
   │                                     TTL propio 30s, writer no claim)│
   ▼                                                                     │
grafo: GraphBuildConfig.max_age_secs = budget (60) ◀── MISMO knob ──▶ backfill TTL
```

- **Default 60s** = 5 bloques × cadencia 12s = el lag canónico de la capa evaluación
  (MAX_RESERVE_LAG_BLOCKS=5): grafo y sizing declaran staleness a la MISMA edad.
- Un pool activamente descubierto (event-driven) se re-backfilla con cada intent; sin intents, la
  key decae honestamente (R8: ausencia declarada, jamás fabricada).
- **Cero hardcode RULE 00**: valores vienen de knobs `ARBX_KNOB_RESERVES_FRESHNESS_BUDGET_S` /
  `ARBX_KNOB_RESERVES_BACKFILL_ON_DISCOVERY`; direcciones/valores on-chain REALES del RPC.

## 3. Diffs aplicados (marcadores `// BR-02 (2026-09-07)`; ver `git diff` en el árbol)

**Anclas file:line (post-edit):** struct+knobs pool_discovery.rs:66-71/97-110 · V2 backfill
:545-641 (event `discovery.reserves_backfilled` :619, R8 parse :570/:637) · V3 backfill :678-716
(event `discovery.v3_slot0_backfilled` :708) · knobs canonical_knobs.rs:112-135 (doc), :202-203
(defaults), :325-332 (env), :466-482 (validate), :602-609 (to_json) · grafo wiring
route_scanner_worker.rs:252-260 (:258) · doc reserves.rs:12-22.

### 3.1 pool_discovery.rs — V2 backfill (rama V2 de `hydrate_and_persist_pool`)
El sobre `with_retry` ahora también trae el **head block number real** (`provider.get_block_number()`
en el mismo envelope — sin hop extra) para `ReservesEntry.blk`. Tras el insert in-proc, publica a
Redis `set_reserves(chain, pool_lower, ReservesEntry{r0, r1, token0_addr: Some(token0 real), blk,
ts: now}, ttl = reserves_freshness_budget_s)`. Telemetría: `discovery.reserves_backfilled` /
`discovery.reserves_backfill_failed` (non-fatal: el pool sigue persistido + in-proc).

### 3.2 pool_discovery.rs — V3 backfill (rama V3)
Los valores reales fetcheados (sqrtPriceX96, liquidity) dejan de descartarse: se publican como
`V3Slot0Entry{sqrt_price_x96, liquidity, ts: now}` vía `set_v3_slot0(...)` — la key que
graph_builder.rs:423 lee para las aristas V3. Telemetría `discovery.v3_slot0_backfilled(_failed)`.

### 3.3 pool_discovery.rs — R8 en el parse
`unwrap_or_default()` (fabricaba (0,0) si el parse decimal fallaba) → `match (r0, r1)`: fallo =
warn `discovery.reserves_parse_failed` + skip del insert Y del backfill (miss declarado, jamás 0).

### 3.4 canonical_knobs.rs — 2 knobs de coherencia (53→55 knobs)
- `reserves_freshness_budget_s: u64 = 60` — env `ARBX_KNOB_RESERVES_FRESHNESS_BUDGET_S`.
  Doc in-file explica el anclaje (5 bloques × 12s) y el gap residual del writer (§6).
- `reserves_backfill_on_discovery: bool = true` — env
  `ARBX_KNOB_RESERVES_BACKFILL_ON_DISCOVERY`; OFF = comportamiento pre-BR-02 (rollback quirúrgico
  del operador, sin deploy).
- `validate()`: budget ∈ 1..=600 **y** budget ≥ `block_cadence_s` (un presupuesto más corto que una
  cadencia declara stale toda observación al escribirla).
- `to_json()`: +2 entries (con `source` = 56); tests actualizados (defaults, rejects, env-serial,
  snapshot 54→56 — los casos env van en la ÚNICA fn serial existente: `set_var` es process-global).
- `PoolDiscoveryService::new()` resuelve los knobs UNA vez al construirse (boot env).

### 3.5 route_scanner_worker.rs — el grafo lee el knob
`ScannerConfig::from_env` construye `GraphBuildConfig { max_age_secs:
knobs.reserves_freshness_budget_s, ..Default::default() }` (antes: default 120 hardcode). El
`stale_reserves` del grafo vuelve a ser alcanzable y DECLARADO a la misma edad que la evaluación.

### 3.6 reserves.rs — doc corregido
Header TTL: "re-set every 5s" era FALSO (tick real 12s, `POOL_SYNC_INTERVAL_MS`,
workers/mod.rs:49); ahora documenta tick real, la expiración por 2 ticks fallidos, y las keys de
backfill BR-02 (ambas familias de keys).

**Sin cambios** (quirúrgico): pool_candidate.rs, pool_sources/ (los 6 archivos), y TODOS los
archivos fuera de claim (scanner.rs, hot_path_emitter.rs, pool_sync_worker.rs,
triangular_engine.rs, runner.rs, state_projector.rs, graph_builder.rs, lib.rs/main.rs/
runtime_knobs.rs de CB-02 — preservados byte a byte, `git diff` vacío para los primeros dos).

## 4. Medición antes/después (charter: "ssh read-only conteo pools-con-reserves-frescas/total")

- **ANTES (VPS, read-only, 2026-09-07 ~00:46Z)**: keys V2 114 / universo 235 (charter); steady-state
  del working set 233/235 (99.1%); **ventana discovery→Redis ~12 min** (0% durante la ventana);
  aristas de un pool recién descubierto: `missing_reserves` 100% de la ventana.
- **DESPUÉS (esperado post-deploy — NO deployado, NO-GIT)**: para todo pool recién descubierto,
  reserves/slot0 en Redis en t=0 con TTL 60s ⇒ cobertura durante la ventana de reload pasa de
  **0% → 100% mientras el par siga produciendo intents** (re-backfill por evento); pools sin
  actividad decaen honestamente a ausencia (R8). La verificación local exigida por el charter
  (tests de TTL/coherencia) está en §5; el "después" en VPS requiere deploy del orquestador +
  re-medición del conteo fresh/total (comandos en §7).

## 5. Verificación local (árbol principal, target caliente; diffs ajenos preservados)

- `cargo check -p searcher-rs` → **PASS** (49.8s).
- `cargo clippy -p searcher-rs --all-targets` → **PASS, 0 warnings** (2m04s).
- `cargo fmt -p searcher-rs -- --check` → territorio BR-02 **LIMPIO**; diffs residuales SOLO en
  `runtime_knobs.rs` (claim ajeno CB-02, pre-existentes, INTOCADOS por BR-02).
- `cargo test -p searcher-rs --lib` → **1158 passed / 0 failed / 3 ignored** (§5.1).

### 5.1 Tests (resultado FINAL — todo verde)

- `cargo test -p searcher-rs --lib` (suite COMPLETA) → **1158 passed / 0 failed / 3 ignored**
  (1.25s de ejecución; incluye los 12 de canonical_knobs con los 2 knobs nuevos:
  `snapshot_json_has_all_55_knobs_and_source`, `env_overrides_win_over_workbook_defaults` con los
  overrides 90/off, `defaults_match_workbook_01_config_exactly`,
  `validate_rejects_invariant_violations` con los 3 rejects de coherencia, familia freshness
  íntegra, y todos los tests pre-existentes de graph_builder/route_scanner/pool_discovery).
- `cargo test -p searcher-rs --test cartridge_simulate_swap_test --no-run` (árbol estable) →
  **PASS en 16m21s** — prueba directa de que los 43 errores de la primera corrida full fueron
  la escritura concurrente del agente hermano BR-00 (territorio cartridge/): con el árbol
  estable + los diffs BR-02 presentes, TODO compila.
- Primera corrida full (`cargo test -p searcher-rs` sin filtro) NO compiló por esos 43 errores
  transitorios en `tests/cartridge_simulate_swap_test.rs` ("use of undeclared type `String`" en
  un archivo coherente a HEAD — lectura truncada mid-write; `git status` de tests/ y cartridge/
  LIMPIO antes y después; NINGÚN archivo BR-02 aparece en los errores).
- NOTA de método: esa primera corrida fue `... 2>&1 | tail -15` — el exit code quedó enmascarado
  por `tail` (reportó 0). Corregido: verificación por contenido + corridas sin máscara.

## 6. Gaps residuales y handoff a pares (fuera de claim BR-02 — NO editados)

1. **Writer TTL (pool_sync_worker.rs, no claim)**: consts 30s. Con budget 60s, el reader puede
   declarar fresco lo que el writer ya expiró (missing por expiración antes de stale por edad).
   Peer sugerido: `RESERVES_TTL_SECS`/`V3_SLOT0_TTL_SECS` leídos de
   `ARBX_KNOB_RESERVES_FRESHNESS_BUDGET_S` (el knob YA existe y valida; 1-línea por const).
2. **In-proc ReservesCache sin ts/blk + merge sin evicción (triangular_engine.rs, no claim)**:
   la evaluación puede valuar contra reserves expiradas hace minutos. Peer sugerido: guardar
   `(U256,U256,ts)` + evicción en `hydrate_from_redis` (las entries Redis YA llevan ts).
3. **had_reserves V2-only (cartridge/runner.rs:427, no claim)**: artefacto de medición — toda
   primera pierna V3 = f. Peer sugerido (BR-01/BR-10): distinguir protocolo en
   `read_pool_reserves` o etiquetar `had_reserves_v2_key` para no comparar peras con manzanas.
4. **Universe V3 parcial**: 121/121 activos, 2 slot0 fails persistentes (pools V3 sin slot0
   cacheable por el writer) — outside claim; visible en ticks del writer.

## 7. Gate de re-verificación post-deploy (re-ejecutable, read-only)

```bash
# cobertura fresh/total (budget 60s = ts dentro de los últimos 60s)
ssh arbx "docker exec arbitragex-v2-redis-1 sh -c 'redis-cli --raw --scan --pattern \"arbx:pool_reserves:1:*\" | wc -l; redis-cli --raw --scan --pattern \"arbx:v3_slot0:1:*\" | wc -l'"
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -c \"SELECT count(*) FROM pools WHERE is_active\""
# post-deploy: los logs deben mostrar discovery.reserves_backfilled / discovery.v3_slot0_backfilled
ssh arbx "docker logs arbitragex-v2-searcher-rs-1 --since 30m 2>&1 | grep -c discovery.reserves_backfilled"
# knobs vivos (snapshot debe traer las 2 keys nuevas)
curl -s http://127.0.0.1:8080/api/v1/config/canonical-knobs | jq '.reserves_freshness_budget_s, .reserves_backfill_on_discovery'
```

## 8. Reglas duras cumplidas

- **RULE 00**: cero mocks/hardcodes — reserves vienen del RPC real en el instante; knobs
  declarativos; None≠0 preservado y ENDURECIDO (el unwrap_or_default() era un (0,0) latente).
- **§32/§33**: 0 executor/wallets/capital/firma/broadcast; VPS solo lectura (2 SSH de medición).
- **§34.3**: relays-client intacto (sin diff); nada roza el terminus. **§34.1**: la matemática no
  cambia por modo — el backfill publica el mismo hecho on-chain en todos los modos.
- **NO-GIT**: 0 commit/push/PR/deploy. Diffs viven en el árbol de trabajo.
- **Diffs marcados** `// BR-02 (2026-09-07)` (18 en 4 archivos) · diffs ajenos preservados
  (scanner.rs/hot_path_emitter.rs sin diff; lib.rs/main.rs CB-02 intactos; runtime_knobs.rs
  intocado pese a fmt-dirty).
- **Presupuesto dominio público**: 0/5 requests HTTP (toda la evidencia: árbol local + SSH read-only).
