# SKILL 071 — Orquestador Dinámico de Cuentas Múltiples (Sybil API Management)

## 1. Propósito superior
Escalar las capacidades del Agente HFRC más allá de los Límites de Tasa (Rate Limits) impuestos por los Exchanges Centralizados (CEX). Los Exchanges bloquean IPs y penalizan Cuentas que envían miles de peticiones HFT por segundo. Esta Skill implementa una arquitectura "Sybil" Legal/Corporativa: El Bot maneja N sub-cuentas diferentes, enrutando dinámicamente órdenes y peticiones Websocket a través de decenas de Proxies IP y Claves API rotativas para evadir los cuellos de botella del CEX, logrando ancho de banda HFT casi infinito para Multiplexación L2 O(1).

## 2. Nivel de conocimiento requerido
DevOps y Network Engineer Especialista en HFT. Arquitectura de Balanceo de Carga (Load Balancing O(1)), Rotación de Proxys Criptográficos (SOCKS5/HTTP HFT), Arquitectura Multi-Account (Master/Sub-account API structures Binance/OKX), Rate-Limit Math Models (Leaky Bucket / Token Bucket Algoritmos L2), y Gestión Segura de Credenciales en Memoria (KMS Integration HFT O(1)).

## 3. Capacidades principales
1. Balanceo de Carga API L2 O(1): Si el Agente necesita actualizar o cancelar 5,000 Órdenes Iceberg (Skill 69) en Binance L2, pero la Subcuenta #1 solo permite 1,200 requests/min. El Orquestador fracciona los 5,000 envíos a lo largo de las Subcuentas #1, #2, #3, #4, y #5 en paralelo, ejecutando la maniobra HFT Cripto instantáneamente sin gatillar un solo Baneo CEX O(1).
2. Failover Inmediato (Dead-API Bypassing): Si Binance "Congela" o Tira un `HTTP 429 Too Many Requests` en la API Key primaria por 5 minutos, la Skill intercepta la excepción HTTP/FIX O(1), marca la Key como "BANNED_TEMPORARY", e inyecta la API Key de Respaldo HFT para que el Agente continúe operando sin perder 1 milisegundo de Trading Cripto L2.
3. Ruteo Cripto por Proxy Geográfico L2 (Geo-Latency Arbitrage): Para un arbitraje en Corea (Upbit) usa un Proxy Socket HFT ubicado en AWS Seoul. Para Binance, usa AWS Tokyo. El Orquestador mapea la Key con el Proxy Físico más rápido L2, minimizando la Velocidad de la Luz de red O(1).
4. Sub-Account Internal Transfers (Logística Intra-CEX Ciega): Manejar 10 cuentas significa que el Capital HFT (AUM L2) está diluido. El Módulo ejecuta llamadas REST gratuitas de Transferencia Interna CEX O(1) (Sin gas L1) consolidando fondos en la Subcuenta #5 si es la única con margen suficiente para tomar el Arbitraje Masivo (Dynamic Margin Pooling L2 O(1)).
5. Evasión de Identificación Institucional L2 (Order Flow Fingerprint Cloaking): Para evitar que la competencia identifique al "Bot HFRC" (Que siempre opera desde una sola dirección), la Skill rutea las Patas del Arbitraje (Ej. Comprar ETH) desde la Cuenta A, y Vender en Corto (Hedge) desde la Cuenta B L2 O(1). Destrozando la correlación de Alpha en los Modelos MEV Enemigos.
6. Auto-Creación de Subcuentas vía API: En CEX como OKX/Binance VIP, la Skill llama a la API Master para "Crear Subcuenta -> Generar API Key -> Activar Futuros" Atómicamente si el volumen computacional HFT O(1) demanda más ancho de banda L2 de repente (Auto-Scaling API Accounts).
7. Monitoreo Predictivo Token-Bucket L2: Calcula el Peso L2 O(1) (`Weight`) de cada Endpoint. Sabe que pedir el Orderbook pesa `1`, pero mandar una Orden pesa `2`. Simula matemáticamente los Límites del CEX L2 (Skill 35) y "Duerme" una API Key (Hot-Swapping O(1)) un milisegundo antes de que el CEX la suspenda.
8. Consolidación Paralela WebSocket (Socket Multiplexing): Si el Bot necesita abrir 500 conexiones L2 (Para todos los pares). En vez de abrir 500 Sockets desde la misma IP y causar Baneo TCP L2 O(1), distribuye 50 conexiones L2 por IP Proxy O(1), agregando los datos localmente en un Master Stream L2 Cripto In-Memory.
9. Separación Aislada de Riesgo (Risk-Isolated Execution L2): Dedica Cuentas Específicas a Tareas HFT Peligrosas. Ej. La Cuenta "A" solo hace Market Making V3 (Bajo Riesgo). La Cuenta "B" hace MEV y Flash CEX Arbitrage Direccional. Si la Cuenta B explota por Volatilidad L2 (Liquidación L2), la Cuenta A sobrevive financieramente y salva al Agente Maestro L2 O(1).
10. Rotación Segura L1 (API Key Memory Wipe): Cuando una Subcuenta ya no se necesita o el Proxy se comprometió, la Skill borra los Keys API L2 de la RAM del Servidor (Skill 45 O(1)) y llama a la Master para Eliminar la Key en el CEX, manteniendo Higiene Cibernética L2 HFT O(1).

