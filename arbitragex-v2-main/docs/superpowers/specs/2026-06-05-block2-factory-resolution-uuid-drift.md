# SPEC — Block 2: resolución de factory + reconciliación de drift UUID (desbloqueo de A)

> Entregable del guardián (Sesión C) para **A**. Documento de diseño, NO código.
> Carril: shadow / read-only / capital=0. No activa executor, wallets ni broadcast.
> Branch: `feat/cartridge-hotpath-shadow` (HEAD `fb3f862` al escribir, 2026-06-05).

## Context

A está estancado en **Block 2**: tomar los pools rankeados top-N-por-TVL del
subgraph (`fetch_top_pools_by_tvl`, commit 4145a66) y persistirlos/activarlos.
El plan asumía que el único bloqueo era "RankedPool no trae `factory_id`". La
auditoría read-only encontró algo **más profundo y que hay que arreglar primero**:

**El path de persistencia Rust completo está roto por drift de tipos contra columnas UUID.**
Las tablas `factories`, `pools`, `tokens` usan `id UUID` (migraciones 021/022), pero
`pool_discovery.rs` lee y bindea esos IDs como `i64`/`u64`:

- `get_factories` ([pool_discovery.rs:97](../../../backend/searcher-rs/src/pool_discovery.rs#L97)) hace
  `query_as::<_, (i64, String, String, String)>` sobre `SELECT f.id …` donde `f.id` es UUID →
  **el decode falla siempre** → `facts` queda vacío → `warn!("discovery_failed:no_factories_configured")`.
  **Los factories nunca cargan.**
- `upsert_pool_in_db` ([pool_discovery.rs:736](../../../backend/searcher-rs/src/pool_discovery.rs#L736))
  bindea `factory_id as i64` (y `token0_id/token1_id as i64`) a columnas **UUID** → el INSERT
  **siempre falla** (capturado → `warn!("pool_discovery.upsert_pool_failed")`).
- `upsert_token_in_db` ([pool_discovery.rs:707](../../../backend/searcher-rs/src/pool_discovery.rs#L707))
  devuelve `Ok(id as u64)` decodificando un UUID → mismo defecto.

→ Conclusión: **Block 2 no puede construirse encima de un path muerto.** Primero se
reconcilia el drift UUID (P0); luego se añade la resolución de factory para Block 2
con el fork **RESOLVE on-chain** (decisión del operador, 2026-06-05).

Decisión de fork (descartado **BYPASS** `arbx:config:pools`): ese canal está declarado
en [config_reload_omni.rs:109](../../../backend/searcher-rs/src/config_reload_omni.rs#L109) pero
**no tiene consumidor** (no existe `PoolsCatalog`) → key muerta hoy; requeriría ~200-400
líneas nuevas en `searcher-rs` (alta colisión con el WIP de B). RESOLVE reusa el path de
hidratación existente, es aislable y de menor colisión.

---

## P0 — Reconciliar el drift UUID (prerequisito, bloquea todo lo demás)

Cambiar la representación de IDs de `u64`/`i64` a **UUID** end-to-end en el path de
persistencia de `searcher-rs`. Recomendado: tipo `uuid::Uuid` nativo de sqlx
(verificar primero que el feature `uuid` esté habilitado en `sqlx` del Cargo de
`searcher-rs`; si no, usar `String` y castear con `::uuid` en el SQL).

Cambios (todos en [backend/searcher-rs/src/pool_discovery.rs](../../../backend/searcher-rs/src/pool_discovery.rs)):

1. **`get_factories`** (L86-125): cambiar el tuple de retorno de
   `(u64, Address, ProtocolType, String)` → `(Uuid, Address, ProtocolType, String)`;
   `query_as::<_, (i64, …)>` → `query_as::<_, (Uuid, …)>`; quitar el `f_id as u64`.
2. **`upsert_token_in_db`** (~L690-711): retornar `Uuid` en vez de `u64`; decode/insert con `Uuid`.
3. **`upsert_pool_in_db`** (L713-748): firma `factory_id/token0_id/token1_id: Uuid`;
   quitar los `as i64`; bindear `Uuid` directo.
4. **`discover_from_intent`** y `hydrate_and_persist_pool` (L128+, ~L297-386): propagar
   `Uuid` en lugar de `u64` por toda la cadena (los tuples cacheados, los args).

**Verificación P0:** tras el fix, contra una DB real con factories seeded (migración
[043](../../../database/migrations/043_seed_multichain_dexes_factories.sql)), `get_factories`
ya NO debe loggear `no_factories_configured`, y un `discover_from_intent` debe poder
persistir un pool sin `upsert_pool_failed`. (No inventar IDs — si la DB no tiene el
factory, el path de Block 2 hace skip+observation, ver abajo.)

---

## Block 2 — RESOLVE on-chain (sobre P0 ya verde)

Wire el código muerto `fetch_top_pools_by_tvl` a un worker de enumeración. Por cada
`RankedPool` ([subgraph_client.rs:186-210](../../../backend/math-engine/src/subgraph_client.rs#L186),
que trae `address`, `token0/1`, símbolos, decimals, `fee_tier`, `tvl_usd`, `kind` —
**sin factory**), ejecutar el pipeline que el propio comentario
[subgraph_client.rs:181-185](../../../backend/math-engine/src/subgraph_client.rs#L181) ya prescribe:
"dedup → **resolve the factory** → token-safety screen → on-chain hydration".

1. **Dedup** por `(chain_id, address)` contra lo ya activo (evita trabajo repetido).
2. **Resolve factory (on-chain):** llamar la view `factory()` del pool/par
   (existe tanto en pares V2 como en pools V3) vía el `rpc_pool` client que ya usa el
   path de hidratación. → devuelve la **address del factory**.
   - El subgraph NO trae el factory (la GraphQL query en
     [subgraph_client.rs:233-235](../../../backend/math-engine/src/subgraph_client.rs#L233) no lo pide),
     por eso la lectura on-chain es obligatoria. Encaja con el patrón existente:
     `hydrate_and_persist_pool` ya re-verifica el pool on-chain antes de activarlo.
3. **Lookup factory.id:** buscar en el cache de `get_factories` (ya cargado en P0) por
   `(chain_id, factory_address)` — la natural key de `factories` es `UNIQUE (chain_id, address)`
   ([021_defi_registries.sql:47-54](../../../database/migrations/021_defi_registries.sql#L47)).
   - **Si el factory NO está seeded → NO insertarlo a ciegas.** Registrar
     `observation` con razón `factory_unresolved` (fail-honest, R8) y **skip** ese pool.
     Seedear factories nuevos es decisión aparte (migración tipo 043), no improvisar.
4. **Token-safety screen:** reusar el cache existente
   ([008_token_safety_cache.sql](../../../database/migrations/008_token_safety_cache.sql)) +
   el umbral `min_token_safety_score` de
   [pre_execute_checklist.rs](../../../backend/shared-rs/src/pre_execute_checklist.rs). Si algún
   token no pasa → skip + observation (`token_unsafe`).
5. **Hydrate + persist:** llamar el `hydrate_and_persist_pool` existente con el
   `factory_id: Uuid` resuelto + los `token*_id: Uuid` (de `upsert_token_in_db`).
   Esto reusa la re-verificación on-chain y el `upsert_pool_in_db` ya arreglado en P0.

**Aislamiento de colisión:** el worker de enumeración nuevo debe vivir en su propio
módulo (`searcher-rs/src/pool_enumeration.rs` o similar), invocando funciones de
`PoolDiscoveryService` — NO reescribir `pool_discovery.rs` más allá del fix P0. B está
en frontend, así que no colisiona; coordinar igualmente cualquier toque a `searcher-rs`.

---

## Critical files

| Acción | Archivo | Refs |
|---|---|---|
| Fix drift UUID (P0) | `backend/searcher-rs/src/pool_discovery.rs` | get_factories L86-125 · upsert_token L690-711 · upsert_pool L713-748 · discover L128+ |
| Fuente RankedPool / fetch | `backend/math-engine/src/subgraph_client.rs` | struct L186-210 · fetch L332-376 · comentario L181-185 · query L233-235 |
| Worker nuevo (Block 2) | `backend/searcher-rs/src/pool_enumeration.rs` *(crear)* | invoca PoolDiscoveryService |
| Schema (verdad) | `database/migrations/021_defi_registries.sql`, `022_defi_pools_routes.sql`, `044_*` (is_active) | factories/pools UUID + FKs |
| Reuso token-safety | `database/migrations/008_token_safety_cache.sql`, `backend/shared-rs/src/pre_execute_checklist.rs` | umbral safety |
| Factories seeded | `database/migrations/043_seed_multichain_dexes_factories.sql` | natural key (chain_id,address) |

## Reuse (no reinventar)
- `PoolDiscoveryService::hydrate_and_persist_pool` — re-verificación on-chain + persist.
- `PoolDiscoveryService::get_factories` (cache) — lookup factory por address tras P0.
- `rpc_pool` client — para la lectura `factory()` on-chain.
- `token_safety_cache` + `min_token_safety_score` — screen, no escribir uno nuevo.

## Constraints / doctrina
- Shadow/read-only: capital=0, sin executor/wallets/broadcast. OMEGA SEAL intacto.
- **Zero-invención (RULE 00 / R8):** factory no resuelto o token inseguro → `observation`
  con razón exacta + skip. NUNCA insertar factory a ciegas ni fabricar IDs.
- No tocar el WIP de B (frontend `pools/chains/rpcs/EdgeState`) ni `pool_discovery.rs`
  fuera del fix P0.
- `searcher-rs` es crate compartido → coordinar el merge con la cadena del gate.

## Verification (end-to-end)
1. **Build+unit:** `cd backend && cargo test -p searcher-rs -p shared-rs -p math-engine --lib`
   (= step 4 del gate, [scripts/integration-gate.sh](../../../scripts/integration-gate.sh)).
2. **P0 vivo:** contra DB con factories seeded, confirmar que desaparece
   `discovery_failed:no_factories_configured` y que un pool persiste sin `upsert_pool_failed`.
3. **Block 2 vivo (shadow):** correr el worker de enumeración contra el subgraph real;
   verificar en logs el reparto: `persisted` vs `factory_unresolved` vs `token_unsafe`
   (todas razones honestas, ninguna fabricada).
4. **Invariante:** `XLEN arbx:opps:detected` no decrece (step 6 del gate).
5. Gate completo GREEN antes de cualquier merge→main (lo dispara el operador).

## Out of scope (este spec = solo desbloqueo de A)
- Bug abierto de `/pools` (schema `{success,data}` vs API `{count,items}`) → carril de B.
- Protocolo de re-run del gate como entregable separado → no incluido por decisión de alcance.
- Block 3 IO (`risk_ledger_io.rs`, publish a `arbx:risk:breakers`) → carril de A/B aparte.
