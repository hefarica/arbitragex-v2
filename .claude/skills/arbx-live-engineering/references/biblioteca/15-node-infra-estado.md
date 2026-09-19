# 15. INFRAESTRUCTURA DE NODOS Y GESTIÓN DE ESTADO

CUÁNDO CARGAR ESTA REFERENCIA: provisionar o auditar nodos RPC (full vs archive, reth/erigon/geth/nethermind, self-host vs managed); diseñar ingesta por streams (`eth_subscribe` newHeads/logs/pending); construir estado caliente local (reserves desde logs, preload por bloque) o el ladder de ticks V3 para un quoter offline (§15.10); diagnosticar gaps de stream, reorgs y drift entre providers; dimensionar fees EIP-1559/blob y presupuestos de gas; operar cuotas 429, latencia p99 y circuit breaker por provider; integrar o portar simulación revm contra la versión pineada del lockfile (§15.11).

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Full vs archive | geth `--gcmode archive`, reth modo archive, erigon | Archive = forense/backfill de estado histórico; full = hot-path |
| Cliente del nodo | reth (Rust, performance), erigon (archive barato), geth (estándar), nethermind (.NET) | Erigon txpool no hace gossip completo → inútil para pending flow |
| Streams WS | `eth_subscribe` con `newHeads` / `logs` / `newPendingTransactions` | El payload de logs trae `removed` para detectar reorgs gratis |
| Backfill + live-tail | `eth_getLogs` por rangos + suscripción live | Idempotencia por clave `(blockHash, logIndex)` |
| Fork pineado | `anvil --fork-url <url> --fork-block-number <n>` | Mismo bloque pineado ⇒ mismo resultado, siempre |
| Simulación in-proc | revm con `CacheDB` + Evm builder (ver núcleo §1.2) | Firmas del crate varían por versión: validar antes de portar |
| Tracing forense | `debug_traceTransaction` con `callTracer` / `prestateTracer` | `prestateTracer` con `diffMode: true` entrega pre/post storage |
| Fees | `eth_feeHistory [blockCount, newestBlock, rewardPercentiles]` | baseFee se mueve como máx ±12.5% por bloque (paso 1/8 del desvío vs target) |
| Reorgs PoS | mismatch de `parentHash` en newHeads | Profundidad típica 1; finalidad ≈ 2 épocas (~12.8 min) |
| Estado caliente | logs `Sync` de V2 + preload por bloque del watchlist | V2: reserves en slot 8 del pair; V3 NO tiene reserves (tick/sqrtPrice) |
| Cuota y 429 | backoff exponencial con jitter + presupuesto por provider | Respetar header `Retry-After`; nunca reintentar en línea recta |
| Circuit breaker RPC | patrón núcleo §3.1 + sonda half-open por provider | Abrir por error-rate, no por un 429 aislado |
| Ladder de ticks V3 | snapshot `tickBitmap`/`ticks` + eventos Mint/Burn/Swap (§15.10) | 1 palabra bitmap = 256 ticks comprimidos (×tickSpacing crudos); quoter offline ANTES de revm; matemática en referencia 11 §11.2 |
| revm versión pineada | lockfile primero → doc de ESA versión (§15.11) | Divergencia in-proc vs anvil fork pineado = bug de hidratación o de versión |

## 15.1 Elección de nodo: full vs archive, cliente, self-host vs managed

**Full vs archive.** Un full node (snap sync) sirve estado reciente (`latest`, `pending`) y ejecuta el presente: es TODO lo que el hot-path necesita. Un archive node conserva el trie de estado de cada bloque histórico y sirve `eth_getStorageAt(pair, slot, bloqueDeHace6Meses)` en milisegundos. Usos en arbx:

- Hot-path (detección, simulación, fees, bundles): full es suficiente y más barato/rápido de operar.
- Forense y calibración (backfill de reserves históricas, reproducción de incidentes, `debug_traceTransaction` sobre bloques viejos): exige archive.
- Diferencia operativa real: un geth sincronizado con `--syncmode full` puede regenerar estado antiguo re-ejecutando bloques (lento pero responde); un geth snap-sync (default) NO — responde `missing trie node` para estado podado; reth en modo full poda el estado y devuelve error para bloques podados. No asumas que "full" se comporta igual en todos los clientes ni en todos los modos de sync.

**Clientes de ejecución (fortalezas honestas):**

| Cliente | Fortaleza | Debilidad para nuestro uso |
|---|---|---|
| reth | Rust (afín al stack arbx), performance de ejecución/tracing, WS sólido | Archive demanda IO y disco TB-scale; configuración de pruning afinada |
| erigon | El archive más barato de disco (snapshots + accumulator); backfill masivo con `eth_getLogs` histórico | Su txpool NO participa del gossip completo de la red: no sirve como fuente de pending transactions |
| geth | Estándar de facto, txpool completo, `debug_*`/`txpool_*` completos, máxima compatibilidad de providers | Archive con `--gcmode archive` pesa TB-scale y crece |
| nethermind | Execution client .NET, correcto en modo archive, tracing razonable | Menor afinidad con el stack Rust del repo |

**Self-host vs managed.**

- Self-host (bare metal tipo Hetzner, NVMe, 2TB+ para full, TB-scale para archive): control total de latencia y cuota, cero 429 propio; costo = ops (actualizaciones, peering, monitoreo de disco — ver doctrina de disco del repo).
- Managed (Alchemy, QuickNode, Ankr, Infura, dRPC): cero ops, geo-DNS, APIs mejoradas; costo = cuotas (compute units), rate limits (429), y flujos restringidos (p.ej. Alchemy no ofrece `newPendingTransactions` full-tx en planes estándar; ofrece su subscripción filtrada `alchemy_pendingTransactions` con `toAddress`/`fromAddress`).
- Patrón recomendado: híbrido — 2-3 providers managed con pesos para el hot-path (failover según la skill `arbx-rpc-failover-discipline` del repo) + self-host erigon/reth archive para forense y backfill offline, desacoplado del camino crítico.

**Matriz de decisión por función del sistema:**

| Función arbx | Tipo de nodo | Por qué |
|---|---|---|
| Detección + head tracking | Full managed ×2-3 (WS) | Solo necesita `latest`/`pending`; el failover es del proveedor, no tuyo |
| Simulación hot-path | revm in-proc + CacheDB (§15.4) | Cero RTT por simulación; el nodo solo alimenta el preload |
| Backfill de reserves/calibración | Archive self-host (erigon) | `eth_getLogs`/`eth_getStorageAt` históricos masivos sin quemar cuota managed |
| Post-mortem / forense | Archive (cualquier cliente) | Tracing de estado en bloques viejos (§15.5) |
| Contabilidad irreversible | Cualquier full con tags `safe`/`finalized` | Finalidad no exige archive |

