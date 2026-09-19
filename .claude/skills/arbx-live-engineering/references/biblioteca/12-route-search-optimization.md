# 12. BÚSQUEDA Y OPTIMIZACIÓN DE RUTAS DE LIQUIDEZ (ROUTE SEARCH & OPTIMIZATION)

CUÁNDO CARGAR ESTA REFERENCIA: al diseñar, auditar o depurar el detector de ciclos y el motor de rutas (TokenGraph, searcher-rs, route-discovery); al decidir entre Bellman-Ford/SPFA/DFS/Yen; cuando el hot path emite falsos positivos (rate sin capacidad, tokens trampa tipo XEN); al integrar búsqueda con sizing (referencia 11) y scoring post-gas; al optimizar el costo de cómputo por bloque del grafo en memoria.

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Grafo de liquidez | tokens=nodos, pools=aristas dirigidas; peso `w = -ln(rate)` | ciclo de peso negativo ⇔ arbitraje rentable pre-gas |
| Detección de ciclos | Bellman-Ford (supernodo fuente) / SPFA con cola | peor caso O(V·E); SPFA casi lineal en grafos reales |
| Reconstrucción del ciclo | cadena `predecessor` desde nodo relajado en ronda n | revalidar Π rate > 1 + margen gas o es artefacto |
| Múltiples ciclos | ban de la arista dominante + re-run (3-8 pasadas) | el 1er ciclo muere al ejecutarse; el 2º suele ser el ejecutable |
| Top-k rutas simples | Yen sobre pesos Johnson-reponderados; alternativa: DFS best-first con hop bound | Yen exige pesos ≥ 0; reponderar preserva orden s→t |
| Poda dura | capacity bound (reserva del lado de salida) + gas floor + max-hop 2-5 | rate sin capacidad = lección XEN (ver 12.4.4) |
| Scoring | EV neto post-gas con `amount_in` óptimo (ref 11), jamás rate bruto | dedup por solapamiento de pools antes de simular |
| Actualización incremental | invalidar solo aristas de eventos Sync/Mint/Burn + warm-start | re-correr el grafo entero por evento es inviable |
| Split multi-path | waterfilling igualando yield marginal entre rutas cóncavas | solo si la arista delgada satura y el bundle cabe en gas |
| Hot loop Rust | CSR + `FxHashMap` + `SmallVec` + `rayon` por token base + `criterion` | cero alloc por bloque; medir ns/iter, no intuir |
| Frescura de reserves | último evento `Sync` cacheado con altura de bloque | `getReserves()` on-demand prohibido en hot path |
| Competencia sobre la ruta | el ciclo detectado es público en cuanto se ejecuta | detección de toxicidad y defensa: ver `arbx-mev-ethics-gate` |

Flujo por bloque que esta referencia especifica (presupuesto típico dentro del slot de 12s):

```
 eventos Sync/Swap/Mint/Burn  (WebSocket: eth_subscribe ["logs", {...}])
        │   coalescing de la ráfaga + apply sobre slots de reserves
        ▼
 grafo CSR en memoria ── invalidar SOLO aristas dirty
        │   warm-start SPFA (cola inicial = endpoints de aristas dirty)
        ▼
 ciclos negativos → revalidar Π r > 1+θ → ban arista dominante × k
        ▼
 poda dura: cap_usd / gas floor / max-hop 2-5 / staleness
        ▼
 sizing amount* (referencia 11) → EV_net post-gas → dedup por solapamiento
        ▼
 cola de simulación revm (fuera del hot path; ver núcleo §1.1)
```

## 12.1 Modelado del grafo

### 12.1.1 Nodos, aristas y rate efectivo

