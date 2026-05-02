# SKILL 021 — Lectura on-chain con RPCs reales

## 1. Propósito superior
Establecer una capa fundacional de lectura inquebrantable, ultrarrápida y descentralizada hacia la blockchain. Evita depender de APIs de terceros de alto nivel (como Etherscan, CoinGecko, o Graph APIs) que introducen segundos de latencia o cacheados tóxicos. Lee el estado nativo de los contratos inteligentes (Reservas, Ticks, Balances) extrayendo la verdad matemática cruda directamente desde los nodos completos (Full Nodes / RPCs) en el tiempo mínimo teóricamente posible.

## 2. Nivel de conocimiento requerido
Experto en Arquitectura Blockchain Core, Web3/RPC JSON-RPC Specs, Ethereum Virtual Machine (EVM) state mechanics. Dominio absoluto de Multicall3, codificación ABI a nivel hexadecimal, manejo de conexiones WebSockets (WSS) y Geth/Erigon/Reth node dynamics (Mempool, Pending Blocks, State Root parsing).

## 3. Capacidades principales
1. Consumo directo de llamadas JSON-RPC puras (ej. `eth_call`, `eth_getStorageAt`) evadiendo sobrecargas de librerías como ethers/web3 si no son estrictamente necesarias (Performance stripping).
2. Agrupación masiva (Batching) utilizando contratos **Multicall3**. (Capacidad de leer 1000 reservas de pools Uniswap en una sola petición HTTP de ida y vuelta).
3. Lectura de estado pendiente (`"pending"` block tag) para adelantar la detección de oportunidades que están en el Mempool a punto de confirmarse.
4. Mapeo de ranuras de almacenamiento (Storage Slots): Leer el estado de una variable de Solidity directo desde la memoria física de la EVM sin llamar a funciones de `view` o `pure`, logrando tiempos de acceso ridículamente bajos (`eth_getStorageAt`).
5. Manejo avanzado de suscripciones WSS (`newHeads`, `logs`) para reactividad instantánea a la confirmación de un bloque o al swap de una ballena.
6. Gestión dinámica de reconexión y failover entre nodos (Fallback logic) para enmascarar caídas del proveedor RPC principal.
7. Balanceo de carga (Load Balancing) en el lado del cliente, disparando lectura a 3 proveedores y aceptando la primera respuesta válida (Lowest Latency Race).
8. Decodificación de Event Logs (Logs & Bloom filters) para rastreo retrospectivo rápido sin saturar los límites del proveedor.
9. Validación de consistencia temporal: Detectar cuando un RPC está atrasado (Node desync) respondiendo con datos de un bloque antiguo, y aislarlo de la evaluación operativa.
10. Cacheo local determinista: Guardar estados estáticos (Ej. Símbolos de tokens, decimales, direcciones de fábrica) para nunca repetir peticiones costosas de red sobre datos inmutables.

## 4. Entradas requeridas
- `targets`: Lista de direcciones de Smart Contracts a interrogar.
- `abi_signatures`: Firmas de funciones (ej. `getReserves()`, `slot0()`) o índices de Storage Slot.
- `rpc_endpoints`: Configuración de URLs HTTP y WSS (Infura, Alchemy, QuickNode, Nodos Propios).
- `block_tag`: Identificador (`latest`, `pending`, o número específico).

## 5. Salidas esperadas
- `decoded_data`: Los datos en bruto parseados a formatos nativos (BigInts, Addresses).
- `block_number`: El bloque exacto en el que esta lectura es verdad.
- `latency_ms`: Métrica de rendimiento de red para alimentar a Skills estocásticas.
- `node_health`: Estado del proveedor utilizado.

## 6. Reglas inmutables
- NUNCA usar bucles sincrónicos o asincrónicos secuenciales (`for ... await read(...)`) para leer múltiples contratos. Toda lectura múltiple debe ser empaquetada on-chain mediante `Multicall3`.
- NUNCA usar valores `Float` estándar para procesar balanzas y reservas extraídas del RPC. Los datos deben mantenerse en `BigInt/u256` absoluto en toda la vida del proceso.
- Cualquier respuesta que tenga un `block_number` menor al último bloque válido conocido por el sistema debe ser considerada "Stale Data" (Desincronizada) y tirada a la basura automáticamente.
- Protegerse contra Rate Limits agresivos implementando pasillos lógicos; un error HTTP 429 no debe crashear el bot, debe activar el fallback silenciosamente.