## 15.2 Streams `eth_subscribe`: newHeads, logs, pending — backfill + live-tail

**Suscripciones que importan (JSON-RPC sobre WS, `wss://`):**

```
{"jsonrpc":"2.0","id":1,"method":"eth_subscribe","params":["newHeads"]}
{"jsonrpc":"2.0","id":2,"method":"eth_subscribe","params":["logs",{"address":"0x...","topics":["0x..."]}]}
{"jsonrpc":"2.0","id":3,"method":"eth_subscribe","params":["newPendingTransactions"]}
```

- `newHeads`: headers de cada nuevo bloque (`number`, `hash`, `parentHash`, `baseFeePerGas`, timestamps). Es tu reloj del sistema: TODO lo demás se ancla aquí.
- `logs` con filtro `{address, topics}`: el filtro por topic0 es el mecanismo primario para estado caliente (§15.8). Cada log llega con `removed` (true = este log desapareció por reorg), `blockHash`, `blockNumber`, `transactionHash`, `logIndex`.
- `newPendingTransactions`: por defecto devuelve SOLO hashes (requiere un segundo `eth_getTransactionByHash` por tx = N+1 RTT). geth y reth modernos aceptan el parámetro booleano fullTx (`["newPendingTransactions", true]`) que entrega el objeto completo; verifica soporte con `web3_clientVersion` por provider antes de confiar en él.

**Patrón backfill + live-tail (el estándar anti-gap):**

```
1. last_acked = leer checkpoint persistido (block_number, block_hash)
2. BACKFILL: eth_getLogs({fromBlock: last_acked+1, toBlock: "latest", address, topics})
3. aplicar logs con idempotencia por (blockHash, logIndex)  ← dedupe contra el tail
4. SUBSCRIBE logs con el mismo filtro
5. live-tail: aplicar cada log; checkpoint SOLO tras procesar
6. watchdog: si un newHead llega con number > esperado+1 → hay hueco → volver a 2
```

**Detección de gaps (dos señales independientes):**

1. Discontinuidad en `newHeads`: recibí el bloque N+2 sin pasar por N+1 → el provider perdió un push → re-sincronizar con `eth_getLogs`/`eth_getBlockByNumber` del rango faltante.
2. Drift entre providers: comparar `eth_blockNumber` de todos los providers contra el máximo observado; un provider rezagado >N bloques queda excluido de decisiones de head (pero puede seguir sirviendo reads).

**Resuscripción con reconstrucción de estado.** Un WS caído NO se reconecta "en vivo": el estado del período desconectado no llega. Protocolo correcto al reconectar: re-leer checkpoint → `eth_getLogs` del hueco → re-subscribir → dedupe por `(blockHash, logIndex)`. La suscripción nueva puede re-entregar logs del borde: por eso la idempotencia es obligatoria, no opcional.

**Head tracking tolerante a reorg (confirmaciones N).** Mantén dos nociones de head:

- `head_hot` = último newHead: ancla la detección y el target-block de bundles (bloque siguiente).
- `head_stable` = bloque con N descendientes confirmados (N=2-3 pragmático) o tag RPC `safe`/`finalized`: ancla contabilidad, persistencia y decisiones irreversibles.

Nunca derive estado persistente de `head_hot`; si ese bloque queda huérfano, el estado derivado es ficción (ver §15.7).

**Máquina de estados de aceptación de bloque (implementación mínima correcta):**

```
              newHead(N, hash_N)
                     │
                     ▼
        ┌─ hash(N-1) conocido? ──no──> fetch bloque N-1 por número
        │            │                     │ hash coincide? → continuar
        │           sí                     └ mismatch → REORG(§15.7)
        ▼
  parentHash(N) == hash(N-1)?
        │ sí                              │ no
        ▼                                 ▼
  aceptar N (head_hot=N)          REORG: fork en el último ancestro común
  procesar logs/anclas            invalidar rama huérfana y re-procesar
  head_stable = N-(confirmaciones) si N tiene suficientes descendientes
```

Regla de almacenamiento que hace esto barato: un map `number → hash` de los últimos K bloques (K = confirmaciones + margen) vive en memoria; el costo de detectar una reorg de profundidad d es d lookups, no un re-scan.

## 15.3 txpool y la realidad del orderflow

**Anatomía del txpool (geth/reth; namespace `txpool_`):**

- `txpool_status` → `{pending, queued}` (conteos).
- `txpool_content` → snapshot completo: `pending` = txs ejecutables (nonce contiguo, saldo suficiente), ordenadas por cuenta y nonce; `queued` = txs con hueco de nonce o sin saldo, no ejecutables.
- El "orden por gas" NO es una lista global ordenada: el txpool agrupa por (cuenta, nonce) y los builders/propositores seleccionan y ordenan después. Tu lectura de "qué se ejecutará primero" es una hipótesis sobre la ordenación del builder, no un hecho del pool.

**Realidad actual (2024+): el mempool público de Ethereum está mayoritariamente vacío para flow significativo.** El orderflow con valor migró a canales privados: bundles directos a builders (`eth_sendBundle`), MEV-Share (orderflow con hints), y acuerdos de wallets/APs directamente con builders. Consecuencias operativas honestas:

1. Suscribirse a `newPendingTransactions` público y esperar capturar flujo arb-relevante devuelve mayormente spam y bots de bajo valor: la señal no está ahí.
2. El flujo observable relevante llega por (a) eventos on-chain ya minados (logs `Sync`/`Swap` → reactivo al bloque, no al mempool), y (b) MEV-Share hints si se integra (núcleo §1.2/§8).
3. **Riesgo defensivo (no receta):** cualquier tx propia enviada al mempool público es visible y puede ser objetivo de frontrun/sandwich por terceros. La mitigación es no exponerse: submission privada (Protect/MEV-Share) como default. Toda consideración ofensiva sobre txs ajenas está prohibida por la skill `arbx-mev-ethics-gate` del repo — aquí solo como clasificación de riesgo.

**Inspección de candidatos (uso legítimo del txpool):** dado un hash pendiente (de MEV-Share hints o de diagnóstico), `eth_getTransactionByHash` + estimación de impacto sobre pools tocadas (`to` + calldata decodificada → rutas afectadas). Es lectura defensiva: dimensionar cómo cambiaría el estado ANTES de que confirme.

## 15.4 Simulación de estado: fork pineado, state-override, revm in-proc, bundles

