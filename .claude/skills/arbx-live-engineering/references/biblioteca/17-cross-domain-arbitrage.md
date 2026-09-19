# 17. ARBITRAJE CROSS-DOMAIN (CEX-DEX Y CROSS-CHAIN)

CUÁNDO CARGAR ESTA REFERENCIA: la oportunidad cruza dominios — precios CEX (Binance/Coinbase/OKX) vs pools on-chain; el mismo asset en varias chains vía bridges; hedging con perp/funding y ejecución con inventario propio no-atómico; portar una estrategia EVM a Solana; o evaluar riesgo de puente/depeg antes de tocar sizing. Extiende el pipeline del núcleo (ver núcleo §1); no lo sustituye.

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Feed L2 de CEX | `@depth@100ms` (Binance), `level2` (Coinbase), `books` (OKX) | Siempre snapshot REST + updates WS; ante gap, resync completo |
| Normalización cross-venue | `exchangeInfo` / `products` / `public/instruments` | tick/lot/minNotional difieren por venue y por símbolo |
| Fees reales por tier | `/api/v3/account/commissionRate`, `/fapi/v1/commissionRate`, `/api/v5/account/trade-fee` | Leer el fee del propio account; nunca el de la documentación |
| Señal de microestructura | microprice (Stoikov), OFI (Cont-Kukanov-Stoikov) | El mid plano miente cuando el book está desbalanceado |
| Ejecución con inventario | delta-neutral spot-perp + funding como componente de P&L | `GET /fapi/v1/premiumIndex` para `lastFundingRate` |
| Algos de ejecución | TWAP / VWAP / POV, marco Almgren-Chriss | Mover tamaño no es un market order único |
| Mensajería cross-chain | CCIP / LayerZero V2 / Hyperlane / Wormhole | Mensaje no es puente de liquidez; finalidad no es confirmación |
| Atomicidad cross-chain | NO existe (problema de dos generales) | Todo patrón cross-chain es gestión de inventario + riesgo temporal |
| Arbitraje no-atómico | pre-funding bilateral + rebalanceo asíncrono | Sizing acotado por exposición máxima en tránsito |
| Solana | Jito `sendBundle`, `SetComputeUnitPrice` | Sin mempool público; la latencia hacia el leader lo decide todo |
| Fragmentación de liquidez | spread neto = Δprice − fee_puente − gas − capital×tiempo | USDC nativo vs USDC.e no son el mismo instrumento |
| Depeg de stablecoin | señales: peg, balances Curve, presión de redención | Riesgo de cola: el peg puede no volver (defer arbx-token-safety-screen) |
| Ciclo de vida de órdenes CEX | REST firmado + user-data stream + reconciliación | clientOrderId = idempotencia; divergencia venue/local = alerta, no best-effort |

## 17.1 INGESTIÓN DE FEEDS WS DE CEX (BINANCE / COINBASE / OKX)

Los tres venues sirven el book L2 por WebSocket con el mismo patrón de fondo:
snapshot inicial + updates incrementales. La única diferencia real es cómo se
verifica la continuidad de la secuencia.

| Venue | WS público | Canales de book | Verificación de continuidad |
|---|---|---|---|
| Binance spot | `wss://stream.binance.com:9443/ws` | `<symbol>@depth@100ms` (diff), `@depth20@100ms` (parcial 20 niveles) | Campos `U`/`u` vs `lastUpdateId` del snapshot |
| Binance USDT-perp | `wss://fstream.binance.com/ws` | `btcusdt@depth@100ms` | Campo `pu` = `u` del update anterior |
| Coinbase | `wss://ws-feed.exchange.coinbase.com` | `level2` (`snapshot` + `l2update`), `full` (L3) | Sin seq global; size "0" en `l2update` elimina el nivel |
| OKX | `wss://ws.okx.com:8443/ws/v5/public` | `books` (400 niveles, push 100 ms), `books5`, `books50-l2-tbt`, `books-l2-tbt` | `action:"snapshot"/"update"` + campo `checksum` (CRC32) |

**Patrón snapshot+update (Binance spot, algoritmo documentado):**

```
t0: conectar WS a btcusdt@depth@100ms ── bufferizar updates (no aplicar aún)
t1: REST GET /api/v3/depth?symbol=BTCUSDT&limit=1000 ──► snapshot (lastUpdateId = L)
t2: descartar todo update con u <= L
t3: el primer update aplicable debe cumplir U <= L+1 AND u >= L+1
t4: cada update siguiente debe cumplir U == u_prev + 1
    ── si falla: descartar el book ENTERO y volver a t1
       (jamás "patchear" a ciegas: un book corrupto produce precios falsos,
        que es exactamente el bug que RULE 00 prohíbe propagar)
```

