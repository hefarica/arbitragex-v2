# API Contracts — ArbitrageX v2

Canonical fuente de verdad vive en **`configs/schemas/*.json`** (JSON Schema Draft 2020-12).
Este documento los resume y fija el pipeline operativo.

## Pipeline canónico

```
Detect → Validate-cheap → Simulate → Select/Rank → Fund → Execute → Recon → Learn
```

⚠️ Simular se ejecuta **antes** de Select/Rank para ejecución (corrección respecto a
scaffolds anteriores que invertían el orden).

## Servicios y endpoints — Sprint 1

### searcher-rs  (puerto 9001, Rust)

| Verb | Path       | S1 behavior |
|------|------------|-------------|
| GET  | `/health`  | `200 {ok, service, version, uptime_s}` |
| GET  | `/metrics` | Prometheus text exposition |

Pub/Sub (Sprint 2+): `XADD arbx:opps:detected *` en Redis. No activo en S1.

### selector-api  (puerto 3002, Node+TS)

| Verb | Path             | S1 behavior |
|------|------------------|-------------|
| GET  | `/health`        | 200 |
| GET  | `/metrics`       | Prometheus |
| POST | `/score`         | Body: `{opportunity, simulation?, safety_score?}`. Returns `ScoredOpportunity {score, decision, reason, factors}`. 400 on invalid. |
| GET  | `/opportunities` | Query `status`, `limit`. Lee DB real. |

### sim-ctl  (puerto 3003, Rust)

| Verb | Path        | S1 behavior |
|------|-------------|-------------|
| GET  | `/health`   | 200 |
| GET  | `/metrics`  | Prometheus |
| POST | `/simulate` | **501 `NotImplementedPayload{requires:["ANVIL_FORK_URL","RPC_HTTP_<chain_id>"], sprint:"S4"}`**. |

### relays-client  (puerto 3005, Rust)

| Verb | Path       | S1 behavior |
|------|------------|-------------|
| GET  | `/health`  | 200 |
| GET  | `/metrics` | Prometheus |
| POST | `/execute` | Killswitch ON → 503. Killswitch OFF → **501** `{requires:["FLASHBOTS_SIGNER_KEY","FLASHBOTS_RELAY_URL"], sprint:"S5"}`. **Nunca retorna tx_hash**. |

### recon  (puerto 3004, Rust)

| Verb | Path                                | S1 behavior |
|------|-------------------------------------|-------------|
| GET  | `/health`                           | 200 |
| GET  | `/metrics`                          | Prometheus |
| GET  | `/pnl/:opportunity_id`              | 200 con datos DB reales; **404** si no hay execution. No fabrica. |
| GET  | `/pnl/summary?since=<iso>`          | Agrega filas reales; si `sample_count=0`, se expone explícito. |

### api-server  (puerto 8080, Node+TS — NUEVO)

| Verb | Path                                   | S1 behavior |
|------|----------------------------------------|-------------|
| GET  | `/health`                              | 200 |
| GET  | `/metrics`                             | Prometheus |
| GET  | `/status`                              | Agrega healths de upstreams + killswitch state. |
| POST | `/admin/killswitch`                    | Requiere header `X-ArbX-Admin-Token`. Body `{enabled, reason?, triggered_by?}`. 401 si token ausente/erróneo. |
| GET  | `/admin/config`                        | Requiere admin token. Retorna config efectiva (sin secretos). |

### edge  (puerto 8787 local / Workers URL prod)

| Verb | Path                         | S1 behavior |
|------|------------------------------|-------------|
| GET  | `/health`                    | 200 |
| GET  | `/metrics`                   | Prometheus (sólo dev-local — el worker expone su propio formato en S7) |
| GET  | `/status`                    | Proxy + cache 2s a `api-server /status`. Inyecta `X-ArbX-Edge-Token`. |
| GET  | `/api/opportunities/live`    | Proxy + cache 2s read-only |
| GET  | `/api/risk/alerts`           | Proxy read-only |

Rate-limit S1: in-memory per-isolate/IP, 120 req/min. **Documentado como temporal**; S7 migra a KV-backed.

### frontend  (puerto 5173, Next.js)

- `/` — index con link a `/status`.
- `/status` — consume `edge /status`. Si edge falla, **muestra error explícito**, no valores falsos.

## Headers internos

| Header                 | Propósito | Quién lo pone |
|------------------------|-----------|---------------|
| `X-ArbX-Trace-Id`      | Correlación end-to-end | Edge genera si ausente; cada servicio lo propaga. |
| `X-ArbX-Edge-Token`    | Auth interno edge→api-server | Edge inyecta. |
| `X-ArbX-Admin-Token`   | Auth para `/admin/*` | Operador humano / CI. |
| `X-ArbX-Actor`         | Identifica al actor admin (opcional) | Operador al invocar killswitch. |

## Contratos de tipos

Ver JSON Schemas:
- `configs/schemas/opportunity.schema.json`
- `configs/schemas/simulation_result.schema.json`
- `configs/schemas/execution_request.schema.json`
- `configs/schemas/execution_result.schema.json`
- `configs/schemas/recon_report.schema.json`
- `configs/schemas/killswitch_state.schema.json`
- `configs/schemas/app.schema.json`

Implementaciones:
- Rust: `backend/shared-rs/src/contracts.rs` + `config.rs`.
- TS: `shared-ts/src/contracts/index.ts` + `config/index.ts`.

## Errores estructurados

Todos los endpoints devuelven errores como JSON con forma estable:

```json
{ "error": "invalid_request" | "not_implemented" | "unauthorized" | "rate_limited" | "db_error" | "...",
  "detail": "string | object",
  "requires": ["ENV_VAR_NAME"],
  "sprint": "S4" }
```

El campo `requires` y `sprint` sólo aparecen en 501.