**anvil con bloque pineado (validación contra estado real, ver núcleo §4.3 para CI):**

```bash
anvil --fork-url $RPC_URL --fork-block-number 21000000 --no-rate-limit
```

- `--fork-block-number` pinea el fork: sin `--block-time` y mientras solo se lancen calls de lectura (`cast call`), el estado pineado no deriva → resultados deterministas y reproducibles. Ojo: anvil automina por defecto ante cada `cast send` (mina un bloque local encima del fork) — para replay determinista registra también esos bloques locales. Cambiar el pineo invalida comparaciones previas: registra SIEMPRE el bloque pineado junto al resultado.
- Flags útiles reales: `--auto-impersonate` (ejecutar como cualquier cuenta sin firmar — solo sandbox), `--fork-retry-backoff` (resistencia al upstream), `--gas-price` para fijar precio.
- Contra el fork: `cast call`, `cast send` (con cuenta anvil), y tracing con `debug_traceTransaction` sobre lo ejecutado.

**`eth_call` con state-override (tercer parámetro).** La extensión estándar de geth/reth: `eth_call(tx, block, stateOverride)` donde `stateOverride` es un map `address → {balance, nonce, code, state, stateDiff}`. Casos de uso arbx: fingir el balance del executor para probar el camino de fallo por fondos insuficientes, o parchear un slot antes de evaluar una ruta. En Foundry, verifica en `cast call --help` el flag vigente de state-override de tu versión (el mecanismo RPC subyacente es el descrito); en anvil el override llega vía RPC directo.

**revm in-proc (simulación sin red, ver núcleo §1.2).** Patrón conceptual con nombres reales del crate `revm`:

- `CacheDB<DB>` (módulo `revm::database` en la versión pineada; la ruta `revm::db` de la era 3.x ya no existe — otra razón para PASO 0 de §15.11) envuelve un `Database` fuente (p.ej. un fetcher remoto alloy o un DB prefabricado). El CacheDB cumple dos roles: (1) caché — el segundo acceso al mismo slot no golpea la red; (2) overlay — permite insertar cuentas/slots manualmente (métodos de inserción del CacheDB) para simular balances/código arbitrarios, el equivalente in-proc de los state-overrides y de los cheatcodes de forge (revm puro NO tiene cheatcodes `vm.*`: esos son de Foundry).
- Construcción del intérprete vía el Evm builder del crate (`with_db`, spec de la forquilla correcta) y ejecución de la transacción con el `TxEnv` de la ruta. Las firmas exactas han cambiado entre versiones mayores del crate (3.x en adelante): antes de portar código, valida contra la versión pineada en el workspace arbx — port-with-validation, no copy ciego (política FUSILE del repo).
- **Diff de estado post-sim vs pre-sim:** tras ejecutar, el journal/estado de la evm expone las cuentas y storage slots tocados. Ese conjunto ES la verdad de "qué cambió mi ruta": compáralo contra lo esperado (balances de flash loan repagados, tokens recibidos). Un diff con slots no anticipados = la ruta tocó algo que el modelo no conoce → fallar honesto (R8), no asumir.

**Simulación de bundles.**

- Flashbots `eth_callBundle` con `{txs: [...raw], blockNumber, stateBlockNumber}`: simula la secuencia contra el estado de `stateBlockNumber` y devuelve por-tx `gasUsed` y si alguna revirtió — es la validación pre-envío del bundle entero, no de txs sueltas.
- El campo `revertingTxHashes` del esquema de bundle MEV-Share (legacy) permite declarar hashes que pueden revertir sin invalidar el bundle; verifica el esquema vigente en la spec de MEV-Share antes de usarlo.
- En anvil/revm el equivalente es ejecutar las txs en orden contra el mismo estado pineado y aplicar la misma semántica de reverting-tx que el destino (un bundle que requiere "ninguna revierte" falla si cualquiera revierte).

**Jerarquía de simulación por fidelidad/latencia (elige el tier según la fase del pipeline):**

| Tier | Herramienta | Latencia típica | Qué valida | Uso en el pipeline |
|---|---|---|---|---|
| 0 | revm in-proc + CacheDB | sub-ms a ms | Lógica de la ruta contra estado cacheado | Filtro masivo de candidatos (decenas de miles por bloque) |
| 1 | `eth_call` / `eth_estimateGas` contra nodo | 1 RTT | Ejecutabilidad contra estado REAL del nodo | Segundo filtro de los sobrevivientes |
| 2 | anvil fork pineado | arranque de fork: s; luego ms | Estado real + reproducibilidad + tracing | Validación pre-decisión del plan final; replay (núcleo §9.2) |
| 3 | `eth_callBundle` / `mev_simulateBundle` | 1 RTT al destino | Inclusion-plausibility contra el flujo del builder | Última verificación antes de submission (gateada) |

Regla de honestidad entre tiers: un plan que pasa el tier 0 con estado cacheado puede fallar el tier 1 si la caché está vieja o hubo reorg — la discrepancia entre tiers NO es un bug para silenciar: es la señal de gap/reorg que §15.2/§15.7 explotan.

## 15.5 Tracing forense: callTracer, prestateTracer, reverts, estimateGas

**`debug_traceTransaction`** (geth/reth/erigon; parámetros `[txHash, options]`, donde `options` = `{tracer, tracerConfig, ...}`):

- `{"tracer":"callTracer","tracerConfig":{"withLog":true}}` → árbol anidado de calls internas. `withLog` agrega los logs emitidos por cada frame: permite reconstruir QUÉ swaps ejecutó una tx competidora sin decodificar su calldata a mano. Forma del frame (forma canónica del tracer en geth/reth):

```json
{
  "type": "CALL",
  "from": "0x...", "to": "0x...",
  "value": "0x...", "gas": "0x...", "gasUsed": "0x...",
  "input": "0x...", "output": "0x...",
  "logs": [ { "address": "0x...", "topics": ["0x..."], "data": "0x..." } ],
  "calls": [ ...recursivo... ]
}
```

  Los tipos de frame son `CALL`, `STATICCALL`, `DELEGATECALL`, `CREATE`, `CREATE2`; un frame reverteido expone el dato en `output`/campo de error del tracer — encadena con la decodificación de reverts de abajo. Un DELEGATECALL a un router con logs `Sync` dentro es la firma inequívoca de un swap: correlación directa con la invalidación de tu caché de reserves (§15.8).
