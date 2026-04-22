# ArbitrageX v2 — Sprint 6 "Recon + Learning loop" — Design Spec

**Fecha**: 2026-04-21
**Sprint**: 6 de 8
**Depende de**: S1-S5 cerrados. `arbx:opps:executed` publicado por relays-client (S6 añade este publish en relays-client).
**Credenciales nuevas**: Ninguna obligatoria. Opcional para S6.1: oracle price feed (Chainlink, Uniswap V3 TWAP) para convertir PnL a USD.

## 0. Objetivo

Cerrar el loop de feedback del pipeline MEV:

1. **Reconcile each execution** contra la realidad on-chain (receipt, logs, gas).
2. **Compute PnL** por opportunity — en unidades nativas de token (S6 main) o USD (S6.1 con oracle).
3. **Detect variance** entre expected y actual → alerta en outliers.
4. **Aggregate rolling scores** por strategy y relay → feedback adaptativo para selector (S3).
5. **Detect anomalies** en revert_rate, inclusion_rate → escalate a risk_events + opcional auto-kill-switch.

El output alimenta al selector (S3 lee `strategy_scores` y `relay_scores` para scoring adaptativo) y a operadores humanos vía incident log.

## 1. Arquitectura

```
relays-client terminates execution → XADD arbx:opps:executed {opp_id, tx_hash, status, ...}
                                          │
                                          ▼ consumer group recon-g0
                                  recon consumer
                                          │
                                          ├─ fetch tx receipt via RPC_HTTP_1
                                          ├─ decode Swap / Transfer logs
                                          ├─ compute amount_out_actual in wei
                                          │
                                          ▼
                                  PnlEngine.compute(opp, receipt, logs)
                                          │
                                          ▼
                                  ReconReport (native units)
                                          │
                                          ├─ variance_pct > threshold → risk_event(severity=warning)
                                          │
                                          ▼
                                  Persist ReconReport (table recon_reports NEW)
                                          │
                                          ▼
                                  UPDATE opportunities.status='reconciled'
                                          │
                                          ▼
                                  XACK

┌──────────────────────────────────────────────────────────────┐
│  Periodic aggregator task (every reset_interval_s)           │
│                                                               │
│  1. Aggregate executions in last 1h by (strategy_kind,chain) │
│      → UPSERT strategy_scores with success_rate, revert_rate │
│  2. Aggregate executions by (relay_name, chain) → relay_scores│
│  3. Run anomaly_check:                                        │
│      - revert_rate > high_threshold → risk_event(critical) + │
│        opcional auto-trip kill_switch                        │
│      - anomalous inclusion drop → risk_event(warning)        │
└──────────────────────────────────────────────────────────────┘
```

## 2. Decisiones estructurales

| # | Decisión | Justificación |
|---|---|---|
| 1 | **Nueva tabla `recon_reports`** (migration 012). Distinta de `executions` porque es el análisis post-hoc, no el registro del submit. | Separa la intención (tx submitted) de la verdad (on-chain outcome analyzed). |
| 2 | **PnL en unidades nativas primero** (wei). USD conversion queda como S6.1 enriquecimiento via oracle Chainlink o Uniswap V3 TWAP. | Sin oracle, no inventamos precios — exigencia de honestidad. |
| 3 | **Receipt fetch con timeout 5s**. Si falla → recon marca `fail_reason="receipt_unavailable"` y no persiste ReconReport — solo un risk_event. | No persiste datos imprecisos. |
| 4 | **Variance threshold configurable**. `recon.variance_threshold_pct` default 20%. Outliers generan risk_event pero **no** revierten la persistencia. | Observabilidad sin pánico. |
| 5 | **Aggregator task corre cada `aggregator_interval_seconds`** (default 300s). Calcula windows del último `strategy_score_window_hours` (default 1h) y upsert al table. | Rolling no-overlap: cada ejecución aporta a un único window por strategy y relay. |
| 6 | **Anomaly detection simple en S6**: `revert_rate(15m) > anomaly_revert_rate_pct` (default 50%) → risk_event critical. ML en S7+. | Fácil de explicar; fácil de calibrar. |
| 7 | **Auto-kill-switch opcional**: flag `recon.auto_trip_on_high_revert_rate = true`. Si hay ≥ `anomaly_min_samples` (default 10) en la ventana Y revert_rate supera umbral → `KillSwitchClient.set(enabled=true, reason="auto_trip:high_revert_rate")`. | Fail-fast defensivo; operador puede deshabilitar. |
| 8 | **Consumer al-least-once + XACK post-persist**. Mismo patrón que S3/S4/S5. | Consistencia. |
| 9 | **Adaptive scoring influye a selector por DB, NO por config en vivo**. Selector-api lee `strategy_scores` + `relay_scores` cuando decide; S6 los actualiza. | Separación de concerns. |
| 10 | **NO migra el schema legacy de S1** — las 4 tablas `relay_scores`, `strategy_scores`, `risk_events`, `incident_log` ya existen desde S1. Solo añade `recon_reports`. | Respeta S1 decisions. |