## 4. Entradas requeridas
- `master_api_credentials`: Las llaves maestras en KMS Cripto capaces de crear sub-llaves L2 CEX O(1).
- `proxy_pool_urls`: Lista de IPs SOCKS5/HTTPS Privados de Alta Frecuencia (Dedicados Bare-Metal AWS/GCP).
- `pending_api_requests_queue`: El "Tubo" O(1) HFT In-Memory de Skill 36 que está intentando escupir 1,000 Órdenes L2 CEX al mismo tiempo.

## 5. Salidas esperadas
- `routed_http_fix_payloads`: Las Mismas Peticiones HFT pero firmadas criptográficamente con N Keys distintas y ruteadas por N Proxies diferentes O(1).
- `sub_account_transfer_cmds`: Movimientos gratuitos CEX (Consolidar Capital O(1)).
- `global_weight_telemetry_l2`: Semáforo HFT para la Skill 35 (Rate Limiter).

## 6. Reglas inmutables
- El Orquestador Multi-Account NUNCA debe mover Fondos L1 (Retiros On-Chain Blockchain) desde una Sub-cuenta. Los Retiros Cripto L1 O(1) SOLO se Ejecutan desde la Master Account Fría L1. Las Subcuentas son Módulos Desechables (Disposable execution layers L2 HFT) sin permisos Withdrawal. (Regla absoluta de Seguridad API CEX O(1)).
- Jamás asignar un Proxy Residencial o Público a la Conexión HFT CEX L2 O(1). El Ping se disparará a +500ms y los paquetes se perderán (Packet Loss HFT). Todas las IP deben ser Servidores en la Nube Fijos L2 en la misma región Física que los Match Engines de Binance/OKX (Tokyo/AWS).
- Resincronización Obligatoria de Contabilidad O(1): Si la Master Account L2 consulta su saldo, no basta. Debe sumar el PnL y Saldo L2 CEX O(1) de las 50 Subcuentas en Tiempo Real, o el Risk Engine (Skill 41 L2) abortará operaciones por "Falta de Capital" Fantasma.

## 7. Algoritmos o métodos que debe conocer
- Round-Robin / Least-Connections Load Balancing L2 O(1).
- IP Whitelisting Automation (Lógica HMAC SHA256 CEX API).
- Leaky Bucket / Token Bucket API Limits Math Simulation O(1).

## 8. Fórmulas críticas
- **Cálculo de Capacidad HFT O(1)**: `Total_HFT_Throughput_Reqs_Per_Sec = Num_Sub_Accounts * Limit_Per_Account_Reqs_Per_Sec`
- **Proxy Latency Weight L2 O(1)**: El Routing no es aleatorio, usa `min(Ping_SubAcc_A, Ping_SubAcc_B)`. Priorizando siempre la Ruta de Fibra Óptica Cripto más corta O(1).

