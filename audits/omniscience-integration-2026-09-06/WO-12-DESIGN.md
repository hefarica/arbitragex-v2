# WO-12 — DECISIÓN DOCUMENTADA: backend de simulación declarado (REVM 42 in-process vs Anvil-fork)

- **WO:** WO-12 · kind: design (READ-ONLY — CERO edición de código de producción, CERO git)
- **Agente:** ecc:code-architect (Gang Omniscience, IA OMEGA) · 2026-09-06
- **Charter:** decidir entre (a) upgrade de ruta REVM in-process como backend declarado, o (b) Anvil-fork como backend declarado eliminando el claim de fidelidad REVM — con ventajas/riesgos medidos y gates (`arbx-simulation-mandatory`). Todo reclamo stale del informe §2.6 se declara CORREGIDO con evidencia.
- **Archivos bajo claim (solo lectura + diffs propuestos):** `backend/Cargo.toml` · `backend/Cargo.lock` · `docker/compose.prod.yml`
- **Presupuesto:** 0/5 requests HTTP públicos usados (los curls fueron loopback 127.0.0.1 dentro del VPS vía SSH). 3 comandos SSH read-only. CERO git. CERO mutación VPS (§32/§33).

---

## 0. RESOLUCIÓN EJECUTIVA

**DECISIÓN: camino (a) — REVM in-process como backend declarado del Tier 1/2 (hot-path de simulación), con Anvil-fork retenido exclusivamente para Tier 3 (replay adversarial del control-plane).**

No es un "upgrade de ruta": la ruta REVM in-process **ya existe completa, compilada y desplegada en el binario de producción**. Lo que falta es un *flip de selección* (configuración), propiedad del operador (§34.3-disciplina, roadmap P1-3). El camino (b) fue analizado y **RECHAZADO**: no corrige por sí mismo el rechazo estructural del probe anvil, contradice la expectativa de infraestructura de `arbx-simulation-mandatory` ("el hot-path usa `revm` o EVM in-process equivalente para sims Tier 1 y 2, sub-milisegundo"), y dejaría flotante la imagen `foundry:latest` (EVM sin versión anclable).

El hallazgo semilla del informe ("REVM 3.5 pinneado") es **STALE y queda CORREGIDO** (§1.1): el pin real es **revm 42.0.1**, probado en árbol, lockfile y binario vivo.

---

## 1. CORRECCIONES FAIL-HONEST AL INFORME §2.6

### 1.1 Claim "REVM 3.5 pinneado (línea vieja)" → **CORREGIDO: FALSO en el árbol actual**

