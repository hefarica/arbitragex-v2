# HERMES-RUN7 — Motor de Rutas Exóticas (PC-08 parte 2)

Fecha: 2026-09-19 · Modo: READ-ONLY (sin modificaciones de código; RULE 00 / R8 respetados)
Repo: `C:/Users/HFRC/Desktop/arbitragex-v2-main (17)` · branch `feat/s1-fee-dual-unit-20260918` @ `84407fda`
Graph Intelligence: PASS · `graph_sha256=e9b97fd8cdfd8bb5e8e51b3ac2355b43338e6d6a1611e2bceefd4fbcb7f60e93`

> Veredicto ejecutivo: el repo YA tiene un motor multi-hop 2–7 hops con presupuesto de trabajo
> y un worker por bloque (RU-3) que está **apagado por default**. El 80% de la familia "rutas
> exóticas" multihop es encender y afinar lo existente, no escribir un motor nuevo. Los quick-wins
> correctos son: (1) encender RU-3, (2) priorizar el escaneo por pools sucios (ventana temporal),
> (3) banda long-tail de liquidez. V4/flash-accounting es el único ítem que exige proyecto nuevo.

---

## 0. Mapa del route-discovery actual (verificado contra código)

Pipeline vivo (dos consumidores del mismo kernel):

```
A) Radar shadow (12s tick) — ARBX_ROUTE_DISCOVERY_MODE=shadow (default off)
   route_discovery_worker.rs (2,614 líneas) · evaluate_tick pura
B) RU-3 por bloque — ARBX_ROUTE_SCANNER_MODE=on (default off)          ← EL SLEEPING GIANT
   workers/route_scanner_worker.rs (1,328 líneas)
   scanner.rs:1042-1053 (spawn) · newHeads → rebuild graph → cycles → RouteIntent
```

Kernel compartido:

| Pieza | file | Rol | Estado |
|---|---|---|---|
| `build_graph` | graph_builder.rs:460-540 | pools → 2 aristas dirigidas/pool (V2 reservas, V3 slot0) | vivo |
| `build_edges_for_pool` | graph_builder.rs:316-451 | peso `−ln((1−fee)·rate)` V2 y V3 (RU-2) | vivo |
| `find_profitable_cycles(_with_limits)` | multi_hop_search.rs:135-230 | DFS acotado 2–7 hops, Σlog_weight<0, presupuesto `max_edge_visits` (100k default) + deadline cooperativo | vivo |
| `build_line_graph` (MMBF) | multi_hop_search.rs:42-75 | line-graph L(G) para MMBF (arXiv:2406.16573) | **latente: cero callers fuera de tests** |
| `anchor_verdict` | route_scanner_worker.rs:185-192 | h≤3 dispatch · h4-5 ≥1 anchor · h6-8 ≥2 anchors→SHADOW | vivo (RU-3) |
| dirty pools | dirty_signal.rs / dirty_consumer.rs | SET `arbx:dirty_pools:<chain>` TTL 30s; consumer drena por tick | vivo (drain) pero **re-eval gate default OFF** (ARBX-QB-05-009, patrón observe-only) |
| `V3Slot0Entry` | reserves.rs:128-137 | cachea sqrtPriceX96 + liquidity **sin tick** (el writer pool_sync_worker.rs:85-97 recibe 7 ABI-words y descarta el int24) | gap F2 |

Rechazos del graph builder (los "asesinos" de hoy): `missing_reserves/stale_*`, `missing_slot0`
(incluye `liquidity==0`, graph_builder.rs:379-381), `low_liquidity` (gate `min_liquidity_hint`,
graph_builder.rs:413-415 — YAML en `backend/searcher-rs/config/strategies/route_applicability.yaml:21`
lo puso en **1.0** desde PR-ROUTE-02; el default embebido es 0.0, strategy_applicability.rs:107),
`unsupported_protocol` (Curve/Balancer/Unknown, graph_builder.rs:408-410), `non_hot_token_edge`
(solo si `hot_token_only=true`; default off).

Nota de.branch: en ESTE checkout el bug E0004 (fee V3 dividido entre 10_000) YA está corregido
por S1 fee dual-unit (commit `cdb4c890`, `fee_fraction` con divisor 1e6 para V3 en
quote_anchor_runtime.rs:119-135). La memoria del defecto abierto corresponde a otra rama.

Presupuesto/tunables RU-3 (route_scanner_worker.rs:68-99): `ARBX_ROUTE_SCANNER_MAX_ROUTES_PER_BLOCK=500`,
`MAX_SCAN_MS=250` (p95), `MAX_HOPS=7` (clamp 2..=7 en multi_hop_search.rs:158),
`CANONICAL_PER_BLOCK=25`, anchors default {WETH,USDC,USDT,DAI,WBTC}.

