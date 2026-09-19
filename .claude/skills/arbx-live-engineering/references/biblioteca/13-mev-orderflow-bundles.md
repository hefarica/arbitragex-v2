# 13. MEV, ORDERFLOW Y EJECUCIÓN POR BUNDLES

> Archivo 13 de la Parte II de la biblioteca (secciones §13.1-§13.15). Extiende el núcleo v1.0.0 (§1-§10, ver
> `00-nucleo-ingenieria-mev.md`) hacia la capa de ejecución real: supply chain PBS,
> bundles, orderflow privado, bidding y latencia. No repite el núcleo: lo cita.

CUÁNDO CARGAR ESTA REFERENCIA: implementar o depurar el envío de bundles (`eth_sendBundle`, `eth_sendPrivateTransaction`); integrar relays/builders o MEV-Share; diagnosticar bundles no incluidos o inclusiones silenciosas; diseñar el bidding/pricing del bundle y el margen del searcher; modelar probabilidad de inclusión y envío multi-builder; defender transacciones propias contra sandwich/frontrun; razonar sobre reorgs, finalidad PoS y re-simulación por bloque; optimizar latencia searcher→builder; gestionar nonces del signer bajo concurrencia (pool por signer, fuente de verdad latest-vs-pending, txs atascadas y replace-by-fee); evaluar cross-domain (Jito/Solana); extender el
motor a L2 rollups (Arbitrum/Base: FCFS+Timeboost, orden por priority fee, fees de
dos pisos L1/L2, finalidad por ventanas).

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Supply chain MEV | searcher → builder → relay → proposer (PBS/MEV-Boost) | El searcher nunca habla con el proposer; el relay es el fiduciario |
| Envío de bundle | `eth_sendBundle` (relay Flashbots, `https://relay.flashbots.net`) | Válido para UN `blockNumber`; re-enviar por bloque objetivo |
| Simulación previa | `eth_callBundle` + REVM local + Anvil fork | Un bundle que revierte sin permiso invalida el bundle completo |
| Pago al builder | `block.coinbase` transfer vs priority fees (EIP-1559) | Los builders ordenan por coinbase diff efectivo por gas |
| Tx privada | `eth_sendPrivateTransaction` (`https://rpc.flashbots.net`) | Inclusión NUNCA garantizada; fallos 100% silenciosos |
| Orderflow privado | MEV-Share (`https://mev-share.flashbots.net`), hints | Subasta: backruns con pago/reparto al originador del flujo |
| Bidding | Subasta de primer precio, valor común | Winner curse: E[profit \| win] < E[profit]; shading obligatorio |
| Defensa sandwich | Slippage estricto + routing privado + auditoría `Sync` logs | Vectores ofensivos SOLO como defensa (ver `arbx-mev-ethics-gate`) |
| Estado/reorgs | Re-simular por cada bloque objetivo; finalidad ~2 épocas | La no-inclusión cambia el estado: nunca reusar un bundle simulado viejo |
| Latencia | Conexiones persistentes + TLS resumption + medición p99 | El ack del relay ≠ inclusión; milisegundos deciden el cutoff del builder |
| Cross-domain | Jito Block Engine (Solana) | Leader schedule determinista, sin mempool público, slots de 400ms |
| L2 rollups | FCFS+Timeboost (Arbitrum) / priority fee (OP-Stack) | Sin PBS ni relays: la subasta §13.1-§13.4 NO aplica (§13.14) |
| Nonces bajo concurrencia | Pool por signer: contador atómico, reserva al construir (antes de firmar), devolución al expirar; signers separados por estrategia/familia de rutas | Nonces crecientes del mismo signer quedan acoplados: el N+1 no aterriza sin el N (§13.15) |
| Tx atascada | Esperar hasta deadline / replace-by-fee mismo nonce con bump 10% default geth, ambos caps (por builder) / rellenar hueco con no-op o abandonar (los posteriores esperan) | `eth_getTransactionCount` "latest" es la base de reconciliación; "pending" es ruido con routing privado (§13.15) |
| Crates oficiales | `ethers-flashbots` (middleware ethers-rs), `mev-share-sse`/`mev-share-rpc`, `artemis`, `revm` | Ver núcleo §8.1 para el middleware FlashbotsMiddleware |

## 13.1 ANATOMÍA DEL SUPPLY CHAIN MEV

La ejecución post-merge es una cadena de custodia de cuatro roles. El searcher no
negocia con el validador: entrega valor a un builder, que compite por el bloque.

```
 searcher ──bundle──▶ builder ──payload+bid──▶ relay ──header firmado──▶ proposer
    ▲                   (construye, ordena,      (fiduciario: firma        (validador,
    │                    simula)                  ciegamente, custodia      firma y
    │                                             el payload)               publica)
    └─── ack / stats / eventos de inclusión ◀──────────────────────────────┘
```

**Roles y alineación de incentivos:**
- **Searcher**: produce bundles (1..N txs atómicas). No paga cuota al relay; paga al
  builder vía coinbase transfer o priority fees. Su ventaja es información + latencia.
- **Builder**: agrega cientos de bundles y txs públicas en un bloque, simula para
  maximizar valor y minimizar riesgo de invalidación. Oligopolio real (pocos builders
  concentran >80% de los bloques MEV-Boost). Su corte de construcción termina ANTES
  del inicio del slot del proposer (cutoff privado, típicamente cientos de ms antes).
- **Relay**: custodia el payload del builder, presenta solo el header (blinded) al
  proposer, verifica que el pago prometido esté en el payload y libera el cuerpo solo
  tras recibir el bloque firmado (`getHeader` / `submitBlindedBlock`). Punto único de
  falla y de censoring: filtrar aquí es barato y silencioso.
- **Proposer**: elige el header con mayor bid. Con MEV-Boost delega la construcción y
  nunca ve el contenido antes de comprometerse (PBS: proposer-builder separation).

**MEV-Boost/PBS** separa QUÉ se incluye (builder) de QUIÉN finaliza (proposer). Para el
searcher la consecuencia práctica: el "mercado" al que le vende es el builder, no la
chain. Cada builder es una subasta de primer precio independiente → envío multi-builder
(§13.9) y trazabilidad por builder, no agregada.

## 13.2 TIMELINE DEL SLOT DE 12s: POR QUÉ LOS MILISEGUNDOS DECIDEN

Slot = 12s. El deber (duty) del proposer cubre el slot completo, pero el consenso exige
que el bloque esté firmado y propagado antes de que los attesters voten en t+4s. Todo el
margen real del searcher vive en el primer tramo:

```
 t=-12s..0    Builders ya construyen contra el slot N+1 (bundles del slot N en vuelo)
 t=0          Inicia el slot. El validator client emite getHeader a los relays
 t=0..~1s     Relays devuelven headers con bids; el vc elige el mejor y firma
 t=1..3s      submitBlindedBlock del proposer al relay; este valida y revela el
              payload; el bloque firmado se propaga
 t=4s         Attestations deadline: sin bloque visible ≈ slot perdido (miss)
 t=4..12s     Attestations se agregan; el siguiente builder ya trabaja con este estado
```

Consecuencias operativas:
1. **El cutoff del builder, no el tuyo, es el deadline.** Un bundle entregado a t=1s
   del slot objetivo ya llegó tarde para la mayoría de builders: su bloque estaba
   cerrado o enviado al relay. Objetivo práctico: bundle en manos del builder ANTES del
   inicio del slot objetivo (t<0), idealmente nada más detectar en el slot previo.
2. **Bundles tardíos** no producen error: el builder los descarta en silencio o los
   considera para un bloque futuro SOLO si seguirá proponiendo (raro). El sistema
   honesto asume descarte y re-envía para el siguiente bloque (§13.11).
