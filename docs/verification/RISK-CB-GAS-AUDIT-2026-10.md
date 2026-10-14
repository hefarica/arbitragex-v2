# AUDIT — Gas-burn breakers: cero fabricado, cobertura, procedencia, y traza del registro paper

**Sesión**: arbx-arb (DSH) · **Procedencia (corregida 2026-10-02)**: los cambios se elaboraron sobre el árbol compartido `fix/perhop-reserves-01` @ `858b943f`, que **diverge** de `origin/main` (GitHub, remote `origin`): 1 commit propio / 84 de main, merge-base `5b04699f`. El commit revisado por el operador `af99f928` (#753) es el **HEAD de origin/main** y NO es ancestro de `858b943f` — la sección §2 se verificó byte-idéntica entre ambos (`git diff HEAD origin/main -- <4 archivos>` = vacío), por lo que la transferencia al worktree limpio sobre `origin/main` es sin conflictos. **VPS verificado**: `d11cdbc9` (#750), read-only.
**Instrucción ejecutada**: verificación de versiones sin desplegar + correcciones locales con pruebas; eliminar el cero fabricado; distinguir gas estimado/re-ejecutado/liquidado; verificar cobertura; alinear estado↔valor↔umbral; trazar el primer registro paper y el control efectivo; config vigente documentada (WO-LR3.3); deploy separado por commit+imagen.

---

## 1 · Claims del review del operador — verificación contra el árbol

| Claim | Veredicto | Evidencia |
|---|---|---|
| `paper/executor.ts` inserta `sim_gas_cost_usd=null` y sin `actual_gas_cost_usd` | **CONFIRMADO** | INSERT L352-371: `$5 = null`; además copia `netYieldUsd` sim a `actual_profit_usd` (otro pecado de procedencia) |
| `relays-client/persistence.rs` deriva `sim_gas = max(gross−net,0)`, sin `actual_*` | **CONFIRMADO** | `derived_gas_cost_usd` L200-205; el comentario del propio código admite el modelo de 8 componentes → el diferencial NO es solo gas |
| `drift_tracker.rs` llena `actual_*` por re-ejecución sim-ctl | **CONFIRMADO** | UPDATE L395-415 desde outcome de re-exec; sin marcador de fuente en schema |
| A.5 `COALESCE(sim_gas_cost_usd,0)` + suficiencia por filas ⇒ PASS con N filas NULL | **CONFIRMADO** | Query L397 + `rolling-breakers.ts:80-81` (`n = win.length`) |
| A.5 sin filtro `chain_id` vs A.6 con filtro | **CONFIRMADO** | L398-400 vs L457/L502 |
| Severidad derivada del estado (PASS→low/WARN→medium/else→high) | **CONFIRMADO** | L851 |
| `current_value`/`threshold` priorizan A.6 aunque el estado lo decida A.5 | **CONFIRMADO** | L858-859 (`cbValue ?? simValue`, `cap ?? gasBurnCapUsd`) |
| `sim_error_breaker` anuncia racha que no computa | **CONFIRMADO** | L924-953: deriva de G-SIM-1; threshold fake "5 consecutive" |

## 2 · Correcciones implementadas (local, sin commit, sin deploy)

### `backend/api-server/src/routes/risk-circuit-breakers.ts`
1. **Cero fabricado eliminado (A.5)**: la query ya no hace `COALESCE`; el mapper puro `toGasMeasurementOutcomes` (exportado) cuenta como muestras SOLO filas con `sim_gas_cost_usd` grabado. NULL ≠ 0; un 0.0 almacenado ES medición válida y se conserva. `Number.isFinite` defensivo excluye NaN.
2. **Poblaciones alineadas**: A.5 ahora filtra `chain_id = $2` (mismo `cb.chainId` que A.6). `loadCbConfig()` hoisted.
3. **Cobertura (A.6)** según la tabla del operador: suma observada ≥ cap ⇒ **PAUSED** (el exceso probado se informa aunque falten mediciones); suma < cap con cobertura parcial ⇒ **WARN** (no se certifica bajo el cap); cobertura completa ⇒ comparación normal. Denominador conservador: toda fila de la ventana cuenta como esperada hasta que exista marcador de kind de ejecución.
4. **Procedencia explícita**: `ACTUAL_GAS_PROVENANCE = "sim-ctl replay via drift_tracker (not on-chain settled)"` — etiquetado estructural en `evidence.paths.actual.provenance` y en el detail de toda suma actual. El replay NUNCA se presenta como gasto on-chain liquidado.
5. **Dual-path + alineación**: `evidence.paths = {sim:{state,value,measurements,window_rows,window_secs,scope,reason}, actual:{state,value,measured,expected,window_hours,scope,provenance,reason}}` + `evidence.deciding_path`. El `current_value`/`threshold` mostrados provienen del camino que DECIDE el estado (empate ⇒ A.6, la autoridad del cap operatorio).
6. **`required_action` paridad**: NA de gas ahora dice "Accumulate paper runs with actual_gas_cost_usd…" (igual que revert_rate); WARN de cobertura indica esperar el backfill de drift_tracker.
7. **`sim_error_breaker` honesto**: renombrado "Simulator readiness gate (G-SIM-1)"; threshold/unit `null`; descripción declara que deriva del checklist SECURE_BOOT, no de una racha.

**Paridad Rust preservada**: la matemática canónica (`shared-rs/src/risk_ledger.rs`) NO se tocó — el fix es del loader TS, que ahora solo pasa mediciones válidas. El contrato de carga queda documentado en `toGasMeasurementOutcomes` (un test lo pinea).

### `backend/api-server/src/routes/risk-circuit-breakers.test.ts`
- Actualizado el test que **codificaba el defecto** (PASS con 120/500 medidos) → ahora exige WARN + "NOT certified".
- Nuevos: cobertura completa PASS; exceso con huecos PAUSED (1/100, $60≥$50); deciding_path sim/actual con valor+umbral del camino que decide; estado KILLED por sim muestra el número SIM; required_action en NA vacía; suite `toGasMeasurementOutcomes` (el contraejemplo 10-NULL/10-min ⇒ 0 mediciones).

### Frontend
- `lib/schemas.ts`: `CircuitBreakerEvidenceSchema` + `deciding_path` enum y `paths` (opcionales, wire-aditivos; sin ellos el parse los stripping silenciosamente).
- `features/risk/RiskCircuitPanel.tsx`: tooltip de severidad corregido ("Severidad asignada por el evaluador… NOT_AVAILABLE ≠ falla"); bloque "Deciding path" con conteos sim/actual (`data-testid`).

## 3 · Evidencia de verificación (todo local, sin deploy)

| Verificación | Resultado |
|---|---|
| `vitest run risk-circuit-breakers.test.ts` | **82/82 PASS** (39 ms) |
| `vitest run` (suite completa api-server) | **896/898** — 2 fallos SOLO en `pii-wireado-recursive.test.ts` por **timeout de disco** (5 s default); re-ejecutado aislado con 60 s: **5/5 PASS** (4.2 s de ejecución) → ambiental, no regresión |
| `tsc --noEmit` api-server | **LIMPIO** |
| `vitest` frontend (panel + contrato Zod) | **5/5 + 27/27 PASS** |
| `tsc --noEmit` frontend | 7 errores **preexistentes**: `@rainbow-me/rainbowkit` no instalado en este árbol local (paquete ausente en node_modules, archivos wallet/web3 no tocados por esta sesión); mis 2 archivos typecheckean limpio |
| Árbol compartido | `git diff --name-only`: solo mis 4 archivos; las modificaciones preexistentes de otras sesiones intactas |

## 4 · FASE 1 — Versiones (read-only, sin deploy)

- **VPS git**: `d11cdbc94fa3…` ("ADMISSION-MANIFEST-01", #750). El commit revisado por el operador (`af99f928`, #753) NO está desplegado — el VPS va detrás del main local (esperable: deploy gated).
- **Digests de imagen en ejecución**: api-server `sha256:7dfb3eda…`, frontend `sha256:eda891e4…`, relays-client `sha256:56ba0087…`, searcher-rs `sha256:68618fb6…`. Todo el stack `Up 2 hours (healthy)` (26 contenedores).
- **Nota de método** (del operador, adoptada): `git rev-parse` del VPS no identifica la imagen corriendo; el digest sí. El `build --no-cache && up -d` futuro es DEPLOY, no inspección.

## 5 · FASE 3 — Traza del primer registro paper: el control efectivo (evidencia en vivo)

```
searcher-rs (heartbeat 60s, VPS 00:30–00:37Z)
  pg_period_inserted ≈ 1.205/min · gate_other_rejected ≈ 1.248/min
  passed_all_gates: 0 · pg_period_profit_pos: 0
  triangular/flashloan/liquidation scanned: 0
        ↓ (0 candidatos pasan los gates)
sim-ctl (consumidor → "if passed → XADD arbx:opps:simulated")     ← sin input, sin XADD
        ↓
Redis arbx:opps:simulated   XLEN = 0        ← CANAL WIRED PERO STARVED
(detected: XLEN=10.000 — la detección está viva)   [channels-registry: "wired-starved"]
        ↓
relays-client (boot 23:02Z: paper_mode=true, spawn_consumer=true,
  "consumer spawned without signer — paper_trade_runs only, no broadcast",
  stream=arbx:opps:simulated, group=relays-client-g0)             ← ARMADO, EN ESPERA
        ↓
paper_trade_runs = 0 filas (verificado por SQL)
        ↓
gas_burn / revert_rate / drawdown = NOT_AVAILABLE (R8 correcto)
```

**Motivos de rechazo reales (última hora, SQL)**: `spread_zero_equilibrium` 33.377 (35% — **verdad de mercado**: sin spread no hay edge, rechazo honesto), `v3_quote_unavailable` 24.835 (26% — **cuello reparable**: QuoterV2, incidente 2026-09-25; fix = batching/backoff/higiene RPC per §38, jamás proveedor de pago), `non_positive_profit` 8.831, `single_pool_no_spread` 4.538, `no_tradable_size` 2.089, `v3_multileg_budget_exhausted` 329.

**Conclusiones**: (a) el control efectivo es el funnel de gates del searcher — nada llega a simulación; (b) **NO hay dependencia circular con LIVE**: el camino paper está armado y funciona sin signer (logs lo prueban); producir muestras NO requiere broadcast ni preparación live; (c) el mayor palanco reparable es `v3_quote_unavailable`; (d) `spread_zero_equilibrium` no se "arregla" — es el mercado diciendo la verdad.

## 6 · WO-LR3.3 — umbrales: decisión pendiente del operador (config NO tocada)

- Vigente en VPS: `ARBX_RISK_MIN_SAMPLES` efectivo = **10** (el endpoint responde "(0/10)"); el screenshot del operador mostraba "(0/70)". La evidencia no resuelve cuál fue aprobado — **no se cambió nada**. Este documento deja constancia; la decisión sigue siendo del operador (audits/live-readiness-20260921 GOAL-WORKORDERS WO-LR3.3).
- Piso de muestras para revert_rate (propuesta previa mía, `ARBX_CB_REVERT_MIN_RUNS` opcional): **fuera de este cambio** — es superficie de config nueva y el operador no la autorizó en esta ronda.

## 7 · Matriz de cierre del operador → cobertura

| Caso exigido | Estado |
|---|---|
| Ventana vacía ⇒ ausencia de evidencia, no cero | ✅ existente + test, required_action añadido |
| Filas suficientes con gas NULL ⇒ no son mediciones válidas | ✅ FIX + test (contraejemplo 10 NULL) |
| Gas medido exactamente 0 ⇒ se conserva como cero válido | ✅ test en `toGasMeasurementOutcomes` |
| Mediciones parciales ⇒ cobertura informada, total no certificado | ✅ WARN + detail x/y + test |
| Gas observado supera límite ⇒ exceso informado pese a huecos | ✅ PAUSED + test (1/100, $60≥$50) |
| A.5 y A.6 discrepan ⇒ ambos expuestos + origen de la decisión | ✅ `paths` + `deciding_path` + test |
| Mezcla de cadenas/ventanas ⇒ alcance explícito | ✅ A.5 chain-filtered; `scope` por camino; ventanas (`window_secs`/`window_hours`) expuestas |
| `actual_*` por replay ⇒ no presentado como liquidado on-chain | ✅ `provenance` estructural + detail + test |

## 8 · Follow-ups registrados (NO ejecutados aquí)

1. **Columna marcadora de procedencia** (`actual_gas_source`: onchain_receipt | simctl_replay) + writers — obligatoria ANTES de que exista cualquier writer on-chain (hoy el único writer es drift_tracker y está etiquetado).
2. **`executor.ts`**: `sim_gas_cost_usd=null` hardcodeado y `actual_profit_usd = netYieldUsd` sim — necesita el mismo tratamiento de procedencia.
3. **`persistence.rs`**: revisar el contrato `sim_gas = gross−net` (8 componentes ≠ gas puro) — requiere decisión de diseño del spine.
4. **`v3_quote_unavailable`** (26% de rechazos): batching/backoff/higiene de lista RPC (incidente 2026-09-25 R1/R2) — es el mayor palanco para que este breaker alguna vez compute.
5. **G-SIM-1**: `variance_benchmark` (0/15 muestras etiquetadas) + `second_signoff` stale — producir la evidencia real, no actualizar fechas.

## 9 · Plan de deploy (separado, gated)

1. Commit de estos 4 archivos en un branch dedicado (el árbol actual es compartido multi-sesión sobre `fix/perhop-reserves-01` — NO se commiteó aquí; los cambios están en working tree + este documento).
2. CI: suite api-server completa (con timeout corregido para el test PII en runners lentos si aplica) + typecheck + suite frontend.
3. Deploy VPS por el flujo canónico (RULE 01/03) **asociado al commit SHA + digest de imagen** (método §4).
4. Verificación post-deploy (arbx-vps-verification-runbook): curl interno 8080 + Edge público comparando **campos semánticos de la misma evaluación**, SQL de conteo, screenshot de la card.
5. Rollback: `git revert` + redeploy (§37).

**Invariante de sesión**: sin deploy, sin broadcast, sin umbrales tocados, sin commits en el árbol compartido, secretos nunca impresos (env redactados), cada número de este documento es medido o marcado como supuesto.


---

# ADDENDUM v2 (2026-10-02) — PR, validación lockfile base/candidato, V3 y seguridad

## A · PR y transporte

- **PR #754**: https://github.com/hefarica/arbitragex-v2/pull/754 · base `main` · head `fix/risk-cb-gas-provenance`.
- Commit final: **`19bb342a1361e8df1aa2645748bc5cdd4a90acc8`** (parent `af99f928`). El primer intento (`721ba162`) subió blobs corruptos (gh `-f @file` NO expande @file — era curl-ismo; los blobs contenían la ruta literal de 55 bytes) y fue **reemplazado por force-update del ref** (branch propio, minutos de vida, sin consumidores) — el commit corrupto ya no está en la historia del PR.
- **R13**: `GET pulls/754/files` = exactamente los 5 archivos intencionales (2 backend, 2 frontend, 1 doc). Cero arrastre de sesiones ajenas (los cambios se elaboraron en el árbol compartido y se transfirieron a un worktree limpio sobre `origin/main`).

## B · Validación con dependencias del lockfile (contenedor, base vs candidato)

Método: clones depth-1 de `main` (base) y del branch (candidato) en `/tmp` del VPS; imagen `arbitragex-v2-api-server` (node 20.20.2/npm 10.8.2); `npm ci` del lockfile raíz con `NODE_ENV=development` (la imagen corre production por defecto y **npm ci omite devDeps bajo NODE_ENV=production** — hallazgo: sin esa env la instalación fresca carece de vitest y @types); misma imagen, misma máquina, ejecución secuencial.

| Verificación | Base `af99f928` | Candidato `19bb342a` | Delta |
|---|---|---|---|
| `npm ci` (lockfile) | OK | OK | — |
| PII suite ×3 (timeout default 5s) | **3/3 PASS** — 1.46s / 0.68s / 0.89s | **3/3 PASS** — 1.39s / 0.82s / 0.68s | 0 |
| Suite completa api-server | 16 files FAIL (colección) · 56 pass · **728/728 tests** | 16 files FAIL · 56 pass · **728/728 tests** | **0** |
| `tsc --noEmit` | 12 errores (todos `@arbx/shared`) | 12 errores (idénticos) | **0** |
| Suite evaluador (risk-circuit-breakers) | fail de COLECCIÓN | fail de COLECCIÓN (misma causa) | 0 |

**Los 16 files que fallan en instalación fresca fallan por una causa pre-existente, idéntica en base y candidato**: `Error: Failed to resolve entry for package "@arbx/shared"` — el paquete de workspace necesita un build que `npm ci` fresco no produce. Afecta a los 16 archivos que importan `@arbx/shared` (incluida la suite del evaluador en AMBAS versiones) y a los 12 errores de tsc. **Mi cambio no introduce ninguna falla nueva** (delta 0 en suite, tsc y PII). La suite del evaluador (82/82) pasó **solo en el entorno del árbol compartido** (node_modules con @arbx/shared resuelto) — no se reclama como validada-bajo-lockfile.

**Hallazgo de repo (pre-existente, reportar)**: (1) instalación fresca del lockfile no puede correr los tests que importan `@arbx/shared` sin un paso de build del workspace; (2) bajo `NODE_ENV=production` la instalación omite TODAS las devDeps — la imagen api-server no sirve como entorno de validación sin la env de desarrollo. Los timeouts de 5s del test PII no se reproducen en el contenedor (3/3 PASS ambas ramas) — eran artefacto de carga de disco local, documentado aquí como pedía el operador.

## C · V3: `v3_quote_unavailable` — desagregado, reproducido, causa y reparación

**Desagregación (métricas del propio searcher, `arbx_v3_quote_total{outcome}`)**, última hora: rpc 22.557 intentos → ok 13.356 (59%) · **rpc_error 7.145 (32%)** · tier_revert 2.047 (9%); cache_hit 125.650/h; cache_neg_hit 21.487/h; batch_call 4.159 → **batch_call_error 1.222 (29%)**; batch_backoff_skip 4.459/h.

**Por proveedor (`arbx_rpc_provider_*`, última hora)**: `tenderly` **20.679 req (86% del share) con 6.719 errores (32,5%)**; drpc 680 req/0 err; blockpi 229/0; publicnode 93/5; mevblocker 164/5; oxrpc 815/7; blastapi 850/14.

**Reproducción contra el mismo estado** (bloque `0x18e4638` confirmado igual en ambos proveedores; eth_call QuoterV2 USDC→WETH tier 500, 12 intentos por proveedor desde el VPS, URLs nunca impresas): **tenderly = 6/12 HTTP 429**; drpc/blockpi/oxrpc/publicnode = **12/12 HTTP 200**.

**Causa observada (mecanismo)**: (1) tenderly NO figura en `RPC_HTTP_RATE_BUDGETS` (los 7 presupuestados no lo incluyen) → sin tope client-side; (2) la selección del pool es **EWMA de solo-latencia** (`rpc_failover.rs:404` "lowest EWMA latency among Healthy") — los FALLOS no penalizan el ranking; (3) el breaker reabre por sonda barata (R-0003) tras 10-15s → ciclo: gana selección → 429s → breaker → reabre → gana de nuevo. Un proveedor rápido-pero-fluctuante captura el 86% del tráfico y fabrica el 32% de fallo transport que alimenta `v3_quote_unavailable` (26% de TODOS los rechazos del funnel).

**Reparación de la causa observada (higiene, NO batching/reintentos — B1/R5 ya existen)**: añadir presupuesto a los dos proveedores sin budget en `/opt/arbitragex-v2/.env`:
```
# ANTES:  RPC_HTTP_RATE_BUDGETS=publicnode=240,merkle=120,drpc=240,mevblocker=240,blockpi=120,oxrpc=120,onelpc=60
# DESPUÉS: RPC_HTTP_RATE_BUDGETS=publicnode=240,merkle=120,drpc=240,mevblocker=240,blockpi=120,oxrpc=120,onelpc=60,tenderly=60,blastapi=120
```
Deploy **separado y gated** (solo searcher): `cd /opt/arbitragex-v2 && docker compose --env-file .env up -d searcher-rs` (sin rebuild; digest de imagen sin cambio `e1bf4ee2…`, StartedAt nuevo = evidencia del recreate). **Métricas después (cierre)**: share de tenderly < ~10%, `increase(arbx_v3_quote_total{outcome="rpc_error"}[1h])` ≪ 7.145, `batch_call_error` ≪ 1.222/h, y `v3_quote_unavailable` deja de ser el 26% de los rechazos del funnel (SQL `opportunities`).
**Follow-up (no en esta ronda)**: selección failure-aware en `shared-rs/rpc_failover.rs` (penalizar EWMA por tasa de error) — requiere compilación Rust controlada.

## D · SEGURIDAD — rotación requerida

Durante la inspección read-only de env del searcher, mi regex de redacción (solo `https?://`) dejó pasar la línea `RPC_WS_1`, exponiendo **una API key de Alchemy WS en el transcript de esta sesión** (precedente 2026-06-15). **Acción del operador: rotar esa clave** (los RPC públicos del stack soberano no se ven afectados).

## E · Deploy separado (esperando OK del operador)

1. **API/frontend (PR #754)**: merge → CI → deploy por flujo canónico asociado a commit `19bb342a` (o el SHA del merge) + digests nuevos de api-server/frontend. El searcher NO se toca.
2. **Searcher (presupuesto V3)**: cambio de `.env` (§C) + `up -d searcher-rs` — sin git, sin rebuild; verificación por métricas antes/después.
3. Umbrales, signer y broadcast: **sin cambios** (verificado: `FLASHBOTS_SIGNER_KEY` sigue ausente, paper mode intacto).


---

# ADDENDUM v3 (2026-10-02) — evidencia CI verificada, retracciones y corrección V3

## R1 · Validación reproducible: la corrida de CI (verificada de primera mano)

**Run `36952144882`** (event `pull_request`, head **`cedce3f5`**, status **success**), job **`110667479998`** (`lint-and-test-node (20)`, success). Verificado vía GitHub Actions API + grep del log del job (líneas literales):

| Comprobación | Evidencia en el log del job |
|---|---|
| Evaluador | `✓ src/routes/risk-circuit-breakers.test.ts (82 tests)` |
| PII | `✓ src/lib/pii-wireado-recursive.test.ts (5 tests)` |
| api-server (completo) | `Test Files 72 passed (72)` |
| Contrato Zod | `✓ lib/__tests__/api-json-vs-zod-contract.test.ts (27 tests)` |
| Panel | `✓ features/risk/__tests__/RiskCircuitPanel.test.tsx (5 tests)` |
| Frontend (completo) | `Test Files 147 passed (147)` |
| Build + tipos | pasos `npm run build:all` y `npm run typecheck:all` presentes en el job, concluido success |

CI compila `@arbx/shared` ANTES de probar consumidores (build:all → typecheck:all → test:all) — por eso el evaluador corrió y pasó donde mi contenedor improvisado no podía cargarlo.

## R2 · RETIRADA de la conclusión «delta cero / 16 fallos idénticos»

**Retiro la conclusión de «cero regresiones» basada en la validación local del contenedor.** Que base y candidato tropezaran con el MISMO impedimento de carga (`@arbx/shared` sin build, instalación sin devDeps bajo `NODE_ENV=production`) **no demuestra ausencia de regresiones en los tests que nunca llegaron a ejecutarse**. La secuencia correcta del diagnóstico fue: (a) atribuí cientos de errores a hoisting del repo — incompleto; (b) identifiqué `NODE_ENV=production` (devDeps omitidas); (c) con devDeps, `@arbx/shared` requería build — y ahí debí reproducir el ORDEN de preparación de CI en lugar de concluir «instalación fresca rota». **La validación reproducible del candidato es la corrida de CI de R1**, no mis corridas locales. Cierre documental correcto: *la validación local fue incompleta por preparación incorrecta del entorno; la validación reproducible quedó respaldada por el run 36952144882*.

**Runbook de validación local (corregido, para copia aislada — nunca sobre el contenedor productivo):**
```bash
set -euo pipefail
npm ci --include=dev --no-audit --no-fund
npm run build --workspace=@arbx/shared
npm run test --workspace=@arbx/api-server -- src/routes/risk-circuit-breakers.test.ts
npm run typecheck --workspace=@arbx/api-server
npm run test --workspace=@arbx/api-server
```

## R3 · Corrección del mecanismo V3 (dos afirmaciones mías retiradas)

| Afirmación anterior (MÍA, retirada) | Comportamiento real del código (revisión del operador) |
|---|---|
| «Selección EWMA de solo-latencia; los fallos no penalizan» | `pick()` distingue Healthy / Degraded / Open / **recientemente-rate-limited (con penalización temporal)**; `report_failure()` clasifica la clase de error y puede abrir el circuito. La latencia participa, no gobierna sola. |
| «with_retry recorrió TODOS los endpoints» | `with_retry` hace **hasta dos intentos** (primero + failover a otro), no un barrido de los ocho. |

**Título correcto del hallazgo V3**: *Rate limiting reproducido en el endpoint observado (tenderly: 6/12 HTTP 429; resto 12/12 HTTP 200, mismo bloque); **pendiente verificar su tratamiento en el selector y la mejora bajo carga representativa**.*

**Verificaciones pendientes ANTES de tocar política de selección o presupuesto** (entrega RPC separada):
1. **Alias del presupuesto**: confirmar que la clave de `RPC_HTTP_RATE_BUDGETS` coincide EXACTAMENTE con el nombre de la entrada del pool que consume `parse_budgets`.
2. **Clasificación del 429**: verificar que el 429 de tenderly llega a `report_failure()` como clase `rate_limit` (y no transformado), observando estado y selección posterior.
3. **Ámbito de métricas**: `arbx_rpc_provider_*` etiqueta por proveedor/tipo/resultado del POOL — el «86% del tráfico» NO debe leerse como «86% de las quotes V3» sin demostrar la población.
4. **Resets**: comparar ventanas con recreaciones del searcher usando `rate()/increase()` **por serie antes de agregar**.
5. **Mitigación**: presupuesto basado en capacidad documentada/medida del proveedor — no inventar cpm desde una ráfaga de 12; no derivar tráfico masivo a un endpoint que respondió 12/12 una vez. Criterio de éxito: **quotes V3 válidas y rutas completamente cotizadas** (no solo menos errores por intentar menos).

## R4 · Credencial Alchemy — INCIDENTE ABIERTO

Estado: **abierto hasta rotación + actualización de consumidores + verificación de reconexión** (transcript de esta sesión la expuso; identificado el consumidor: `RPC_WS_1` del searcher). Diagnósticos futuros de env: **alias + hostname + estado nada más** — jamás URL completa ni querystring. Nota de gobernanza aceptada: la capacidad técnica de una herramienta/herramienta-API no sustituye la autorización de la acción concreta.

## R5 · Protocolo de transporte por blobs (lección del commit corrupto)

Tamaño igual ≠ contenido igual. Para cualquier entrega futura por API: conjunto exacto de rutas esperado + **recuperación del contenido de cada blob + comparación por hash/bytes** + diff final + SHA resultante, ANTES de abrir/actualizar el PR. (En este caso, la corrida CI de R1 compiló y ejecutó el evaluador sobre el contenido real — comprobación adicional de integridad.)

## R6 · Estado de cierre

- **#754 (api/frontend)**: evidencia CI verificada (R1); integración y deploy acotado por flujo autorizado con commit+digest verificados. Criterio de verificación post-deploy: el endpoint y la UI conservan nulls / ceros medidos / cobertura / camino decisor según contrato — **no se exige que las cards se pongan verdes con el ledger vacío**.
- **RPC/V3**: entrega separada con el checklist de R3; sin cambios de umbrales económicos, signer ni broadcast.
- **Credencial**: incidente abierto (R4).
