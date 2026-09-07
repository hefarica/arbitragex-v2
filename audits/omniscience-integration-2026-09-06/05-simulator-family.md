# N5 — Familia de Simulación (sim-ctl + simulator-v2/sim-core + math-engine + labels)

- **Agente**: verificador N5 (simulator-family), round-table omniscience 2026-09-06
- **Estado**: FINALIZADO
- **Veredicto**: **DEGRADED** — servicios sanos, integrados y honestos, pero la cadena detección→label NO completa: 0 simulaciones aprobadas en TODO el historial (1.000.718 filas), labels=0, stream `arbx:opps:simulated` vacío. La causa es estructural (backend anvil con probe sin inventario) + flip operador pendiente (`SIM_BACKEND=revm` + `REVM_RPC_URL` ausente).

---

## 0. Respuesta directa al charter

> **¿Sigue sim-ctl en 501 pending?** NO. La era 501 terminó: sim-ctl booteó 2026-09-06T22:58:11Z con fork anvil conectado (block 25921444, `fork_ready=true`) y consumidor activo sobre `arbx:opps:validated`. Hoy responde simulaciones reales (anvil), no 501.

> **¿La simulación pasa de detección a label?** NO. El hallazgo HG "cero sims passed" PERSISTE hoy, 2026-09-06 23:30Z:
> - `simulations`: 1.000.718 filas, `passed=f` en el 100% (0 aprobadas en la historia completa).
> - `XLEN arbx:opps:simulated` = **0** (nada cruza al stream de salida).
> - `paper_trade_runs`: 598.878 filas, `actual_timestamp` NULL en todas (0 labels), `sim_attempts` max=0, congelada desde 2026-09-01 16:32.
> - El flujo SÍ corre: Prometheus reporta ~37.900 sims/24h y el gate G-SIM-1 está **green** — pero green mide FLUJO, no aprobaciones. 37,9K sims/24h que fallan 100% honestamente.

---

## 1. Evidencia por capa

### 1.1 LOCAL (repo en `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)`, branch `a6-cbprom-01`) — **MATCH**

