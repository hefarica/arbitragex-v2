# E3 — Mapa de código: mecanismo `pools.is_active` → detección
> Agente: code-path · Repo local branch fix/567-canonical-plan-consumer (= origin/main 2cdbce35 + PR #574) · Solo lectura · Salida completa en transcript (task a041ee26e9431431d)

## Lecturas de la tabla `pools` en searcher-rs (6 sitios SQL + 1 INSERT auxiliar)
| # | Sitio | Boot/hot | Filtra is_active | Efecto |
|---|---|---|---|---|
| 1 | impact_index.rs:582-608 load_pools_from_pg (desde scanner.rs:481-519, cap 120s) | BOOT | SÍ (:602) | Puebla universo de detección en memoria |
| 2 | scanner.rs:586-718 pool_sync_watcher (tick 60s) | HOT | NO (cursor created_at>boot) | SOLO AGREGA (idx.add_pool). Nunca remueve |
| 3 | **pool_sync_worker.rs:1028-1096+1196-1209 load_pools/load_v3_pools** — re-ejecuta cada `POOL_SYNC_RELOAD_EVERY_TICKS` (default 60, :530-533) × `POOL_SYNC_INTERVAL_MS` (default 12_000, workers/mod.rs:43-49) ≈ **cada 12 min** | BOOT+HOT | SÍ (:1044/:1079/:1204) | `pools = new_v2; v3_pools = new_v3` (:553-554) **REMPLAZA el working set**; fallo del query conserva el anterior (:566-573) |
| 4 | pool_sync_worker.rs:743-746 | HOT | no | INSERT pool_reserves (persistencia) |
| 5 | pool_enumeration_worker.rs:476-547 | HOT default OFF (ARBX_POOL_ENUM_MODE) | dedup | is_active monatónico — nunca desactiva |
| 6 | pool_discovery.rs:986-996 upsert | HOT | `is_active OR EXCLUDED` | Monotónico ascendente |
| 7 | cycle_enumerator.rs:170-198 | SIN CALLER runtime (solo tests) | — | pool_cycles append-only, boot-only |

## Universo de pools: quién puebla/refresca
- ImpactIndex (memoria): boot desde PG(is_active) + Redis; refresco SOLO aditivo. **No hay ruta de remoción en caliente.**
- Redis reserves (arbx:pool_reserves / arbx:v3_slot0): SOLO pool_sync_worker escribe, SETEX TTL 30s. Pool fuera del working set ⇒ key muere a los 30s.
- Índice pares V2 (arbx:pool_index): SET sin TTL, **reescrito desde el set activo en cada reload** (bootstrap_pool_index_cache :558+:1152-1177) — pares mixtos pierden a las desactivadas EN EL INSTANTE del reload.
- Índice V3 (arbx:pool_index_v3): boot-only + merge aditivo CAS.
- ReservesCache (memoria motores): hydrate INSERT-ONLY sin TTL/eviction — valor congelado, no ausencia.

## Hot path
- Cero PG en hot path (orchestrator resuelve contra memoria). Block mode watched = ImpactIndex::all_pools() (add-only → XEN seguían vigiladas).
- Guard de universo vacío: NO apaga emisión — logs debug (`block_scanner.no_watched_pools`, `orchestrator.impact_zero` → return sin emitir).
- Cartridge ACTIVE (paralelo a motores): lee reserves DIRECTO de Redis TTL 30s.

## Feed y 429
- Fuentes: pending-tx firehose (timeout 15s + fallback); block mode newHeads+getLogs; workers legacy default OFF.
- WS muerto: rotación + backoff 1s→30s perpetuo — proceso healthy, 0 detecciones, solo logs.
- HTTP: breaker 5err/60s; clase-429 cooldown floor 120s, reopen cap 600s; todos Open → chunk_abandoned → reserves expiran GLOBALMENTE a 30s.

## Gates "reserves frescas → emisión" (HP-06)
1. **GATE PRINCIPAL (cartridge ACTIVE)**: cartridge/runner.rs:422-439 read_pool_reserves Redis TTL 30s; miss ⇒ None ⇒ cartridge_boot.rs:1061-1065 omite clave ⇒ "reserve-dependent cartridges fail-honest" ⇒ **cero emisión, solo debug** (docstring :263-267).
2. TTL writer: pool_sync_worker.rs:80 (30s). Pool fuera del working set ⇒ key expira ⇒ gate 1 la mata en silencio.
3. Legacy: scanner.rs:1721-1728 miss ⇒ continue; single_pool_no_spread debug.
4. Motores nativos: caché insert-only ⇒ valores congelados (no ausencia).
5. Shadow/graph: missing_reserves/stale_reserves vs budget 60s (canonical_knobs.rs:129).

## RESPUESTA CENTRAL
**DEPENDE (dos planos):**
- **SÍ existe mecanismo hot**: reload del working set de pool_sync (~12 min; el comentario "≈5 min" :533 está desactualizado vs default 12s). Timing encaja: 01:05 + ~12 min ≈ 01:16-17 (+30s TTL). Al recargar: (i) dejan de escribirse reserves de XEN → keys mueren 30s; (ii) **el índice V2 pool_index se REESCRIBE en el instante** (pares XEN desaparecen del resolution).
- **PERO ese mecanismo NO puede congelar el 100% por sí solo**: el universo de detección es boot+add-only; el daño máximo del UPDATE en caliente era matar los candidatos tocando las 5 XEN (el 58% del flood). El "~42% restante debería seguir emitiendo"… **salvo que no existiera** — que es lo que E1 probó (flujo independiente ya en 0 desde 00:38). E2+E3+E1 se reconcilian: reload mató el 99.86% XEN; el 0.14% real ya estaba muerto.

## Causas alternativas para "0 detecciones con proceso healthy" (code-supported)
Feed WS muerto (429 perpetuo) · cuota HTTP global (breakers Open → reserves expiran → gate cartridge silencia TODO en silencio) · restart con boot degradado (orchestrator_build_timeout → block mode "disabled idling" permanente, block_scanner.rs:80-89) · trading_config.enabled=false (legacy retorna sin publicar, debug-only) · cartuchos vacíos/no pertinentes · getLogs fallando · ImpactIndex vacío.
No-causas descartadas: kill-switch (no detiene detección), OppDedup (solo aceptados), PG/Redis caídos (serían ruidosos).

Archivo clave de una sola cita: `pool_sync_worker.rs:536-575` (único lector hot de is_active) + `cartridge/runner.rs:427-439` (único gate que esa desactivación silencia en caliente).