- El payload del diff stream trae `e:"depthUpdate"`, `E` (event time ms),
  `s` (símbolo), `U`/`u` (rango de update ids), `b`/`a` (bids/asks `[precio, cantidad]`).
- OKX: el `checksum` es un CRC32 sobre la concatenación `precio:tamaño` de los
  top 25 de cada lado; si difiere del book local → resync. Los canales `-tbt`
  son tick-by-tick; `books-l2-tbt` requiere tier VIP.
- Coinbase: sin cambios en el book no llegan `l2update` — suscribir también el
  canal `heartbeat` (latido ~1 s) para detectar staleness. El acceso sin
  autenticación a `level2` ha cambiado históricamente (variante `level2_batch`,
  feed Advanced Trade `wss://advanced-trade-ws.coinbase.com` con el mismo patrón
  `snapshot`→`update` — eventos `snapshot`/`update` en el canal `level2`,
  envelope distinto al legacy): verificar estado vigente.
- Límites vigentes documentados (revisar en cada integración): Binance spot
  ~6000 request-weight/min (`depth limit=1000` ≈ weight 50), 5 mensajes/s por
  conexión WS, ~300 conexiones/5 min por IP, 1024 streams por conexión. OKX
  desconecta si no hay ping en 30 s (enviar el string `ping` periódicamente).

## 17.2 NORMALIZACIÓN CROSS-VENUE: SÍMBOLOS, DECIMALES, FEES, RELOJ

**Símbolos.** Mismo par, tres gramáticas: `BTCUSDT` (Binance), `BTC-USD`
(Coinbase), `BTC-USDT` (OKX). La diferencia de quote (USD vs USDT/USDC) es una
pierna de FX implícita: normalmente despreciable, pero durante el depeg de USDC
(marzo 2023) fue del ±3 % — comparar siempre quotes idénticos o modelar el FX.

**Decimales y mínimos** (fuente: metadata por venue, cacheada y refrescada):

| Venue | Endpoint | Campos |
|---|---|---|
| Binance | `GET /api/v3/exchangeInfo?symbol=BTCUSDT` | filtros `PRICE_FILTER.tickSize`, `LOT_SIZE.stepSize`, `NOTIONAL.minNotional` (antes `MIN_NOTIONAL`) |
| Coinbase | `GET /products/BTC-USD` | `quote_increment`, `base_increment`, `min_market_funds` |
| OKX | `GET /api/v5/public/instruments?instType=SPOT&instId=BTC-USDT` | `tickSz`, `lotSz`, `minSz` |

Regla de redondeo: cantidad SIEMPRE hacia abajo (nunca exceder saldo ni
mínimos), precio de cruce hacia el lado que garantiza la ejecución (up en buy,
down en sell) presupuestando el tick como coste; para órdenes resting, precio
exacto a grid. El residuo de redondeo se registra como fee implícito en el P&L.

**Fees por tier — leer, no asumir.** Los fees publicados son del tier base; el
propio account casi siempre difiere (descuentos BNB, VIP por volumen, maker
promos):

| Venue | Endpoint SIGNED | Retorno |
|---|---|---|
| Binance spot | `GET /api/v3/account/commissionRate?symbol=BTCUSDT` | `standardCommission.maker/.taker` |
| Binance perp | `GET /fapi/v1/commissionRate?symbol=BTCUSDT` | `makerCommissionRate`, `takerCommissionRate` |
| OKX | `GET /api/v5/account/trade-fee?instType=SPOT&instId=BTC-USDT` | `makerFee`, `takerFee` por nivel |

Hardcodear un fee en config viola `arbx-no-hardcode-doctrine`: el fee es un
dato vivo del venue, igual que reserves o gas price.

**Sincronización de reloj cross-venue.**
- El VPS corre chrony con `makestep` (salto inicial) y disciplina continua;
  PTP (linuxptp) solo si hay colocation con requirements de µs.
- Medir offset por venue: `E` (event time del venue) vs timestamp de recepción
  local con reloj MONOTÓNICO (`Instant` en Rust, no `SystemTime`). Registrar la
  distribución; un offset que crece = feed degradado o reloj local drenado.
- Nunca ordenar eventos CEX contra timestamps de bloque EVM: granularidad 12 s
  y slack del proponente. La causalidad cross-dominio se establece con el
  timestamp de captura LOCAL en un único dominio de reloj.

## 17.3 ESTRUCTURA DEL BOOK: L2 vs L3, MICROPRICE, OFI

| Aspecto | L2 (agregado por precio) | L3 (por orden) |
|---|---|---|
| Qué ofrece | Tamaño total por nivel | Órdenes individuales con id y ciclo de vida |
| Quién lo sirve | Binance, OKX, Coinbase (`level2`) | Coinbase legacy `full` (`received`/`open`/`match`/`done`/`change`) |
| Suficiente para | Taker arb, señales de imbalance, pricing | Modelar queue position y probabilidad de fill maker |
| Coste/truco | Ya agregado, sin identidad de orden | Reconstrucción pesada; FIFO dentro del nivel es suposición |

