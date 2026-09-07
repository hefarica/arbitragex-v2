# N6 — EXEC-TERMINUS — CROSS-EXAMINATION (ronda 2)

- **Agente:** N6 "exec-terminus" — round-table integración DApp ArbitrageX, 2026-09-06.
- **Base:** mi reporte ronda-1 `06-exec-terminus.md` (ventana 22:32Z→23:35Z) + **evidencia fresca post-ronda** (~23:47Z→23:52Z UTC, generación de flota 23:45:32Z).
- **Presupuesto HTTP público:** 2/5 usados (ambos declarados abajo; el terminus sigue sin exposición pública).
- **Método:** re-verificación del terminus en la NUEVA generación de contenedores + lectura de los 7 reportes + verificación local independiente de los 2 P0 de N3 y del claim de cableado de N5.

---

## 0. EVENTO MAYOR QUE REENCUADRA EL ROUND-TABLE: el deploy de #544 ATERRIZÓ a las 23:45:32Z

TODAS las ventanas de auditoría de la ronda 1 (N1-N8) cerraron ANTES de que terminara el tercer ciclo de auto-deploy. Evidencia propia (23:47Z):

```
$ ssh arbx git -C /opt/arbitragex rev-parse HEAD
9ac06d2dc70594dd8eac904aea027613a22a1940   ← == GitHub main (#544) — deploy veraz RESTAURADO
$ docker ps: frontend/edge/relays-client "Up 2 minutes" · vault/anvil "Up 3 minutes"
$ docker logs relays-client-1: boot 2026-09-06T23:45:32Z (3ª generación de flota en ~47 min: 22:58, 23:33, 23:45)
```

Y en el dominio público (request 1/5, 23:47Z):

```
$ curl -s https://arbx.ape-tv.net/live-readiness | grep -o 'go-no-go-signoff-card\|go-no-go-panel'
1 go-no-go-signoff-card    ← el panel #544 YA está servido públicamente
```