SHA base: el clone local está en `086347f7` ("chore: update branch with main (post-#545 merge)"); el `main` local reflejado está stale en `28d48cdd` (#531) — PERO la familia de simulación está **idéntica** al main desplegado `d4d3ff63`:

```
$ git diff --stat d4d3ff63 -- backend/sim-ctl backend/sim-core backend/simulator-v2 backend/math-engine database/migrations/111_paper_trade_runs_calibration_eligibility.sql database/migrations/112_simulations_simulator_revm.sql database/migrations/113_simulations_revm_idempotency.sql
(vacío — sin drift en la superficie)
```

Cartografía del código (lectura directa, file:line):

| Pieza | Rol | Evidencia clave |
|---|---|---|
| `backend/sim-ctl/src/main.rs` | Servicio axum :3003; selección de backend | `"revm"` → `backend="revm-v2"` (exige `REDIS_URL`+`REVM_RPC_URL`); `"anvil"|""` → `backend="anvil"`. `SimulatorV2` solo se instancia si `SIM_BACKEND=revm`; `b2c_ctx` solo si (simulator, real_sim_env, redis) todos Some + `flashloan_executor_boot_ready()`. Drain-guard: "REVM selected → b2c_ctx MUST exist; ANVIL selected → fork MUST exist. Fail loud, consume nothing." |
| `backend/sim-ctl/src/consumer.rs` | XREADGROUP/XAUTOCLAIM `arbx:opps:validated` → grupo `sim-ctl-g0` → `arbx:opps:simulated` (MAXLEN 10.000) | b2c=None → `backend.simulate`; insert falla → `error!(event="sim_consumer.persist_err")` SIN ack (retry PEL); `sim.passed && inserted_fresh` → XADD salida. Ghost dead-letter si la fila de opportunities fue purgada (`opportunity_row_missing`). |
| `backend/sim-ctl/src/sim_engine.rs` | Path legacy anvil | L44: `UnsupportedStrategy` → `strategy_not_simulatable_in_s4`; sin fork → `anvil_fork_not_configured` (era del 501). |
| `backend/sim-ctl/src/tx_builder.rs` | build_probe | `from: signer_from` (SIM_SIGNER_ADDRESS) sobre fork — **sin deal/impersonate/set_balance** → revert estructural TRANSFER_FROM_FAILED/STF cuando el probe toca tokens. |
| `backend/sim-ctl/src/persistence.rs` | INSERT `simulations` | `ON CONFLICT (opportunity_id) WHERE simulator='revm' DO NOTHING` + `.context("insert simulation")?` — el label de contexto es lo ÚNICO que aparece en logs (el error PG subyacente queda tragado). |
| `backend/sim-ctl/src/capabilities.rs` | /capabilities | `dispatch_gate` lee `ARBX_USE_SIMULATOR_V2` EN TIEMPO DE REQUEST ("canonical consumer: searcher-rs"); `build.sha` via `option_env!` (null si no se inyecta al build). |
| `backend/recon/src/drift_tracker.rs` | ÚNICO escritor de `sim_attempts` | L33: "Feature-flagged OFF by default (`ARBX_DRIFT_TRACKER_MODE`)" → explica sim_attempts=0 (nunca corrió). |
| `backend/api-server/src/readiness/verifiers/g-sim-1.ts` | Gate G-SIM-1 | 3 capas: /health vivo; `ARBX_SIMULATOR_V2_READY=true` + checklist 7 claves frescas (30d estricto); Prometheus `sum(increase(arbx_simulation_total[24h]))>0`. **El green exige flujo, NO aprobaciones** (L49-56, L296-317). |
| `database/migrations/111/112/113` | Schema labels + idempotencia revm | 111: `paper_trade_runs.calibration_eligible/sim_attempts/actual_*`; 112: CHECK `simulator IN (...,'revm')`; 113: índice único parcial `simulations_revm_idempotency_uq WHERE simulator='revm'` (exactly-once publish). |
| `docker/compose.prod.yml` | Wiring | sim-ctl (:3003) con `ANVIL_URL`, `DATABASE_URL`, `REDIS_URL`, depends postgres+redis; math-engine (:3006) cómputo puro sin DB/Redis. **`REVM_RPC_URL` y `SIM_BACKEND` NO están cableados en NINGÚN docker/*.yml** — solo llegarían vía `env_file ../.env`. |

### 1.2 REMOTE_MAIN (GitHub) — **MATCH**

```
$ git ls-remote origin main
d4d3ff63...  refs/heads/main
```
La familia de simulación en main tip (`d4d3ff63`) es exactamente lo auditado en LOCAL (diff vacío, §1.1). Los PRs de la familia relevantes (#474/#475 labels+Capa B, #484 SIMWIRE, #542-#545) ya aterrizaron en main según el historial local (`git log main`).

### 1.3 VPS (`ssh arbx`, read-only) — **MATCH** (SHA verídico) con hallazgos funcionales

**SHA desplegado = main tip:**
```
$ ssh arbx git -C /opt/arbitragex-v2 rev-parse HEAD
d4d3ff63...   (== GitHub main)
```

**Contenedores y puertos (solo loopback, by design):**
```
arbitragex-v2-sim-ctl-1     127.0.0.1:3003->3003/tcp
arbitragex-v2-math-engine-1 127.0.0.1:3006->3006/tcp
arbitragex-v2-api-server-1  127.0.0.1:8080->8080/tcp
```

**sim-ctl logs (`docker logs arbitragex-v2-sim-ctl-1 --tail 100`):**
- Boot 2026-09-06T22:58:11Z: `sim.backend_selected backend="anvil"`, `fork.connected block=25921444`, `fork_ready=true`, consumer spawn con `b2c=false`.
- `/health` ok, uptime ~1897s al momento del chequeo. **501 ya no aparece.**
- Patrón observado en ventana de 27 min: ~6 `event="sim_consumer.persist_err" ... error="insert simulation"` (≈1% del flujo; FK violations por carreras con el purge de retención — el ghost dead-letter `opportunity_row_missing` del consumer hace el resto y SÍ funciona).
- `/capabilities` (vía curl 127.0.0.1:3003 en VPS): `build.sha=null`, `fork_suite=null`, `dispatch_gate.active=true` (ARBX_USE_SIMULATOR_V2=true — el gate del lado searcher-rs), backend real de consumo = **anvil**.

**Postgres (SELECT only, `docker exec postgres psql -U postgres -d arbitragex`):**
```
simulations: 1.000.718 filas | passed=t: 0 | passed=f: 1.000.718
  fail_reason top: strategy_not_simulatable_in_s4 = 539.611 (historia)
                anvil (TRANSFER_FROM_FAILED/STF/timeout)   = 461.110 (historia)
  Últimas 24h: strategy_not_simulatable_in_s4=37.309 | TRANSFER_FROM_FAILED=440
               sim_timeout=183 | STF=160 | build_error(router→catálogo: chain=1, dex=PancakeSwap V3=38)
               + un puñado de rpc_error (Alchemy 408 free-plan)
  Última hora: 927 not_implemented vs 156 anvil
paper_trade_runs: 598.878 filas | actual_timestamp NOT NULL: 0 (labels=0)
  sim_attempts: max=0 (drift_tracker OFF por flag, §1.1)
  MAX(created_at) = 2026-09-01 16:32 → congelada 5 días (0 filas/24h)
Esquema vivo: índice simulations_revm_idempotency_uq CONFIRMADO + CHECK simulator='revm' presente
  (migraciones 111/112/113 aplicadas — re-corren en cada deploy, sin ledger de estado).
```

**Redis (read-only):**
```
XLEN arbx:opps:simulated → 0
XINFO GROUPS arbx:opps:validated (grupo sim-ctl-g0): 49 consumers huérfanos
  (uno por boot — leak de IDs), lag 2.974 entries, last-delivered ~18 min
```

**math-engine (`docker logs --tail 50` + `/health`):** `{"ok":true,"operators":31}` — sano, 31 operadores cargados, log de una sola línea de boot (idle; cómputo puro, no persiste).

**Prometheus:** `sum(increase(arbx_simulation_total[24h]))` = 37.907,47 (2 series) — el flujo de simulación está VIVO.

**readiness_evidence (G-SIM-1):** checklist 7/7 `evidenced` (verificado 2026-08-17→08-22; fresco hasta ~16-sep).

**Gate G-SIM-1 vivo (curl interno api-server :8080):**
```
GET /api/v1/readiness → 19 items
G-SIM-1 => green | sim-ctl alive, simulator-v2 ready, 37853 simulations in last 24h
```
(implica `ARBX_SIMULATOR_V2_READY=true` + 7/7 fresco + flujo>0 — el gate mide FLUJO, no aprobaciones; ver §3 riesgo R-3).

**readiness decision (edge interno):** `verdict=NO_GO`, `phase=P2_READINESS`, `go_a4=true`, `go_a5=true`, capital_exposure_usd=0, paper_mode=true; blockers A.6 (partial, emisión Prometheus CB pendiente), A.7 (partial, call-site no-submit pendiente), A.9 (critical, sign-off formal pendiente). **G-SIM-1 NO es blocker.**

**Env del contenedor sim-ctl (NOMBRES solamente, RULE secreto):** `REVM_RPC_URL` **AUSENTE** del entorno del contenedor; presentes: `ARBX_USE_SIMULATOR_V2`, `DATABASE_URL`, `REDIS_URL`, `ANVIL_URL`, `SIM_SIGNER_ADDRESS`, `ARBITRAGE_EXECUTOR`, `EXECUTOR_1`, `FLASHLOAN_EXECUTOR_1`. `SIM_BACKEND` no aparece (default→anvil).

### 1.4 LIVE_DOMAIN — **N-A**

Superficie interna por diseño: sim-ctl (:3003) y math-engine (:3006) bind solo a `127.0.0.1` en el VPS (§1.3); no hay ruta pública propia que auditar. Presupuesto HTTP público: **0/5 requests usados**. El único reflejo público es el panel /readiness (captura local `postdeploy-readiness-de576965-2026-09-06.png` del operador: chips PAPER·TLS SHADOW verde, overall NO-GO — consistente con la consulta interna directa §1.3).

---

## 2. Drifts medidos (hallazgo central: la cadena corta en "passed")

| # | Drift | Medida | Evidencia |
|---|---|---|---|
| D-1 | **0 sims aprobadas en TODO el historial** | 1.000.718/1.000.718 `passed=f` | PG counters §1.3 — el hallazgo HG 0/10 persiste HOY |
| D-2 | Stream de salida vacío | `XLEN arbx:opps:simulated`=0 | Redis §1.3 — nada cruza detección→simulated |
| D-3 | **Labels inexistentes** | `actual_timestamp` NULL 598.878/598.878; `sim_attempts` max=0 | PG §1.3; drift_tracker OFF por flag (`ARBX_DRIFT_TRACKER_MODE`, recon/drift_tracker.rs:33) |
| D-4 | paper_trade_runs congelada | última fila 2026-09-01 16:32 (5 días, 0/24h) | PG §1.3 |
| D-5 | Path anvil NO puede aprobar estructuralmente | probe `from=SIM_SIGNER_ADDRESS` sin inventario en fork → TRANSFER_FROM_FAILED/STF garantizado (440+160/24h); logs anvil muestran evm_revert STF real del probe 0x1234… llamando exactInputSingle | tx_builder.rs + logs anvil |
| D-6 | Flip revm pendiente + env ausente | `SIM_BACKEND` unset (→anvil) y `REVM_RPC_URL` ausente del contenedor; además no cableado en ningún docker/*.yml | env names §1.3 + compose §1.1 |
| D-7 | Estrategias S4 no simulables dominan | `strategy_not_simulatable_in_s4`=37.309/24h (98% del fallo reciente) | PG §1.3 — el catálogo de build_probe no cubre las estrategias emitidas |
| D-8 | Error PG tragado en persist | `.context("insert simulation")` oculta la causa raíz (~6/27min FK-purge) | persistence.rs + logs |
| D-9 | Higiene del consumer group | 49 consumers huérfanos (1/boot), lag 2.974, PEL retrabajando | XINFO §1.3 |
| D-10 | Identidad de build opaca | `/capabilities build.sha=null`, `fork_suite=null` (option_env no inyectado) | capabilities.rs + curl |

No hay drift de CÓDIGO entre capas (local == GitHub main == VPS desplegado, todo `d4d3ff63`): el degradamiento es de CONFIGURACIÓN/FLIP, no de versión.

---

## 3. Riesgos

- **R-1 (P0)**: Sin labels ni sims aprobadas, la calibración real del sistema es imposible (Capa A del HG cert 0/10 sigue en 0). Cualquier métrica de Topological Yield esperado no tiene base empírica; paper-shadow corre "a ciegas" de feedback.
- **R-2 (P0)**: El flip a revm requiere `REVM_RPC_URL` que NO está presente ni cableado — si el operador flipea `SIM_BACKEND=revm` sin eso, el drain-guard hace fail-loud en boot (bien), pero el flujo 37,9K/24h se detiene mientras tanto (ventana de pérdida de cobertura de simulación).
- **R-3 (P1)**: **G-SIM-1 green es engañoso para el lector**: mide que las sims CORREN, no que APRUEBEN. Un panel green con 0 aprobaciones históricas puede inducir un GO erróneo (A.9) si nadie mira passed-rate.
- **R-4 (P1)**: `strategy_not_simulatable_in_s4` 98% del fallo reciente — con revm activo gran parte se re-distribuirá a fallos reales de ruta/TLS; si el catálogo de estrategias del b2c no cubre lo que emite el searcher, el flip mejora poco.
- **R-5 (P2)**: persist_err traga el error PG (FK purge race ~1%) — ante un cambio de esquema o retención, un fallo masivo de persistencia sería indistinguible de carreras benignas.
- **R-6 (P2)**: 49 consumers huérfanos + lag 2.974 — degradación lenta de observabilidad del grupo; XAUTOCLAIM recicla pero el lag reportado infla.
- **R-7 (P2)**: `build.sha=null` imposibilita la verificación deploy-veraz desde /capabilities (la doctrina exige SHA anclado; hoy solo `git rev-parse` en host lo prueba).

---

## 4. Propuestas (para nivel production-mainnet)

| What | Why | Priority | Effort | Gate |
|---|---|---|---|---|
| **Flip operador a simulación route-aware real**: setear `SIM_BACKEND=revm` + `REVM_RPC_URL` (RPC pago/dedicado, no free-plan Alchemy que ya da 408) en `.env` del VPS, añadir ambas vars a `docker/compose.prod.yml` (sección sim-ctl environment) y redeploy sim-ctl con R3 (cache-busting + `--env-file`) | Es el único path que aprueba sin inventario pre-fondeado (TLS step 0 = Temporal Liquidity Superposition en simulator-v2/revm 42.0.1). Corta D-1/D-2/D-5 de raíz. Pre-checks ya verdes: migraciones 112/113 aplicadas, checklist G-SIM-1 7/7 fresco, REDIS_URL/ARBITRAGE_EXECUTOR/FLASHLOAN_EXECUTOR_1 presentes, drain-guard protege boot | **P0** | Bajo (env+compose+redeploy; el binario ya contiene simulator-v2) | §34.3 flips = operador-only con autorización explícita; `arbx-simulation-mandatory` + `arbx-paper-trade-first` PASS previos; verificación post-flip: primer `passed=t` en `simulations` + `XLEN arbx:opps:simulated` > 0 |
| **Cobertura de estrategias del path b2c**: auditar qué `strategy_kind` emite el searcher vs las que el route-aware simulator-v2 resuelve; cerrar el gap de `strategy_not_simulatable_in_s4` (37,3K/24h) | El flip revm solo ayuda a lo que el b2c resuelve; hoy el 98% del flujo ni entra a simulación | **P1** | Medio | `arbx-simulation-mandatory`; test con fixture real de cada strategy_kind (generated-table+probe pattern del repo) |
| **G-SIM-1 capta passed-rate**: añadir al verifier (o gate hermano G-SIM-2) la condición `passed>0` reciente (p.ej. `sum(increase(arbx_simulation_passed_total[24h]))>0` o query a simulations), para que green signifique "simula Y aprueba" | R-3: green de flujo con 0 aprobaciones es una señal tramposa para el GO/NO-GO A.9 | **P1** | Bajo | P-∅ (PR con ID de anomalía); test del verifier con counter passed=0 |
| **Log de error completo en persist**: reemplazar `.context("insert simulation")` por cadena con fuente (`{:source}` de anyhow) en consumer.rs/persistence.rs | D-8/R-5: hoy un fallo masivo de INSERT es indistinguible del race benigno FK-purge | **P1** | Bajo | P-∅; unit test del mensaje de error |
| **Anclar `build.sha` en el build Docker** (ARG/ENV `SIM_CTL_BUILD_SHA` → `option_env!`) y exponerlo en /capabilities | R-7: deploy-veraz comprobable desde el servicio mismo, doctrina §37 G4 | **P2** | Bajo | P-∅; verify-deploy L2 lo consume |
| **Higiene del consumer group**: DELCONSUMER de huérfanos en boot (o GC periódico) + alerta si lag crece sostenido | D-9/R-6: 49 huérfanos hoy, crece 1/boot | **P2** | Bajo | — |
| **Activar drift_tracker** (`ARBX_DRIFT_TRACKER_MODE`) para poblar `sim_attempts` una vez existan labels revm | D-3 segunda mitad: labels sin attempts no permiten medir convergencia post-calibración | **P2** | Bajo | operador flip; `arbx-risk-limits-enforcement` |

---

## 5. Notas de método

- Todos los comandos fueron read-only (docker ps/logs/port/inspect, psql SELECT, redis XLEN/XINFO, curl 127.0.0.1 dentro del VPS, git read-only). Cero mutación. 0/5 requests HTTP públicos usados.
- Fail-honest R8: "no verificado" ≠ "no existe" — no leí VALORES de secretos (solo nombres de variables); el valor exacto de `ARBX_SIMULATOR_V2_READY` se infiere del comportamiento del verifier (green exige =true), no de lectura directa.
- Lexicón OMEGA aplicado (TLS, Topological Yield, Asimetría Topológica) sin oscurecer la evidencia.