**Microprice** (Stoikov, "The Micro-Price", 2018) — fair value ponderado por
tamaño, robusto al imbalance que el mid plano ignora:

```
M = (b·Qa + a·Qb) / (Qb + Qa)
b/a = mejor bid/ask · Qb/Qa = tamaño en bid/ask
Si Qb >> Qa → M se pega al ask: presión compradora empuja el fair value arriba.
```

Usar M (no last-trade, no mid) como precio de referencia CEX contra el que se
mide el precio del pool DEX.

**Order-Flow Imbalance** (Cont, Kukanov, Stoikov, "The Price Impact of Order
Book Events", 2014): sobre una ventana, OFI = Σ (incremento de tamaño bid con
precio no bajado) − Σ (incremento de tamaño ask con precio no subido). La
relación empírica es lineal: Δmid ≈ β·OFI; la extensión multi-nivel agrega
profundidad a k niveles. Uso en CEX-DEX: OFI sostenido predice dirección CEX;
el pool DEX cotiza con lag → el edge estimado es OFI-condicional, no solo el
spread instantáneo. Advertencia de datos: el OFI es tan limpio como el book —
computarlo solo entre snapshots verificados por secuencia (§17.1), nunca a
través de un resync.

**Queue position (maker):** con L3 se estima el volumen ahead en tu nivel y su
decaimiento (cancelaciones/matches) para modelar P(fill antes de que el precio
se mueva contra ti). Con L2 solo se aproxima; un maker CEX sin modelo de cola
está regalando adverse selection.

## 17.4 COSTE MAKER/TAKER REAL POR TIER

El fee de schedule es la punta del coste. Coste real por pierna:

```
taker:  fee_taker + half_spread + impacto(size) + fee_residuo_redondeo
maker:  fee_maker − rebate + P(no fill | precio se va)·costo_oportunidad
        + adverse_selection (te llenan justo cuando el mercado va contra ti)
        + carry_de_inventario_durante_la_espera
```

Magnitudes típicas de tier base (VERIFICAR siempre con el endpoint del §17.2;
estos números cambian y son solo calibración de orden de magnitud):

| Venue/mercado | maker | taker | Nota |
|---|---|---|---|
| Binance spot VIP0 | 10 bps | 10 bps | 7.5 bps pagando fee en BNB (−25 %) |
| Binance USDT-perp VIP0 | 2 bps | 5 bps | descuento BNB −10 % en futures |
| Coinbase Advanced tier 1 (<10 k/30d) | ~40 bps | ~60 bps | el venue caro: filtra casi todo el edge retail |
| OKX spot L1 | ~8 bps | ~10 bps | baja con volumen 30d |

Consecuencia económica dura: a tiers retail, el round-trip taker CEX (60-120
bps round-trip) excede el spread CEX-DEX típico (1-10 bps). El edge CEX-DEX a
escala vive en: (a) tiers VIP/agencia, (b) pierna maker en CEX + pierna taker
en DEX (con el riesgo de cola del maker), o (c) horizontes de inventario
largos donde el maker domina. Este filtro es la traducción cross-domain de
`arbx-net-profit-gate`: edge_bruto − costes reales ambos lados (ver núcleo §1.2).

## 17.5 EJECUCIÓN CON INVENTARIO: DELTA-NEUTRAL Y FUNDING

Patrón: comprar spot barato (DEX o CEX), vender perp del mismo subyacente en
CEX por notional equivalente → delta ≈ 0; el P&L queda expuesto a basis,
funding y ejecución, no a dirección.

**Mecánica de funding (Binance USDT-M):** intercambio cada 8 h (00:00/08:00/16:00
UTC; algunos símbolos 4 h), pago = notional × funding_rate, el signo lo fija el
premium (perp > index → longs pagan shorts). El interés base del clamp es por
defecto 0.01 % por período de 8 h; cada contrato tiene cap propio. Endpoints:
`GET /fapi/v1/premiumIndex?symbol=BTCUSDT` (campos `markPrice`, `lastFundingRate`,
`nextFundingTime`), `GET /fapi/v1/fundingRate?symbol=BTCUSDT` (histórico), OKX
`GET /api/v5/public/funding-rate?instId=BTC-USDT`.

**Descomposición de P&L del libro delta-neutral:**

```
P&L = Σ edge_bruta_piernas
    − Σ fees_cexs (tier real, §17.2) − gas_dex − bridge_fee_amortizado
    − Σ_t funding(t)·notional(t)        ← puede cambiar de signo en vuelo
    − basis_drift                        ← perp y spot no convergen gratis
    − hedging_puente (§17.9)             ← funding extra durante el tránsito
```