| Afirmación del informe | Realidad verificada | Evidencia (file:line) |
|---|---|---|
| "REVM 3.5 pinneado" | Pin de workspace = **revm mayor 42**: `revm = { version = "42", default-features = false, features = ["std"] }` | `backend/Cargo.toml:35` |
| (versión resuelta) | **revm 42.0.1** en lock; revm-primitives 42.0.0; revm-interpreter 42.0.0 | `backend/Cargo.lock:6194-6195` (revm), `:6325-6326` (interpreter), `:6362-6363` (primitives) |
| (alloy) | alloy 1.8.3 (`Cargo.lock:74-75`), alloy-primitives 1.6.1 (`:399-400`) — la unificación G-SIM-1 (#443, `5f850f90`) aterrizada | `backend/Cargo.toml:70-72` ("alloy 1.0 — workspace canonical") |
| (prueba en el binario de PRODUCCIÓN) | `/capabilities` del contenedor vivo reporta `revm_version: "42.0.1"`, `alloy_primitives_version: "1.6.1"`, `simulator_backend: "v2"`, módulos `["bellman_ford","lazy_db","revm_runner","sequence_runner"]` | curl loopback VPS `127.0.0.1:3003/capabilities` (2026-09-06, generación boot 23:45:32Z); derivación por `build.rs` que escanea `../Cargo.lock` (`sim-ctl/src/capabilities.rs:43-48`) |
| (por qué el informe leyó "3.5") | Existe un **comentario stale** — no un pin — que dice "consistent with revm 3.5 / alloy-primitives 0.7 / ethers 2 in use today". El informe citó documentación podrida como si fuera el constraint | `backend/simulator-v2/Cargo.toml:10` |

**Raíz del error del informe:** comentario pre-G-SIM-1 en `simulator-v2/Cargo.toml:10`. Corrección propuesta en §5.3 (diff documental, marcado `WO-12`). Ídem el header de `sim-ctl/src/revm_backend.rs:3-9` ("Sprint 4, Tasks 4.2 + 4.3 pending… `SimError::NotImplemented`") — stale frente a `simulator-v2/src/lib.rs:96-100` ("`NotImplemented`… will never be constructed by `SimulatorV2::simulate()` after Tasks 4.2/4.3") y a la implementación real `execute_multistep_revm` (`sim-core/src/sim_multistep.rs:545`, orquestador multi-paso sobre `CacheDB<LazyDb>` resuelto desde `simulator.rpc_url`, `:508-510`).

### 1.2 Claim "producción corre Anvil fork, no REVM in-process" → **CONFIRMADO, con precisión mayor a la del informe**

Verificación propia (read-only, generación actual del contenedor, boot `2026-09-06T23:45:32Z`):

```
docker inspect arbitragex-v2-sim-ctl-1 | grep ^SIM_BACKEND=   → SIM_BACKEND=anvil   (explícito)
docker inspect … | env names                                   → REVM_RPC_URL AUSENTE
docker logs … | boot                                           → {"event":"sim.backend_selected","backend":"anvil"}
                                                                → {"event":"sim_consumer.started","b2c":false}
docker inspect arbitragex-v2-anvil-1                           → ghcr.io/foundry-rs/foundry:latest, running
```

- **Sí corre Anvil-fork** (`ANVIL_URL: http://anvil:8545` — `docker/compose.prod.yml:157`; servicio `anvil` L119-144 con `--fork-url $$ANVIL_FORK_URL` L127).
- **PERO** "no REVM in-process" es una media verdad: el binario contiene y reporta el path REVM (`/capabilities` §1.1); lo ausente es la **selección** (`SIM_BACKEND=revm`) y su endpoint (`REVM_RPC_URL`). La distinción importa: no hay código por escribir — hay configuración por flappear.
- Delta vs N5 (23:30Z): entonces `SIM_BACKEND` estaba **ausente** (default→anvil); hoy está **explícito en `anvil`** (el operador lo pineó en `.env` del VPS entre ambas lecturas). `env_file: ../.env` (`compose.prod.yml:150-151`) ya propaga cualquier var del `.env` — el wiring compose propuesto en §5.1 es contrato-documental, no prerrequisito funcional.

### 1.3 Implicación del informe ("haría falta upgrade de ruta REVM") → **FALSA**

La ruta está completa en main: trait `SimulatorBackend` (`sim-ctl/src/simulator_backend.rs:35-43`), `RevmBackend` con gas real desde Redis (`revm_backend.rs:46-68`, CRITICAL #2 net-of-gas), selección por `SIM_BACKEND` (`main.rs:655-686`), pipeline B2c route-aware con `execute_multistep_revm` (`main.rs:723-814`), drain-guard fail-loud (`main.rs:845-874`; regla: "REVM selected → b2c_ctx MUST exist; ANVIL selected → fork MUST exist. Fail loud, consume nothing."), migraciones 112/113 vivas en PG (N5 §1.3: CHECK `simulator IN (…,'revm')` + índice único parcial `simulations_revm_idempotency_uq`).

---

## 2. ESTADO REAL VERIFICADO (base de la decisión)

| Dimensión | Hecho | Evidencia |
|---|---|---|
| Pin/capacidad | revm 42.0.1 + alloy-primitives 1.6.1 en el binario prod | §1.1 |
| Selección runtime | `SIM_BACKEND=anvil` explícito; `b2c=false`; path legacy `SimEngine` (snapshot→eth_call→revert contra fork) | §1.2; `anvil_backend.rs:13-14` |
| Env del contenedor (nombres) | Presentes: `ANVIL_URL`, `ARBITRAGE_EXECUTOR`, `ARBX_USE_SIMULATOR_V2`, `FLASHLOAN_EXECUTOR_1`, `REDIS_URL`, `SIM_SIGNER_ADDRESS`, `SIM_BACKEND`. Ausente: `REVM_RPC_URL` | docker inspect (§1.2) — **el 75% del env B2c ya está**; faltan 2 vars |
| Resultado del path anvil | 1.000.718 sims, `passed=f` al 100%; causas anvil: TRANSFER_FROM_FAILED=440/24h, STF=160/24h (histórico 461.110); `strategy_not_simulatable_in_s4`=37.309/24h | 05-simulator-family.md §1.3 (N5, medido 23:30Z) |
| Causa estructural del 0-passed en anvil | El probe se firma `from: SIM_SIGNER_ADDRESS` **sin deal/impersonate/inventario** en el fork → TRANSFER_FROM_FAILED garantizado al tocar tokens | `sim-ctl/src/tx_builder.rs` (build_probe); N5 D-5 |
| Imagen anvil | `ghcr.io/foundry-rs/foundry:latest` — **flotante**, EVM sin versión anclable | `compose.prod.yml:120` + docker inspect |
| Verdad de capacidades | `/capabilities` reporta lo que el binario CONTIENE (linkage), no el switch runtime — honesto por diseño | `capabilities.rs:36-38` |
| Matiz de verdad pendiente | `dispatch_gate.active=true` reporta que `ARBX_USE_SIMULATOR_V2` está seteado, pero searcher-rs lee esa var y la **descarta** ("no runtime flip yet") — el consumidor canónico del sim real es sim-ctl vía `SIM_BACKEND`, no searcher-rs | `searcher-rs/src/main.rs:462-465`; `capabilities.rs:49-54` |
| Latencia comparada anvil vs revm | **NO INSTRUMENTADA** en este repo (declarado R8; la expectativa "sub-milisegundo" Tier 1/2 es doctrinal del skill, no medida aquí) | `arbx-simulation-mandatory` §"Expectativas de infra"; gap WO-10-familia |

---

## 3. CAMINO (a) — REVM IN-PROCESS COMO BACKEND DECLARADO · **RECOMENDADO**

### 3.1 Qué es declarativamente

El switch Tier 1/2 pasa a `SIM_BACKEND=revm`: el consumidor de `arbx:opps:validated` simula vía pipeline B2c route-aware (`route_lookup::fetch_candidate_inputs` → `execute_multistep_revm`, `main.rs:785-799`) contra `CacheDB<LazyDb>` pineado a bloque, con TLS (Temporal Liquidity Superposition) como step 0 — **sin inventario pre-fondeado**, gas cobrado dentro de revm desde `gas_price_wei` vivo de Redis (`revm_backend.rs:83-120`), Topological Yield neto-de-gas. Anvil NO se elimina: retiene Tier 3 (replay adversarial con inyección de competencia y state drift, `arbx-simulation-mandatory` Tier 3).

### 3.2 Ventajas (medidas/doctrinales)

1. **Doctrina:** es exactamente la expectativa de infraestructura del skill obligatorio ("hot-path usa `revm` o EVM in-process equivalente para sims Tier 1 y 2 (sub-milisegundo)… control-plane orquesta sims Tier 3 vía subproceso anvil"). El camino (a) no inventa arquitectura: la alinea.
2. **Corta el rechazo estructural:** elimina la clase TRANSFER_FROM_FAILED/STF (600+/24h actuales) porque el step 0 es TLS sobre bytecode forked real (`sim_multistep.rs:17-30,53`), no un probe firmado sin inventario. Es el único path que aprueba sin fondeo previo (N5 §4 P0).
3. **Versión anclada:** revm 42.0.1 pineado en lock (auditable, reproducible) vs `foundry:latest` flotante (drift invisible, clase §37 G4 — hoy sin build-SHA que lo delate, `build.sha=null`).
4. **Sin hop inter-proceso por sim:** ejecución in-process con `CacheDB` persistente entre pasos de la secuencia round-trip (`simulator-v2/src/lib.rs:23-25`) vs snapshot→eth_call→revert por HTTP contra otro contenedor.
5. **Experiencia migratoria ya pagada:** migraciones 112/113 vivas (idempotencia exactly-once), drain-guard compilado, 75% del env B2c ya presente (§2).
6. **Desbloquea el funnel completo de labels → calibración** (roadmap P1-3/P1-4): primer `passed=t` → `XLEN arbx:opps:simulated`>0 → primer `relay_sim.no_submit` → labels → writer de calibración (WO-07).

### 3.3 Riesgos (medidos) y mitigaciones

| # | Riesgo | Medida/Mitigación |
|---|---|---|
| A-1 | `REVM_RPC_URL` apunte al pool degradado (5/6 breakers OPEN, D-14; Alchemy free-plan ya da 408/429) | **RPC DEDICADO de pago obligatorio** (gate G-2). `LazyDb` fetchea estado on-demand por RPC — un RPC degradado degrada el sim |
| A-2 | Ventana de recreate del servicio sim-ctl durante el flip (flujo ~37,9K sims/24h en pausa minutos) | `up -d sim-ctl` es service-scoped (no flota completa); el drain-guard garantiza que si el env queda incompleto el consumidor NO arra el stream validado a rejections vacíos (`main.rs:845-874`) — fail-loud, consume nothing |
| A-3 | Camino post-flip `consumer→persistence→paper_trade_runs` jamás ejercitado en runtime (grupo `relays-client-g0` con `last-delivered-id 0-0`) | Gate G-6 data-layer ANTES del flip (P1-4): consumer verificado + alerta `lag > length` en TODOS los `arbx:opps:*` |
| A-4 | El flip no arregla la cobertura: `strategy_not_simulatable_in_s4` = 98% del fallo reciente no entra a simulación | Declarado: P1-5 queda abierto (cobertura de `strategy_kind` b2c). El flip es necesario, no suficiente |
| A-5 | G-SIM-1 seguirá green midiendo FLUJO con 0 aprobaciones | P1-6: reescribir el verifier a passed-rate 24h ANTES de confiar en el green post-flip |
| A-6 | Latencia real del path revm sin benchmark propio | Declarado (R8): no fabricar cifras; instrumentar post-flip (familia WO-10) |

---

## 4. CAMINO (b) — ANVIL-FORK COMO BACKEND DECLARADO (eliminar claim REVM) · **ANALIZADO Y RECHAZADO**

### 4.1 Qué exigiría (no es gratis ni mera documentación)

1. **Reparar el rechazo estructural del probe** (si no, "declarar anvil" es declarar un embudo que aprueba 0/1.000.718): provisionar inventario del signer en el fork por sim (`anvil_deal`/`anvil_setBalance`/`anvil_impersonateAccount`) → N round-trips RPC extra por simulación, mutación de estado del fork compartido por todo el servicio, y semántica de "fork ensuciado" entre sims (snapshot/revert ya existe pero acoplado al HTTP hop).
2. **Anclar la imagen:** `ghcr.io/foundry-rs/foundry:latest` → digest/tag fijo (propietario del drift: hoy el EVM cambia silenciosamente en cada pull).
3. **Reescribir la verdad declarada:** `/capabilities` (el linkage `simulator_backend:"v2"` pasaría a ser capability muerta reportada como viva — violación del espíritu R8), verifier G-SIM-1 (`ARBX_SIMULATOR_V2_READY`), roadmap §4 ventaja (3), y simulator-v2/sim-core se convierten en cargo inerte compilado sin consumidor runtime.
4. **Aceptar el hop HTTP por paso** en secuencias multi-paso (cada paso del round-trip forward→read→backward viaja por JSON-RPC).

### 4.2 Ventajas honestas de (b)

- Un solo motor EVM para Tier 1/2/3 (menos superficie de sincronización de estados).
- Anvil ES internamente revm (motor foundry): fidelidad a nivel bytecode comparable — el claim eliminado sería el de *versión anclada + in-process*, no el de "EVM revm".
- Debugging con tooling RPC estándar; ops ya probada (fork conectado al bloque 25921444 en el boot actual).

### 4.3 Por qué se rechaza

1. Contradice la expectativa explícita de `arbx-simulation-mandatory` para el hot-path (in-process Tier 1/2).
2. No resuelve el 0-passed por sí mismo: exige el trabajo de fork-state provisioning (ítem 4.1.1) que es MÁS invasivo que el flip (a) que ya está construido, migrado y dren-guardado.
3. `foundry:latest` flotante sin identidad de build (`build.sha=null`) = EVM sin versión verificable en la frontera de evaluación de capital — inaceptable para §37/G4 y para la pregunta canónica §34.4 ("¿funcionaría con capital real?" — con un EVM que cambia solo, no se puede responder).
4. Estranda la inversión viva: binario prod ya linkea revm 42.0.1 y las migraciones 112/113 ya corren en PG.

**Sinergia retenida de (b):** el ítem 4.1.2 (anclar la imagen anvil) se adopta COMO HIGIENE dentro del camino (a) — Anvil sigue vivo para Tier 3 y también merece versión anclada (§5.2).

---

## 5. DIFFS EXACTOS PROPUESTOS (NO APLICADOS — diseño read-only)

### 5.1 `docker/compose.prod.yml` — contrato del backend declarado (claim file propio)

Contexto actual exacto (L152-157, servicio `sim-ctl`):

```yaml
    environment:
      ARBX_CONFIG_PATH: /app/configs/app.toml
      SIM_PORT: '3003'
      DATABASE_URL: ${DATABASE_URL:?DATABASE_URL required}
      REDIS_URL: redis://redis:6379
      ANVIL_URL: http://anvil:8545
```

Diff propuesto:

```diff
       DATABASE_URL: ${DATABASE_URL:?DATABASE_URL required}
       REDIS_URL: redis://redis:6379
       ANVIL_URL: http://anvil:8545
+      # WO-12 (2026-09-06) — backend de simulación declarado en el contrato compose.
+      # Pass-through: la fuente de verdad del VALOR sigue siendo el .env del VPS
+      # (flip = operador, disciplina §34.3/Roadmap P1-3). Default explícito 'anvil'
+      # = fail-closed al estado actual. Valores válidos: 'anvil' | 'revm'
+      # (sim-ctl/src/main.rs:655-686). Nota: env_file ../.env ya propaga esta var —
+      # este entry hace el contrato VISIBLE y auditable en el compose.
+      SIM_BACKEND: ${SIM_BACKEND:-anvil}
+      # WO-12 (2026-09-06) — endpoint del fork para LazyDb/CacheDB (path revm).
+      # Inocuo en modo anvil (no leído); en modo revm, su ausencia dispara el
+      # drain-guard fail-loud (main.rs:845-874) — nunca fall-through silencioso.
+      # Contiene API key de RPC de pago → el VALOR vive SOLO en el .env del VPS,
+      # jamás en-repo (§33.2; arbx-no-hardcode-doctrine).
+      REVM_RPC_URL: ${REVM_RPC_URL:-}
```

### 5.2 Higiene de imagen anvil (claim file propio; sinergia adoptada de (b))

```diff
   anvil:
-    image: ghcr.io/foundry-rs/foundry:latest
+    # WO-12 (2026-09-06) — imagen ANCLADA por digest/tag: el motor EVM del Tier 3
+    # deja de mutar silenciosamente en cada pull (foundry:latest flotante). El digest
+    # exacto lo fija el operador con `docker images --digests` sobre la imagen que
+    # corre HOY (verificada viva desde 2026-09-06T23:45:26Z) — no se inventa aquí.
+    image: ghcr.io/foundry-rs/foundry:<digest-operador>
```

### 5.3 Corrección documental de las líneas stale que engañaron al informe (fuera de claim propio — se entrega como diff propuesto para el PR del WO; cero cambio de comportamiento)

```diff
--- backend/simulator-v2/Cargo.toml
-# Workspace deps (consistent with revm 3.5 / alloy-primitives 0.7 / ethers 2 in use today).
+# Workspace deps — resolved by the workspace lock: revm 42.0.1 / alloy-primitives 1.6.1
+# (unified by G-SIM-1 #443 5f850f90), surfaced live by sim-ctl /capabilities
+# (build.revm_version scans ../Cargo.lock in build.rs).
+# WO-12 (2026-09-06): comment corrected — the stale "revm 3.5" line misled the
+# external report into claiming an outdated PIN where only an outdated COMMENT existed.
```

```diff
--- backend/sim-ctl/src/revm_backend.rs  (header doc, lines 3-9)
-//! `SimulatorV2::simulate` deliberately returns `SimError::NotImplemented` until
-//! the `lazy_db` (Task 4.2) and `revm_runner` (Task 4.3) sub-modules land.
+//! WO-12 (2026-09-06): header corrected — Tasks 4.2/4.3 LANDED (lazy_db,
+//! revm_runner, sequence_runner in simulator-v2; execute_multistep_revm in
+//! sim-core). `NotImplemented` is kept only for variant-compat and "will never
+//! be constructed" (simulator-v2/src/lib.rs:96-100).
```

### 5.4 Acciones del OPERADOR en VPS (los agentes NO mutamos VPS — §32/§33; runbook)

```bash
# 1) .env del VPS (/opt/arbitragex-v2/.env) — flip §34.3-disciplina:
#    SIM_BACKEND=revm
#    REVM_RPC_URL=<RPC DEDICADO de pago>   # PROHIBIDO el pool degradado (5/6 breakers)
# 2) Recreate SOLO del servicio (env runtime; NO requiere rebuild — el binario ya
#    contiene simulator-v2/revm 42.0.1, prueba /capabilities):
docker compose --env-file .env -f docker/compose.prod.yml up -d sim-ctl
# 3) Rollback documentado (revert del flip):
#    SIM_BACKEND=anvil en .env + mismo `up -d sim-ctl`. El drain-guard garantiza
#    que ningún estado intermedio drena el stream validado silenciosamente.
```

---

## 6. INVARIANTE

- **INV-1 (§33.3):** a través del flip, `XLEN arbx:opps:detected` delta = 0 — el flip toca selección de simulación, no detección.
- **INV-2 (drain-guard, YA en código, `main.rs:845-874`):** `SIM_BACKEND=revm` con env B2c incompleto (`REVM_RPC_URL`/`ARBITRAGE_EXECUTOR`/`REDIS_URL`/`FLASHLOAN_EXECUTOR_1`) → consumidor NO spawneado + `sim_consumer.refused_drain_guard` — jamás drena `arbx:opps:validated` hacia rejections de calldata vacío. Configuración parcial = fail-loud, consume nothing.
- **INV-3 (exactly-once):** filas `simulator='revm'` dedup por índice único parcial `simulations_revm_idempotency_uq` (migración 113, viva en PG) + `ON CONFLICT … DO NOTHING` (`sim-ctl/src/persistence.rs`).
- **INV-4 (R8 fail-honest):** ambos backends devuelven SIEMPRE `SimulationResult` poblado — `passed=false` + `fail_reason` — nunca datos fabricados (`simulator_backend.rs:11-13`).
- **INV-5:** kill-switch verificado por el consumidor antes de procesar (`main.rs:887-890`) — el flip no lo toca; capital expuesto permanece 0 (PAPER_SHADOW, terminus intacto).

## 7. GATES (camino (a), en orden)

1. **G-1 · Operador:** flip ejecutado por el operador con autorización explícita documentada (roadmap P1-3; §34.3-disciplina). NO es flip de capital (capital=0, `MainnetRefused`/default-deny INTOCABLES) — pero VPS y `.env` son del operador (§32/§33).
2. **G-2 · RPC dedicado de pago** para `REVM_RPC_URL` (no free-plan 408; no pool con 5/6 breakers — D-14).
3. **G-3 · Pre-checks ya verdes:** migraciones 112/113 vivas; `ARBITRAGE_EXECUTOR`+`FLASHLOAN_EXECUTOR_1`+`REDIS_URL` presentes en el contenedor (§2); drain-guard compilado.
4. **G-4 · `arbx-simulation-mandatory` PASS** con mapeo de tiers: Tier 1/2 → revm in-process; Tier 3 → anvil fork (replay + inyección adversarial); Tier 4 → CI/job. El knob del trade-off es frescura-de-fork vs latencia, nunca simulación vs no-simulación.
5. **G-5 · `arbx-paper-trade-first` PASS** (PAPER_SHADOW invariante).
6. **G-6 · Data-layer bloqueante (P1-4) ANTES del flip:** consumer verificado sobre `arbx:opps:simulated` + alerta `lag > length` en TODOS los `arbx:opps:*`.
7. **G-7 · Post-flip medible:** primer `passed=t` en `simulations` · `XLEN arbx:opps:simulated` > 0 · `lag < length` sostenido 24 h · primer evento `relay_sim.no_submit` procesado en relays-client · G-SIM-1 reescrito a passed-rate (P1-6) antes de leer el green.
8. **G-8 · P-∅:** los diffs de §5 aterrizan por PR con ID (WO-12) — CERO commit/push/PR desde esta sesión (protocolo operador 2026-08-23).

## 8. VERIFICACIÓN DE ESTE DISEÑO (ya ejecutada, read-only)

- Árbol: lectura directa `backend/Cargo.toml`, `backend/Cargo.lock` (grep versiones), `docker/compose.prod.yml`, `sim-ctl/*`, `simulator-v2/*`, `sim-core/*`, `searcher-rs/src/main.rs`.
- Runtime: `docker inspect` sim-ctl/anvil (env NOMBRES + toggle no-secreto `SIM_BACKEND=anvil`), `docker logs` boot line, `curl 127.0.0.1:3003/capabilities` (loopback VPS) → `revm_version 42.0.1` en el binario vivo.
- Skill `arbx-simulation-mandatory` cargada y citada literal en §3.2.1/§4.3.1.
- CERO git, CERO edición de producción, CERO mutación VPS, 0/5 requests HTTP públicos.

## 9. HUECOS DECLARADOS (R8)

- Latencia comparada anvil↔revm: no instrumentada en este repo (§2, A-6) — no se fabrican milisegundos.
- El digest exacto de la imagen anvil a anclar (§5.2) lo determina el operador en el VPS (`docker images --digests`); no se inventa desde local.
- `dispatch_gate.active=true` (capabilities) reporta env-set, no dispatch runtime del searcher (`searcher-rs/src/main.rs:465` descarta la var). Verdad-telling a enderezar en el PR del WO o en WO propio (no bloquea esta decisión: el consumidor canónico del sim real es sim-ctl vía `SIM_BACKEND`).
- Cobertura b2c por `strategy_kind` (98% `strategy_not_simulatable_in_s4`): fuera del alcance de WO-12 — P1-5 abierto; el flip es necesario, no suficiente.
- No verificado: comportamiento de `LazyDb` bajo el RPC de pago específico que elija el operador (cold-cache vs warm) — medir post-flip.

## 10. ESTADO DEL BOARD

WO-12: **DISEÑADO** — decisión (a) REVM in-process declarado + Anvil Tier 3 retenido; hallazgo semilla §2.6 CORREGIDO con evidencia árbol+lock+binario vivo (§1); diffs exactos propuestos sin aplicar (§5); invariante (§6) y gates (§7) emitidos. La ejecución del flip es acción del operador (§5.4); los diffs de §5 aterrizan por PR con ID (G-8).

---

*Lexicón OMEGA: TLS (Temporal Liquidity Superposition) · Topological Yield · Variedad de Liquidez · Decoherencia de Estado. Fail-honest R8 en todo: "no computado" se declara, jamás se inventa.*
