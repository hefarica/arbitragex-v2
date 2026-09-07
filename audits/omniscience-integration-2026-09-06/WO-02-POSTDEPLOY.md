# WO-02 — POST-DEPLOY FAIL-HONEST + BOARD-SYNC (gang fix ronda 1)

- **Work-order:** WO-02 · **Tipo:** POSTDEPLOY (el reporte post-deploy fail-honest que faltaba — gap **G2** de `WO-02-CROSS.md` §3) + BOARD-SYNC de la fila WO-02 de `GOAL-WORKORDERS.md`.
- **Charter del fixer:** el board podía leer "N3#2 cerrado" (`**APPLIED_VERIFIED + DEPLOYED** ✅`) cuando el estado real de producción es stream vacío POR MODO. Volcar la condición medida al board row citando el CROSS.
- **Reglas respetadas:** 0 requests HTTP al dominio público (0/5) · 3 SSH read-only a `arbx` (ninguna mutación: `git rev-parse/log`, `docker ps/inspect/logs`, `redis-cli XLEN` ×3, `grep`/`sed -n` sobre el checkout) · 0 git local (sin commit/push/PR, protocolo operador 2026-08-23) · 0 cambios de código · §32/§33 audit/read-only intactos.
- **Fecha:** 2026-09-07, medición fresca ~06:00Z (≈15 min tras el boot 05:47:54Z del searcher).

---

## 0. VEREDICTO: N3#2 SIGUE ABIERTO EN PRODUCCIÓN — confirmado con medición propia POST-PR#549

El wiring WO-02 está deployado y presente en el checkout, PERO producción corre `ARBX_ORCHESTRATOR_MODE=v2` y la función que contiene TODO el wiring retorna temprano bajo v2 ⇒ `arbx:hot:simulated` sigue XLEN=0. La condición medida por el CROSS en `a60de001` PERSISTE en el deploy más nuevo `931ad736` (boot verificado propio).

## 1. Medición fresca (SSH read-only, propia — no heredada)

