# G7-KILLSWITCH-DRILL-01 — Drill trip/untrip del kill-switch: EJECUTADO

**Orden:** t56 (kind: work) · **Ejecutor:** Reviewer — independiente del autor del checklist (no escribí `pre_execute_checklist.rs`) · **Fecha de ejecución:** 2026-10-06 03:33:05Z → 03:35:21Z
**Base medida:** `main` = **`c89d21a3`** (clon aislado) · **deploy vivo observado** = **`3f00b359`** (`deploy.sha` de `https://edge-arbx.ape-tv.net/status`, run `37399887059`)
**Host:** VPS `195.201.235.70` (= `/opt/arbitragex-v2`, 25 contenedores) · **Postura medida:** `ARBX_TRADE_MODE=paper`, `ARBX_LIVE_EXEC_ENABLED=False`, `ARBX_LIVE_EXEC_CHAINS=11155111`
**Alcance escrito:** `docs/security/` (este archivo, vía rama + PR; **no mergeé**). No firmé, no emití, no moví capital.

---

## 0. VEREDICTO

**El drill se EJECUTÓ, no se describe.** G7 pedía `drill trip/untrip NO documentado`; queda **ejecutado y documentado con artefactos reproducibles**, en producción y en el stack de CI.

| Criterio del contrato | Resultado |
|---|---|
| El drill se EJECUTA (trip detiene / untrip reanuda), con comandos y salida | **EJECUTADO** — 2 ciclos trip/untrip en el VPS; detención y reanudación **observadas** en 2 consumidores independientes (`selector-api`, `sim-ctl`), con latencias de 0,7–1,0 s y ventanas armadas de **55 s** y **20 s** |
| Se cita `archivo:línea` del checklist real y se confirma que **check 1 = kill-switch** | **CONFIRMADO** — `backend/shared-rs/src/pre_execute_checklist.rs:213-214` (primer check de 12), semántica en `:260-279` |
| Se declara el alcance: qué detiene y qué NO | **DECLARADO Y MEDIDO** — §5: detiene consumo (`selector-api`, `sim-ctl`) y el gate de broadcast; **no** detiene descubrimiento (`searcher-rs`), ni el plano de lectura (`api-server`/`edge`/`frontend` 200 con el switch armado) |
| El estado es OBSERVABLE desde fuera | **DEMOSTRADO** — `GET https://edge-arbx.ape-tv.net/status` → `killswitch.enabled=true` **durante** la ventana armada (leído desde una estación externa), y `enabled=false` después |
| No mueve P/N (0/115), no firma, no emite, no toca capital, no altera el producto | **DECLARADO + MEDIDO** — §8: `capital_exposure_usd=0`, `live_trading=false`, `submit_enabled=false`, `paper_mode=true` tras el drill; los 25 contenedores siguen `healthy` |

**Efecto colateral declarado (no oculto):** el drill escribió 4 filas en `audit_log` (`killswitch.armed` ×2, `killswitch.disabled` ×2) y dejó el estado del switch con `reason`/`triggered_by` del drill en vez del registro anterior (`VER`/`admin`/2026-09-26). **No restauré el JSON viejo a propósito: reescribirlo sería falsificar el registro de auditoría.** Postura final = misma que al empezar (desarmado, sistema operativo). Detalle en §7.

---

## 1. Lo que el gate pide, textual (no parafraseado)

`audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md:13`:

> | G7 | Risk-limits + checklist pre-ejecución | ⚠️ CÓDIGO OK / DRILL FALTA | pre_execute_checklist.rs presente (check 1 = kill-switch); drill trip/untrip NO documentado |

Criterio macro en `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md:59-61`:

> **GATE-7: A.9-REINFORCED**
> - Evidence: A.9 checklist (G7) with verified reproducible artifacts (§34.5.3)
> - Requirement: Max notional, max loss, min profit, slippage, gas ceiling, bribe ceiling, expiry blocks, RPC quorum, stale quote protection, reorg protection ALL configured and tested

Este documento ataca **el único ítem que el gate declara faltante**: el drill. El estándar de evidencia que aplico es §34.5.3 — artefactos reproducibles, no claims.

