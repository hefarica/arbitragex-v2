# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# ArbitrageX v2 â€” Sprint 3 "Selector + Scoring + Risk gates" â€” Design Spec

**Fecha**: 2026-04-21
**Sprint**: 3 de 8
**Depende de**: S1 + S2 cerrados (searcher-rs publicando a `arbx:opps:detected`, 9 tablas SQL operativas).
**Nuevas credenciales opcionales**: `GOPLUS_API_KEY` (token safety). Sin ella, fallback a heurÃ­stica interna + estado `unknown`.

## 0. Objetivo

Convertir la pipa de detecciÃ³n bruta de S2 en **decisiones accept/reject trazables** con filtrado multi-factor, token-safety, blacklist dinÃ¡mica y circuit breakers activos. Al cierre de S3:

- Las oportunidades de `arbx:opps:detected` son consumidas por `selector-api`, enriquecidas y decididas.
- Cada decisiÃ³n queda persistida en `opportunities.status` + `risk_events` (si rechazo).
- Circuit breakers se disparan automÃ¡ticamente ante condiciones adversas y publican risk_event.
- Blacklist/whitelist de tokens y pairs es dinÃ¡mica vÃ­a admin endpoints.

## 1. Pipeline de decisiÃ³n

```
arbx:opps:detected (Redis Stream)
           â”‚
           â–¼  (consumer group: "selector-g0", ack manual)
    â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
    â”‚          selector-api consumer           â”‚
    â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
           â”‚
           â–¼
    â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
    â”‚          PolicyEngine.prefilter()        â”‚
    â”‚  1. kill-switch? â†’ drop + metric         â”‚
    â”‚  2. blacklist token_in/token_out?        â”‚
    â”‚  3. circuit breaker "detection" open?    â”‚
    â”‚  Pasa â†’ seguir; Falla â†’ reject + event   â”‚
    â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
           â”‚
           â–¼
    â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
    â”‚          TokenSafety.check_pair()        â”‚
    â”‚  - TTL cache hit? â†’ usar                 â”‚
    â”‚  - miss â†’ GoPlus API (si hay key)        â”‚
    â”‚  - sin key â†’ heurÃ­stica interna          â”‚
    â”‚  Resultado: safety_score 0â€“100           â”‚
    â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
           â”‚
           â–¼
    â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
    â”‚          ScoreEngine.score()             â”‚
    â”‚  Factores (cada uno 0â€“100, pesos config):â”‚
    â”‚   - liquidity   (0.20)                   â”‚
    â”‚   - depth       (0.15)                   â”‚
    â”‚   - safety      (0.20)                   â”‚
    â”‚   - slippage    (0.15)                   â”‚
    â”‚   - gas         (0.15)                   â”‚
    â”‚   - risk        (0.15)                   â”‚
    â”‚  score = Î£(factor Ã— peso)                â”‚
    â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
           â”‚
           â–¼
    â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
    â”‚          PolicyEngine.decide()           â”‚
    â”‚  - score < MIN_ACCEPT_SCORE â†’ reject     â”‚
    â”‚  - safety < min_token_safety_score â†’ rej â”‚
    â”‚  - revert_risk_pct > thresh â†’ reject     â”‚
    â”‚  â†’ accept: status='validated'            â”‚
    â”‚  â†’ reject: status='rejected' + risk_evt  â”‚
    â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
           â”‚
           â–¼
    UPDATE opportunities SET status, risk_score, rejection_reason
    INSERT risk_events (if rejected with high severity)
    XACK / XADD en stream siguiente (arbx:opps:validated)
    incrementar contadores
```

## 2. Decisiones estructurales