- `{"tracer":"prestateTracer","tracerConfig":{"diffMode":true}}` → `{pre, post}` del estado tocado por la tx: accounts y slots antes/después. Es la herramienta canónica para "¿qué storage tocó exactamente esta tx?" — forense de por qué una reserva ya no coincide con tu caché.
- `debug_traceCall` (mismo formato, sobre una call hipotética): trazar sin tx minada — simular tu ruta y obtener el call-tree que causaría.
- Advertencia: tracing es caro y con cuota propia en providers managed; no pertenece al hot-path. Es herramienta de post-mortem y calibración.

**Decodificación de reverts (returndata):**

- `0x08c379a0` + ABI string = `Error(string)` (require con mensaje).
- `0x4e487b71` + uint256 = `Panic(uint256)` (assert / desbordes).
- Cualquier otro selector de 4 bytes = custom error. Flujo: extraer los 4 bytes del returndata que devuelve el error del provider → resolver nombre con `cast 4byte <selector>` (diccionario público) o contra los artifacts locales → decodificar argumentos con `cast abi-decode "error MiError(uint256)" <data>`. Registrar el nombre del error como `rejection_reason` (R8 del repo), nunca "revert genérico" si el selector es resoluble.

**`eth_estimateGas` como detector de fallos.** Una estimación revierte si la call revierte en cualquier punto → sirve como probe barato de "¿esta ruta es ejecutable HOY?". Dos matices de producción: (1) algunos clientes añaden margen sobre el gas medido — usa el valor como cota, no como constante; (2) una estimación que "pasa" con gas cercano al límite del bloque es sospecha de loop dependiente de estado futuro: trátala como fallo. El presupuesto por intento se arma con este gas estimado × el fee proyectado de §15.6.

## 15.6 Fees EIP-1559 y blobs: feeHistory, bidding, presupuesto por intento

**`eth_feeHistory [blockCount, newestBlock, rewardPercentiles]`** devuelve por bloque: `baseFeePerGas` (array que incluye el del siguiente bloque — es proyectable), `reward` (priority efectiva pagada, por percentil pedido: p.ej. `[25,50,75]`), `gasUsedRatio` (0-1). Es la ÚNICA fuente honesta para bidding: el percentil alto de `reward` de los últimos bloques es tu piso competitivo si quieres top-of-block. Ejemplo de par y respuesta (forma real del método):

```
→ {"method":"eth_feeHistory","params":["0x5","latest",[25,50,75]]}
← {"oldestBlock":"0x14d3a2",
   "baseFeePerGas":["0x1d1a94a105","0x1cf1a2a34d", ...],   // len = blockCount+1
   "gasUsedRatio":[0.42,0.87, ...],
   "reward":[["0x3b9aca00","0x5f5e1000","0x77359400"], ...]}
```

Notas de la forma: `baseFeePerGas` tiene UN elemento más que `blockCount` (el extra es el proyectado del siguiente bloque — úsalo para el ceiling); `reward[i]` es paralelo a los percentiles pedidos; `gasUsedRatio` > 1 imposible (por diseño: gasUsed ≤ gasLimit), y ratios sostenidos ~0.95+ (bloque cerca del gas limit = ~2× el target) aplican el paso máximo +12.5% — señal para el ceiling del bundle, no para improvisar el bid.

**Dinámica del base fee (EIP-1559):** el target es la mitad del gas limit (15M al desplegarse EIP-1559; el gas limit de mainnet se ha subido varias veces desde entonces — el mecanismo 1/8 no cambia); el base fee del siguiente bloque sube/baja proporcionalmente al desvío del target, con paso máximo 1/8 (±12.5%). Consecuencia práctica: `maxFeePerGas` = baseFee_actual × 1.125 para 1 bloque de margen; × 1.125² ≈ 1.27 para 2 bloques de incertidumbre; × 1.125³ ≈ 1.42 para 3. Un bundle con maxFee ajustado sin margen muere por base fee cuando el bloque previo se llena.

**Bidding de priority fee:** el tip efectivo pagado es `min(maxPriorityFeePerGas, maxFeePerGas - baseFee)`. En bundles, el coinbase transfer + tip compiten contra otros bundles: subestimar el percentil 75 de `reward` histórico = exclusión sistemática; sobreestimar de forma fija = caro en blocks vacíos. Deriva el bid del feeHistory reciente, no de una constante (anti-hardcode, RULE 00).

**Blob fees (EIP-4844):** txs tipo 3 pagan blob-gas aparte (`maxFeePerBlobGas`) con su propio base fee ajustado exponencialmente hacia el target de blobs por bloque (Dencun: target 3 / máx 6; Pectra EIP-7691: target 6 / máx 9); `eth_feeHistory` también lo expone como `baseFeePerBlobGas`. Para arbitrage estándar (txs tipo 0/2 en bundles) el blob fee es costo indirecto apenas relevante; importa si la estrategia mueve datos masivos (da disponibilidad barata vs calldata). Regla: no meter blobs en bundles de searcher sin validar que el builder destino los acepta igual.

**Presupuesto de gas por intento y ceilings:**

```
gas_cost_proyectado = gas_estimado(§15.5) × (baseFee_proyectado + priority_bid)
condición de envío:  gas_cost_proyectado ≤ gas_ceiling_por_intento (config arbx)
                    AND net_profit post-gas ≥ umbral (gate arbx-net-profit)
```

El fee ceiling por intento es un límite de risk, no una meta: si el feeHistory dice que superar el techo es necesario para inclusion, la respuesta correcta es NO enviar (el gate de economics lo rechaza), no subir el techo ad-hoc.

## 15.7 Reorgs: detección, respuesta operativa, finalidad PoS

**Profundidad típica post-merge:** reorgs de 1 bloque ocurren rutinariamente (propositantes desconectados, timing de attestation); 2+ es raro; reorgs profundos son prácticamente inexistentes bajo finalidad. La finalidad llega en ~2 épocas (época = 32 slots × 12s ≈ 6.4 min → ~12.8 min). Los tags RPC `safe` y `finalized` de `eth_getBlockByNumber` dan el anclaje sin calcular épocas a mano.

**Detección (mecánica concreta):**

1. Al recibir newHead N con `hash`, verificar `parentHash == hash(N-1)` que ya conoces. Mismatch → el bloque N-1 que tenías fue reemplazado (o el provider te mintió): re-obtener el bloque N-1 por número y comparar hashes.
2. En la suscripción `logs`, un log con `removed: true` es el retro-aviso directo de que un evento que procesaste ya no existe.
3. Estructura de datos obligatoria: todo estado derivado guarda `(block_number, block_hash)` del bloque que lo causó. Sin `block_hash` guardado, la invalidación por reorg es imposible de hacer correcta.

