# Strategy Dataflow E2E Audit — ArbitrageX v2

> **Generado**: 2026-05-12 — auditor: Claude Opus 4.7.
> **Alcance**: estrategias, runtime-status, opportunities/live, DEXes/pools, searcher-rs wiring, observabilidad.
> **Restricciones honradas**: NO frontend, NO VPS deploy, NO mocks, NO destructivo. Solo lectura del VPS productivo y código fuente.
> **Estado del repo al auditar**: HEAD = `c0a55c9` (merged PR #43 de Codex). Container productivo todavía corriendo código pre-`48824d1`.

---

## 1. Executive summary (20 bullets clave)

1. **4 engines existen en código** y están registrados en `orchestrator.rs` (líneas 78-83): `triangular_engine`, `flashloan_engine`, `liquidation_engine` + DEX (parte del flujo principal).
2. **Solo `dex_arb` produce candidatos**. PG últimas 24h: **5003 filas dex_arb, 0 triangular, 0 flashloan, 0 liquidation**.
3. **`dex_arb` tiene 100% de rechazo**. Última 1h: 112 candidatos, 0 viables. Razón única: `safety_below_threshold`.
4. **Heartbeat confirma engines NO invocados**: `triangular_cycles_scanned=0`, `flashloan_arb_pairs_scanned=0`, `liquidation_positions_scanned=0`.
5. **Mempool decoder pierde 99.7%**: `pending_received=761`, `decoded_ok=2` (período 60s). Bottleneck severo en decoding.
6. **Token allowlist es restrictivo a 4 tokens**: WETH, USDC, USDT, WBTC. Esto explica el alto drop en gates.
7. **`runtime-status` no diferencia "enabled config" vs "enabled runtime"**: `liquidation.enabled` está hardcoded `false`, pero `trading_config.enabled_strategies` lo lista activo. Semánticas contradictorias.
8. **🚨 BUG CRÍTICO P0 — claves duplicadas en HEAD**: `strategy-runtime-status.ts:212-215` tiene dos definiciones de `impacted_lending_positions_1h` y `hf_below_one_count`. Último gana en JavaScript → los valores `null` curados son sobrescritos por los conflateos `heartbeat.liquidation_positions_scanned ?? null` y `dbStats.liquidation.candidates_1h`. Introducido por merge de PR #43 (Codex) — **AÚN NO desplegado, container VPS corre versión vieja sin el bug**.
9. **🚨 BUG CRÍTICO P0 — Strategy naming inconsistente**: `persistence.rs:14` y `triangular_worker.rs:78` emiten `"triangular"`; `strategy_label.rs:114` serializa `"triangular_arb"`; `trading_config.enabled_strategies` lista `"triangular"`. Esto puede causar drops silenciosos en gates por strategy_kind no-match.
10. **`/api/v1/strategies/runtime-status` NO refleja el bug P0**: la respuesta VPS muestra `impacted_lending_positions_1h: 0, hf_below_one_count: 0` (versión vieja). Pero al re-build del container el bug aparecerá.
11. **Pool indexación**: 46 `pool_index` + 34 `v3_slot0` + 26 `pool_reserves` keys en Redis. Pequeño universe.
12. **Liquidation BLOCKED por `lending_watchlist_size=0`**: no hay LendingPositionIndexer alimentando posiciones Aave/Compound a Redis.
13. **Flashloan BLOCKED por dependencia downstream**: requiere `base_candidates_seen_1h > 0 && wrappedCandidates=0` para reportar "waiting_for_profitable_base". Actualmente `base_candidates=0` porque ningún dex_arb pasa el gate `safety_below_threshold`.
14. **Triangular ARMADO pero NO invocado**: tiene 46 pool_index entries + 60 reserves. Heartbeat dice `triangular_cycles_scanned=0`. Posible causa: pools indexados ≠ pools impactados por mempool reciente (cycles intersection vacía).
15. **`/opportunities/live` por default ahora muestra rejected** (`viable_only=false` desde commit `48824d1`). Dashboard ve "wall of safety_below_threshold" porque no hay viables.
16. **Endpoint `runtime-status` retorna 200 + JSON** en `7ms` (fast). Pero algunos campos hardcoded `0` (Item #4 audit previo) ya fueron corregidos a `null` en `48824d1`.
17. **Workers spawning gated por env** (`legacy_triangular_worker_enabled`, `legacy_liquidation_worker_enabled`). Si los env vars no están seteados, workers no arrancan aunque engines existan.
18. **Métricas heartbeat completas** (40+ counters) pero NO existe endpoint público que las consuma con desglose por strategy. Solo se proyecta parcialmente a `runtime-status`.
19. **Catálogo de estrategias** (`/api/v1/strategy-catalog`) tiene `lifecycle_status` con valores `live|designed|scaffold|not_started|defensive_only`. **`dex_arb`** y **`flashloan_arb`** marcadas como `"live"`. **`cex_dex`** como `"designed"`.
20. **Cero edge de mercado en última hora**: 112 candidatos detectados, 100% bajo threshold de safety. No es problema de cableado — el sistema sí está corriendo, pero el universo de tokens elegibles + thresholds actuales = cero rentable.

---

## 2. Tabla: estrategia × cableado × realtime × publicación

| Estrategia | ¿Cableada en código? | ¿Consume realtime? | ¿Publica a `opportunities`? | Bloqueadores |
|------------|---------------------|---------------------|------------------------------|---------------|
| **`dex_arb`** | ✅ `orchestrator.rs` flujo principal | ✅ Mempool → scanner → spine | ✅ 5003 filas/24h, 100% rejected | Thresholds + universe tokens. `safety_below_threshold` constante → market edge ausente o `max_token_risk_score` muy estricto |
| **`triangular_arb`** | ✅ `triangular_engine.rs` + `triangular_worker.rs` (spawn condicional) | ⚠️ Pool index armado (46+60) pero `triangular_cycles_scanned=0` | ❌ 0 filas históricas | Cycles intersection vacía: pools indexados no coinciden con pools impactados por mempool reciente. Posiblemente cycle definitions desactualizadas |
| **`flashloan_arb`** | ✅ `flashloan_engine.rs` + `flashloan_arb_worker` (módulo existe) | ⚠️ `flashloan_arb_pairs_scanned=0` | ❌ 0 filas históricas | Engine wraps base candidates rentables. Como `dex_arb.paper_viable_1h=0`, nunca hay base profitable que envolver |
| **`liquidation`** | ✅ `liquidation_engine.rs` + `liquidation_worker.rs` (spawn condicional) | ❌ `liquidation_positions_scanned=0`, `lending_watchlist_size=0` | ❌ 0 filas históricas | **`LendingPositionIndexer` no produce a Redis**. Sin watchlist Aave/Compound, engine no tiene posiciones para evaluar HF |

---

## 3. Tabla: cada campo de runtime-status × source × verdad × fix

`backend/api-server/src/routes/strategy-runtime-status.ts` (HEAD `c0a55c9`):

| Campo | Fuente actual (código) | Source-of-truth real | Estado | Acción recomendada |
|-------|-------------------------|-----------------------|--------|---------------------|
| `source.postgres` | sourceStatus init `"ok"`, catch sets `"unavailable"` + 503 (linea 110-115) | Correcto | ✅ OK | Mantener |
| `source.redis` | `"ok"` default, `"unavailable"` en catch granular | Correcto | ✅ OK | Mantener |
| `source.logs` | hardcoded `"not_used"` | Correcto | ✅ OK | Mantener |
| `dex_arb.enabled` | hardcoded `true` | Debería leer `trading_config.enabled_strategies` | ⚠️ INCORRECTO | Derivar de trading_config |
| `dex_arb.engine_loaded` | derivado: `redisOk && heartbeat keys > 0` | Inferencia razonable | ✅ OK | Mantener |
| `dex_arb.engine_invoked` | hardcoded igual a `engineLoaded` | Inferencia débil | ⚠️ DÉBIL | Usar counter heartbeat `decoded_ok > 0` o similar |
| `dex_arb.last_invoked_at` | de `heartbeat.emitted_at_unix` | Aproximación (cuando heartbeat se emitió, no cuando engine corrió) | ⚠️ DÉBIL | Añadir `last_dex_engine_invoke_at` al heartbeat (Rust side) |
| `dex_arb.candidates_1h` | PG `COUNT(*)` from opportunities | Real | ✅ OK | Mantener |
| `dex_arb.rejections_1h` | PG `COUNT(rejection_reason IS NOT NULL)` | Real | ✅ OK | Mantener |
| `triangular_arb.engine_invoked` | `heartbeat.triangular_cycles_scanned > 0` | Correcto | ✅ OK | Mantener |
| `triangular_arb.triangular_pool_map_entries` | `redis SCAN arbx:pool_index:*` count | Correcto | ✅ OK | Mantener |
| `triangular_arb.pool_index_entries` | mismo valor que arriba | DUPLICADO de triangular_pool_map_entries | ⚠️ REDUNDANTE | Eliminar uno |
| `triangular_arb.reserves_cache_entries` | `redis SCAN arbx:pool_reserves:* + arbx:v3_slot0:*` | Correcto | ✅ OK | Mantener |
| `triangular_arb.impacted_cycles_last_1h` | hardcoded `null` con comentario "Hard to infer without observation logs" | No-source explícito | ✅ OK (honest null) | Mantener hasta que existan observations |
| `triangular_arb.cycles_with_missing_reserves` | `heartbeat.triangular_v3_quote_failures \|\| 0` | Heartbeat real pero `\|\| 0` enmascara undefined | ⚠️ R8 VIOLATION | Cambiar a `?? null` |
| `flashloan_arb.base_candidates_seen_1h` | `heartbeat.flashloan_arb_pairs_scanned \|\| 0` | Heartbeat real pero `\|\| 0` enmascara | ⚠️ R8 VIOLATION | Cambiar a `?? null` |
| `flashloan_arb.wrapped_candidates_1h` | `heartbeat.flashloan_arb_opps_emitted \|\| 0` | Heartbeat real pero `\|\| 0` | ⚠️ R8 VIOLATION | Cambiar a `?? null` |
| `flashloan_arb.no_provider_rejections` | PG count de `rejection_reason="flashloan_no_provider"` | Real (emitido en `flashloan_engine.rs:174`) | ✅ OK | Mantener |
| `flashloan_arb.negative_after_fee_rejections` | `heartbeat.flashloan_arb_sanity_reject ?? null` | Correcto | ✅ OK | Mantener |
| `liquidation.enabled` | **🚨 línea 201: `liqInvoked \|\| candidates_1h > 0`** | Conflación: config flag vs runtime activity | ❌ INCORRECTO | Cambiar a `false` o derivar de `trading_config.enabled_strategies` |
| `liquidation.lending_watchlist_size` | `heartbeat.liquidation_positions_scanned \|\| 0` | Heartbeat counter de scan, no de watchlist size | ⚠️ SEMÁNTICA INCORRECTA | Renombrar a `lending_positions_scanned_1m`; agregar real `watchlist_size` desde indexer |
| `liquidation.indexed_positions` | mismo valor que `lending_watchlist_size` | DUPLICADO | ⚠️ REDUNDANTE | Eliminar o usar otra fuente |
| `liquidation.impacted_lending_positions_1h` | **🚨 DUPLICADO con valor null (212) y derivado (214)** | Sin source real | ❌ BUG CRÍTICO | Eliminar línea 214, mantener `null` (212) |
| `liquidation.hf_below_one_count` | **🚨 DUPLICADO con null (213) y `dbStats.liquidation.candidates_1h` (215)** | Sin source real (HF<1.0 está en `lending_position_indexer.rs` interno, no en heartbeat) | ❌ BUG CRÍTICO | Eliminar línea 215, mantener `null` (213) |
| `liquidation.liquidation_candidates_1h` | PG count de strategy_kind=liquidation | Real | ✅ OK | Mantener |

**Anti-patrones detectados en este archivo**:
- Object literal con claves duplicadas (P0 bug)
- `enabled` conflado entre config y runtime
- `lending_watchlist_size` semánticamente erróneo (es scan counter, no tamaño de watchlist)
- `|| 0` en lugar de `?? null` para campos opcionales del heartbeat

---

## 4. Tabla: DEX / Pool coverage

| Capa | Estado actual | Riesgo | Acción |
|------|---------------|--------|--------|
| **`enabled_dex_ids`** en trading_config | `null` (todos? ninguno?) | Ambigüedad: backend interpreta como "todos habilitados" o "ninguno" según código | Revisar `scanner.rs` para confirmar default + documentar explícitamente |
| **`strategy_configs.dex_arb.enabled_dex_ids`** | `null` | Mismo problema | Idem |
| **Redis `arbx:pool_index:1:*`** | 46 keys | Universo V2 pequeño | Verificar si discovery dinámico expande (`pool_discovery.rs`) o solo hay 46 hardcoded |
| **Redis `arbx:pool_index_v3:1:*`** | (en mismo SCAN total = 46) | Si V3 está vacío explica triangular sin cycles | Verificar separado |
| **Redis `arbx:v3_slot0:*`** | 34 keys (presumiblemente cross-chain) | Posible Uniswap V3 incompleto para chain 1 | Filtrar por chain_id=1 específico |
| **Redis `arbx:pool_reserves:*`** | 26 keys | Reserves frescas < pool_index → algunos pools sin reservas | Verificar staleness de reservas |
| **Universe efectivo de tokens** | `allowed_token_symbols=[WETH,USDC,USDT,WBTC]` (4 tokens) | EXTREMADAMENTE restrictivo. Mempool tiene cientos de tokens. | Expandir allowlist o cambiar a deny-by-token-risk-score |
| **`gate_token_not_allowed`** en heartbeat | 2 (en último período 60s, de 2 decoded) | 100% de mempool decoded cae en este gate | Confirma allowlist es bottleneck #1 |
| **Token risk score** | `max_token_risk_score=1` | Probablemente muy estricto si `token-enricher` calcula score>1 para tokens no-bluechip | Validar distribución de scores en `tokens` table |

---

## 5. Anti-patrones detectados (compilados)

| # | Anti-pattern | Ubicación | Severidad |
|---|--------------|-----------|-----------|
| 1 | **Claves duplicadas en object literal** | `strategy-runtime-status.ts:212-215` | **P0** |
| 2 | **Strategy naming inconsistente** (`triangular` vs `triangular_arb`) | `persistence.rs:14`, `triangular_worker.rs:78`, `strategy_label.rs:114`, `trading_config.enabled_strategies` | **P0** |
| 3 | **`enabled` conflado config/runtime** | `strategy-runtime-status.ts:201` (liquidation), `135`/`161`/`192` (todas hardcoded `true/false`) | **P1** |
| 4 | **`\|\| 0` en lugar de `?? null`** para heartbeat fields | `strategy-runtime-status.ts:173,176,177,205` | **P1** |
| 5 | **`liquidation.lending_watchlist_size` derivado de `liquidation_positions_scanned`** | `strategy-runtime-status.ts:194,210` | **P1** (semántica errónea) |
| 6 | **Campos duplicados sin valor agregado** (`indexed_positions == lending_watchlist_size`) | `strategy-runtime-status.ts:211` | **P2** |
| 7 | **Redis `KEYS` en lugar de `SCAN`** (con comentario admitiendo el riesgo) | `strategy-runtime-status.ts:41-46` | **P2** (acceptable para 46 keys pero antipattern documentado) |
| 8 | **`engine_invoked` hardcoded igual a `engine_loaded`** (dex_arb) | `strategy-runtime-status.ts:137` | **P2** |
| 9 | **Endpoint con `any` types** | `strategy-runtime-status.ts:26,64` | **P2** |
| 10 | **`liquidation_worker` y `triangular_worker` gated por env vars** sin documentación visible | `main.rs:91-107` | **P1** (si env no set, worker no arranca silenciosamente) |

---

## 6. Backlog priorizado (P0 / P1 / P2)

### P0 — Rompe operación o doctrina

| ID | Issue | Archivo | Fix mínimo |
|----|-------|---------|------------|
| **P0-1** | Duplicate keys en `liquidation` object | `strategy-runtime-status.ts:212-215` | Eliminar líneas 214-215, mantener `null` en 212-213 |
| **P0-2** | `liquidation.enabled` derivado mal | `strategy-runtime-status.ts:201` | Cambiar a `enabled: false` (config flag) |
| **P0-3** | Strategy naming `triangular` vs `triangular_arb` divergente | múltiples archivos Rust + DB | Decidir UN canonical name y migrar |
| **P0-4** | LendingPositionIndexer NO produce a Redis | `lending_position_indexer.rs` + Redis keys | Verificar si el worker está corriendo; si sí, por qué no escribe |

### P1 — Degrada confiabilidad

| ID | Issue | Archivo | Fix |
|----|-------|---------|-----|
| **P1-1** | `enabled` para dex_arb/triangular_arb hardcoded `true`, liquidation hardcoded `false` (P0-2 lo cubre) | `strategy-runtime-status.ts` | Leer de `trading_config.enabled_strategies` |
| **P1-2** | `\|\| 0` en 4 lugares enmascara `undefined` como `0` | `strategy-runtime-status.ts:173,176,177,205` | Cambiar a `?? null` |
| **P1-3** | `lending_watchlist_size` no es el tamaño de la watchlist, es contador de scan | `strategy-runtime-status.ts:194,210` | Renombrar campo o conectar al indexer real |
| **P1-4** | `legacy_*_worker_enabled` gates sin visibilidad | `main.rs:91-107` | Exponer estado de gate en heartbeat o `/readiness` |
| **P1-5** | Mempool decoded_ok=2 de 761 received | `scanner.rs` + decoder | Diagnosticar por qué 99.7% drop |
| **P1-6** | Token allowlist 4 tokens → 100% gate drops | `trading_config.allowed_token_symbols` | Expandir o cambiar a allowlist-by-risk-score |

### P2 — Deuda técnica

| ID | Issue | Archivo | Fix |
|----|-------|---------|-----|
| **P2-1** | Redis `KEYS` en lugar de `SCAN` | `strategy-runtime-status.ts:41-46` | Reemplazar con `SCAN cursor`-based helper |
| **P2-2** | `triangular.pool_index_entries` duplica `triangular_pool_map_entries` | `strategy-runtime-status.ts:170-171` | Eliminar uno |
| **P2-3** | `liquidation.indexed_positions` duplica `lending_watchlist_size` | `strategy-runtime-status.ts:210-211` | Eliminar o usar otra fuente |
| **P2-4** | `let heartbeat: any` + `Record<string, any>` | `strategy-runtime-status.ts:26,64` | Tipos explícitos |
| **P2-5** | Endpoint sin tests | `tests/` | Añadir `strategy-runtime-status.test.ts` |

---

## 7. Plan en 3 fases

### Fase 1 — Observabilidad honesta (2-3 horas, sin tocar Rust)

**Objetivo**: que `runtime-status` refleje la verdad, sin invenciones.

1. Resolver **P0-1** (duplicate keys) — 1 commit, 4 líneas.
2. Resolver **P0-2** (`liquidation.enabled` config flag) — 1 commit, 1 línea.
3. Resolver **P1-2** (`?? null` cleanup) — 1 commit, 4 líneas.
4. Resolver **P1-3** (renombrar `lending_watchlist_size`) — coordinado con frontend (NO HOY).
5. Build api-server + verificar runtime-status JSON match con expectativa.

**Output esperado**: endpoint `/api/v1/strategies/runtime-status` con cero conflateos, cero duplicate keys, cero `|| 0` masking.

### Fase 2 — Hardening de dataflow (1 día, toca Rust con cuidado)

**Objetivo**: que workers de strategies no-dex_arb realmente se invoquen.

1. Diagnosticar **P0-3** (strategy naming): decidir canonical (`triangular_arb` recomendado por consistency con catalog/runtime). Migrar `persistence.rs:14` + `triangular_worker.rs:78` + `trading_config.enabled_strategies` en DB.
2. Diagnosticar **P0-4** (LendingPositionIndexer): verificar worker enabled + dependencias RPC. Si funciona pero no escribe, encontrar dónde se pierde.
3. Diagnosticar **P1-5** (decoded_ok 2/761): logging detallado en `route_decoder.rs` para identificar por qué se descartan txs. Probablemente decoded como swaps válidos pero hop count o calldata shape no match.
4. Diagnosticar **P1-6** (token allowlist): operativamente, expandir cautelosamente (¿agregar top 20 tokens por TVL?) Y/O cambiar a allowlist-by-risk-score.

### Fase 3 — Validación operativa (1 semana paper-trade)

**Objetivo**: confirmar que cada strategy produce candidatos o reportar honestamente por qué no.

1. Tras Fase 2, deployar con paper trade activo.
2. Esperar 24h de shadow paper trading.
3. Verificar PG:
   ```sql
   SELECT strategy_kind, COUNT(*), MAX(detected_at)
   FROM opportunities
   WHERE detected_at > NOW() - INTERVAL '24 hours'
   GROUP BY strategy_kind;
   ```
4. **Criterio de éxito**: las 4 estrategias aparecen en el GROUP BY. Si alguna queda en 0, hay un bloqueador real (no mercado, no infra).
5. Si dex_arb sigue 100% rejected con `safety_below_threshold`, decidir: ¿ajustar threshold o expandir universe?

---

## 8. Comandos exactos para reproducir cada verificación

### A — Estrategias activas / catálogo
```bash
ssh arbx 'curl -s "http://localhost:8080/api/v1/strategy-catalog" | jq ".entries[] | {kind, lifecycle_status, is_implemented}"'
ssh arbx 'curl -s "http://localhost:8080/api/v1/strategy-catalog/active?chain_id=1" | jq'
ssh arbx 'docker exec -i arbitragex-v2-redis-1 redis-cli GET "arbx:trading_config:1" | jq ".enabled_strategies, .strategy_configs | keys"'
```

### B — Runtime status
```bash
ssh arbx 'curl -s "http://localhost:8080/api/v1/strategies/runtime-status?chain_id=1" | jq'
ssh arbx 'docker logs arbitragex-v2-api-server-1 --since 10m 2>&1 | grep -E "strategy_status\.(pg|redis)_failed"'
```

### C — Opportunities live por estrategia
```bash
ssh arbx 'curl -s "http://localhost:8080/api/v1/opportunities/live?viable_only=false&max_age_seconds=600" | jq ".items | group_by(.strategy_kind) | map({kind: .[0].strategy_kind, count: length, viable: map(select(.rejection_reason==null)) | length})"'

# SQL directo:
ssh arbx 'docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c "SELECT strategy_kind, COUNT(*) AS rows, COUNT(*) FILTER (WHERE rejection_reason IS NULL) AS viable, MAX(detected_at) AS last FROM opportunities WHERE chain_id=1 AND detected_at > NOW() - INTERVAL '"'"'1 hour'"'"' GROUP BY strategy_kind;"'
```

### D — DEX/pool coverage en Redis
```bash
ssh arbx 'for p in "arbx:pool_index:1:*" "arbx:pool_index_v3:1:*" "arbx:v3_slot0:1:*" "arbx:pool_reserves:1:*"; do echo "=== $p ==="; docker exec -i arbitragex-v2-redis-1 redis-cli --scan --pattern "$p" | wc -l; done'
```

### E — Searcher heartbeat (engine invocation)
```bash
ssh arbx 'docker exec -i arbitragex-v2-redis-1 redis-cli GET "arbx:heartbeat:scanner:1:latest" | jq "{period_secs, pending_received, decoded_ok, gate_token_not_allowed, passed_all_gates, db_persisted, triangular_cycles_scanned, flashloan_arb_pairs_scanned, liquidation_positions_scanned}"'
```

### F — Rejection breakdown últimas 24h
```bash
ssh arbx 'docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c "SELECT rejection_reason, COUNT(*) FROM opportunities WHERE chain_id=1 AND detected_at > NOW() - INTERVAL '"'"'24 hours'"'"' GROUP BY rejection_reason ORDER BY COUNT(*) DESC;"'
```

### G — Auditar archivo runtime-status para bug duplicate keys
```bash
grep -n "impacted_lending_positions_1h\|hf_below_one_count\|enabled: liqInvoked\|enabled: false" backend/api-server/src/routes/strategy-runtime-status.ts
```

### H — Verificar workers spawned
```bash
ssh arbx 'docker logs arbitragex-v2-searcher-rs-1 --since 5m 2>&1 | grep -E "triangular_worker|flashloan_arb_worker|liquidation_worker|price_worker|heartbeat_worker" | head -20'
```

---

## 9. Apéndice: evidencia (outputs reales capturados 2026-05-12 ~17:59 UTC)

### 9.1 Runtime-status JSON actual (producción)
```json
{
  "chain_id":1, "window_seconds":3600,
  "source":{"postgres":"ok","redis":"ok","logs":"not_used"},
  "strategies":[
    {"strategy_kind":"dex_arb","enabled":true,"engine_loaded":true,"engine_invoked":true,
     "last_invoked_at":"2026-05-12T17:59:06.000Z","candidates_1h":102,"rejections_1h":102,
     "last_rejection_reason":"safety_below_threshold","paper_viable_1h":0,"paper_rejected_1h":102},
    {"strategy_kind":"triangular_arb","engine_invoked":false,"candidates_1h":0,
     "data_dependencies_status":"armed_waiting_for_impact",
     "triangular_pool_map_entries":46,"pool_index_entries":46,"reserves_cache_entries":60,
     "impacted_cycles_last_1h":null,"cycles_with_missing_reserves":0},
    {"strategy_kind":"flashloan_arb","engine_invoked":false,"candidates_1h":0,
     "base_candidates_seen_1h":0,"wrapped_candidates_1h":0,
     "no_provider_rejections":0,"negative_after_fee_rejections":0},
    {"strategy_kind":"liquidation","enabled":false,"engine_invoked":false,
     "data_dependencies_status":"missing_lending_watchlist",
     "lending_watchlist_size":0,"indexed_positions":0,
     "impacted_lending_positions_1h":0,"hf_below_one_count":0,"liquidation_candidates_1h":0}
  ]
}
```

### 9.2 Heartbeat Redis raw (último período 60s)
```json
{"period_secs":60,"emitted_at_unix":1778608746,
 "pending_received":761,"decoded_ok":2,"enriched_v2":0,"enriched_v3":0,
 "gate_token_not_allowed":2,"gate_strategy_disabled":0,"gate_no_config":0,
 "gate_unknown_token_price":0,"gate_anomalous_math":0,"gate_other_rejected":0,
 "passed_all_gates":0,"db_persisted":2,"db_errors":0,
 "triangular_cycles_scanned":0,"triangular_opps_emitted":0,
 "flashloan_arb_pairs_scanned":0,"flashloan_arb_opps_emitted":0,
 "flashloan_arb_sanity_reject":0,
 "liquidation_positions_scanned":0,"liquidation_opps_emitted":0,
 "liquidation_sanity_reject":0}
```

### 9.3 PG opportunities últimas 24h (GROUP BY strategy_kind)
```
 strategy_kind | rows | viable | rejected |           last_seen
---------------+------+--------+----------+-------------------------------
 dex_arb       | 5003 |      0 |     5003 | 2026-05-12 17:59:58.609435+00
```

### 9.4 PG rejection_reason últimas 1h
```
    rejection_reason    | count
------------------------+-------
 safety_below_threshold |   112
```

### 9.5 Trading config (parcial — sin secrets)
```
enabled_strategies: ["dex_arb_v2v2","dex_arb","flashloan_arb","liquidation","triangular"]
allowed_token_symbols: ["WETH","USDC","USDT","WBTC"]
min_profit_usd: 2, min_roi_pct: 0.3, max_token_risk_score: 1
enabled_dex_ids: null
strategy_configs.dex_arb.enabled: true (min_profit_usd: 10, min_roi_pct: 2)
strategy_configs.triangular.enabled: true (min_profit_usd: 20, min_roi_pct: 2)
```

### 9.6 Redis key counts (chain_id=1)
- `arbx:pool_index*`: 46 keys
- `arbx:v3_slot0:*`: 34 keys
- `arbx:pool_reserves:*`: 26 keys

### 9.7 Citas de código clave

**Bug duplicate keys** — `strategy-runtime-status.ts:199-217`:
```typescript
const liquidation = {
  strategy_kind: "liquidation",
  enabled: liqInvoked || dbStats.liquidation.candidates_1h > 0,  // ← P0-2 conflated
  ...
  impacted_lending_positions_1h: null,                            // ← line 212 (curated)
  hf_below_one_count: null,                                        // ← line 213 (curated)
  impacted_lending_positions_1h: heartbeat.liquidation_positions_scanned ?? null,  // ← line 214 (DUP)
  hf_below_one_count: dbStats.liquidation.candidates_1h,           // ← line 215 (DUP)
  liquidation_candidates_1h: dbStats.liquidation.candidates_1h,
};
```

**Strategy naming divergence**:
- `persistence.rs:14`: `StrategyKind::Triangular => "triangular"`
- `triangular_worker.rs:78`: `const STRATEGY_KIND: &str = "triangular"`
- `strategy_label.rs:114`: `StrategyLabel::TriangularArb => "triangular_arb"`
- `trading_config.enabled_strategies` (Redis): `["..., "triangular"]`

**Workers spawn gating**:
- `main.rs:91`: `fn legacy_triangular_worker_enabled() -> bool`
- `main.rs:107`: `fn legacy_liquidation_worker_enabled() -> bool`
- `main.rs:382`: `if legacy_triangular_worker_enabled() { ... }`

---

## 10. Diffs mínimos sugeridos (SIN APLICAR — esperando autorización)

### Diff P0-1 + P0-2 — Fix duplicate keys + restore `enabled: false`

```diff
--- a/backend/api-server/src/routes/strategy-runtime-status.ts
+++ b/backend/api-server/src/routes/strategy-runtime-status.ts
@@ -198,7 +198,7 @@ export function mountStrategyRuntimeStatus(
       const liquidation = {
         strategy_kind: "liquidation",
-        enabled: liqInvoked || dbStats.liquidation.candidates_1h > 0,
+        enabled: false,
         engine_loaded: engineLoaded,
         engine_invoked: liqInvoked,
@@ -211,8 +211,6 @@ export function mountStrategyRuntimeStatus(
         indexed_positions: lendingWatchlistSize,
         impacted_lending_positions_1h: null,
         hf_below_one_count: null,
-        impacted_lending_positions_1h: heartbeat.liquidation_positions_scanned ?? null,
-        hf_below_one_count: dbStats.liquidation.candidates_1h,
         liquidation_candidates_1h: dbStats.liquidation.candidates_1h,
       };
```

### Diff P1-2 — `?? null` cleanup (3 lugares)

```diff
@@ -173,3 +173,3 @@
-        cycles_with_missing_reserves: heartbeat.triangular_v3_quote_failures || 0,
+        cycles_with_missing_reserves: heartbeat.triangular_v3_quote_failures ?? null,

@@ -176,3 +176,3 @@
-        base_candidates_seen_1h: baseCandidates,
+        base_candidates_seen_1h: heartbeat.flashloan_arb_pairs_scanned ?? null,

@@ -177,3 +177,3 @@
-        wrapped_candidates_1h: wrappedCandidates,
+        wrapped_candidates_1h: heartbeat.flashloan_arb_opps_emitted ?? null,
```

(Notas: las variables locales `baseCandidates`/`wrappedCandidates` se usan también en `flDataStatus`. Solo cambiar el sitio del JSON output, no la variable local.)

---

## 11. Lo que NO se pudo verificar (etiquetado honestamente)

- **NO VERIFICADO**: si `legacy_triangular_worker_enabled` y `legacy_liquidation_worker_enabled` están retornando `true` o `false` en producción. Requiere lectura de logs específicos del último restart del searcher-rs.
- **NO VERIFICADO**: si `pool_discovery.rs` discovery dinámico está realmente invocándose o si los 46 pool_index entries son estáticos.
- **NO VERIFICADO**: distribución real de `token_risk_score` en la tabla `tokens` — requiere query directa con scope >1 hora.
- **NO VERIFICADO**: si el `LendingPositionIndexer` está spawn-eado o gated. Requiere `docker logs --since 1h | grep lending_position`.
- **NO VERIFICADO**: si `enabled_dex_ids: null` en config se interpreta como "todos" o "ninguno" en `scanner.rs`. Requiere lectura adicional de `scanner.rs:1394+` y test de comportamiento.

Estos puntos se pueden cerrar con comandos del §8 ejecutados en una iteración posterior.

---

*Auditoría producida sin alterar código, sin tocar VPS productivo (más allá de lecturas via SSH), sin deploy, sin mocks. Toda la evidencia es citable a archivos y líneas reales o salidas reales del runtime en producción.*
