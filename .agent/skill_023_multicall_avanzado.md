# SKILL 023 — Multicall avanzado

## 1. Propósito superior
Orquestar la lectura y agregación masiva del estado de cientos de Smart Contracts en una única petición a la blockchain. Minimiza el consumo de I/O, elude restricciones de "Rate Limits" de los proveedores RPC, y garantiza que todas las lecturas pertenecen exactamente al mismo instante en el tiempo (Atomicidad temporal). Si Multicall no existiera, la sincronización en cadena de miles de pools tomaría minutos en lugar de milisegundos.

## 2. Nivel de conocimiento requerido
Experto en EVM (Ethereum Virtual Machine) State, ABI Encoding/Decoding (Application Binary Interface) y Smart Contract Interoperability (EIPs). Profundo conocimiento del estándar `Multicall3`, manejo dinámico de payloads hexagonales, y mitigación de límites de gas en simulaciones (`eth_call` gas constraints).

## 3. Capacidades principales
1. Generación asíncrona de lotes (Chunking/Batching): Romper arreglos de 10,000 llamadas en "Chunks" seguros de 500-1000 llamadas para no exceder los límites del payload JSON o límites de gas de simulación del nodo RPC.
2. Composición de tuplas ABI `(address target, bool allowFailure, bytes callData)`.
3. Soporte para fallos controlados (`allowFailure = true`): Evita que un contrato en mantenimiento/roto destruya todo el lote entero si revierte.
4. Desempaquetado inverso (Zip mapping): Vincular exactamente la respuesta hexadecimal anónima número `N` con la dirección y función de origen original para alimentar la base de datos de pares.
5. Inyección nativa en memoria (Multicall3 `tryAggregate` u otras variantes).
6. Determinación de saldos masivos de cuenta (Batch balanceOf) instantánea.
7. Cálculo heurístico de Chunk Size dinámico (Si el nodo RPC arroja error "Out of Gas", reducir automáticamente el tamaño del chunk a la mitad y reintentar).
8. Conversión en caliente de strings o tuplas anidadas complejas devueltas por contratos oscuros, sin paniquear el parser del nodo.
9. Uso de State Overrides en el Multicall para simular balances falsos y observar cómo reaccionan cientos de Smart Contracts simultáneamente.
10. Agrupamiento semántico: Paquetes para Uniswap V2, paquetes para Uniswap V3 (requiere leer `slot0`, `liquidity`, y `tickBitmap`), optimizando el empaquetado.

## 4. Entradas requeridas
- `calls_array`: Lista de objetos que definen a qué contrato llamar, y qué método codificado ejecutar.
- `chunk_size_limit`: Cota máxima de llamadas por paquete (Configurable por proveedor de RPC).
- `multicall_contract_address`: Dirección desplegada oficial de `Multicall3` en la cadena actual.

## 5. Salidas esperadas
- `decoded_results`: Array plano o diccionario mapeado con las respuestas tipadas (e.g., `BigInt`, `Address`, `Boolean`).
- `failed_calls_log`: Direcciones y firmas de las peticiones que revirtieron on-chain, para marcarlas como incompatibles (Blacklist).
- `execution_block`: Altura del bloque de la fotografía leída.

## 6. Reglas inmutables
- Toda extracción masiva de datos (ej. Scanner de Pools) DEBE hacerse con Multicall. Iterar `getReserves()` una por una vía asincrónica individual (Promise.all con 10,000 requests HTTP) está estrictamente penado por ineficiencia letal.
- Los fallos de ejecución on-chain individuales dentro del Multicall (cuando un par está deprecado y lanza `Revert`) deben ignorarse amablemente, nunca deben paniquear el proceso global de extracción.
- El objeto resultado siempre debe devolver el `blockNumber` en el cual fue ejecutado para sincronización matemática con Skill 1.

## 7. Algoritmos o métodos que debe conocer
- Vector Chunking & Pagination.
- Buffer / Hexadecimal byte slicing.
- Tuple encoding `abi.encode` / `abi.decode`.
- Retries iterativos asíncronos bajo degradación de payload.