- **Nodos**: tokens (address). Tipos índice `u32`, nunca addresses en el loop.
- **Aristas**: UNA arista dirigida por (pool, dirección). Un pool UniswapV2 WETH/USDC aporta dos aristas: WETH→USDC y USDC→WETH. Varios pools entre el mismo par → aristas paralelas distintas (la multiplicidad es la fuente del arbitraje).
- **Rate efectivo de una arista**: `r_e(x) = out(x)/x` para un input de tamaño `x`, INCLUYENDO fee del pool y slippage implícito. Para CPMM (UniswapV2/SushiSwap) con `getAmountOut` de `UniswapV2Library`:

```
out(x) = (x · 997 · R_out) / (R_in · 1000 + x · 997)      // fee 0.30%
r_marginal = lim(x→0) r_e(x) = 0.997 · R_out / R_in        // rate de tamaño cero
```

`r_marginal` es la cota superior del rate de la arista (`r_e(x)` decrece monotónicamente en `x` por el slippage de la curva x·y=k). Para BUSCAR basta `r_marginal`; el rate real al tamaño óptimo lo calcula el sizing (referencia 11) y la simulación revm lo verifica.

- **UniswapV3** no tiene rate lineal: spot desde `slot0.sqrtPriceX96` con `price_raw_token1_per_token0 = (sqrtPriceX96 / 2^96)^2`, ajuste de decimales `· 10^(dec0 − dec1)`, dirección (invertir si el swap va token1→token0), y fee `(1 − feeTier/1e6)`. La liquidez concentrada (`liquidity()`) solo es comparable dentro del mismo tick: para el grafo de búsqueda se usa spot como `r_marginal` aproximado y se corrige downstream (QuoterV2 `quoteExactInputSingle` / revm). Curve: `get_dy(i, j, dx)` para `r_e(x)` exacto en puntos muestreados; Balancer V2: `queryBatchSwap` fuera del hot path (es una llamada, no una fórmula local).

### 12.1.2 Transformación de peso y el teorema del ciclo negativo

```
w(e) = -ln(r_marginal(e))
Σ_{e∈C} w(e) = -ln( Π_{e∈C} r_marginal(e) )
Π r > 1  ⇔  Σ w < 0     (ciclo de peso negativo ⇔ arbitraje pre-costos)
```

La transformación convierte el PRODUCTO de tasas a lo largo del ciclo en una SUMA de pesos: eso es lo que hace aplicable la maquinaria de caminos más cortos (Bellman-Ford, Dijkstra, DP por capas) sobre un dominio multiplicativo. Con costos fijos (gas + flash loan fee) el umbral se desplaza:

```
condición ejecutable:  Σ_{e∈C} w(e) < -ln(1 + θ)
θ = (gas_usd + flash_fee_abs) / amount_in_usd   // costo fijo relativo al capital
```

Con flash loans: Balancer V2 cobra fee 0% del principal; Aave V3 premium por defecto 0.05%. Un ciclo de 2 hops V2-V2 ya paga 2×0.30% de fee de pool ⇒ necesita > 0.60% de edge bruto en rate antes de hablar de gas. Este es el error #1 de los detectores naïve (ver 12.10).

### 12.1.3 Peso, capacidad y dirección como triple por arista

Cada arista guarda `(w_marginal, cap_out, pool_id)`. El peso sirve para BUSCAR; la capacidad para PODAR y ACOTAR; juntas evitan el antipatrón rate-sin-capacidad. Las aristas con `R_in == 0 || R_out == 0` (pool vacío/drenado) no entran al grafo: `-ln(0)` es +∞ y `R_out=0` significa que no hay nada que comprar.

## 12.2 Detección de ciclos negativos

### 12.2.1 Bellman-Ford: por qué detecta y cuánto cuesta

Bellman-Ford relaja todas las aristas V−1 rondas. Sin ciclos negativos, todo camino más corto es simple (≤ V−1 aristas) y converge; si en la ronda V todavía hay una relajación, existe un walk de ≥ V aristas con peso estrictamente decreciente, y un walk de V aristas sobre V nodos repite nodo ⇒ ese segmento repetido es un ciclo de peso negativo alcanzable. Costo: **O(V·E)** por pasada completa.