## 3. Componentes nuevos / modificados

```
backend/recon/src/
  main.rs              — spawn consumer + aggregator + HTTP (ya existe)
  consumer.rs          — NEW: XREADGROUP arbx:opps:executed
  pnl_engine.rs        — NEW: receipt fetch + log decode + PnL compute
  variance.rs          — NEW: expected vs actual + outlier flagging
  aggregator.rs        — NEW: periodic strategy/relay score rollups
  anomaly.rs           — NEW: anomaly detection + risk_events + optional kill_switch
  persistence.rs       — NEW: ReconReport + risk_events + incident_log writes

backend/relays-client/src/
  consumer.rs          — modified: publish arbx:opps:executed after persist
  submit_engine.rs     — unchanged

database/migrations/
  012_recon_reports.sql — NEW

configs/app.toml:
  [recon] section — NEW

configs/schemas/app.schema.json:
  simulation schema — extended with [recon]
```

## 4. Schema nuevo (migration 012)

```sql
CREATE TABLE IF NOT EXISTS recon_reports (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  opportunity_id UUID NOT NULL REFERENCES opportunities(id) ON DELETE CASCADE,
  execution_id UUID REFERENCES executions(id),
  tx_hash TEXT,
  chain_id INTEGER NOT NULL,

  expected_amount_out_wei NUMERIC(78,0),
  actual_amount_out_wei NUMERIC(78,0),
  variance_native_units NUMERIC(78,0),
  variance_pct NUMERIC(10,4),

  expected_profit_usd NUMERIC(20,8),
  actual_profit_usd NUMERIC(20,8),
  pnl_source TEXT NOT NULL CHECK (pnl_source IN ('native_only','oracle_chainlink','oracle_uniswap_twap','derived','unavailable')),

  actual_gas_used_wei NUMERIC(78,0),
  actual_gas_price_wei NUMERIC(78,0),

  fail_reason TEXT,
  raw_receipt JSONB,
  trace_id UUID NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_recon_opp ON recon_reports(opportunity_id);
CREATE INDEX IF NOT EXISTS idx_recon_time ON recon_reports(created_at DESC);

GRANT SELECT, INSERT ON recon_reports TO arbx_rw;
GRANT SELECT ON recon_reports TO arbx_ro;
```

## 5. Config (additive)

```toml
[recon]
pnl_source_default = "native_only"           # "native_only"|"oracle_chainlink"|"oracle_uniswap_twap"
variance_threshold_pct = 20.0                # flag risk_event if |variance_pct| > this
receipt_fetch_timeout_ms = 5000
aggregator_interval_seconds = 300
strategy_score_window_hours = 1
anomaly_window_minutes = 15
anomaly_min_samples = 10
anomaly_revert_rate_pct = 50.0
auto_trip_on_high_revert_rate = true
```

JSON Schema gets corresponding entries.

## 6. Métricas nuevas

| Métrica | Tipo | Labels |
|---|---|---|
| `arbx_recon_reports_total` | counter | `chain_id`, `pnl_source` |
| `arbx_recon_receipt_fetch_errors_total` | counter | `chain_id`, `reason` |
| `arbx_recon_variance_pct` | histogram | `strategy_kind`, `chain_id` |
| `arbx_recon_anomalies_detected_total` | counter | `kind`, `severity` |
| `arbx_recon_strategy_score_updates_total` | counter | `strategy_kind`, `chain_id` |
| `arbx_recon_relay_score_updates_total` | counter | `relay`, `chain_id` |
| `arbx_recon_auto_trips_total` | counter | `reason` |