**Respuesta operativa (en orden):**

```
reorg detectada (depth d sobre head_stable):
 1. Invalidar TODO estado derivado de bloques > punto de fork:
    rutas detectadas, reservas cacheadas de esos bloques, planes pendientes
 2. Re-procesar el rango [fork_point+1 .. nuevo head]: eth_getLogs re-emite
    los logs de la nueva cadena (dedupe por blockHash ya lo cubre)
 3. Re-simular planes pendientes contra el nuevo estado (§15.4) — la matemática
    NO se reutiliza: el estado cambió, el resultado previo es basura
 4. Persistir el incidente (bloque huérfano, profundidad, hash viejo/nuevo)
    — los incidentes del repo exigen registro (docs/incidents/)
```

**Estado interno del proceso:** los planes "pendientes de inclusión" deben escucharse contra el bloque objetivo: si el bloque target se mina y el bundle no está incluido, el plan expira (no reintentar ciegamente — recalculo con estado nuevo). El ciclo replay/debugging del núcleo §9.2 se apoya aquí: el ancla `block_hash` es lo que permite reproducir.

## 15.8 Estado caliente local: reserves desde logs, warm/cold, preload

**Regla base: las reservas se mantienen desde eventos, no con `getReserves` on-demand.** Cada llamada `getReserves` RPC por oportunidad es 1 RTT en el hot-path y además devuelve el estado "ahora", no el del bloque de referencia de tu simulación. El patrón correcto:

- Uniswap V2: el evento `Sync(uint112 reserve0, uint112 reserve1)` se emite en CADA swap/mint/burn que mueve reservas. Su topic0 es `0x1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1`. Cache por pool = último Sync aplicado anclado a `(block_number, block_hash)`.
- Alternativa de lectura (validación, no hot-path): `eth_getStorageAt(pair, "0x8", tag)` — en el pair V2 estándar el slot 8 empaqueta `(reserve0, reserve1, blockTimestampLast)` (112+112+32 bits). Útil para reconciliar caché vs cadena.
- Uniswap V3: NO hay reserves. El evento `Swap` expone `sqrtPriceX96`, `liquidity`, `tick`; el estado relevante para simular es la tick-data del rango activo. El modelo de caché es distinto por diseño: no reutilices la mentalidad V2.

**Preload por bloque del watchlist:** al cerrar cada bloque, prefetch batch (JSON-RPC batch o Multicall3 en `0xcA11bde05977b3631167028862bE2a173976CA11`, `aggregate3((address,bool,bytes)[])` con allowFailure) del estado de las pools del watchlist: code hash, slots de reserves, balances relevantes. Así la simulación del siguiente bloque arranca con CacheDB poblado y ningún fetch remoto en el camino crítico.

**Warm vs cold access (EIP-2929, por qué el orden importa):** primera lectura de una cuenta paga acceso frío (~2600 gas), slot frío ~2100, accesos subsecuentes ~100. Dos consecuencias:

1. En una ruta multi-hop, los pares ya tocados por hops previos quedan warm dentro de LA MISMA transacción: el gas medido de hop k+1 depende de lo que tocó hop k. Medir hops aislados y sumar SOBREestima (asumes todo frío) — medir la ruta completa en revm/anvil refleja el warm real.
2. En simulación in-proc, "warm/cold" tiene una segunda dimensión: el CacheDB frío (sin preload) paga el fetch de red por primer acceso. El preload de arriba convierte el cold path I/O en warm path memoria.

**Higiene de la caché:** tamaño acotado (pools del watchlist), invalidación por reorg (§15.7 — ancla block_hash), y métrica de hit-rate: un hit-rate que cae = el filtro de watchlist está desalineado del flujo real, no un bug de caché.

## 15.9 Operación del plano RPC: health-checks, 429/quota, p99, circuit breaker

**Health-check continuo por provider (cada pocos segundos, no por request):**

```
por provider P:
  eth_blockNumber  → drift(P) = max_block_conocido - block(P)
  eth_syncing      → debe ser false (un provider sincronizando sirve estado viejo)
  latencia medida con reloj monotónico, guardada en histograma por provider+method
healthy(P) = drift ≤ N bloques AND error_rate(5m) < X AND p99(eth_call) < Y ms
```

El drift es el health más honesto: un provider "verde" en uptime pero 3 bloques rezagado envenena decisiones de head y simulación contra estado viejo.

**429 y cuota (backoff con jitter y presupuesto):**

- Al recibir 429: leer `Retry-After` si viene y respetarlo; si no, backoff exponencial con full jitter (`delay = random(0, min(cap, base × 2^attempt))`) — el jitter evita que todos los workers re-sincronicen sus reintentos (thundering herd contra el mismo provider saturado; la lección repetida del repo: golpear un rate-limit con más presión solo genera más 429).
- Presupuesto por provider: asignar un peso de tráfico por provider según su cuota (compute units/credits) y su p99; los reads de backfill (barato, batcheable) van al provider de mayor cuota; el hot-path usa el de menor p99 con failover inmediato. Repartir deliberadamente — 100% en el más rápido es fragilidad, no optimización.
- Paceo preventivo con token bucket (núcleo §3.1, `governor`): limitar por debajo de la cuota declarada para dejar headroom a picos de detección.

**p99 por provider y por método:** histogramas separados — `eth_call` (simulación), `eth_getLogs` (backfill), `eth_sendBundle`/privado (submission) tienen perfiles distintos; un p99 agregado esconde que el provider es bueno para reads y pésimo para el método que te importa. El circuit breaker del núcleo §3.1 se arma sobre ESTOS histogramas: abrir por tasa de error sostenida o p99 degradado > umbral sostenido, con sonda half-open (un request de prueba) antes de restaurar el provider al pool — no restaurar por el mero paso del tiempo.

**Regla anti-fuente-única:** ningún planeamiento de failover cabe en esta referencia: la skill `arbx-rpc-failover-discipline` del repo es la autoridad (orden, quorum, prioridades). Aquí solo el invariante: el sistema debe seguir detectando (degradado si hace falta, fail-honest) con un provider menos — un single-provider setup es un incidente agendado.

**Tabla de señal → respuesta (runbook mínimo del plano RPC):**