---

## 1. Familias exóticas — algoritmo, inyección, costo, gates

### F1 — Multi-hop >3 con poda inteligente

**Estado**: el motor EXISTE y enumera 2–7 hops con Σlog_weight<0, dedup por rotación, contadores
honestos (`capped`, `dropped_for_cap`, `noise_dropped`≤1e-6). Nadie lo corre en prod porque
`ARBX_ROUTE_SCANNER_MODE` default `off`. Lo que NO existe: poda por cota inferior (el DFS no
descarta ramas cuya suma parcial ya no puede cerrar en negativo), ni prioridad por yield.

**Poda inteligente a añadir (incremento, no reescritura)** — en `CycleSearch::dfs`
(multi_hop_search.rs:265-388):

```
# Poda A — cota inferior de cierre (admissible lower bound):
#   min_negative_weight[t][h_restante] = menor suma posible de (max_hops-h) pesos
#   saliendo del token t. Precomputado 1 vez por bloque:
#   best_exit[t] = min sobre aristas (t→u) de weight; bound(t, k) ≈ k · min_global_weight
#   (min_global_weight = min weight de TODAS las aristas, O(E) una vez).
#   En el dfs: if sum + (max_hops - depth) * min_global_weight >= -epsilon: return
#   → poda exacta (nunca descarta un ciclo negativo) porque cualquier extensión
#     suma ≥ k·min_global_weight.

# Poda B — orden de aristas por peso ascendente en cada nodo:
#   la rama más prometedora se explora primero; con max_cycles pequeño
#   el cap se llena con los MEJORES ciclos, no con los primeros.

# Poda C — subgrafos por banda (ver F4): segunda pasada solo-tail.
```

**Inyección**: multi_hop_search.rs:289 (loop `for index in graph.out_edge_indices(&current)`)
para poda A/B; el precompute en `find_profitable_cycles_with_limits` tras ordenar starts (:198-204).

**Costo**: poda A = O(E) precompute + O(1) por nodo del DFS (una comparación); poda B = O(deg·log deg)
por nodo. Red esperada de `edge_visits` 3–10× (libera presupuesto para profundidad real 6–7).
Riesgo: cero sobre exactitud si `min_global_weight` se calcula sobre pesos finitos existentes
(los `None` ya se saltan, multi_hop_search.rs:311-314).

**Gate que lo mata hoy**: (1) `ARBX_ROUTE_SCANNER_MODE=off`; (2) `anchor_verdict` manda h6-7 a
ShadowForced (no dispatch) — rutas 6-7 solo se ven, no se evalúan; (3) el gate de net-profit
≥3×gas del cartridge mata ciclos largos con muchas fee-tiers encadenadas (4 legs × 30bps = 1.2%
de fee solamente); (4) XLS-QB-05-009: re-eval dirty default OFF.

### F2 — Liquidez concentrada V3 fuera de rango / rango delgado

**Reencuadre honesto (agent-math)**: una ruta que CRUZA un rango sin liquidez no es arbitrable
por nosotros — es una avalancha de slippage que OTROS pueden arbitrar. El edge exótico real es
detectar **rango delgado activo** (liquidity > 0 pero bajo tras un swap grande) donde el precio
de ese pool quedó desviado vs el resto del mercado → ciclo corto contra ese pool con sizing
calibrado al inventario real del rango. Los "ticks lejanos = competencia cero" son el caso
límite: pools cuyo activo quedó atrapado fuera de rango cotizan precio fantasma (sqrtPrice
congelado) — señal de REVERSION cuando algo los vuelva a rango (sync event / mint LP).

**Algoritmo**:

```
# Etapa 1 — dato: persistir tick en V3Slot0Entry (el multicall del writer YA lo recibe:
#   slot0 devuelve 7 words, word2 = tick actual int24; pool_sync_worker.rs:85-97 lo descarta).
#   Campo nuevo opcional `tick: Option<i32>` — sin RPC adicional (RULE 00: dato real, no inferido).

# Etapa 2 — clasificación de arista V3 (en build_edges_for_pool, graph_builder.rs:370-407):
#   depth_usd = liquidity_hint     # ya normalizado
#   if liquidity == 0        → REJECT missing_slot0 (hoy, correcto: precio fantasma)
#   elif liquidity_hint < thin_threshold  → edge.tag = ThinRange (NO rechazar; hoy
#        muere en el gate min_liquidity_hint=1.0 del YAML)
#   else                     → edge normal
#   Con tick persistido: tick_distance = |tick − nearest_initialized_boundary|
#        (requiere tick_bitmap → SOLO fase 2; no bloquear el quick-win en él).

# Etapa 3 — scoring: thin_range_rank = |Δln(rate_pool vs rate_referencia_par)|
#   donde rate_referencia = mediana de las demás pools del par (dense_pair_edges,
#   graph_builder.rs:264-269 da las pools paralelas en O(1)). El ciclo que incluye
#   una arista ThinRange con dispersión grande = candidato exótico prioritario.

# Etapa 4 — sizing honesto: v3_amount_out_single_tick (amm_math.rs:134) falla
#   exactamente por diseño en rango delgado → usar QuoterV2 multicall
#   (amm_math.rs:401 v3_quote_exact_in_multicall) que camina ticks reales.
```

