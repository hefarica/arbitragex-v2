# WO-02 apply — BLOCKED (R8 fail-honest)

**Agente:** rust-topology-engineer (ecc:rust-reviewer + ecc:tdd-guide) · **Fecha:** 2026-09-19 · **kind:** apply
**Estado:** BLOCKED — prerrequisito ausente. CERO ediciones al árbol. CERO compilación. CERO git.

## 1. Razón del bloqueo

El charter exige leer `audits/e2e-cards-20260919/WO-02-unknown-dex-cause.md` y
`audits/e2e-cards-20260919/WO-01-field-map.md` y aplicar el diff que WO-02 (design) proponga.
Ninguno existe:

```
$ ls -la audits/e2e-cards-20260919/
GOAL-WORKORDERS.md            (único archivo, 4545 bytes, 2026-09-18 23:02)

$ find audits -iname "*WO-02*" -o -iname "*WO-01*"
audits/first-understand-20260917/WO-02a..d-*      ← misión distinta (2026-09-17)
audits/omniscience-integration-2026-09-06/WO-02-* ← misión distinta (2026-09-06)
```

Los `WO-02*` hallados pertenecen a otras misiones y no contienen un diseño ni diff sobre
`dex_a / dexes_used / dex_adapters = "unknown"`. Inventar el diseño desde el apply violaría
la secuencia design→apply del board y RULE 00 / R8 ("no computado" se declara, no se inventa).
El board (`GOAL-WORKORDERS.md`) muestra WO-01 y WO-02 en estado OPEN (design todavía no entregado).

## 2. Lo que sí se verificó (read-only, sin editar) — para el diseñador de WO-02

Objetivo: que quien entregue el design no parta de cero. Clasificación por afirmación.

### 2.1 Los archivos claimados NO fabrican el "unknown" en el path vivo — CANONICAL_REPO

| Archivo:línea | Qué hace | Veredicto |
|---|---|---|
| `backend/searcher-rs/src/engines/dex_engine.rs:825` | `dex_adapters: vec![pool_a.dex_name.clone(), pool_b.dex_name.clone()]` | Copia lo que trae `PoolRef.dex_name`. No origina el valor. |
| `backend/searcher-rs/src/engines/dex_engine.rs:906` | `ProtocolType::Unknown => "unknown"` en `protocol_type_to_str` | Afecta `protocol_type`, NO `dex_name`. |
| `backend/searcher-rs/src/engines/dex_engine.rs:971` | fixture `make_pool` bajo `#[cfg(test)]` | Solo tests. |
| `backend/searcher-rs/src/workers/route_scanner_worker/provenance.rs:73-76` | `if name.is_empty() \|\| name.eq_ignore_ascii_case("unknown") { return Err("missing_dex_identity") }` | **Rechaza** "unknown"; deja `dex_hint = None`. Es un guardián, no un origen. |
| `backend/searcher-rs/src/cartridge/runner.rs:516` | `author` del cartucho = "unknown" | Metadato de cartucho; irrelevante para dex. |
| `backend/searcher-rs/src/persistence.rs:417` | `leg("0xB", "0xC", None, "unknown")` | Fixture de test documentando el fallback. |
| `backend/searcher-rs/src/opportunity_emitter.rs` | sin ocurrencias de `"unknown"` | — |

### 2.2 Orígenes candidatos FUERA del claim (el diseño debe trazarlos) — INFERRED

1. **`backend/searcher-rs/src/scanner.rs:773`** (pool_sync_watcher):
   `dex_name: "unknown".to_string(), // resolved later via factory`. Si la resolución por
   factory no ocurre para esos pools, `dex_engine.rs:825` copia "unknown" tal cual a
   `route_metadata.dex_adapters`. Hipótesis compatible con `dex_a "unknown"` en la card.
2. **`backend/searcher-rs/src/cartridge_boot.rs:1180-1182` y `:1310-1317`** (capa cartucho):
   `dex_a = first_leg.dex_hint.unwrap_or("unknown")` y `RouteLeg{dex_id,dex_name} =
   leg.dex_hint.unwrap_or("unknown")`. Si `provenance.rs` devolvió `missing_dex_identity`
   (o no se llamó), `dex_hint = None` y CADA leg emite "unknown". Hipótesis compatible con
   `dex_adapters ["unknown"×4]` (4 legs) observado en el board (GET /api/opportunities/live).

Ambas convergen en la misma raíz probable — HYPOTHESIS: **`PoolRef.dex_name` llega vacío o
"unknown" desde el catálogo/pool-sync y ningún productor lo resuelve contra `chains.rs` /
factory antes de emitir**. Qué hace el diseño: confirmar con evidencia (a) si el "resolved
later via factory" del scanner realmente ejecuta para los pools que aparecen en cards, (b) si
`provenance::hydrate` corre en el path cartucho antes de `cartridge_boot.rs:1176`, y (c)
cuál de los dos paths de detección (orchestrator engines vs cartridge layer, memoria
2026-09-17) produce las filas que la card muestra.

### 2.3 Consecuencia para el apply (cuando exista el design)

- Un fix en `dex_engine.rs`/`persistence.rs` que sustituya "unknown" por un nombre derivado
  localmente sería un **mock por otro nombre** (RULE 00). El fix correcto es resolver
  `dex_name` desde el catálogo real (`chains.rs` / factory) en el productor, o emitir
  fail-honest (`missing_dex_identity`) y NO persistir la fila como ejecutable.
- Si el diff toca `opportunity_emitter.rs` o `persistence.rs`, coordinar en serie con WO-03
  apply (mismos archivos), WO-02 primero, según charter.
- Los orígenes candidatos (`scanner.rs`, `cartridge_boot.rs`) NO están en mi claim: el
  design debe o bien extender el claim o bien asignarlos a otro dueño en el board.

## 3. Verificación

No aplica (sin cambios). No se ejecutó `cargo check` ni `cargo test` porque no había diff
que verificar; compilar sin cambio sería evidencia decorativa.

## 4. Archivos tocados

- `audits/e2e-cards-20260919/WO-02-apply-BLOCKED.md` (este reporte) — único archivo escrito.

## 5. Siguiente paso para el board

Re-despachar WO-02 en `kind: design` (dueño rust-reviewer, read-only) con entregable
`WO-02-unknown-dex-cause.md` conteniendo causa + diff + test rojo. Cuando exista, relanzar
este apply; todo lo de §2 queda disponible para el diseñador.