---

## 2. Check 1 = kill-switch (archivo:línea, verificado por lectura del código real)

El checklist pre-ejecución vive en `backend/shared-rs/src/pre_execute_checklist.rs` y corre **12 checks en orden, devolviendo el primer fallo** (`:206-207`):

| Ancla | Contenido |
|---|---|
| `:212` | `pub async fn pre_execute_checklist(ctx: &mut PreExecuteContext<'_>) -> Result<(), ChecklistError>` |
| **`:213-214`** | **`// 1. Kill switch — cheapest abort, no DB round-trip.` → `check_kill_switch(ctx.redis).await?;`** — **es el check 1**, el primero de los 12 |
| `:216-251` | checks 2..12 (paper mode, chain active, trading config, RPC health, gas fresco, net profit, allowlist, factories, slippage, mempool, circuit breaker) |
| `:260-262` | `/// Check 1: Kill switch — reads arbx:killswitch JSON from Redis.` · clave ausente ⇒ **NO armado** (operación normal) · **Redis inalcanzable ⇒ BLOCK (fail-closed)** |
| `:266-275` | implementación: `state.enabled == true` ⇒ `Err(ChecklistError::KillSwitch)`; **JSON no parseable ⇒ fail-closed** |
| `:86-91` | `ChecklistError::KillSwitch` = `"kill_switch active"` |

**Quién lo invoca (único llamador en todo el repo):** `backend/relays-client/src/submit_engine.rs:330` (`match pre_execute_checklist(&mut ctx).await`) — todo error distinto de `PaperModeActive` es bloqueo fatal (`:372-382`). Debajo queda un guard legado documentado como *no-op fallback* (`:386-396`) que vuelve a consultar el switch vía cliente (`:393-395`, evento `submit.blocked` `reason="kill_switch"`).

**Cliente y estado** (`backend/shared-rs/src/killswitch.rs`): clave `arbx:killswitch` (`:15`), canal pub/sub `arbx:killswitch:changes` (`:16`), shape `{enabled, reason, triggered_by, updated_at}` (`:26-32`), caché de 1 s (`:61`), `is_enabled()` con fallback `default_when_absent` ante error (`:72-77`), gauge Prometheus `arbx_killswitch_enabled` (`:101`, `:127`), escritura `set()` = `SET` + `PUBLISH` (`:107-129`).
**Valor desplegado del fallback:** `configs/app.toml:7` → `kill_switch_enabled_default = true   # fail-closed in non-dev`, y el contenedor corre con `ARBX_CONFIG_PATH=/app/configs/app.toml` (medido: `docker exec arbitragex-v2-relays-client-1 env`).

**Superficie de control** (lo que un operador usa): `edge/worker/src/index.ts:989-1016` `POST /api/killswitch/:action` (activate|deactivate → `POST /admin/killswitch` con `{enabled}`); `edge/worker/src/index.ts:948-968` `POST /admin/killswitch`; `backend/api-server/src/index.ts:263-285` handler con `requireAdminToken(ARBX_ADMIN_TOKEN)`, acción de auditoría `killswitch.armed|disabled` (`:277`, `:281`); lectura `backend/api-server/src/index.ts:290-292` `GET /admin/killswitch/status`.

---

## 3. Estado ANTES (observable desde fuera y por dentro)