| Señal observada | Diagnóstico probable | Respuesta |
|---|---|---|
| `newHeads` silencioso > 15s pero otros providers avanzan | Stream WS muerto (proxy, idle-timeout) | Reconnect + reconstrucción (§15.2) — el provider queda en cuarentena para head |
| Hueco en secuencia de bloques (N → N+2) | Provider perdió un push | `eth_getLogs`/`eth_getBlockByNumber` del rango, dedupe, seguir |
| Log con `removed: true` | Reorg confirmada | Protocolo §15.7 completo |
| 429 con `Retry-After` | Cuota de burst | Respetar header; revisar presupuesto por provider (§15.9 arriba) |
| 429 en cadena (todos los workers) | Cuota agotada del ciclo | Backoff con jitter + degradar cadencia de reads no críticos; NO añadir presión |
| `eth_call` devolviendo estado inconsistente entre providers | Drift / provider rezagado o indexando | Comparar `eth_blockNumber`; excluir al rezagado de head/simulación |
| p99 de un método degradado pero error-rate 0 | Degradación silenciosa (red del provider) | Bajar peso del provider, sondear half-open antes de restaurar |
| `eth_syncing: true` en un provider "production" | Nodo reiniciado/re-syncando | Sacarlo del pool de reads inmediatamente (estado viejo = decisiones viejas) |

## 15.10 Ladder de ticks V3 local (quoter offline)

**Qué es y por qué existe.** §15.8 estableció que V3 no tiene reserves: el estado que gobierna una quote es `(sqrtPriceX96, tick, liquidity)` + el ladder de ticks inicializados con su `liquidityNet`. Un quoter offline computa el quote de un swap V3 con esa estructura en memoria — sin llamar al `Quoter` on-chain (1 RTT + ejecución de bytecode por quote) y sin pasar por revm. Su rol en el pipeline: filtro y priorizador masivo ANTES del tier 0 de §15.4 — ordenar candidatos por rate esperado y acotar el monto que luego se simulará. El límite honesto de §15.10.4 es innegociable.

### 15.10.1 Snapshot inicial: contabilidad de storage reads y presupuesto

Semillas y estructura, todo vía accessors públicos del pool (eth_call / Multicall3 `aggregate3` batcheado — §15.8):

1. **Semilla de estado actual:** `slot0()` → `(sqrtPriceX96, tick, ...)`; `liquidity()` → liquidez in-range (`uint128`). Son los valores que el swap-step loop consume como estado inicial.
2. **Barrido del tickBitmap:** `tickBitmap(int16 word)` devuelve un `uint256` cuya clave es la palabra del índice COMPRIMIDO. Compresión (TickBitmap del core, aplica SOLO al bitmap): `compressed = tick / tickSpacing` — solo múltiplos del spacing pueden inicializarse (referencia 11 §11.2.1) — luego `word = compressed >> 8` (shift aritmético con signo: ticks negativos caen en palabras −1, −2, …) y `bit = compressed mod 256` en representación positiva (para negativos, `compressed & 0xFF` en complemento a dos la entrega). Una palabra cubre 256 ticks COMPRIMIDOS = `256 × tickSpacing` ticks crudos de la grilla de la fee tier (100→1, 500→10, 3000→60, 10000→200). Asimetría que no hay que pisar: el accessor `ticks(int24)` del punto 3 se direcciona por tick CRUDO — la compresión vive solo en el bitmap. Solo las palabras no-cero importan: cada bit set marca un tick inicializado.
3. **Lectura de cada tick inicializado:** `ticks(int24)` → `(liquidityGross uint128, liquidityNet int128, feeGrowthOutside*, ..., initialized bool)`. Del struct, el ladder SOLO necesita `liquidityNet` (`liquidityGross` sirve para detectar ticks que quedaron en cero y podarlos).

Nota de layout: `tickBitmap` y `ticks` son mappings públicos del pool — para el SNAPSHOT alcanzan sus accessors vía eth_call, sin conocer el layout de storage. El layout (slots crudos) sí hace falta para el preload de revm: ver §15.10.5.

**Contabilidad de reads por pool:** 2 (semillas) + W (palabras barridas) + N (ticks inicializados leídos), con N ≥ bits set y W ≥ palabras no-cero del rango. Un pool blue-chip con LP activo puede tener miles de ticks inicializados: barrer TODO el rango de vida del pool es un backfill (archive, §15.1), no un arranque.

**Presupuesto correcto (ventana acotada + expansión lazy):**

```
arranque:  ventana = [tick_actual - K, tick_actual + K]
           K = cuántos ticks puede cruzar un swap del orden del monto máximo
           del bloque a la liquidez observada (la matemática del paso por tick
           es referencia 11 §11.2.3; la L por rango, referencia 11 §11.2.2)
runtime:   una simulación local que cruza el borde de la ventana → fetch batch
           de las palabras/ticks faltantes, expandir, continuar
backfill:  el resto del ladder se completa offline fuera del hot-path,
           solo si la pool justifica el costo en compute units (§15.9)
```

El arranque de una pool NUEVA del watchlist es el evento caro (se paga en cuota); el mantenimiento (§15.10.2) es gratis — llega por los logs ya suscritos. Ancla OBLIGATORIA del snapshot: `(block_number, block_hash)` — sin ancla no hay invalidación por reorg (§15.7).

### 15.10.2 Mantenimiento por eventos

Tres eventos del pool V3 gobiernan el ladder (firmas reales del contrato):

- `Mint(sender, owner, tickLower, tickUpper, amount, amount0, amount1)` — `amount` (uint128) es la liquidez de la posición, el único campo que alimenta el ladder. Efecto: `liquidityNet[tickLower] += amount; liquidityNet[tickUpper] -= amount` y `liquidityGross[ambos bordes] += amount`; si `liquidityGross` cruza 0→positivo en un borde, el bit del bitmap de ese tick se ENCIENDE (el `flipTick` del core). Si el rango `[tickLower, tickUpper)` envuelve el `tick` vigente: `liquidity += amount`.
- `Burn(owner, tickLower, tickUpper, amount, amount0, amount1)` → mismo `amount`, signo invertido en TODO: `liquidityNet[tickLower] -= amount; liquidityNet[tickUpper] += amount`, `liquidityGross[ambos] -= amount`; el bit del bitmap se APAGA cuando `liquidityGross` llega a 0 (última posición retirada de ese tick); resta de `liquidity` si el rango está in-range.
- `Swap(sender, recipient, amount0, amount1, sqrtPriceX96, liquidity, tick)` → el evento trae el estado POST-swap. Regla: NO derives — SOBRESCRIBE la semilla con `(sqrtPriceX96, liquidity, tick)` del evento. El ladder subyacente NO cambia por un Swap: solo cambia qué parte de él está in-range.

