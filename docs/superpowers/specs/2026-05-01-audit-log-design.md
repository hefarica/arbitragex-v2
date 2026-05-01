# ArbitrageX v2 — PR-2 "Audit log: killswitch + auth events" — Design Spec

**Fecha**: 2026-05-01
**Sprint**: 1 closure (item 7 del checklist Sprint 1)
**Depende de**: V-AT-1 (admin session via httpOnly cookie, commit `ce32fb4`); migración 011 (`audit_log` table) ya aplicada; helper `writeAudit()` en `backend/api-server/src/index.ts`.
**No depende de**: searcher-rs hot path; PR-3 RPC failover; PR-7 frontend exchange terminal.
**Doctrinas aplicables**: arbx-no-hardcode-doctrine; arbx-pre-edit-audit (TDD); honesty (no synth on failure).

## 0. Objetivo

Cerrar dos brechas concretas en la trazabilidad de acciones admin:

1. **Killswitch sin audit**: el handler `POST /admin/killswitch` toggles el estado pero NO escribe a `audit_log`. Es la única acción mutadora de admin que carece de trail. Esta brecha es la más crítica porque killswitch es exactamente el evento que un postmortem de incidente buscaría primero.
2. **Auth events del edge sin audit**: `POST /admin/session` (login), `/admin/session/logout`, y los eventos derivados (rate-limit, lockout) ocurren íntegramente en el edge y no quedan en `audit_log`. Sin esto, un atacante que hiciera brute-force hasta lockout y luego pasara al primer intento exitoso post-lockout no dejaría rastro estructurado.

Al cierre de PR-2:

- Cada toggle del killswitch (armed o disabled) genera 1 row en `audit_log` con before/after state, reason, actor.
- Cada uno de los 6 eventos de auth (login_ok, login_fail, logout, lockout_triggered, rate_limited, locked_attempt) genera 1 row con `actor` ranqueado (token-fingerprint en éxito, "anonymous" en fallo).
- Si `api-server` está caído o lento, el edge degrada a `console.error` (Loki captura) y NO sintetiza filas — la honesty doctrine prevalece sobre la completud.

## 0.1. Lo que NO está en este PR

- ❌ Frontend viewer `/audit-logs` (planeado para PR-2.b futuro).
- ❌ Retention policy (TTL/partitioning mensual) — la tabla actual crece indefinidamente; aceptado para Sprint 1 close. PR-2.b lo cubre.
- ❌ Audit de `/admin/config` reads u otras lecturas non-mutadoras (decisión Q4).
- ❌ Reemplazar `actor TEXT` por FK a futura tabla `users` (Auth0 territory, Sprint 4).
- ❌ Firma criptográfica de filas para tamper-detection más fuerte (DB-level `REVOKE UPDATE/DELETE` de migración 011 es suficiente para Sprint 1).

## 1. Pipeline de eventos

```
┌────────────┐  POST /admin/session       ┌──────────────────────┐
│  operator  │ ─────────────────────────► │  edge handler        │
│  browser   │                            │  (dev-local OR CF)   │
└────────────┘ ◄─────────────────────────  └──────────────────────┘
                  set-cookie httpOnly        │
                                             │  rate-limit OK?
                                             │  lockout?
                                             │  api-server probe?
                                             │
                                             ├──► fire-and-forget (≤2s timeout)
                                             │    POST /internal/audit/auth
                                             │    {action, actor, ip, ua, target, after}
                                             │    headers: x-arbx-edge-token
                                             ▼
                                  ┌──────────────────────┐  writeAudit()  ┌───────────┐
                                  │     api-server       │ ──────────────►│ audit_log │
                                  └──────────────────────┘                └───────────┘
                                             ▲
                                             │  on emit failure (timeout / 5xx / network):
                                             │      console.error({event:"audit.emit_failed",...})
                                             │      → Loki via stderr
                                             │      → handler proceeds normally (operator unaffected)


┌────────────┐  POST /admin/killswitch    ┌──────────────────────┐  writeAudit("killswitch.armed"|".disabled")
│  operator  │ ─────────────────────────► │  api-server          │ ──────────────────────────────────────► audit_log
│  CLI/UI    │                            │  (toggleSwitch)      │
└────────────┘                            └──────────────────────┘
```

### 1.1. Eventos detallados (8 actions)