**Inyección**: graph_builder.rs:413-415 (banda ThinRange en vez de reject), route_scanner_worker
`evaluate_scan` (:316-359) para el boost de prioridad.

**Costo**: etapa 1-3 ≈ O(P_v3) comparaciones extra por bloque (trivial); etapa 4 = 1 multicall
RPC por candidato top-K (ya existe la utilidad; ~50-100k gas de quota por quote, gratis via
QuoterV2 view). 

**Gate que lo mata hoy**: `min_liquidity_hint: 1.0` del YAML (mata TODA la banda thin);
`liquidity==0` → reject (correcto); sin tick cacheado no hay "ticks lejanos" computables;
slippage 0.5% y 3×gas del risk-management matan la mayoría de trades tail (honesto: muchos
candidatos thin NO son ejecutables — por eso el entregable es radar + quota, no autodispatch).

### F3 — V2+V3+V4 con flash accounting

**Estado**: V4 NO EXISTE en el repo. `ProtocolType` = {V2,V3,Curve,Balancer,Unknown}
(route_intent.rs:219-230); grep de `v4|PoolManager|flash_accounting` en backend = 0 hits.
graph_builder.rs:408-410 rechaza cualquier protocolo fuera de V2/V3. No hay indexer de
PoolManager, ni StateView, ni calldata para V4 actions, ni flash-loan accounting en el ejecutor.
Lo más cercano: flashloan_arb_worker (V2 Aave, §IV) y el cartridge flashloan_atomic/flashmint.

**Algoritmo (proyecto, no quick-win)**:

```
# F3.1 Indexación: subscribir Mint/Burn/Swap de PoolManager (eventos V4) → pools por
#      (currency0, currency1, hooks, fee) → tabla PG + cache Redis (análogo a pool_sync).
# F3.2 Pricing: StateView.slot0(poolId) multicall → misma fórmula −ln((1−fee)·rate).
#      ProtocolType::V4 como variante nueva → graph_builder admite sus aristas.
#      Flash accounting = el ciclo se cierra EN EL HOOK del bundle: flashLoan(currency,
#      amount) → swaps → settle() con saldo neto ≥ 0; sin capital propio.
# F3.3 Rutas: el MISMO find_profitable_cycles corre sobre el grafo extendido sin cambios
#      (V4 es solo un proveedor de aristas más) — esta es la belleza del diseño actual.
# F3.4 Ejecución: Universal Router commands (EXECUTE) o bundle directo a PoolManager
#      unlock(); calldata nuevo en calldata/ (hoy universal_router.rs no emite V4 actions).
# F3.5 Sim: revm con el estado del PoolManager (ítem más caro: deploy fork + IStateView).
```

**Inyección**: route_intent.rs:219 (enum), graph_builder.rs:337 (match protocolo), pool_sync
(writer), calldata/, sim. Es el único ítem que toca TODAS las capas (R7 trazabilidad completa).

**Costo**: semanas, no días. Computacional en runtime: +25-40% del universo de aristas por
chain con V4 desplegado; el presupuesto del DFS absorbe con poda F1.

**Gate que lo mata hoy**: TODO — sin dato V4 no hay nada que podar. Además: riesgo de hooks
(hook-reentrancy y fees dinámicas rompen el peso estático `−ln((1−fee)·rate)`; un hook con fee
variable hace la arista NO-computable → debe rechazarse fail-honest, no estimarse). Competencia
real en V4 mainnet ya existe (no es tierra virgen); el edge es en L2s con V4 reciente.

### F4 — Pares de baja liquidez con spread alto (long-tail)

**Estado**: el YAML (PR-ROUTE-02) subió `min_liquidity_hint` 0.0→1.0 precisamente para matar
"wash 2-hop cycles y bowtie 4-hop noise" — la decisión fue correcta para el feed canonical,
pero tiró la criba con el bebé: la banda [0.02, 1.0) normalizados contiene pares reales con
spread alto que los searchers grandes ignoran por size mínimo.

