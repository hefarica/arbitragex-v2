# OMEGA LOOP — Tracker de Corrección Total (ArbitrageX v2)

Fuente de verdad: `Auditoria_Frontend_ArbitrageX_v2.md` (19 anomalías: A-01…A-03, B-01…B-04, C-01…C-07, D-01…D-05).
Regla: solo el agente mueve un ID a CLOSED, y solo con evidencia pegada (curl / log / screenshot).

> **Reconciliación cross-rama (2026-08-10):** el tracker en `main` (traído por el merge B-01, `5704c5f4`) marca A-01/FASE0/A-03 como CLOSED — **inexacto**. `git branch --contains` confirma que `634e414c` (A-01) y `f0319a58` (FASE0+A-03) **NO** están en main; solo B-01 (`5704c5f4`) llegó. Producción sigue filtrando A-01 (curl 2026-08-10: 482 KB + PII). Esta rama `fix/omega-A01-combined` es A-01 combinado (previo edge-layer + api-server-layer). FASE0+A-03 se recuperan de `fix/omega-loop-fase0-a03` tras este PR.

| ID | Estado | Commit / Rama | Evidencia de cierre | Fecha |
|---|---|---|---|---|
| A-01 | VERIFY (L1/L2 ✓; L3 prod pending deploy) | `fix/omega-A01-combined` (previo `634e414c` + api-server layer) | vitest **14/14**; tsc **0** en api-server+frontend+edge; contrato Zod preservado. Prod curl 482 KB+PII (antes). L3 prod tras deploy. | 2026-08-10 |
| A-02 | OPEN | — | — | — |
| A-03 | OPEN (fix previo en `fix/omega-loop-fase0-a03`) | — | — | — |
| FASE0 | OPEN (fix previo en `fix/omega-loop-fase0-a03`) | — | — | — |
| B-01 | CLOSED (en `main` `5704c5f4`) | `5704c5f4` | Web3 aislado a /wallet; STATUS/HOME WC count 0 | 2026-08-09 |
| B-02…B-04 | OPEN | — | — | — |
| C-01…C-07 | OPEN | — | — | — |
| D-01…D-05 | OPEN | — | — | — |

---

## A-01 — `/audit-logs` expone el registro administrativo sin autenticación (🔴 P0)

**Causa raíz (confirmada en código):** `frontend/app/audit-logs/page.tsx` era un Server Component SSR que llamaba `getAuditLogs()`, la cual en SSR inyectaba `process.env.ARBX_ADMIN_TOKEN` como header `x-arbx-admin-token`. El edge `/admin/audit` **sí** exige admin (401 sin token/cookie), pero el SSR se autenticaba con el token del servidor y servía las filas a cualquier visitante.

**Corrección aplicada (rama `fix/omega-A01-combined` — COMBINE de previo + nuevo, decisión operador 2026-08-10):**
1. `getAuditLogs()` ahora autentica **solo** vía cookie httpOnly (`credentials: "include"`); eliminado el fallback de token SSR (previo).
2. `/audit-logs` → Server Component fino que reenvía `searchParams` a `AuditLogsClient` (client). Sin sesión → **gate admin** (nunca filas); con sesión → filas (previo).
3. Edge `/admin/audit` (worker) → redacción PII en la **respuesta** (store append-only intacto): `ip_address` → `/48` (v6) o `/24` (v4); `actor` → SHA-256 (previo).
4. **NUEVO — api-server layer (origen de datos):** `/admin/audit` en `backend/api-server/src/index.ts` mapea `redactAuditRow` (`backend/api-server/src/lib/audit-redact.ts`). Cubre `ip_address` **Y** `target_id` (52 IPv6 crudos que el edge NO redactaba) + `actor` email→`sha256:12hex`. Defense-in-depth: origen + proxy. **14 unit tests** (`audit-redact.test.ts`).

**Verificación (2026-08-10):**
- L1: vitest **14/14** (`audit-redact.test.ts`); `tsc --noEmit` **0 errores** en api-server, frontend, edge-worker.
- L2: contrato Zod preservado por construcción (campos redactados mantienen tipos compatibles: `actor:string`, `ip_address/target_id:string|null`).
- L3 (prod, ANTES): `curl /audit-logs` anónimo → 200, **482 KB**, email `b***@gmail.com`, **76 `auth.login_ok`**, 46 `killswitch.disabled`, **52 IPv6 crudos en `target_id`**. **L3 (prod, DESPUÉS): pendiente deploy.** (El L3 "anónimo→gate" del 2026-08-09 fue contra **dev-local**, no producción.)
- L4: pendiente barrido post-deploy.

**Criterio de aceptación (§4):**
- [x] Anónimo → sin filas/IPs/emails (dev-local verificado; estructuralmente garantizado: sin cookie → edge `401 missing_admin_token`).
- [ ] Con sesión admin → filas con IP `/48` y actor hasheado — **requiere deploy** del worker + api-server.

**Residuales honestos:**
- `hashActor` (api-server y edge) es SHA-256 **sin salt** (pseudónimo consistente). Para redacción irreversible de emails de baja entropía, usar HMAC con clave en la fuente de escritura (follow-up).
- IPs embebidos en `before_state`/`after_state` JSON no se escrubeán (solo columnas `ip_address` + `target_id`).

**NO se tocó:** store append-only, write path (`audit-emit.ts`, `arbx_anonymize_ip` write-side), kill-switch, radar de rutas, `pmiCalculator.ts`, doctrina de estados honestos.