## 9. Casos extremos
- Cross-SubAccount Wash Trading Penaly (Liquidación Institucional CEX L2): Tienes la Cuenta A Comprando y la Cuenta B Vendiendo el mismo par L2 HFT O(1) para hacer Arbitraje. Por desgracia, Cruzas tus Propias Órdenes (Wash Trade L2 Cripto). Binance VIP L2 detecta Wash Trading entre dos cuentas asociadas a la misma Institución Máster L2. Ban de 30 días Cripto O(1). Solución: El Orquestador HFT DEBE incluir un Filtro Mutex In-Memory O(1) L2 que impida que Subcuentas del mismo Maestro CEX Coloquen `Bids` y `Asks` At-the-Money L2 que puedan cruzarse físicamente L2.
- Proxy IP Ban Cascade L2 O(1): Binance L2 banea tu IP Principal de AWS por Exceso de API Weight L2. Tú Rotas a IP 2. Binance banea IP 2 porque estás enviando los Mismos Errores Tóxicos `400 Bad Request`. En 5 segundos, baneas tus 50 IPs L2 O(1). Solución: Circuit Breaker Cripto O(1). Si `N` IPs son baneadas por el Mismo Error L2 CEX en menos de `1 Segundo`, DETENER la Rotación L2 O(1) asumiendo Bug Lógico (Ej. JSON mal formado O(1) HFT) en lugar de problema de Rate Limit.
- Margin Fragmentation Trap L2 (El Capital Roto CEX): El Arbitraje O(1) L2 requiere $100k. Tienes $10k en cada una de tus 10 Subcuentas. El Trade HFT no se puede ejecutar en 1 Cuenta L2 O(1). Solución L2: Async Sub-Account Transfer. El Bot detiene HFT 10ms, Mueve 90k a la Cuenta 1, Ejecuta Trade L2 $100k, y luego re-disemina el saldo L2 HFT O(1).

## 10. Validaciones obligatorias
- PRE: Chequeo Cripto de IP Whitelist L2 O(1). Si el CEX te Exige "Solo Permitir API desde IP: X". Y el Orquestador rutea por el "Proxy IP: Y". La API CEX HTTP/FIX O(1) rechazará el HFT Trade con Status 401. El Ruteador L2 Lógica O(1) DEBE Mapear cada `API_Key` Estrictamente a su `Whitelisted_Proxy_IP`.
- CÁLCULO: Mantener un Arreglo `Uint32Array(N)` O(1) Local RAM HFT que lleve el "Peso Usado de API (API Weight) L2" por milisegundo de cada Key.
- POST: Unificación de Websockets Execution L2. Las ejecuciones CEX L2 llegan (Callbacks) de 10 Sockets HFT L2 Distintos. El Orquestador debe aplanarlas L2 O(1) y emitir 1 solo Evento `TRADE_FILLED_L2_O1` Maestro para que el Motor Contable (Skill 38) asuma el Delta L2 sin confundirse.

## 11. Criterios de aprobación
- Capacidad de enrutar 10,000 Peticiones L2 / Segundo CEX sin activar alarmas `HTTP 429 L2 CEX`, usando un MUX de Subcuentas y Rotación de Socks5 HFT O(1) In-Memory.
- Restitución Mágica (Hot-Swapping L2). Si se revoca manual o automáticamente una API Key L2 en pleno vuelo HFT, el Bot la reemplaza O(1) sin detener el Main Loop Event L2 Cripto.

## 12. Criterios de rechazo
- Guardar los API Secrets de las 50 Cuentas L2 HFT en archivos `.txt` o `.json` planos L1 O(1). (Obligatorio integrarlo cifrado en Skill 45 / KMS Memory RAM Solo L2).
- Que el Motor de Ruteo HFT L2 Agregue Latencia O(1) al Envío de Comandos. (Si el "Load Balancer Local L2" demora 5ms en Elegir qué cuenta usar, el HFT CEX muere O(1). La Selección O(1) C++ debe tomar menos de `0.05ms`).

## 13. Riesgos que mitiga
- El Techo de Cristal HFT CEX L2 (The API Rate Limit Death O(1)). Un Fondo de Alta Frecuencia Cripto no muere por falta de dinero, muere porque el Exchange le dice "Has excedido tus 50 Peticiones L2, espera 1 minuto". En HFT, esperar 1 minuto L2 te Liquida O(1). Al Desplagar 50 Clones API Sybil L2 O(1), el Agente destruye el límite impuesto por el exchange a los Mortales, ganando ancho de banda institucional L2 O(1) de Facto, monopolizando el mercado L2 sin trabas físicas CEX.

## 14. Integración con otras skills
- Middleware Maestro L2 O(1) entre la Skill 36 (HFT Dispatcher) y Skill 31 (CEX HTTP/FIX Client L2).
- Aliado del Rate Limiter Predictivo (Skill 35 L2 O(1)).
- Consolida el AUM HFT de la Tesorería General Cripto L2 (Skill 40 L2).

