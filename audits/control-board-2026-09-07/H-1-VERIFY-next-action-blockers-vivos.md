# H-1 — VERIFY: re-verificación adversarial del fix (Gang Omniscience, ronda 2)

- **WO**: verify H-1 (`H-1-FIXER-next-action-blockers-vivos.md`, ronda 1) · kind: adversarial verify
- **Agente**: VERIFIER ronda 2 · **Fecha**: 2026-09-08 (sesión local) · Read-only total: 0 git-write,
  0 VPS, 0 HTTP público. Verificación local: vitest + tsc + lectura línea por línea.
- **Pregunta del operador**: ¿el gap original quedó cerrado? ¿sin regresiones? ¿gaps restantes?

## 0. Sincronía de mesa (leída ANTES de verificar)

Leídos: `GOAL-WORKORDERS.md`, `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md` (fuente del
hallazgo H-1 §3/§5), `H-1-FIXER-next-action-blockers-vivos.md` (el fix bajo verificación). Contexto
lateral: `H-2-FIX.md`, `H-3-FIX.md`, `H-4-FIX-APPLY.md` (fixers pares, en verificación por otros).

## 1. Veredicto: **PASS** — gap cerrado a nivel código, 0 regresiones encontradas

Cada claim del fixer fue re-derivada independientemente (no tomada por fe). Evidencia file:line
contra el árbol de trabajo actual:

### 1.1 El fix existe y hace lo declarado

| Claim del fixer | Verificación propia | Evidencia |
|---|---|---|
| `composeNextAction` pura, deriva SOLO de blockers | Confirmado línea por línea | `backend/api-server/src/routes/readiness-extras.ts:546-567` (banner de sección :524-539; función ~:553; call-site **:638** exacto) |
| Severidad critical>high>medium>low, sort estable, máx 3, formato `id [severity] required_action` | `SEVERITY_RANK` {critical:0,high:1,medium:2,low:3} ascendente; `Array.prototype.sort` es estable por spec ES2019+; `.slice(0,3)` TRAS el sort (caen las severidades bajas primero); template exacto | `readiness-extras.ts:540-541,558-564` |
| A.4 primero si volviera a existir | `if (a4) parts.push(A4_NEXT_ACTION)` — string idéntico byte a byte al bloque inline viejo (diff verificado) | `readiness-extras.ts:552-556` + diff `-:586-595 / +:635-638` |
| A.9 SIEMPRE terminal | `b !== a4 && b !== a9` lo excluye del top-3 y `if (a9) parts.push(A9_NEXT_ACTION)` lo apendiza al final | `readiness-extras.ts:557-560,565` |
| Default "All known blockers cleared" SOLO para lista vacía | `parts.length > 0 ? join : NO_BLOCKERS_NEXT_ACTION` — con blockers no vacíos parts nunca es [] (A.4→runbook, A.9→sign-off, resto→top-3) | `readiness-extras.ts:566` |
| A.9 siempre presente hoy → default hoy inalcanzable | `doctrinalBlockers()` devuelve EXACTAMENTE un blocker: `a9_go_no_go_formal_pending` (A.4/A.5/A.6/A.7/A.8 todos resueltos y removidos) | `readiness-extras.ts:349-422` |
| Orden estable dentro de severidad = env→readiness→doctrinal | Ensamblado `[...env, ...readinessBlockers, ...doc]` | `readiness-extras.ts:502` |
| No muta el array de entrada | `.filter()` crea copia antes de `.sort()` + test 7 lo pinea | `readiness-extras.ts:557-561`, `readiness-extras.test.ts:308-316` |

### 1.2 Verificación reproducible — re-ejecutada por mí (no citada del fixer)

| Check | Resultado propio |
|---|---|
| `npx vitest run src/routes/readiness-extras.test.ts` (api-server) | **29/29 PASS** (2.63s) — 22 pre-existentes + 7 nuevos, contados |
| `npm run typecheck` (tsc --noEmit -p tsconfig.json) | **CLEAN** (cero errores) |
| Diff del test file | **Puramente aditivo** — 0 tests pre-existentes modificados; el destructuring solo AÑADE `readinessItemsToBlockers, composeNextAction` |

Los 7 tests nuevos (`readiness-extras.test.ts:221-317`) son adversarialmente sólidos: el test 1
(:232-251) inserta g_disk_1 (high) ANTES que g_pipe_1 (critical), así que el orden por severidad se
prueba de verdad (sin sort el test fallaría); el test 2 (:253-268) consume el mapping real de
`readinessItemsToBlockers` (guardia de id-drift); test 3 preserva el runbook A.4 pre-existente;
test 5 punea el cap de 3 con lows cayendo primero. El único detalle no asertado explícitamente es el
separador `" Then: "` — verificado por lectura directa (:566) y consistente con el preview del fixer
(re-derivado: `join(" Then: ")` sobre los shapes vivos reproduce el string publicado exactamente).

### 1.3 Sin cambios de contrato wire / sin cambios FE de H-1

- `DecisionResponse.next_action: string` (`readiness-extras.ts:124`) — sin cambio.
- `frontend/lib/schemas.ts` `next_action: z.string()` — hoy en **:846** (el fixer citó :842; la
  deriva de 4 líneas es por el diff CONCURRENTE de **H-4** que añadió `window_total` en :116-119 del
  mismo archivo — atribución verificada por `git diff`; H-1 NO tocó FE).
