# WO CATALOG-BACKFILL-01 — Backfill del catálogo V3 (fee tiers) + label plumbing honesto

**Fecha:** 2026-09-17 · **Agent:** AGENT-BUILDER (gang PROVE-IT Oleada 3)
**Branch:** `fix/fee-tier-aware-quoting-20260917` · **Modo:** LOCAL ONLY (sin commit/push/VPS/mutación de producción)
**Marco del diff:** `// CATALOG-BACKFILL-01 (2026-09-17)` en cada sitio tocado.

---

## 1. Root cause exacto (por qué 93 entradas no parsean y por qué pools activas quedan fuera)

### 1.1 Las 93 entradas malformadas de Redis — schema legado `"address"`

`V3FeeCatalog::load_from_redis` hace `serde_json::from_str::<Vec<V3PoolInfo>>(&json)`.
`V3PoolInfo` exige el campo **`pool_addr`**. Las 93 claves malformadas contienen
entradas pre-WO-06 con el campo **`address`** en su lugar → serde rechaza el ARRAY
entero → la clave se descuenta como `malformed` y **ninguna pool de ese par entra al catálogo**
(`NotCatalogued` para todas). Además sus `fee_bps` están en unidad bps (30 = 0.30%),
no pips — basura doble.

Agravante estructural: `set_pool_index_v3` (bootstrap aditivo) hace
`merged_v3_bootstrap(previous)` que devuelve `Err("redis_v3_index_invalid")` sobre un
previous no parseable → el writer llevaba **fallando el CAS en esas 93 claves en CADA boot**
(`v3_index_bootstrap_incomplete`): el garbage legado era unpreservable e infranqueable
para el merge aditivo.

### 1.2 Tiers en unidad equivocada en PG (44 de 121 pools activas chain 1)

`bootstrap_v3_pool_index_cache` copiaba `pools.fee_tier` verbatim si `0 ≤ f < 1_000_000`.
PG contiene tiers **1/5/30** (bps-unit garbage de una ingesta legada: 100 pips=1 bps,
500=5, 3000=30) y **NULL**. Esas pools entran al índice con tier inexistente → el quoter
reverte en ese tier (`rpc_tier_revert`/`rpc_error`) o la pool se salta (`not_catalogued`).

### 1.3 Pools descubiertas después del boot nunca entraban al índice V3

La rama de refresh (`POOL_SYNC_RELOAD_EVERY_TICKS`, ~5 min) solo refrescaba el índice V2
(`bootstrap_pool_index_cache`); el bootstrap del índice V3 era **boot-only**. Toda pool V3
descubierta post-boot quedaba `NotCatalogued` hasta el reinicio del worker.

### 1.4 (Addendum) Label aplanado: catalog gap → `v3_quote_unavailable`

`DexEngine::get_pool_quote` usaba `project_v3_quote(...).ok()` — `Option::ok()`
**descartaba el `ProjectV3Error` checked** que WO-06 definió con labels precisos
(`as_label()`: `v3_pool_not_catalogued` / `v3_pair_no_pools`). `compute_v3_gross_usd`
devolvía `V3GrossOutcome::QuoteUnavailable` para cualquier `None` y el match de rechazo
(linea 335) lo publicaba como `v3_quote_unavailable`. Por eso la métrica contaba
`not_catalogued`=13.880 pero `opportunities.rejection_reason` para `dex_arb_v3v3` no
tenía NI UN string honesto. (El plumbing posterior — `StrategyCandidate.rejection_reason`
→ orchestrator:1153-1175 `emit_rejected` → emitter — pasa el string verbatim; el
aplanamiento ocurría SOLO en dex_engine.)

---

## 2. Evidencia REAL muestreada (2026-09-17, read-only vía `ssh arbx`)

### 2.1 Redis (contenedor `arbitragex-v2-redis-1`)

`--scan --pattern "arbx:pool_index_v3:*"` → **186 claves**. Dump completo de key/value:

- **93 claves OK** (141 entries pool), **93 claves malformadas** — TODAS con el schema
  legado `address` (0 clasificaciones "otras"). Muestra cruda:

  ```
  KEY: arbx:pool_index_v3:1:paw:weth
  VAL: [{"address":"0xaea3df60e99c4726abc1e7dd9a2fa570e4eed638","fee_bps":30}]
  ```

- Distribución de tiers en las claves PARSEABLES (141 entries):
  `100→48, 3000→35, 30→25, 5→15, 1→8, 500→6, 10000→3, 2500→1`
  → **48 entries con tier bps-unit (1/5/30)** viven en claves que el lector SÍ acepta
  (tier equivocada silenciosa: catálogo cita a un tier inexistente → revert).
  Ejemplos: `arbx:pool_index_v3:1:WETH:cbETH` (tiers 1,5,30,5), `arbx:pool_index_v3:1:ENA:WETH` (1,5,30).