| # | Action | Origen | Cuándo | Actor | target_kind / target_id |
|---|---|---|---|---|---|
| 1 | `killswitch.armed` | api-server | POST `/admin/killswitch` con `enabled=true` | header `x-arbx-actor` o `"admin"` | `killswitch` / `global` |
| 2 | `killswitch.disabled` | api-server | POST `/admin/killswitch` con `enabled=false` | idem | `killswitch` / `global` |
| 3 | `auth.login_ok` | edge → api-server | probe a api-server retorna 2xx → cookie set | `tok:<sha256(token)[:8]>` | `ip` / `<ip>` |
| 4 | `auth.login_fail` | edge → api-server | probe retorna 401/403 | `anonymous` | `ip` / `<ip>` |
| 5 | `auth.logout` | edge → api-server | POST `/admin/session/logout` | `tok:<hex8>` si había cookie válido; `anonymous` si no | `ip` / `<ip>` |
| 6 | `auth.lockout_triggered` | edge → api-server | 10º fallo consecutivo → IP bloqueada 15min | `anonymous` | `ip` / `<ip>` |
| 7 | `auth.rate_limited` | edge → api-server | 6º intento dentro de ventana de 60s | `anonymous` | `ip` / `<ip>` |
| 8 | `auth.locked_attempt` | edge → api-server | request mientras IP ya está locked-out | `anonymous` | `ip` / `<ip>` |

### 1.2. Token fingerprint

`actor = "tok:" + sha256(adminToken).substring(0, 8)` (hex). Este valor:

- **NO** revela el token (no reversible, hash criptográfico).
- **SÍ** correlaciona múltiples eventos del mismo token holder.
- Idéntico entre dev-local y CF Worker (mismo algoritmo, mismo input).
- Para `auth.login_fail`, el token submitted es typically inválido — NO hasheamos el wrong-token (decisión Q3): `actor=anonymous`, contexto suficiente vía `ip_address`.

### 1.3. Ejemplos de filas resultantes

**`auth.login_ok`**:
```sql
actor       = 'tok:a3f9b2c4'
action      = 'auth.login_ok'
target_kind = 'ip'
target_id   = '192.0.2.1'
before_state= NULL
after_state = '{"remaining_rate":4}'
ip_address  = '192.0.2.1'
user_agent  = 'Mozilla/5.0 (...)'
trace_id    = '<uuid del request>'
```

**`killswitch.armed`**:
```sql
actor       = 'admin'   -- o x-arbx-actor header del CLI
action      = 'killswitch.armed'
target_kind = 'killswitch'
target_id   = 'global'
before_state= '{"enabled":false}'
after_state = '{"enabled":true,"reason":"investigating revert spike"}'
ip_address  = '<operator IP>'
trace_id    = '<uuid>'
```

**`auth.lockout_triggered`**:
```sql
actor       = 'anonymous'
action      = 'auth.lockout_triggered'
target_kind = 'ip'
target_id   = '203.0.113.7'
before_state= NULL
after_state = '{"blocked_until_ms":1714571520000,"total_fails":10}'
ip_address  = '203.0.113.7'
trace_id    = '<uuid>'
```

## 2. Componentes

### 2.1. Archivos nuevos (4)

| Archivo | Responsabilidad | Líneas aprox |
|---|---|---|
| `edge/dev-local/src/audit-emit.ts` | Helper `emitAuditEvent()` para Express edge. Fire-and-forget con timeout 2s, fallback a `console.error`. Pure module — testeable sin levantar Express. | ~50 |
| `edge/worker/src/audit-emit.ts` | Misma API, implementación Hono + `fetch` global. Compatible con CF Worker isolate. | ~50 |
| `backend/api-server/src/audit-events.test.ts` | Vitest, 10 tests. TDD: red → green → refactor. | ~160 |
| `edge/dev-local/src/audit-emit.test.ts` | Vitest, 3 tests del helper. | ~70 |

### 2.2. Archivos modificados (3)

| Archivo | Cambios |
|---|---|
| `backend/api-server/src/index.ts` | (a) En handler `/admin/killswitch`: dos `writeAudit()` calls (uno por rama armed/disabled). (b) Nuevo route `POST /internal/audit/auth` que valida `x-arbx-edge-token` y llama `writeAudit()` con los campos de body. ~40 líneas añadidas. |
| `edge/dev-local/src/index.ts` | Import `emitAuditEvent`. 5 llamadas en `/admin/session` (líneas relevantes: rate-limit hit, lockout check, login fail, login ok, locked attempt). 1 llamada en `/admin/session/logout`. ~30 líneas añadidas. |
| `edge/worker/src/index.ts` | Mismo patrón con CF Worker context. ~30 líneas añadidas. |

### 2.3. Helper `emitAuditEvent` — signature