```
$ curl -s https://edge-arbx.ape-tv.net/status          # estación externa, 03:24:51Z y 03:32:35Z
"killswitch":{"enabled":false,"reason":"VER","triggered_by":"admin","updated_at":"2026-09-26T20:01:27.367Z"}
"services":{ 7/7 → ok:true }, "deploy":{"sha":"3f00b359beca82685280c5d8d30f099d8bd7d921","id":"37399887059"}, "env":"production-like"

$ ssh <vps> 'docker exec arbitragex-v2-redis-1 redis-cli GET arbx:killswitch'
{"enabled":false,"reason":"VER","triggered_by":"admin","updated_at":"2026-09-26T20:01:27.367Z"}      # idéntico al campo externo
$ ... 'redis-cli XLEN arbx:opps:detected' → 10001
$ ... 'curl -s http://127.0.0.1:8080/admin/killswitch/status -H "x-arbx-admin-token: $TOK"'  → mismo JSON
$ ... 'curl -s http://127.0.0.1:8787/api/killswitch/status -H "x-arbx-admin-token: $TOK"'    → mismo JSON
$ ... 'docker ps' → 25/25 contenedores "Up ... (healthy)"
$ ... 'grep -E "^(ARBX_TRADE_MODE|ARBX_LIVE_EXEC_ENABLED|ARBX_LIVE_EXEC_CHAINS)=" .env'
ARBX_TRADE_MODE=paper · ARBX_LIVE_EXEC_ENABLED=False · ARBX_LIVE_EXEC_CHAINS=11155111
```

Baseline de consumidores: `selector-api` procesando el stream (eventos `consumer.invalid_message` en vivo), `sim-ctl` con `sim_consumer.started` en `arbx:opps:validated` (grupo `sim-ctl-g0`), `relays-client` con `relays_consumer.spawned_paper_only` (`paper_mode:true`, *no broadcast*).

---

## 4. EJECUCIÓN — dos ciclos trip/untrip

### 4.1 Comandos del drill (los cuatro que un tercero debe reproducir)

```bash
# en el VPS (o cualquier host con el token de admin):
TOK=$(grep -m1 '^ARBX_ADMIN_TOKEN=' /opt/arbitragex-v2/.env | cut -d= -f2-)

# 1) TRIP  — ruta pública del operador: edge → api-server → Redis
curl -sS -X POST http://127.0.0.1:8787/api/killswitch/activate \
     -H "x-arbx-admin-token: $TOK" -H 'x-arbx-actor: g7-drill-reviewer'

# 2) VERIFICACIÓN EXTERNA (desde cualquier estación, sin token)
curl -s https://edge-arbx.ape-tv.net/status      # campo .killswitch  ⇒ enabled:true

# 3) UNTRIP
curl -sS -X POST http://127.0.0.1:8787/api/killswitch/deactivate \
     -H "x-arbx-admin-token: $TOK" -H 'x-arbx-actor: g7-drill-reviewer'

# 4) VERIFICACIÓN EXTERNA ⇒ enabled:false
curl -s https://edge-arbx.ape-tv.net/status
```

### 4.2 Ciclo 1 — T0 `03:33:05Z` → T1 `03:33:36Z` (ventana armada: 55 s)

```
-- POST /api/killswitch/activate --
{"enabled":true,"reason":"operator_activate","triggered_by":"g7-drill-reviewer","updated_at":"2026-10-06T03:33:05.358Z"}
-- redis GET arbx:killswitch (post-trip) --
{"enabled":true,"reason":"operator_activate","triggered_by":"g7-drill-reviewer","updated_at":"2026-10-06T03:33:05.358Z"}
-- prometheus arbx_killswitch_enabled (post-trip) --
[('relays-client','0'), ('recon','0'), ('sim-ctl','1'), ('api-server','1'), ('searcher-rs','0'), ('selector-api','1')]
-- selector-api: eventos de switch --   1 × "event":"consumer.halted_kill_switch"
-- sim-ctl: eventos de switch --        1 × "event":"sim_consumer.halted_kill_switch"
-- relays-client: submit.blocked / kill_switch -- 0 eventos
-- LO QUE SIGUE CORRIENDO CON EL SWITCH ARMADO --
searcher-rs  → "pool_sync.v3_tick","pools":381,"ok":362,"failed":19,"latency_ms":296   (descubrimiento vivo)
XLEN arbx:opps:detected → 10001
api-server /api/health → 200 · edge /api/health → 200 · frontend :5173 → 200
```

### 4.3 Ciclo 2 — T `03:34:32Z` → `03:34:49Z` (ventana armada: 20 s), con observación externa

