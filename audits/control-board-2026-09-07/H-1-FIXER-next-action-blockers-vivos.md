# H-1 — FIXER: next_action de readiness/decision deriva de los blockers VIVOS (Gang Omniscience, ronda 1)

- **WO**: H-1 (gap #1 de `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md` §3/§5) · kind: fix + verify
- **Agente**: FIXER ronda 1 · **Fecha**: 2026-09-08 (sesión local) · **Estado**: FIX aplicado y verificado LOCAL (NO-GIT: 0 commit/push/PR/deploy — protocolo operador 2026-08-23)
- **Archivos tocados**: `backend/api-server/src/routes/readiness-extras.ts` + `backend/api-server/src/routes/readiness-extras.test.ts` — ambos git-clean antes de mi edición (verificado `git status --porcelain`): **sin conflicto de claims con ningún WO par** (única otra referencia a readiness-extras en la mesa: el propio reporte del auditor).

## 0. Sincronía de mesa redonda (leída ANTES de editar)

Leídos: `GOAL-WORKORDERS.md`, `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md` (fuente del WO),
`CB-01-CROSS-EXAM.md` (completo). No contradigo ningún hallazgo previo; construyo directamente sobre:

- **BROWSE-Auditor-R8 §3 H-1**: next-action "All known blockers cleared at this layer" es falso en la cara
  del propio panel (G-PIPE-1 critical + G-DISK-1 high + A.9 pending vivos a 2 cm). Root cause citado
  en `readiness-extras.ts:586-595` (pre-fix): `nextActionParts` se poblaba SOLO con el blocker A.4, y
  A.4 está resuelto desde 2026-08-20 → el string caía SIEMPRE al default. Confirmé la lectura de código
  línea por línea antes de editar.
- **BROWSE-Auditor-R8 §1**: el resto del endpoint es honesto (veredicto NO_GO, "3 PENDING", `go_live`
  estructuralmente false en triple capa) — mi fix NO toca nada de eso. Sólo la prosa del next-action,
  como pide el WO.

## 1. El fix (surgical, fail-honest)

**Nueva función pura `composeNextAction(blockers)`** en `readiness-extras.ts:524-569`
(`function composeNextAction` en :553), usada por el handler de `/api/v1/readiness/decision` en :638
(reemplaza el bloque inline ex-:586-595). Composición:

1. Si A.4 volviera a estar presente → su runbook específico primero (comportamiento pre-existente
   preservado).
2. Los **blockers vivos de mayor severidad** (critical > high > medium > low; sort estable — dentro de
   una misma severidad conserva el orden de inserción env → readiness → doctrinal), **máximo 3**, cada
   uno como `` `${id} [${severity}] ${required_action}` `` — contenido 100% derivado de `blockers`,
   cero prosa inventada (RULE 00/R8).
3. A.9 (`a9_go_no_go_formal_pending`) va SIEMPRE al final como paso terminal ("Await A.9 formal
   GO/NO-GO sign-off.") — es el gate doctrinal, no un item accionable del mismo tipo.
4. El default "All known blockers cleared at this layer…" **sobrevive SOLO para la lista realmente
   vacía**, único estado donde esa afirmación es verdadera. (Nota: con el censo doctrinal actual A.9
   está siempre presente, así que el default hoy es inalcanzable — correcto: hoy SIEMPRE hay al menos
   un blocker vivo y la prosa ahora lo dice.)

Sin cambios de contrato wire: `next_action` sigue siendo `z.string()` libre
(`frontend/lib/schemas.ts:842`) renderizado tal cual en `GoNoGoPanel.tsx:177,245` — cero cambios FE
necesarios, cero bump coordinado.

**Preview con los shapes VIVOS que el auditor observó** (via las funciones reales del módulo:
`readinessItemsToBlockers` con G-PIPE-1 red + G-DISK-1 yellow + `doctrinalBlockers()`):

```
OLD: All known blockers cleared at this layer; await A.9 formal GO/NO-GO sign-off.
NEW: readiness_g_pipe_1 [critical] Resolve readiness item G-PIPE-1 (Paper pipeline stream flow (detected→validated→simulated)). Then: readiness_g_disk_1 [high] Resolve readiness item G-DISK-1 (Host disk usage below critical threshold). Then: Await A.9 formal GO/NO-GO sign-off.
```

Crítico antes que high, A.9 terminal — exactamente el "DEBERÍA mostrar" del auditor.

## 2. Verificación (evidencia reproducible)

| Check | Resultado |
|---|---|
| `npx vitest run src/routes/readiness-extras.test.ts` (api-server) | **29/29 PASS** (22 pre-existentes + 7 nuevos, 0 modifies de tests previos) |
| `npm run typecheck` (`tsc --noEmit -p tsconfig.json`) | **CLEAN** |
| Suite completa api-server (`npx vitest run`) | **790/793 PASS**; 3 fails = `websocket-carnot.test.ts` + `websocket.test.ts` (runtime_ack ×2), todos "timed out in 5000ms" bajo carga de suite completa (510s) — **re-ejecutados en aislamiento: 3/3 PASS**. Sin relación de import con mi cambio (consumidores de readiness-extras: `index.ts`, `agents-status.ts` — strings estáticos de evidence, verificado), test files de WS no lo importan. Clasificación: flakes de timing pre-existentes del entorno local, no regresión mía. |
| Preview live-shape (§1) | String honesto, severidad-ordenado |

Tests nuevos (`readiness-extras.test.ts:220-317`), patrón existente `__forTesting`:

1. **regression H-1** — G-PIPE-1 critical + G-DISK-1 high + A.9 pending → NO contiene "All known
   blockers cleared", G-PIPE-1 antes que G-DISK-1, termina con el sign-off A.9, cada item lleva
   `[severity]` + required_action.
2. **guards id drift** — consume el mapping exacto de `readinessItemsToBlockers` (`G-PIPE-1` →
   `readiness_g_pipe_1`): si alguien cambia la transformación de ids, este test falla.
3. A.4 presente → runbook RPC_HTTP_1+EXECUTOR_1 primero (comportamiento pre-existente intacto).
4. Estado A.9-only → "Await A.9 formal GO/NO-GO sign-off." (nada de all-clear falso).
5. Cap de 3 derivados — severidades bajas caen primero.
6. Lista vacía → default honesto (all-clear afirmado SOLO cuando nada bloquea).
7. No muta el array de entrada (sort sobre copia del filter).

## 3. Conflicto de claims: NINGUNO

`readiness-extras.ts` / `.test.ts` estaban clean en git; ningún reporte par los reclama (grep en
`audits/control-board-2026-09-07/`: sólo el reporte BROWSE-Auditor-R8 los cita). Los archivos `M` del
árbol (`index.ts`, `control-board.ts`, etc.) pertenecen a WOs CB-02/03/04 — **no los toqué**; mi diff
es exactamente 2 archivos.

## 4. Hand-off a la mesa y al operador

1. **El fix NO es visible en el dominio público hasta deploy** (NO-GIT): `/live-readiness` seguirá
   mostrando la prosa vieja hasta que este diff entre por PR + pipeline aprobado. Cuando eso pase, el
   par browser (CB-06/BROWSE) puede re-verificar con el mismo journey §2 fila 3 del auditor.
2. **H-2/H-3/H-4** (gaps #2-#4 del auditor, `frontend/app/page.tsx`) siguen abiertos — no eran mi WO y
   los dejo intactos para el fixer que los tome (H-2 ya tiene el patrón correcto 3 líneas abajo con
   `confidence`).
3. **Gap #5 (disco 94.5%, 8.2 GB libres) es operator-gated** y el fix lo hace MÁS visible, no menos:
   con el fix, `next_action` nombrará `readiness_g_disk_1 [high]` de primera mano mientras siga yellow.
   Eco de CB-01-CROSS-EXAM: la retención ARBX-RETENTION-01 (48% el 09-04) se re-llenó en 4 días.
4. Diffs marcados `// WO H-1 (2026-09-07)` en los 3 puntos de código (header doc, `__forTesting`,
   call-site).

**Presupuesto dominio público: 0 requests HTTP** (todo verificado local: vitest + tsc + preview con
funciones reales). 0 ssh. 0 git. 0 escrituras VPS. §32/§33/§34.3 intactos.