3. **El proposer lookahead** (asignación conocida con ~1 epoch ≈ 6.4 min de antelación)
   permite pre-dirigir bundles solo a los builders que sirven al próximo proposer —
   poco práctico dada la concentración; el default es fan-out a todos.
4. Missed slots (proposer offline) retrasan todo el pipeline: el bloque N+1 se construye
   sobre N-1; los bundles dirigidos al slot perdido deben re-simularse (el estado base
   cambió de rama).

## 13.3 BUNDLES FLASHBOTS: `eth_sendBundle` Y SIMULACIÓN PREVIA

Un bundle es una lista ordenada de txs firmadas con semántica todo-o-nada DENTRO de un
bloque. Formato real del request al relay (`https://relay.flashbots.net`, autenticado
con header `X-Flashbots-Signature: <address>:<sig>` firmando el body con una clave
efímera que no necesita fondos):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "eth_sendBundle",
  "params": [{
    "txs": ["0x02f8b00184...raw-signed-tx", "0x02f8b00184..."],
    "blockNumber": "0x10f5c8",
    "minTimestamp": 0,
    "maxTimestamp": 1789452000,
    "revertingTxHashes": ["0x<tx-hash-permiteda-a-revertir>"],
    "replacementUuid": "e2f1d6a1-6e0f-4c8e-9ab7-2d5c1f0a3b44"
  }]
}
```

**Semántica exacta de cada campo:**
- `txs`: txs firmadas y serializadas (hex `0x...`), en ORDEN de ejecución. EIP-1559 o
  legacy; blob txs (EIP-4844) NO permitidas en bundles. La primera tx determina la
  account que paga gas; los nonces deben ser consecutivos y coherentes con la cuenta.
- `blockNumber`: hex del bloque objetivo. El bundle vale para ESE bloque. Inclusión
  multi-bloque NO existe: si no entra, re-enviar (§13.11).
- `minTimestamp` / `maxTimestamp`: ventana unix en segundos que el builder debe
  respetar al elegir el timestamp del bloque. Útil para evitar inclusión en bloques con
  timestamps fuera de rango (ej: expiración de quote de flash loan).
- `revertingTxHashes`: hashes de txs del bundle a las que se PERMITE revertir. Cualquier
  otra tx que revierta invalida el bundle completo (el builder lo descarta; no arriesga
  su bloque). Patrón: lista vacía para arbitraje atómico puro; el hash del swap de
  exploración si tu estrategia tolera fallo parcial.
- `replacementUuid`: UUIDv4 opcional que habilita reemplazo/cancelación posterior (§13.5).
- Respuesta: `{ "bundleHash": "0x..." }` — SOLO acuse de recibo. No es promesa de
  simulación ni de inclusión.

**Simulación previa obligatoria (fail-loud antes de enviar):**
1. REVM local contra estado reciente (ver núcleo §1.2 y §9.2) — descarta la mayoría.
2. `eth_callBundle` contra el relay para validar el estado exacto del bloque:
```json
{
  "method": "eth_callBundle",
  "params": [{
    "txs": ["0x02f8b001..."],
    "blockNumber": "0x10f5c8",
    "stateBlockNumber": "latest"
  }]
}
```
   La respuesta incluye por-tx `coinbaseDiff`, `ethSentToCoinbase`, `gasFees`,
   `totalGasUsed` y `stateBlock`: es la fuente para el pricing (§13.4) y para detectar
   que el `stateBlock` ya rotó (re-simular).
3. Anvil fork como contraste contra estado real cuando la ruta toca contratos nuevos.

**Consulta de estado post-envío**: el relay expone `flashbots_getUserBundleStatsV2`
(params `{ blockNumber, bundleHash }`; campos `isSimulated` / `sentToBuildersAt`,
consultable ~20 bloques alrededor del target) y `flashbots_getBundleReceiptsV2`
(resultado de inclusión). Ausencia de datos ≠ error: es el caso normal de descarte
silencioso.

## 13.4 PRICING DEL BUNDLE: REFUNDS, COINBASE TRANSFER Y MARGEN

El builder maximiza lo que le pagan por gas consumido. Dos mecanismos de pago:

| Mecanismo | Cómo funciona | Costo marginal | Nota |
|---|---|---|---|
| `block.coinbase` transfer | CALL con value al coinbase dentro de tu tx (opcode `COINBASE` 0x41 para leerlo) | ~9000 gas por CALL | Monto íntegro al builder; snapshot del pago en la propia tx |
| Priority fees (EIP-1559 tip) | `maxPriorityFeePerGas` elevado en las txs del bundle | 0 extra | El builder recibe `tip × gasUsed`; dependes del orden/agregación del builder |

Regla práctica del mercado: el bid efectivo del bundle ≈
`(coinbase_transfer + priority_fees) / gas_used`. El builder ordena por esa densidad; a
igual densidad gana quien llega antes (latencia, §13.12). El refund en ETH es el mecanismo
dominante en arbitraje atómico porque es verificable en la propia simulación.

**Estructura del margen del searcher** (extiende el P&L del núcleo §1.2):
```
gross_profit     = output_amount - input_amount - flash_loan_fee - protocol_fees
bid_to_builder   = coinbase_transfer + priority_fees_total
net_searcher     = gross_profit - gas_cost(signer) - bid_to_builder
```
El bid NO es un costo opcional: sin bid no hay inclusión competitiva. La decisión de
bid es una subasta de primer precio (§13.9): bid alto sube win-rate y hunde margen; bid
bajo al revés. Regla de parada dura: `net_searcher < min_profit_threshold` → no enviar
(gate `arbx-net-profit-gate`), incluso si `gross_profit` luce bien.

**Winner's margin**: en steady state la competencia empuja el bid hacia ~90-99% del
gross profit en rutas públicas populares. El edge sostenible viene de (a) orderflow
privado (MEV-Share, §13.7), (b) latencia inferior, (c) rutas/estrategias menos
transitadas. Si tu simulación asume bids bajos para "ganar", el modelo miente.

## 13.5 CANCELACIÓN, REEMPLAZO Y CUOTAS DEL RELAY

- **`eth_cancelBundle`**: cancela por `replacementUuid` (el UUIDv4 usado al enviar; la
  API clásica también acepta `bundleHash` + `blockNumber`). Cancelar es best-effort:
  si el builder ya lo integró
  al candidato, puede aún incluirse. Cancelar SIEMPRE que cambies de opinión sobre un
  bundle con nonces reutilizables — evita que dos bundles con nonces solapados compitan
  entre sí y se invaliden mutuamente.
- **Reemplazo atómico**: nuevo `eth_sendBundle` con `replacementUuid` igual al original
  y `replacingTxHashes: ["0x<hash-de-tx-del-bundle-anterior>"]` (+ `droppingTxHashes`
  para solo eliminar). El relay autentica el reemplazo contra la misma signing key del
  envío original. Caso de uso: sube el bid cuando detectas competencia, sin esperar a
  que expire el bundle viejo.
- **Cuotas y rate-limits**: el relay limita bundles por segundo por auth key y el
  endpoint Protect limita txs pendientes por origen; los límites exactos se negocian
  con Flashbots y cambian. Operativa: tratar todo `429` con backoff exponencial +
  jitter (misma doctrina que la memoria QUOTA-BACKOFF del repo y la skill
  `alchemy-rpc-robust-integration`); NUNCA reintentar en ráfaga contra el relay.
- **Idempotencia**: el mismo bundle re-enviado al mismo bloque es idempotente por
  contenido (`bundleHash` derivado de las txs). El reemplazo de bid cambia el contenido
  → nuevo `bundleHash`: registrar ambos en el ledger de ejecuciones (núcleo §3.2) para
  no doble-contar P&L.

## 13.6 TRANSACCIONES PRIVADAS: `eth_sendPrivateTransaction`

Para txs propias (sin bundle, sin backrun ajeno) que no deben exponerse al mempool
público — defensa primaria contra sandwich (§13.10):

```json
{
  "method": "eth_sendPrivateTransaction",
  "params": [{
    "tx": "0x02f8b001...raw-signed-tx",
    "maxBlockNumber": "0x10f5d0",
    "preferences": {
      "hints": ["calldata"],
      "builders": ["flashbots"]
    }
  }]
}
```

- `maxBlockNumber`: el relay deja de intentar incluirla después de ese bloque. Úsalo
  SIEMPRE: sin él, una tx con nonce atascado bloquea la cuenta indefinidamente.
- `preferences.hints`: qué revelar a terceros vía MEV-Share (matriz de §13.7). Sin
  hints, solo el hash es visible.
- `preferences.builders`: allowlist de builders; vacío = fan-out a todos los del relay.
- Cancelación: `eth_cancelPrivateTransaction` con `{ "txHash": "0x..." }` (best-effort).

**La inclusión NUNCA está garantizada.** No hay error, no hay receipt, no hay callback
de rechazo: si ningún builder la incluye, la tx expira en silencio en `maxBlockNumber`.
Patrón honesto (R8 del repo): una tx privada se contabiliza como `EXPIRED` cuando el
head de la chain pasa `maxBlockNumber` sin receipt — nunca como "pendiente" perpetua.
`None = no computado`; la ausencia de evento es el fallo.

## 13.7 MEV-SHARE: SUBASTAS DE ORDERFLOW

MEV-Share convierte el orderflow de usuarios en una subasta programática: el usuario
(wallet/DSL) envía su tx vía MEV-Share con hints seleccionables; los searchers ven los
hints en tiempo casi real (SSE), simulan backruns y pujan por incluirse después de esa
tx. El originador del flujo recibe una parte del bid.

**Matriz de hints (ambos lados de la API):** `tx_hash` (siempre presente),
`calldata`, `contract_address`, `function_selector`, `logs`, `default_logs`. Menos
hints = más privacidad, menos bids. `calldata` completo maximiza la puja pero expone la
intención completa (frontrunneable por builders honestos-deshonestos por igual).

**Flujo del searcher sobre el endpoint `https://mev-share.flashbots.net`:**
1. Suscribirse al stream de eventos de orderflow (crates oficiales `mev-share-sse` /
   `mev-share-rpc` de Flashbots para consumirlo tipado).
