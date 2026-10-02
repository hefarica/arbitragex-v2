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