Reglas operativas: rebalancear el hedge cuando |delta| del libro supera banda
(con histéresis, §17.6); el funding esperado del horizonte de tenencia entra al
cálculo de edge ANTES de ejecutar (ver núcleo §1.2 evaluador); correlación de
cola: en stress el premium del perp explota exactamente cuando más se necesita
el hedge — presupuestar funding adverso, no el promedio.

## 17.6 EXECUTION ALGOS (TWAP/VWAP/POV) Y REBALANCEO DE INVENTARIO

| Algo | Objetivo | Palanca | Cuándo |
|---|---|---|---|
| TWAP | precio medio en el tiempo | slices iguales cada Δt | book delgado, urgencia baja |
| VWAP | replicar el precio medio ponderado por volumen | slices ∝ perfil de volumen esperado | tamaño vs volumen del día relevante |
| POV (participation) | no superar X % del volumen | child orders hasta participation target | mover tamaño sin ser el mercado |
| IS (implementation shortfall) | minimizar coste vs arrival price | trayectoria E[coste]+λ·Var (Almgren-Chriss, 2000) | benchmark explícito y auditoría |

Prácticas: participation ≤ 5-10 % del volumen del book objetivo; aleatorizar
timing/tamaño de slices para reducir la firma estadística del flujo propio
(footprint, no manipulación — el marco ético es `arbx-mev-ethics-gate`);
respetar rate limits de órdenes (spot Binance: ~100 órdenes/10 s, 5 msg/s
entrante por WS; el endpoint `cancelReplace` es de futures, no de spot); medir slippage
real vs benchmark (arrival/VWAP) por child order y alimentar el modelo de
impacto con datos propios, no con supuestos.

**Rebalanceo de inventario por venue/chain:** banda objetivo con histéresis —
disparar rebalanceo al cruzar umbral X, detener al volver bajo Y < X — para no
oscilar (thrash) pagando fees en cada rebote. Liquidación parcial: si
exposición_en_tránsito × volatilidad breachVaR, reducir libro ANTES de abrir
nuevas piernas (orden de prioridad: de-risk primero, edge después). El marco de
límites vive en `arbx-risk-limits-enforcement`; el kill-switch del núcleo §6.3
debe poder congelar también el rebalanceador, no solo el detector.

## 17.7 CROSS-CHAIN: TAXONOMÍA DE PUENTES, FINALIDAD, RIESGO

| Categoría | Ejemplos | Mecánica | Latencia típica de salida |
|---|---|---|---|
| Canonical L1↔L2 | Arbitrum (Inbox/Outbox), OP-Stack: Optimism/Base (`L1StandardBridge`/`OptimismPortal`) | lock/mint con salida por challenge | L2→L1 ~7 días (ventana de fault proofs) |
| Canonical sidechain | Polygon PoS (FxPortal) | checkpoint de validadores | salida ~30 min–1 h (según checkpoints) |
| Bridge de liquidez | Across (relayers + repago canónico), Hop (bonders + hTokens), Stargate (LayerZero, liquidez unificada), Synapse, cBridge | LP/bonder adelanta liquidez, se re-equipa por la vía canónica | segundos–minutos (riesgo transferido al LP) |
| Mensajería genérica (GMP) | Chainlink CCIP (DONs + Risk Management Network, rate limits por lane), LayerZero V2 (DVNs configurables), Hyperlane (ISMs modulares), Wormhole (19 guardians, umbral 13/19, VAAs) | pasar mensajes con verificación propia | según atestación; el asset lo mueve otro contrato encima |
| Burn-and-mint nativo | Circle CCTP (USDC; V2 con finalidad de segundos-minutos) | quemar origen + atestación Circle + mint destino | minutos; contraparte concentrada en Circle |

**Finalidad vs confirmación (no confundir):** incluirse en un slot de Ethereum
(12 s) NO es finalidad (2 épocas, ~13 min en condiciones normales); la
confirmación soft del sequencer de un rollup NO es herencia de seguridad L1
(esa llega al postear datos +, para salidas canónicas, la ventana de reto); la
optimistic confirmation de Solana (<1 s típica) depende del skip rate. Un
puente de liquidez puede acreditar tu balance en segundos, pero su propio
balance de respaldo viaja por la vía lenta: el riesgo del LP es sistémico para
todos los usuarios del bridge a la vez.

**Riesgo de puente — historial público (todas cifras aprox., registro 2021-2023):**

| Bridge | Año | Pérdida | Causa raíz |
|---|---|---|---|
| Poly Network | 2021 | ~$611 M (devueltos) | bug de llamadas cross-contract |
| Ronin (Axie) | 2022 | ~$624 M | robo de 5/9 claves firmantes |
| Wormhole | 2022 | ~$325 M | verificación de firma defectuosa en contrato Solana |
| Harmony Horizon | 2022 | ~$100 M | multisig 2/5 comprometida |
| Nomad | 2022 | ~$190 M | trusted root mal inicializado → replays válidas para todos |
| BNB Bridge | 2022 | ~$570 M en BNB | prueba IAVL forjada |
| Multichain | 2023 | ~$126 M | custodia de claves / gobernanza opaca |