| # | DecisiÃ³n | JustificaciÃ³n |
|---|---|---|
| 1 | **Redis Streams** con consumer groups (`XREADGROUP`, `XACK`). Group por servicio `selector-g0`. | Permite mÃºltiples instancias del selector en el futuro sin pÃ©rdida ni duplicados. XACK manual = at-least-once semantics. |
| 2 | **Factor calculators son puros** (input â†’ score 0â€“100). NingÃºn I/O. | Testeable sin infraestructura. Deterministic. |
| 3 | **ScoreEngine lee pesos desde `configs/app.toml`**, no hardcode. | Iterables por operador. En S6 el learning loop escribirÃ¡ estos pesos de vuelta. |
| 4 | **TokenSafety es async con timeout duro 1.5 s** por call externa; si timeout, retorna `unknown` + enqueue background refresh. No bloquea la decisiÃ³n. | Latencia dominante en el pipeline. |
| 5 | **Circuit breakers con 3 estados**: `closed â†’ open â†’ half_open â†’ closed`. Se instancian por "nombre" (ej. `token_safety_api`, `db_writes`, `stream_consumer`). Ventana deslizante N errores en T segundos. | PatrÃ³n estÃ¡ndar; previene cascadas. |
| 6 | **Blacklist en Redis SET** (`arbx:blacklist:tokens:1` para chain 1). TTL opcional por entrada. Admin endpoint para CRUD. | RÃ¡pido (O(1) check), centralizado, sobrevive reinicios. |
| 7 | **Decisiones persisten vÃ­a UPDATE de `opportunities`** (no insert nuevo row). `rejection_reason` describe el motivo si aplica. | La oportunidad ya existe desde S2; solo avanza estado. |
| 8 | **`risk_events` se escribe en rejects con severity â‰¥ warning** (safety_below_min, blacklist_hit, cb_tripped). No en rechazos triviales (score bajo). | Evita ruido; solo eventos accionables. |
| 9 | **Publish a `arbx:opps:validated`** solo si accept. Consumers futuros (sim-ctl en S4) leerÃ¡n este stream. | Pipeline clara: detected â†’ validated â†’ simulated â†’ â€¦ |
| 10 | **OperaciÃ³n idempotente**: si un mismo opportunity_id llega dos veces del stream, el UPDATE es no-op (status ya avanzado). | Resilencia a reprocesamiento. |

## 3. Componentes a construir

### 3.1 shared-ts: CircuitBreaker

```
shared-ts/src/circuit_breaker/
  index.ts       â€” clase CircuitBreaker + tipos
  index.test.ts  â€” tests
```

API:

```ts
class CircuitBreaker {
  constructor(opts: { name, threshold, window_ms, cooldown_ms, half_open_probe_count })
  execute<T>(fn: () => Promise<T>): Promise<T>   // lanza si estÃ¡ open
  state(): "closed" | "open" | "half_open"
  trip(reason: string): void
  reset(): void
  on(ev: "trip" | "reset", cb: (s) => void): void
}
```

MÃ©tricas:
- `arbx_cb_state{name, service}` gauge
- `arbx_cb_trips_total{name, service, reason}` counter

### 3.2 selector-api: Consumer de streams

```
backend/selector-api/src/
  consumer.ts           â€” loop XREADGROUP + dispatch
  scoring/
    factors.ts          â€” calc de cada factor individual
    engine.ts           â€” combinaciÃ³n con pesos
    engine.test.ts
  token_safety/
    client.ts           â€” fachada de proveedores
    goplus.ts           â€” cliente GoPlus
    internal_heuristic.ts â€” fallback sin API key
    cache.ts            â€” CRUD token_safety_cache (TTL, upsert)
  policy/
    engine.ts           â€” prefilter + decide
    blacklist.ts        â€” Redis SET ops
    engine.test.ts
  persistence.ts        â€” UPDATE opportunities, INSERT risk_events
```

### 3.3 api-server: Admin endpoints

Extensiones a `backend/api-server/src/index.ts`:

| Verb | Path | Body / Query | FunciÃ³n |
|------|------|--------------|---------|
| `POST` | `/admin/blacklist/tokens` | `{chain_id, token_address, reason?, ttl_s?}` | AÃ±ade a blacklist |
| `DELETE` | `/admin/blacklist/tokens/:chain/:addr` | â€” | Remueve |
| `GET` | `/admin/blacklist/tokens?chain_id=` | â€” | Lista |
| `GET` | `/admin/circuit_breakers` | â€” | Estado de todos los CBs |
| `POST` | `/admin/circuit_breakers/:name/reset` | â€” | Manual reset |
| `POST` | `/admin/circuit_breakers/:name/trip` | `{reason}` | Manual trip (emergency) |
| `GET` | `/admin/scoring/weights` | â€” | Pesos efectivos |

Todos requieren `X-ArbX-Admin-Token` + escriben a `audit_log`.

### 3.4 Config additions a `configs/app.toml`

```toml
[scoring]
min_accept_score = 55.0
weight_liquidity = 0.20
weight_depth = 0.15
weight_safety = 0.20
weight_slippage = 0.15
weight_gas = 0.15
weight_risk = 0.15

[token_safety]
ttl_seconds_ok = 3600
ttl_seconds_bad = 86400
provider = "goplus"   # "goplus" | "honeypot_is" | "internal_only"
api_call_timeout_ms = 1500
min_acceptable_score = 70

[circuit_breakers]
[[circuit_breakers.instance]]
name = "token_safety_api"
threshold = 5
window_ms = 60000
cooldown_ms = 120000

[[circuit_breakers.instance]]
name = "db_writes"
threshold = 10
window_ms = 30000
cooldown_ms = 60000

[[circuit_breakers.instance]]
name = "stream_consumer"
threshold = 20
window_ms = 60000
cooldown_ms = 30000
```

Schema update: `configs/schemas/app.schema.json` con secciones `scoring`, `token_safety`, `circuit_breakers` (backwards-compatible: si ausentes, usar defaults en cÃ³digo).

### 3.5 MÃ©tricas nuevas

| MÃ©trica | Tipo | Labels |
|---|---|---|
| `arbx_selector_decisions_total` | counter | `decision`, `reason`, `chain_id` |
| `arbx_selector_score_bucket` | histogram | `decision` |
| `arbx_selector_consumer_lag` | gauge | `stream`, `group` |
| `arbx_selector_consumer_processing_seconds` | histogram | â€” |
| `arbx_token_safety_calls_total` | counter | `provider`, `result` |
| `arbx_token_safety_cache_hits_total` | counter | `hit` (true/false) |
| `arbx_blacklist_hits_total` | counter | `chain_id`, `reason` |
| `arbx_cb_state` | gauge | `name` |
| `arbx_cb_trips_total` | counter | `name`, `reason` |

### 3.6 Factor calculators (detalle)

Input: `{ opportunity, sim_result | null, safety_score }`. Output: `0â€“100`.

**liquidity**: proxy basado en `expected_profit_usd` y `amount_in_wei`.
```
liquidity = clamp(0, 100, log10(max(1, expected_profit_usd * 20)) * 20)
```

**depth**: si hay `dex_b` y diferente de `dex_a` â†’ 60 base; single-dex â†’ 40. Placeholder hasta S6.

**safety**: `safety_score` del cache/provider directamente (0â€“100).

**slippage**: `sim_result.slippage_pct` invertido; `null` â†’ 50 neutral.
```
slippage_factor = clamp(0, 100, 100 - slippage_pct * 30)
```

**gas**: `sim_result.gas_estimate_wei` vs `max_gas_price_gwei` del config. Sin sim â†’ 40 conservador.

**risk**: `sim_result.revert_risk_pct` invertido; sin sim â†’ 50 neutral.
```
risk_factor = clamp(0, 100, 100 - revert_risk_pct)
```

## 4. Reglas de decisiÃ³n (PolicyEngine.decide)