```
[VPS] POST /api/killswitch/activate  → {"enabled":true,"reason":"operator_activate",
        "triggered_by":"g7-drill-reviewer-cycle2","updated_at":"2026-10-06T03:34:32.083Z"}

[MI ESTACIÓN, 03:34:36Z, 4 s después, SIN token]
$ curl -s https://edge-arbx.ape-tv.net/status
killswitch={"enabled":true,"reason":"operator_activate","triggered_by":"g7-drill-reviewer-cycle2",
            "updated_at":"2026-10-06T03:34:32.083Z"}   services_all_ok=True   deploy.sha=3f00b359…

[VPS] POST /api/killswitch/deactivate → {"enabled":false,"reason":"operator_deactivate",
        "triggered_by":"g7-drill-reviewer-cycle2","updated_at":"2026-10-06T03:34:49.145Z"}

[MI ESTACIÓN, 03:35:21Z]
$ curl -s https://edge-arbx.ape-tv.net/status
killswitch={"enabled":false,"reason":"operator_deactivate","triggered_by":"g7-drill-reviewer-cycle2",
            "updated_at":"2026-10-06T03:34:49.145Z"}  services_all_ok=True   deploy.sha=3f00b359…
```

**Observación clave del ciclo 2: con el switch ARMADO, `services_all_ok = True`.** El producto servido (plano de lectura) siguió respondiendo; lo que se detuvo fue el consumo/ejecución, no la observabilidad. Es exactamente la separación que el gate necesita poder leer desde fuera.

### 4.4 Cronología consolidada de los dos ciclos (dos consumidores independientes, misma fuente: logs del host)

| Evento | `selector-api` | `sim-ctl` | Δ vs POST |
|---|---|---|---|
| trip 1 `03:33:05.358` | `consumer.halted_kill_switch` `03:33:06.364` | `sim_consumer.halted_kill_switch` `03:33:06.350` | **+1,0 s / +1,0 s** |
| untrip 1 `03:33:59.340` | `consumer.resumed_after_kill_switch` `03:34:01.371` (`halted_for_s: 55`) | `sim_consumer.resumed_after_kill_switch` `03:34:01.365` (`55`) | +2,0 s / +2,0 s |
| trip 2 `03:34:32.083` | `consumer.halted_kill_switch` `03:34:32.756` | `sim_consumer.halted_kill_switch` `03:34:33.569` | **+0,7 s / +1,5 s** |
| untrip 2 `03:34:49.145` | `consumer.resumed_after_kill_switch` `03:34:52.758` (`halted_for_s: 20`) | `sim_consumer.resumed_after_kill_switch` `03:34:53.576` (`20`) | +3,6 s / +4,4 s |

Reanudación efectiva comprobada además por volumen: en los 30 s posteriores al untrip 1 el consumidor procesó **1 677** mensajes (`consumer.invalid_message`, avisos de esquema preexistentes ajenos al switch).

---

## 5. ALCANCE — qué detiene el switch y qué NO

| Componente | ¿Consulta el switch? | Evidencia | Efecto medido en el drill |
|---|---|---|---|
| `selector-api` (consumo del stream `arbx:opps:detected`) | **SÍ** — `backend/selector-api/src/consumer.ts:160-178` | código + **logs** | **DETENIDO y REANUDADO** (2/2 ciclos) |
| `sim-ctl` (consumo de `arbx:opps:validated`) | **SÍ** — `backend/sim-ctl/src/consumer.rs:99-123` | código + **logs** | **DETENIDO y REANUDADO** (2/2 ciclos) |
| `relays-client` (broadcast) | **SÍ** — check 1 en `submit_engine.rs:330` + guard `:393-395` | código | **NO OBSERVABLE en este drill**: sólo consulta el switch *al enviar* y en paper no había nada que enviar → **0 eventos** `submit.blocked` |
| `searcher-rs` (descubrimiento / pool sync) | **NO** | código: sin referencias al switch | **SIGUE CORRIENDO** con el switch armado (`pool_sync.v3_tick` vivo, 4 eventos/60 s) |
| `recon` (drift / stage2) | SÍ, en modo *idle* (`drift_tracker.rs:296`, `stage2_calibration.rs:146`) | código | no medido en logs (arranca idle) |
| `api-server` / `edge` / `frontend` (plano de lectura) | NO (sólo leen y publican el estado) | `api-server/src/index.ts:240,1053` (payload), `edge/worker/src/index.ts:589-592` (proxy `/status`, caché 2 s) | **SIGUEN 200** con el switch armado; `services_all_ok=True` externo |