**Límite de iteraciones ≤ n**: jamás correr más de V rondas "por si converge". La ronda V con relajación ES el veredicto de ciclo; seguir iterando solo quema el presupuesto por bloque.

**Cobertura total del grafo**: el ciclo puede estar en cualquier componente. Patrón supernodo: fuente virtual `s` con arista de peso 0 hacia cada token (equivalente: inicializar `dist[v] = 0` para todo `v`); así todo ciclo es alcanzable desde `s` y una sola corrida lo detecta.

APIs reales: `petgraph::algo::bellman_ford(g, source)` (petgraph 0.6.5, la versión fijada en `backend/Cargo.lock`) devuelve `Result<Paths<NodeId, W>, NegativeCycle>` donde `Paths` expone `distances: Vec<W>` y `predecessors: Vec<Option<NodeId>>` indexados por índice de nodo (exige un único source — combinar con supernodo); para el ciclo mismo existe `petgraph::algo::find_negative_cycle(g, source) -> Option<Vec<NodeId>>`, que ya implementa la caminata hacia atrás por predecesores (ver 12.2.3). El crate `pathfinding` (`pathfinding::prelude::bellman_ford`) toma los sucesores como closure (encaja natural con CSR) y devuelve costos y predecesores como `Option<(Vec<C>, Vec<Option<N>>)>`: `None` = ciclo negativo alcanzable.

### 12.2.2 SPFA (Bellman-Ford con cola)

SPFA mantiene una cola de nodos "cuya arista de salida puede mejorar": solo relaja las aristas salientes de nodos que cambiaron. Mismo peor caso O(V·E), pero en grafos de liquidez reales (grado medio bajo, pocas aristas mutando por bloque) converge en 1-3 rondas efectivas. Refinamientos estándar: SLF (insertar al frente si el dist del nodo es menor que el del frente) y conteo de encolaciones por nodo (si un nodo entra a la cola ≥ V veces ⇒ ciclo negativo, abortar y pasar a reconstrucción). Presupuesto duro: contador global de relajaciones con tope (p.ej. `4·V·E` en unidades de trabajo) → si vence, degradar a "mejor solución parcial" y emitir la métrica, no colgar el slot.

### 12.2.3 Reconstrucción del ciclo vía predecessor (y falsos ciclos)

El grafo `pred[]` construido durante la relajación NO es un árbol de caminos simples cuando hay ciclos negativos: contiene walks mezclados de distintas "épocas" de relajación. Receta robusta:

```
1. x ← cualquier nodo relajado en la ronda V (o con encolaciones ≥ V en SPFA)
2. y ← x; repetir n veces: y ← pred[y]        // n pasos garantizan caer DENTRO del ciclo
3. recorrer pred desde y hasta volver a y, coleccionando la ARISTA (from,to,edge_id)
   usada en cada relajación — guardar pred_edge[], no solo el nodo
4. REVALIDAR: Π r_marginal(aristas del ciclo) ≥ 1 + θ_margen
```

El paso 4 es obligatorio por dos razones: (a) precisión f64 — un "ciclo" con `Π r = 1 + 1e-12` es ruido numérico que el gas mata; (b) si el grafo mutó durante la búsqueda (eventos en vivo sobre la misma estructura), las aristas recolectadas pueden mezclar snapshots. Todo ciclo que no revalida se descarta con razón explícita (`cycle_revalidation_failed`) — fail-honest, ver núcleo §1 y R8 del proyecto.

### 12.2.4 Enumeración de MÚLTIPLES ciclos distintos

Un solo ciclo por snapshot es información empobrecida: el dominante suele ser el más competido (y muere al consumirse sus reserves). Enumeración práctica con presupuesto:

1. Correr SPFA → ciclo C₁ (revalidado).
2. Identificar la arista dominante de C₁: la de menor `cap_out` (cuello de botella) o la de mayor contribución negativa al peso.
3. Marcar esa arista como banned (bitset de E bits, NO reconstruir el grafo) y re-correr SPFA → C₂.
4. Repetir hasta k ciclos (k = 3-8), sin nuevos ciclos, o agotar el presupuesto de relajaciones compartido.

