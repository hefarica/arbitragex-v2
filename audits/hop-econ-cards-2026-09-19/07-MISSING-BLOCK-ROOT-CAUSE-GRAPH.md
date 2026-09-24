# §30 missing_block — Grafo de defecto y hardening (2026-09-19)

**Síntoma (FE):** banner rojo `QUARANTINED — missing_block`.
**Fix:** PR #599 (branch `fix/missing-block-anchor`, commit 55019cd4).

## 1. Grafo de defecto (causalidad de la anomalía)

```
                          ┌──────────────────────────────┐
                          │  Fuentes de RouteIntent      │
                          └──────────────────────────────┘
   block_scanner.rs:385            route_intent_dispatcher (tick)
   (bloque CONOCIDO: scan_block)   (bloque CONOCIDO: current_block)
        │ intent SIN                      │ intent SIN
        │ observed_block_number           │ observed_block_number
        ▼                                 ▼
   dex_engine.build_accepted/      route_discovery_worker:1549
   rejected_opportunity            dispatch → spawn_cartridge_eval /
   block_number: None  ← RAIZ#1              shadow_evaluate_intent
        │                                 │
        │                                 ▼
        │                        cartridge_boot.rs:1201
        │                        block_number: intent.observed_block()
        │                        SIN fallback al head  ← RAIZ#3
        ▼                                 ▼
   persistence.rs INSERT block_number NULL (funnel único)
        │
        ▼
   api-server /opportunities → FE lib/store/types.ts:778
   validateOpportunitySemantics: block_number == null → "missing_block"
        │
        ▼
   BANNER QUARANTINED — missing_block (marca, no oculta: §30 by design)
```

**Invariante violada:** "toda fila detectada debe anclarse a un bloque".
**Nota honesta:** `RouteIntent::observed_block()` (route_intent.rs:86) solo
expone altura si `source_event == NewBlock` — los intents de mempool NO llevan
bloque por diseño (fail-honest). La RAIZ es que las fuentes que SÍ conocían el
bloque (scanner, dispatcher) no lo fijaban.

## 2. Grafo de hardening (defensa en profundidad, 4 capas)

```
Capa 1 (fuente de verdad)   block_scanner: intent.observed_block_number = Some(scan_block)
Capa 2 (dispatcher-tick)    route_discovery_worker: if none && current_block>0 → Some(current_block)
Capa 3 (builder)            dex_engine: block_number = intent.observed_block() (4 call sites)
Capa 4 (última milla)       cartridge_boot: .or_else(host_block_number_handle head > 0)
```

Cada capa es suficiente por sí sola para las filas de su camino; las 4 juntas
cubren los 2 paths de detección (orchestrator engines + cartridge layer).

## 3. Evidencia

- Censo PG live (read-only, 3h): 6800/514601 filas NULL block, 100% rejected-audit;
  0 visible/viable afectadas. Productores: dex_engine (381), mev_01_* (228 c/u).
- `cargo test --lib` = 1312 passed / 0 failed (incl. regresión
  `candidates_anchor_observed_block`, caminos accepted + rejected).
- `cargo clippy -p searcher-rs --all-targets -- -D warnings` limpio; `cargo fmt` aplicado.

## 4. Verificación post-deploy (pendiente)

1. `SELECT count(*) FROM opportunities WHERE block_number IS NULL
   AND detected_at > now()-interval '30 minutes'` → esperado ≈ 0
   (excepción honesta: intents de mempool puros sin head conocido).
2. Banner ausente en https://arbx.ape-tv.net.