**Lectura honesta del alcance:** el switch **detiene el consumo y el gate de broadcast**, y **deja intacto el descubrimiento y el plano de lectura** — por diseño. **Nada "siguió ejecutando" de lo que el switch debe detener**; lo que sigue corriendo es lo que no debe detenerse (observabilidad + descubrimiento). El único punto donde la observación no alcanzó es `relays-client`, y ahí el bloqueo está probado **por código**, no por log (§9).

---

## 6. Auditoría — el drill quedó registrado (4 filas, `audit_log`)

```
$ curl -s 'http://127.0.0.1:8080/admin/audit?limit=60' -H "x-arbx-admin-token: $TOK"
total_items=60  killswitch_rows=4
  killswitch.disabled | killswitch | 2026-10-06T03:34:49.146Z | {"reason":"operator_deactivate","enabled":false}
  killswitch.armed    | killswitch | 2026-10-06T03:34:32.083Z | {"reason":"operator_activate","enabled":true}
  killswitch.disabled | killswitch | 2026-10-06T03:33:59.341Z | {"reason":"operator_deactivate","enabled":false}
  killswitch.armed    | killswitch | 2026-10-06T03:33:05.358Z | {"reason":"operator_activate","enabled":true}
```

Los `created_at` coinciden **al milisegundo** con las respuestas del POST (§4.2/§4.3): la traza es del drill, no una reconstrucción. La ruta que escribe estado + auditoría es `backend/api-server/src/index.ts:277-285`; su contrato está pinneado por tests en `backend/api-server/src/audit-events.test.ts:53-88` (`killswitch.armed` before=disabled/after=enabled y la inversa).

**Contexto del histórico (medido):** `audit_log` acumula `killswitch.armed` = **3** (2 son de este drill) y `killswitch.disabled` = **78**; en ventana de 30 días, `killswitch.armed` pasaba de **0** a **2** con este drill. Es decir: **este es el primer armado registrado en 30 días.**

---

## 7. Restitución y estado final (y la diferencia que NO oculté)

```
$ docker exec arbitragex-v2-redis-1 redis-cli GET arbx:killswitch
{"enabled":false,"reason":"operator_deactivate","triggered_by":"g7-drill-reviewer-cycle2","updated_at":"2026-10-06T03:34:49.145Z"}

$ curl -s https://edge-arbx.ape-tv.net/status        # 03:35:21Z, externo
killswitch={"enabled":false,"reason":"operator_deactivate","triggered_by":"g7-drill-reviewer-cycle2",…}
services_all_ok=True   deploy.sha=3f00b359beca82685280c5d8d30f099d8bd7d921 (sin cambios)

$ docker ps → 25/25 contenedores "Up ... (healthy)"     # el drill NO dejó el sistema detenido
$ XLEN arbx:opps:detected → 10000 (el consumidor vuelve a drenar)
```

**Diferencia declarada:** antes del drill el registro decía `{"reason":"VER","triggered_by":"admin","updated_at":"2026-09-26T20:01:27.367Z"}`. La **postura** es idéntica (desarmado) y el sistema quedó operativo, pero el registro ahora dice lo que realmente pasó. **No restauré el JSON anterior: sobrescribirlo volvería el registro una ficción.** El gate pide precisamente que exista un drill documentado; la huella del drill ES el documento.

---

## 8. No-movimiento de P/N y de capital (declarado y medido)