- Renderizado verbatim, sin parseo ni truncamiento: `frontend/features/readiness/GoNoGoPanel.tsx:177,245`
  (el panel de `/live-readiness`) y `frontend/app/readiness/page.tsx:105`. El string compuesto más
  largo renderiza como párrafo normal.
- **Edge NO duplica la lógica**: `edge/worker/src/index.ts:1386` PROXY-pasa
  `/api/readiness/decision` → `/api/v1/readiness/decision` (KV TTL 15s). El fix en api-server se
  propaga solo tras deploy; no existe una segunda copia de la prosa vieja (grep repo-wide de
  "All known blockers cleared": solo constante inalcanzable + tests + reportes de auditoría).

### 1.4 Sin consumidores rotos (regresión)

- Grep `next_action` repo-wide: ningún FE/edge hace string-matching contra la prosa vieja (solo
  render). `agents-status.ts` la referencia con evidence strings estáticas (:188,:222,:239) que no
  dependen del contenido de next_action.
- `a4Blocker` sigue vivo en el handler para `go_a4`/`go_a5` (`readiness-extras.ts:619-624`) — el fix
  no dejó variable muerta (tsc lo confirma).
- Los 3 flakes WS que el fixer reportó bajo suite completa (`websocket-carnot.test.ts`,
  `websocket.test.ts`): **grafo de imports concluyente** — importan solo `websocket.js` /
  `websocket-carnot.js` / `services/carnotStore.js`, NI index.ts NI readiness-extras →
  clasificación del fixer (flakes de timing ambientales) confirmada sin necesidad de re-run.

### 1.5 RULE 00 / R8

Cada carácter del string compuesto proviene de `blockers` (id, severity, required_action) más dos
constantes doctrinales pre-existentes (runbook A.4, sign-off A.9 — texto que ya existía en el código
viejo, atado a gates reales). Cero prosa inventada, cero datos fabricados. El "all-clear" ahora solo
se afirma cuando nada bloquea — exactamente el "DEBERÍA mostrar" del auditor R8 §3 H-1.

## 2. Ángulos adversariales intentados (y por qué NO son gaps)

1. **"¿Y si hay ≥4 critical?"** → cap de 3 deja fuera los de menor severidad relativa; el panel de
   blockers completo (mismo endpoint, campo `blockers` + `/api/v1/readiness/blockers`) sigue
   mostrando TODOS. next_action es resumen, no reemplazo. No es gap.
2. **"¿Sort inestable / mutación?"** → ES2019 sort estable garantizado en Node; filter→sort sobre
   copia; test 7 pinea inmutabilidad. No es gap.
3. **"¿Ids duplicados A.4/A.9 burlan la exclusión por referencia (`b !== a4`)?"** → teóricamente
   posible, pero ningún productor actual (`envBlockers`/`readinessItemsToBlockers`/
   `doctrinalBlockers`/fallback verifyAll-failed) genera ids duplicados (ids fijos / readiness ids
   canónicos). **Observación de endurecimiento NO bloqueante**, no gap.
4. **"¿Severity desconocida → NaN comparator?"** → `BlockerSeverity` es union sellada y todos los
   productores son internos; worst case sería orden impredecible sin crash. Misma clasificación:
   endurecimiento teórico, no gap.
5. **"¿Persistencia del string largo en DB/cache con límite?"** → next_action vive SOLO en la
   respuesta HTTP (sin columna PG ni Redis dedicado; el KV del edge cachea la respuesta completa
   15s). No es gap.

## 3. Gaps RESTANTES (solo los que persisten)

| # | Gap | Clase |
|---|---|---|
| 1 | **El fix NO es visible en el dominio público hasta deploy** (NO-GIT intacto, correcto): producción `/live-readiness` seguirá mostrando la prosa falsa "All known blockers cleared…" hasta que el diff entre por PR + pipeline aprobado. Hand-off del fixer (§4.1) re-confirmado. | operator-gated (deploy pipeline; §37 un PR = un ID) |
| 2 | **Gap #5 del auditor (disco VPS 94.5%, crit 95%, 8.2 GB libres)** — fuera del scope de H-1, sigue vivo y empeorando (re-llenado post-ARBX-RETENTION-01). El fix lo hace MÁS visible (lo nombra `readiness_g_disk_1 [high]` de primera mano), no lo resuelve. | operator-gated (acción VPS) |

H-2/H-3/H-4 (gaps #2-#4) tienen ya sus fixers (reportes en este dir) y sus propios verifiers — no
son gaps de este WO.

## 4. Hand-off

1. El diff H-1 está listo para PR cuando la mesa/operador abran la ventana de git (post
   verificación de los 4 WOs H-*). Sugerencia: re-ejecutar los 3 checks de §1.2 en CI del PR.
2. Tras deploy, el par browser puede cerrar el loop con el mismo journey §2 fila 3 del auditor R8
   (`/live-readiness` → el bloque "Next action" debe listar `readiness_g_pipe_1 [critical] … Then:
   readiness_g_disk_1 [high] … Then: Await A.9 formal GO/NO-GO sign-off.`) — con los shapes vivos
   de ese momento.

**Presupuesto dominio público: 0 requests HTTP. 0 ssh. 0 git. 0 escrituras VPS. §32/§33/§34.3
intactos.**
