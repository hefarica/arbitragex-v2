# STEP 3 — Umbrales pre-registrados (BEFORE building, per operator directive 2026-10-12)

> Decisiones de promote/archive para los engines experimentales, fijadas ANTES de
> construir. Si no se pre-registran, STEP 3 se convierte en "los dejamos porque
> ya invertimos". Con el umbral, la decisión es mecánica.

## Umbral de PROMOCIÓN (engine se queda y se invierte en él)

Un engine se PROMUEVE si, tras 24h de operación con `Some(_)`:
- Produce ≥ 1 fila/hora en su `strategy_kind` en PG, O
- Produce ≥ 1 rechazo/ hora con `reason` que NO sea `no_data_dependency`

## Umbral de ARCHIVO (engine se congela con `#[cfg]` y se documenta)

Un engine se ARCHIVA si, tras 24h de operación con `Some(_)`:
- Produce 0 filas Y 0 rechazos con dependencia presente, O
- Su costo de operación (RPC calls, latencia) excede su valor (0 filas)

## Estado por engine

| Engine | Construido | Umbral | Ventana | Veredicto |
|---|---|---|---|---|
| `spanning_tree_engine` | ✅ STEP 2.5a | ≥1 fila/h o ≥1 rechazo/h con dep. presente | 24h post-deploy | PENDIENTE |
| `liquidation_snipe_engine` | ❌ `None` (2.5b) | idem | — | NO MEDIDO |
| `cross_chain_engine` | ❌ `None` (2.5c) | idem | — | NO MEDIDO |

## Anti-recurrencia

`v2.engine.none{engine=<nombre>}` en `debug!` por intent — un engine `None` es
delatado por el wiring de capa 1 en cada intent, no opera en silencio.
