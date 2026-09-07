# WO-09 — APPLY: endurecimiento CSP en 2 fases (P0-6)

- **Work-order:** WO-09 (GOAL-WORKORDERS.md:17 · `00-PREDATOR-ROADMAP.md:104-108`)
- **Estado:** **APPLIED_VERIFIED (gated-OFF)** — mecánica de fase 1 landeada localmente; verificación local verde; **flip real = OPERADOR** (§3). Diseño completo: `WO-09-DESIGN.md` (mismo directorio).
- **Hora (UTC):** 2026-09-06 ~20:55 local (probe/verify) · verificación dominio 2026-09-07T01:48Z · re-verificación respawn §6 (~21:31-21:37 local)
- **Reglas respetadas:** NO-GIT (0 commit/push/PR/deploy — `git diff --stat` confirma solo working tree: 11 insertions, 0 deletions) · RULE 00 (todo dato citado es medido) · presupuesto dominio: **1 de 2 requests usados**.

---

## 1. Qué se aplicó (y qué NO)

### Aplicado — `frontend/next.config.js` (+11, −0, marcado `// WO-09 (2026-09-06)`)

Duplicación de la política `csp()` como cabecera **`content-security-policy` ENFORCING**, gated tras `ARBX_CSP_ENFORCE === "true"` (exact-string, fail-closed), junto a la report-only incondicional (next.config.js:149-160). Propiedades verificadas:

- Ambas cabeceras llaman a la MISMA `csp()` → idénticas byte a byte por construcción.
- Default OFF → el build de prod queda **idéntico al actual** hasta que el operador cablee el build-arg (§3) — no se aplicó enforcing a ciegas (charter §c/§4 del design).
- Precedente de patrón: gate HSTS SEC-3 (`ARBX_TLS_ENABLED`, next.config.js:162) — `headers()` se evalúa en `next build` (frontend/Dockerfile:12-17), por lo que el flip es rebuild RULE 03, no `environment:` de compose.

### NO aplicado (fuera de claim o operador) — diffs exactos en `WO-09-DESIGN.md` §2.3/§3

- Wiring del flip: `frontend/Dockerfile` (`ARG/ENV ARBX_CSP_ENFORCE=false`) + `docker/compose.prod.yml` (`build.args`) — PR del operador (archivos no claimados).
- Fase 2 (nonce/middleware): **design-only** — `WO-09-DESIGN.md` §3 incluye el `middleware.ts` propuesto completo, el diff de `next.config.js` fase 2a/2b, y el audit real de "qué rompe" (0 scripts inline propios; 41 `style={{…}}` → style-src conserva unsafe-inline; 1 página force-static `app/deploy-pipeline/page.tsx:56`; unsafe-eval dev-only per docs Next).

## 2. Verificación (comandos exactos y salida)

```
$ cd frontend && node -e '<probe headers()>'            # GATE OFF (default)
{"keys":["x-frame-options","x-content-type-options","referrer-policy",
 "permissions-policy","content-security-policy-report-only"],
 "enforcing_present":false,"total":5,"report_only_ok":true}

$ ARBX_CSP_ENFORCE=true node -e '<probe>'               # GATE ON
{"keys":[...5 previas...,"content-security-policy"],
 "enforcing_present":true,"byte_identical":true,"total":6}

$ ARBX_CSP_ENFORCE=True node -e '<probe>'               # fail-closed (valor no exacto)
{"enforcing_present":false}

$ cd frontend && npx vitest run next.config.guard.test.ts
  ✓ next.config.guard.test.ts (6 tests) 8ms
  Test Files 1 passed · Tests 6 passed (6)

$ cd frontend && npx tsc --noEmit
  TSC_EXIT=0
```

Consumidores del archivo auditados: el único require funcional de `next.config.js` es el guard test (`next.config.guard.test.ts:17`); `lib/api-client.ts:37,48` lo menciona solo en comentarios. Por eso la no-regresión se ejecutó con el guard suite + tsc completo, y NO con el vitest full (las demás suites testean componentes que OTROS WO están editando simultáneamente en el mismo working tree — signal ajena, no atribuible a este cambio quirúrgico de config).

## 3. El flip es del operador (checklist, no automático)

