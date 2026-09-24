# BOARD — LIVE-READINESS 100% (programa umbrella, 2026-09-21)

> /goal (operador, aprobado 2026-09-21): cerrar TODAS las actividades pendientes de
> https://arbx.ape-tv.net/live-readiness con evidencia real end-to-end, sin forzar
> avisos, hasta que NO_GO dependa SOLO del sign-off A.9 [OPERADOR].
> Plan canónico: `docs/superpowers/plans/2026-09-21-live-readiness-5w2h.md` (5W2H).
>
> Reglas BOARD (§9.1.1): leer ANTES de trabajar, CLAIM con dueño + LOCK, actualizar
> al cerrar con evidencia reproducible. Doctrinas: RULE 00 · R8/R10 · §32/§33
> (read-only, $0 capital, sin broadcast/wallets/flips) · no-git-until-final-gate.
> FASE 1 delega en los BOARD ya existentes (forensic + Stream A) — NO se duplica.

## FASE 0 — Higiene (hoy, paralela)

### WO-LR0.1 — Diagnóstico kill-switch socket DISCONNECTED + ignición 0/4 wiring ✅ DONE (2026-09-21 12:15Z) → `WO-LR0.1-PANEL-DIAG.md`
- **Veredicto**: DISCONNECTED/CONNECTING = estado inicial honesto (R1/§34, `wsConnected:false` SSR
  hasta el primer `connect`); NO bug de transporte (handshake 200 en :5173/:8080, sesión headless
  WS viva >10 min). Ignición 0/4 NO reproducible: `/api/readiness/steps` = **4/4 PASS** (topology
  4 WSS+5 RPC · signer present · 6/23/1750/4803 · 8 engines) y panel renderiza 4/4. Paso 2
  (signer) NO es [OPERADOR]: ya presente server-side.
- **2 defectos reales nuevos**: (A) `quote_anchor ERROR "Failed to fetch"` = FE-EDGE-DIRECT-01
  (en prod f0e94d91, no en main-local) manda anchor/opportunities/trading-config a
  edge-arbx.ape-tv.net y el edge NO emite ACAO — `wrangler.toml:16` ALLOWED_ORIGINS apunta a
  localhost:3000 (prod comentado en :59). Fix: config wrangler [OPERADOR] o revert edge-direct
  (WO-LR4.1). (B) `runtime_ack ERROR` = room admin-gated sin sesión (ROOM-AUTH-01, by design).
- 6º endpoint documentado (stepper usa `/api/readiness/steps`, fuera de los 5 del charter);
  `go-no-go/status.generated_at` congelado 09-07 (menor); ventana de fallo en bloque ~12:05Z
  documenta cómo pudo verse 0/4. CERO edits de código.

### WO-LR0.2 — Disco VPS <85% (G-DISK-1) ✅ DONE (orquestador, 2026-09-21 11:26Z)
- Diagnóstico: disco real 91% (peor que el 87.4% del endpoint). Consumidor = PG 94GB;
  la pieza mayor era `route_discovery_outcomes_pre122` (18GB, 36.4M filas, legado
  migración 122, sin ningún reader en el repo ni en runtime).
- **Hallazgo crítico**: la asunción de la migración 122 ("su rollup ya está materializado")
  era FALSA — 64 buckets 5m contiguos (2026-09-20 08:30–13:45Z, ~215K filas c/u) sin
  filas `__totals__` en `route_discovery_outcome_rollup_5m`. Se reparó ANTES del DROP:
  backfill desde pre122 en 8 chunks (misma semántica dims del `rdo_backfill_chunk` de
  pg_retention.sh; MATERIALIZED CTE; ON CONFLICT DO NOTHING; ~22.6K filas rollup).
  Verificación: 193/193 buckets de la ventana pre122 en rollup.
- Runbook migración 122 paso (c) ejecutado: `DROP TABLE route_discovery_outcomes_pre122`
  (metadata unlink, instantáneo, sequence OWNED BY NONE). Builder cache ya 0.
- Gate PASS: `df -h /` 131G→113G usados = **79%** (31GB libres) · PG vivo
  (`MAX(detected_at)=2026-09-21 11:26:04`, sirviendo en vivo) ·
  `/api/readiness/blockers` SIN `readiness_g_disk_1` (blockers 4→3: quedan g_sim_1,
  g_pap_1, a9 — todos con WO en FASES 1/2/4).