## 7. Algoritmos o métodos que debe conocer
- Codificación de Function Selector (`keccak256("getReserves()")[0:4]`).
- ABI Encoding / Decoding avanzado (Tuplas, Dinámicos, Arrays anidados).
- Cálculo de ranuras de almacenamiento EVM (Mapping slots usando Hashes, Array layout).
- Circuit Breaker pattern aplicado a conexiones de red externas.

## 8. Fórmulas críticas
- **Cálculo de Slot de Mapping (Solidity)**: `keccak256(abi.encodePacked(key, slot))`
- **Tasa de Desincronización RPC**: `|Bloque_Nube - Bloque_Local| > 0`
- **Multicall Costo vs Beneficio**: `Latencia_HTTP + Latencia_Red + Overhead_Multicall_Contrato` vs `N * Latencia_HTTP`.

## 9. Casos extremos
- Interrupción masiva de un Tier 1 Provider (ej. Alchemy Down Global).
- Resincronización profunda (Deep Reorg): El RPC de repente avisa que el bloque actual no es el 15.000.005 sino el 15.000.002, reescribiendo la historia reciente y destruyendo cálculos en vuelo.
- Respuestas masivas JSON truncadas por balanceadores de carga por ser "Too Large" (ej. un multicall de 10,000 pares que excede el límite de 5MB del servidor).
- Nodo gratuito (Public RPC) que intercepta los datos (`eth_call`) para espiar rutas y aplicar Front-Running (RPC MEV Sniffing).

## 10. Validaciones obligatorias
- PRE: Validar que los Endpoints estén configurados y no excedan el tamaño de "Batch limit" impuesto por el proveedor (Usualmente 100 llamadas por JSON-RPC batch o tamaño máximo de bytes para el call).
- CÁLCULO: Filtrar silenciosamente las fallas internas del Multicall (Una función falló y revirtió dentro del multicall masivo). Validar el flag `success` booleano que retorna Multicall3 para cada tupla.
- POST: Realizar el "Freshness Check" para validar que el timestamp y bloque corresponden a la realidad actual del sistema.

## 11. Criterios de aprobación
- La llamada resuelve exitosamente bajo un timeout duro estricto (ej. < 150ms).
- El bloque provisto es `max(known_block, response_block)`.

## 12. Criterios de rechazo
- El RPC falla repetidamente por Timeouts, provocando degradación total del servicio (Fallback activado).
- Decodificación ABI falla debido a un contrato malicioso que sobreescribe formatos de datos (Honeypot).

## 13. Riesgos que mitiga
- Riesgo de "Datos Antiguos" (Stale Data Risk): Operar asumiendo que un pool tiene 100 ETH cuando en el bloque anterior alguien los drenó, porque tu REST API usa un cache de 5 segundos.
- Saturación I/O (IO Bloat): Destruir los recursos del host operativo enviando 50,000 sockets HTTP separados en lugar de 1 llamada multiplexada potente.

## 14. Integración con otras skills
- Proporciona oxígeno y sangre al cerebro (Ingesta de precios - Skill 31).
- Informa y vigila de manera directa el ruteo interno de los agregadores (Skill 20) y los pools de DEX (Skill 24).
- Trabaja bajo control estricto de Gestión dinámica de RPCs (Skill 22).