2. Por cada evento: re-simular el estado POST-tx del usuario (la tx objetivo aún no
   ejecutó) con REVM; detectar residual capturable (precio fuera de equilibrio,
   liquidación parcial, etc.).
3. Enviar un bundle `mev_sendBundle` cuyo `body` referencia la tx objetivo POR HASH
   (`{ "hash": "0x..." }` — no puedes serializar la tx de un tercero) seguida de tus
   backruns firmados (`{ "tx": "0x..." }`), con ventana de inclusión
   `inclusion: { block, maxBlock }`.
4. Pago al originador: transferencia directa de ETH al address del originador dentro
   de tu backrun. El nodo MEV-Share simula tu bundle y solo lo reenvía a los builders
   si esa condición de pago se cumple: el reparto lo audita el protocolo, no tu código.

**Reglas de inclusión**: el bundle compite como cualquier bundle en los builders, PERO
si la tx objetivo expira o se cancela, tu bundle muere con ella (dependencia no
atómica entre participantes, ver §13.11). Modela el win-rate de MEV-Share por separado:
es sistemáticamente menor al de bundles independientes.

Para ArbitrageX, MEV-Share es doblemente relevante: como COMPRADOR de orderflow
(backruns legítimos, §13.8) y como VENDEDOR si el sistema origina flujo propio con hints
calibrados.

## 13.8 ESTRATEGIAS LEGÍTIMAS DEL SEARCHER

Marco ético primero: estrategias donde el searcher provee una función de mercado
(corregir precios, cerrar posiciones insolventes, proveer liquidez puntual) y compite
por velocidad/bid — NO estrategias que extraen de usuarios no informados (eso es §13.10 y
la skill `arbx-mev-ethics-gate`).

1. **Backrun de liquidaciones**: detectar la tx de liquidación en mempool/MEV-Share,
   simular el estado POST-liquidación y capturar el desequilibrio residual con un swap.
   Sin fricción para el liquidante: tu tx va DESPUÉS. Riesgo: otros backrunners → el
   residual se subasta (§13.9).
2. **Backrun de arbitrajes ajenos**: la tx de un searcher deja el pool ligeramente
   fuera de equilibrio; el segundo arbitraje cierra la brecha. Margen fino, volumen
   alto, pura guerra de latencia.
3. **Arbitraje atómico multi-DEX**: todo el ciclo en UNA tx (flash loan → hops →
   repay). Atomicidad elimina el riesgo de inventario entre bloques. Es la topología
   canónica del repo (núcleo §2); aquí la capa nueva es el BID, no la ruta.
4. **JIT liquidity (Uniswap V3)**: mint de un rango estrecho justo antes de un swap
   grande detectado y burn inmediato después; cobras fees del swap sin overnight.
   Riesgo: si el swap no se incluye, tu LP queda expuesto (inclusión no atómica, §13.11).
5. **Backrun propio desde MEV-Share**: convertir orderflow de terceros en señal de
   desequilibrio (§13.7). Menos competencia que el mempool público.

Patrón común: TODO se decide en simulación contra estado post-tx-ajena. La detección
sin re-simulación post-estado es la fuente #1 de bundles perdedores (llegan, se
simulan, pierden contra la realidad ya cambiada).

## 13.9 PROBABILIDAD DE INCLUSIÓN: BIDDING, WINNER CURSE, MULTI-BUILDER

La inclusión es una subasta de primer precio con valor común: el valor del bundle es
el mismo para todos los competidores, pero cada uno lo estima con ruido.

**Winner curse**: condicionado a ganar, tu estimado estaba inflado:
`E[profit | win] < E[profit]`. Si ganas sistemáticamente bundles cuyo profit realizado
< profit simulado, no tienes mala suerte: tienes overbid. Corrección de shading: bid =
fracción del profit estimado, con la fracción calibrada por builder y por estrategia
contra el historial de win-rate y profit realizado (PostgreSQL `executions` del núcleo
§3.2: `status`, `gas_used`, `effective_gas_price` por builder son el dataset).

**Modelo operativo:**
```
p(builder) = win-rate histórico del builder para esta strategy_kind y hora
EV(bid)    = p × (gross_profit_est − bid) − (1−p) × costo_oportunidad(≈0)
bid*       = argmax_bid EV  →  bid* < gross_profit_est  SIEMPRE (shading)
```
Señales de calibración: (a) win-rate por builder decae → sube bid en ese builder o
redirige; (b) profit realizado marginal tras subir bid → estabas bajo-pujando; (c) ganó
otro con el mismo target (visible en el bloque: tu tx NO está, la oportunidad sí se
ejecutó) → lee el tx-trace del ganador y extrae su bid implícito (coinbase transfer /
tip) — auditoría competitiva gratuita con `eth_getBlockByNumber` + receipts.