**Algoritmo**:

```
# Segunda pasada TAIL por bloque (o cada N bloques), aislada del feed canonical:
#   tail_cfg = GraphBuildConfig { min_liquidity_hint: 0.02, hot_token_only: false, ... }
#   tail_graph = build_graph(redis, chain, pools, now, tail_cfg)   # mismo código, otro cfg
#   tail_result = find_profitable_cycles(tail_graph, 2, 5, cap=64)  # hops ≤5: en tail
#        los ciclos largos son puro ruido; cap pequeño
#   tag telemetry: band="tail" · family="long_tail_spread"
#   ADMISIÓN: solo ≥1 anchor (mismo criterio RU-3 h4-5) + dispersión de par ≥ X bps
#        (dispersión ya computable: quote_anchor_runtime pair_rates, :168-176)
#   NUNCA al feed canonical ni a opps:detected — telemetry shadow hasta que el
#        operador apruebe banda de ejecución con size mínimo propio.
```

**Inyección**: route_scanner_worker.rs tras `evaluate_scan` (:359) — segunda llamada con cfg
tail; o en route_discovery_worker evaluate_tick si se prefiere el ciclo de 12s.

**Costo**: 1 build_graph extra (~2N gets Redis por bloque — caro si es por bloque; hacer cada
3-5 bloques o reusar el build principal con post-filtro por liquidity_hint, que es GRATIS:
las aristas ya cargan el hint). DFS sobre subgrafo tail: típicamente <10% de edges del grafo
completo. Con reuso del build: costo marginal ≈ 0.

**Gate que lo mata hoy**: `min_liquidity_hint=1.0`; 3×gas (un trade tail de $200 con gas
$15 necesita $45 de edge — 22% de spread: casi imposible salvo pegón real); slippage 0.5%
con liquidity_hint bajo = rechazo casi seguro en sim. Conclusión honesta: F4 es radar de
OPORTUNIDAD ACUMULADA (señales para sizing manual/semiauto) antes que auto-trading.

### F5 — Ventanas temporales (reorg de precios tras sync events)

**Estado**: la infraestructura de señal YA EXISTE y está viva: `arbx:dirty_pools:<chain>`
(SET Redis, TTL 30s, writer pool_sync_worker marca pools cuyo refresh CAMBIÓ estado;
consumer drena en cada tick de discovery). El block_scanner (ARBX_MEMPOOL_MODE=block)
decodifica Swap V2 confirmado → RouteIntent. Lo que falta: CONECTAR la señal dirty a la
PRIORIDAD del escaneo por bloque (hoy el DFS ordena starts por incident-pools desc,
multi_hop_search.rs:198-204, ciego a qué se movió).

**Algoritmo**:

```
# Ventana exótica: los primeros ~1-2 bloques tras un swap grande, el pool origen
# queda desviado vs el mercado antes de que los bots "normales" re-equilibren.
# El dirty set ES el índice de esa ventana.
#
# priority_starts = tokens incidentes a dirty_pools (drenado NO-destructivo
#   SMEMBERS, dirty_consumer.rs ya lo hace)
# orden DFS: (1) starts en priority_starts (por incident pools desc),
#            (2) resto (orden actual)
# budget split: 70% del max_edge_visits a la partición (1), 30% a (2)
# bonus re-eval: ciclo ya visto que toca un pool dirty → re-admitir UNA vez
#   por state_version (el consumer ya trae AlreadyDirty coalescing)
```

**Inyección**: multi_hop_search.rs:198-228 (ordenación de starts — aceptar un conjunto
`priority: &HashSet<Address>` opcional); route_scanner_worker.rs run_loop pasa el drain del
SET como priority. Alternativa sin tocar el kernel: dos llamadas a `find_profitable_cycles`
con subgrafo inducido dirty-primero.

**Costo**: SMEMBERS 1/tick (ya ocurre); sort extra O(T log T); partición de presupuesto O(1).
Marginal: cero. Payoff: el DFS gasta sus 100k visitas donde el mercado ACABA de moverse.

**Gate que lo mata hoy**: ARBX_ROUTE_SCANNER_MODE=off; XLS-QB-05-009 re-eval default OFF
(obstáculo solo para re-admisión, no para prioridad); block_scanner decodifica Swap V2
únicamente (V3 Swap decode es follow-up declarado, block_scanner.rs:23) — la ventana V3
llega igual por slot0 refresh (dirty), un bloque más tarde.

---

## 2. Priorización (ganancia esperada × factibilidad)