```
function decide({ scored, safety, sim }): Decision {
  // 1. Kill-switch (pre-score, consultado antes)
  // 2. Safety hard floor
  if (safety < cfg.token_safety.min_acceptable_score)
    return reject("safety_below_threshold", severity=warning)

  // 3. Simulation hard floor (sÃ³lo si hubo sim)
  if (sim && sim.passed === false)
    return reject("simulation_failed", severity=info)

  // 4. Score threshold
  if (scored.score < cfg.scoring.min_accept_score)
    return reject("score_below_min", severity=info)

  // 5. Revert risk hard cap
  if (sim && sim.revert_risk_pct > cfg.risk.max_revert_rate_pct * 2)
    return reject("revert_risk_too_high", severity=warning)

  return accept(scored.score, scored.factors)
}
```

## 5. Contratos DB actualizados

Sin cambios de schema. Nuevos flujos:

- **UPDATE opportunities**: `SET status='validated', risk_score=<score>, updated_at=NOW() WHERE id=$1`
- **UPDATE opportunities**: `SET status='rejected', rejection_reason=$2, updated_at=NOW() WHERE id=$1`
- **INSERT risk_events**: `(event_type='blacklist_hit'|'circuit_breaker'|..., severity, source_service='selector-api', payload, trace_id, opportunity_id)`
- **UPSERT token_safety_cache**: `INSERT ... ON CONFLICT (chain_id, token_address) DO UPDATE SET ...`

## 6. Fallos esperados

| CondiciÃ³n | Comportamiento |
|---|---|
| GoPlus API 429/timeout | CB `token_safety_api` cuenta; si no trip, fallback a cache o `internal`; si trip, todas las decisiones usan `unknown` (conservador reject). |
| DB unreachable | CB `db_writes` trip. Consumer se pausa (`NACK` / no XACK) hasta recuperaciÃ³n. MÃ©tricas siguen reportando. |
| Stream vacÃ­o > 30 s | Log info `consumer.idle`, sin alarmar. |
| Message malformado (JSON invÃ¡lido) | Increment `arbx_selector_invalid_messages_total`, XACK (skip), escribe `risk_events` severity=info. |
| Kill-switch ON | Loop pausa consumo, log cada 5 s. |

## 7. Criterios de aceptaciÃ³n S3

- [ ] `vitest run -w @arbx/selector-api` pasa (â‰¥ 15 tests: 3 de cada factor + 6 del policy + 3 del cb).
- [ ] `vitest run -w @arbx/shared` pasa con tests de circuit breaker.
- [ ] Con searcher-rs idle (no RPC): selector-api sube, consumer loop log `idle`, `arbx_selector_decisions_total == 0`.
- [ ] Con fixtures manuales (XADD sintÃ©tico a `arbx:opps:detected`): decisiÃ³n correcta en < 50 ms p95, oportunidad actualiza status en DB, mÃ©trica incrementa.
- [ ] Blacklist: `POST /admin/blacklist/tokens` aÃ±ade, next opp con ese token â†’ rejected con reason `blacklist_hit`.
- [ ] Circuit breaker manual trip: `POST /admin/circuit_breakers/token_safety_api/trip` â†’ todas las decisiones rechazan con `safety_unknown` hasta cooldown.

## 8. Fuera de scope S3

- **SimulaciÃ³n**: sim-ctl sigue 501 hasta S4. Score no invoca simulator.
- **EjecuciÃ³n privada**: relays-client sigue 501 hasta S5.
- **Learning loop**: pesos fijos desde config; S6 los hace adaptativos.
- **Auth productivo** para admin: sigue token estÃ¡tico X-ArbX-Admin-Token hasta S7.
- **HA multi-instancia**: un solo consumer por group en S3. Horizontalizable en S8.

## 9. Honestidad (no-fabrication)

- Sin `GOPLUS_API_KEY` â†’ `token_safety.source='internal'`, score calculado por heurÃ­stica documentada (address length, zero bytes, decimals sane); **nunca** retorna un score inventado proveniente de "provider":"goplus" sin haber llamado.
- DecisiÃ³n siempre persiste el motivo en `rejection_reason` o se deja NULL si accept. No hay rejects silenciosos.
- Stream acknowledge SOLO tras persistencia exitosa (at-least-once). Si crash a media vuelta, el prÃ³ximo boot reprocesa (operaciÃ³n idempotente garantiza consistencia).