**Envío multi-builder**: cada builder corre una subasta independiente → mismo bundle a
N builders (relay Flashbots + endpoints directos de builders mayores: titan,
beaverbuild, rsync, flashbots builder; las URLs privadas se negocian/rotan). Reglas:
(a) el bundle es idempotente por contenido — el riesgo de doble-inclusión lo cierran
los nonces (solo un builder puede incluir un nonce dado); (b) deduplicar re-sends con
el `bundleHash`; (c) registrar POR BUILDER el envío, el ack y el resultado para separar
"perdí la subasta" de "el builder nunca me simuló" — diagnósticos distintos (§13.2 vs §13.9).

## 13.10 VECTORES OFENSIVOS — EXCLUSIVAMENTE COMO DEFENSA

> Encuadre obligatorio: lo que sigue describe ataques PARA detectarlos y mitigarlos
> sobre transacciones propias y del sistema. Cualquier uso ofensivo contra terceros
> está prohibido por la skill `arbx-mev-ethics-gate` y no tiene cabida aquí. El
> searcher legítimo de §13.8 no necesita nada de esta sección como arma.

**Sandwich**: par (frontrun-compra, backrun-venta) alrededor de una swap víctima con
slippage alto. Detección sobre TUS txs incluidas:
- Mismo bloque, pool idéntico (mismo par), una tx ajena ANTES y otra DESPUÉS de la tuya
  con sender común entre ambas → sospecha de sandwich.
- Forense barato: `eth_getLogs` del bloque filtrando el evento `Sync(uint112,uint112)`
  de UniswapV2 (topic0 `0x1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1`)
  sobre tus pools objetivo y comparar reserves antes/después de tu posición.
- Síntoma cuantitativo: `amount_out` realizado consistentemente < simulado sin causa
  de gas → tu flow está filtrado.

**Mitigaciones**: slippage estricto (0.1–0.5% en `minOut`/`amountOutMinimum`, nunca el
default de routers de 0.5–1%+); deadline corto; routing privado total
(`eth_sendPrivateTransaction` / builders allowlist, §13.6) para txs que revelan intención;
splits y RFQ donde exista. El slippage estricto convierte el sandwich en intento no
rentable: el atacante asume riesgo de revert sin captura.

**Frontrun puro** (sin backrun): replicar tu intención antes. Misma mitigación:
privacidad del flow. Regla del repo: NUNCA mempool público para ejecución (núcleo §1.2
lo marca último recurso; esta referencia lo eleva a prohibición operativa salvo
diagnóstico explícito).

**Reorg-MEV / time-bandit**: re-minar bloques ya confirmados para robar MEV capturado.
Mitigación: contabilizar P&L como irrevocable solo en `finalized` (§13.11); no construir
decisiones encadenadas (retirar, migrar capital) sobre bloques no finalizados.

**Censoring**: relays/builders que filtran por política (ej. touchpoints con
addresses sancionadas) descartan tu bundle en silencio — indistinguible de perder una
subasta. Mitigación: diversificación multi-builder real (§13.9), medición de ack-rate
por builder, y alerta cuando el ack-rate de un builder cae a cero sin cambio de bid.

## 13.11 REORGS, DEPENDENCIA DE ESTADO Y FINALIDAD PoS

**El bundle es una función del estado, no solo de la intención.** Toda simulación fue
contra un estado `S(block N)`. Si el bundle no entra en N, el mundo N+1 tiene otro
estado (otros swaps ejecutaron, otra liquidación cerró la brecha) → re-simular ANTES de
re-enviar. El ciclo honesto por bloque objetivo:

```
por cada bloque objetivo T (hasta expiry / maxTimestamp):
  1. head := T-1 confirmado (o best)
  2. RE-simular bundle contra estado head          ← SIEMPRE, nunca reusar veredicto
  3. si net_searcher < threshold → cancelar (eth_cancelBundle) y terminar
  4. re-pricing del bid (estado nuevo ⇒ competencia nueva, §13.9)
  5. eth_sendBundle(blockNumber = T)  (multi-builder)
  6. T incluido: check receipt → INCLUDED | EXPIRED(re-enviar) | REORGED(→ abajo)
```

**Inclusión no atómica entre bloques**: atomicidad solo DENTRO del bundle en UN
bloque. Estrategias de dos fases (fase 1 bloque N, fase 2 bloque N+1) cargan riesgo de
leg en cada frontera: la fase 2 puede no incluirse (competencia) o incluirse contra un
estado distinto. La topología del repo resuelve esto por diseño (ciclo completo en una
tx, núcleo §2); si alguna estrategia futura require multi-bloque, el gap de leg debe
modelarse como cola de pérdida condicional, no como certeza.

**Reorgs PoS**: cortas (1–2 slots) y raras pero no imposibles (misses, latencia de
red). Un bundle "incluido" en un bloque reorgueado desaparece: el receipt deja de
existir. Por eso el estado de ejecución en PostgreSQL (núcleo §3.2) distingue
`INCLUDED` (bloque visto) de `RECONCILED` (finalized). **Finalidad ≈ 2 épocas** (2 × 32
slots × 12s ≈ 12.8 min): bajo esa marca, todo P&L es provisional y todo análisis de
win-rate debe re-chequear receipts en `finalized`.

## 13.12 LATENCIA DEL SEARCHER Y CROSS-DOMAIN (JITO)

**Latencia = el único edge que no se puede copiar del código ajeno.** El ciclo a medir
end-to-end por bundle (histogramas p50/p95/p99, por builder):

```
t_mem  → tx detonante vista (WebSocket mempool / SSE MEV-Share)
t_sim  → REVM post-tx simulado OK
t_sign → bundle firmado (EIP-1559, nonces calientes)
t_send → request entregado al builder/relay (TCP write completado)
t_ack  → respuesta del relay (solo "recibido")
t_inc  → recibo en el bloque (o expiración)
```

Optimizaciones de impacto, en orden:
1. **Conexiones persistentes**: una conexión HTTP(S) keep-alive por builder, abierta en
   boot y mantenida con pings — jamás TCP+TLS handshake frío por bundle (1–3 RTT
   perdidos ≈ decenas de ms ≈ la subasta). Reuso de sesión TLS (session resumption).
2. **Pre-resolución de DNS** y pinned IPs del relay/builders; HTTP/2 para multiplexar.
3. **Co-localización**: builders/relays mayoritariamente en Europa (DE/NL). El VPS
   Hetzner del repo ya está en región óptima; no mover el ejecutor lejos de los
   builders por conveniencia operativa.
4. **Signer caliente en memoria**: nonces y gas price cacheados por bloque; firmar sin
   round-trips a disco (Ghost Protocol del repo, §9 de CLAUDE.md).
5. **El ack no es inclusión** (§13.3): el pipeline de telemetría cierra en `t_inc`, no en
   `t_ack`. Alertar si `t_ack - t_send` degrada (relay congestionado) o si el ack-rate
   de un builder cae (censoring/rate-limit, §13.10/§13.5).

**Cross-domain: Jito (Solana) y por qué la física cambia.**
- **Leader schedule determinista**: el líder de cada slot (400ms) se conoce con ~1
  epoch de antelación (~2 días). No hay PBS equivalente ni subasta de relays: el líder
  es el auctioneer y las pujas (tips) van a las tip accounts públicas de Jito.
- **Sin mempool público**: las txs van directas al líder vía QUIC (TPU) o al Jito Block
  Engine, que ordena bundles por tip y los inyecta. Consecuencias: el sandwich de
  mempool clásico no existe en la misma forma; la competencia es por latencia hacia la
  región del líder (tip servers regionales de Jito), y el schedule determinista
  convierte la latencia en un problema de geografía planificable, no de subasta ciega.