| Qué | Valor | Evidencia |
|---|---|---|
| VPS HEAD | `931ad73619d3beb327b4ae6e621eb5c1bd2285cc` = "Merge pull request #549 from hefarica/fix/wo15-xinfo-shape" (ancestro directo: `a60de001` = PR #548; el CROSS midió sobre `a60de001`) | `git rev-parse HEAD` + `git log --oneline -3` en `/opt/arbitragex-v2` |
| `XLEN arbx:hot:simulated` | **0** | `docker exec arbitragex-v2-redis-1 redis-cli XLEN` |
| `XLEN arbx:hot:detected` | **0** | ídem |
| `XLEN arbx:opps:detected` | **10001** (publicación canónica VIVA) | ídem |
| Modo (config) | `ARBX_ORCHESTRATOR_MODE=v2` (también `ARBX_TRADE_MODE=paper`, `SIM_ORCHESTRATOR_MODE=multistep`) | `docker inspect arbitragex-v2-searcher-rs-1` `.Config.Env` |
| Modo (runtime) | boot log `"event":"scanner.orchestrator_mode","chain_id":1,"mode":"v2"` @ **2026-09-07T05:48:09.340741Z** | `docker logs arbitragex-v2-searcher-rs-1` |
| Ventana R9 íntegra | StartedAt `2026-09-07T05:47:54.457863Z`; primera línea retenida del log = `05:47:54.679346Z` (== boot) ⇒ SIN rotación; la línea de modo es del boot ACTUAL, no un artefacto de ventana | `docker inspect .State.StartedAt` + `docker logs … \| head -1` |
| Wiring en checkout deployado | `emit_simulated` presente: VPS `backend/searcher-rs/src/scanner.rs:2655` | `grep -n emit_simulated` sobre el checkout VPS |
| Salud | `arbitragex-v2-searcher-rs-1` y `arbitragex-v2-redis-1` "Up 15 minutes (healthy)" — redeploy reciente (#549) | `docker ps -a --format` |

**Gotcha R9/auto-infligido (documentado):** la 1ª llamada SSH usó `docker logs searcher-rs` (nombre corto canónico de R7) — el container real es `arbitragex-v2-searcher-rs-1`; el error "No such container" quedó FILTRADO por mi `2>&1 | grep orchestrator_mode` ⇒ salida vacía indistinguible de "sin match". Detectado y resuelto con la 2ª llamada (ps + inspect). El nombre corto R7 está stale para el compose de prod actual.

## 2. Inalcanzabilidad estructural (verificación local de primera mano)

Árbol local `backend/searcher-rs/src/scanner.rs` (con hunks WO-02 unstaged — mismo contenido que el checkout VPS con drift −6):

- `decode_and_score_tx` (def `scanner.rs:1488`, cierre `:2680`) contiene TODO el circuito.
- **Early-return V2**: `scanner.rs:1589-1592` — `if orch_mode == OrchestratorMode::V2 { return Ok(()) }` ("In V2 mode the orchestrator is the sole emit path — skip legacy").
- **Sección sim**: único call-site de `dispatch_orchestrator_and_classify` (def `:2952`) = `scanner.rs:2417`, DENTRO del spawn_blocking legacy (`:2406-2436`) — única productora de `hot_sim` (confirmado también por `sim_orchestrator.rs:28`: "The PRODUCTION producer (`scanner::dispatch_orchestrator_and_classify`)…").
- **Emisor WO-02** (marcador `// WO-02 (2026-09-06)` en `:2651`): `if let Some(sim) = hot_sim { … emitter.emit_simulated(&opportunity, &sim) }` = `scanner.rs:2659-2669`, post-`publisher::publish` (`:2648`), fail-soft.
- **Consecuencia**: bajo v2, la función retorna en `:1591` ANTES de `:2417` y de `:2659` ⇒ wiring vivo-pero-inalcanzable. La pierna viva en v2 (`opportunity_emitter.rs`) no pasa por `dispatch_orchestrator_and_classify` (su comentario `:311` lo confirma: "simulation classification only") ni tiene wiring WO-02.

## 3. Corrección de claim de la fila previa (la parte que engañaba al board)

La fila decía: *"cierra al reactivarse la cadena de sims = flips operador (R-0001/S4)"*. **Insuficiente bajo v2**: reactivar sims es necesario pero NO alcanza — la estructura salta la pierna donde vive el emisor. Además atribuía la no-emisión solo a "flujo 100% rejected + sim fail-closed ⇒ hot_sim=None"; la causa raíz medida es el **early-return del modo** (los 2 boots consecutivos en v2 con XLEN=0 lo soportan). Condiciones de cierre reales (CROSS §3-G1):

- **(a)** nuevo WO cableando la pierna V2 viva (`opportunity_emitter.rs`) — agent-fixable;
- **(b)** flip de modo operador — §34-adjacente, blast radius enorme, NO como acción liviana.

## 4. Fix aplicado al board

`GOAL-WORKORDERS.md` fila WO-02, celda Estado (marcador `[board-fix WO-02 2026-09-07 — G2 del CROSS]`; en markdown-table no existe `//`-comentario, el bracket ES el marcador):

- Headline: `**APPLIED_VERIFIED + DEPLOYED** ✅` → `**APPLIED_VERIFIED (local) + DEPLOYADO PERO INALCANZABLE EN PROD — N3#2 SIGUE ABIERTO** ⚠️`.
- Números medidos §1 volcados con SHA/boot propios (`931ad736` / 05:48:09Z) + los heredados del CROSS (`a60de001` / 05:14:30Z), ambos citados.
- Condición de cierre corregida a (a)/(b) con la corrección de claim §3.
- Citas: `WO-02-CROSS.md` §2 + §3-G2 y este `WO-02-POSTDEPLOY.md`.
- **Claims de otros agentes PRESERVADOS**: el fixer paralelo de G3 (reporte `WO-02-FIX.md`) había actualizado concurrentemente la MISMA fila ("G3 CLOSED … docs/redis-schema/hot-path-v2.md"); su segmento quedó intacto al final de la celda — mi reemplazo cubrió solo headline→condición de cierre. Sin conflicto de contenido (G2 board ≠ G3 docs); la única contención fue el retry del Edit por "file modified since read".

## 5. Verificación del fix (ejecutada, no declarada)

| Check | Resultado |
|---|---|
| Integridad de tabla Kanban (script PowerShell sobre las 15 filas `\| WO-`) | **PASS** — WO-02 cols=5 ✓; 14/15 filas cols=5; excepción WO-07 cols=7 = artefacto PRE-EXISTENTE de pipes escapados `\|log_lr\|` (el splitter los cuenta; markdown los renderiza como pipe literal intra-celda) — fila no tocada por este fix |
| Headline ya no dice "DEPLOYED ✅" cerrado | **PASS** — `**APPLIED_VERIFIED + DEPLOYED**` → 0 hits en el board |
| Segmento G3 del fixer paralelo sobrevive | **PASS** — `G3 CLOSED` → 1 hit en la fila WO-02 |
| Marcador propio presente | **PASS** — `[board-fix WO-02 2026-09-07` → 1 hit |
| Citas de la fila resuelven | **PASS** — `WO-02-POSTDEPLOY.md` 1 hit · `WO-02-CROSS.md` 1 hit · ambos archivos existen (Test-Path True) |
| Ninguna otra fila del board tocada | **PASS** — Edit de substring único limitado a la celda Estado de WO-02 (columns 1-4 byte-idénticas; contención del write concurrente documentada en §4) |

## 6. Presupuesto

0 requests HTTP al dominio público (0/5). 3 SSH read-only a `arbx` (ninguna mutación). 0 git local. 0 escrituras fuera de: fila WO-02 del board + este reporte.

---

*WO-02 POSTDEPLOY — 2026-09-07. Fail-honest: medición propia post-#549 confirma el veredicto GAPS del CROSS; el board ya no puede leerse como "N3#2 cerrado". N3#2 permanece abierto en producción hasta (a) o (b).*