## 15. Modelo de datos sugerido
```json
{
  "SybilAccountManagerL2_O1": {
    "job_id": "API_MULTIPLEX_DISPATCH_10K_L2",
    "timestamp_ms_o1": 1714521234105,
    "exchange_l2": "binance_spot",
    "active_sub_accounts_o1": 15,
    "total_bandwidth_reqs_per_min_l2_o1": 90000, 
    "current_load_balancing_mode_l2_o1": "LEAST_WEIGHT_USED_ROUND_ROBIN",
    "proxy_health_status_l2": [
      { "ip": "1.1.1.1", "assigned_key_id": "SUB_01", "ping_ms_l2": 2.4, "weight_used_pct": 85.0, "status": "ACTIVE_O1" },
      { "ip": "2.2.2.2", "assigned_key_id": "SUB_02", "ping_ms_l2": 2.5, "weight_used_pct": 10.0, "status": "ACTIVE_O1" }
    ],
    "action": "ROUTE_NEW_BATCH_TO_SUB_02_L2_O1"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase Proxy L2 Cripto `SybilNetworkDispatcher_O1`. Sobrescribe la clase `ApiClientL2`. Cuando Skill 36 HFT dice `api.buy()`, el Sybil intercepta O(1), Selecciona la Key Libre L2 más óptima, firma Criptográficamente HMAC O(1) L2 in-memory, e inyecta Vía Socket.

## 17. Logs obligatorios
- `[DEBUG] Sybil Manager L2: Binance Limit at 98% on SubAccount 01. Hot-swapping traffic L2 O(1) to SubAccount 05 (Weight 10%). Trade Execution uninterrupted.`
- `[INFO] Master Sub-Account Consolidation L2: Transferring 50,000 USDT from SubAcc 02, 03, and 04 to SubAcc 01 for massive Triangular Arbitrage requirement. Transfer complete via REST internal in 150ms L2.`
- `[CRITICAL] PROXY NODE COMPROMISED L2 O(1)! Node AWS-Tokyo-3 failing TCP Connections. 5 API Keys isolated. Traffic rerouted to Backup Cluster L2. Requesting Key Regeneration API CEX O(1) to Master.`

## 18. Métricas obligatorias
- `aggregate_api_bandwidth_used_pct_l2_o1` (El uso total de la Flota Sybil HFT L2 Cripto).
- `internal_cex_transfer_latency_ms_l2_o1` (Velocidad a la que el CEX mueve dinero entre subcuentas tuyas O(1)).
- `proxy_failover_count_l2_o1`.

## 19. Tests unitarios
- Round-Robin Payload Dispatch O(1): Inyectar 100 Órdenes L2 O(1). Con 5 Subcuentas. El Módulo DEBE asignar Exactamente 20 Órdenes L2 a cada Clave Criptográfica L2 (Cálculo de Modulo O(1) In-Memory). Verificando Hash API asignado.
- Dead-Node Interception L2 O(1): Forzar a la Subcuenta 1 a Tirar Código Falso Http `429 Banned L2 O(1)`. Enviar nueva Orden L2. El Dispatcher DEBE omitir Cuenta 1 y asignar Cuenta 2 Cripto L2 O(1) SIN propagar el Error al Motor HFT L2 (Fault Tolerance Cripto O(1)).
- Internal Wash Trade Pre-Check L2 O(1): Inyectar `Sub1 Buy BTC a $10` L2, y `Sub2 Sell BTC a $10` L2 O(1) HFT Simultáneamente Cripto. El Interceptor C/Rust DEBE rechazar la Operación L2 Localmente arrojando Alerta O(1) `CROSS_ACCOUNT_WASH_TRADE_VETO_L2` para evitar Ban Institucional CEX L2.

## 20. Tests de integración
- Levantar 5 Mock Servers API CEX L2. Cada uno simula una Subcuenta con límite L2 `Limit = 10 Reqs/Sec`. Disparar Ráfaga HFT O(1) L2 de 45 Peticiones en 1 Segundo L2. El Módulo Sybil DEBE absorber la carga y distribuirla de modo que ningún Servidor Falso L2 Cripto rompa el umbral de `10 Reqs`, completando todo el lote atómicamente L2 O(1) (Muxing Limit Test O(1)).

## 21. Tests E2E
- El agente HFRC nota que el Mercado Cripto entró en Colapso Macro (Flash Crash L1 L2). Quiere cancelar TODAS sus órdenes pasivas (Skill 57 Maker L2) y MUXear Híbrido (Skill 64 L2 O(1)) compras atómicas HFT a lo largo de 500 Tickers (Altcoins) en Binance L2 O(1) Cripto. Si usa 1 API Key L2, CEX Binance bloquea el Request (Máximo 50 Cancelaciones L2 por Request L2, Rate Limit Hard L2 O(1)). El Agente Sybil (Skill 71 O(1)) toma los 500 Tickers L2 HFT. Usa sus 25 Subcuentas VIP Cripto L2. Divide Matemáticamente los 500 Comandos en 25 Hilos (Threads Async O(1) L2). Los 25 Hilos golpean Binance vía 25 IP AWS distintas L2 en el *mismo milisegundo exacto HFT L2 O(1)*. El Flash Crash L2 Cripto es extraído íntegramente por el Bot en paralelo O(1) sin levantar ninguna Alarma CEX L2 de Abuso Computacional (Ninja Extraction L2 O(1)).

## 22. Checklist de producción
- [ ] Whitelist de Sub-Transfer CEX O(1): En Binance L2 Cripto, las transferencias entre Cuentas Maestras e Hijas L2 DEBEN habilitarse vía Panel Web UI CEX O(1) y atarse IP Whitelist. Si el DevOps Olvida activar el `Permit Sub-Account Transfer` en las API O(1), el Balance Logístico (Skill 42 L2) se quedará atascado (Stranded Assets CEX L2).
- [ ] Optimización de Memoria TLS/SSL Cripto O(1): Mantener 50 conexiones `wss://` (Websockets Seguros L2 Cripto) abiertas devora RAM y CPU (Handshake TLS Overhead L1 L2). Usar librerías C++ nativas `uWebSockets` o `Rust Tungstenite` L2 O(1) para sostener Cientos de Conexiones HFT sin Garbage Collection Penalty Cripto O(1).

