# 🜂 DIRECTIVA OMEGA — HARDENING ANTI-REGRESIÓN

> **ArbitrageX v2 · BLINDAJE DE LO CONQUISTADO · v1**
>
> **Estado:** DOCTRINA DE PROYECTO ADOPTADA (2026-08-13). Aplica a toda sesión de
> cambios. Su único objetivo: que nada verificado en verde vuelva a rojo, y que
> ningún cambio entre al sistema sin demostrar que debe existir.
>
> **Baseline de verdad:** `Auditoria_Frontend_ArbitrageX_v2_REAUDITORIA_7.md`
> (R7, 2026-08-13) — 6/9 guardianes verdes, 0 rojos, 56/56 páginas 200.
>
> **Filosofía:** el mejor cambio es el que no se hace. Todo lo demás pasa por el
> embudo.

---

## PRINCIPIO SUPREMO

**P-∅ — LA CARGA DE LA PRUEBA ES DEL CAMBIO, NO DEL SISTEMA.**

El sistema en verde no tiene que justificar por qué se queda; el cambio tiene
que justificar por qué entra. Un PR sin anomalía que curar (ID del tracker), sin
medida de mejora, o sin riesgo declarado, se rechaza por incompleto — no importa
lo bonito que sea el código.

## PARTE 1 — EL EMBUDO DE NECESIDAD (antes de escribir una línea)

Todo cambio propuesto responde, en el body del PR, estas 5 preguntas. Una
respuesta floja = PR devuelto.

1. **¿Qué anomalía cura?** ID del tracker (R6-04, PIPELINE-0, C-03…) o enlace a
   evidencia L4 del defecto (curl/log/screenshot con timestamp). "Mejora",
   "refactor" o "cleanup" sin anomalía medida → no entra.
2. **¿Qué pasa si NO se hace?** Si la respuesta es "nada observable" → no entra.
   Si es "degrada en el futuro" → backlog con fecha de revisión, no PR.
3. **¿Qué toca?** Lista de archivos prevista. Si toca algo de la LISTA DE
   CONGELACIÓN (Parte 2) → requiere justificación doble y revisión humana.
4. **¿Cómo se demuestra que no degrada?** Qué guardián(es) del baseline cubren
   el área tocada, y qué chequeo antes/después se ejecutará.
5. **¿Cómo se revierte?** Todo PR declara su revert en una línea
   (`git revert <sha>` limpio = requisito; cambios con migraciones irreversibles
   o estado no revertible → diseño aparte, no PR normal).

**Prohibiciones absolutas de scope:**
- ❌ **"De paso"** — un PR cura UN ID. Mezclar dos anomalías = dos PRs.
- ❌ **Reformateo/reordenado** de archivos que no se tocan por la anomalía
  (ruido de diff = revisión ciega = regresiones invisibles).
- ❌ **Upgrades de dependencias** mezclados con fixes. Dependabot va en su carril.
- ❌ **Cambios de config de prod** sin PR de config + evidencia del valor anterior.

## PARTE 2 — LISTA DE CONGELACIÓN

**Nivel 1 — INTOCABLE (REGLA 01 histórica):**
`pmiCalculator.ts` · radar de rutas (`route-discovery/dfs_bounded`) · kill-switch
(KILLED/paper_only) · store append-only de auditoría · estados vacíos honestos.