Patrón: la mayoría NO son bugs de AMM sino **custodia de claves y verificación
de pruebas**. Checklist de due diligence antes de confiar inventario a un
puente: tamaño y umbral del set validador/DVN, timelock de upgrades, rate
limits y circuit breakers por lane, concentración de TVL vs tu tamaño,
historial de auditorías, capacidad de pausa y quién la tiene, y si el
"atomicity" reclamado es marketing (ver §17.8).

## 17.8 POR QUÉ NO EXISTE ATOMICIDAD CROSS-CHAIN

La inclusión en la chain A y la inclusión en la chain B las deciden conjuntos
de validadores disjuntos, sin estado compartido ni rollback cruzado: es el
problema de los dos generales en su forma bizantina. Ningún par de transacciones
en dos chains tiene garantía all-or-nothing. Por eso:

- Cualquier producto "atomic cross-chain" reduce a: (a) un tercero de confianza
  (relayer, bonder, LP, exchange) que absorbe el riesgo temporal por una fee,
  o (b) secuenciación en un único dominio (shared sequencers, based rollups —
  dirección de diseño, no despliegue universal), en cuyo caso ya no es
  cross-chain.
- Los **intents** (ERC-7683, estándar abierto de órdenes cross-chain usado por
  Across) no eliminan el riesgo: lo **tasan y lo transfieren** a fillers
  profesionales con inventario — que lo gestionan exactamente con el patrón de
  §17.9. Como taker de intents, tu ejecución es inmediata pero pagas la prima
  del riesgo del filler.

Contraste con el núcleo §1: dentro de una chain, un bundle (`eth_sendBundle`)
es all-or-nothing dentro de un bloque. Esa garantía NO está disponible aquí.
Consecuencia de diseño: toda estrategia cross-domain es una estrategia de
inventario; la "corrección" se define como contabilidad exacta de exposición
en tránsito, no como atomicidad de transacción.

## 17.9 PATRONES PARA ARBITRAJE NO-ATÓMICO

**Pre-funding bilateral** — inventario propio en ambos lados; ambas piernas se
ejecutan contra balance propio de forma inmediata; el re-nivelado es asíncrono:

```
   lado A (chain/venue rápido)             lado B (chain/venue lento)
   ┌──────────────────────────┐            ┌──────────────────────────┐
   │ inventario propio Q_A    │            │ inventario propio Q_B    │
   └───────────┬──────────────┘            └───────────┬──────────────┘
               │ 0. señal: spread neto (§17.11) > umbral + funding + fees
               │ 1. ejecutar pierna A AHORA (balance propio, sin esperar)
               │ 2. ejecutar pierna B AHORA (balance propio)
               │ 3. inventario descuadrado ±Q ── exponencia el libro a dirección
               └────── 4. puente/transfer Q en background (p95 + margen) ─────►
                          5. llegada → inventario re-nivelado → repetir
```

**Máquina de estados por pierna** (idempotencia por pierna, ver núcleo §1.3):

```
CREATED → SUBMITTED → EXECUTED ─┐
                 ├→ FAILED ──────┼→ COMPENSATED → RECONCILED
                 └→ TIMEOUT ─────┘
```

- Timeout = p99 de latencia del venue × ~3. En TIMEOUT el estado es
  DESCONOCIDO: sondear (order status / receipt), NUNCA retry ciego — un retry
  duplica posición. La **compensación** es una posición nueva (hedge perp o
  unwind de la otra pierna), no un reintento.
- **Hedging del riesgo de puente:** mientras Q está en tránsito, el notional
  viaja descubierto → short perp por la duración del tránsito; su coste
  (funding × horas de puente) entra al edge ex-ante (§17.5).
- **Sizing por exposición máxima aceptada** (traducción de
  `arbx-risk-limits-enforcement`):

```
Q_max ≤ E_max / (z_α · σ_Δt)      con Δt = p95(bridge) + reacción operativa
                                    σ_Δt = vol del asset en Δt (escalado √t)
Q_max ≤ x % de la profundidad ejecutable (slippage bound)
Q_max ≤ y % del TVL del puente (riesgo de concentración/salida)
```

  Si el puente se cae (historial §17.7), el inventario queda particionado:
  pre-definir política de liquidación parcial del lado sin salida (§17.6) y
  bloquear nuevas piernas que empeoren el desbalance.

## 17.10 SOLANA (BREVE): SVM, LEADER SCHEDULE, JITO, PRIORITY FEES

Diferencias estructurales vs EVM que cambian la estrategia:

| Dimensión | EVM (mainnet) | Solana |
|---|---|---|
| Ejecución | Serial en el bloque | SVM/Sealevel: paralela por cuentas disjuntas |
| Producción de bloques | proponente aleatorio por slot | leader schedule determinista por epoch (~432,000 slots ≈ 2 días; slot 400 ms; `getLeaderSchedule` RPC) |
| Mempool | público (o privado vía relay) | NO hay mempool público: la tx viaja por QUIC al TPU del leader |
| Incentivo de inclusión | gas auction / bribe a builder | priority fee + tip Jito; latencia al leader |
| Expiración de tx | nonce management / deadline lógico | blockhash válido ~60-90 s; durable nonces para colas offline |

- **Priority fees:** programa `ComputeBudget111111111111111111111111111111`
  con instrucciones `SetComputeUnitLimit` (presupuesto de CU; default 200k por
  instrucción, cap 1.4 M por tx) y `SetComputeUnitPrice` (micro-lamports por CU). Prio fee total =
  CU_limit × CU_price. Medir CU reales de la tx (simulación) y ajustar: pedir
  1M CU "por si acaso" infla el coste sin mejorar inclusión.
- **Jito:** block engine `https://mainnet.block-engine.jito.wtf`, bundles vía
  JSON-RPC `sendBundle` (`POST /api/v1/bundles`); bundle = hasta 5 txs,
  all-or-nothing en el top del bloque — la única "atomicidad" disponible y es
  intra-bloque, NO cross-chain. Tip = transfer a un tip account de Jito al
  final del bundle; el bundle no aterriza sin tip competitivo.
- **El juego de latencia:** conocer qué leader construye los próximos slots
  (schedule pública) y minimizar RTT (la mayoría de leaders/relays operan en
  Frankfurt/FRA) es el equivalente funcional de la colocation de relays EVM.
- Riesgos a detectar (no recetas): sandwiching existe en Solana vía orderflow
  privado/bundles contra routers tipo Jupiter — simular slippage bounds y
  deadlines como en EVM; el skip rate introduce incertidumbre pre-confirmación
  (reorg-like); Marco ético: `arbx-mev-ethics-gate`.

## 17.11 FRAGMENTACIÓN DE LIQUIDEZ CROSS-CHAIN

El mismo asset en N chains no es un mercado: son N mercados correlacionados
con fricciones de transferencia. Además, "USDC nativo" y "USDC.e (bridged)"
NO son el mismo instrumento — distinto emisor de riesgo (Circle vs el bridge
que minte el wrapped), distinta liquidez; arbitrar entre ellos es un trade de
riesgo de puente disfrazado de stablecoin-stablecoin.

**Spread neto monitoreable por ruta:**

```
edge_net = Δprice(chain_i, chain_j)
         − gas_src − gas_dst − fee_puente − slippage_rebalance
         − (r_capital × Δt_puente)          ← coste de capital en tránsito
         − funding_hedge × horas            ← si se hedea el tránsito (§17.9)
```

Monitoreo: precio por chain desde pools (`getReserves()` v2 / `slot0()` v3 —
siempre en el bloque latest y con verificación de staleness, núcleo §1.2) más
referencia CEX (§17.3 microprice); alertar por persistencia del spread neto
(estructura) vs ruido (mean-reverting instantáneo).

**Allocation de inventario por chain:** working_balance(chain) = flujo esperado
del horizonte + k·σ + buffer de seguridad; disparar rebalanceo cuando el
balance proyectado < 0 (run-out) por la vía más barata confiable, con el costo
de mover capital (fee + gas ×2 + slippage + capital×tiempo) amortizado sobre el
flujo esperado — no rebalancear por nostalgia de simetría. CCTP mueve USDC
nativo burn-and-mint (sin wrapped intermedio) al coste de concentrar
contraparte en Circle.

## 17.12 DEPEGS Y STABLECOINS: SEÑALES Y TRAMPA DE COLA

**Señales tempranas (todas on-chain u observables, zero mocks):**
- Desviación sostenida del peg en mercados secundarios vs valor de redención.
- Composición del pool Curve/StableSwap: leer `balances(i)` (view pública en
  pools v1) — un pool deslizándose a 100 % del asset bajo presión es una fila
  de salida (precedentes: UST/3Pool 2022; USDC en SVB 2023 ~$0.87-0.88).
- Presión de redención: tasa mint/burn asimétrica, gates o colas de redención
  activadas, pausas de contratos.
- Divergencia oráculo vs spot del pool (Chainlink `latestRoundData` vs precio
  del AMM) — el mercado "sabe" antes que el oráculo.
- Backing: ratio de reservas declarado y su composición (un "stable" respaldado
 por otro stable hereda su cola).