- **P/N: 0/115 sin cambios.** Este trabajo no ejecuta oportunidades: mueve un interruptor de control.
- **No firma, no emite, no broadcast, no capital:** medido en el host → `ARBX_TRADE_MODE=paper`, `ARBX_LIVE_EXEC_ENABLED=False`; `relays-client` log `relays_consumer.spawned_paper_only` con `paper_mode:true` ("no broadcast").
- **Postura de readiness tras el drill:** `{"go_live":false,"verdict":"NO_GO","paper_mode":true,"capital_exposure_usd":0,"live_trading":false,"submit_enabled":false,"private_relay":false}` · blockers activos = `readiness_g_sim_1`, `readiness_g_pap_1`, `a9_go_no_go_formal_pending`.
- **Consecuencia de segundo orden, buscada y acotada:** el único verificador de readiness que lee historia del switch es `g-ris-1.ts:101-127`, que exige `auto_trip_on_high_revert_rate=true` **o** ≥1 `killswitch.armed` en 30 d. El flag está en **true** en el config desplegado (`configs/app.toml:93`), así que **el veredicto de G-RIS-1 no dependía del drill ni cambió con él**; el drill sólo añadió historia (0 → 2 armados en 30 d), que es el resultado deseable y queda declarado como tal.

---

## 9. Observaciones (no son findings del gate)

**O1 — La métrica Prometheus no converge de forma uniforme (medido).** Con el switch armado: `sim-ctl=1, api-server=1, selector-api=1` pero `relays-client=0, recon=0, searcher-rs=0`. A los 5 s del untrip seguían en 1 `api-server`, `recon` y `sim-ctl`; a los 30 s seguía `recon=1`; recién a los ~2 min **todo 0**. ⇒ **el observable autoritativo es `/status` (edge) y la clave Redis**; `arbx_killswitch_enabled` sirve como indicador con retardo (cada servicio refresca el gauge cuando evalúa el switch), **no como prueba de estado**.

**O2 — Asimetría fail-closed entre el cliente y el check 1.** `is_enabled()` cae a `default_when_absent=true` en prod (`killswitch.rs:72-77` + `configs/app.toml:7`), mientras **check 1 trata la clave ausente como NO armado** (`pre_execute_checklist.rs:261,277`) — fail-open. En el único camino de broadcast los dos coexisten (`submit_engine.rs:330` + `:393`), así que la pérdida de clave sigue bloqueando **por el guard legado**, no por check 1. Es redundancia funcionando; conviene saberlo antes de tocar cualquiera de las dos piezas. **NO VERIFICADO EN RUNTIME**: no borré la clave en producción para probarlo (sería un test destructivo no pedido).

**O3 — No hay evento unificado de halt.** Cada servicio nombra distinto (`consumer.halted_kill_switch` vs `sim_consumer.halted_kill_switch`) y `relays-client` no emite evento de "idle por kill-switch" (sólo `submit.blocked` al intentar enviar). Auditar un halt exige mirar servicio por servicio.

---

## 10. Corrección de instrumento (desbloquea un blocker citado por el propio SKILL)

La orden de t56 asumía: *"El canal de shell desde esta estación no alcanza el VPS"*. **Medido: es falso, y el motivo importa.**

| Cliente | Comando | Resultado |
|---|---|---|
| `C:\Windows\System32\OpenSSH\ssh.exe` (9.5.5.1) | `ssh -V` (no conecta: sólo imprime versión) | **exit 255, stdout VACÍO, stderr VACÍO** — el binario no ejecuta en este contexto (mismo síntoma con `-G`, `-v`, `-F NUL`, ruta absoluta) |
| `C:\Program Files\Git\usr\bin\ssh.exe` | `ssh -i ~/.ssh/arbx_hetzner root@195.201.235.70 'echo VPS_SSH_OK; hostname'` | **exit 0** → `VPS_SSH_OK` / `arbx-v2-clean` |
| red | `Test-NetConnection 195.201.235.70 -Port 22` | **True** (el puerto siempre estuvo alcanzable) |

`.claude/skills/arbitragex-v2-mainnet-live/SKILL.md:29` cita *"Blocker: SSH access to VPS (exit 255)"*: **este hallazgo lo desbloquea** — el 255 no era falta de acceso, era un cliente SSH roto en la estación. Consecuencia operativa: **el drill se pudo ejecutar en el host real** y no hizo falta el rodeo por GitHub Actions. *(Causa raíz del fallo del ssh de Windows: NO ESTABLECIDA — no la atribuyo.)*

---