1. PR wiring §2.3 del design (Dockerfile + compose) — mergeado con CI.
2. Ventana de deploy quieta (**dependencia P0-5**) + sin auditoría en curso.
3. `ARBX_CSP_ENFORCE=true` en `.env` VPS → rebuild RULE 03 (`--env-file .env`, `build --no-cache frontend`, `up -d`).
4. Verificar `curl -sI https://arbx.ape-tv.net/` = AMBAS cabeceras byte-idénticas.
5. Smoke e2e post-deploy con listener `securitypolicyviolation` (`E2E_BASE_URL` público).
6. Ventana 48-72 h: canales de monitoreo + umbrales de abort + rollback exacto — `WO-09-DESIGN.md` §2.4-§2.6.

## 4. Evidencia live (1 request, presupuesto 1/2)

`curl -sI https://arbx.ape-tv.net/` (2026-09-07T01:48Z): única CSP = **report-only** con `unsafe-inline`/`unsafe-eval` (D-1 vigente en dominio); HSTS presente (gate SEC-3 probado en prod — precedente del mecanismo usado aquí); **NEL activo en la zona** (`Report-To: cf-nel` + `Nel` inyectados por Cloudflare) — el feed se observa en dashboard Cloudflare zona → **Network Error Logging → View Reports** (crudo vía Logpush `nel_reports`, Enterprise). Corrección honesta documentada: **NEL colecta errores de red, NO violaciones CSP** (esa stream exige `report-to`/`report-uri` ausente hoy + `csp_reports` es Enterprise) — los canales de violación reales de la ventana son report-only-retenido en consola + smoke e2e + browser-verifiers (`WO-09-DESIGN.md` §2.4).

## 5. Límites y follow-ups (fail-honest R8)

- **La protección XSS por CSP sigue = 0 en el dominio público** hasta el flip del operador — este WO landea la mecánica, no la activa (aplicarla a ciegas rompe prod: charter).
- Fase 2 queda diseñada íntegramente pero sin aplicar (archivos no claimados: `middleware.ts` nuevo, guard test). Bloqueo duro conocido: `app/deploy-pipeline/page.tsx:56` force-static (nonce exige dynamic rendering).
- `frontend/nginx.conf:23-26` emite su propia CSP enforcing estática FUERA de la ruta pública — si nginx entra a la ruta, dos CSP enforcing se intersectan y rompe logos/conexiones. Condición previa del flip: ruta pública sigue CF tunnel→5173.
- Estado sugerido para el board: `APPLIED_VERIFIED (gated-OFF; flip=operador §WO-09-DESIGN §2.6)` — el board GOAL-WORKORDERS.md no es claim de este WO y no fue tocado.

## 6. Re-verificación independiente (respawn-apply, 2026-09-06 ~21:31-21:37 local)

El orquestador re-despachó WO-09 (kind: apply) tras la compleción original. **Nada se re-editó**: el diff de `frontend/next.config.js` fue re-inspeccionado contra `WO-09-DESIGN.md` §2.2 (idéntico byte a byte, +11/−0; sigue siendo el único archivo de código tocado por este WO) y TODA la suite de verificación se re-ejecutó desde cero — las aserciones del pase original NO se tomaron por hecho:

| Verificación | Resultado |
|---|---|
| Probe `headers()` GATE OFF (env default) | 5 cabeceras; report-only presente; enforcing AUSENTE ✅ |
| Probe `ARBX_CSP_ENFORCE=true` | 6 cabeceras; enforcing presente; `byte_identical=true` ✅ |
| Probe fail-closed `ARBX_CSP_ENFORCE=True` | enforcing AUSENTE ✅ |
| Probe fail-closed `ARBX_CSP_ENFORCE=1` (modo extra, no en el pase original) | enforcing AUSENTE ✅ |
| `npx vitest run next.config.guard.test.ts` | 6/6 passed · exit 0 ✅ |
| `npx tsc --noEmit` | exit 0 ✅ |
| Presupuesto dominio | **0 requests nuevos este pase** (acumulado WO: 1/2). El cambio es local gated-OFF y no es observable en el dominio; NO-GIT (0 commits/pushes) garantiza que nada salió del working tree — la evidencia live §4 (01:48Z, misma fecha) sigue vigente |

Estado confirmado: **APPLIED_VERIFIED (gated-OFF)** — sin cambios necesarios. El flip sigue siendo exclusivamente del operador (§3 · `WO-09-DESIGN.md` §2.6/§4).