## 8. Fórmulas críticas
- **Límite Teórico de Chunk**: `Max_Gas_Limit_del_RPC / Promedio_Gas_por_Call` (Típicamente `30,000,000 / 25,000 = 1200` calls max por paquete).
- **Overhead Hexadecimal**: La firma del ABI genera 4 bytes de selector + paddings de 32 bytes (256 bits). Monitorear tamaño máximo de HTTP POST.

## 9. Casos extremos
- Un Token malicioso implementa un `balanceOf` con un bucle infinito ("Gas Griefer Token"). Al agruparlo en el Multicall, consume todo el gas de simulación del nodo y explota la petición completa (`Gas limit exceeded`). El sistema debe detectarlo, aislar el bloque causante, y realizar búsqueda binaria para blacklistar el token tóxico en milisegundos.
- Diferencia de versiones de Multicall (Multicall1 vs Multicall2 vs Multicall3). Usar siempre Multicall3 (`aggregate3`) si está disponible por su capacidad de `allowFailure`.

## 10. Validaciones obligatorias
- PRE: Validar que ninguna dirección en el `calls_array` sea nula (`0x0...0`).
- CÁLCULO: Validar el tamaño del Payload de envío (No exceder 2MB HTTP POST Body).
- POST: Validar que el Array de retorno coincida exactamente en longitud (Length) con el Array de envío para mapear resultados uno a uno (Zip match).

## 11. Criterios de aprobación
- La llamada JSON-RPC devuelve un 200 OK con el Payload Data conteniendo las respuestas empaquetadas.
- La decodificación ABI convierte todos los HEX válidos a números inteligibles.

## 12. Criterios de rechazo
- El RPC rechaza la llamada indicando payload excesivo ("Request entity too large").
- El nodo RPC indica "Execution Reverted" porque la versión de Multicall3 no está desplegada en la red objetivo (Requiere despliegue local o fallback a Multicall2).

## 13. Riesgos que mitiga
- Sincronización Temporal (Time-Skew Risk): Leer el contrato A en el bloque 100 y el contrato B en el bloque 101 arruina el cálculo matemático de un arbitraje, creando oportunidades fantasma. Multicall lee todo en una única transacción congelada en el tiempo de la EVM.
- Costos Operativos por API Limits: Infura cobra por peticiones totales mensuales. Reducir 1,000,000 de llamadas sueltas a 1,000 Multicalls ahorra $1,000s de dólares en facturación de infraestructura.

## 14. Integración con otras skills
- Proporciona la infraestructura de barrido masivo a Lectura on-chain (Skill 21) y Gestión Dinámica de RPC (Skill 22).
- Usado para escanear y rastrear Liquidity Pools para el Grafo (Skill 4).