## 11. Corroboración independiente: el mismo round-trip corre en CI en cada PR

El drill no es un procedimiento que exista sólo en este documento: la suite `tests/e2e/killswitch.spec.ts` (armar → `/status` refleja → desarmar → `/status` refleja + guard de "reason obligatoria", `:14-68` y `:70-83`) se ejecuta en el workflow `e2e` (`e2e.yml:248`, `--repeat-each=3 --retries=0`) sobre un stack real levantado por CI.

Evidencia de una corrida reciente y pública (**run `37406246059`**, rama `docs/econ-path-01`, 2026-10-06):

```
✓ 114 [frontend] › killswitch.spec.ts:14:1 › kill-switch arms and disarms, /status reflects within seconds (2.7s)
✓ 115 [frontend] › killswitch.spec.ts:70:1 › kill-switch form refuses to arm without a reason (audit guard) (1.5s)
✓ 1..6 [frontend] › killswitch.spec.ts (repetidos ×3, sin retries)   →  6 passed (15.8s)
```

⇒ **4 round-trips verdes por corrida, reproducibles por cualquiera** (`https://github.com/hefarica/arbitragex-v2/actions/runs/37406246059`). El drill del §4 es la versión en el sistema desplegado; ésta es la versión automatizada y continua.

---

## 12. Límites declarados (NO COMPUTADO)

1. **La clave Redis ausente en runtime** (fail-open de check 1 vs fail-closed del cliente, O2): razonado sobre código + config desplegado, **no ejecutado**. Borrar `arbx:killswitch` en producción sería un test destructivo no pedido.
2. **El bloqueo de `relays-client`**: probado por código (`submit_engine.rs:330`, `:393-395`), **no observado** en este drill — en paper no había nada que enviar. Un drill del path de broadcast exigiría un envío real, que está fuera de alcance por doctrina (no firma, no emite).
3. **Latencia de propagación pub/sub**: medida de forma indirecta por los logs de los consumidores (0,7–4,4 s, con caché de 1 s del cliente y 2 s de caché del `/status` del edge). No medí el canal `arbx:killswitch:changes` extremo a extremo.
4. **Otros entornos**: el drill es del VPS productivo y del stack de CI. No toqué testnet ni anvil.

---

## 13. Handoff

| Consumidor | Qué recibe |
|---|---|
| **Gate G7** | Ítem `DRILL FALTA` → **cerrado**: drill trip/untrip ejecutado en el sistema desplegado, con detención y reanudación observadas en 2 consumidores, observabilidad externa demostrada y 4 filas de auditoría. Sugerencia: actualizar `GATES-G1-G8-ESTADO.md:13` citando este documento. |
| **Operador** | Un kill-switch que **detiene de verdad** (no sólo presente): 2 ciclos, latencias de ~1 s a la detención, y el plano de lectura intacto mientras está armado. Y una corrección útil: **el acceso SSH al VPS funciona** (usar el ssh de Git, no el de Windows). |
| **Quien repita el drill** | Los 4 comandos de §4.1; el estado se lee **desde fuera** en `.killswitch` de `/status`. Si algo no reanuda: `POST /api/killswitch/deactivate` y, como último recurso, `redis-cli SET arbx:killswitch '{"enabled":false,...}'` — y reportarlo como severidad alta. |
| **Nadie** | Esto **no autoriza**: patch, merge, deploy, mainnet, capital, firma ni broadcast. Es un interruptor de control, en modo paper, con postura `NO_GO` verificada al cierre. |

---

*Ejecutado con `ssh` de Git-for-Windows contra `195.201.235.70` (host `arbx-v2-clean`), `curl` contra `https://edge-arbx.ape-tv.net` y `http://127.0.0.1:8787|8080` en el VPS, `redis-cli`/`docker logs`/`docker exec` en el host y `curl` de Prometheus local. Único path escrito: `docs/security/G7-KILLSWITCH-DRILL-01.md` en un clon aislado; rama + PR, sin merge. No toqué `backend/`, `configs/`, `.github/workflows/` ni `implementation-state/`; no firmé, no emití, no moví capital.*