Alternativa por token base: correr la búsqueda desde cada base token por separado produce ciclos naturalmente distintos (ver paralelismo 12.9).

## 12.3 Rutas top-k: Yen vs heurística DFS acotada

### 12.3.1 Yen (k-shortest simple paths)

Yen genera k caminos simples s→t: para cada camino A[i−1] y cada nodo spur del camino, remueve temporalmente las aristas que los caminos ya aceptados comparten con el root path, corre Dijkstra desde el spur, y mantiene candidatos en un heap por costo total. Complejidad O(k · V · (E + V log V)).

**Trampa crítica**: Yen (vía Dijkstra) exige pesos NO negativos, y `w = -ln(rate)` produce aristas negativas siempre que un rate marginal > 1. Solución estándar: **reponderación de Johnson** — potenciales `h` calculados con Bellman-Ford (supernodo), `ŵ(u,v) = w(u,v) + h(u) − h(v) ≥ 0`; todo camino s→t cambia su peso total por la constante `h(s) − h(t)`, así que el ORDEN entre caminos s→t se preserva y Yen/Dijkstra son válidos sobre `ŵ`. Nota: si existe ciclo negativo, los potenciales no existen — pero ese caso ya lo consumió la etapa 12.2 (se enumera el ciclo, no se piden k caminos). Sobre petgraph: `petgraph::algo::k_shortest_path(g, start, goal, k, edge_cost)` devuelve solo costos acumulados por nodo (`HashMap<NodeId, K>`, sin la secuencia de aristas) y no exige caminos simples — para rutas ejecutables hay que implementar Yen (o usar un crate especializado en simple paths) sobre Dijkstra reponderado.

### 12.3.2 Cuándo basta la heurística

Si el objetivo es "rutas A→B (o A→B→C→A) de ≤ 4-5 hops con buena cobertura", Yen es sobreingeniería. Patrón productivo: **DFS best-first por token base** con tres podas:

- **Hop bound duro**: profundidad máxima 2-5 (el gas por hop y el riesgo de reversion crecen más rápido que la diversidad de rutas).
- **Poda por peso acumulado**: descartar la rama cuando `w_so_far > ln(1/rate_floor)`, con `rate_floor` la fracción mínima de rate que una ruta seria sobrevive de forma acumulada (tolerar perder ≤ 15-30% ⇒ `rate_floor` 0.85-0.70 ⇒ umbral ln(1/0.85)≈0.16 a ln(1/0.70)≈0.36).
- **Cota admisible por capas**: precomputar `dist_k[v]` = mínimo peso de un WALK de ≤ k aristas desde v hasta el token base (k rondas de relajación sobre el grafo inverso — Bellman-Ford limitado a k pasadas). Todo camino de ≤ k hops es un walk de ≤ k hops, así que `w_so_far + dist_{D−depth}[v]` es cota inferior del peso total; si `≥ best_threshold`, podar. Exacta y barata: O(D·E) por base token, cacheable entre bloques si el grafo no mutó.

DFS ordena la expansión por `w` creciente (mejor rate primero) y lleva presupuesto de expansiones (nodos visitados), no solo profundidad.

## 12.4 Poda dura: bounds antes de simular

La simulación (revm) es cara; la poda es barata. Ordenar el embudo: bounds → scoring → sizing → simulación.

### 12.4.1 Capacity bound (la arista más delgada)

En CPMM, `out(x)` es asintótico: cuando x→∞, `out → 0.997 · R_out`. NO puedes recibir del pool más tokens de los que hay en la reserva del lado de salida. Por tanto, para un ciclo C:

```
cap_usd(C) = min_{e ∈ C}  usd(R_out(e))
EV_usd(C)  ≤ cap_usd(C) − gas_usd − min_profit_abs     (cota superior dura)
```