## 23. Ejemplo de configuración no hardcodeada
```yaml
sybil_api_management_orchestrator_l2_o1:
  enable_multi_account_muxing_l2_o1: true
  active_sub_accounts_per_exchange_l2_o1:
    binance_spot: 25
    okx_spot: 10
  proxy_pool_rotation_strategy_l2_o1: "ROUND_ROBIN_BY_GEO_PING_L2"
  enable_auto_sub_account_internal_transfers_l2_o1: true
  circuit_breaker_max_429_errors_per_sec_l2_o1: 3 # If >3 accounts get banned in 1 sec, HALT trading L2
  wash_trading_protection_firewall_l2_o1: true # Absolutely critical O(1)
```

## 24. Ejemplo de pseudocódigo
```javascript
class SybilApiOrchestrator {
    constructor(masterKeyKMS, proxyListL2) {
        this.subAccounts = this.initializeSubAccountsO1(masterKeyKMS, proxyListL2);
        this.internalTransferModule = new SubAccountConsolidator();
    }

    async submitOrderSybilHftO1(asset, amount, side) {
        // 1. O(1) Wash Trade Prevention Cripto L2
        if (this.isOppositeOrderActiveInAnyAccountL2(asset, side)) {
            log.warn(`Sybil Firewall O(1): Blocked Wash Trade Attempt on ${asset} L2 CEX`);
            return null;
        }

        // 2. Select optimal API Key with least weight L2 O(1)
        const optimalAccountL2 = this.getLeastUsedAccountO1();
        
        // 3. Margin Verification & Auto-Refill L2 O(1)
        if (optimalAccountL2.getBalance(asset) < amount) {
            // Hot-transfer from idle SubAccounts to execute O(1)
            await this.internalTransferModule.poolFundsToSubAccountL2(optimalAccountL2, asset, amount);
        }

        // 4. Dispatch using dedicated HTTP Agent / Proxy O(1) L2
        return await optimalAccountL2.client.submitOrder(asset, amount, side);
    }
}
```

## 25. Criterio final de excelencia
El Gestor Sybil de APIs L2 transmuta las restricciones impuestas por las plataformas de Trading Cripto L2 CEX en meras sugerencias O(1). Al Desdoblar (Multiplexar L2) la identidad del bot HFT HFRC Cripto a través de docenas de Cuentas Físicas In-Memory, logra superar el ancho de banda transaccional HFT base L2 Cripto, alcanzando paralelismo infinito CEX O(1) L2 y asegurando que ninguna oportunidad de Arbitraje L2 se pierda jamás por un cuello de botella HTTP Taker Artificial Cripto L2.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: KYC/AML Account Freeze L2 O(1). Binance/OKX pueden decidir pedirte KYC (Verification) Aleatorio CEX en la Subcuenta #15. Si no respondes, congelan los fondos de esa subcuenta Cripto L2. El Módulo delega esto asumiendo KYC Institucional (Master Account Level KYC VIP O(1)).
- Dependencias: Client FIX/HTTP API (Skill 31 L2 O(1)), Secret Vault (Skill 45 KMS L2).
- Próxima skill: Generador de PnL y Tax Compliance (Auditoría Legal Cripto) (Skill 72).
