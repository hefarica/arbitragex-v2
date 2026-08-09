# OMEGA LOOP — Tracker de Corrección Total (ArbitrageX v2)

Fuente de verdad: `Auditoria_Frontend_ArbitrageX_v2.md` (19 anomalías: A-01…A-03, B-01…B-04, C-01…C-07, D-01…D-05).
Regla: solo el agente mueve un ID a CLOSED, y solo con evidencia pegada (curl / log / screenshot).

| ID | Estado | Commit | Evidencia de cierre | Fecha |
|---|---|---|---|---|
| A-01 | WIP | _pendiente commit (rama `fix/omega-loop-a01`)_ | tsc frontend limpio (solo ssh2 ambient pre-existente) + eslint limpio + edge worker `tsc --noEmit` **0 errores**; gate anónimo verificado in-vivo (ver abajo). Deploy del edge pendiente confirmación operador. | 2026-08-09 |
| A-02 | OPEN | — | — | — |
| A-03 | OPEN | — | — | — |
| FASE0 | OPEN | — | — | — |
| B-01 | OPEN | — | — | — |
| B-02 | OPEN | — | — | — |
| B-03 | OPEN | — | — | — |
| B-04 | OPEN | — | — | — |
| C-01…C-07 | OPEN | — | — | — |
| D-01…D-05 | OPEN | — | — | — |

---

## A-01 — `/audit-logs` expone el registro administrativo sin autenticación (🔴 P0)

**Causa raíz (confirmada en código):** `frontend/app/audit-logs/page.tsx` era un Server Component SSR que llamaba `getAuditLogs()`, la cual en SSR inyectaba `process.env.ARBX_ADMIN_TOKEN` como header `x-arbx-admin-token` (`frontend/lib/api-client.ts:546-559` versión previa). El edge `/admin/audit` **sí** exige admin (401 sin token/cookie — `edge/worker/src/index.ts:764-766`), pero el SSR se autenticaba con el token del servidor y servía las filas a cualquier visitante.

**Corrección aplicada (rama `fix/omega-loop-a01`):**
1. `getAuditLogs()` ahora autentica **solo** vía cookie httpOnly (`credentials: "include"` que ya envía `getValidated`); se eliminó el fallback de token SSR.
2. `/audit-logs` → Server Component fino que reenvía `searchParams` a `AuditLogsClient` (client). Sin sesión → **gate admin** (nunca filas); con sesión → filas.
3. Edge `/admin/audit` (worker) → redacción PII en la **respuesta** (el store append-only queda intacto): `ip_address` → `/48` (v6) o `/24` (v4); `actor` → SHA-256 (12 hex). `dev-local` dejado intacto (usa `adminProxy` compartido; no es producción).

**Verificación:**
- L1 (unidad/compilación): frontend `tsc --noEmit` — 0 errores en archivos tocados (solo el ambient `ssh2` pre-existente del `node_modules` roto); `eslint` limpio. Edge worker `tsc --noEmit -p tsconfig.json` — **0 errores**.
- L3 (en vivo, anónimo, verificado 2026-08-09): `python playwright` contra dev local (edge real) → `STATE=[data-testid="audit-logs-gate"]`, `GATE_VISIBLE=True`, `EMAILS_FOUND=[]`, `IPV6_FOUND=[]`. Sin sesión → gate, cero filas/IPs/emails.
- L4 (regresión): pendiente barrido de 10 páginas.

**Criterio de aceptación (§4):**
- [x] Anónimo → sin filas/IPs/emails (gate).
- [ ] Con sesión admin → filas con IP `/48` y actor hasheado — **requiere deploy del worker** (la redacción corre en el edge; el worker desplegado aún es el viejo). Pendiente confirmación de deploy.

**Riesgo residual:** `hashActor` es SHA-256 **sin salt** (pseudónimo de renderizado consistente). Para redacción irreversible de identificadores de baja entropía (emails), usar HMAC con clave en la fuente de escritura (follow-up).

**NO se tocó:** el store append-only, el kill-switch, el radar de rutas, `pmiCalculator.ts`, la doctrina de estados honestos.