Si la cota ya está bajo el umbral de profit, podar ANTES de sizing y simulación. Esta cota también acota el `amount_in` útil: tomar prestado más que ~`cap` (convertido al token base por los rates del ciclo) solo regala flash fee y gas.

### 12.4.2 Cota por gas

Gas estimado por ciclo ≈ `gas_base + hops · gas_por_hop` (orden de magnitud: cada swap V2/V3 consume del orden de 10⁵ gas; medir el valor exacto del bundle con `eth_estimateGas` fuera del hot path y cachearlo por tipo de ruta). Con `eth_gasPrice` (o baseFee del header + priority) se obtiene `gas_usd`; ciclos cuya cota `cap_usd − gas_usd` no supera el mínimo de profit se podan. El proyecto ya materializa este gate como skill `arbx-net-profit-gate` (precedente G-ECON: `net = 0` reportado honesto como `gas_floor_breach`, jamás re-etiquetado).

### 12.4.3 Max-hop 2-5

Con cada hop: +fee de pool (0.01%-1% según venue), +gas, +probabilidad de revert, +decoherencia de estado acumulada. Los ciclos de 6+ hops son casi siempre artefactos de grafos con aristas paralelas ruidosas. Mantener el bound 2-5 tanto en DFS como al filtrar ciclos de Bellman-Ford (un ciclo negativo detectado puede ser largo; los largos se descartan por esta regla).

### 12.4.4 Pools muertos y tokens trampa (lección XEN)

Un token con `r_marginal` enorme y liquidez de USD 3 produce ciclos "negativos" espectaculares e in-ejecutables: `cap_usd ≈ 3` y el gas los mata, pero si la poda por capacity no corre ANTES del scoring, el motor se inunda. En el repo esto ocurrió: la taxonomía de rechazos 2026-09-06 mostró XEN+AGLD ≈ 78% de todos los rechazos con 48.4K detecciones/24h al 100% rejected, y el work order XEN-FLOOD-EXCL-2026-09-15 terminó excluyendo 5 pools XEN del universo observado. Defensa en capas:

1. **Structural**: `cap_usd` por arista con umbral mínimo; aristas bajo el umbral ni siquiera entran al grafo.
2. **Screening**: filtrado de tokens por seguridad/liquidez/edad → skill `arbx-token-safety-screen` (definición de trampa: fee-on-transfer, honeypot, pausable, liquidez fantasma).
3. **Observacional**: si un token acumula rechazos por la misma razón (reason taxonomizada, R8), excluirlo del universo por work order explícito con backup — nunca silenciosamente (RULE 00).

## 12.5 Scoring: EV neto post-gas, no rate bruto

Rankear por `Π r` es el error clásico: un ciclo con 8% de edge bruto sobre $200 de capacidad pierde contra uno con 0.9% sobre $80k tras gas. Score correcto, por ciclo revalidado:

```
EV_net(C) = out(amount*) − amount* · (1 + flash_fee_bps)
            − gas_used_est(C) · gas_price − bribe_reserve
amount*   = argmax_x  out_C(x) − x   // sizing: referencia 11
```

El `amount*` y el `EV_net` son end-to-end: `out_C(x)` compone los `getAmountOut` reales de cada hop con las reserves actuales (cóncava en x para CPMM ⇒ unimodal ⇒ búsqueda unidimensional segura).

**Dedup de rutas por subgrafo similar**: la búsqueda produce 20 variantes del mismo ciclo (misma arista delgada, distinta arista gorda). Antes de encolar a simulación: hash del conjunto ordenado de pools (`route_hash`, mismo determinismo que idempotencia en núcleo §1.3) para eliminación exacta, y para variantes: Jaccard sobre el conjunto de pools — dos rutas con solapamiento ≥ 0.6 son la misma oportunidad; conservar la de mayor cota `EV` y descartar el resto con razón `duplicate_route`. Esto reduce la cola de simulación en órdenes de magnitud.

