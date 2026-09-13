# CB-02 (Rust apply) — v2: APLICADO contra CB-02-DISENO.md §3

- **WO**: CB-02 · kind: apply (mitad Rust del control plane runtime)
- **Agente**: rust-topology-engineer (Gang Omniscience, re-despacho del apply)
- **Fecha**: 2026-09-07
- **Estado**: **APPLIED** — wiring íntegro del diseño §3.1/§3.2/§3.3, verificación 3/3 verde
  (`cargo check` + `clippy -D warnings` + `fmt --check`, `-p searcher-rs`). NO-GIT (cero
  commit/push/PR/deploy). Branch verificada antes de cada edición: `feat/hops-live-01` (§36).
- **Spec**: `audits/control-board-2026-09-07/CB-02-DISENO.md` §3 (D3 — sitios de wiring
  file:line). Este reporte la cita como "el diseño".

---

## 0. Historia v1 → v2 (qué conserva este documento del v1)

La v1 (NO-OP válido) se entregó cuando el diseño no existía; su valor forense se conserva:

1. **Forense v1 §2.3 (ABIERTA — acción sugerida al orquestador):** el diff sin commitear de
   `backend/searcher-rs/src/hot_path_emitter.rs` presente al inicio de aquella sesión ya no
   estaba en el árbol tras un switch de branch ajeno (ningún commit alcanzable lo toca, sin
   stash nuevo, working tree limpio). Yo NO cambié de branch en esta sesión (verificada antes
   de cada edición) — la pérdida NO se repitió. Es exactamente el drift que CB-04 detectará.
2. **Mapa v1 §3** (kill-switch = único poll runtime real; canonical_knobs = publish boot):
   superseded por el diseño §1.3/§2.1, que lo adopta y decide (solo route_scanner run-gate
   promueve a A).
3. v1 §5 (despachar diseño antes del apply): CUMPLIDA — CB-02-DISENO.md aterrizó (con su
   reconciliación §15 contra el apply TS paralelo) y este v2 es el re-despacho contra él.

## 1. Estado del árbol al empezar (fail-honest — trabajo preexistente encontrado)

El charter anticipaba edición desde cero; el árbol ya contenía PARTE de la mitad CB-02 sin
commitear, más diffs de programas PARALELOS. Forense completa antes de tocar (solo aditivo,
todo lo preexistente preservado verbatim):

| Diff preexistente sin commitear | Origen | Trato en esta sesión |
|---|---|---|
| `runtime_knobs.rs` completo (421 líneas, untracked, marcado `// CB-02 (2026-09-07)`) | Mitad CB-02 previa (dispatch caído o mitad A del respawn) | **Verificado contra el diseño sección por sección (§2.1 abajo) y adoptado como base** — cero reescritura |
| `main.rs:173-175` `mod runtime_knobs;` + `lib.rs:97-100` `pub mod runtime_knobs;` (CB-02) | Ídem | Adoptado (necesario: el módulo compila en AMBOS targets porque el lib compila `workers::route_scanner_worker`, que lo consume) |
| `main.rs:86` `mod priors_cache;` + `lib.rs:80-82` `pub mod priors_cache;` | **BR-05 (cerebro, WO-07 port-back) — programa paralelo** | Intacto, no mío |
| `route_scanner_worker.rs:253-265` GraphBuildConfig con `reserves_freshness_budget_s` | **BR-02 (cerebro) — programa paralelo** | Intacto, no mío (ver nota §37 en §5) |
| `canonical_knobs.rs` +112 líneas (knob reserves_freshness_budget_s) | BR-02 (paralelo) | **INTACTO — cero diffs míos** (el diseño §3.4 prohíbe tocarlo: los 53 knobs siguen boot) |
| `opportunity_emitter.rs`, `orchestrator.rs`, `pool_discovery.rs`, `reserves.rs` (M) + `priors_cache.rs` (untracked) | BR-02/BR-05 (paralelos) — aterrizaron DURANTE esta sesión (no estaban en el snapshot inicial) | Intactos, no míos — el árbol compartido sigue activo (§36): otra razón para NO commitear aquí |

Lo que FALTABA del diseño y esta sesión implementó: **publish del boot census en main.rs
(§3.2)** y **todo el wiring §3.3 en route_scanner_worker.rs** (cliente en spawn, threading
run_loop→run_scan_subscription, gate per-block + heartbeat).

## 2. Sitios de wiring entregados (file:line exactos, árbol post-edit)

### 2.1 `backend/searcher-rs/src/runtime_knobs.rs` (nuevo; verificado vs diseño §3.1/§3.2)

Implementación previa adoptada tras auditoría punto por punto contra el diseño:

| Pieza (file:line) | Requisito del diseño | Veredicto |
|---|---|---|
| `CONTROL_BOARD_PREFIX` :55 (`arbx:controlboard:`) | §1.2 namespace | OK |
| `BOOT_CENSUS_REDIS_KEY` :60 (`arbx:config:boot_census`) | §3.2 | OK |
| `HEARTBEAT_TTL_SECS` :63 (75) + estados `"run"/"halted"` :67-68 | §1.2 hb + §3.3 | OK |
| `CACHE_TTL` :71 (1s) | §1.3-1 poll perezoso TTL 1s (killswitch.rs:61) | OK (const, ver D5) |
| `ROUTE_SCANNER_BOARD_ON` :79-87 (IntGauge, registro Lazy en `shared_rs::metrics::REGISTRY`) | §3.3 métricas (espejo `KILLSWITCH_ENABLED` killswitch.rs:101) | OK (nombre con prefijo `arbx_`, D3) |
| `ROUTE_SCANNER_BOARD_HALT_BLOCKS_TOTAL` :90-98 | §3.3 counter | OK (ídem D3) |
| `resolve_toggle` :104-110 — `"true"`→true, `"false"`→false, ausente/ajeno→default | §3.1 state() + §15-R2 string estricto; INV-CB02-1 | OK — SIN trim deliberado: un valor que el board no escribió es drift, no veredicto (R8) |
| `RuntimeToggleClient` :127-133 + `from_manager` :139-151 | §3.1 (mgr ya conectada del spawn, default=boot) | OK (ctor sync, D4) |
| `is_on()` :155-157 — fail-safe → `default_when_absent` | §3.1 + killswitch.rs:72-77 | OK |
| `state()` :162-177 — GET TTL-cacheado; hit = copia de bool (cero allocs); errores NO cacheados | §1.3-1 + INV-CB02-5 | OK |
| `emit_heartbeat()` :183-203 — `SETEX <toggle>:hb 75 {"state","block","ts":RFC3339}` | §1.2 + §3.3(2); contrato peer TS `control-board.ts:292-301` (`ts` string) | OK — match exacto con el parse del peer |
| Sin `set()`, sin PUBLISH/SUBSCRIBE | §3.1/INV-CB02-4 + §15-R7 | OK — cliente Rust read-only |
| Mirrors de parses privados: `orchestrator_mode_from_raw` :220-227 (scanner.rs:164-186), `native_engines_from_raw` :232-234 (scanner.rs:545-548), `route_discovery_outcomes_from_raw` :239-244 (cartridge_boot.rs:382-390) | §3.2 census; fuentes privadas/fuera de claim → espejo citado + pin-test anti-drift | OK — ver D8 |
| `boot_census()` :252-274 — field set EXACTO §3.2 (11 campos), reusando los parses PUBLISHED reales (`RouteScannerMode`/`CartridgeMode`/`MempoolMode`/`GateCScoringConfig`/`MacroMevGateConfig::from_env()`) | §3.2 + RULE 00 | OK |
| Tests :276-420 — absent/garbage→default (GATE-CB02-2 parcial), keys contract, 3 pin-tests de mirrors, census field-set exacto | §3.1 tests espejo | OK (límites en §5) |

### 2.2 `backend/searcher-rs/src/main.rs` — publish del boot census (§3.2; edición de esta sesión)

- `main.rs:389-409` — dentro del MISMO bloque `{}` del publish canonical-knobs, después del
  `set_result.is_err()` warn (:382-387): `runtime_knobs::boot_census()` + `info!` una línea
  (:394-398) + `SET arbx:config:boot_census` (:399-404) + warn no-fatal
  `config.boot_census.publish_failed` (:405-409, event :407). Patrón 1:1 con :375-387
  (no-fatal, retry en el próximo boot). Reusa `knobs_redis` (mgr ya clonada).
- `main.rs:173-175` `mod runtime_knobs;` — preexistente CB-02 (adoptado).

### 2.3 `backend/searcher-rs/src/workers/route_scanner_worker.rs` — wiring clase A (§3.3; edición de esta sesión)

| file:line | Cambio |
|---|---|
| :48-51 | Import `RuntimeToggleClient` + las 2 métricas |
| :852-864 `spawn_route_scanner` | Tras `let mode = RouteScannerMode::from_env()` (:851) y ANTES del early-return `Off` (:877-879, SE CONSERVA): `RuntimeToggleClient::from_manager(redis.clone(), "route_scanner", mode == RouteScannerMode::On)` — mgr derivada de la `redis` del spawn (:787 del diseño); default = veredicto boot |
| :784 / :803 | `run_loop` gana `board: RuntimeToggleClient`; pasa `&board` a cada (re)conexión de `run_scan_subscription` |
| :707 | `run_scan_subscription` gana `board: &RuntimeToggleClient` |
| :730-763 loop per-block | ENTRE la recepción del bloque (:727-729) y `scan_block` (:764): (1) `let board_on = board.is_on().await;` :740; (2) gauge `ROUTE_SCANNER_BOARD_ON.set` :741; (3) SIEMPRE `emit_heartbeat(&mut redis, board_on, number.as_u64())` :742-753 (fallo → `debug!` :744-752; expiración TTL = DESCONOCIDO honesto); (4) `if !board_on` → `debug! halted_by_board` :756-760 + counter inc :761 + `continue` :762; (5) `scan_block` como hoy :764 |
| :30-34 doc del módulo | Corregida la frase "only Redis writes" — ahora incluye el hb CB-02 (stale por mi propio cambio) |

