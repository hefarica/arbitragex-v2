# WO-3 — price-exchange E1+E2: forense + veredicto de verificación

> Programa RESCATE-ORPHANS-0918. Worktree `.claude/worktrees/price-exchange`,
> branch `feat/price-exchange-20260918` @ base `77321555` (0 commits propios).
> Builder original murió en 503 mientras corría vitest. Este documento aporta el
> veredicto que faltaba. SIN commit / push / PR (no-git-until-final-gate).

## 1. Forense — inventario real (`git status` del worktree)

El BOARD listaba 3 archivos nuevos; el inventario real es mayor (9 nuevos + 5 modificados):

| Estado | Archivo | Rol |
|---|---|---|
| M | `backend/api-server/src/index.ts` | +3 líneas: `subscribeToPriceUpdates(io, REDIS_URL, { pool })` — inyecta el pg.Pool al bridge |
| M | `backend/api-server/src/prices-stream.ts` | +211/-27: mirror versionado en proceso, eventos `price_snapshot`/`price_delta`/`resync:prices`, OHLC write-behind, resync en reconnect del subscriber |
| M | `backend/shared-rs/src/price_oracle.rs` | +94: `token_prices_meta_key`, `token_price_meta_value`, `append_meta_sidecar` + 4 tests |
| M | `backend/token-enricher/src/dexscreener.rs` | +13: llama `append_meta_sidecar(..., "dexscreener", ...)` en el mismo MULTI/EXEC |
| M | `backend/token-enricher/src/geckoterminal_tier.rs` | +13: ídem con `"geckoterminal"` |
| ?? | `backend/api-server/src/price-delta.ts` (187) | Motor de deltas PURO (sin Redis/socket/reloj) |
| ?? | `backend/api-server/src/price-delta.test.ts` (143) | 14 tests: ε, source-switch, seq, snapshot-seed |
| ?? | `backend/api-server/src/price-exchange-wire.test.ts` (102) | 3 tests sobre socket.io REAL + Redis fake |
| ?? | `backend/api-server/src/price-history.ts` (133) | Agregador OHLC 1-min + flush upsert batched |
| ?? | `backend/api-server/src/price-history.test.ts` (106) | 7 tests |
| ?? | `database/migrations/122_price_history_ohlc_1m.sql` (38) | Tabla `price_history` PK (chain_id,symbol,bucket_start), SIN volume, rerun-lock-safe |
| ?? | `frontend/lib/hooks/usePriceExchange.ts` (241) | Hook cliente E2: transiciones puras + socket en useEffect |
| ?? | `frontend/lib/hooks/usePriceExchange.test.ts` (121) | 11 tests |
| ?? | `frontend/components/price-exchange/PriceDeltaFeed.tsx` (83) | Superficie standalone del feed (no montada en ninguna page) |

### 1.1 Qué hace el delta (E1 — servidor)

Wire contract congelado (aditivo, el legacy `prices:snapshot`/`prices:update` NO cambia):
- `price_snapshot` `{chain_id, prices, count, ttl_secs, ts, seq, version}` — en subscribe, en `resync:prices`, y en `ready` del subscriber Redis (re-lectura de todas las chains conocidas con clientes).
- `price_delta` `{chain_id, seq, version, ts, deltas:[{token, prev, next, source, ts, seq}]}` — solo cuando hay deltas.
- `resync:prices` `{chain_id}` cliente→servidor.

Semántica implementada (enmiendas del veredicto Sancho a/b/d/e):
- (a) ε = 1bp relativo (`EPS_REL=1e-4`) **gatea la EMISIÓN, no el valor**: el mirror siempre avanza; payload redondeado a 6 cifras significativas; hash Redis intacto.
- (b) source-switch emite aunque el precio sea idéntico; la fuente viene del sidecar `arbx:token_prices:meta:<chain>` (JSON `{source, ts}` por símbolo); sidecar ausente ⇒ `"unknown"` (R8, nunca fabricado).
- (d) OHLC 1-min write-behind: TODA lectura alimenta el agregador (el ε NO crea puntos ciegos en historia); flush cada 30s con timer `unref`; errores de flush se loguean y los buckets se descartan (gap honesto, sin re-encolado). Solo activo si hay `pool`. SIN columna volume (RULE 00).
- (e) keyspace-notify rechazado; `seq` por chain en cada notice; cliente detecta `seq !== last+1` ⇒ `resync:prices`.
- Coste hot-path: una sola lectura `HGETALL×2 + TTL` por notice alimenta mirror + OHLC + broadcast. Cambio de comportamiento: con `pool` presente el HGETALL se hace aunque no haya clientes WS (necesario para historia) — antes se saltaba.

