# BUILD REPORT — FEE-TIER-AWARE-QUOTING (WO-06)

> Builder: agent-rust (gang first-understand-20260917). Especificación contractual:
> `06-FEE-TIER-DESIGN.md`. Branch: `fix/sim-fund01b-slot-padding-20260917`
> (sin commit / push, según charter). Fecha: 2026-09-17.

## Verificación (exit codes reales)

| Gate | Comando | Exit | Resultado |
|---|---|---|---|
| fmt | `cargo fmt -p searcher-rs -- --check` | **0** | limpio |
| clippy | `cargo clippy -p searcher-rs -- -D warnings` | **0** | limpio (targets lib + bin) |
| tests | `cargo test -p searcher-rs --lib` | **0** | **1274 passed; 0 failed; 3 ignored** |

Los 3 ignored son pre-existentes (`hydrate_*` requieren Redis vivo — REDIS_URL en VPS).
AppControl (os error 4551) NO bloqueó los test binaries esta vez; no hizo falta el
workaround `--no-run` + exe directo.

## TDD: evidencia RED → GREEN

1. **RED** (mismo árbol, cuerpos `todo!("WO-06 RED")`): `cargo test --lib v3_fee` →
   `4 failed; 31 passed` (`resolve_catalog_mismatch_not_catalogued`,
   `tiers_for_pair_is_order_insensitive`, `v3_fee_resolution_replaces_blind_default`
   en `todo!()`, y `t1_pin_encoding...` — ver hallazgo abajo).
2. **GREEN**: cuerpos implementados → suite completa 1274/0.

**Hallazgo real durante RED (T1):** mi supuesto inicial de offset de la palabra fee
([132..164]) era incorrecto — el pin test FALLÓ contra el encoder real. Dump del
calldata real demostró el layout: `selector || tokenIn || tokenOut || amountIn || fee
|| sqrtLimit` (5 palabras inline, SIN palabra de offset; fee en bytes [100..132]).
T1/T7 corregidos contra el encoding OBSERVADO + selector verificado externamente.
Esto es exactamente lo que un pin test debe hacer: la palabra en [132..164] era
`sqrtPriceLimitX96` (ceros) — habría pasado un test mal escrito y anclado mal.

## Tests T1–T7 (estado: TODOS VERDE)

| ID | Test | Archivo | Estado |
|---|---|---|---|
| T1 | `t1_pin_encoding_fee_words_are_raw_pips` (3000→0x0bb8, 5→0x05, len==164) | v3_fee_catalog.rs | PASS |
| T2 | `v3_fee_resolution_replaces_blind_default` (None→Catalog(3000); Some(100)→Mismatch cotiza CON 3000; antes 500 ciego) | state_projector.rs | PASS |
| T3 | `v3_pair_without_pools_errors_without_rpc` (Err(PairHasNoV3Pools), mock PANIQUEA si invocado) | state_projector.rs | PASS |
| T4 | `v3_exotic_tiers_quote_exact_catalog_values` (catálogo {1,5}; captura EXACTA [1,5]; 100 jamás pedido) | state_projector.rs | PASS |
| T5 | `v3_pool_not_catalogued_errors_without_rpc_exact_label` (Err(PoolNotCatalogued), cero RPC) | state_projector.rs | PASS |
| T6 | `v3_pool_not_catalogued_errors_without_rpc_exact_label` + `v3_error_labels_are_exact` ("v3_pool_not_catalogued" ≠ "v3_quote_unavailable") | state_projector.rs | PASS |
| T7 | `t7_anchored_quoterv2_calldata_weth_usdc` (calldata byte-exacto WETH/USDC fee 500+3000) | v3_fee_catalog.rs | PASS |

**Vector externo de T7** (no fabricado): selector `0xc6a5026a` para
`quoteExactInputSingle((address,address,uint256,uint24,uint160))` verificado contra el
registro público 4byte.directory (API, única entrada registrada; fetch 2026-09-17,
respaldado por openchain). Palabras derivadas a mano (ABI canónico del struct inline):
WETH `0xC02a...Cc2`, USDC `0xA0b8...b48`, amountIn 1e18=`0x0de0b6b3a7640000`,
sqrtLimit=0. Comentario en el test documenta la derivación completa.

