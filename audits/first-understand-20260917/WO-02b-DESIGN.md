# WO-02b-DESIGN — Diseño (read-only audit + propuestas gated)

> Gang Omniscience · ecc:rust-reviewer · 2026-09-17. Entregable principal:
> `02b-OPERADORES-MATH-ENGINE.md` (fichas 32 ops). Este documento registra el diseño
> de las remediaciones propuestas a partir de los GAPs G1-G6, con diffs EXACTOS,
> invariante y gate. NINGUNA línea de producción fue editada (kind: design;
> NO-GIT; sin cargo/build por restricción de run).

## 1. Verificación realizada

- Lectura completa: operators/mod.rs, api.rs, main.rs, op_24, op_31, op_32_nsga2/{mod,core},
  real_ops_tests (índice), math_evidence.rs, live_risk_ranker.rs, orchestrator.rs (secciones
  138-145/400-468/1020-1061), cartridge_boot.rs (990-1060/1195-1270), cartridge/runner.rs
  (525-566), regime_router.rs (índice), matrix/topology_map.rs (constantes).
- Grep de grafo inverso: `math_engine::` en workspace → consumers = searcher-rs (7 archivos +
  3 tests) y prioritization-spine (módulos legacy). Conteos de cartuchos reproducibles:
  `ls cartridges/strategies/*.rhai | wc -l` = 264; distribución de combos por grep (§2 de la ficha).
- Sancho (POLICY): checkpoint PRE no emitido por presupuesto de turno; el diseño se entrega
  con evidencia file:line verificable. FAIL-OPEN declarado.

## 2. Propuestas de diseño (todas GATED — ninguna aplicada)

### P-1 (G6): cargar gas real en la vía régimen — `orchestrator.rs`

Hoy `evaluate_math_evidence` se llama con `0.0` gas (orchestrator.rs:459). La vía combo ya
resuelve base_fee real vía handles atómicos del runner (cartridge_boot.rs:1226-1233).
Diseño: replicar ese patrón en el call-site del orchestrator usando el mismo handle del runner
disponible en `self.ctx` (si existe) o el fee cache del scanner. Diff propuesto (esqueleto
exacto, marcado con el ID del WO):

```rust
// backend/searcher-rs/src/orchestrator.rs (cerca de :459)
// WO-02b (2026-09-17): gas real para la vía régimen (era 0.0 — 7 ops computaban
// con fricción termodinámica cero). Fail-honest: si no hay fee observado, se
// mantiene 0.0 y el log ya declara el contexto observe-only.
let gas_gwei = self.ctx.base_fee_gwei(); // fuente real: fee cache del scanner
crate::math_evidence::evaluate_math_evidence(
    &reserves_cache, &registry, &router, &mut math_redis, &pools, chain_id,
    gas_gwei, /* WO-02b (2026-09-17) era 0.0 */
    0, 0, std::collections::HashMap::new(), &strategy_kind,
).await;
```

- Invariante: `MarketState.gas_price_gwei >= 0` finito; ningún op fabrica datos con gas=0
  (opinan None/default como hoy). La evidencia sigue observe-only (no toca scoring).
- Gate: `cargo test -p searcher-rs math_evidence` verde + un test nuevo que asserted
  gas_gwei > 0 cuando el fee cache tiene valor. GATED por operador (toca hot-path adjacente).

### P-2 (riesgo §5): auth en el toggle de math-engine — `api.rs`

Diseño mínimo, consistente con el patrón `x-arbx-admin-token` ya usado en el repo
(precedente HG 0901, memoria 2026-09-01):