- **Bundles Jito**: hasta 5 txs por bundle, atomicidad estilo todo-o-nada del paquete,
  tip como transfer directo (equivalente del coinbase transfer, no de priority fees).
  La API JSON-RPC del Block Engine expone envío y consulta de estado de bundles
  (sendBundle / getBundleStatuses); para el código, el SDK `jito-ts` es la vía
  mantenida. NO portable tal cual a EVM: los mismos conceptos (bid, cutoff, atomicidad)
  con física distinta — modelar win-rate y latencia por separado, nunca reusar
  parámetros calibrados en Ethereum.

## 13.13 LIQUIDACIONES: MECÁNICA DE LENDING COMO ESTRATEGIA DE PRIMER ORDEN

Reparto explícito con §13.8: el punto 1 de esa lista cubre el ángulo MEV/backrun —
detectar la liquidación AJENA ya emitida y capturar el residual post-tx. Esta sección
cubre el otro motor: la mecánica del protocolo de lending y el motor que descubre y
EJECUTA la liquidación propia (ser el liquidante, no el backrunner). Es el cuerpo
técnico del concepto 28 del repo ("Jit Liquidation", L_jit = L_target · e^(−k·t),
`.claude/CLAUDE.md` §2): el valor capturable decae exponencialmente desde el instante
en que la insolvencia se vuelve visible on-chain.

### 13.13.1 Health Factor: matemática y parámetros por protocolo

```
HF = Σ_i (colateral_i × liquidationThreshold_i) / Σ_j deuda_j      [todo en USD]
liquidable ⟺ HF < 1
```

Tres parámetros que NO son lo mismo:

| Parámetro | Qué gobierna | Nota |
|---|---|---|
| `LTV` | cuánto puedes borrowear | NO participa en el cálculo de HF |
| `liquidationThreshold` | fracción del colateral que computa en HF | LTV < threshold SIEMPRE; la brecha es el buffer |
| `liquidationBonus` / penalty | spread extra de colateral para el liquidante | en bps > 10000 (10500 = 5%) |

| Protocolo | Lectura de estado | Trigger | Ejecución | Close factor |
|---|---|---|---|---|
| Aave v3 (`Pool`) | `getUserAccountData(user)` → `healthFactor` | `HF < 1e18` | `liquidationCall(collateralAsset, debtAsset, user, debtToCover, receiveAToken)` | default 0.5 de la deuda repagable |
| Compound V2 (`Comptroller`) | `getAccountLiquidity(user)` → `shortfall` | `shortfall > 0` | `cTokenDeuda.liquidateBorrow(borrower, repayAmount, cTokenCollateral)` | `closeFactorMantissa()` |
| Compound V3 (`Comet`) | `isLiquidatable(user)` | bool ya computado on-chain | `absorb(absorber, address[] accounts)` + `buyCollateral(asset, minAmount, baseAmount, recipient)` | cuenta completa (batch nativo) |

RULE 00: estos parámetros son gobernanza-mutable (Aave `PoolConfigurator.setLtv` /
`setLiquidationThreshold` / `setLiquidationBonus`; Comptroller `_setCloseFactor` /
`_setLiquidationIncentive`) → se leen DEL CONTRATO en boot y al armar cada candidato,
jamás de memoria o constantes hardcodeadas. En Aave, `getUserAccountData` ya devuelve
el threshold agregado (`currentLiquidationThreshold`) y el HF final; con eMode activo
(`Pool.getUserEMode(user)`) los thresholds de la categoría reemplazan a los
per-reserve: misma cuenta, parámetros distintos.

### 13.13.2 Descubrimiento de candidatos: dos vías honestas

Recomputar el HF de TODO el universo de cuentas por bloque es inviable (cientos de
miles de posiciones × reservas × precios). El motor combina dos capas:

```
 COLD — indexer histórico               HOT — recompute dirigido (la vía searcher)
 ┌───────────────────────────────┐      ┌──────────────────────────────────────────┐
 │ eventos indexados por usuario: │      │ watchlist HF_estimado < ~1.3 (mem/Redis) │
 │ Borrow / Repay / Liquidation-  │────▶ │ trigger por cuenta:                      │
 │ Call / Supply / Withdraw →     │      │  (a) evento propio toca la posición      │
 │ snapshot de posiciones         │      │  (b) price-move del colateral o deuda    │
 │ (PostgreSQL, núcleo §3.2)      │      │ → re-leer estado on-chain → si HF<1:     │
 └───────────────────────────────┘      │   build tx + re-sim + bid (§13.3-§13.4)  │
                                        └──────────────────────────────────────────┘
```

La mayoría de las cuentas caen por movimiento de PRECIO del colateral, no por su
propia acción — por eso el trigger (b) es el edge: el indexer frío (propio o The
Graph) solo reconstruye el universo; la detección viva es el hot-set de banda de
peligro. Fail-honest (R8 del repo): rama muerta = razón registrada (`hf_above_one`,
`watchlist_empty`, `stale_position`), nunca un candidato fabricado.

### 13.13.3 La ventana del oráculo: el margen vive en el feed ajeno

La liquidación dispara sobre el oráculo DEL protocolo, no sobre el nuestro. Si nuestro
precio marca −8% y el feed ajeno aún no lo refleja, la cuenta NO es liquidable: el
margen ES la ventana entre el movimiento de mercado y la actualización/lectura del
feed del protocolo.

- Aave: router `AaveOracle` sobre feeds Chainlink por asset. Frescura =
  `latestRoundData().updatedAt` vs el heartbeat/deviation threshold DEL feed
  (config por feed: leer del feed, no asumir — RULE 00). Suscribirse al evento
  `AnswerUpdated` de cada aggregator da el trigger "el feed ya movió" sin polling.
- Protocolos con TWAP interno (e.g. Uniswap V3 `observe(secondsAgos)`): el precio
  arrastra el spot con lag estructural — la ventana dura lo que dura el TWAP; a mayor
  ventana, más competidores convergen (k↑ en L_jit, concepto 28).
- Arquitectura completa de oráculos y frescura de pricing: ver referencia 11 §11.5.

Jugada de ventana: detectar el move en fuentes propias rápidas (CEX/DEX, referencia
11), pre-construir el bundle contra el estado POST-actualización del feed y enviar
para el bloque donde el feed ya movió pero la competencia aún no ejecutó. HF es
función del estado (§13.11): re-simular por cada bloque objetivo SIEMPRE.

### 13.13.4 Ejecución atómica: flash loan → liquidate → swap → repay

El colateral capturado casi nunca es el inventario deseado → se repaga la deuda con
flash loan y se swapea el colateral en la MISMA tx:

```
 [1 tx, todo-o-nada]
  flashLoan(debtAsset, D)               ← Balancer Vault (fee 0) o Aave (premium 0.05%)
  liquidationCall(collateral, debt, user, D', receiveAToken=false)
      D' = min(D, closeFactor × deuda)  ← Aave: 0.5 × deuda
      recibe C = D' × (P_coll/P_debt) × (1 + bonus)
  swap(C → debtAsset, minOut estricto)  ← slippage acotado, ruta optimizada
  repay(flashLoan + fee)
  require(delta > 0)                    ← sin margen: revienta y el bundle muere (§13.3)

 gross        = bonus × D'
 net_searcher = gross − gas − slippage_swap − flash_premium − bid_to_builder
```

Gate idéntico al de §13.4: `net_searcher < min_profit_threshold` → no enviar
(`arbx-net-profit-gate`).