```typescript
// edge/dev-local/src/audit-emit.ts (Express variant)
// edge/worker/src/audit-emit.ts (CF Worker variant; same exports)

export type AuditAction =
  | "auth.login_ok"
  | "auth.login_fail"
  | "auth.logout"
  | "auth.lockout_triggered"
  | "auth.rate_limited"
  | "auth.locked_attempt";

export type AuditEmitInput = {
  action: AuditAction;
  actor: string;             // "tok:<hex8>" or "anonymous"
  ipAddress: string;
  userAgent?: string;
  targetId?: string;         // typically the IP for auth events
  afterState?: Record<string, unknown>;
  traceId?: string;
};

/**
 * Fire-and-forget emit to api-server's /internal/audit/auth.
 * Never throws — logs to stderr on failure.
 */
export async function emitAuditEvent(
  apiServerUrl: string,
  edgeToken: string,
  input: AuditEmitInput
): Promise<void>;
```

**Implementación clave**:
- Timeout: 2s via `AbortController`. Edge tokenized auth (header).
- Body shape: igual a `audit_log` columns: `{ action, actor, target_kind: "ip", target_id, ip_address, user_agent, trace_id, after_state }`.
- Errores **silenciados** (try/catch interno) → `console.error({event:"audit.emit_failed", action, error: e.message})`.

### 2.4. Endpoint api-server `POST /internal/audit/auth`

```typescript
// backend/api-server/src/index.ts

const AuditAuthBody = z.object({
  action: z.enum([
    "auth.login_ok", "auth.login_fail", "auth.logout",
    "auth.lockout_triggered", "auth.rate_limited", "auth.locked_attempt",
  ]),
  actor: z.string().min(1).max(64),
  target_kind: z.literal("ip"),
  target_id: z.string().min(1).max(64),
  ip_address: z.string().min(1).max(64),  // INET parses it
  user_agent: z.string().max(512).optional(),
  trace_id: z.string().uuid().optional(),
  after_state: z.record(z.unknown()).optional(),
});

app.post("/internal/audit/auth", requireEdgeToken(ARBX_EDGE_TOKEN), async (req, res) => {
  const parsed = AuditAuthBody.safeParse(req.body);
  if (!parsed.success) { res.status(400).json({error: "invalid_request"}); return; }
  await writeAudit(
    parsed.data.action,
    parsed.data.actor,
    parsed.data.target_kind,
    parsed.data.target_id,
    null,                              // before_state always NULL for auth events
    parsed.data.after_state ?? null,
    parsed.data.ip_address,
    parsed.data.trace_id ?? null,
  );
  res.status(204).end();
});
```

`requireEdgeToken` ya existe en `shared-ts/src/middleware/index.ts` y se exporta vía `@arbx/shared`. Mismo middleware que ya valida `x-arbx-edge-token` en otras rutas internas. Sin nuevas deps, sin nuevos archivos de middleware.

## 3. Error handling y honesty

| Falla | Comportamiento |
|---|---|
| api-server unreachable / DNS fail | Edge `emitAuditEvent` swallows → `console.error({event:"audit.emit_failed",action,error})`. Loki captura via stderr. Operator no ve nada raro. **Audit gap aceptado** durante outage por doctrina honesty. |
| Timeout 2s | Igual ↑. El budget 2s mantiene `/admin/session` response time bounded. |
| edge-token inválido (401 desde api-server) | api-server: `warn({event:"audit.auth.bad_token"})`; sin row escrita. **Bug de config**, no debería ocurrir en setup correcto. CI verifica parity (test de integración). |
| Body inválido (Zod fail) | api-server retorna 400; edge logs `audit.emit_400` y skip. Sin row. |
| DB insert fail (Postgres down) | `writeAudit()` existing pattern: `warn({event:"audit.write_failed",err})`. Sin row, sin throw. |
| `audit_log` table tamper attempt (UPDATE/DELETE) | Bloqueado a nivel DB por `REVOKE UPDATE, DELETE FROM arbx_rw` (migración 011). Postgres responde error de permisos. |

**Doctrina honesty enforce**: ningún path crea filas sintetizadas o "best-guess" cuando la verdad real no se pudo escribir. Un audit gap es aceptable; un audit row inventado NO.

## 4. Testing (TDD: red → green → refactor)

### 4.1. `backend/api-server/src/audit-events.test.ts` (10 tests)

1. **killswitch.armed**: POST `/admin/killswitch` con `{enabled:true, reason:"x"}` → row con `action='killswitch.armed', before_state='{"enabled":false}', after_state='{"enabled":true,"reason":"x"}'`.
2. **killswitch.disabled**: POST con `{enabled:false}` → row análoga inversa.
3. **auth.login_ok**: POST `/internal/audit/auth` con body válido y `action="auth.login_ok"` → 204 + row escrita.
4. **auth.login_fail**: idem, `action="auth.login_fail"` → 204 + row.
5. **auth.logout**: idem.
6. **auth.lockout_triggered**: idem, con `after_state={blocked_until_ms,total_fails}`.
7. **auth.rate_limited**: idem, con `after_state={remaining:0}`.
8. **auth.locked_attempt**: idem (path independiente del lockout_triggered — éste se emite cada vez que una IP bloqueada hace request).
9. **rejects sin x-arbx-edge-token (401)**: POST sin header → 401, NO row.
10. **rejects malformed body (400)**: `{action:"unknown"}` → 400, NO row.