### 2.4 No-op confirmados (§3.4 del diseño — verificado `git status`/diff)

`canonical_knobs.rs` (0 diffs míos; +112 preexistentes BR-02 intactos) ·
`shared-rs/src/killswitch.rs` (INTACTO, 0 diffs) · `config_reload*.rs` / `topology_reload.rs`
· `scanner.rs` · `main.rs` killswitch connect (:349-351) · `chain_supervisor.rs`.

## 3. Divergencias diseño ↔ implementación (declaradas, no silenciosas)

- **D1 — Ubicación del módulo.** Diseño §3.1: `shared-rs/src/control_board.rs`. Charter:
  claim sobre `searcher-rs/src/runtime_knobs.rs` (y shared-rs/lib.rs FUERA de claim).
  Sigo el charter: el único consumidor hoy es searcher-rs; si otro crate lo necesita,
  la promoción es un move de archivo + 1 línea de lib.rs. `killswitch.rs` intacto.
- **D2 — `APPROVED_HASH`.** El sketch §3.1 lo lista como const Rust; NO se define: no tiene
  consumidor Rust (lo escribe/lee solo el PUT/GET del api-server, INV-CB02-4). Definirlo
  sería código especulativo (§37 P-∅).
- **D3 — Nombres de métricas.** Diseño: `route_scanner_board_on` /
  `route_scanner_board_halt_blocks_total`. Implementación: prefijo `arbx_` (convención del
  crate, metrics.rs — p.ej. `arbx_killswitch_enabled`). Misma semántica.
- **D4 — `from_manager` sync** (sketch: `async fn`): no hace I/O (clona handle + 2 allocs);
  un punto await aquí no aporta nada.
- **D5 — `cache_ttl` const 1s** (sketch: campo): semántica idéntica; sin consumidor de otro
  TTL (P-∅ otra vez).
- **D6 — Registro de métricas.** Sin tocar `metrics.rs` (fuera de claim): `Lazy` auto-registra
  en `shared_rs::metrics::REGISTRY` al primer uso. Consecuencia honesta: con
  `ARBX_ROUTE_SCANNER_MODE=off` las 2 métricas NO aparecen en `/metrics` hasta que el worker
  spawn-eado reciba su primer bloque — ausencia = DESCONOCIDO (R8), no cero falso. El LED
  verificado del board NO depende del gauge (lee la clave hb).
- **D7 — Registro del módulo en `lib.rs`:97-100** (fuera de claim, 4 líneas): preexistente
  CB-02 de la mitad previa, mecánicamente requerido (el target lib compila
  `workers::route_scanner_worker`, que referencia `crate::runtime_knobs`). Adoptado, no mío.
- **D8 — Mirrors de parses privados** (`orchestrator_mode`/`native_engines`/
  `route_discovery_outcomes`): sus fuentes (scanner.rs / cartridge_boot.rs) están fuera de
  claim y el parse es privado allá. El census ESPEJA la semántica exacta (mismos defaults,
  misma case-insensibilidad, sin trim donde la fuente no trimea) con cita file:line y
  pin-test que rompe primero si la fuente deriva. Los demás campos usan los parses `pub`
  reales (`from_env()`), cero re-derivación.
- **D9 — TTL-cache "1 GET/s" y "hb expirada→verified null" sin test ejecutable**: exigen un
  Redis vivo (el repo no tiene test-infra Redis; montar un doble sería mock de infraestructura
  de datos — RULE 00). La mitad PURA de GATE-CB02-2 (absent→default, garbage→default, parse
  estricto) sí está testeada (§5). La verificación end-to-end de cadencia y expiración es de
  CB-06 (browser-verify), que ya está en el board.

## 4. Semántica de propagación (insumo CB-06 — diseño §3.3 tail)

PUT (admin-token) → `SET arbx:controlboard:route_scanner` inmediato → worker lo ve en ≤1s
(TTL cache) + cadencia newHeads ~12s → hb refleja `run/halted` ≤2 bloques → LED verificado
voltea SIN restart. Ventana drift (declared≠verified) parpadea y alinea: contrato correcto.
Fail-safe en cada esquina: clave ausente/ajena/Redis caído → veredicto boot (INV-CB02-1);
hb expirada → DESCONOCIDO (null), jamás run-stale.