**Por qué la liquidación tardía vale 0**: (a) otro liquidante ya ejecutó y consumió la
fracción liquidable (closeFactor) o la posición completa — el bonus ya se lo llevaron;
(b) si HF siguió cayendo hasta colateral < deuda, la cuenta es bad debt: el bonus
computable no cubre el repago y ejecutar es EV negativo. Es el decay exponencial del
concepto 28: k lo fija cuántos bots observan ese protocolo/asset; ganan los primeros
1–2 bloques y el resto compite por cero. La carrera se gana con ventana (§13.13.3) y
latencia (§13.12), no con reintentos.

**Por qué las cascadas correlacionan**: un mismo price-move hunde todas las cuentas
con colateral común priceado por el mismo oráculo, y cada liquidación swapea ese
colateral (presión vendedora) fabricando el siguiente HF<1. El tratamiento de
exposición correlacionada (colateral común + oracle común como un solo factor) ya está
formalizado en la referencia 18 §18.3 — aplicar tal cual, no re-derivar aquí.

### 13.13.5 JIT-liquidity como lado comprador del colateral

En una cascada, N liquidadores swapean el MISMO colateral por los mismos pools: el
lado comprador captura el volumen. Variante de §13.8 punto 4: mint de un rango
estrecho de Uniswap V3 en el pool colateral→base justo antes de la ola, captura de
fees + price impact favorable, burn inmediato después. Matemática de rangos: skills
`uniswap-v3-concentrated-liquidity-math` / `cfmm-optimal-routing` del repo. Riesgos:
la ola puede no llegar (LP expuesto mientras estás dentro) y la inclusión no es
atómica entre bloques (§13.11) — modelar como posición con leg, no como bundle
garantizado.

### 13.13.6 Encuadre ético y admisión del colateral

Igual que §13.8: liquidar es función de mercado — cerrar posiciones insolventes
mantiene solvente al protocolo y protege a los depositantes; se compite por velocidad
y bid. PROHIBIDO el targeting de personas: la cuenta se descubre por estado y eventos
(nunca por identidad; nada de inducir o presionar al borrower). Admisión del colateral
gateada por `arbx-token-safety-screen` ANTES de armar la tx — colateral tóxico u
honeypot = swap de salida imposibilitado = pérdida total del bonus. Encuadre completo
de vectores y límites: `arbx-mev-ethics-gate`.

## 13.14 EJECUCIÓN EN L2 ROLLUPS: SECUENCIADOR FCFS, TIMEBOOST Y FEES

> Reparto de contenido: esta sección es la FÍSICA de ejecución en rollups canónicos
> (Arbitrum One/Nova; OP-Stack: OP Mainnet, Base, Unichain). Puentes e inventario
> cross-domain ya viven en referencias 17.7-17.9; la física de blobs EIP-4844 en
> 14.9/15.6; la jerarquía contable head_hot/head_stable en 15.2. Mismo patrón que
> Jito en §13.12: aquí la física de ejecución; el porteo de estrategia, en la 17.

**Sin PBS: el secuenciador ES el mercado.** Los rollups canónicos no tienen MEV-Boost,
ni red de relays, ni builders compitiendo por el bloque: un secuenciador único (hoy
centralizado — Offchain Labs, OP Labs, Coinbase respectivamente) recibe, ordena y
soft-confirma. La subasta de bundles de §13.1-§13.3 y el coinbase transfer de §13.4 NO
aplican como mecanismo de inclusión: no hay auctioneer a quien pagarle posición. El
análogo de "ganar el bloque" es llegar antes al endpoint del secuenciador — o comprar
la vía exprés donde exista.

**Ordenación por plataforma** (vigencia verificada 2026-09 contra docs oficiales de
cada cadena; re-verificar antes de calibrar — la política de orden es un parámetro del
operador del secuenciador, no un consenso de protocolo):

| Cadena | Política de orden | Cómo se compra prioridad | Nota clave |
|---|---|---|---|
| Arbitrum One/Nova | FCFS + Timeboost | Subasta Dutch por minuto de la express lane | Head start de 200ms: TODA tx de la normal lane sufre un delay artificial de 200ms; el ganador de la ronda lo salta |
| Base | Priority fee + tiempo de llegada | `maxPriorityFeePerGas` L2 | Flashblocks live (jul-2025): sub-bloques de ~200ms (~10 por bloque de 2s) con builder propio |
| OP Mainnet | Priority fee (itera hacia ordering stake-based) | `maxPriorityFeePerGas` + política del operador | Cambio de dirección cambiaría el bribe: monitorear notices de Optimism |
| Unichain (OP-Stack) | Priority fee | `maxPriorityFeePerGas` + Flashblocks | Misma familia que Base |

- **Arbitrum/Timeboost**: la subasta corre cada minuto; el ganador obtiene la express
  lane para esa ronda y sus transacciones capturan el flow sensible a latencia (los
  estudios empíricos de 2025 confirman adopción masiva de la lane). La normal lane NO
  es neutral: su delay de 200ms es el precio que financia la ventaja (ingresos de la
  subasta al tesoro de la DAO). El tip L2 NO reordena bajo FCFS puro: la ventaja
  temporal se compra en la subasta, no en el gas.
- **OP-Stack/Base**: la documentación oficial de Base especifica orden por priority
  fee y arrival time; el secuenciador op-geth ordena su txpool por tip. Los priority
  fees L2 acumulan en el SequencerFeeVault del operador: el tip es LITERALMENTE el
  mecanismo de compra de orden (análogo funcional del tip de §13.4; el base fee L2 no
  se quema — también fluye a fee vaults del operador).
- **Flashblocks** (feature OP Stack "subblocks"): un builder separado del secuenciador
  emite sub-bloques de ~200ms dentro de la ventana de 2s; endpoints dedicados con el
  block tag `pending` exponen el estado preconfirmado a esa cadencia. Para el motor:
  latencia de observación 2s → 200ms en Base sin tocar el código de detección.

**Consecuencia para el motor:**
1. **OP-Stack**: el priority fee L2 es el ÚNICO bribe nativo. El bidding de §13.9 se
   reduce a fijar el tip contra la distribución de tips observada en el feed del
   secuenciador — unidimensional, sin fan-out multi-builder ni shading por builder
   (no hay builders que diversificar).
2. **Arbitrum FCFS**: la latencia al endpoint del secuenciador ES la estrategia
   (geografía planificable, análogo exacto de lo dicho de Jito en §13.12: tips servers
   regionales ↔ endpoint del secuenciador). Con Timeboost, quien ya gana latencia
   compra además la lane: dos edges multiplicativos, no sustitutos. La decisión
   "pujar la express lane" es un costo por ronda a comparar contra el EV capturable
   del flow sensible a latencia en ese minuto.
3. **NO portar parámetros de bidding calibrados en L1 mainnet**: win-rates, shading y
   cutoffs de §13.9/§13.2 son propiedades de la subasta PBS. En L2 el dataset de
   calibración se recolecta desde cero, por chain y por política de orden.

**Orderflow "privado" en L2.** No hay mempool L2 con gossip público ni red de relays
que filtrar: TODA transacción pasa por el secuenciador. El "canal privado" es la
submission directa al endpoint del secuenciador del operador (ej.
`https://sequencer.arbitrum.io/rpc`; el feed público de mensajes ya secuenciados
`wss://arb1.arbitrum.io/feed` sirve para observar el orden POST-decisión, no para
competir por él — existe además un feed delayed intencional para relays de lectura).
Consecuencias: (a) MEV-Share no tiene equivalente — no hay subasta de orderflow de
terceros, el "backrun" es competencia pura de llegada/orden; (b) sin censoring de
relay, PERO sí riesgo de POLÍTICA del operador del secuenciador: rate-limit por
origen, filtrado de spam, términos de servicio. La mitigación análoga a la
diversificación multi-builder de §13.9 NO existe: hay UN auctioneer. Si el endpoint
throttlea o rechaza, registrar la observación con la razón exacta (R8) y aplicar
backoff — jamás ráfaga.