| # | Familia | Ganancia esperada | Factibilidad | Score | Justificación |
|---|---|---|---|---|---|
| 1 | F5 ventanas dirty | Alta — edge temporal real, datos ya fluyen | Muy alta (días) | ★★★★★ | Señal existe; solo falta ordenar el DFS |
| 2 | F1 multihop on + poda | Alta — cobertura 4-7 hops de lapso | Muy alta (día: encender; semana: poda) | ★★★★☆ | Motor terminado y testeado, apagado |
| 3 | F4 long-tail | Media — spread alto, size chico | Alta (días) | ★★★☆☆ | Reuso total de build_graph; limitado por 3×gas |
| 4 | F2 V3 thin-range | Media — sizing exacto necesario | Media (1-2 semanas; tick persist + banda) | ★★★☆☆ | Valor mayor como radar de reversión |
| 5 | F3 V4 flash-accounting | Potencial alta (competencia baja en L2-V4) | Baja (semanas, todas las capas) | ★★☆☆☆ | Proyecto estructural; requiere quorum de spec |

---

## 3. Los 3 quick-wins de esta semana (landing order)

**QW1 — Encender RU-3 (martes)**: `ARBX_ROUTE_SCANNER_MODE=on` en el env del VPS (deploy por
el flow RULE 01; es cambio de CONFIG, no código). Ajustes sugeridos primer vuelo:
`ARBX_ROUTE_SCANNER_CANONICAL_PER_BLOCK=10` (conservador, sube después con evidencia),
`MAX_HOPS=5` mientras se valida el anchor gate, resto defaults. Verificación: telemetría
`route_scanner.done` por bloque + `arbx:controlboard:route_scanner:hb` + conteo de intents
`multihop_negcycle` en el orchestrator. Riesgo: consumo CPU acotado por 250ms/block — si p95
excede, `MAX_SCAN_MS` lo reporta honesto (R8), bajar cap. NO requiere quorum (no es cambio
estructural: el worker ya pasó su propio review; es activación documentada del operador).

**QW2 — Prioridad dirty en el escaneo (jueves)**: parche mínimo — `find_profitable_cycles_with_limits`
acepta `priority_starts: Option<&[Address]>` (orden de starts), route_scanner_worker drena
`arbx:dirty_pools:<chain>` (SMEMBERS no destructivo, patrón ya probado en dirty_consumer) y
pasa los tokens incidentes como priority. Tests: mismo set de ciclos con/sin priority cuando
no hay dirty (invariante), prioridad efectiva cuando hay dirty. ~80 líneas + tests. Va por
branch propio + CI (cargo test -p searcher-rs).

**QW3 — Banda tail como telemetry (viernes)**: reuso del build principal: post-filtro de aristas
por `liquidity_hint ∈ [0.02, 1.0)` SOBRE el grafo ya construido (no segunda tanda de Redis),
segunda llamada del finder con `cap=64, max_hops=5`, telemetría `family=long_tail_spread`,
`band=tail`, shadow-only hard-coded (nunca opps:detected — NO-ACTIVE preservado). Entregable:
census de cuántos pares tail con dispersión >X bps existen por bloque → decide si vale banda
de ejecución propia (decisión de operador CON datos, semana siguiente).

---

## 4. Invariantes respetados (checklist RULE 00 / R8 / NO-ACTIVE)

- Ninguna familia fabrica pesos: `None`/no-finito → skip+count (multi_hop_search.rs:305-315).
- F2 rechaza liquidity==0 (precio fantasma NUNCA entra al grafo); tick sería dato real del
  multicall existente, no inferido.
- F3 hooks con fee dinámica → arista no-computable → reject fail-honest (nunca estimada).
- F4 nunca escribe opps:detected; QW3 shadow-only por diseño.
- Todo quick-win es activación de código YA testeado o parche mínimo con tests nuevos.
- Este documento no modifica código (auditoría read-only); deploy de QW1 es cambio de env
  operador-side.
- Evidencia: cada cita file:line fue verificada con rg/lectura en este checkout (branch
  feat/s1-fee-dual-unit-20260918 @ 84407fda, 2026-09-19).

## 5. Línea final honesta

Ninguna ganancia fue medida en este pase (no hubo simulación ni ejecución — read-only). Las
estimaciones de costo computacional son de orden de magnitud derivadas de los límites del
propio código (`max_edge_visits`, `MAX_SCAN_MS`, caps por bloque). El primer dato real llega
con QW1 encendido: `route_scanner.done.enumeration_ms`, `cycles_found`, `anchor_rejected`
por bloque during 24h — esa telemetría decide la calibración de QW2/QW3.
