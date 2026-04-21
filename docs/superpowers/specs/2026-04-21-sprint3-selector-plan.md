# Sprint 3 — Plan de implementación

Orden lineal. Cada paso debe compilar / pasar tests antes del siguiente.

## Paso 1 — shared-ts: CircuitBreaker

**Archivos**:
- `shared-ts/src/circuit_breaker/index.ts`
- `shared-ts/src/circuit_breaker/index.test.ts`

**Export** desde `shared-ts/src/index.ts`.

**Tests** (min 5):
- `closed → open` cuando N fallos en T segundos
- `open → half_open` tras cooldown
- `half_open → closed` tras M successes
- `half_open → open` si falla durante probe
- `trip()` manual + `reset()` manual

**Validación**: `npm run -w @arbx/shared build && npm run -w @arbx/shared test`.

## Paso 2 — Config + Schema updates

**Archivos**:
- `configs/app.toml` — añadir `[scoring]`, `[token_safety]`, `[[circuit_breakers.instance]]`
- `configs/schemas/app.schema.json` — extensiones
- `shared-ts/src/config/index.ts` — Zod schema + loader
- `backend/shared-rs/src/config.rs` — structs + parse

**Validación**: `python3 automation/tools/validate-config.py configs/app.toml configs/schemas/app.schema.json`.

## Paso 3 — selector-api: scoring engine (puro)

**Archivos**:
- `backend/selector-api/src/scoring/factors.ts` — 6 calculators puros
- `backend/selector-api/src/scoring/engine.ts` — combinación + pesos desde config
- `backend/selector-api/src/scoring/engine.test.ts`

**Refactor**: el `score.ts` existente de S1 queda como shim que importa desde `scoring/`.

**Tests** (≥ 6):
- Cada factor calc con input extremo (0, 100, negativo, null)
- Engine combina correctamente los pesos
- Score con todos los factores altos > umbral
- Score con safety 30 rechaza

## Paso 4 — selector-api: token safety

**Archivos**:
- `backend/selector-api/src/token_safety/cache.ts` — UPSERT/SELECT contra `token_safety_cache`
- `backend/selector-api/src/token_safety/internal_heuristic.ts` — heurística sin API
- `backend/selector-api/src/token_safety/goplus.ts` — cliente HTTP (fetch)
- `backend/selector-api/src/token_safety/client.ts` — fachada: cache first, fallback chain
- `backend/selector-api/src/token_safety/client.test.ts`

**Tests**:
- Cache hit → no llama provider
- Cache miss + sin API key → internal heuristic
- Cache miss + con API key → goplus; mock HTTP
- Timeout → safe_default `unknown` (score=0)

## Paso 5 — selector-api: policy + blacklist

**Archivos**:
- `backend/selector-api/src/policy/blacklist.ts` — Redis SADD/SREM/SISMEMBER
- `backend/selector-api/src/policy/engine.ts` — prefilter + decide
- `backend/selector-api/src/policy/engine.test.ts`

**Tests** (≥ 6):
- prefilter: kill-switch ON → reject
- prefilter: token en blacklist → reject with reason=blacklist_hit
- prefilter: cb token_safety open → reject with reason=cb_open
- decide: safety < 50 → reject
- decide: score < min → reject
- decide: acceptable → accept

## Paso 6 — selector-api: persistence

**Archivos**:
- `backend/selector-api/src/persistence.ts` — updateOpportunityStatus, insertRiskEvent

Usa el `pg.Pool` ya configurado en `index.ts`.

## Paso 7 — selector-api: consumer loop

**Archivos**:
- `backend/selector-api/src/consumer.ts` — XREADGROUP + dispatch + XACK

**Integra**: policy prefilter → token_safety → scoring → policy decide → persistence → publish validated (si accept).

**main** (`index.ts`) spawna consumer como task separada.

## Paso 8 — api-server: admin endpoints

**Archivos**:
- `backend/api-server/src/admin_routes.ts` — nuevo módulo con routes
- Wire en `backend/api-server/src/index.ts`

**Endpoints** (spec §3.3) con auditoría a `audit_log`.

## Paso 9 — Métricas

Añadir 9 métricas nuevas a `shared-ts/src/metrics/index.ts` (igual patrón que S1/S2).

## Paso 10 — Tests E2E locales

`backend/selector-api/src/e2e.test.ts` — levanta postgres (testcontainers o ensureDb), inserta opportunity fixture, ejecuta consumer.processOnce, verifica status.

(Marcado `describe.skipIf(!process.env.ARBX_E2E)` — opcional en CI; principal validación en Sprint 8).

## Paso 11 — Validación final

- `npm run -w @arbx/shared typecheck && npm run -w @arbx/shared test`
- `npm run -w @arbx/selector-api typecheck && npm run -w @arbx/selector-api test`
- `python3 automation/tools/validate-config.py ...`
- Fake-data marker grep: `grep -rnE "TODO|FIXME|FAKE|MOCK|placeholder" backend/selector-api/src shared-ts/src | grep -v ".test.ts"` → 0 matches fuera de tests.

## Paso 12 — Commit + push

Un solo commit grande descriptivo: `feat(selector): S3 scoring + token safety + policy + circuit breakers`.

## Out-of-scope S3 (explícito)

- Cliente GoPlus real (solo skeleton + mock en test). Integration real S3.1 cuando usuario tenga API key.
- HA multi-instance consumer. Un solo consumer group member por ahora.
- Auto-tuning de pesos. Los pesos son estáticos desde config.
- Reintento exponencial de DB writes. Fail → CB trip → consumer pause.
