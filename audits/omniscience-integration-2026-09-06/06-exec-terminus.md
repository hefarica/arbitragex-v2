# N6 — EXEC-TERMINUS (relays-client + vault + anvil) — §34

- **Agente:** N6 "exec-terminus" — round-table integración DApp ArbitrageX, 2026-09-06.
- **Superficie:** Terminus de ejecución §34 — `backend/relays-client` (`live_exec_policy.rs`), `arbitragex-v2-vault-1`, `arbitragex-v2-anvil-1`.
- **Estado:** COMPLETADO
- **Veredicto:** **INTEGRATED** (default-deny VIVO en runtime; capital expuesto = 0; hallazgos de hardening documentados abajo)
- **Ventana auditada:** ~22:32Z→23:35Z UTC (VPS). Nota: el stack fue **redesplegado en vivo durante la auditoría** (imagen relays-client creada 23:31:39Z, contenedores reemplazados 23:33:28-34Z, mismo SHA d4d3ff63); todo lo reportado fue **re-verificado post-replace a 23:34:48Z**.
- **HTTP público usado:** 0/5 (el terminus no está publicado; no hacía falta).

---

## 1. CAPA LOCAL — MATCH

### 1.1 Política de ejecución (código fuente)

`backend/relays-client/src/live_exec_policy.rs` (leído completo, 174 líneas):

- **Default-deny:** `enabled` es true SÓLO con el string exacto `"true"` (L61-62: `matches!(enabled, Some("true"))`); cualquier otro valor (`"False"`, `"1"`, `"TRUE"`, vacío, ausente) deja la barrera armada. Test `non_true_values_are_disabled` (L158-166).
- **Mainnet físicamente rechazado:** L84-86 — `chain_id == 1` → `MainnetRefused` **INCONDICIONAL**, incluso si `1` está explícitamente en `ARBX_LIVE_EXEC_CHAINS` (test L136-145 y regresión del "triple peligro" L148-156).
- **Fail-closed:** chains no-parseables → default Sepolia `[11155111]` (L27-28, L169-172).

### 1.2 Wiring real (no código muerto)

- **Boot fail-fast:** `backend/relays-client/src/main.rs:177-196` — resuelve `LiveExecPolicy::from_env()`, loguea `event="live_exec.policy"` con `enabled/allowed_chains/chain_id/live_mode`, y hace `anyhow::bail!` si `live_mode && !assert_broadcast_allowed(chain_id)`.
- **Barrera runtime ineludible:** `backend/relays-client/src/bundle_builder.rs:106-112` — `build_and_sign` tiene `assert_broadcast_allowed(opp.chain_id)` como **PRIMERA sentencia** (re-leída por llamada; captura flips de `paper_mode` post-boot). Tests del wiring en L444-467.

### 1.3 Compose (nombres de env, sin valores)

- `docker/compose.prod.yml:218-254` — relays-client: `RELAYS_PORT=3005`, `DATABASE_URL` required, puerto `127.0.0.1:3005:3005` (loopback-only).
- `docker/compose.prod.yml:119-140` — anvil: `expose: 8545` (SIN publicación a host), `--fork-url $ANVIL_FORK_URL`.
- `docker/compose.prod.yml:706-740` — vault 1.18.1: TLS, `127.0.0.1:8200:8200`. **Hallazgo:** healthcheck L734 = `vault status || true` → siempre exit 0 → "healthy" aunque esté SEALED.
- `docker/compose.dev.yml:17-19` — comentario del plan pendiente: "Migrate all T0 secrets to Vault... /run/secrets/arbx.env rendered by vault-agent at boot" → **la integración Vault→secrets NO existe aún** (ver capa VPS).

### 1.4 Git local vs main

```
$ git branch --show-current        → a6-cbprom-01
$ git rev-parse HEAD               → f7db6867f99445c116827bcced9e35d55f760421
$ git remote -v                    → origin = https://github.com/hefarica/arbitragex-v2.git
$ git ls-remote origin main        → 9ac06d2dc70594dd8eac904aea027613a22a1940
$ git merge-base HEAD origin/main  → 9ac06d2d (= origin/main) → branch local ADELANTE de main, sin divergencia
$ git diff --stat origin/main HEAD -- backend/relays-client/ → (vacío: CERO drift de terminus entre HEAD y main)
```

---

## 2. CAPA REMOTE_MAIN — DRIFT (menor, sin impacto terminus)