## Diff resumido por archivo

### NUEVO `backend/searcher-rs/src/v3_fee_catalog.rs` (~330 líneas)
- `FeeResolution { Catalog(u32), Mismatch{offered,catalog}, NotCatalogued }`.
- `V3FeeCatalog`: `RwLock<HashMap<Address,u32>>` (by_pool) +
  `RwLock<HashMap<(Address,Address),BTreeSet<u32>>>` (by_pair).
- `load_from_redis` (SCAN `arbx:pool_index_v3:<chain>:*` COUNT 500 + GET por clave,
  precedente price_worker.rs:672-688; merge sin clear — un pool que desaparece de
  Redis no flapea a rechazado; error Redis → Err, caller conserva snapshot previo).
- `record_observed`, `fee_for_pool`, `tiers_for_pair`, `resolve(pool, offered)`,
  `pool_count`. Gauge `arbx_v3_fee_catalog_pools` actualizado en load/record.
- `fee_resolution_metric(resolution)` pub(crate) — emisión junto al enum.

### `state_projector.rs` (+~300 netas, la mayoría tests)
- **Trait `V3QuoteProvider` BYTE-IDÉNTICO** (verificado: líneas 69-81 intactas).
- `ProjectV3Error { PoolNotCatalogued, PairHasNoV3Pools, ProviderUnavailable,
  QuoteFailed(String) }` + `as_label()` ("v3_pool_not_catalogued" / "v3_pair_no_pools"
  / "v3_quote_unavailable" reservado a fallo real de provider).
- `project_v3_quote_checked(...) -> Result<V3VirtualQuote, ProjectV3Error>`:
  ELIMINADO `unwrap_or(500)`. Orden: zero-amount → provider None → resolve (Catalog →
  cotiza; Mismatch → catalog + warn + métrica; NotCatalogued → tiers_for_pair vacío →
  Err(PairHasNoV3Pools) sino Err(PoolNotCatalogued), AMBOS cero RPC) → provider →
  post-OK `record_observed`.
- `project_v3_quote` existente = wrapper `.ok()` (call sites dex_engine:502 sigue
  compilando sin cambios).
- `LegQuote::Unavailable(&'static str)` (antes unidad) — lleva el label honesto;
  `quote_leg` mapea `Err(e) → Unavailable(e.as_label())`.
- Tests: `CapturingV3Mock` (registra (pool,fee) exactos), `PanicV3Mock` (cero RPC),
  T2-T6 + existentes actualizados (pool catalogado para el forward test).

### `size_optimizer.rs`
- `OptimizeRejectReason`: nuevos `V3PoolNotCatalogued`→"v3_pool_not_catalogued",
  `V3PairNoPools`→"v3_pair_no_pools" + `from_v3_unavailable_label(&'static str)`.
- Loop del grid V3: `v3_unavailable_label: Option<&'static str>` captura el primer
  label; el sitio de rechazo mapea el label al variant preciso (antes: siempre
  V3QuoteUnavailable).
- Tests: helpers `empty_v3_fee_catalog()` / `v3_test_fee_catalog()` (pools 0x10/0x11,
  par AAAA/BBBB, tier 500 = el fixture existente); ~20 sitios `StateProjector::new`
  actualizados mecánicamente; label-coverage test extendido.

### `metrics.rs`
- `V3_FEE_RESOLUTION_TOTAL` (counter vec {resolution}) + `V3_FEE_CATALOG_POOLS`
  (gauge), registradas en el `Lazy` (patrón del crate) Y force-registradas con las 4
  series sembradas ("catalog","mismatch","not_catalogued","pair_no_pools") en
  `init_orchestrator_metrics()` — patrón ROUTES-0 (series existen desde boot).
- Doc de `V3_QUOTE_TOTAL` actualizado con `rpc_tier_revert`.