**Nivel 2 — CONGELADO POR CONQUISTA** (verificado verde en R7 — tocar = regresión
potencial, exige anomalía + L4 antes/después):
- Contrato defi `{success, data}` en `/api/pools`, `/api/chains` (#329) y sus
  contract tests (`schemas.defi-contract.test.ts`).
- Las 46 rutas de paridad del worker (#327) — añadir es libre, modificar/quitar
  es Nivel 2.
- CORS: allowlist apex + same-origin `getApiBaseUrl()` (#326/#328/#331).
- Reshape `{count, items}` → `{success, data}` — vive en un solo lugar; no se
  duplica ni se "optimiza".
- Caché de readiness 20s + `PG_POOL_MAX=35` (#330, cura de C-06).
- Gate admin (401/307 en `/admin/*`, #310).
- Outlier guard del paper ledger (#318).
- `LocalTime` y las superficies ya migradas (#315).
- Contadores V2 del heartbeat (#319).

**Nivel 3 — LIBRE** con el embudo de la Parte 1.

## PARTE 3 — GATES AUTOMÁTICOS (CI + deploy)

- **G1 · Contract tests obligatorios (required check):**
  `schemas.defi-contract.test.ts` + todo test de contrato futuro corren en CI de
  todo PR. Un drift de shape = CI rojo = no merge (branch protection activa — P-02;
  verificar que estos tests están en la lista de required checks, no solo "corren").
- **G2 · Gate de paridad permanente (lección R5.2):** script en CI que cruza (a)
  las rutas que el frontend consume (grep de `api-client.ts` + `fetch(` en `app/`)
  contra (b) las rutas que el edge sirve. Diferencia ≠ 0 → CI rojo.
- **G3 · Guardian smoke en CI/post-deploy:** job que golpea los 9 guardianes
  contra staging (o prod tras deploy) y falla si alguno retrocede vs el baseline R7.
- **G4 · Deploy veraz (REGLA 0h):** todo deploy termina con
  `git rev-parse HEAD` en el VPS == SHA despachado, o el job falla. Sin excepción.
- **G5 · L4 post-deploy obligatorio:** tras cada auto-deploy, smoke de 60s (los 9
  guardianes + `/` 200). Si falla: rollback automático al SHA anterior y fila de
  incidente. El disparador se declara ANTES del deploy.
- **G6 · Secuencia blindada (REGLA 0f mecanizada):** PRs de secuencia llevan
  "NO mergear antes que #N"; el auto-deploy del servicio afectado se habilita
  solo cuando el último PR de la secuencia mergea.

### Los 9 guardianes (baseline R7)

| # | Guardián | Chequeo mínimo |
|---|----------|----------------|
| 1 | Feed oportunidades | `/api/opportunities/live` → shape `{count, items}` válido |
| 2 | Heartbeat | `/api/scanner/heartbeat` → snapshot con `decoded_ok`/`decoded_err` presentes |
| 3 | Contrato defi | `/api/pools` + `/api/chains` → `{success:true, data:[...]}` |
| 4 | Paper ledger | `/api/paper/history?limit=1` → `{ok:true, source:"postgres"}` |
| 5 | Hidratación | `/risk` sin `#425`/`#418`/`#422` (Playwright headless) |
| 6 | Kill-switch | postura `paper_only` / gated |
| 7 | CORS | `Origin: https://arbx.ape-tv.net` → ACAO correcto; origen extraño → sin ACAO |
| 8 | Gate admin | `/admin/audit` anónimo → 401 |
| 9 | Home gates | `/` SSR con `GateSection` server-driven, 0 errores consola |

## PARTE 4 — DISCIPLINA DE EVIDENCIA

- Todo CLOSED exige merge + deploy + L4 con timestamp (REGLA 0 — no se negocia).
- La evidencia se pega cruda: curl completo, consola Playwright, log con timestamp.
- Antes de declarar frescura de datos: comprobar el header `Date` del servidor (UTC).
- Parsear antes de afirmar: un `{"error":"not_found"}` no es "0 filas".
- Tracker reconciliado <24h tras cada merge (REGLA 0b). Desviaciones documentadas
  <24h (REGLA 0g).
- El barrido de las 56 páginas + 9 guardianes se repite entero tras cualquier
  deploy que toque edge o frontend — no solo el área del PR.

## PARTE 5 — PROTOCOLO DE EMERGENCIA

1. **Detectar:** G3/G5 en rojo, o guardián amarillo/rojo en verificación manual.
2. **Restaurar primero, entender después:** `git revert` + redeploy del SHA verde
   anterior. La disponibilidad de lo conquistado manda sobre la curiosidad del
   diagnóstico. (Única excepción: pérdida de datos — ahí se para todo y se piensa.)
3. **Fila de incidente en el tracker el mismo día:** qué se rompió, qué PR lo
   introdujo (bisect si hace falta), por qué los gates no lo pillaron, qué gate
   nuevo lo pilla la próxima vez.
4. **El gate nuevo es parte del fix:** ningún incidente se cierra solo con el
   revert — se cierra con revert + gate que lo habría detectado.
5. **Post-mortem sin culpa y con números:** tiempo de detección, tiempo de
   restauración, blast radius. El objetivo del hardening es bajar esos tres.

## CHECKLIST DE CADA PR

```text
[ ] Cura el ID: ____ (una sola anomalía)
[ ] Si no se hace, pasa: ____
[ ] Archivos tocados: ____ (ninguno de Nivel 1; Nivel 2 justificado)
[ ] Guardianes afectados: ____ — chequeo antes/después: ____
[ ] Revert: git revert limpio ✔
[ ] Contract tests verdes · Gate de paridad verde · CI verde
[ ] Sin "de paso", sin reformateo ajeno, sin deps mezcladas
[ ] L4 post-deploy planeado (guardianes + páginas del área)
[ ] Tracker preparado (fila en <24h)
```

## CRITERIO DE ÉXITO DEL HARDENING

- Cero guardianes verdes perdidos en 30 días.
- Todo incidente restaurado en <15 min con revert limpio.
- 100% de PRs con ID de anomalía; 0 PRs "de paso" mergeados.
- Todo incidente cierra con gate nuevo (G1–G6 crecen, nunca encogen).

---

## APÉNDICE A — Estado de implementación (auditoría 2026-08-13)

Auditoría honesta de qué gates existen vs la doctrina. **Cada hueco = su propio
PR con ID de anomalía (no un mega-PR — violaría P-∅).**

| Gate | Estado | Evidencia / hueco |
|------|--------|-------------------|
| P-∅ embudo | **Parcial** — doctrina adoptada aquí; PR template upgraded (este cambio). Falta: CI que rechace PR sin ID. |
| P-02 branch protection | **Verde** — 14 required checks activos en `main`. |
| G1 contract tests required | **Parcial** — `opportunities-fidelity-gate.yml` + `schemas.defi-contract.test.ts` existen; **hueco**: verificar que `schemas.defi-contract.test.ts` está en los 14 required checks (hoy NO aparece por nombre — puede correr sin ser required). |
| G2 paridad frontend↔edge | **Hueco** — `spec-drift-gate.yml` cubre drift de specs OpenAPI/AsyncAPI, NO el cruce `api-client.ts`+`fetch(` ↔ rutas edge servidas. Anomalía a abrir: **PIPELINE-PARITY**. |
| G3 guardian smoke | **Parcial** — `e2e.yml`/`integration-tests.yml` existen; **hueco**: ningún job golpea los 9 guardianes post-deploy contra prod. Anomalía: **GUARDIAN-SMOKE-9**. |
| G4 deploy veraz | **Hueco** — `auto-deploy-vps.yml` marca success sin anclar `git rev-parse HEAD` al SHA despachado (lección `arbx-auto-deploy-silent-failure`). Anomalía: **DEPLOY-VERAZ-G4**. |
| G5 L4 post-deploy | **Hueco** — no hay smoke de 60s post-auto-deploy ni rollback automático. Anomalía: **POSTDEPLOY-L4-G5**. |
| G6 secuencia | **Hueco** — no hay check que lea "NO mergear antes que #N" en bodies. Anomalía: **SEQ-GATE-G6**. |
| no-hardcode (Nivel 1/2) | **Verde** — `no-hardcode.yml` + `lint-no-hardcode.sh` + `opportunities-fidelity-gate.yml` cubren la ruta opportunities. |

**Orden de ejecución propuesto (cada uno su PR, anomalía propia):**
1. `DEPLOY-VERAZ-G4` (cierra el silencio del auto-deploy — mayor impacto, bajo riesgo).
2. `GUARDIAN-SMOKE-9` (G3 — visibility de regresión en prod).
3. `PIPELINE-PARITY` (G2 — previene la reincidencia exacta de R5.2).
4. `POSTDEPLOY-L4-G5` (monta sobre G3 + G4).
5. `SEQ-GATE-G6` (G6).
6. `G1-required-verification` (auditar/forzar que los contract tests sean required).

**v1** — nace del arco R5.2→R7: un swap sin gate tumbó el sitio; los gates
correctos (paridad, guardianes, deploy veraz, embudo de necesidad) son la vacuna.
El sistema conquistado se defiende con máquinas, no con buenas intenciones.
