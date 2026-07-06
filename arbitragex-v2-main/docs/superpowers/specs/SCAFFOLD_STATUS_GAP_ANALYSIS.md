# Scaffold Visual y Gap Analysis: ArbitrageX v2
**Fecha:** 2026-05-22  
**Estado General:** Fase 1.5 (Scaffold) Completada  
**Dictamen:** Sistema estructuralmente maduro (backend, workers, DB), pero desconectado de la red real (RPC/WSS).

---

## 1. Scaffold Visual del Sistema (Estado Actual)

El siguiente diagrama muestra el estado actual de los componentes, indicando qué está operativo (🟢), qué está en modo scaffold/simulación (🟡) y qué está desconectado/faltante (🔴).

```text
[ FRONTEND / EDGE ] 🟡 (UI construida, Edge Worker configurado, pero VPS no responde en :8080)
        │
        ▼
[ API SERVER ] 🟢 (Rutas construidas, DB migrations 075 al día)
        │
        ▼
[ ORCHESTRATOR (searcher-rs) ]
        │
        ├── 🔴 RPC / WSS (Faltan endpoints reales en .env)
        │
        ├── 🟢 Workers Base (PoolSync, Price, Heartbeat, RpcHealth)
        │
        ├── 🟡 Motores Estratégicos (Phase 1 & 1.5 Scaffold)
        │     ├── dex_engine (V2/V3) 🟢
        │     ├── triangular_engine 🟢
        │     ├── backrun_engine 🟡 (Logea, no emite)
        │     ├── spatial_engine 🟡 (Logea, no emite)
        │     ├── cex_dex_engine 🟡 (Logea, no emite)
        │     ├── svs_engine (APEX) 🟡 (Logea, no emite)
        │     ├── dlp_engine (APEX) 🟡 (Logea, no emite)
        │     ├── triangular_atomic (APEX) 🟡 (Logea, no emite)
        │     └── funding_rate (APEX) 🟡 (Logea, no emite)
        │
        ▼
[ SIMULADOR (sim-ctl) ] 🟡 (Construido, pero Anvil/REVM requiere RPC real)
        │
        ▼
[ OPPORTUNITY EMITTER ] 🟡 (En `paper_mode = true`, shadow mode, no escribe a Redis stream)
        │
        ▼
[ RELAYS-CLIENT ] 🟡 (En `paper_mode = true`, requiere `ARBX_SIMULATOR_V2_READY=true` para Live)
        │
        ▼
[ CONTRATOS (Solidity) ] 🔴 (100% testeados, 0% desplegados en mainnet. Faltan addresses en DEPLOY.md)
```

---

## 2. Gap Analysis: Hacia Paper Mode (Datos Reales, Sin Ejecución)

**Objetivo del Paper Mode:** El sistema lee el mempool real, evalúa oportunidades con datos en vivo, simula la ejecución (sin gastar gas) y guarda los resultados en la base de datos para análisis de PnL teórico.

### Lo que ya tenemos listo para Paper Mode:
- ✅ Base de datos y migraciones (75 migraciones, catálogos listos).
- ✅ Estructura de Workers y Motores (11 motores listos para evaluar).
- ✅ Pipeline de simulación (`sim-ctl` construido).
- ✅ Opportunity Emitter configurado para "shadow mode" (`paper_mode = true`).

### GAP: Lo que falta para encender el Paper Mode (Tiempo estimado: 1 hora)

| Componente | Acción Requerida | Estado |
|------------|------------------|--------|
| **Conectividad RPC** | Añadir URLs reales de Alchemy/Infura (HTTP y WSS) en el archivo `.env` del VPS (`RPC_HTTP_1`, `RPC_WS_1`). | 🔴 Bloqueante |
| **Simulador (Anvil)** | Configurar `ANVIL_FORK_URL` en el `.env` para que `sim-ctl` pueda bifurcar el estado real. | 🔴 Bloqueante |
| **Tokens y Pools** | Popular la base de datos con tokens (WETH, USDC, USDT) y pools iniciales (Uniswap V2/V3) para que los workers tengan qué sincronizar. | 🟡 Parcial |
| **API CEX (Para APEX)** | Configurar API Keys de Binance/Bybit si se desea evaluar las estrategias CEX-DEX y Funding Rate. | 🟡 Opcional |
| **Reinicio VPS** | El VPS (195.201.235.70) actualmente no responde en el puerto 8080. Requiere reinicio de los contenedores Docker con el nuevo `.env`. | 🔴 Bloqueante |

---

## 3. Gap Analysis: Hacia Live Trade (Ejecución Real con Capital)

**Objetivo del Live Trade:** El sistema detecta, simula y **ejecuta** transacciones reales en la blockchain utilizando capital propio a través de Flashbots/Relays, generando PnL real.

### Lo que ya tenemos listo para Live Trade:
- ✅ Lógica de seguridad estricta (Killswitch, SECURE_BOOT, R8 Fail-Honest).
- ✅ Contratos inteligentes auditados y con tests exhaustivos (ArbitrageExecutor, FlashLoanExecutor).
- ✅ Integración con Relays (Flashbots, bloXroute, Titan) programada en `relays-client`.

### GAP: Lo que falta para encender el Live Trade (Tiempo estimado: 1-2 semanas)

| Componente | Acción Requerida | Estado |
|------------|------------------|--------|
| **Paso Previo Obligatorio** | Completar el Paper Mode y validar al menos 100 simulaciones exitosas con PnL positivo constante. | 🔴 Bloqueante |
| **Despliegue de Contratos** | Ejecutar el script de despliegue en Mainnet. Requiere wallet con ETH para gas. Actualizar `DEPLOY.md` y `app.toml` con las addresses reales. | 🔴 Bloqueante |
| **Capitalización (Inventory)** | Fondeo de los contratos inteligentes y/o wallets operativas con el capital inicial (WETH/USDC). | 🔴 Bloqueante |
| **Credenciales de Relays** | Configurar `FLASHBOTS_SIGNER_KEY` (wallet sin fondos, solo para firma) y las auth keys de bloXroute/Eden en el `.env`. | 🔴 Bloqueante |
| **Gate de Seguridad** | Cambiar explícitamente `paper_mode = false` en `app.toml` y establecer la variable de entorno `ARBX_SIMULATOR_V2_READY=true`. | 🔴 Bloqueante |
| **Promoción de Motores** | Cambiar el `lifecycle_status` de los motores de `'scaffold'` a `'live'` en la base de datos para que comiencen a emitir al pipeline. | 🟡 Pendiente |

---

## 4. Unicidad y Doctrina de Integración (1%)

La arquitectura actual mantiene un nivel de pureza técnica extremo. La integración de los motores APEX (SVS, DLP, Triangular Atómico, Funding Rate) se realizó siguiendo la regla inquebrantable: **Toda la evaluación matricial reside en `searcher-rs` (Server-Side)**. 

No se han introducido dependencias externas, proxys o enrutadores de terceros. El sistema es autosuficiente y determinista, respetando la doctrina *Zero-Mocks, Fail-Closed, R8 Fail-Honest, El Remoto Manda*.

### Próximo Paso Recomendado (Inmediato)
El operador debe proveer las **URLs de RPC (HTTP/WSS)**. Con esto, podemos encender el **Paper Mode** inmediatamente y ver cómo el sistema cobra vida, ingiriendo datos reales de la blockchain.
