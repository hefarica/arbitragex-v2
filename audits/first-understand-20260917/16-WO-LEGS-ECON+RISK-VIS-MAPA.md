# Mapa combinado WO-LEGS-ECON-01 + WO-RISK-VIS-01 (investigación read-only 2026-09-18)

## Economía por-leg (montaje: RouteMetadata.leg_economics, molde HOPS-LEDGER-04 all-or-nothing)
| Campo | Estado | Coste |
|---|---|---|
| fee | YA viaja en RouteLeg.fee_bps hasta persistencia y se descarta ahí | ~nulo |
| liquidity | hop_reserves ya en stack del kernel | ~nulo (+2 wei-strings/hop) |
| rate | derivable out/in (wire-owned, FE no recomputa) | ~nulo |
| gas | ruta-level (base_fee atomic per-block); semántica de diseño | ~nulo |
| state age | V2 tiene ts+blk en mano; V3 split (slot0 sin block); ReservesCache (U256,U256)→+u64 | bajo, ROZA builder activo |
| impact | NADIE lo computa hoy (slots existen, todos None) — matemática NUEVA | medio → gate Hermes §12.1 |

## Riesgo/target
- risk_score: NUNCA asignado en triangular_worker (1589/2017), cartridge_boot(1200), relays/sim-ctl; scanner usa Some(0.0)+reason para rechazadas (patrón honesto mínimo). Test live.test.ts:234 fija null → actualizar.
- Umbrales: viven en trading_config (PG+Redis), endpoint público GET /api/v1/trading-config YA EXISTE — el FE puede mostrarlos sin backend nuevo. max_gas_cost_ratio=0.5 y max_price_impact_pct=0.05 HARDCODEADOS (comentarios lo admiten).
- Gates por-leg: StrategyConfigGate (spine) con leg ofensivo en el enum; hoy string plano — falta array estructurado.
- simulated_target null ⇐ sin strategy_configs entry para el strategy_kind (cartuchos NO matchean) Y pisos globales null.
- **WINS GRATIS detectados**: (1) simulated_notes YA viaja y NO se renderiza (FE-only); (2) estimation_basis/target_roi_pct sin render; (3) cascada 9 componentes ya pintada en Economics tab.
- Spine CostBreakdown Rust (9 comp) sólo a logs/mev/opportunity_scored.jsonl — exposición = emisión nueva.

## Decisión operador pendiente
¿Crear strategy_configs keys para los 264 cartridge strategy_kinds (para que simulated_target exista en filas de cartucho)?
