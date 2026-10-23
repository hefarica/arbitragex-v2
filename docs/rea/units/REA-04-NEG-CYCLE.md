ID:            REA-04-NEG-CYCLE
Fuente:        paradigmxyz/artemis @4bb158070833ec2b789a2ece14c896c4b28ce3be — NO EXISTE el segmento `cycle_enumerator`.
               Verificado con checkout del sha exacto: crates = artemis-core / clients / generator / strategies;
               grep `arbitrage|triang|triangle|finder|bellman` en todo el repo → solo las 2 estrategias de ejemplo
               (crates/strategies/mev-share-uni-arb/src/lib.rs:1, crates/strategies/opensea-sudo-arb/src/lib.rs:7).
               → PASO 0 (REA_MASTER §5) FALLA: la fuente citada por el backlog no contiene el algoritmo.
Licencia:      artemis MIT OR Apache-2.0 (LICENSE-MIT + LICENSE-APACHE presentes en el sha) — irrelevante sin segmento.
Tipo:          INSPIRACION (no hay test numérico del repo de origen que reproducir; el algoritmo ya vive en el destino)
Invariante:    un ciclo es rentable sii la suma de pesos `-ln(rate_i · (1 − fee_i))` sobre sus aristas es < 0,
               equivalente a Π rate_i > 1 — y un ciclo con Π ≤ 1 se descarta.
Matematica:    ya implementada en el destino (no requiere port):
               · PoolEdge::log_weight  → backend/searcher-rs/src/engines/spanning_tree_engine.rs:66-85
               · bellman_ford_cycles    → backend/searcher-rs/src/engines/spanning_tree_engine.rs:291
               · rate_to_bellman_ford_weight → backend/math-engine/src/route_math.rs:25 (sin consumidores: código muerto)
Test origen:   NINGUNO (artemis no aporta vectores; se declara en vez de inventarse).
Test propio:   N/A — esta unidad no escribe código. El gate del backlog ("ciclo conocido inyectado se detecta;
               ciclo sin beneficio se descarta") NO está cubierto: los tests del engine son solo
               test_pool_edge_log_weight (:633), test_pool_edge_exchange_rate (:653), test_empty_cache_returns_empty_cycles (:671).
Gate:          NO EJECUTABLE hoy: el grafo productivo se construye de `well_known_token_pairs()` (spanning_tree_engine.rs:579),
               con el comentario explícito en build_graph_from_cache (:217-225) "use a minimal set of well-known token pairs;
               in production, this would scan all pools from the reserves cache". El BF nunca ve el universo real de pools.
Control neg.:  N/A (sin código nuevo). La deuda equivalente: el gate exige un ciclo inyectado — factible llamando
               bellman_ford_cycles(&nodes,&edges) directamente desde un test del módulo.
Evidencia:     · artemis@4bb1580 checkout: `git cat-file -t 4bb1580 → commit`; greps vacíos de cycle/bellman
               · destino: enable_bfm=true default (canonical_knobs.rs:209), env ARBX_KNOB_ENABLE_BFM (:338);
                 enum BFM_NEG_CYCLE (canonical_enums.rs:28,36,145); orquestador wirea SpanningTreeEngine (orchestrator.rs:103)
               · omega-search/src/cycles.rs:19-21 documenta "por qué DFS acotado y NO Bellman-Ford" en esa capa
Riesgo:        N/A (cero cambios de código en esta unidad).
FOLLOWUP:      REA-04-CLOSE — el cierre real tiene DOS piezas, ambas fuera de esta iteración:
               (1) DATOS: construir el grafo desde el universo de pools requiere pool→(token0,token1) en el cache;
                   ReservesCache (engines/triangular_engine.rs:84) solo guarda (U256,U256) por address — MISMA familia de
                   bloqueo que REA-03-WIRE;
               (2) GATE: tests de ciclo inyectado (rentable detectado / no rentable descartado) contra bellman_ford_cycles.
Estado:        BLOCKED (PASO 0 falla: la fuente citada no tiene el algoritmo; el cierre local está bloqueado por datos)