### 2.2 PostgreSQL (read-only, `arbitragex-v2-postgres-1`)

`pools.fee_tier` de 121 pools V3 activas chain 1:

| fee_tier | count | veredicto |
|---|---|---|
| 100 | 36 | válido |
| 3000 | 24 | válido |
| 30 | 22 | bps-unit garbage |
| 5 | 14 | bps-unit garbage |
| NULL | 10 | sin resolver |
| 1 | 8 | bps-unit garbage |
| 500 | 5 | válido |

**44/121 pools activas (36%) con tier inválida o NULL** — la fuente del writer.

---

## 3. Remedio implementado (lado ESCRITOR, `pool_sync_worker.rs`)

1. **Clasificación**: `tier_needs_resolution(fee_tier)` — NULL o fuera del set
   protocolo-canónico `{100, 500, 2500, 3000, 10000}` (constante de protocolo, misma
   clase que MULTICALL3_ADDR; NO dato de operador) ⇒ sospechosa.
2. **Resolución on-chain una vez por pool**: las sospechosas pasan por un multicall
   `fee()` (mismo pipeline resiliente `multicall_resilient`: chunk + timeout + backoff +
   bisección; label `v3_fee_resolution`). El decode usa `V3FeePips::from_abi_word`
   (uint24 canónico, <1e6) — el tier exótico real (p.ej. 5 pips leído on-chain) entra
   verbatim, jamás se redondea.
3. **Cache en PG**: `UPDATE pools SET fee_tier = $1 WHERE chain_id = $2 AND address = $3`
   por pool resuelta (best-effort, warn contabilizado `fee_db_update_failed`).
4. **R8 sin fabricación**: `fee()` revierte / falla / palabra inválida ⇒ la pool se
   OMITE del índice V3 con contador (`pool_sync.v3_fee_resolution_failed`,
   `omitted=<n>`) — nunca entra con tier basura.
5. **Reparación de las 93 claves legado**: el publish del bootstrap usa el nuevo
   `set_pool_index_v3_repair` → `publish_v3_bootstrap_repair`: si el previous es
   unparseable se reintenta el merge como si la clave no existiera (el snapshot
   fee-verificado REEMPLAZA al garbage; el CAS Lua sigue guardando concurrencia).
   Un previous parseable sigue siendo merge aditivo (contract original intacto,
   test de guarda incluido).
6. **Refresh periódico del índice V3**: la rama `reload_every` ahora también re-corre
   el bootstrap V3 normalizador (warn-only si falla; el índice previo queda) — las pools
   descubiertas post-boot entran al catálogo en ≤5 min.
7. **Lector endurecido y testeable**: `ingest_index_payload` extraído de
   `load_from_redis` (contract idéntico; los casos null/legacy son unit-testeables sin Redis).
8. **(Addendum) Label plumbing**: `get_pool_quote` → `Result<U256, V3QuoteLegError>`
   usando `project_v3_quote_checked` + `err.as_label()`; nuevo
   `V3GrossOutcome::V3Labeled(&'static str)` que el match de rechazo publica verbatim.
   `v3_quote_unavailable` queda reservado a fallos reales de transporte (test de guarda).

### Diff resumido (5 archivos, +894/−137)

| Archivo | Cambio |
|---|---|
| `backend/searcher-rs/src/workers/pool_sync_worker.rs` (+419) | const canónicas, `tier_needs_resolution`, `apply_fee_resolution`, `BatchConfig` compartido, bootstrap V3 con resolución on-chain + UPDATE PG + contadores R8, repair-CAS, refresh V3 en reload, 4 tests |
| `backend/searcher-rs/src/pool_discovery/v3_fee.rs` (+130) | `publish_v3_bootstrap_repair` (replace-on-unparseable, CAS intacto), helper `observed_fee`, 3 tests |
| `backend/searcher-rs/src/v3_fee_catalog.rs` (+104) | `ingest_index_payload` extraído (testeable sin Redis), 3 tests anclados al dump real |
| `backend/searcher-rs/src/reserves.rs` (+103) | refactor `set_pool_index_v3_opt` + nuevo `pub set_pool_index_v3_repair` (plumbing CAS idéntico) |
| `backend/searcher-rs/src/engines/dex_engine.rs` (+275) | `V3Labeled(&'static str)`, `V3QuoteLegError`, `get_pool_quote` checked, match de rechazo con label verbatim, doc-table de labels, 3 tests con mock provider + catálogo real |

---

## 4. Tests añadidos (12, todos verdes)

**WO ítem 4a — entrada Redis null no entra al catálogo y cuenta malformed:**
- `v3_fee_catalog::tests::catalog_backfill_null_fee_tier_is_malformed_and_not_catalogued`
- `v3_fee_catalog::tests::catalog_backfill_legacy_address_field_is_malformed_and_not_catalogued` (el shape EXACTO de las 93 claves)
- `v3_fee_catalog::tests::catalog_backfill_canonical_payload_enters_with_exact_pips`