**Fees de dos pisos — el evaluador de EV debe leer AMBOS.** El costo de una tx L2 =
componente L1 de datos (batches posteados como blobs EIP-4844 desde los upgrades
Atlas en Arbitrum y Ecotone en OP-Stack; física de blobs en referencias 14.9/15.6) +
ejecución L2 (EIP-1559 L2). El error clásico: extrapolar gas L1 o leer solo
`eth_gasPrice`.

| Cadena | ¿`eth_gasPrice` cubre todo? | Dónde leer la componente L1 |
|---|---|---|
| OP-Stack/Base | NO — cubre solo L2 | Receipt: `l1Fee`, `l1GasUsed`, `l1BaseFee`. En caliente: predeploy GasPriceOracle `0x420000000000000000000000000000000000000F` (`l1BlobBaseFee()`, `getL1Fee(bytes)`), actualizado cada bloque vía `setL1Values` |
| Arbitrum | Integrado (ArbGas ya incluye el costo L1) | `eth_estimateGas` del nodo Nitro descuenta ambos pisos; descomponer con el estimador oficial si se audita |

Regla para pricing de opportunities en L2: leer el blob base fee VIVO (oracle L2 o
indexer L1), jamás un escalar fijo "L1 gas × constante". Un blob base fee elevado
infla la componente de datos aunque el gas L2 esté barato — un EV que ignora ese piso
miente por diseño (`arbx-net-profit-gate` aplica al total de ambos pisos, no al L2).

**Finalidad y contabilidad: soft-confirm ≠ herencia L1.** Escalones reales:

```
soft-confirm del secuenciador      (~250ms Arbitrum / 2s OP-Stack / 200ms flashblock)
  → batch posteado a L1 (blob)     datos disponibles; un reorg L1 arrastra el batch
    → L1 finalized (~13 min)       batch irrevocable → historia soft L2 estable
      → challenge window cerrado   (~7 días optimistic rollups): escape L2→L1
                                    sin confianza en el operador
```

El anclaje contable sigue la jerarquía de la referencia 15.2: detectar/simular sobre
head_hot (soft-confirm) es legítimo; `INCLUDED` sigue siendo provisional y
`RECONCILED` exige al menos batch + finalized L1 (extensión directa de §13.11). La
ventana de reto es la capa EXTRA para inventario cross-domain (referencias 17.7-17.9):
capital que requiere escape a L1 u otra vía cruzada NO es liquidez operativa hasta
cerrar la ventana — dimensionar el buffer de inventario en consecuencia.

**Downtime del secuenciador: modo operativo esperado, no anomalía exótica**
(precedentes reales en ambas familias). Comportamiento del motor (patrón dead-man de
13/15):
1. Feed pausado (sin soft-confirms nuevos N segundos) → pausa fail-honest de la
   emisión con razón explícita (`l2_sequencer_stalled`): NADA de fabricar detecciones
   sobre un L2 congelado (RULE 00/R8). Un universo de pools "quiet" durante un
   incidente es artefacto del stall, no señal.
2. Las txs ya aceptadas viven en la cola del secuenciador — semántica de REENVÍO, no
   de pérdida. Pisos anti-censura: Arbitrum delayed inbox en L1 con ventana de
   force-inclusion (~24h); OP-Stack `OptimismPortal.depositTransaction` fuerza
   inclusión si el secuenciador no avanza (límite de drift del operador).
3. Post-restart: el backlog drenado reordena el estado de golpe → re-simular TODO
   contra el primer bloque nuevo antes de emitir (doctrina de §13.11: la oportunidad
   es función del estado, no de la intención).

## 13.15 GESTIÓN DE NONCES DEL SIGNER BAJO CONCURRENCIA: POOL, FUENTE DE VERDAD Y TXs ATASCADAS

> Consolidación de fragmentos previos: §13.3 exige nonces consecutivos dentro
> del bundle, §13.5 manda cancelar bundles con nonces reutilizables, §13.9 usa
> el nonce como exclusión mutua anti doble-inclusión y §13.11 re-envía por
> bloque. El gap que esta sección cierra: CÓMO se asigna, reconcilia y
> desbloquea el nonce cuando N bundles viven en paralelo. El pool vive en el
> terminus de ejecución (`relays-client`, el único binario que firma —
> CLAUDE.md §34.3), junto al signer caliente de §13.12, no en el hot-path de
> detección.

### 13.15.1 Pool de nonces por signer: reserva al construir, devolución al caducar o descartar

El nonce de una cuenta es un recurso serial: un contador monotónico por signer.
Con N bundles in-flight (multi-builder §13.9, re-envío por bloque §13.11) la
asignación es un POOL POR SIGNER, jamás un fetch-then-send por bundle:

- **Contador atómico**: el caso común (sin huecos) es un `fetch_add` sobre un
  contador atómico (`AtomicU64` en Rust); la base confirmada y la lista de
  huecos comparten la MISMA sección crítica (mutex mínimo o task dedicada con
  canal). El pool es parte de t_sign (§13.12): cero round-trips a disco.
- **Reserva AL CONSTRUIR el bundle, antes de firmar**: la firma cubre el nonce;
  cambiarlo después exige re-firmar. El nonce reservado se liga al `bundleHash`
  en el ledger de ejecuciones (núcleo §3.2).
- **Devolución al caducar o descartar**: `eth_sendBundle` vale para UN
  `blockNumber` (§13.3) y los bundles MEV-Share valen hasta su ventana
  `inclusion.maxBlock` (§13.7) — pasado ese límite sin receipt, el bundle está
  muerto por definición → su nonce es reutilizable SI Y SOLO SI el conteo
  on-chain aún no lo cubre (§13.15.3). También se devuelve al descartar por
  `net < threshold` (paso 3 del ciclo de §13.11), previa `eth_cancelBundle`.
  NUNCA liberar el nonce de un bundle vivo: dos bundles con el mismo nonce se
  invalidan mutuamente — la patología que §13.5 previene por el lado de la
  cancelación.

### 13.15.2 Acoplamiento de orden: el N+1 no aterriza sin el N

Regla de cuenta EVM: la tx con nonce n+1 del signer A está correctamente
firmada pero NO es ejecutable mientras la tx con nonce n de A no esté incluida
(los nodos la retienen como futura; ningún bloque puede incluirla antes).
Dentro de un bundle el orden es interno
(§13.3); ENTRE bundles del MISMO signer el acoplamiento cruza builders y
bloques:

- El builder que recibe B2 (nonce 11) sin que B1 (nonce 10) esté a su alcance
  lo descarta: no puede armar una secuencia ejecutable. B2 solo compite si B1
  aterrizó en un bloque previo o llega al MISMO builder (mismo bloque).
- La probabilidad de inclusión se correlaciona en cadena:
  `p(B2) = p(B1 incluido) × p(B2 gana su subasta | B1 incluido)`. El modelo de
  subastas independientes de §13.9 NO aplica a bundles encadenados: calibrar
  el win-rate de B2 sin condicionar por B1 sesga el shading hacia overbid.
- Un B1 perdido en silencio (§13.6: fallos 100% silenciosos) envenena TODO
  nonce posterior del signer: esperan un nonce que nunca aterrizará.

