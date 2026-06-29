# ArbitrageX V2 — Doctrine and Operational Directives

## Reglas Inmutables de Operacion

### RULE 00 — Doctrina Zero Mocks

ESTRICTAMENTE PROHIBIDO inyectar, generar o servir datos falsos, hardcodeados, simulados o decorativos (mocks) en CUALQUIER capa.

**Alcance:**
- Frontend: Renderiza exactamente lo que devuelve la API. Array vacio = mostrar vacio o "Esperando datos de la red".
- Backend: Datos UNICAMENTE de fuentes veraces (Mempool real, RPC, Contratos on-chain, PostgreSQL, Redis, Configuracion declarativa validada).
- Manejo de Errores: Si un servicio esta caido, Fail-Fast ruidosamente o mostrar estado degradado real. NUNCA ocultar error retornando datos falsos.

**Prohibiciones Absolutas:**
- Prohibido usar mocks para hacer pasar pipelines
- Prohibido hardcodear pools, tokens, oportunidades, strategy_kind, rutas, reserves, impacted_pools, o fabricas
- NUNCA fabricar una Opportunity ni ocultar silencios operacionales

### RULE 01 — Deployment Workflow (LOCAL a GIT a VPS)

```
[LOCAL: Desarrollo] -> [GIT: Commit & Push] -> [VPS: Deploy]
```

**LOCAL (Windows Desktop):**
- Proposito: Desarrollo, edicion de codigo, testing unitario, validacion de logica
- Ubicacion: `C:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\`
- Se hace: Escribir codigo, tests unitarios (vitest, cargo test), type-checking (tsc --noEmit)
- NO se hace: NO Docker Desktop, NO servicios de produccion, NO UI funcional con datos reales

**VPS (Produccion - Hetzner CX43):**
- IP: `<VPS_IP>`
- SSH Alias: `arbx`
- Ruta: `/opt/arbitragex-v2`
- OS: Ubuntu, 8 vCPU, 16 GB RAM, 160 GB SSD
- Lo que corre: `docker compose -f docker/compose.prod.yml up -d` (stack completo)

**Flujo:**
```bash
# 1. Local: Editar, tests, validar
# 2. Git: Commit & Push
git add . && git commit -m "descripcion" && git push origin main
# 3. VPS: Deploy
ssh arbx
cd /opt/arbitragex-v2
git pull origin main
docker compose -f docker/compose.prod.yml up -d --build
```

**Reglas Inmutables:**
1. NUNCA instalar Docker Desktop en la maquina local Windows
2. NUNCA levantar servicios de backend en local
3. Frontend local es SOLO para desarrollo de UI
4. Todo cambio validado localmente ANTES de push
5. Acceso funcional siempre via web apuntando al VPS

### RULE 02 — Infrastructure Strictness and Routing

- REST -> Edge Worker (`NEXT_PUBLIC_EDGE_URL`, puerto 8787 / `<VPS_HOST>`)
- WebSocket -> api-server DIRECTO (`NEXT_PUBLIC_WS_URL`, puerto 8080). NUNCA via Edge
- No-Hardcode: En produccion, FAIL-FAST si falta configuracion
- `SIM_SIGNER_ADDRESS` debe estar en `.env`. Si falta -> Crash on Boot

### RULE 03 — Next.js Docker Build Strictness

Variables `NEXT_PUBLIC_*` se hornean estaticamente durante `next build`. Si `.env` se actualiza despues del build, NO tiene efecto.

Comando obligatorio ante cambio de env:
```bash
docker compose --env-file .env -f docker/compose.dev.yml build --no-cache frontend
docker compose --env-file .env -f docker/compose.dev.yml up -d frontend
```

### RULE 04 — Next.js Docker Env Propagation

Docker Compose busca `.env` en el directorio del YAML, no en la raiz del proyecto.
- SIEMPRE usar `--env-file .env` explicitamente
- Validacion post-build: `curl -I http://127.0.0.1:5173/opportunities`

## Reglas Anti-Reincidencia (R1-R8)

### R1 — Cero Mismatch: Mounted Snapshot Pattern

Toda pagina SSR en Next.js App Router:
- `page.tsx` = Server Component puro. Hace `fetch()` al edge para snapshot serializable
- `*Client.tsx` = Client Component. Recibe `initialSnapshot` como prop
- Todo no deterministico (Date.now(), WebSocket, window) -> SOLO dentro de `useEffect()`

### R2 — Build-Time Guard

`next.config.js` contiene guard INMUTABLE que rechaza localhost en EDGE_URL durante produccion.

### R7 — Trazabilidad E2E del Pipeline

```bash
# 1. Searcher detecta?
docker logs searcher-rs --tail 200 | grep -i 'simulator.success'
# 2. Redis recibe?
docker exec redis redis-cli XLEN arbx:opps:detected
# 3. PostgreSQL recibe?
docker exec postgres psql -U postgres -d arbitragex -c 'SELECT MAX(detected_at) FROM opportunities;'
# 4. API-server sirve?
curl localhost:8787/api/opportunities/live | head
```