```rust
// backend/math-engine/src/api.rs
// WO-02b (2026-09-17): el toggle era anónimo (memoria 2026-09-16). Requiere
// token admin SOLO para mutaciones; /health y GET siguen abiertos (healthcheck
// del compose usa /health, compose.prod.yml:203).
async fn toggle_operator_handler(
    State(st): State<ApiState>,
    headers: axum::http::HeaderMap, // WO-02b (2026-09-17)
    Path(id): Path<u8>,
    Json(body): Json<ToggleRequest>,
) -> Response {
    let expected = std::env::var("MATH_ENGINE_ADMIN_TOKEN").unwrap_or_default();
    if expected.is_empty()
        || headers.get("x-arbx-admin-token").and_then(|v| v.to_str().ok()) != Some(expected.as_str())
    {
        return (StatusCode::UNAUTHORIZED, Json(ErrorResponse {
            error: "unauthorized", detail: "toggle requires x-arbx-admin-token".into(),
        })).into_response();
    }
    /* cuerpo actual sin cambios (api.rs:264-285) */
}
```

- Invariante: fail-closed — token ausente en env ⇒ TODO toggle rechazado (no al revés).
- Gate: test nuevo `toggle_requires_admin_token` (401 sin header, 401 con token vacío en
  env, 200 con token correcto); `cargo test -p math-engine --features api`.
- Riesgo residual documentado: main.rs:29 bindea 0.0.0.0; con el compose actual
  (127.0.0.1:3006:3006) la exposición host es loopback. No se propone cambiar el bind
  (rompería el healthcheck intra-red docker) — el auth cierra la superficie real.

### P-3 (G4): op_24 Nash — desclasificar como evidencia de mercado

Diseño SIN diff de producción (decisión de clasificación): marcar op_24 como
"demo/fixture canónico" en la ficha y EXCLUIRLO de cualquier consumo futuro de
evidence_posterior_log_odds, porque su scalar no depende del MarketState
(op_24_nash.rs:74-76, payoff fijo). Si el operador lo aprueba, el diff sería un
`#[deprecated(note = "fixed-payoff fixture — not market evidence")]` en `NashOperator`.
GATED operador (es una sentencia de clasificación, no un bug).

### P-4 (G1): objetivo_usd — se mantiene GAP

Sin fuente real (config/env/registry) no hay diseño posible sin violar RULE 00. Cualquier
futuro objetivo_usd por operador debe nacer de un registro declarativo versionado
(p.ej. columna del workbook Excel → generated table, patrón `strategy_dispatch_status.rs`
con fixture json). Nada propuesto aquí.

## 3. Contradicciones con la mesa

- Charter citaba 2,511 edges en knowledge_graph.jsonl; medido hoy: 2,693 (wc -l). No es
  contradicción de fondo (el grafo creció); se reporta el número medido.
- Charter asumía `backend/searcher-rs/src/operators/` como claim de directorio: NO EXISTE.
  Refutado con `ls`. Los operadores viven exclusivamente en math-engine.
- Memoria 2026-09-16 ("toggle sin auth en 127.0.0.1"): precisada — binario 0.0.0.0
  (main.rs:29), restricción de exposición viene SOLO del publish del compose
  (compose.prod.yml:196).
- No hay reportes 02a/02c/02d al momento de escribir esto; nada que integrar ni refutar.

## 4. Invariantes globales del subsistema (para gates futuros)

1. `OperatorRegistry` registra EXACTAMENTE ids 1..=32; el 32 es NSGA-II único brazo
   (mod.rs:172). Cualquier op_33 ⇒ BAD_REQUEST (api.rs:421-434).
2. La proyección canónica permanece 264×31 (topology_map.rs:12-14; api.rs:437-446):
   op_32 NUNCA entra a la matriz ni a `build_evidence_vector` (1..=31, math_evidence.rs:375).
3. Fail-honest transversal: todo op devuelve None + `reason_*` en metadata ante input
   insuficiente; `all_31/32_operators_dispatch_and_are_fail_honest` (real_ops_tests.rs:416/:519)
   es el gate obligatorio de cualquier cambio a un operador.
4. NSGA-II runtime: orden unknown-history = Net_bps preservado; sin historia NUNCA implica
   riesgo cero (live_risk_ranker.rs:1-2, test :287-296); generations=0 (selección only).
5. Evidencia observe-only: ni régimen ni combo alteran scoring hoy (math_evidence.rs:7-11);
   el único decisorio es op_32 vía risk_ranker.