Dos reglas de aplicación: (1) **orden** — aplicar en orden de `logIndex`: el test in-range de un Mint/Burn se evalúa contra el `tick` vigente EN ese punto de la secuencia (el de la semilla o del último Swap ya aplicado), no contra un estado final. (2) **alcance** — SOLO estos tres eventos alimentan el ladder; el resto de eventos del pool se filtra por topic0 o se ignora al aplicar: `Collect(owner, recipient, tickLower, tickUpper, amount0, amount1)` retira tokens acumulados (mueve saldos ERC-20, NO toca `liquidityNet`/`liquidity`/bitmap — la confusión clásica), `Flash`, `SetFeeProtocol`, `IncreaseObservationCardinalityNext`.

El cruce de ticks durante la quote local (cuándo sumar/restar `liquidityNet` según dirección, cómo avanza el precio tick a tick) es exactamente el swap-step loop de la referencia 11 §11.2.3 — la derivación vive ahí y NO se repite aquí. Esta sección aporta la estructura de datos que lo alimenta y su ciclo de vida.

### 15.10.3 Invalidez y rebuild

| Causa | Detección | Respuesta |
|---|---|---|
| Reorg | `removed: true` en logs / mismatch `parentHash` (§15.2) | Rebuild desde checkpoint: descartar eventos del rango reorgueado, re-emitir `eth_getLogs` del hueco, dedupe por `(blockHash, logIndex)` (§15.7) |
| Evento incoherente | Mint/Burn sobre tick no inicializado en el ladder, o `liquidity` que quedaría negativa | El ladder divergió (evento perdido o arranque incompleto): REBUILD completo de la pool, no patch ad-hoc — el patch esconde drift acumulado |
| Pool fuera de watchlist | eviction del watchlist | Drop del ladder (higiene de §15.8); re-snapshot desde cero si re-entra |
| Stale | `head_stable` avanzó K bloques sin eventos de la pool | Reconciliación barata: 1 read `slot0()` + `liquidity()` contra la semilla; mismatch → rebuild |

**Invalidación en el grafo de rutas:** un Mint/Burn no solo actualiza el ladder — invalida la fila/arista de esa pool en el grafo de descubrimiento (capacidad y rate efectivo cambiaron). La invalidación selectiva por eventos está especificada en la referencia 12 §12.6.1; el ladder de aquí es la fuente que alimenta esos nuevos pesos.

### 15.10.4 Límite honesto: cache, no veredicto

El ladder local es una CACHE para filtrar y priorizar. El veredicto económico sigue siendo la simulación revm contra el bytecode real del pool (tier 0, §15.4 / núcleo §1.2) — jamás la aritmética local del ladder. La divergencia entre quote offline y resultado revm NO es ruido a silenciar: es la señal de que el ladder está viejo o mal mantenido (o de reorg), y dispara el rebuild de §15.10.3. Esta jerarquía es la doctrina del archivo aplicada a V3.

### 15.10.5 Puente al preload del CacheDB (§15.11)

El ladder no vive solo para el quoter local: cuando la ruta V3 pasa a revm, el bytecode del pool lee SU mapping `ticks` durante la ejecución real del swap. El preload del CacheDB (§15.11.2 / §15.8) debe entonces incluir los slots de storage de esos ticks — computados con la misma fórmula de slots de mappings del EVM y con la ventana del ladder como guía de qué slots están calientes. Doble uso honesto: la MISMA estructura alimenta (1) el filtro offline de esta sección y (2) la lista de slots a precargar para revm. Un swap en revm que fuerza fetch de red a mitad de ejecución significa que la ventana del preload era más chica que la del ladder, o que el ladder estaba desactualizado — en ambos casos, bug de hidratación (§15.11.3), no ruido.

## 15.11 Receta de integración revm contra la versión pineada

**El problema que resuelve.** revm es la pieza del tier 0 (§15.4) con mayor superficie móvil: el crate reorganiza módulos, builders y tipos entre versiones — incluso entre releases menores. La política del repo es port-with-validation (FUSILE): portar con validación, nunca copy ciego. Esta receta convierte esa política en pasos verificables.

### 15.11.1 PASO 0 — la versión EXACTA del lockfile, antes de cualquier firma

```
grep -A 1 '^name = "revm"' backend/Cargo.lock
```

A la fecha de esta referencia el workspace pinea `revm 42.0.1` (`revm-primitives 42.0.0`; declarado `revm = "42"` en `backend/Cargo.toml`, consumido por `simulator-v2` y `prioritization-spine` vía workspace). Ese dato es una fotografía: el LOCKFILE es la fuente de verdad en el momento de leer, no este texto — corre el grep siempre. Con la versión exacta en la mano: abrir docs.rs de ESA versión (docs.rs sirve la doc por versión publicada; "latest" es otra página) o generar la doc del lockfile local con `cargo doc --open -p revm`. Para resolver el grafo: `cargo tree -i revm` (quién depende, con qué versión resuelta). Ojo semver: `revm = "42"` admite upgrades compatibles — un `cargo update` puede mover el patch y con él firmas: re-verificar tras CUALQUIER cambio del lockfile, no solo al editar el manifest.

Guard de revisión: un PR que toque simulación debe mostrar el diff del lockfile para `revm`/`revm-primitives` — un bump silencioso de versión invalida toda firma previamente verificada.

### 15.11.2 Pipeline general: hidratar → contexto pineado → ejecutar → diff

Patrón a nivel de concepto. Tipos reales del crate VERIFICADOS contra 42.0.1 en el código vivo del workspace: `revm::database::CacheDB`, `revm::context::{BlockEnv, TxEnv}`, `revm::primitives::hardfork::SpecId`, y el builder `Context::mainnet().with_db(...).with_block(...)` terminado en `build_mainnet()` (`simulator-v2/src/revm_runner.rs` y `simulator-v2/src/sequence_runner.rs` son la referencia viviente). Las demás firmas se validan contra la versión pineada, no se citan de memoria:

1. **Hidratar la base de estado.** Fuente A: proveedor de fork — revm puede usar un endpoint JSON-RPC como fuente de estado: en 42 existe como feature aparte (`alloydb` del crate `revm-database`: tipo `AlloyDB` con el bloque pineado por `BlockId` AL CONSTRUIR la DB). El workspace NO la habilita (features de `backend/Cargo.toml`: solo `std`, `optional_no_base_fee`) — usa su propio `LazyRpcDatabase` (`prioritization-spine/src/lazy_db.rs`, impl del trait `Database` sobre un provider ethers Ws); valida el nombre del módulo contra la versión pineada antes de asumir. Fuente B (hot-path): `CacheDB` pre-cargado — prefetch batch (§15.8) de los slots calientes del watchlist vía `eth_getStorageAt`/Multicall3, más `eth_getCode` de los contratos de la ruta y balance/nonce de las cuentas, insertados como overlay del CacheDB. Para mappings (balanceOf de los tokens, `ticks` del ladder de §15.10) el slot se computa con la fórmula estándar de slots de mappings del EVM (keccak sobre clave y slot-base empaquetados); el slot-base NO se adivina: se verifica con `prestateTracer` diffMode (§15.5) sobre una tx real que toca ese mapping.
2. **Construir el Evm con contexto pineado.** BlockEnv con número/timestamp/base fee del bloque de referencia (mismo bloque ⇒ mismo resultado, §15.4), spec de la forquilla activa, TxEnv de la ruta (caller, `to`, calldata, valor, gas). El mecanismo es el builder del crate (`with_db` + spec, como §15.4); la firma exacta se toma de la doc de la versión pineada (PASO 0).
3. **Ejecutar.** El resultado expone el outcome (éxito / revert con returndata — decodificar según §15.5), el gas usado y el estado post-ejecución.
4. **Extraer el diff de estado.** El conjunto de cuentas/slots tocados por la ejecución ES la verdad de la ruta (§15.4): balances del flash loan repagados, tokens recibidos. Slots no anticipados por el modelo → fallar honesto (R8), nunca asumir benigno.

Un miss de CacheDB durante la ejecución con fuente B es un bug de hidratación (la cuenta/slot debía estar precargada): en producción, el fallback a red en el hot-path es la fuga de latencia que §15.8 quiere evitar. El preload debe cubrir el 100% de los touches del watchlist; el miss se mide y se alerta, no se tapa.

### 15.11.3 Verificación cruzada obligatoria contra anvil fork pineado

Ground truth periódico (job de calibración, NO hot-path):

```
mismo plan + mismo bloque:
  1. sim in-proc (revm de la versión del lockfile, CacheDB hidratado)
  2. anvil --fork-url $RPC --fork-block-number <mismo bloque> + cast call
  3. comparar: outcome, returndata, gas y (si aplica) storage post
```

Divergencia in-proc vs anvil = uno de cuatro bugs, en orden de probabilidad: (1) slot/cuenta faltante en la hidratación (anvil resuelve sus misses contra el upstream; tu CacheDB no), (2) código de contrato no precargado, (3) spec de forquilla equivocada, (4) la versión de revm difiere de la que el fork ejecuta. La divergencia NO se silencia: se diagnostica contra una de esas cuatro causas y se registra. El bloque pineado se registra SIEMPRE junto al resultado (regla de §15.4); en CI esto se congela como test de regresión con vector fijo (núcleo §4.3).

Tabla de síntomas → causa probable (drill-down de los cuatro bugs):

| Síntoma de divergencia | Causa probable |
|---|---|
| revm revierte, anvil sucede | Slot/cuenta faltante en hidratación (anvil resuelve contra upstream) |
| Mismo outcome, gas distinto | Spec equivocada, o warm/cold distinto por preload incompleto de touches (§15.8) |
| Mismo outcome, balances post distintos | Mapping slot mal computado (slot-base adivinado) — verificar con prestateTracer diffMode (§15.5) |
| Diverge solo tras `cargo update` | Versión de revm movida por semver — re-ejecutar PASO 0 |

Cadencia operativa: el job de verificación cruzada es backfill-barato y batcheable — va al provider de mayor cuota, no al de menor p99 (presupuesto por provider, §15.9), desacoplado del hot-path.

### 15.11.4 Semántica de selección de estado y gas (los dos drifts silenciosos)

**Selección de estado.** Qué bloque sirve cada fuente lo decide la DB, NO el BlockEnv: el trait `Database` de revm 42 (`basic`/`storage`) no recibe número de bloque en los fetches — `AlloyDB` pinea su `BlockId` al construir, `LazyRpcDatabase` decide el bloque en cada fetch, y `BlockEnv.number` solo lo ve la ejecución. La consistencia `BlockEnv.number == bloque del snapshot de la DB` es un invariante que el integrador mantiene a mano (simulator-v2 lo fija explícitamente; ver el doc-comment «BlockEnv consistency (MAJOR #6)» en `simulator-v2/src/lib.rs`). Con CacheDB (fuente B), el estado hidratado es el que precargaste — del bloque que marcaste al hidratar. Mezclar bloques (hidratar contra N y simular con BlockEnv N+k) produce divergencia silenciosa que ningún error va a reportar. Regla: un CacheDB se hidrata contra un bloque y muere con ese bloque — el ciclo de vida del cache es por-bloque; el preload por bloque de §15.8 es la implementación de esa regla.

**Gas.** El gas medido por revm es el canon del EVM para la traza ejecutada — SIN el margen que algunos clientes añaden a `eth_estimateGas` (§15.5). Consecuencia práctica: usa el gas de la simulación in-proc como cota inferior y aplica el margen al presupuestar contra el tier 1 (§15.6), nunca al revés. Y el warm/cold (EIP-2929) depende de los touches de la MISMA transacción (§15.8): solo el gas de la ruta completa es comparable contra el fork — sumar hops aislados sobreestima.

### 15.11.5 Anti-patrón y regla RULE 00

- **ANTI-PATRÓN central:** escribir el código contra la doc "latest" de docs.rs cuando el lockfile pinea otra. El compile-fix iterativo que sigue ("cambia la firma hasta que compila") es copy ciego con pasos extra — no port-with-validation.
- **Firma no verificable:** si una firma concreta del builder/API no puedes verificarla contra la versión pineada, describe el paso conceptualmente (como hace esta sección), marca CÓMO verificarla — docs.rs de la versión exacta, `cargo doc -p revm` del lockfile local, o grep del código ya integrado (`simulator-v2/src/revm_runner.rs`, `simulator-v2/src/sequence_runner.rs`, `prioritization-spine/src/lazy_db.rs` — la referencia viviente del workspace) — y NO la inventes. Pseudocódigo que parece API real es un mock con disfraz de código (RULE 00).

## GOBERNANZA

Esta referencia es conocimiento de infraestructura read/simulate: nada aquí autoriza flip a live ni broadcast con capital real. El envío de bundles/txs que se menciona queda subordinado a los gates `arbx-paper-trade-first`, `arbx-simulation-mandatory`, `arbx-risk-limits-enforcement`, `arbx-pre-execute-checklist` y a CLAUDE.md §34 (LIVE_MAINNET gated, default-deny en el terminus `relays-client`). Vectores contra terceros (frontrun y afines) solo se tratan como riesgos a mitigar — ver `arbx-mev-ethics-gate`.