### R8 — Fail-Honest Pattern

El sistema debe fallar honestamente:
- `None` = no computado
- `Some(0.0)` = computado y exactamente cero

Si no hay datos reales, registrar observation con razon exacta:
`impact_zero`, `discovery_failed`, `discovery_no_pool_found`, `missing_reserves`, `unknown_token_price`, `no_base_candidates`, `watchlist_empty`

NUNCA inventar datos para avanzar. NUNCA fabricar una Opportunity.

## OMEGA Architectural Fidelity

### Reglas Inmutables de Codigo (Top 1% Standards)

1. **Asincronia Paralela (Shotgun Dispatch):** Todo I/O = 100% Non-Blocking. La latencia es la muerte.
2. **Zero-Trust & Kill-Switch:** Defensa perimetral criptografica. Kill-switch sub-milisegundo.
3. **Milisegundos son Millones:** Cero allocaciones innecesarias en hot-paths. Uso nativo de buffers, optimizacion a nivel opcode en EVM, simulacion en memoria (revm).
4. **Asimetria Topologica & Stealth Routing:** Cero mempool publico (Dark Pool Routing/Flashbots). Slippage calculado algoritmicamente.
5. **Cero Dependencias Obesas:** Protocolos puros, bypassing de kernel TCP si es necesario, WebSockets invisibles (Ghost Protocol).

### Arquitectura C-S-E (Canonica de Nivel PhD)

1. **Collector (Rust Hot-Path):** Escucha WebSockets de Mempool real. Ingestion ultra-rapida, latencia sub-milisegundo.
2. **Strategy Engine (TS Control-Plane):** Modelos Predictivos Bayesianos, filtros de toxicidad de flujo, algoritmos Bellman-Ford.
3. **Risk Engine (Risk-Management Institucional):** Interceptor estricto pre-ejecucion. Probabilidad estocastica, tail risk (EVT), rentabilidad contra gas/slippage.
4. **Executor (Paper Trade / Cloudflare Edge):** Manejo de red en el edge y ejecucion silenciosa. Modo actual: `ARBX_TRADE_MODE=paper`.

## Directivas OMEGA (Prompts Operacionales)

### Directiva 1: QUANTUM_CARTRIDGE_ECOSYSTEM_MATRIX

**Objetivo:** Modularidad, Escalabilidad y Versatilidad Absoluta. El nucleo de Rust debe ser permanentemente agnostico. Toda nueva estrategia debe requerir CERO cambios en el codigo base.

**Mandatos:**
1. Contrato Universal del Cartucho (3 funciones obligatorias)
2. Exposicion de Perifericos (Host Bindings seguros)
3. Sandboxing y Proteccion de Memoria (max_ops, fail-safe)
4. Flujo de Inyeccion E2E (UI -> Postgres -> Redis PubSub -> Hot-reload)
5. Cartucho Maestro de Prueba (dex_arb.rhai como plantilla definitiva)

### Directiva 2: QUANTUM_ORCHESTRATOR_SYMBIOSIS

**Objetivo:** Integracion del CartridgeRunner con el Orchestrator hot-path (scanner de mempool).

**Mandatos:**
1. Auditoria de Host Bindings (async no-bloqueante, RwLock/Arc)
2. Universal Data Mapper (Mempool -> pool_data estandar Rhai)
3. Integracion Hot-Path (bucle de evaluacion por cada evento)
4. Adaptacion de Estrategias Evolutivas (absorcion automatica de nuevos cartuchos)
5. Prueba de Estres y Despliegue

**Flujo del Hot-Path:**
```
A. Por cada evento/tx detectado...
B. Iterar sobre cartuchos activos en AST Registry (HashMap)
C. Filtrar por target_chains
D. Invocar evaluate_opportunity(pool_data)
E. Si is_opportunity == true -> build_payload(opportunity)
F. Emitir payload al Execution Pipeline
```

### Directiva 3: QUANTUM_AUTONOMOUS_EVOLUTION_LOOP

**Objetivo:** Escanear, auditar, descubrir el eslabon perdido, implementarlo y desplegarlo.

**Fases:**
1. AUDITORIA DE CONTEXTO ABSOLUTO (RECON) - Escanear 5 capas
2. DETERMINACION DEL OBJETIVO TACTICO - UN unico objetivo critico
3. IMPLEMENTACION QUIRURGICA (CERO MOCKS)
4. VALIDACION ESTRICTA PRE-DEPLOY (CERO-BLIND-PUSH)
5. COMMIT, PUSH Y DESPLIEGUE EN PRODUCCION (VPS)

**Entregable:**
1. DIAGNOSTICO: Que se audito y que eslabon perdido se encontro
2. ACCION: Que se implemento exactamente
3. PRUEBAS: Como se valido que funciona
4. ESTADO VPS: Confirmacion del despliegue exitoso
5. SIGUIENTE PASO: Proximo eslabon perdido para la siguiente iteracion

## Mapa de Activacion de Skills