**Desempate**: con flash loan el capital propio aportado es ~0 (solo gas y fee), así que el objetivo es `EV_net` absoluto; si hay capital propio en juego, desempatar por `EV_net / capital_atrisk` y aplicar los límites de `arbx-risk-limits-enforcement` antes de ordenar la cola final.

## 12.6 Actualización incremental del grafo

### 12.6.1 Grafo en memoria e invalidación selectiva por eventos

La estructura es estable (pools y tokens cambian a escala de horas); lo que muta por bloque son las reserves. Patrón: array denso de reserves indexado por pool_id + CSR de aristas que apunta al slot de reserves de su pool. Invalidez SOLO las aristas afectadas:

- **UniswapV2 `Sync(uint112 reserve0, uint112 reserve1)`**: emitido en TODO swap/mint/burn/skim del pool — es el evento canónico; actualizar el slot y marcar las ≤ 2 aristas del pool como dirty. `Mint`/`Burn` (V2) también mueven reserves y emiten Sync, así que Sync basta.
- **UniswapV3 `Swap(...)`**: cambia `sqrtPriceX96`/tick → recalcular `r_marginal` de las aristas del pool; `Mint`/`Burn` de V3 (liquidez por rango) cambian la profundidad efectiva → recalcular la aproximación de capacidad.
- **Fee switch / nuevos pools**: reconstrucción del CSR en frío (fuera del hot path, por bloque o por hora).

Ingesta por WebSocket: `eth_subscribe` con `["logs", {address/topics}]` (el topic0 del evento `Sync` de V2 se filtra server-side). Tabla de invalidación:

| Evento | Origen | Acción sobre el grafo |
|---|---|---|
| `Sync(uint112,uint112)` | UniswapV2 (emitido en todo swap/mint/burn/skim) | actualizar slot de reserves; recalcular `w` y `cap` de las ≤ 2 aristas del pool |
| `Swap(address,address,int256,int256,uint160,uint128,int24)` | UniswapV3 | recalcular `r_marginal` desde el nuevo `sqrtPriceX96`/tick del evento |
| `Mint`/`Burn` (liquidez por rango) | UniswapV3 | recalcular profundidad/capacidad efectiva del rango afectado |
| `Mint`/`Burn` | UniswapV2 | redundante si Sync llegó (siempre llega); usarlo como heal-check de consistencia |

Regla R7-style de trazabilidad: si el feed de logs se calla, el grafo envejece — todo slot de reserves lleva `last_sync_block`; aristas con staleness > N bloques se podan o degradan (skill `stale-state-detection`), porque detectar sobre un grafo viejo es fabricar oportunidades (violación de fail-honest).

### 12.6.2 Warm-start de Bellman-Ford/SPFA

Entre bloques consecutivos cambian una fracción mínima de aristas. Patrón: conservar `dist[]`/`pred[]` del bloque anterior, aplicar solo las aristas dirty, e inicializar la cola de SPFA con los ENDPOINTS de esas aristas (no desde cero). Convergencia típica: 1-2 rondas locales. Al detectar ciclo negativo, el snapshot nuevo invalida el warm-start del ciclo afectado (la solución anterior "contenía" un ciclo que ya no existe o viceversa) — revalidación obligatoria (12.2.3).

### 12.6.3 Presupuesto de cómputo por bloque, timeout y backlog

El slot es de 12s (mainnet); el presupuesto de búsqueda es una fracción (típicamente 50-300ms tras procesar eventos). Reglas:

1. **Deadline por wall-clock** (`Instant::now() + budget`): al vencer, entregar lo mejor encontrado y emitir métrica `search_deadline` — nunca bloquear el slot esperando convergencia.
2. **Coalescing de eventos**: si llega un ráfaga de Sync (bloque denso), aplicar TODOS los eventos pendientes y correr UNA búsqueda sobre el grafo resultante — no una búsqueda por evento.
3. **Backlog acotado**: cola de eventos con drop-oldest contado (métrica `events_dropped`); un backlog creciente indica que el presupuesto excede el hardware o que el universo de pools creció — alertar, no degradar en silencio.
4. **Warmup en bloques vacíos**: pre-correr cotas `dist_k` y candidatos top en bloques tranquilos (núcleo §1.2 ya menciona el patrón de warmup).