**Por qué tradear contra un depeg es trampa de riesgo de cola:** el payoff es
asimétrico — si el mecanismo de redención responde, converges al peg (ganancia
acotada, bps); si está roto (insolvencia, gate, death spiral), la pérdida del
leg largo es −100 % y la salida se evapora justo cuando todos quieren salir
(liquidez del pool → un solo asset). El spread observado tasa LIQUIDEZ, no
probabilidad de convergencia; ex-ante USDC-2023 y UST-2022 se ven iguales en
el gráfico de spread y terminan opuestos. Doctrina: prohibido asumir
mean-reversion sin hedge de cola; sizing como si la pierna pudiera valer cero;
la admisión del asset pasa por `arbx-token-safety-screen`, no por esta
referencia.

## 17.13 CICLO DE VIDA DE ÓRDENES: APIs FIRMADAS, USER-STREAMS Y RECONCILIACIÓN

La mitad ejecutora de la integración CEX: §17.1-§17.3 cubren el market data
público; aquí es trading autenticado — firmar, colocar, seguir y reconciliar cada
orden. Los algos de §17.6 consumen este ciclo (no lo implementan); el libro de
piernas de §17.9 usa la máquina de estados de aquí como fuente de verdad por
venue. Límites de capital y sizing: `arbx-risk-limits-enforcement`.

**Autenticación — firma exacta por venue, verificada contra docs vigentes (RULE 00):**

| Venue | Esquema | Se firma | Transporte |
|---|---|---|---|
| Binance | HMAC-SHA256 (hex) del query-string completo (`timestamp` + `recvWindow` + params); alternativa RSA o Ed25519 por configuración de la API key | query-string | header `X-MBX-APIKEY`; `signature` como último param |
| OKX | HMAC-SHA256 → Base64 de `timestamp + method + requestPath + body` concatenados | string completo | `OK-ACCESS-KEY`, `OK-ACCESS-SIGN`, `OK-ACCESS-TIMESTAMP` (ISO 8601), `OK-ACCESS-PASSPHRASE` |
| Coinbase Advanced | JWT ES256 (ECDSA P-256) | claims `sub`, `uri`, `iat`/`exp` (≤120 s), `nonce`; `kid` = API key name | `Authorization: Bearer <jwt>` |

El material de firma es secreto de runtime (núcleo §6.1), jamás un literal en
config (`arbx-no-hardcode-doctrine`).

**Rate-limit signed ≠ público.** El weight de request (§17.1) no agota el budget
de trading: Binance mantiene además un order-rate propio (~100 órdenes/10 s spot
más un cap diario — verificar vigencia en cada integración; §17.6 ya lo
presupuesta para los algos). Dos buckets aislados por clase de endpoint: agotar
el público jamás debe bloquear un cancel del signed. `recvWindow` (Binance,
default 5000 ms; error `-1021` fuera de ventana) convierte el drift de reloj de
§17.2 en un error de firma, no solo de ordenación: chrony verde es prerequisito
del trading firmado.

**Instrucciones y modos.** LIMIT/MARKET en los tres venues; time-in-force
GTC/IOC/FOK (Binance `timeInForce`; OKX lo codifica en `ordType` — `ioc`, `fok`,
`post_only`; Coinbase Advanced en `time_in_force` — `good_until_time`,
`immediate_or_cancel`, `fill_or_kill`, `post_only`). `reduce-only` solo existe en
derivados. Self-trade-prevention por venue: Binance `selfTradePreventionMode`
(`EXPIRE_TAKER`/`EXPIRE_MAKER`/`EXPIRE_BOTH`), OKX `stpMode` — configurarlo
explícito, el default no es uniforme. Filtros tick/lot/minNotional: la
normalización de §17.2 se aplica ANTES de firmar.

**clientOrderId = idempotency-key del dominio venue.** Binance
`newClientOrderId`, OKX `clOrdId`, Coinbase `client_order_id`: asignado ANTES del
primer envío y derivado determinísticamente de la intención (mismo invariante que
núcleo §1.3). Regla dura: el reintento REUSA el mismo client id — un reintento
con id nuevo es una orden duplicada. En timeout el estado es DESCONOCIDO (§17.9):
sondear por client id (`GET /api/v3/order?origClientOrderId=...` en Binance)
antes de cualquier reenvío.

**Máquina de estados de orden (normalizada):**

```
NEW ──▶ PARTIALLY_FILLED ──▶ FILLED        terminal de éxito
  ├──▶ CANCELED / EXPIRED / REJECTED       terminales (EXPIRED incluye STP)
  └──▶ REPLACED (amend / cancel-replace)   la identidad de la orden cambia:
                                           re-vincular el client id antes de seguir
```