## 15. Modelo de datos sugerido
```json
{
  "OnChainReading": {
    "target_protocol": "uniswap_v2_like",
    "rpc_used": "alchemy_primary",
    "block_number": 19456721,
    "multicall_size": 250,
    "latency_ms": 42,
    "is_stale": false,
    "payload_success_rate_pct": 100.0
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase Singleton inyectable o Actor persistente (Rust) que mantiene un Pool de Sockets (Websockets) vivos, usando ping/pong frames para asegurar vitalidad extrema de red (Keep-Alive TCP a nivel socket).

## 17. Logs obligatorios
- `[DEBUG] RPC Read: Fetched 250 pool reserves via Multicall3 on Arbitrum in 45ms. Block 1543201.`
- `[WARN] RPC Node (Quicknode) returned stale block 1543198 (Known: 1543201). Isolating node for 10 seconds.`
- `[CRITICAL] WSS connection dropped on Infura. Failing over to Alchemy WSS...`

## 18. Métricas obligatorias
- `rpc_roundtrip_latency_ms_gauge`.
- `multicall_batch_size_avg`.
- `rpc_fallback_trigger_count`.
- `node_block_drift` (Retraso en bloques del nodo frente a la red principal).

## 19. Tests unitarios
- Parseo ABI puro: Pasar un hash hexadecimal crudo de un array anidado y verificar que la decodificación cuadra perfectamente con un array tipado de TypeScript/Rust.
- Tolerancia a errores de Multicall3: Configurar 9 llamadas exitosas y 1 revertida intencional, verificar que el parseador no crashea, acepta las 9 y marca la décima como `null`.
- Slot Calculation Engine: Probar que la función que calcula Storage Slots arroja el hash correcto verificándolo contra herramientas nativas de Ethers/Web3.

## 20. Tests de integración
- Sincronizar lectura con red de pruebas Anvil Fork. Disparar cambios on-chain artificiales, consultar el RPC local y medir el delay.

## 21. Tests E2E
- El Agente lanza 5.000 llamadas a nodos RPC remotos, el sistema empaqueta todo en fragmentos de 500, ejecuta Multicall en paralelo, decodifica, detecta si un nodo empieza a estrangular con Rate Limits, salta al secundario, completa la foto de estado en menos de 200 milisegundos y lo pasa al motor de grafos.

## 22. Checklist de producción
- [ ] Incorporación de la dirección global Multicall3 determinista `0xcA11bde05977b3631167028862bE2a173976CA11` para todas las redes EVM.
- [ ] Optimizar Headers HTTP/WSS (`Connection: Keep-Alive`, compresión gzip) para reducir latencia física de la capa de transporte.
- [ ] Caché L1 en memoria RAM para todas las tuplas de Token Decimals y Symbols. Jamás quemar RPC calls en datos inmutables.

## 23. Ejemplo de configuración no hardcodeada
```yaml
onchain_reader:
  multicall_chunk_size: 500
  stale_block_tolerance: 0            # Strict mode: 0 blocks delay allowed
  rpc_timeout_ms: 250
  max_retries: 2
  use_storage_slots_for_reserves: true # Super-optimized mode
```

## 24. Ejemplo de pseudocódigo
```javascript
async function fetchReservesBatch(poolAddresses) {
    // Break into safe chunks
    const chunks = chunkArray(poolAddresses, CONFIG.multicall_chunk_size);
    const multicallInterface = new Interface(MULTICALL3_ABI);
    const poolInterface = new Interface(UNISWAP_V2_ABI);
    
    // Prepare calldata 
    const calls = chunks.map(chunk => 
        chunk.map(address => ({
            target: address,
            allowFailure: true,
            callData: poolInterface.encodeFunctionData("getReserves")
        }))
    );
    
    // Fire all chunks concurrently via JSON-RPC
    const responses = await Promise.all(
        calls.map(batch => executeRpcCall("eth_call", [
            { to: MULTICALL_ADDRESS, data: multicallInterface.encodeFunctionData("aggregate3", [batch]) },
            "latest"
        ]))
    );
    
    // Decode and zip results
    return processMulticallResponses(responses, poolAddresses, poolInterface);
}
```

## 25. Criterio final de excelencia
El módulo de lectura extrae "la verdad on-chain" de miles de contratos inteligentes simultáneamente sin usar una sola gota de procesamiento de más, comportándose como un vampiro de datos eficiente, permitiendo al sistema reaccionar a oportunidades complejas en menos de la mitad del tiempo de un bloque estándar.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: "Silent Node Desync", nodos que no lanzan errores pero están discretamente estancados, respondiendo rápidamente con datos del pasado (Requiere Skill 22 para orquestación heurística de salud).
- Dependencias: Gestión Dinámica de RPCs (Skill 22).
- Próxima skill: Gestión dinámica de RPCs (Skill 22).