## 12.7 Integración con sizing (referencia 11)

El buscador NO decide tamaño: entrega ciclos revalidados con (aristas, reserves, `cap_usd`, `Π r_marginal`). Por cada candidato, el sizing optimiza `amount_in` (búsqueda unidimensional — golden-section / ternary son seguros por unimodalidad del CPMM; ver referencia 11 para el método y sus guardas) y devuelve `amount*`, `EV_net(amount*)`. Solo entonces la ruta entra a la cola de simulación revm, que verifica el resultado contra execution real (fees on-transfer, hooks, approvals, gas real). Orden de verdad creciente y costo creciente: bounds → fórmula cerrada → sizing → revm. Saltarse niveles quema el presupuesto de simulación en basura (lección G-SIM-1 del repo).

## 12.8 Split multi-path

Cuando una sola ruta satura su arista delgada (el yield marginal `d out/d x` cae por debajo del de otra ruta paralela), dividir `amount` entre rutas maximiza el output total. Para rutas DISJUNTAS en pools, cada `out_r(x_r)` es cóncava y el óptimo iguala derivadas marginales (waterfilling):

```
max Σ out_r(x_r)  s.t. Σ x_r = X   ⇒   d out_r/d x (x_r*) = μ  para toda ruta activa
```

Iteración práctica: partir equitativo, evaluar derivadas numéricamente, mover cuota de baja-marginal a alta-marginal, repetir 2-3 veces. Si las rutas COMPARTEN pools, son acóncavas acopladas (el split altera las reserves del pool compartido): resolver como independiente + re-evaluar reserves tras aplicar el split (descenso coordinado) y validar el total en revm — sin garantía de óptimo global, con verificación final exacta.

**Cuándo NO vale el split**: cada swap adicional cuesta ~10⁵ gas y, en un bundle atómico con un solo flash loan, UNA rama que revierta tumba TODO el bundle. El split se justifica solo si `EV(split) − EV(single) > gas_marginal + prima_de_riesgo_revert`. Ejemplo de decisión con aritmética ilustrativa: si `EV(single) = 120 USD` con la arista delgada saturada, `EV(split 70/30) = 148 USD` y las dos rutas extra cuestan `2·10⁵ gas ≈ 0.004 ETH` a 20 gwei (~$12 a ETH $3k), el delta neto es +$16 ⇒ split sí, PERO solo si la ruta B no comparte pools con A; si comparte el pool delgado, el waterfilling independiente sobre-estima y hay que iterar + validar en revm antes de creerse el +$16. En paper/shadow el split se audita igual que en live (§34.1: la matemática es mode-invariant).

## 12.9 Ingeniería Rust del hot loop

```
pub struct Graph {
    offsets:   Vec<u32>,              // CSR: V+1 offsets
    edges:     Vec<Edge>,             // ordenadas por nodo origen (cache-friendly)
    token_idx: FxHashMap<Address,u32> // rustc-hash: hashing rápido sin DoS-guard
}
pub struct Edge {
    to: u32,
    pool: u32,
    w_marginal: f64,   // -ln(r marginal post-fee)
    cap_out_usd: f32,  // cota de capacidad para poda
    reserves_slot: u32 // índice al array denso de reserves
}
```