## 7. Contratos nuevos / extendidos

### `ReconReport` (Rust, shared-rs) — ya existe, extender con:

```rust
pub struct ReconReport {
    pub opportunity_id: Uuid,
    pub execution_id: Option<Uuid>,
    pub tx_hash: Option<String>,
    pub chain_id: u64,
    pub expected_amount_out_wei: Option<String>,
    pub actual_amount_out_wei: Option<String>,
    pub variance_native_units: Option<String>,
    pub variance_pct: Option<f64>,
    pub expected_profit_usd: f64,       // ya existe
    pub actual_profit_usd: f64,         // ya existe
    pub pnl_source: String,             // NEW
    pub actual_gas_used_wei: Option<String>,
    pub actual_gas_price_wei: Option<String>,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub trace_id: Uuid,
}
```

### Nuevo stream `arbx:opps:executed`

Publicado por relays-client tras persist_execution. Payload = Opportunity + ExecutionResult juntos:

```json
{
  "opportunity": { ... },
  "execution": { ... }
}
```

## 8. Anomaly detection — detalles

Query simple en aggregator (cada 5 min):

```sql
SELECT e.opportunity_id, o.strategy_kind, o.chain_id,
       COUNT(*) FILTER (WHERE e.status='reverted') AS reverts,
       COUNT(*) AS total
FROM executions e
JOIN opportunities o ON o.id = e.opportunity_id
WHERE e.submitted_at >= NOW() - INTERVAL '15 minutes'
GROUP BY o.strategy_kind, o.chain_id
HAVING COUNT(*) >= 10;
```

Si `reverts::FLOAT / total > anomaly_revert_rate_pct / 100`:
- INSERT `risk_events` severity=critical, event_type=degradation, payload={strategy, chain, rate}
- Si `auto_trip_on_high_revert_rate`: `KillSwitchClient.set(enabled=true, reason="auto_trip:revert_rate:<pct>")`

## 9. Fallos esperados

| Condición | Comportamiento |
|---|---|
| RPC_HTTP_1 unreachable | Fetch receipt fails with timeout → recon fails fast. `risk_event` severity=warning. NO persiste ReconReport inexacto. |
| Receipt not found (tx dropped) | ReconReport con `fail_reason="tx_not_found"`, `actual_*` null. Variance no calculable. Persiste. |
| Log decode fails (unknown ABI) | ReconReport con `actual_amount_out_wei=null`, `pnl_source="unavailable"`. |
| Kill-switch ON durante aggregator | Aggregator NO auto-trips (ya está ON). Logs skip. |
| Anomaly detected + auto_trip=false | Solo `risk_event` critical. Operador lee dashboard. |

## 10. Criterios de aceptación S6

- [ ] Migration 012 aplica limpia (tabla recon_reports creada).
- [ ] `cargo test -p recon` pasa ≥ 4 tests (variance calc, anomaly detector, pnl native compute).
- [ ] Consumer procesa msg de `arbx:opps:executed` y persiste `recon_reports`.
- [ ] Aggregator task escribe `strategy_scores` y `relay_scores` cada 5 min con datos reales.
- [ ] Variance > 20% genera `risk_events` severity=warning.
- [ ] Revert_rate > 50% durante ≥10 muestras → `risk_event` critical + (si auto_trip=true) kill_switch activa.
- [ ] Selector-api en siguiente iteración puede leer `strategy_scores`/`relay_scores` para pesos adaptativos (consumo, no parte de S6 core).

## 11. Fuera de scope S6

- **USD conversion via oracle real** (Chainlink / Uni V3 TWAP) → S6.1.
- **ML-based scoring** → S7+.
- **Incident correlator con grouping** → S7.
- **Frontend timeline de incidents** → S7.
- **Backtest histórico** → out-of-roadmap.

## 12. Honestidad

- Sin receipt → NO persiste PnL. Solo risk_event explícito.
- `pnl_source="native_only"` cuando no hay oracle — marca explícita.
- Variance=null cuando amount_out no es decodificable, NO se infiere.
- Auto-kill-switch siempre registra `triggered_by="recon_auto:<reason>"` en `KillSwitchState`.
- Aggregator upsert idempotente: misma ventana + misma strategy = mismo row updated.