**WO ítem 4b — fee resuelta on-chain entra con tier correcto (vector 100/500/3000/10000 + exótico real 5):**
- `pool_sync_worker::tests::catalog_backfill_tier_needs_resolution_classification`
- `pool_sync_worker::tests::catalog_backfill_fee_resolution_accepts_canonical_and_exotic_pips`

**WO ítem 4c — fee() revierte ⇒ fuera + contador:**
- `pool_sync_worker::tests::catalog_backfill_fee_resolution_revert_and_failure_are_omitted`
  (revert / None request-level / palabra corta / pips ≥1e6 → 4 omitidos, 0 fabricados)

**Repair-CAS:**
- `pool_discovery::v3_fee::retry_regression::catalog_backfill_repair_replaces_legacy_unparseable_index`
- `...::catalog_backfill_repair_still_merges_additively_when_parseable`
- `...::catalog_backfill_plain_bootstrap_still_fails_on_legacy_index` (guarda: el publish original NO reemplaza)

**Addendum — rechazo not_catalogued sale con SU label, no el viejo:**
- `engines::dex_engine::tests::catalog_backfill_not_catalogued_rejection_keeps_precise_label`
- `...::catalog_backfill_pair_no_pools_rejection_keeps_precise_label`
- `...::catalog_backfill_provider_failure_still_uses_transport_label` (guarda inversa: fallo RPC real sigue siendo `v3_quote_unavailable`)

---

## 5. Gates de verificación (exit codes exactos)

| Gate | Comando | Exit | Resultado |
|---|---|---|---|
| fmt | `cargo fmt --check` (después de `cargo fmt`) | **0** | limpio |
| clippy | `cargo clippy -p searcher-rs --all-targets -- -D warnings` | **0** | 0 warnings |
| tests lib | `cargo test -p searcher-rs --lib` | **0** | **1290 passed, 0 failed, 3 ignored** (incluye los 12 nuevos) |
| tests all-targets | `cargo test -p searcher-rs --all-targets` | **101** | **PREEXISTENTE** — ver abajo |

**`--all-targets` falla por causa AMBIENTAL preexistente, no por este diff:** los targets
`tests/` (integración: `cartridge_simulate_swap_test`, `orchestrator_parallel_run`,
`calldata_test`, `cartridge_shadow_replay`), `benches/` (`amount_matrix`,
`discovery_matrix`) y el `bin` fallan con `E0463 can't find crate` /
`crate required to be available in rlib format` (link-stage, artefactos del workspace).
**Prueba:** se stashearon los 5 archivos de este WO y `cargo test --all-targets --no-run`
falló IDÉNTICO (46 líneas de error, exit 101) — el fallo existe sin este diff en este
equipo Windows local (target dir frío/parcial; consistente con la lección del ledger
"cargo check ≠ compila tests" y el AppControl local). La suite `--lib` — que sí enlaza y
contiene todo el código tocado — pasa 1290/1290. CI del repo ejecuta los targets de
integración en Linux donde sí compilan.

---

## 6. Qué esperar tras deploy (NO ejecutado — prohibido por el WO)

1. Boot del pool_sync: 1 multicall `fee()` de ~44 pools (una sola vez; luego cacheado en PG).
2. Las 93 claves legado se REEMPLAZAN por snapshots fee-verificados → el warn
   `v3_fee_catalog.entries_malformed malformed=93` desaparece del boot del scanner.
3. `arbx_v3_fee_resolution_total{not_catalogued}` debe caer de 23% → residual solo por
   pools reales aún no indexadas; `catalog` sube.
4. `opportunities.rejection_reason` para `dex_arb_v3v3` empieza a mostrar
   `v3_pool_not_catalogued` / `v3_pair_no_pools` (y `v3_quote_unavailable` solo para
   fallos de transporte reales — coherente con `arbx_v3_quote_total{rpc_error}`).
5. Eventos nuevos observables: `pool_sync.v3_index_bootstrapped{canonical_tier,
   fee_resolved, fee_omitted, fee_db_update_failed}` y `pool_sync.v3_fee_resolution_failed`.

**Post-deploy check sugerido (operador):**
`docker logs arbitragex-v2-searcher-rs-1 2>&1 | grep v3_fee_resolution` y
`SELECT fee_tier, COUNT(*) FROM pools p JOIN factories f ON p.factory_id=f.id JOIN dexes d ON f.dex_id=d.id WHERE p.chain_id=1 AND p.is_active AND d.protocol_type='UNISWAP_V3' GROUP BY 1;`
→ la columna 30/5/1/NULL debe migrar a 100/500/3000/10000.