**Consecuencias para el round-table:**
1. El drift nº1 de N1 ("dominio 1 PR detrás de main") y el D3 de N8 ("VPS 1 merge detrás, ciclo en cola") quedan **RESUELTOS** — no por intervención del operador sino por la cola de auto-deploy. La propuesta P1 de N1 ("sincronizar VPS + rebuild") ya no aplica.
2. La P0 de N8 (deploy coalescing/serialización) se **AGRAVA**: ahora son **3 recreaciones completas de flota en ~47 minutos** (22:58:05Z #545-cycle, 23:33:07Z #543-cycle, 23:45:32Z #544-cycle), cada una con build `--no-cache` de la flota sobre el host de producción. Esto además **alimenta el hallazgo de N7**: cada rebuild regenera build-cache que el `builder prune -f` semanal no puede reclamar (17.11 GB reclaimable medidos por N7) — churn de deploy y ENOSPC proyectado (~09-12/13) son el mismo problema compuesto.
3. **El CSP de N1 SIGUE VIGENTE en el deploy nuevo** (request 2/5): única política = `content-security-policy-report-only` con `unsafe-inline`/`unsafe-eval` baked-in; CERO enforcing en el dominio público tras #544. La P1 CSP de N1 NO está resuelta por el redeploy.

---

## 1. Re-verificación del terminus en la generación 23:45Z (mi superficie)

| Chequeo | Resultado | Evidencia |
|---|---|---|
| Default-deny | **VIVO (3ª generación consecutiva)** | boot 23:45:32Z: `live_exec.policy enabled:false, allowed_chains:"[11155111]", chain_id:1, live_mode:false` + `signer.missing` + `paper_mode:true, has_signer:false, relay_backends:"none"` |
| Vault | SEALED, healthy-engañoso | `Up 3 minutes (healthy)` pese a Sealed=true (healthcheck `vault status \|\| true`, compose.prod.yml:734 re-verificado) |
| Anvil | fork mainnet vivo, sin publicación | healthy; `docker port` vacío |
| Relay catálogo | 0 backends | `relay_catalog.loaded count=0 chain_id=1` + `relay_catalog.empty` (post-23:45) |
| Consumer paper | spawn OK sobre stream vacío | `relays_consumer.spawned_paper_only` + `started stream=arbx:opps:simulated group=relays-client-g0` |

### 1.1 Hallazgos NUEVOS de mi superficie (no estaban en mi ronda-1)

**N-1. El grupo `relays-client-g0` NUNCA ha consumido nada en su historia.**
```
$ redis-cli XINFO GROUPS arbx:opps:simulated
relays-client-g0 | consumers=58 | pending=0 | last-delivered-id=0-0 | lag=0
```
`last-delivered-id 0-0` = el grupo jamás recibió una entrada desde su creación (los 58 consumers ≈ 1/boot acumulados). El stream `arbx:opps:simulated` tiene XLEN=0 (consistente con N5 D-2). **Corolario crítico para la P0 de N5:** el flip `SIM_BACKEND=revm` hará fluir el stream y ejercerá **por primera vez en producción** el camino consumer→persistence→paper_trade_runs del terminus. Es un path runtime NUNCA probado (0 entregas históricas). El flip de N5 debe incluir ventana de observación del terminus (persist_err, DLQ `dlq_max_retries:3`, rate de paper_trade_runs), no solo de sim-ctl.

**N-2. ¿Quién escribió entonces las 598.878 filas de `paper_trade_runs`?** Medido por mí (confirma N5 D-4): `MAX(created_at)=2026-09-01 16:32:06`, `COUNT=598.878`, filas en 24h = **0**. Si `relays-client-g0` nunca consumió `arbx:opps:simulated`, el ledger histórico fue escrito por OTRO camino (p.ej. paper executor legado sobre el stream validado / archiver). El flip revm reactivará un ledger cuya ruta de escritura cambia — comparabilidad histórica de labels post-flip no garantizada. Pregunta directa a N5 (§4).

**N-3. Inventario preciso de los 4 interruptores del terminus (refina mi R-6.1).** Lectura de código + runtime:
1. `ARBX_LIVE_EXEC_ENABLED` (env; exige `"true"` exacto) — plano `.env`, requiere recreate.
2. `FLASHBOTS_SIGNER_KEY` (env) — ausente del contenedor (re-verificado por grep de nombres).
3. **Paper-mode dinámico por cadena vía Redis** (`arbx:papermode:<chain>`, B0.2, `shared-rs/src/paper_mode.rs`): HOY el freno de chain-1 está **explícitamente armado**: `GET arbx:papermode:1` → `{"enabled":true,"updated_at":"2026-08-29T16:30:15Z","updated_by":"omega-diagnosis-2026-08-29","chain_id":1}`. NO existe `arbx:papermode:11155111` → para Sepolia el freno descansa en el default de config `execution.paper_mode=true` (`shared-rs/src/config.rs:124,139`, serde default). Fail-closed en ambos planos HOY, pero el freno Sepolia es implícito (config-default), no explícito (key).
4. `MainnetRefused` (código, incondicional para chain 1).
**N-3b. El interruptor Redis es mutable en runtime SIN redeploy ni .env** (write a la key + pub/sub `:changes`). Es un freno INTERNO (no bypass: `assert_broadcast_allowed` sigue siendo la primera sentencia de `build_and_sign`), pero es el switch más "alcanzable" del stack: debe entrar al alcance del gate de alertas de flip (mi propuesta 2 refinada) y al checklist pre-live fijar `arbx:papermode:11155111` EXPLÍCITAMENTE (no dejarlo al default).

**N-4. Doble cerradura estructural (refuerza el veredicto INTEGRATED).** El terminus bootea configurado para **chain_id=1** (pool RPC, catálogo de relays y signer son chain-1) mientras la política solo permite **11155111** y rechaza físicamente 1. Es decir: hoy ni siquiera un flip completo de env podría broadcastear — chain 1 → `MainnetRefused`; chain 11155111 → sin pool/catálogo/signer para esa cadena. Detalle de código: `HttpRpcPool::from_env(chain_id)` se construye UNA vez en boot para la cadena de config (`main.rs:214-228`); `submit_engine` tiene un ÚNICO `rpc_pool` (`submit_engine.rs:45,360-372`), sin pool por-cadena. El nombre `RPC_HTTP_11155111` EXISTE en el env del contenedor (inspección de nombres), pero no verifiqué que se cargue en un pool de ejecución — fail-honest: no verificado runtime. **Consecuencia práctica:** el flip a testnet-live NO es solo "env + redeploy": exige reconfigurar la cadena del servicio (cfg.chain_id o pool por-cadena), onboarding de relays Sepolia (catálogo=0) y signer. Esto ELEVA mi propuesta 5 (onboarding relays) a prerrequisito estructural documentado.

**N-5. Huérfanos de consumer: patrón SISTÉMICO, no anecdótico.** Mi superficie lo padece igual que las de N3/N5/N7: `relays-client-g0`=58 (mío) · `ws-emitter-g0`=60 (N3) · `sim-ctl-g0`=49 (N5) · `paper-archiver-g0`=61 y `selector-g0`=57 (N7). **Cinco grupos, ~285 consumers fantasma en total.** El fix correcto es ÚNICO en `shared-rs` (DELCONSUMER en shutdown / GC en boot), no 4 PRs por servicio.

---

## 2. CONFIRMACIONES (evidencia de otros que replica la mía)

1. **N2 + N8 — corrección del hallazgo semilla:** VPS corría `d4d3ff63` (que CONTIENE #545 como ancestro) y GitHub main estaba ADELANTADO (9ac06d2d), no atrás. Mi git local coincide (`ls-remote` → 9ac06d2d; HEAD local f7db667 = main+1 chore, `git diff --stat origin/main HEAD -- backend/relays-client/` vacío). El snapshot del orquestador estaba desalineado; N2/N8 lo corrigieron bien.
2. **N5 — REVM_RPC_URL y SIM_BACKEND no cableados en NINGÚN docker/*.yml:** verificado por mí con grep sobre `docker/compose.prod.yml` + `docker/compose.dev.yml` → 0 matches. Su P0 (flip operador) está correctamente gated §34.3.
3. **N3 — mismatch de eventos WS:** verificado por mí de forma independiente: `backend/api-server/src/websocket.ts:340` (`io.to('opportunities').emit('new_opportunity', opp)`) vs `frontend/lib/websocket-client.ts` (handlers `onDetected`/`onValidated` alimentados por los poll-loops de `websocket.ts:746-747` sobre los streams HOT, que tienen XLEN=0 por el `HotPathEmitter` sin call-sites). La descripción de N3 ("el productor vivo emite a nadie; el evento consumido no tiene productor") es EXACTA. Ver detalles/refinamiento en §3.4.
4. **N5 D-4 / N7 — ledger paper congelado:** confirmado con mi propio SELECT (`MAX(created_at)=2026-09-01 16:32`, 0 filas/24h).
5. **N8 D2 — reglas circuit_breakers solo en branch local:** consistente con mi HEAD (estoy EN esa branch; `c498773c` no-ancestro de main). Cuando se fusione por PR, añadir la alerta de flip del terminus (mi propuesta 2) al mismo archivo.
6. **N8 R1 — cadena de notificación inverificable:** el webhook de Alertmanager con URL `<secret>` y 0 evidencia de recibo. Esto DEGRADA mi propuesta 2 original (alerta vía Prometheus/AM): una alerta que nadie recibe no es detección. Refinada en §5-P2 (rutas duales: readiness panel A.9 + AM).
7. **N7 — problema de disco/retención:** sin contradicción desde mi superficie; añado la compounding con el churn de deploy (§0.2).

---

## 3. DESAFÍOS / ACTUALIZACIONES MATERIALES (contra-evidencia propia)

### 3.1 vs N1 (frontend-web) — drift "dominio 1 PR detrás de main": RESUELTO tras su ventana
- **Claim N1:** "Live domain es 1 PR de frontend detrás de main... acción del operador (pull+rebuild)".
- **Contra-evidencia (23:47Z):** VPS HEAD = `9ac06d2d` == GitHub main; `go-no-go-signoff-card` PRESENTE en `https://arbx.ape-tv.net/live-readiness` (curl, 1 hit). El ciclo de auto-deploy en cola cerró el drift por sí solo ~10 min después del cierre de su ventana.
- **Matiz:** su P1 "sincronizar VPS" queda OBSOLETA; su P0 (identidad de build) y P1 (CSP enforcing) SIGUEN VIGENTES — CSP re-verificado report-only en el deploy nuevo (request 2/5).

### 3.2 vs N5 (simulator-family) — su capa REMOTE_MAIN quedó stale
- **Claim N5 (§1.2/§1.3):** "`git ls-remote origin main → d4d3ff63`... SHA desplegado = main tip".
- **Contra-evidencia:** ese sample es anterior al merge de #544 (23:27:05Z). Main actual = `9ac06d2d` y el VPS YA lo despliega (23:45:32Z). No invalida sus hallazgos funcionales (0 passed, XLEN=0, flip pendiente — todo re-confirmado por mí), pero el operador NO debe citar el reporte de N5 como estado actual de main/VPS.

### 3.3 vs N8 (monitoring-fleet) — D3 resuelto + el churn es PEOR de lo que reportó
- **Claim N8:** 2 recreaciones en 35 min; "ciclo de #544 en cola"; riesgo de ciclos pisándose "no verifiqué serialización".
- **Contra-evidencia/complemento:** la serialización EXISTIÓ (3ª recreate 23:45:32Z, ~12 min después de la 2ª, sin solaparse) pero el costo sigue siendo inaceptable: 3 recreates/47 min. Su P-1 (coalescing) pasa de "urgente" a "urgente con evidencia adicional".

### 3.4 vs N3 (api-ws) — su P0 nº1 es correcto pero WO-01 (solo frontend) es insuficiente y tiene riesgo de FLOOD
- **Confirmación:** mismatch verificado (§2.3).
- **Refinamiento con mi evidencia + N7:** `new_opportunity` nace del trigger PG sobre TODA inserción de `opportunities` (~58.007/24h medido por N7; 78% XEN+AGLD flood según taxonomía de rechazo 2026-09-06). Cablear el handler del dashboard a `new_opportunity` SIN filtro cambia "silencio" por "inundación" (≈0.67 eventos/s de spam al browser). WO-01 necesita (a) filtro server-side (viable/scored) o (b) ejecutarse JUNTO a WO-02 (wiring del HotPathEmitter, que es el feed curado post-simulación para `opportunity:detected/validated`). Los dos WOs son mitades del mismo diseño; aplicarlos aislados produce o flood o nada.

### 3.5 Menor — N7 "23/23 contenedores" vs N8 "24/24"
Discrepancia de inventario entre ambos reportes (misma flota, minutos distintos). Irrelevante para conclusiones, pero el operador debe saber que uno de los dos conteos es erróneo (probablemente 24 con thanos-store o un efímero de build).

---

## 4. PREGUNTAS DIRECTAS

1. **a N5 (simulator-family):** ¿Tu plan de flip revm incluye observación del TERMINUS? `relays-client-g0` tiene `last-delivered-id 0-0` (jamás consumió; 58 huérfanos): el flujo post-flip ejerce por primera vez consumer→persistence→`paper_trade_runs`. Y: ¿verificaste quién escribió las 598.878 filas históricas del ledger (este grupo no fue)? Si fue el paper-executor legado, los labels post-flip no son comparables con la historia.
2. **a N5 + N7 (conjunta):** el stream `arbx:opps:simulated` usa MAXLEN 10.000 (consumer.rs de sim-ctl, lectura de N5) — el MISMO patrón trim-vs-consumo que N7 detectó en `arbx:opps:detected` (lag>length → ~488 entradas recortadas sin consumir para paper-archiver/selector). ¿Convendrá revisar la política de trim del stream simulated ANTES del flip revm, para que el primer flujo real no empiece perdiendo entradas del ledger paper?
3. **a N8 (monitoring-fleet):** el pool chain-1 de relays-client bootea con 5 providers `[publicnode, drpc, flashbots, mevblocker, blockpi]` — EXACTAMENTE los 5 con `RpcCircuitBreakerOpen` en tu muestra de Alertmanager (instance=searcher-rs:9001). ¿Comparten searcher-rs y relays-client el CSV `RPC_HTTP_1`? Si sí, el triage de tus 5 breakers es un problema de salud RPC de FLOTA (afecta también al terminus), no solo del searcher.
4. **a N1 (frontend-web):** dado que el dominio ya sirve #544 (mi curl 23:47Z), ¿re-validaste tus otros hallazgos contra la generación 23:45Z? Mi CSP check dice que report-only persiste — confirma tu lado (paridad de assets/build-id) para cerrar tu drift-list con estado actual.
5. **a N4 (searcher-pipeline):** tu reporte quedó EN_CURSO sin veredicto y el round-table lo necesita: la evidencia dispersa (target UP en N8, 58K opps/24h en N7, 37.9K sims/24h en N5) sugiere pipeline VIVO, pero nadie formaliza esa conclusión ni el sub-diagnóstico de los 5 breakers RPC abiertos. ¿Cierras veredicto?

---

## 5. PROPUESTAS REFINADAS (mi tabla ronda-1, actualizada con dependencias)

| # | What (delta vs ronda-1) | Why / dependencia | Pri | Gate |
|---|---|---|---|---|
| P1 | **Provisionar signer vía Vault** (kill del camino `.env` plano) — SIN cambios; añade: fijar EXPLÍCITAMENTE `arbx:papermode:11155111` en el checklist pre-testnet (hoy el freno Sepolia es config-default, no key; y el switch Redis es mutable sin redeploy — N-3/N-3b) | El switch más alcanzable del stack debe ser auditable y explícito antes de testnet-live | **P0** | §34.3 + arbx-pre-execute-checklist + operador |
| P2 | **Alerta de flip con rutas DUALES**: gauge `arbx_live_exec_enabled{chain}` + log/audit de cambios de `arbx:papermode:*` (PaperModeState ya lleva `updated_by` — exponerlo) → Alertmanager **Y** panel readiness/A.9 (GoNoGo) | Dependencia descubierta: N8 R1 demostró que la entrega del webhook AM es inverificable — una alerta que nadie recibe no es detección; el panel A.9 es la ruta que el operador mira | **P1** | arbx-risk-limits-enforcement + N8 P-2 (verificar webhook) |
| P3 | Healthcheck honesto de vault (sealed≠ready) — SIN cambios; re-confirmado en la generación 23:45Z | Sinergia con GOAL-WORKORDERS WO-03 (Vault decorativo) | **P1** | safe-production-observability |
| P4 | Probe E2E en staging de `MainnetRefused` — AMPLIADO: incluir probe del path Sepolia completo, porque el terminus bootea chain-1-configurado (pool/catálogo/signer chain-1) y NO existe pool por-cadena (`submit_engine.rs:45`) — el flip testnet requiere reconfig de servicio, no solo flags (N-4) | La barrera tiene tests pero 0 ejercicio runtime; y el camino Sepolia tiene un hueco de wiring nunca verizado | **P1** | arbx-simulation-mandatory |
| P5 | Onboarding relays Sepolia (catálogo 0→≥1) — ELEVADO a prerrequisito estructural del flip testnet (junto a reconfig de cadena) | Sin backends ni pool Sepolia el terminus es 501/NotSubmitted aunque todo lo demás se flipee | **P1** | Operador + §32/§33 |
| P6 | **DELCONSUMER/GC sistémico en `shared-rs`** para los 5 grupos con fuga (~285 huérfanos: relays-client-g0 58, ws-emitter-g0 60, sim-ctl-g0 49, paper-archiver-g0 61, selector-g0 57) — UN PR, no cuatro | Patrón medido por 3 verificadores independientes (N3, N5, N7, yo) | **P1** | P-∅ + test de shutdown |
| P7 | **Ventana de observación del terminus en el flip revm de N5** (persist_err, DLQ, rate de paper_trade_runs en la primera hora) — el consumer se ejercerá por primera vez en su historia (N-1/N-2) | La P0 de N5 sin esto deja ciego el extremo del pipe que escribe el ledger | **P1** | coordinado con N5 P0 (§34.3 flips operador) |
| P8 | Normalizar `ARBX_LIVE_EXEC_ENABLED=False`→`"false"` + documentar contrato `"true"` exacto — SIN cambios | Trampa cognitiva capital | P2 | — |
| P9 | RPC pool chain-1: UNIFICAR triage con N8 P-3 (mismos 5 providers en searcher y relays; posible CSV `RPC_HTTP_1` compartido) | Si los breakers del searcher aplican al pool del terminus, el pool está más degradado de lo que su boot sugiere | P2 | alchemy-rpc-robust-integration |
| P10 | ~~Desplegar 9ac06d2d~~ **RESUELTO** (23:45:32Z, verificado por mí en VPS + dominio). Sustituido por: **documentar la cola de auto-deploy** (los drifts de deploy se auto-curan en ~15 min; las auditorías deben esperar 1 ciclo antes de declarar drift) + endorsement de N8 P-1 coalescing con mi evidencia de 3ª recreate | 3 recreates/47 min + regeneración de build-cache alimentan el ENOSPC de N7 | P2 | G4/G5 deploy-veraz |
| P11 | **Identidad de build en TODA la flota** (consolidación): build-arg `GIT_SHA` → frontend (meta/header), sim-ctl `/capabilities build.sha`, edge `/health`, relays-client `service.boot` — 4 superficies (N1, N5, N2, yo) tropezaron con el MISMO hueco por separado | Un solo mecanismo, consumido por G4/G5 deploy-veraz | **P1** | P-∅; verify-deploy lo consume |

---

## 6. VEREDICTO CROSS

**Mi superficie sigue INTEGRATED — y más fuerte que en ronda-1:** el default-deny sobrevivió intacto a 3 recreaciones de flota (re-verificado en la generación 23:45Z), el freno paper chain-1 está explícitamente armado en Redis, y el terminus tiene HOY una doble cerradura estructural (runtime chain-1-configurado vs política Sepolia-only + MainnetRefused incondicional) que hace el broadcast imposible sin una reconfiguración deliberada de servicio. Capital expuesto = 0.

Los P0/P1 que emergen del cross son de COORDINACIÓN entre superficies: (a) el flip revm de N5 ejercerá por primera vez el consumer del terminus (nunca consumió; ledger histórico escrito por otro camino), (b) la fuga de consumers es sistémica (5 grupos), (c) el churn de deploy agrava el ENOSPC de N7, y (d) la falta de identidad de build es un hueco de flota que 4 verificadores redescubrieron por separado. El hallazgo semilla del orquestador quedó definitivamente corregido y el drift dominio↔main auto-resuelto a las 23:45Z.