### `v3_quote_provider.rs`
- Split del outcome post-RPC: per-pool revert (`success=false` → error "wrong fee
  tier / pool revert") ahora emite `rpc_tier_revert`; fallos de transporte
  (failover exhausted / empty result set) siguen en `rpc_error`. Reestructurado con
  `map`/`unwrap_or_else` (sin string-matching).

### `scanner.rs` (wiring)
- Boot: `V3FeeCatalog` + `load_from_redis` con timeout 30s (mismo guard P0 que la
  hidratación de reserves); logs `scanner.v3_fee_catalog_loaded/_timeout/_failed`.
- Timer 60s de refresh (`tokio::spawn`, patrón idéntico al refresh de ReservesCache).
- `StateProjector::new(reserves_cache, v3_provider, fee_catalog)`.

### `amm_math.rs`
- `encode_quote_calldata`: `fn` → `pub(crate) fn` (única forma de anclar T1/T7 al
  bytes real sin pasar por el multicall). Encoding intacto.

### `lib.rs` / `main.rs`
- `pub mod v3_fee_catalog;` (lib) + `mod v3_fee_catalog;` con `#[allow(dead_code)]`
  (bin — el crate compila el árbol dual; sin esto el bin target falla E0433 aunque
  la lib quede verde, hallazgo del primer clippy).

## Desviaciones del diseño (mínimas, justificadas)

1. **`record_observed(pool, fee)` → `record_observed(pool, token0, token1, fee)`.**
   El diseño lista la firma con 2 args, pero `by_pair (t0,t1)` necesita los tokens y
   el wire de Redis (`V3PoolInfo{pool_addr, fee_bps}`) NO trae direcciones de token
   (índice simbol-keyed) — es imposible poblar by_pair desde Redis tal cual. La firma
   ampliada mantiene el nombre y la semántica ("alta pasiva post-RPC exitosa") y
   actualiza AMBOS mapas. Consecuencia documentada: by_pair arranca vacío en frío y
   se puebla por observación; para pools catalogados (vía Redis) la resolución no
   depende de by_pair, y para no-catalogados ambos errores son honestos y cero-RPC.
2. **`LegQuote::Unavailable` ahora lleva `&'static str`.** El diseño pide labels de
   rechazo honestos pero el embudo LegQuote aplanaba el motivo a "unavailable". Sin
   el label en LegQuote, el `OptimizeRejectReason` nuevo nunca recibiría el motivo
   (el trait `V3QuoteProvider` sí quedó byte-idéntico, como exigía el diseño).
   Ripple: 3 match sites + 1 constructor — mínimo posible.
3. **`refresh()` separado no se creó** — el timer 60s del scanner llama
   `load_from_redis` directamente (el diseño ofrecía "cada ciclo pool_sync O timer
   60s"; elegido timer para no acoplar el catálogo al ciclo del worker).
4. **`encode_quote_calldata` pub(crate)** en amm_math (el diseño sitúa T1/T7 en
   v3_fee_catalog.rs pero el encoder era privado; sin esto los tests no compilan).

## Fuente de verdad respetada

Redis `arbx:pool_index_v3` (contrato wire `V3PoolInfo`, misma fuente que scanner.rs
y pool_sync_worker) — sin lector PG nuevo, sin hardcodes (RULE 00), fail-honest en
todas las ramas nuevas (R8). Modos de trading intactos (§34: matemática mode-
invariant; esto es detección/quoting read-only, sin tocar el terminus).

## Archivos NO tocados (modificaciones pre-existentes de la branch)

`counters.rs`, `opportunity_emitter.rs`, `workers/heartbeat_worker.rs`,
`workers/route_scanner_worker.rs` aparecen modificados en `git status` pero son
cambios PRE-EXISTENTES de la branch (presentes en el snapshot inicial) — este builder
no los editó.

## Criterios de aceptación pendientes de producción (post-deploy, no verificables aquí)

2 (caída >90% de v3_quote_unavailable), 3 (fee_resolution decide rama), 4 (accepts>0),
5 (cache_neg_hit <10%), 6 (fail-honest total — cubierto por tests T3/T5/T6).