## 5. Verificación (target caliente del árbol principal, §36.4)

| Gate | Resultado |
|---|---|
| `cargo check -p searcher-rs` (desde `backend/`) | **EXIT 0** (7m10s; lock contention de sesiones paralelas) |
| `cargo clippy -p searcher-rs -- -D warnings` | **EXIT 0** (10m49s) |
| `cargo fmt -p searcher-rs -- --check` | **EXIT 0** |
| `cargo test -p searcher-rs --lib runtime_knobs::` | **NO run-verificado esta sesión** — run atascado >35 min en el build lock del árbol compartido (§5.1, fail-honest). Re-ejecutar con árbol quieto al fusionar |
| XLEN `arbx:opps:detected` (§33.3) | Nada de mi diff puede tocarlo: el worker JAMÁS escribe ese stream (doc route_scanner_worker.rs:32-34), el cliente es read-only y el census/hb usan claves `arbx:config:`/`arbx:controlboard:` propias. 0 acceso a VPS en esta sesión (§32 read-only respetado por omisión). |

### 5.1 Tests unitarios (GATE-CB02-2 — mitad pura) — NO run-verificados esta sesión (fail-honest)

Los 8 tests están escritos en `runtime_knobs.rs:276-420` (absent/garbage→default, parse
estricto, keys contract, 3 pin-tests de mirrors, census field-set exacto) y revisados
estáticamente, PERO el run `cargo test -p searcher-rs --lib runtime_knobs::` quedó atascado
>35 min tras el build lock del árbol compartido (hasta 5 procesos `cargo` de sesiones
paralelas activas; 0 `rustc` con CPU — cola de lock, no progreso) y NO completó dentro de
esta sesión. `check`/`clippy` NO compilan el perfil `#[cfg(test)]` (lección #460), así que
"tests compilan y pasan" queda PENDIENTE de verificación — quien fusione debe re-ejecutar
`cargo test -p searcher-rs --lib runtime_knobs::` con el árbol quieto. Nada de esto afecta
los 3 gates del charter (check/clippy/fmt), que están VERDE sobre el código no-test.

## 6. Para la mesa redonda (pares posteriores)

1. **CB-01 (censo)**: tu fila `route_scanner_multihop` clase **B** (CB-01-CENSO.md:46,
   consumidor `ARBX_ROUTE_SCANNER_MODE` :104-125,796-807) describe el gate SPAWN — coincide
   con la fila 15 del diseño. El gate RUN (fila 2, clase A, `redis:arbx:controlboard:
   route_scanner`) es el que este wiring enciende. Ambos conviven: spawn (env, boot) decide
   si EXISTE el worker; run (Redis, runtime) decide si ESCANEA por bloque. Sin contradicción
   — pero tu `arbx:config:control_board` census debería reflejar AMBAS filas para que el
   board no duplique ids (el diseño §8 reserva `route_scanner_multihop` para la A y
   `route_scanner_spawn` para la B).
2. **CB-03/CB-04**: hooks listos — hb `arbx:controlboard:route_scanner:hb` (SETEX 75,
   ambos estados), census `arbx:config:boot_census` (11 campos exactos §3.2, con
   `published_at`), métricas `arbx_route_scanner_board_on` /
   `arbx_route_scanner_board_halt_blocks_total` en `/metrics` del searcher. El revert de
   CB-04 clase A es re-SET del valor `approved` (escritor: api-server, no este worker).
3. **BR-02/BR-05 (cerebro, paralelos)**: sus diffs en MIS archivos claim quedaron intactos
   (§1). Ojo al fusionar: `canonical_knobs.rs` +112 y `route_scanner_worker.rs` graph-knob
   son de ese programa, no de CB-02 — un PR por WO (§37 P-∅) los separa limpio.
4. **§37 Nivel-1 (route-discovery)**: INTACTO — este wiring no toca `route_discovery/`,
   `multi_hop_search`, `graph_builder` ni el DFS; solo omite INVOCAR el scan desplegado
   (INV-CB02-7). El diff BR-02 que SÍ toca `GraphBuildConfig` es del programa paralelo y
   preexistía — reportado, no revertido (claim ajeno).

## 7. Entrega

Edición local + verificación SOLAMENTE (NO-GIT operador 2026-08-23): cero commit, cero push,
cero PR, cero deploy, cero VPS. Archivos tocados POR ESTA sesión (todos marcados
`// CB-02 (2026-09-07)`): `main.rs`, `workers/route_scanner_worker.rs`. Adoptados-verificados
de la mitad previa: `runtime_knobs.rs`, registros `mod` en `main.rs`/`lib.rs`.