### WO-LR0.3 — Continuidad paper YA (arranca reloj G-PAP-1 7 días) ✅ DONE-honest (orquestador, 2026-09-21 11:30Z)
- Infraestructura VERIFICADA ON: `ARBX_TRADE_MODE=paper` + `ARBX_PAPER_ARCHIVER_MODE=on`
  (envs searcher/relays confirmados) · watchdog SCANNER-STALL-01 (#624) landed ·
  searcher VIVO sin stall (pool_sync multicall ok continuo, logs 11:29:53Z, reserves
  n=119/119) · detecciones fluyendo (`MAX(detected_at)=11:26:04Z`).
- Estado del funnel 24h (real, no degradado): `scored_opportunities` 4.02M filas/24h ·
  candidatos `is_opportunity` con profit>0: 316,924 · **tabla `simulations` VACÍA**
  (MAX(simulated_at)=NULL) · `paper_trade_runs`=0.
- Rechazos dominantes 24h (cambio vs 09-20 — `v3_quote_unavailable` YA NO domina):
  `missing_reserves` 6.23M · `funding_feed_unavailable` 5.72M · `bridge_state_unavailable`
  5.72M · `nft_floor_feed_unavailable` 3.33M · `intent_feed_unavailable` 2.59M.
- **Conclusión honesta**: el reloj G-PAP-1 NO puede arrancar por env/config (todo está ON)
  sino porque 0 candidatos cruzan todos los gates (0 sims persistidas, 0 viables) —
  causa raíz = FASE 1 (forensic FE chain + Stream A). Sin stall que corregir aquí.
- Gate: DONE-honest (dependencia documentada; vigilancia activa vía watchdog + este BOARD).

## FASE 1 — Pipeline vivo (delega en BOARDs existentes; no duplicar)
### WO-LR1.1 — Forensic FE1→merge→FE4..FE9 → ver `audits/forensic-engine-2026-09-21/GOAL-WORKORDERS.md`
### WO-LR1.2 — Stream A F3/F4/F5 (cards CEX join + CexDex Phase 2 + deploy R7+E2E)
- Gate FASE 1 (medible): viables>0/día, funnel v3_quote_unavailable fuera del
  bloqueo, `arbx_simulation_total` flow 24h >0.

## FASE 2 — G-SIM-1: 5 evidencias stale (registry readiness_evidence, STRICT 30d)
### WO-LR2.1 — modules_merged + unit_tests + dep_tree (CI auto: push main tocando simulator-v2)
### WO-LR2.2 — fork_suite (workflow_dispatch `sim-fork-evidence.yml`)
### WO-LR2.3 — eth_callbundle_staging (workflow_dispatch `sim-staging-callbundle.yml`; simulate-only, sin broadcast)
### WO-LR2.4 — variance_benchmark [VPS-OPS] `bash scripts/gsim1_variance_benchmark.sh` — REQUIERE ≥100 opps reales (FASE 1)
### WO-LR2.5 — second_signoff [OPERADOR/INGENIERO-2] — preparamos paquete de review; firma humana
- Gate FASE 2: blockers sin `readiness_g_sim_1`; sim_error_breaker fuera de PAUSED.

## FASE 3 — Breakers + scoring
### WO-LR3.1 — executor_breaker probe read-only (getCode/owner/paused de EXECUTOR_1) — TDD
### WO-LR3.2 — Vigilancia breakers acumulación (drawdown/revert_rate): se llenan con F0.3+F1; bug de wiring solo con anomalía ID
### WO-LR3.3 — Propuesta umbrales ARBX_CB_MAX_GAS_BURN_USD + ARBX_RISK_* [decisión OPERADOR]
### WO-LR3.4 — Verificar calibración scoring (calibrated_pairs>0 cuando corran los runs)
- Gate FASE 3: breakers con datos reales; executor fuera de WARN.

## FASE 4 — Ignición accionables + A.9
### WO-LR4.1 — Accionables de código de los 4 pasos (según diagnóstico WO-LR0.1)
### WO-LR4.2 — Ledger A.9 regenerado + runbook listo [firma quorum-2 = OPERADOR]

## Convenciones
- Rust serial en árbol principal (target compartido); TS paralelo OK.
- Gang vía Hermes local (127.0.0.1:8642, max_runs=1, runs=memoria: recolectar al terminar).
- Respawns: presupuesto 8, oleadas ≤4, tripwire muertes≥6 & >2× éxitos ⇒ suspender.
- Cierre de cada WO: evidencia reproducible (salida comando/hash/path/endpoint).