- **CSR** (offsets + edges planos): iteración de adyacencia sin pointer-chasing; rebuild del CSR solo cuando cambia la topología, no por Sync.
- **`FxHashMap`** (crate `rustc-hash`) para token→índice; **`SmallVec<[T; N]>`** (crate `smallvec`) para adyacencias temporales pequeñas inline.
- **Cero alloc por bloque**: `dist`, `pred`, `pred_edge`, `in_queue`, la cola SPFA y los bitsets de ban viven en un `Scratch` reutilizado (`clear()` entre corridas, `with_capacity` inicial). Prohibido `format!`, `String`, `Vec::new()` o clonar addresses dentro del loop de relajación.
- **Paralelismo por token base**: `base_tokens.par_iter().map(|b| search(b, &scratch_pool))` con `rayon` (crate `rayon`, `rayon::prelude::*`); cada worker del pool posee su `Scratch` (rayon reusa hilos → thread-local o pool de scratch por worker). No compartir `&mut` entre bases: la búsqueda por base es independiente por construcción.
- **Medición con `criterion`** (crate `criterion`: `criterion_group!`/`criterion_main!`, `c.bench_function("search_per_block", |b| b.iter(|| ...))`, `black_box` sobre inputs): fijar un fixture de grafo sintético con cardinalidad realista (p.ej. 5k tokens / 40k aristas) y presupuestar ns/iter contra el deadline del bloque en CI, no en producción. Un regresor de latencia aquí es un incidente futuro (§37).

## 12.10 Antipatrones documentados

| Antipatrón | Síntoma | Corrección |
|---|---|---|
| Buscar rate sin capacidad | floods de "oportunidades" XEN/AGLD 100% rejected (78% del total en la taxonomía 2026-09-06) | capacity bound estructural ANTES del grafo + `arbx-token-safety-screen` |
| Ciclo A→B→A con spot prices, sin fee ni gas | "arbitraje" de 0.2% que al simular da net negativo | peso con `r_marginal` post-fee y umbral `Σ w < -ln(1+θ)` (12.1.2) |
| `getReserves()` on-demand en el hot path | latencia 50-200ms por pool + rate-limit 429 del provider | último evento `Sync` cacheado con `last_sync_block`; staleness > N bloques ⇒ podar |
| Re-correr Bellman-Ford completo por cada evento | CPU 100%, backlog creciente, slots perdidos | coalescing + invalidación selectiva + warm-start (12.6) |
| Un solo ciclo por snapshot | oportunidad muere con la 1ª ejecución y no hay plan B | enumeración multi-ciclo por ban de arista dominante (12.2.4) |
| Rank por `Π r` bruto | cola de simulación llena de rutas de $3 de capacidad | score por `EV_net(amount*)` post-gas + dedup (12.5) |
| Usar Dijkstra/Yen con pesos negativos | rutas "óptimas" incorrectas o panic | Johnson reweighting antes de Yen/Dijkstra (12.3.1) |
| Confundir walk con ciclo (falso ciclo de pred) | ciclos que al revalidar no cierran | `pred_edge[]` + paso n hacia atrás + revalidación Π r (12.2.3) |
| Detectar sobre grafo stale | oportunidades fantasma post-inclusión | staleness por arista + degradación honesta (`stale-state-detection`) |
| Yen para todo | 30ms de Yen donde bastaba DFS 4 hops | heurística con cota por capas cuando el hop bound es ≤ 5 (12.3.2) |

Nota de defensa: cualquier ciclo detectado es visible para el resto de la red en cuanto se ejecuta (backrun competitivo, sandwich de la propia tx). El tratamiento de contrapartes agresivas (detectar si SOMOS el objetivo, private routing, mitigación) es dominio exclusivo de la skill `arbx-mev-ethics-gate`; esta referencia solo cubre la matemática de búsqueda.

## GOBERNANZA

Todo lo anterior opera bajo shadow/paper y está subordinado a los gates `arbx-paper-trade-first`, `arbx-simulation-mandatory`, `arbx-risk-limits-enforcement` y `arbx-pre-execute-checklist`, y a CLAUDE.md §34: el terminus de ejecución (`relays-client`) permanece default-deny; nada aquí autoriza flip a `LIVE_MAINNET` ni broadcast con capital real. La matemática de búsqueda es mode-invariant (§34.1) y se valida idéntica en paper/shadow antes de cualquier promoción.