- GitHub `origin/main` HEAD = **9ac06d2d** (`feat(frontend): A9-GONOGO-VISIBILITY-01 #544`).
- VPS desplegado = **d4d3ff63** (`feat(relays): A7-RELAYSIM-CALLSITE-01 #543`) — **1 commit detrás** de GitHub main.
- El commit faltante (9ac06d2d) es **frontend-only** (panel A.9): el código del terminus en VPS (d4d3ff63, que YA incluye el wiring #543 `relay_no_submit_sim` en el terminus paper/exec) es **idéntico** al de origin/main para relays-client/vault/anvil. Drift de deploy global, no de superficie.

---

## 3. CAPA VPS — MATCH (default-deny VIVO en runtime)

Comandos ejecutados read-only vía `ssh arbx`. SHA verificado: `cd /opt/arbitragex-v2 && git rev-parse HEAD` → `d4d3ff634537a8b3626ae0fcdaabac70ef3a89f0` (branch `main`). Compose en uso (labels): **`/opt/arbitragex-v2/docker/compose.prod.yml`** + `/opt/arbitragex-v2/.env`.

### 3.1 Contenedores del terminus

```
$ docker ps -a --format '{{.Names}}|{{.Status}}' | grep -Ei 'relays|vault|anvil'
arbitragex-v2-relays-client-1|Up (healthy)|arbitragex-v2-relays-client
arbitragex-v2-vault-1|Up (healthy)|hashicorp/vault:1.18.1
arbitragex-v2-anvil-1|Up (healthy)|ghcr.io/foundry-rs/foundry:latest
```

### 3.2 EL DEFAULT-DENY ESTÁ VIVO (evidencia runtime, boot post-redeploy 23:33:34Z)

```
$ docker logs arbitragex-v2-relays-client-1 2>&1 | grep -E 'live_exec.policy|signer.missing|...'
{"event":"live_exec.policy","enabled":false,"allowed_chains":"[11155111]","chain_id":1,"live_mode":false,
 "message":"M1 live-execution policy resolved (default-deny, testnet-only; mainnet refused)"}
{"event":"signer.missing","message":"FLASHBOTS_SIGNER_KEY empty/unset — /execute stays 501, consumer idle"}
{"event":"rpc_pool.ready","chain_id":1,"count":5,"providers":"[\"publicnode\",\"drpc\",\"flashbots\",\"mevblocker\",\"blockpi\"]"}
{"event":"relays_consumer.spawned_paper_only","paper_mode":true,
 "message":"consumer spawned without signer — paper_trade_runs only, no broadcast"}
```

Boot completo del servicio: `paper_mode:true, has_signer:false, relay_backends:"none", max_value_eth:1.0` (línea `service.boot`).

### 3.3 Env del contenedor (NOMBRES completos; valores SÓLO de flags de política, no secretos)

- **`ARBX_LIVE_EXEC_ENABLED=False`** — existe y está explícitamente en "False" (mayúscula). El parser exige `"true"` exacto → **disabled confirmado dos veces** (env + boot log `enabled:false`).
- **`ARBX_LIVE_EXEC_CHAINS=11155111`** (Sepolia-only).
- `ARBX_TRADE_MODE=paper`, `ARBX_ORCHESTRATOR_MODE=v2`, `ARBX_CARTRIDGE_MODE=active`, `ARBX_SIMULATOR_V2_READY=true`, `ARBX_USE_SIMULATOR_V2=true`, `SIM_BACKEND=anvil`, `ARBX_ENABLED_CHAINS=1,10,42161,8453,137`.
- **`FLASHBOTS_SIGNER_KEY`: AUSENTE del contenedor** (`grep -c '^FLASHBOTS_SIGNER_KEY='` → `0`). Tampoco `BLOXROUTE_AUTH_HEADER` ni `TITAN_AUTH_HEADER`. **No hay ninguna llave de firma en el terminus.**
- `SIM_SIGNER_ADDRESS=0x1234567890123456789012345678901234567890` — cuenta determinística anvil #0 (identidad de simulación por diseño; ver riesgos).
- Direcciones de contratos presentes: `ARBITRAGE_EXECUTOR=0x0c82...1B9B`, `FLASHLOAN_EXECUTOR_1=0xb47B...9ACE`, `EXECUTOR_1=0xE9d3...581d` — no son sentinelas `0x...dEaD`; no verifiqué on-chain su code (fuera de charter; fail-honest: "no verificado").
- Ventana de log honesta (R9): StartedAt 22:58:10Z / 23:33:34Z (replace), primera línea = boot → sin rotación; log config `max-file:5 max-size:10m`.

### 3.4 Vault — SEALED (re-verificado 23:34:48Z, post-restart 23:33:28Z)

```
$ docker exec arbitragex-v2-vault-1 vault status
Initialized        true
Sealed            true        ← SELLADO
Total Shares       3   Threshold 2   Unseal Progress 0/2
Storage Type       file        Seal Type shamir
```
El sello **sobrevive al restart** del stack (sin auto-unseal). Puerto: `8200/tcp -> 127.0.0.1:8200` (loopback-only). Logs (tail 30): sólo boot + `incrementing seal generation: generation=1`.

### 3.5 Anvil — fork de MAINNET (superficie de simulación, no de broadcast)

```
$ docker exec arbitragex-v2-anvil-1 cast chain-id --rpc-url http://127.0.0.1:8545 → 1
$ docker exec arbitragex-v2-anvil-1 cast block-number ...                  → 25921620
ANVIL_FORK_URL = https://eth.drpc.org (host; API-key redacted) — Ethereum MAINNET
```
Fork mainnet chain_id=1 en bloque ~25.92M. Correcto según §34.1-34.2 (simulación canónica contra LIVE_MAINNET; `SIM_BACKEND=anvil`). Logs muestran `EthCall` al QuoterV2 de Uniswap (0xe592427a...) con DAI/WETH mainnet + `evm_revert` — actividad de simulación real. **Sin puerto publicado a host** (`docker port` vacío) → inaccesible desde fuera de la red docker interna.

### 3.6 Exposición de puertos del terminus (todos loopback-only)

```
$ docker port arbitragex-v2-relays-client-1 → 3005/tcp -> 127.0.0.1:3005
$ docker port arbitragex-v2-vault-1         → 8200/tcp -> 127.0.0.1:8200
$ docker port arbitragex-v2-anvil-1         → (sin publicación)
```

### 3.7 Inercia del terminus (fail-honest)

- Relay catalog: `relay_catalog.loaded count=0` + `relay_catalog.empty` + `flashbots.disabled (reason:no_endpoint)` + `bloxroute.skipped` + `titan.skipped` + `multi_relay.no_backends` → **cero backends de broadcast** (modo NotSubmitted).
- Consumer: `relays_consumer.started stream=arbx:opps:simulated group=relays-client-g0` — activo pero **sin eventos de procesamiento ni denials en la ventana** (grep `LiveExecDenied|MainnetRefused|NotEnabled|no_submit` sobre el log completo → 0 eventos de negación runtime). La barrera está ARMADA pero NO fue EJERCIDA por tráfico real en la ventana observada.
- RPC pool chain-1 degradado: 4/9 proveedores caídos al boot (alchemy 429 "Monthly capacity limit exceeded", llama reto Cloudflare, 0xrpc 404, 1rpc 403) → pool vivo con 5.

---

## 4. CAPA LIVE_DOMAIN — N-A

El terminus no existe en la superficie pública: puertos 3005/8200 loopback-only y anvil sin publicación. No se realizaron requests HTTP (0/5). La frontera capital es interna por diseño.

---

## 5. DRIFTS MEDIDOS

1. **VPS d4d3ff63 vs GitHub main 9ac06d2d** — 1 commit atrás (#544, frontend-only; terminus idéntico). Deploy de 9ac06d2d pendiente.
2. **`ARBX_LIVE_EXEC_ENABLED=False`** con mayúscula — fail-closed correcto (sólo `"true"` exacto habilita), pero inconsistente con el canónico `"false"`; y `"False"`-explícito es indistinguible de "ausente" en el boot log (ambos → `enabled:false`).
3. **Flags de migración §34.2 vivos en runtime:** `ARBX_ORCHESTRATOR_MODE=v2` y `ARBX_CARTRIDGE_MODE=active` siguen definidos en el contenedor (la doctrina dice que son temporales y dejan de definir semántica; hoy coexisten con la política M1).
4. **`SIM_SIGNER_ADDRESS=0x1234...7890`** — placeholder determinístico anvil (válido para sim; RULE 02 exige signer real del operador antes de LIVE_MAINNET).
5. **Relay catálogo = 0** → terminus de broadcast sin backends; incluso con flip a Sepolia-live no habría a quién enviar bundles.
6. **RPC pool chain-1 a 5/9 proveedores** (alchemy sin capacidad mensual).
7. **Vault "healthy" engañoso** — healthcheck `vault status || true` (compose.prod.yml:734) reporta healthy estando SEALED.
8. **Redeploy silencioso durante la auditoría** (imagen 23:31:39Z, replace 23:33Z, SHA idéntico d4d3ff63) — desde mi ventana read-only no pude verificar quién/qué lo disparó ni su anclaje de deploy-veraz; estado re-auditado post-replace.

## 6. RIESGOS (frontera capital)

- **R-6.1 (medio):** El único camino de firma hoy sería `.env` plano: `ARBX_LIVE_EXEC_ENABLED=true` + `FLASHBOTS_SIGNER_KEY=<set>` + `paper_mode=false` habilitaría firma/broadcast Sepolia SIN Vault. Mainnet sigue bloqueado por `MainnetRefused` (incondicional) — el riesgo capital real queda acotado a testnet y exige 3 cambios simultáneos deliberados. Pero no existe ninguna ALERTA ante el flip: sólo una línea INFO de boot.
- **R-6.2 (medio):** La cfg de cadena activa del signer es `chain_id=1` (mainnet): toda la protección anti-mainnet descansa en el enum `MainnetRefused` del código (belt-and-suspenders probado por unit tests, no ejercido en runtime).
- **R-6.3 (bajo):** Vault está decorativo en runtime: SEALED + sin consumidor/vault-agent (el plan "T0 secrets to Vault" del compose no está implementado). No es riesgo hoy (no hay secrets que proteger en el terminus) pero el hardening prometido para el flip no existe aún.
- **R-6.4 (bajo):** Barrera M1 sin evidencia runtime ejercida (0 denials observados; consumer paper-only no llama `build_and_sign`).
- **R-6.5 (info):** `SIM_SIGNER_ADDRESS` placeholder + direcciones de executor no verificadas on-chain — obligatorio cerrar antes de LIVE_MAINNET.

## 7. PROPUESTAS → PRODUCTION-MAINNET

| # | What | Why | Priority | Effort | Gate |
|---|------|-----|----------|--------|------|
| 1 | Provisionar el signer vía Vault (unseal controlado por operador + vault-agent → `/run/secrets/arbx.env`) y eliminar `FLASHBOTS_SIGNER_KEY` del `.env` como camino posible | Hoy un flip de 3 vars de `.env` plano habilita firma Sepolia sin Vault; la frontera capital merece el store sellado YA operativo | **P0** (gate de flip) | M | §34.3 + `arbx-pre-execute-checklist` + autorización operador explícita (NUNCA auto-unseal) |
| 2 | Gauge Prometheus `arbx_live_exec_enabled{chain}` + alerta si ≠ 0, y alerta si vault pasa a unsealed fuera de ventana de operación | Un flip de env hoy sólo produce una línea INFO de boot — cero detección | **P1** | S | `arbx-risk-limits-enforcement` (extensión natural de A6-CBPROM-01) |
| 3 | Healthcheck honesto de vault (reportar sealed como no-ready o métrica separada) + readiness del terminus que exponga policy enabled/chains | `vault status \|\| true` hace "healthy" a un vault sellado — observabilidad mentirosa en la frontera capital | **P1** | S | `safe-production-observability` |
| 4 | Probe E2E runtime del `MainnetRefused` en staging (terminus real con `ARBX_LIVE_EXEC_ENABLED=true` efímero, NUNCA en prod): debe negar chain 1 | La barrera tiene unit tests pero 0 evidencia runtime ejercida | **P1** | S-M | `arbx-simulation-mandatory` |
| 5 | Onboarding de relays para Sepolia (catálogo 0 → ≥1 relay, p.ej. Flashbots protect) vía `POST /admin/relays` | Sin backends el terminus es NotSubmitted/501 — requisito previo a testnet-live | **P1** | S | Operador + §32/§33 (acreditar relays) |
| 6 | Normalizar `ARBX_LIVE_EXEC_ENABLED` a `"false"` minúscula + documentar contrato "true" exacto | Elimina la trampa cognitiva del "False" capital | **P2** | XS | — (fail-closed ya garantizado) |
| 7 | Reparar pool RPC chain-1 (Alchemy PAYG o reemplazo; 4/9 caídos) | Pool degradado = latencia/failover débil del terminus en vivo | **P2** | S | `alchemy-rpc-robust-integration` |
| 8 | Desplegar 9ac06d2d en VPS para cerrar el drift VPS↔main (frontend-only) | Higiene de deploy-veraz | **P2** | XS | `git rev-parse HEAD` == SHA despachado post-deploy |

## 8. VEREDICTO

**INTEGRATED** — El terminus de ejecución es coherente en las 4 capas para su fase (paper/testnet-first): default-deny VIVO en runtime (doble evidencia env+boot), mainnet físicamente rechazado en código con tests de regresión, SIN signer en el contenedor, SIN backends de relay, vault SEALED, anvil = fork mainnet de simulación sin exposición, puertos loopback-only, y CERO requests HTTP necesarios. Capital expuesto = 0. Los hallazgos P1 son hardening del CAMINO a production-mainnet (signer-vault, alertas de flip, healthchecks honestos, probe del refusal), no fugas actuales.