## 15. Modelo de datos sugerido
```json
{
  "MulticallBatchResult": {
    "batch_id": "req_884",
    "total_calls": 500,
    "successful_calls": 498,
    "failed_calls_indexes": [14, 280],
    "rpc_response_time_ms": 65,
    "block_number": 20111222,
    "chunk_degradation_applied": false
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Librería Helper interna altamente optimizada. Debe aceptar parámetros como un Map de diccionarios, y devolver el mismo Map hidratado con las respuestas decodificadas.

## 17. Logs obligatorios
- `[DEBUG] Firing Multicall3 (aggregate3) with 500 UniV2 pairs on Arbitrum RPC.`
- `[WARN] Multicall Batch reverted due to OutOfGas exception. Bisecting payload into chunks of 250 calls...`
- `[INFO] Multicall complete. Extracted 498 reserves. Blacklisted 2 pairs (Reverted on-chain). Time: 65ms.`

## 18. Métricas obligatorias
- `multicall_batch_size_dynamic` (El tamaño de paquete en vivo, si baja mucho indica que el RPC está ahorcando recursos).
- `multicall_decode_time_us` (Tiempo de CPU gastado en decodificar cientos de HEX en Node.js, debe cuidarse para no bloquear el Event Loop).
- `on_chain_failure_rate` (Porcentaje de tuplas que devuelven `success=false` indicando contratos rotos).

## 19. Tests unitarios
- Encoding `aggregate3`: Testear empaquetamiento manual del ABI frente a la implementación de `ethers.js` para asegurar que ambos producen exactamente el mismo String Hex.
- Payload bisection: Inyectar error de "Gas limit" simulado. El algoritmo debe dividir el array de 1000 a 500, luego a 250, hasta tener éxito, todo asincrónicamente.
- Mapeo de errores: Asegurar que si el índice 5 revierte on-chain, el índice 6 no ocupa su lugar en el array de retorno, destrozando todo el mapeo posterior de pares y datos.

## 20. Tests de integración
- Interrogar a un Fork Local con 100 contratos de prueba (50 funcionales, 50 con panic/revert) para validar el comportamiento del `allowFailure`.

## 21. Tests E2E
- El bot despierta, no tiene estado local, usa Multicall para leer el estado y saldos de TODOS los pares de Uniswap V2 activos (~5000 pares relevantes) en < 5 segundos usando chunks de 1000, e hidrata su base de datos de Memoria (Redis / RAM) para empezar a operar de inmediato.

## 22. Checklist de producción
- [ ] Uso exclusivo del método `aggregate3(struct Multicall3.Call3[] calls)` (Evita requerir balances de ETH no deseados o fallos globales).
- [ ] Implementación de "Búsqueda Binaria de Tokens Venenosos": Si un nodo RPC genérico revierte todo el multicall por culpa de 1 token gas-griefer, partir por la mitad hasta aislar el index exacto y reportarlo.
- [ ] Caché de la dirección global de Multicall3 en variables estáticas para evitar consultas DNS o Storage innecesarias.

## 23. Ejemplo de configuración no hardcodeada
```yaml
multicall_engine:
  default_chunk_size: 800
  min_chunk_size_fallback: 50
  multicall_contract_address: "0xcA11bde05977b3631167028862bE2a173976CA11"
  decode_in_worker_thread: true # Essential for Node.js event loop health
```

## 24. Ejemplo de pseudocódigo
```javascript
async function executeMulticall(callsArray, rpcManager) {
    let chunkSize = CONFIG.default_chunk_size;
    let chunks = chunkArray(callsArray, chunkSize);
    let results = [];
    
    for (const chunk of chunks) {
        let success = false;
        while (!success) {
            try {
                const encodedPayload = encodeAggregate3(chunk);
                const response = await rpcManager.call("eth_call", [encodedPayload, "latest"]);
                
                const decoded = decodeAggregate3Response(response);
                results.push(...decoded);
                success = true;
            } catch (error) {
                if (error.message.includes("out of gas") || error.message.includes("too large")) {
                    log.warn(`Chunk size ${chunkSize} failed. Bisecting...`);
                    chunkSize = Math.floor(chunkSize / 2);
                    if (chunkSize < CONFIG.min_chunk_size) throw new Error("Irrecoverable toxic payload");
                    
                    // Re-chunk remaining data
                    const newChunks = chunkArray(chunk, chunkSize);
                    chunks.splice(chunks.indexOf(chunk), 1, ...newChunks);
                    break; // Restart loop with new smaller chunks
                } else {
                    throw error;
                }
            }
        }
    }
    return results;
}
```

## 25. Criterio final de excelencia
El motor Multicall actúa como la pala excavadora masiva del bot, barriendo y procesando todo el estado financiero de un DEX completo en milisegundos con total robustez contra contratos venenosos y límites restrictivos de red.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Bloqueo del Event Loop de JavaScript al procesar respuestas masivas (Mitigado desplazando el descifrado ABI pesado a un WebWorker/Rust FFI).
- Dependencias: Gestión Dinámica de RPC (Skill 22).
- Próxima skill: AMM mathematics (Skill 24).
