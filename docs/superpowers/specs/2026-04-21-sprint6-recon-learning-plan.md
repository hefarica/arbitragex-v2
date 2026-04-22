# Sprint 6 — Plan de implementación

## Paso 1 — Migration 012 (recon_reports)

- `database/migrations/012_recon_reports.sql` con schema completo, FK, indexes, grants.

## Paso 2 — Config + schema

- `configs/app.toml`: sección `[recon]`.
- `configs/schemas/app.schema.json`: schema para `recon`.
- `backend/shared-rs/src/config.rs`: `ReconCfg` struct + añadir a `AppConfig` como `Option<ReconCfg>`.

## Paso 3 — shared-rs: extend ReconReport contract

- `contracts.rs`: campos nuevos (execution_id, tx_hash, chain_id, native units, pnl_source, gas, fail_reason).
- `configs/schemas/recon_report.schema.json`: mismo update.

## Paso 4 — relays-client: publicar arbx:opps:executed

- `backend/relays-client/src/consumer.rs`: tras persist_execution OK, XADD a `arbx:opps:executed` con `{opportunity, execution}`.
- MAXLEN ~10_000.

## Paso 5 — recon: pnl_engine.rs

- `pub async fn compute(opp, exec, provider) -> ReconReport`
- Fetch receipt via `provider.get_transaction_receipt(tx_hash)`.
- Decode Swap/Transfer events from router contract logs.
- Compute `actual_amount_out_wei`.
- Compute `variance_pct = (actual - expected) / expected * 100`.
- pnl_source: "native_only" en S6 main.

## Paso 6 — recon: variance.rs

- `pub fn check(report, threshold_pct) -> Option<RiskEvent>`
- Si |variance_pct| > threshold → returns risk_event structured.

## Paso 7 — recon: aggregator.rs

- Periodic task via tokio::time::interval.
- Query executions last 1h, group by strategy/chain → UPSERT strategy_scores.
- Same for relay_scores.
- Después, ejecuta anomaly_check.

## Paso 8 — recon: anomaly.rs

- Revert rate query per strategy/chain en last 15m.
- Si > threshold: risk_event + opcional kill_switch auto-trip.
- Logs estructurados con trace_id.

## Paso 9 — recon: persistence.rs

- `insert_recon_report(pool, report)` transaccional.
- `insert_risk_event(pool, event)`.
- `update_opportunity_to_reconciled(pool, id)`.
- UPSERT strategy_scores + relay_scores helpers.

## Paso 10 — recon: consumer.rs

- XREADGROUP `arbx:opps:executed` group `recon-g0`.
- Por msg: parse opportunity + execution → pnl_engine.compute → persist → XACK.

## Paso 11 — recon: main.rs wiring

- Spawna:
  - HTTP server (ya existe, /pnl endpoints).
  - Consumer task (si RPC + DB disponibles).
  - Aggregator task (siempre si DB disponible).

## Paso 12 — Tests

- `variance_test.rs`: cálculo + threshold.
- `pnl_engine_test.rs`: decode de Swap event fixture.
- `anomaly_test.rs`: trigger con fixture data.

## Paso 13 — Métricas S6 en shared-rs

- Añadir a `shared-rs/src/metrics.rs` y/o `shared-ts/src/metrics/index.ts` las 7 métricas nuevas.

## Paso 14 — Validación + commit + push

- Python schema, fake-data scan.
- Commit único.

## Out-of-scope S6 (S6.1 o S7)

- Oracle USD conversion.
- Selector-api consuming strategy_scores for adaptive weights (S6.1).
- Incident correlator.
- ML scoring.
