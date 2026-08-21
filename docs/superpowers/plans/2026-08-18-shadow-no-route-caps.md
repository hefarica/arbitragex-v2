# PLAN — Shadow jamás capea rutas (SHADOW-NO-ROUTE-CAPS)

> **Directiva operador (2026-08-18):** "El modo shadow no debe tener rutas capadas… para que nunca bajo ninguna condición me capee rutas."
> **Evidencia live al emitir la directiva:** badge rojo `routes capped` + tick real: `routes_capped:true, routes_found:500, routes_dropped_for_cap:13, mode:shadow`.

---

## 1. QUÉ SUCEDE (diagnóstico con evidencia)

### 1.1 El hecho observado
El worker de route discovery corre en `shadow` con TRES límites duros del `dfs_bounded` ([unique_route_finder.rs:37-60](backend/searcher-rs/src/route_discovery/unique_route_finder.rs)):

| Cap | Default | Env en VPS | Efecto |
|---|---|---|---|
| `max_routes_per_tick` | **500** | **no seteado → 500** | Corta la enumeración al emitir la ruta 500 → `routes_capped:true` (el badge) |
| `max_pools_per_pair` | 8 | no seteado | Descarta pools paralelos entre un par de tokens → `pools_truncated` |
| `max_depth` | 3 (built-in) / yaml | **7** | Ciclos >7 hops nunca se exploran |

Tick live capturado (canal `arbx:route_discovery:telemetry`):
```
routes_capped: true · routes_found: 500 · routes_dropped_for_cap: 13 · pools_truncated: false · mode: shadow
```
`routes_found: 500` = **exactamente** el cap → el finder se topo contra el muro ESE tick (y `dropped_for_cap:13` es cota inferior: los subárboles abandonados tras el corte no se cuentan, documentado en el propio R8 del código).

### 1.2 El defecto estructural (peor que el cap)
El loop NO tiene cursor de reanudación: cada tick (12s) el DFS **arranca de nuevo desde el primer token**. Consecuencia: redescubre las mismas ~500 rutas cada tick y **el resto de la topología jamás se explora** — no es "500 por tick rotando", es "las mismas 500 para siempre". El cap no solo trunca: **congela la frontera de exploración**.