### 1.2 Qué hace E2 (cliente)
`usePriceExchange(chainId|null)`: estado `{prices, sources, recentDeltas(≤50), lastSeq, version, ts}` + status `CONNECTING|LIVE|STALE`. Transiciones puras exportadas (`applyPriceSnapshot`, `applyPriceDelta`, `needsResync`); frame con gap se DESCARTA y dispara resync fuera del updater (ref). `PriceDeltaFeed.tsx` es standalone: no está importado por ninguna page (verificado por grep: 0 referencias fuera de su propio archivo) — explícitamente diseñado para componerse cuando aterrice `feat/cex-ui-existing-20260918` (#589).

### 1.3 Pieza Rust
- `append_meta_sidecar` hace HSET por símbolo + EXPIRE del sidecar con el mismo TTL del hash de precios, dentro del pipeline atómico existente, antes del PUBLISH. No-op si no hay símbolos.
- Cableado: dexscreener + geckoterminal. **PENDIENTE declarado en el propio código**: `searcher-rs price_worker` NO escribe el sidecar (constraint no-touch de otro builder). Consecuencia operativa: precios escritos por `price_worker` llegan al api-server con `source:"unknown"` — honesto, no roto, pero la detección de source-switch será parcial hasta cablearlo.

## 2. Verificación — números reales

Entorno: el worktree NO tenía `node_modules` raíz (install parcial del builder muerto: solo `backend/api-server/node_modules` con socket.io 4.8.3, pg 8.23.0, etc.). Remedio no invasivo: junction `worktree/node_modules → árbol-principal/node_modules` (vitest, ioredis, socket.io-client resuelven vía junction; los nested del api-server tienen precedencia y coinciden con lo que el lockfile del worktree pide). El junction NO es un archivo git (untracked, fuera de cualquier diff).

### 2.1 vitest api-server — suite COMPLETA (`npx vitest run` en `backend/api-server`)
```
Test Files  69 passed (69)
     Tests  875 passed (875)
  Duration  14.75s
```
(stderr `ioredis ETIMEDOUT` en `readiness.test.ts` = pre-existente, sin Redis local; no afecta el resultado.)

### 2.2 vitest — solo los 3 archivos nuevos
```
src/price-delta.test.ts          14 tests  ✓
src/price-history.test.ts         7 tests  ✓
src/price-exchange-wire.test.ts   3 tests  ✓  (socket.io real, puerto efímero)
Test Files 3 passed (3) · Tests 24 passed (24) · 922ms
```

### 2.3 vitest frontend — `lib/hooks/usePriceExchange.test.ts`
```
Test Files 1 passed (1) · Tests 11 passed (11) · 918ms
```

### 2.4 typecheck api-server
`npm run typecheck` (tsc --noEmit) → salida vacía, exit 0.

### 2.5 Rust (target caliente del árbol principal vía `CARGO_TARGET_DIR`, §36.4)
```
cargo check -p shared-rs -p token-enricher   → Finished dev in 2m 08s (0 warnings en los crates tocados)
cargo test  -p shared-rs --lib meta_sidecar  → 4 passed; 0 failed (232 filtered out)
  append_meta_sidecar_noop_on_empty_symbols ... ok
  append_meta_sidecar_encodes_hsets_and_expire ... ok
  meta_sidecar_key_matches_wire_contract ... ok
  meta_sidecar_value_shape_is_source_plus_unix_ms_ts ... ok
```
AppControl 4551 NO bloqueó el binario de test en perfil dev (consistente con memoria: dev ejecuta mayormente).

## 3. Fixes aplicados
**Ninguno.** Todo el trabajo huérfano pasa tal cual. No se tocó ninguna línea de código del worktree.

## 4. Dictamen por pieza (RULE 00 / R8)
| Pieza | Dictamen |
|---|---|
| price-delta.ts + tests | RESCATABLE, verde. Puro, sin fabricación. |
| prices-stream.ts | RESCATABLE, verde. Legacy intacto; nuevos eventos aditivos. |
| price-history.ts + migración 122 | RESCATABLE, verde. Sin volume; upsert idempotente. Migración no aplicada en ningún entorno (solo archivo). |
| usePriceExchange + PriceDeltaFeed | RESCATABLE, verde. Componente NO montado — integración UI depende de #589. |
| price_oracle.rs sidecar + enricher | RESCATABLE, verde. Gap declarado: `price_worker` sin sidecar. |

## 5. Riesgos / pendientes para el gate del operador
1. `searcher-rs price_worker` no escribe el sidecar ⇒ `source:"unknown"` para esa fuente (WO de seguimiento, ~5 líneas idénticas al patrón enricher).
2. Con `pool` inyectado, el bridge hace HGETALL en cada notice aunque no haya clientes WS (necesario para OHLC). Coste: 2 HGETALL + TTL por notice por chain; aceptable, pero es un cambio de perfil de carga sobre Redis a documentar en el PR.
3. Migración 122 debe correr antes del deploy del api-server (si no, el flush falla cada 30s con log R8 — no rompe nada más, pero es ruido).
4. `PriceDeltaFeed` sin page: entregable E2 es el hook + contrato; la UI real llega con #589.
5. El junction `node_modules` del worktree es un artefacto local de verificación — borrar antes de cualquier `npm install` en ese worktree.

## 6. Paquete PR sugerido (NO abierto)
Título: `feat(prices): WO-PRICE-EXCHANGE-V1 E1+E2 — delta-streaming ε=1bp + source sidecar + OHLC 1m write-behind`
Un solo ID (§37 P-∅). Revert = revert del commit único; legacy wire intacto ⇒ rollback sin impacto en clientes actuales.