Binance notifica por `executionReport` (`X` = tipo de ejecución
NEW/TRADE/CANCELED/REJECTED/EXPIRED, `z`/`Z` cantidades acumuladas, `l`/`L`
último fill); OKX por `state`: `live`/`partially_filled`/`filled`/`canceled`;
Coinbase Advanced: `OPEN`/`FILLED`/`CANCELLED`/`EXPIRED`/`FAILED`. Cada
transición alimenta el libro de §17.9: PARTIALLY_FILLED actualiza qty ejecutada
con avg price vivo; FILLED cierra la pierna EXECUTED; CANCELED/EXPIRED la
regresa a FAILED→COMPENSATED; REJECTED nunca salió del venue y se registra con
su razón exacta (fail-honest, abajo).

**User-data streams — el fill llega por push, el REST es la verdad:**

| Venue | Canal privado | Keepalive | Nota |
|---|---|---|---|
| Binance | `POST /api/v3/userDataStream` → WS `wss://stream.binance.com:9443/ws/<listenKey>` (`executionReport`, `outboundAccountPosition`, `balanceUpdate`) | `PUT /api/v3/userDataStream` cada ~30 min (el listenKey expira a 60) | al cerrar el WS los eventos del gap NO se recuperan del stream |
| OKX | `wss://ws.okx.com:8443/ws/v5/private` + `op:"login"` con la misma firma REST; canales `orders`, `positions`, `account`, `fills` | ping WS <30 s (§17.1) | el login re-firma en cada conexión |
| Coinbase Advanced | `wss://advanced-trade-ws.user.coinbase.com`, canal `user` con JWT | rotar el JWT antes de `exp` | cada evento trae `client_order_id` |

Balances y posiciones son snapshot+delta con la MISMA disciplina de secuencia
que §17.1 aplica a books: `outboundAccountPosition` es delta sobre el último
snapshot conocido — ante gap o reconnect, resnapshot REST (`GET /api/v3/account`;
`GET /api/v5/account/balance`) y reconciliar; nunca extrapolar un balance.

**Bucle de reconciliación — divergencia = alerta, no "mejor esfuerzo".** En cada
reconexión del user-stream y por cadencia (p.ej. cada minuto y cierre diario),
cruzar tres conjuntos por client id + trade id: (a) órdenes locales no terminales;
(b) open orders del venue — `GET /api/v3/openOrders`,
`GET /api/v5/trade/orders-pending`, `GET /api/v3/brokerage/orders/historical`
(Coinbase); (c) fills — `GET /api/v3/myTrades`, `GET /api/v5/trade/fills-history`,
`GET /api/v3/brokerage/orders/historical/fills`. Orden local sin contraparte en
venue, fill del venue sin registro local, o estado incompatible → alerta con
razón exacta; jamás "sincronizar silenciosamente". El cierre diario es trial
balance del libro completo: balance DEX on-chain (`balanceOf`) + balances CEX +
ledger interno; toda diferencia queda explicada línea a línea (fee, funding,
transfer) o es incidente (R8; trazabilidad R7).

**P&L de fills.** avg fill price = Σ(price_i·qty_i)/Σqty_i sobre los trades de la
orden (`myTrades`), nunca el precio límite de la instrucción. La fee puede
cobrarse en OTRO asset (BNB con descuento en Binance spot; OKB/USDT en OKX):
convertirla al numeraire del ledger con el precio del momento del fill (misma
fuente que §17.2); fee en asset no computada = P&L inflado — el mismo pecado
contable que omitir el gas (R8; núcleo §1.2 evaluador).

**Taxonomía de errores venue → fail-honest:**

| Clase | Ejemplos | Acción |
|---|---|---|
| credencial | Binance `-2015` (invalid API-key/IP/permissions), OKX `50113` (sign/passphrase), 401 JWT Coinbase | kill-switch del venue; NUNCA retry (agrava el bloqueo) |
| filtro/balance | Binance `-1013` (filters), `-2010` (insufficient balance), OKX `51119` (tamaño) | orden no computable: registrar razón exacta, corregir la instrucción |
| reloj | Binance `-1021` (recvWindow) | verificar chrony (§17.2) antes de reintentar |
| rate-limit | HTTP 429; Binance 418 = ban por insistir tras 429 | backoff del bucket signed; 418 = pausa larga + alerta |
| sistémico | 5xx / timeout | estado DESCONOCIDO → sondeo por client id (§17.9), nunca reenvío ciego |

## GOBERNANZA

Referencia 17 de la biblioteca de `arbx-live-engineering`; extiende el núcleo
v1.0.0 y está subordinada a los gates `arbx-paper-trade-first`,
`arbx-simulation-mandatory`, `arbx-risk-limits-enforcement` y
`arbx-pre-execute-checklist`, y a CLAUDE.md §34 (LIVE_MAINNET gated,
default-deny en el terminus `relays-client`). Modo de operación shadow/paper:
nada aquí autoriza flip a live, broadcast con capital real, ni exposición de
inventario propio en mainnet. Los vectores ofensivos mencionados existen solo
como riesgos a detectar y mitigar bajo `arbx-mev-ethics-gate`.