**Mitigación estructural: un signer por estrategia / familia de rutas** —
aisla espacios de nonce: un atasco en la familia X no bloquea la Y. El mismo
mecanismo produce las dos propiedades: exclusión mutua anti doble-inclusión
(§13.9) y acoplamiento de orden; no hay una sin la otra — el diseño decide
dónde concentra cada efecto. Para oportunidades INDEPENDIENTES, fan-out por
signers; paralelizar nonces de un mismo signer solo cuando las txs forman
secuencia intencional (fases multi-bloque, §13.11).

### 13.15.3 Fuente de verdad: `eth_getTransactionCount`, "latest" vs "pending"

- `eth_getTransactionCount(signer, "latest")` = txs del signer en la cadena
  canónica, sin nada pendiente. Determinista post-bloque: ES la base de toda
  reconciliación.
- `eth_getTransactionCount(signer, "pending")` = nonce según el txpool DE ESE
  NODO (geth responde `GetPoolNonce`, vista local del pool): "latest" + la
  tanda CONTIGUA de txs pendientes del signer que ese nodo conoce — un hueco
  NO avanza el conteo. Para un searcher con routing privado es ruido doble:
  tus bundles y txs privadas NO están en ningún mempool público (ese es el
  punto, §13.6) y el "pending" difiere por nodo. Mezclar fuentes produce gaps:
  un "pending" desincronizado del conteo local hace saltar el contador → se
  emiten nonces con hueco → todo lo posterior queda sin ejecutar hasta
  rellenar el hueco.

**Reconciliación al reconectar / reiniciar** (post-crash, failover RPC):
1. Leer `latest` → `on_chain` (piso).
2. `local_next := max(on_chain, mayor nonce en vuelo + 1)`: el cursor de nonces
   frescos vive POR ENCIMA de todo bundle vivo. Tomar el MENOR nonce vivo como
   cursor re-asignaría el nonce de ese bundle (colisión = doble emisión).
3. Nonces emitidos > `on_chain` cuyo bundle ya expiró → huecos reutilizables;
   el pool los reintegra en orden ascendente (por debajo de todo nonce vivo).
4. Reorg (§13.11): si `on_chain` DISMINUYÓ, tu tx incluida desapareció con el
   bloque reorgueado → vuelve a estar in-flight: re-enviar el mismo contenido
   firmado para el NUEVO bloque objetivo (mismas txs → mismo `bundleHash`,
   §13.5; la exclusión mutua por nonce evita doble aterrizaje). La operativa
   se reconcilia contra head; la contabilidad, contra `finalized`.

**Tooling real (RULE 00)**: ethers-rs provee `NonceManagerMiddleware` y alloy
el filler de nonce de `alloy-provider` (módulo `fillers`, incluido en
`RecommendedFiller`). Ambos cachean e incrementan localmente y asumen un ciclo
secuencial envío→receipt: NO modelan N in-flight con expiry por bloque. Para
bundles, envolver el pool propio. Las firmas exactas se verifican contra la
versión pineada del workspace (`ethers` workspace + `alloy` 1.x en
`backend/*/Cargo.toml`; `cargo doc --open`), nunca de memoria.

### 13.15.4 Tx atascada: árbol de decisión con criterios objetivos

"Atascada" = deadline vencido (`maxTimestamp`/`blockNumber` de §13.3, o
`maxBlockNumber` de §13.6), sin receipt, y `latest` no cubre su nonce.

```
edad < deadline        → ESPERAR: el ciclo por-bloque de §13.11 sigue
                          re-enviando y re-simulando; reemplazar antes de
                          tiempo desperdicia el bid vivo sin nueva información
deadline vencido, sin receipt:
  causa = fees (base fee > maxFeePerGas; o densidad §13.4 insuficiente)
      → REPLACE-BY-FEE: misma intención, MISMO nonce, fees mayores
  causa = intento muerto (la re-simulación contra estado actual da
          net < threshold; la oportunidad ya no existe, §13.11)
      → ABANDONAR el intento (gratis en topologías atómicas de 1 tx: una tx
         nunca incluida no movió capital) y resolver el nonce:
         (a) RELLENAR: no-op barato (self-transfer, 21000 gas) con ese nonce
             → desbloquea los posteriores del signer;
         (b) SALTAR sin rellenar → TODO nonce posterior de ese signer espera
             hasta rellenar n; aceptable solo sin dependientes o rotando a
             otro signer.
  dependientes > 0 (nonces posteriores del signer esperan n)
      → replace-by-fee o relleno CON PRIORIDAD: la cola no se desbloquea sola
```

**Regla real de reemplazo**: la sustituta usa el MISMO nonce y debe pagar más
que la sustituida. En mempool geth-compatible el bump por defecto es 10%
(`--txpool.pricebump`): `maxFeePerGas` Y `maxPriorityFeePerGas` deben ser
AMBOS estrictamente mayores que los del original Y superar el umbral
`original × (100+pricebump)/100` (≥ 110%; en legacy ambos caps son el
`gasPrice` — misma regla). Geth compara los CAPS, no un "precio efectivo"
(ese depende del base fee de cada bloque). Con routing privado no
hay mempool compartido: cada builder mantiene su propia vista → REPETIR el
reemplazo POR BUILDER (el sustituto debe llegar a todo endpoint que vio el
original; el builder que no lo vio puede incluir el original si sus fees
finalmente alcanzan). Mismo nonce = exclusión mutua: solo una aterriza (§13.9).

Criterios objetivos de rama: (a) **edad** vs deadline del bundle; (b)
**dependientes**: nonces posteriores emitidos del mismo signer que esperan n;
(c) **capital bloqueado**: 0 en el ciclo atómico de 1 tx del repo; > 0 solo en
fases multi-bloque (§13.11) donde una fase previa YA ejecutó y la atascada es
el cierre — ahí replace-by-fee es URGENTE, no opcional. Cada rama elegida se
registra en el ledger con su razón exacta (R8): `wait_deadline`,
`replace_fee_bump`, `filler_noop`, `abandon_state_gone`.

### 13.15.5 Reemplazo de bundle completo vs reemplazo de tx individual

- **Bundle completo**: cubierto en §13.5 (`replacementUuid` +
  `replacingTxHashes` / `droppingTxHashes`, autenticado contra la signing key
  del envío original). Cross-ref; no se repite aquí.
- **Tx individual NO es reemplazable in-place**: el bundle ES su lista de txs
  firmadas (`bundleHash` derivado del contenido, §13.5); tocar UNA tx (fees,
  nonce, calldata) cambia el bundle entero. "Reemplazar una tx del bundle" =
  reconstruir el bundle con esa tx re-firmada + `replacementUuid` del original.
  Disciplina de nonce: el sustituto CONSUME LA MISMA RESERVA — nunca reservar
  un nonce nuevo para un reemplazo (el original quedaría vivo con nonce n y el
  nuevo con n+1: ambos pueden aterrizar = doble ejecución).
- **Tx privada standalone**: `eth_cancelPrivateTransaction` (best-effort,
  §13.6) + re-envío con el mismo nonce y fees mayores. La higiene de
  `maxBlockNumber` (§13.6) es el techo que vuelve todo atasco una expiración
  acotada, nunca un bloqueo indefinido de la cuenta.

## GOBERNANZA

Todo este conocimiento está subordinado a los gates `arbx-*` (`arbx-paper-trade-first`,
`arbx-simulation-mandatory`, `arbx-risk-limits-enforcement`, `arbx-pre-execute-checklist`)
y a CLAUDE.md §34: `LIVE_MAINNET` es canónico pero gated, con default-deny en el terminus
de ejecución (`relays-client`). Nada aquí autoriza flip a live, broadcast con capital
real, ni uso ofensivo de técnica alguna (ver `arbx-mev-ethics-gate`).
