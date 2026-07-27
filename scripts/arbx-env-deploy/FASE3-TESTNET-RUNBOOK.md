# FASE 3 — Camino a Testnet (capital $0, paper/shadow)

Runbook para llevar la DAPP al máximo punto permitido por doctrina (§32/§33):
**testnet verificado end-to-end con evidencia, capital expuesto = $0, cero broadcast a mainnet.**
El flip a mainnet/capital real es **acción manual del operador con sus llaves** (gate A.9), fuera de este runbook.

---

## 0. Estado de la doctrina de gates (A.4 → A.9)

| Gate | Estado | Qué lo desbloquea | ¿Automático? |
|---|---|---|---|
| `rpc_http_1` (testnet) | ⚠️ fragmento listo | Aplicar `fragments/testnet_rpc.env.example` al VPS `.env` | Parcial (tú aplicas) |
| `executor_1` | ❌ | Deploy `ArbitrageExecutor` a Sepolia + set `EXECUTOR_1` | ❌ Tu firma (KMS) |
| `a4_fork_real` | ❌ | Correr `multistep_fork` vs RPC archive + executor | Bloqueado por los 2 anteriores |
| `a5_paper_shadow` | ⏳ | ≥7 días continuo post-A.4 | Tiempo |
| `a6_circuit_breakers` | ⚠️ parcial | Suite comprehensiva (código) | ✅ Código |
| `a7_relay_no_submit` | ❌ | Cliente relay paper-only no-submit (código) | ✅ Código |
| `a8_confidence_scoring` | ⚠️ no cableado | Wire bayesian+kelly al scoring path (código) | ✅ Código |
| `a9_go_no_go` | ❌ | Firma formal 2 operadores | ❌ Tu decisión |

**Lo que ESTE runbook te deja listo para hacer tú:** los gates que dependen de tus llaves (RPC, Executor, A.9).
**Lo que el código (yo) puede construir sin tus llaves:** A.6, A.7, A.8.

---

## 1. Aplicar el fragmento RPC testnet al VPS (no-secreto)

El fragmento `fragments/testnet_rpc.env.example` contiene **solo URLs públicas keyless** de 6
testnets EVM (Sepolia, Holesky, Hoodi, Arbitrum Sepolia, Optimism Sepolia, Base Sepolia).
Generado desde `ArbitrageX_Unified_Config.xlsm` (RPC Providers + _RED_lookup) — cero secretos.

```bash
# 1. Copia el fragmento a .env aplicable (nombre sin .example)
cp scripts/arbx-env-deploy/fragments/testnet_rpc.env.example /tmp/testnet_rpc.env

# 2. Upsert idempotente al VPS .env (con backup), luego recrea los consumidores
scp /tmp/testnet_rpc.env arbx:/tmp/testnet_rpc.env
ssh arbx 'bash /opt/arbitragex-v2/scripts/arbx-env-deploy/arbx_env_upsert.sh \
    /opt/arbitragex-v2/.env /tmp/testnet_rpc.env --backup \
  && docker compose --env-file /opt/arbitragex-v2/.env -f /opt/arbitragex-v2/docker/compose.prod.yml \
       up -d --force-recreate --no-deps searcher-rs api-server sim-ctl relays-client \
  && rm -f /tmp/testnet_rpc.env'
```

> El upsert (`arbx_env_upsert.sh`) es idempotente + structure-preserving + backup (tested 15/15).
> No toca `paper_mode` ni ninguna llave — solo inyecta las URLs RPC testnet.

**Verificación tras aplicar:**
```bash
ssh arbx 'grep -E "SEPOLIA_RPC_URL|RPC_HTTP_11155111" /opt/arbitragex-v2/.env | head'
ssh arbx 'docker compose -f /opt/arbitragex-v2/docker/compose.prod.yml logs --tail=20 searcher-rs | grep -iE "rpc|connected|chain"'
```

---

## 2. Deploy del Executor a Sepolia (TU FIRMA — fuera de mi ejecución)

El gate `executor_1_missing` requiere un `ArbitrageExecutor` deployado a testnet.
**CI es keyless** (m5-sepolia-validation solo SIMULA el deploy). La firma real la haces tú:

```bash
# Desde tu plano KMS/operador (NUNCA desde CI, NUNCA con llave en el repo):
cd contracts
SEPOLIA_RPC_URL="https://ethereum-sepolia-rpc.publicnode.com" \
  forge script script/DeployTestnet.s.sol \
    --rpc-url "$SEPOLIA_RPC_URL" \
    --broadcast \
    --verify  # opcional, requiere ETHERSCAN_API_KEY
# → anota la dirección del ArbitrageExecutor resultante

# Set en el VPS .env (TU paso, con la dirección real):
#   EXECUTOR_1=0x<direccion_sepolia>
```

> Doctrina §33.2: ninguna llave privada en archivos versionados ni en mi contexto.
> `DEPLOYER_PRIVATE_KEY`/`CRUCIBLE_DEPLOYER_KEY` del config maestra las manejas tú (KMS/env local).

---

## 3. A.4 fork validation (tras 1 + 2)

```bash
ssh arbx 'cd /opt/arbitragex-v2 && \
  RPC_HTTP_1=<sepolia_archive> EXECUTOR_1=0x<...> \
  cargo test -p searcher-rs --test multistep_fork -- --ignored --nocapture'
```
> Nota: los RPC públicos keyless NO son archive-complete. Para A.4 (REVM con estado histórico)
> puede requerirse un endpoint archive (Alchemy/QuickNode archive plan). Esa API key es tuya.

---

## 4. Código que yo (IA OMEGA) construyo sin tus llaves

- **A.7** `relays-client` no-submit paper client: construye+firma bundle, valida forma de
  aceptación del relay (Flashbots/MEV-Blocker/Titan), DESCARTA sin enviar.
- **A.8** wire `bayesian_filter::adverse_selection_score` + `kelly_sizing::compute_position_size`
  al scoring path del orchestrator → `/api/v1/sim/pipeline`.
- **A.6** suite comprehensiva de circuit breakers (DD tiers, max revert/gas/latency, RPC health,
  blacklists) → `risk_events` + Prometheus.

## 5. Crucible 72h (gate final pre-capital)

`/omega-s5/crucible` con anti false-green. Requiere A.4+A.5 verdes y las 3 testnets
(Holesky/Sepolia/Amoy) con RPC aplicado (paso 1). ≥95% success en 72h.

---

## ⚠️ Recordatorio de seguridad (obligatorio)

El archivo `ArbitrageX_Unified_Config.xlsm` contiene **secretos reales en texto plano**
(tokens admin, DB passwords, JWT secret, llaves de firma, GitHub PAT). Detectado en esta sesión:
- Una nota `Hector01*` junto a `ARBX_EDGE_TOKEN` — si es contraseña real reutilizada, **rótala ya**.
- El README del pipeline (línea 61) ya advierte: "Rotate the 4 creds that leaked to the chat transcript earlier."

**Acción recomendada (tuya):** rotar todos los secretos del config que hayan aparecido en
transcripts/repos, y migrar el vault a un gestor (Vault integrado, SOPS, o KMS) en vez de Excel plano.