Cobertura: las 8 actions tienen test dedicado + 2 tests de input validation = 10.

### 4.2. `edge/dev-local/src/audit-emit.test.ts` (3 tests)

1. **emit on success**: mock `fetch` retorna 204 → `emitAuditEvent` resuelve sin error, fetch llamado con `(API_URL + "/internal/audit/auth", {method:"POST", headers:{"x-arbx-edge-token":TOKEN}, body:JSON.stringify(input)})`.
2. **emit on timeout**: mock `fetch` cuelga >2s → AbortController dispara, `console.error` llamado, `emitAuditEvent` resuelve sin throw.
3. **emit on 5xx**: mock retorna 500 → `console.error` llamado, no throw.

### 4.3. Coverage E2E (deferida a integration tests, NO en PR-2)

Full flow `login_fail` real → SQL `SELECT * FROM audit_log WHERE action='auth.login_fail' ORDER BY created_at DESC LIMIT 1` → row exists con campos correctos. Esto va en una integration test suite separada cuando exista la infra (planeada Sprint 2).

## 5. Métricas

- `arbx_audit_events_total{action}` — counter, 1 por row escrita exitosa (api-server, prom-client).
- `arbx_audit_emit_failed_total{reason}` — counter, edge-side; reasons: `timeout`, `network_error`, `5xx`, `bad_token`, `invalid_body`.

**Decisión de scope**: como api-server y edge son TypeScript (no Rust), las métricas viven en `shared-ts/src/metrics/` (TS prom-client registry), NO en `shared-rs/src/metrics.rs` (Rust registry, usado por searcher-rs/sim-ctl/recon/relays-client). Verificar si `shared-ts/src/metrics/` existe; si no, las métricas se **difieren a PR-2.b** (no son bloqueantes para el cierre del audit gap funcional). El audit_log es la SOURCE OF TRUTH; las métricas son indicador agregado complementario.

## 6. Plan de rollout y rollback

**Rollout**:
1. Merge PR-2 a main.
2. Deploy api-server + edge (dev-local) en VPS via `docker compose up -d --force-recreate`. CF Worker via `wrangler deploy` (separado, no en este sprint pero código listo).
3. Smoke test manual: trigger 1 login_fail desde frontend, ver row en `audit_log` via psql.
4. Monitorear Loki por 24h: `audit.emit_failed` debe ser 0 o muy esporádico.

**Rollback** (si emit_failed >5%):
1. Revert el commit en main, redeploy.
2. Las rows ya escritas quedan (tamper-evident). No se pierden.
3. La brecha de auth events vuelve a estar abierta — aceptable temporal.

## 7. Notas de seguridad

- `x-arbx-edge-token` (vía `requireEdgeToken`) es la única autenticación del endpoint `/internal/audit/auth`. Si este token se compromete, un atacante podría inyectar audit rows arbitrarias. Mitigación: el token solo vive en el contexto del edge container (env var) y rota con cada deployment. Riesgo aceptado para Sprint 1.
- El handler `/internal/audit/auth` NO está expuesto al público — accesible solo desde la red de docker (`arbx-net`) o CF Worker bindings. El path `/internal/*` es convención: la edge **NO** lo proxy-pasa hacia afuera.
- `writeAudit` usa `arbx_rw` role que tiene `INSERT` permitido pero `UPDATE`/`DELETE` revoked (migración 011). Tamper-evident a nivel DB.

## 8. Trabajo futuro (PR-2.b)

- Frontend viewer `/audit-logs` con filtros, paginación, expand row para before/after.
- Retention policy: partition `audit_log` por mes; drop partitions >90 días en prod, >30 días en dev.
- Métricas Prometheus: `arbx_audit_events_total`, `arbx_audit_emit_failed_total`.
- Integration test suite con fixtures de DB.
- Cuando llegue Auth0 (Sprint 4): `actor` se rellena con email del operador, no token fingerprint.

---

**Aceptación final**: cuando los 13 tests (10 + 3) estén verdes, los 8 actions estén mapeados en código, el commit pase CI, y un smoke manual en VPS muestre 1 row de cada action en `audit_log` después de gestos UI correspondientes.