| Trigger | Skills a activar |
|---------|-----------------|
| Caidas RPC, Rate Limits (429) | `alchemy-rpc-robust-integration` |
| Frontend no actualiza, WS muerto | `viem-websocket-resilience`, `01-hydration-forensics-expert` |
| Desarrollo del motor Rust | `rust-topology-architecture`, `artemis-simulator-framework` |
| Despliegue al VPS | `safe-production-observability`, `vps-automated-deployment-protocol` |
| Logging, env vars, secrets | `safe-production-observability` |
| Bug en produccion | `anti_reincidencia_operativa` (SIEMPRE) |
| Datos vacios en Dashboard | Ejecutar R7, luego `redis-hot-path-cache`, `postgres-schema` |
| Modificar frontend | `01-hydration-forensics-expert` a `20-deployment-runtime-scaling-strategist` |
| Optimizacion de rutas DeFi | `cfmm-optimal-routing`, `uniswap-v2-cpmm-math`, `uniswap-v3-concentrated-liquidity-math` |
| Flashbots/MEV-Share | `flashbots-bundle-construction`, `mev-share-backrun-searching` |
| Scoring de oportunidades | `mev-opportunity-prioritization-engine`, `expected-value-scoring-for-arbitrage` |
| Deteccion de anomalias | `stale-state-detection`, `token-risk-and-asset-safety-filter` |

## Risk Management Institucional

### Risk Engine (Paranoia Institucional)

- Matriz Algoritmica: Calcula rentabilidad neta rigurosa (Profit > Gas + Slippage Dinamico) antes de armar transaccion
- Stress Testing / Drawdown: Ajuste de posicion instantaneo mediante Kelly Criterion y modelos ARIMA-GARCH
- No interactua con oraculos manipulados ni liquidez toxica (VPIN detection)

### Circuit Breakers (Microstructure Defense)

- Latencia de red o divergencia RPC > 500ms -> Bloqueo tactico
- Riesgo de Drawdown > threshold estocastico -> Liquidacion/Kill switch

### Configuracion de Circuit Breakers

| Nombre | Threshold | Window | Cooldown |
|--------|-----------|--------|----------|
| token_safety_api | 5 | 60s | 120s |
| db_writes | 10 | 30s | 60s |
| stream_consumer | 20 | 60s | 30s |
| sim_engine | 8 | 60s | 60s |

## Agent Dispatch Model

El sistema opera con un modelo de despacho de agentes especializados:

1. Tarea Rust/backend -> despacha `rust-topology-engineer` + valida con `cs-validator` y `math-validator`
2. Tarea frontend -> despacha `frontend-architect` + valida con `cs-validator`
3. Tarea deploy -> despacha `devops-platform` + valida con `security-auditor`
4. Tarea contratos -> despacha `solidity-engineer` + valida con `security-auditor` y `math-validator`
5. Tarea estrategia -> despacha `strategy-architect` + valida con `economics-validator` y `math-validator`
6. Tarea datos -> despacha `data-analytics` + valida con `economics-validator`

**Regla:** Un builder sin validator = trabajo sin peer review = inaceptable.

## Onboarding Checklist

### Step 1 — Levantar stack (pre-requisito: .env con DATABASE_URL y POSTGRES_PASSWORD)

```bash
cd /opt/arbitragex-v2
docker compose -f docker/compose.dev.yml up -d
```

### Step 2 — Conectar scanner a la red

```bash
# Agregar a .env:
RPC_WS_1=wss://eth-mainnet.g.alchemy.com/v2/<KEY>
RPC_HTTP_1=https://eth-mainnet.g.alchemy.com/v2/<KEY>

# Restart servicios afectados
docker compose -f docker/compose.dev.yml restart searcher-rs sim-ctl relays-client

# Confirmar conexion
docker compose -f docker/compose.dev.yml logs --tail=30 searcher-rs | grep scanner
# Esperar: event="scanner.subscribed" chain_id=1

# Confirmar deteccion
curl -s http://localhost:9001/metrics | grep 'arbx_opportunity_total'
```

### Step 3 — Seed catalogs (opcional)

```bash
export ARBX_ADMIN_TOKEN=$(grep '^ARBX_ADMIN_TOKEN=' .env | cut -d'=' -f2)

# Agregar relay Flashbots (paper-mode)
curl -X POST http://localhost:8080/admin/relays \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"name":"flashbots","chain_id":1,"endpoint":"https://relay.flashbots.net","auth_scheme":"x-flashbots-signature","enabled":true,"priority":10}'
```

### Pending (Progressive Solicitation)

| Pending | Unlocks | Phase |
|---------|---------|-------|
| Slack webhook URL | Warning alerts to Slack | 2 |
| PagerDuty key | Critical pages on-call | 5 |
| Flashbots signer key (zero-balance) | Bundle signing paper-mode | 4 |
| Cloudflare API token + domain | Public Worker deploy | 4 |
| Backblaze B2 creds + age pubkey | Off-site encrypted backups | 5 |
| Vault seal keys (3-of-5 Shamir) | Secrets out of .env into Vault | 3 |