### 1.3 Por qué existen los caps (honestidad de ingeniería)
DFS no acotado sobre un grafo que crece (PR #410 lleva la cobertura de 166 → miles de pools) es autonegación de servicio combinatoria. La directiva "nunca capear" NO puede implementarse como "DFS infinito": se implementa como **"ninguna ruta se pierde jamás por un cap"** — presupuesto con continuación exacta. La distinción es la esencia del plan.

---

## 2. INVENTARIO COMPLETO DE CPEATORES DE RUTAS (auditoría 0-100)

| # | Mecanismo | Dónde | Clase | Trato en este plan |
|---|---|---|---|---|
| C1 | `max_routes_per_tick=500` | unique_route_finder | **Presupuesto** | F1: Defer-never-drop |
| C2 | Restart sin cursor por tick | route_discovery_worker loop | **Pérdida estructural** | F1: cursor de continuación |
| C3 | `max_pools_per_pair=8` | collect_out_edges | Presupuesto (branching) | F2: rotación justa entre ticks |
| C4 | `max_depth=7` | dfs depth check | **Completitud por profundidad** | F3: iterative deepening completo 2..N |
| C5 | `route_shape_out_of_bounds` (cartuchos, n exacto por estrategia) | mev_*.rhai | Definición de estrategia (cuadrangular=4 por definición) | Fuera de alcance — no es cap, es shape-matching del strategy |
| C6 | `ArbitrageExecutor: UnsupportedRouteLength(>2)` | contrato Solidity | **Realidad on-chain de EJECUCIÓN** (no discovery) | Fuera de alcance — documentar frontera; shadow/sim no pasa por ahí |
| C7 | MVP_CYCLES hardcoded (legacy V1) | triangular_worker | Deuda legacy (C2 de la auditoría vivid-grove) | Ya cubierto por doctrina previa; no tocar aquí |

**Frontera doctrinal explícita:** discovery (shadow) = completitud; execution (live) = viabilidad on-chain. C6 NO se toca: el contrato solo modela seguridad de gasto para 1-2 legs; eso no limita qué rutas se DESCUBREN ni simulan.

---

## 3. DISEÑO — Política `DeferNeverDrop` para shadow

### F1. Presupuesto con continuación exacta (mata C1+C2)
- El finder recibe `budget: usize` (rutas EMITIDAS este tick) y mantiene un **cursor determinista** `(start_token_idx, último estado del DFS)`.
- Al agotar presupuesto: NO resetea. Termina el tick con `deferred: true` + `cursor` persistido en memoria del worker; el próximo tick **continúa exactamente donde cortó** (DFS iterativo con stack explícito en vez de recursión — condición técnica necesaria para resumir).
- Telemetría honesta renombrada: `routes_capped` → **`routes_deferred`** (+ `deferred_cursor`), y el semántico cambia de "perdiste rutas" a "postergadas al próximo tick". El badge frontend muestra `DEFERRED (continúa)` en ámbar, no `capped` en rojo.
- **Garantía:** en K ticks sin cambios de grafo, la enumeración es EXHAUSTIVA (cada ruta candidata aparece al menos 1 vez). Con cambios de grafo, el cursor re-valida contra el grafo vigente (invalidación por versión de grafo — si el grafo cambió, se reinicia el pasaje de profundidad actual, no todo).

### F2. Rotación justa de pools paralelos (mata C3)
- `max_pools_per_pair` deja de ser "primeros 8 para siempre": el conjunto retenido **rota por tick** (round-robin offset = tick % total_pools_del_par). En K ticks, todo pool paralelo participó.
- Telemetría: `pools_truncated` → `pools_rotated` (con `rotation_k`) — la cobertura sigue siendo exhaustiva en el tiempo.

### F3. Completitud por profundidad: iterative deepening (mata C4)
- El loop pasa a ejecutar pasajes **completos por profundidad d=2..max_depth**, un pasaje (d) por tick hasta terminarlo, luego d+1. Ningún ciclo de profundidad ≤ max_depth es inalcanzable.
- `max_depth` en shadow **no puede configurarse por debajo del máximo soportado por la capa de simulación (7)** — ver hardening H2.

### F4. Modo-aware por diseño (la directiva)
- `RouteFinderConfig` gana `policy: enum { BoundedLegacy, DeferNeverDrop }`. Shadow (route discovery + outcomes + outcomes_sink, los 3 env del VPS) → `DeferNeverDrop` SIEMPRE. `BoundedLegacy` queda SOLO para live (donde el cap protege el hot-path de emisión real).
- La asignación es en UN solo sitio (`from_env_and_engine`), sin overrides env que puedan rebajarla en shadow (ver H2).

---

## 4. HARDENING — que NUNCA (bajo ninguna condición) vuelva a capear shadow

- **H1 · Test de exhaustividad (unit, bloqueante en CI):** grafo sintético con rutas > budget; assert: tras K ticks con presupuesto b, la UNIÓN de rutas emitidas == conjunto exhaustivo esperado (calculable en el test), `deferred` apareció ≥1 vez, y ninguna ruta aparece fuera del orden del cursor. Es la prueba matemática de "no se pierde nada".
- **H2 · Gate de política (`gate-shadow-no-route-caps.sh`, wired a `omega8-m3-grep-gates.yml`):**
  1. El constructor de config: `policy = DeferNeverDrop` cuando `mode.starts_with("shadow")` — grep + test de contracto.
  2. **Ningún env puede rebajar**: en shadow, `ARBX_ROUTE_DISCOVERY_MAX_ROUTES_PER_TICK`/`_MAX_POOLS_PER_PAIR` se IGNORAN para presupuesto de pérdida (solo ajustan el granularity del defer) y `MAX_DEPTH` < 7 en shadow → **CI rojo** (el gate lee el default y los compose/env del repo).
  3. El símbolo `routes_capped` no puede reaparecer como semántica de pérdida (grep de regresión, mismo patrón que el gate §IV de A1/A2/M6).
- **H3 · Telemetría de completitud:** nuevo campo `enumeration_coverage` (rutas emitidas acumuladas / estimación exhaustiva del pasaje actual) publicado por tick — el dashboard puede VER que la cobertura tiende a 1 y quedarse quieto si no.
- **H4 · Doctrina:** sección en `docs/governance/HARDENING_ANTI_REGRESION.md` (lista de congelación Nivel 2): "route discovery en shadow = DeferNeverDrop; ningún PR puede reintroducir drop por cap en shadow sin anomalía + evidencia". Mismo patrón del gate §IV.

---

## 5. FASES / PRs (§37: una anomalía, PRs secuenciales)

| PR | Contenido | Riesgo |
|---|---|---|
| PR-1 (core) | F1+F2: DFS iterativo con cursor, DeferNeverDrop, rotación, telemetría renombrada, H1 (test exhaustividad), H2 (gate CI) | Medio — reemplaza el corazón del finder; H1 es la red de seguridad |
| PR-2 (profundidad) | F3: iterative deepening 2..7 + H2-regla-depth | Bajo — monta sobre PR-1 |
| PR-3 (UX+doctrina) | Badge `DEFERRED (continúa)` en RouteDiscoveryPanel/RoutesDiscoveryClient, `enumeration_coverage` en panel, H4 (doctrina congelada) | Bajo |

**Archivos núcleo:** `route_discovery/unique_route_finder.rs` (DFS iterativo + cursor + rotación), `route_discovery_worker.rs` (loop con continuación), `telemetry.rs` (campos), `automation/tools/gate-shadow-no-route-caps.sh` + workflow, frontend `RouteDiscoveryPanel.tsx`/`RoutesDiscoveryClient.tsx`, `HARDENING_ANTI_REGRESION.md`.

**Revert:** cada PR es `git revert` limpio (sin migraciones; el cursor es estado en memoria).

---

## 6. VERIFICACIÓN (success criteria)

1. `cargo test -p searcher-rs` verde con H1 (exhaustividad matemática probada).
2. Gate H2 verde en CI (y rojo intencionalmente verificado contra un commit malicioso de prueba).
3. Post-deploy: tick live muestra `routes_deferred:true` con `deferred_cursor` avanzando entre ticks y **rutas acumuladas distintas** tick a tick (la frontera se mueve); a los K ticks, `enumeration_coverage → 1.0` y el badge ámbar `DEFERRED` sustituye al rojo `capped`.
4. Con PR #410 (cobertura) madurando el grafo, rutas descubiertas crecen sin tope de emisión por tick — el único límite que queda en shadow es el tiempo, y es honesto y visible.

## 7. FUERA DE ALCANCE (declarado)
- C5 (shape exacto por estrategia): es la definición de cada estrategia, no un cap.
- C6 (executor 1-2 legs): realidad de gasto on-chain en live; shadow/sim no la atraviesa.
- C7 (MVP_CYCLES legacy): deuda preexistente rastreada en la auditoría vivid-grove §C2.
