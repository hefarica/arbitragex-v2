# SKILL 069 — Order Flow Spoofing & L2 Deception Tactics

## 1. Propósito superior
Desplegar Tácticas de Señalización Falsa e Inyección de Ruido Microestructural (Spoofing & Ghost Orders L2) de manera puramente defensiva y algorítmicamente legal, para despistar, ahuyentar y agotar computacionalmente a la competencia de bots HFT hostiles y Searchers MEV. En el Dark Forest y en los Orderbooks CEX de Alta Frecuencia, no basta con ser rápido; hay que evitar que los demás te sigan o te usen como liquidez direccional. Esta Skill funciona como contramedida de Guerra Electrónica Financiera L2 (Electronic Warfare / Deception CEX L2): Plantando liquidez que desaparece al instante y rompiendo el modelo de Machine Learning de la competencia.

## 2. Nivel de conocimiento requerido
Market Maker Institucional Experto en Decepción Algorítmica CEX L2. Entendimiento profundo del "Ping-Ponging", Layering (Estratificación), Quote Stuffing (Saturación de Cotizaciones - Legal Constraints CEX), Dynamic API Cancel-Rates, Manipulación de "Order Book Imbalance" Enemigo y Regulación Estricta HFT (Límites donde el engaño es permitido sin incurrir en Wash Trading penalizable CEX L2).

## 3. Capacidades principales
1. Absorción Defensiva (Iceberg / Hidden Orders): Enviar órdenes masivas al exchange ocultando el 90% del tamaño L2. Así, la Ballena HFT Enemiga no detecta Muros de Venta asustándose; simplemente golpea nuestro límite pasivamente creyendo que el libro está vacío L2, siendo atrapado en nuestra liquidez CEX Asimétrica (Skill 57).
2. Ghost Walls (Spoofing L2 Predictivo): Colocar Muros Gigantes L2 de $5 Millones en Compra a 50 Ticks de distancia (Lejos del precio). El Bot Enemigo L2 lee su Skill de Microestructura, ve el "Mega Imbalance Alcista" y empieza a comprar impulsivamente (Front-running the wall). Nuestro Bot le vende la subida en el Tick 1, y luego CANCELA mágicamente el Muro de $5 Millones del Tick 50, habiendo usado el pánico sintético para liquidar su inventario CEX de forma ultra-rentable.
3. Quote Stuffing L2 (Inyección de Ruido): Para proteger un Arbitraje L2 CEX delicado, la Skill inyecta y cancela 100 órdenes de $1 por milisegundo en pares colaterales. Esto inunda la API de los competidores HFT y ahoga su procesador en "Basura Websocket", otorgando milisegundos ciegos al Agente HFRC para ejecutar su Delta Neutral CEX L2 con exclusividad de banda ancha CEX.
4. Ping-Trading Discovery: Mandar órdenes pequeñísimas ($10 L2 CEX) que cruzan el límite, solo para ver "Quién reacciona" y con cuánta latencia L2. Usa esta telemetría para mapear la velocidad de los bots competidores en ese específico Par CEX L2 y calibrar la Skill 57 (Market Maker).
5. Parity Fuzzing (Cegamiento de Bots de Arbitraje L1 L2): Colocar Órdenes CEX que temporalmente "Crean" un Arbitraje Artificial L2-L1 (Triangular Falso) para que Bots hostiles Takers gasten Gas L1 Cripto o Fees CEX L2 intentando atajar una oportunidad que será cancelada atómicamente antes del Fill (Cancel/Replace asíncrono HFT O(1)). Sangrado por Fricción Enemiga.
6. Auto-Cancel on Toxic Pulse (Retiro Flash): Si el Order Flow O(1) de Skill 62 predice una ráfaga verdadera que va a golpear tu Ghost Wall L2. La orden es retirada inmediatamente L2 EVM RAM, garantizando que NUNCA (0% prob) una orden diseñada para manipular la microestructura termine siendo Fildeada (Llenada / Executed) perdiendo capital.
7. Anti-Wash Trading Firewall: La decepción HFT roza líneas legales finas CEX. Si la Skill de Decepción emite una Orden Falsa, el Firewall O(1) se asegura matemáticamente de que esa orden JAMÁS cruce (Match) con una orden legítima del PROPIO Agente (Evitando el Wash Trading L2 sancionado con Bloqueos de Cuenta de CEX Institucionales).
8. Camuflaje de Sizing HFT L2 (Randomized Execution Tiers): Si el Agente siempre compra $10,000 exactos, los ML de la competencia lo huellan (Fingerprinting CEX L2). Esta Skill disfraza toda salida MUX (Skill 64): Compra `$9,842`, luego `$245`, luego `$12,010`, rompiendo el Pattern Recognition Adversarial CEX L2.
9. Phantom Hedging (Manipulación Beta L2): Shortear pequeñas porciones visibles (Asks en Perpetuos L2) para convencer al algoritmo Funding Rate (Skill 16) de que el Momentum es bajista, mientras se acumula Long Ciego (Iceberg) en el Spot subyacente.
10. Evasión CEX Penalty Rate (Control de Frecuencia L2): Los Exchanges multan por `Order/Cancel Ratios` altos L2 (Generar Cargas Inútiles L2). El Spoofing Engine L2 calcula exactamente el umbral de penalidad (`Ej. Binance te banea si cancels/fills > 1000`). La Skill mantiene la decepción por DEBAJO de ese límite para mantener la licencia CEX API VIP L2.

## 4. Entradas requeridas
- `enemy_microstructure_latency`: Telemetría (Ping-Trading L2) del tiempo de respuesta del "Mercado" (Competencia HFT).
- `unfilled_inventory_target`: Orden Macro del Agente que necesita ser completada O(1) (Ej. Quiero Vender 100 BTC sin asustar a nadie).
- `api_weight_limit_telemetry`: Estado actual del Límite de Peticiones HTTP/FIX Rest API CEX (Skill 35 L2 O(1)).

## 5. Salidas esperadas
- `phantom_order_dispatches`: Ráfagas de Comandos de Inyección Limit a Ticks lejanos L2 CEX O(1).
- `flash_cancel_triggers`: Señales de Auto-Destrucción Cripto HFT que borran la orden fantasma L2.
- `stealth_execution_slices`: Pedazos aleatorios `(TWAP/VWAP Camouflage L2)` para ejecutar órdenes maestras indetectables Cripto L1 L2.

## 6. Reglas inmutables
- TODA ORDEN L2 catalogada como "Ghost/Spoof Order" (Diseñada para influir la Microestructura y no para Ejecutarse) DEBE contener una condición local de Time-To-Live HFT de (Ej. 10 a 50 milisegundos). Si el Timeout se alcanza, el Socket L2 REST/FIX lanza `CancelOrder` atómicamente, garantizando Riesgo Financiero Cero O(1) L2 de quedarse Atrapado en Direccionalidad Falsa.
- La Táctica de "Quote Stuffing L2" y Saturación solo debe emplearse en Regímenes HMM Competitivos L2 CEX (Skill 48 L2) (Cuando los Márgenes de Arbitraje L1 L2 caen a menos de 0.02% por culpa del exceso de bots Takers HFT L2). Apagar la Decepción en Bull-Runs limpios (Menos manipulación, Más extracción O(1)).
- Obligación Regulatoria Compliance CEX L2: Solo usar Iceberg y Fuzzing HFT de Ruteo O(1). El Spoofing Agresivo Puro L2 CEX es baneable en jurisdicciones SEC/CFTC CEX L1 L2. Solo habilitarlo en protocolos DEX L1 L2 Decentralizados o Jurisdicciones sin Veto Micro-Estructural L2, operando bajo estricta asimetría cripto legal O(1).

## 7. Algoritmos o métodos que debe conocer
- Game Theory of High Frequency Trading (Adversarial HFT Dynamics).
- Poisson Arrival Processes for Order Concealment L2 (Simular ser muchos Traders L2 distintos).
- Micro-Latency Spoof Dynamics CEX FIX Protocol L2.

## 8. Fórmulas críticas
- **Ghost Wall Distance Limit L2**: `Safe_Distance_Ticks = Maximum_Competitor_Latency_Ms * Maximum_Tick_Speed_Per_Ms_L2` (Poner el muro a una distancia matemática inalcanzable por un Market Order enemigo).
- **Iceberg Sizing Randomizer O(1)**: `Display_Amount = Optimal_Execution_Size * Random_Gaussian(0.05, 0.15)` (Muestra solo del 5% al 15% aleatorio del Total Base O(1) HFT).
- **Ping Discovery Delta L2**: `Latency_Diff = Time_To_Fill_Ping - Time_Sent_Ping_L2` (Detectar si hay Market Makers Institucionales O(1) conectados al API Server CEX de Tokyo).

## 9. Casos extremos
- Front-Running de tu propio Ghost Wall (Sniper Trap L2): Pones un Muro de Compra Fantasma L2. Un Bot ENEMIGO rapidísimo de Jane Street CEX L2 lee tu Muro, asume que es real L2, te "Front Runea" y pone un Bid un tick encima del tuyo L2 CEX. El Precio de Verdad Cae L1 L2. El Bot Enemigo es Llenado (Liquidado Negativo). El Mercado se acerca a TU MURO Fantasma L2. Entras en pánico y Cancelas O(1) CEX (El Plan original L2). PERO el CEX se satura de TPS y tu orden de Cancelar L2 queda atascada 500ms API Lag O(1). ¡Tu Ghost Wall se ejecuta! Tienes 10 Millones Long L2 no deseados. Solución Atómica: Toda Ghost Wall L2 HFT O(1) DEBE enviarse con Parámetro API Institucional `TIME_IN_FORCE = FOK o IOC / Cancel-Delay Nativo CEX L2` (Kill Switch Server Side CEX O(1)) o a un nivel de Pánico Macro Inalcanzable.
- Detección Enemiga de tu Patrón de Fuzzing L2 (Reverse Fingerprinting CEX): Tu Bot usa un generador Aleatorio simple `Math.random()`. La IA predictiva HFT Enemiga Cripto se da cuenta que TODAS tus órdenes acaban en ".345" o en secuencias lógicas L2. Dejas de ser invisible CEX L2. Solución: Usar Cryptographically Secure Pseudo-Random Number Generators (CSPRNG HFT O(1)) inyectados con Semillas L1 Externas L2 (Variables de estado caóticas del mempool O(1) HFT) para despistar Redes Neuronales enemigas.
- Exceso de Ratio de Cancelación (Binance Penalty Tier L2): Binance HFT API penaliza Cuentas si el `UCR (Unfilled-to-Cancel Ratio)` pasa de 10,000 CEX L2 O(1). Tu Bot CEX L2 se la pasó 5 horas haciendo Fake Walls L2. Binance cierra tu API por Abuso HFT L2 CEX. Solución O(1): Módulo Limitador L2 O(1) que cuenta C/Rust in-memory. Si el Ratio toca 8,000, Apaga Atómicamente la Lógica L2 Deception Cripto O(1) por 24 Horas HFT.

## 10. Validaciones obligatorias
- PRE: Chequeo de Mismatch Contable. Si el Agente HFRC Cripto O(1) REALMENTE quiere comprar 10 ETH (Skill 13 L2). La Skill 69 Deception NO DEBE enviar una Ghost Order contraria (Vender Falso 50 ETH L2) que induzca "OrderBook Imbalance O(1) Bajista" porque eso sabotearía nuestra propia Skill 62 Predictora L2 CEX de Orquestación General. Las mentiras L2 no deben intoxicar los Datos Propios O(1).
- CÁLCULO: Evaluar el Costo de Ejecución de la Decepción. (Enviar 500 órdenes HTTP CEX L2 consume CPU Node/Rust, O(1), Ram). Limitar Spoofing O(1) exclusivamente a cuando el `AUM_USD_Value_At_Risk_HFT` justifica una operación de Guerra Táctica Cripto L1/L2 de Enmascaramiento.
- POST: Si la Orden Fantasma O(1) Falla el Cancelar y se Ejecuta por Accidente API L2, Emitir Alerta de Desastre Direccional ("Ghost Wall Breached CEX L2"). La Skill 61 HFT L2 Perpetuos debe Atajar Inmediatamente el 100% de la Ejecución L2 Atrapada con un Hedge Neutro L1 L2 para apagar el Incendio Financiero O(1).

## 11. Criterios de aprobación
- Capacidad Empírica L2 de ejecutar Iceberg Orders y Random-Sized TWAPs L2 O(1) demostrando (En Backtest HFT O(1)) una Reducción Drástica del Slippage Taker CEX L2 (Muestra de que la competencia HFT Cripto O(1) NO reaccionó/Huyó de tus envíos masivos L2).
- Mantenimiento riguroso del Ratio de Cancelaciones CEX por Debajo del Umbral de Baneo L2 API O(1) Institucional L2 Cripto.

## 12. Criterios de rechazo
- Usar Spoofing/Deception L2 HFT O(1) en Tickers/Símbolos que NO ESTÁS OPERANDO ACTUALMENTE L1 L2. Generar ruido en pares ajenos L2 Cripto no te da Beneficio HFT, CEX L2 lo marca como Manipulación Pura y Peligrosa L1 O(1), gastando tu ancho de banda Rate Limit L1 L2 O(1) inútilmente.
- Dependencia Total de Tácticas "Grises" Cripto. Un Algoritmo HFT de Clase Mundial Gana O(1) por su Alpha O(1) y Precisión Matemática de Arbitraje L1 L2 (Skill 53 L2, Skill 64 L1 L2), no por asustar ciegamente el Orderbook L2 CEX O(1) para ver qué pasa. Este Módulo es Táctico/Opcional L2.

## 13. Riesgos que mitiga
- La Asfixia Estructural por Huella de Carbono HFT (Algorithmic Signatures L2 O(1)). Si operas millones Cripto L2 HFT L1 con la misma Secuencia L2 (Ej. Siempre Compras y Vendes 1 BTC L2), te vuelves predecible HFT. Los Searchers Enemigos O(1) te leen la mente (Skill 47 Inversa Enemiga L2), y comienzan a Poner Órdenes L2 O(1) exactamente por debajo de ti (Vampirism L2 CEX). El Spoofing y Camuflaje L2 Destroza O(1) los modelos ML de tus enemigos L2, dándote Invisibilidad Dinámica y Protegiendo tu Alpha L2 L1 O(1) para que no sea Arbitrado por un Competidor L2 HFT mejor fundado.

## 14. Integración con otras skills
- Escudo Ocultador del Execution Dispatcher (MUX Skill 64 L1 L2 O(1)).
- Interacciona Peligrosamente con Microestructura O(1) L2 (Skill 62).
- Contabiliza sus fallos fatales L2 O(1) (Penalidades) en Cost-Ledger (Skill 38 L2).

## 15. Modelo de datos sugerido
```json
{
  "DeceptionTacticsEngine": {
    "job_id": "ICEBERG_STEALTH_DUMP_100BTC",
    "timestamp_ms": 1714521234105,
    "strategy": "ICEBERG_PLUS_FUZZING_CEX_L2",
    "target_asset": "BTCUSDT",
    "total_size_to_hide_usd": 6500000.0,
    "stealth_parameters_o1": {
      "visible_display_amount_chunk_min": 0.05, // Random 5%
      "visible_display_amount_chunk_max": 0.12, // Random 12%
      "fuzz_delay_ms_min": 150,
      "fuzz_delay_ms_max": 800
    },
    "ghost_walls_active_l2": false, // Banned on Binance due to Strict Enforcement, Disabled.
    "api_cancellation_ratio_current_l2": 2450.5, // Well below 10,000 penalty CEX threshold L2
    "status": "EXECUTION_CLOAKED_L2_ACTIVE"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase Middleware HFT L2 `OrderFlowCamouflageEngine`. Todo envío Real del Agente (Skill 36 O(1) Maestro) pasa por este Filtro L2. Si el Tamaño es Masivo L2 O(1), El Filtro intercepta la Llamada L2, Muta el Payload JSON API L2, lo divide O(1), y Dispara Múltiples Request HTTP/FIX CEX HFT In-Memory O(1) Cripto L2.

## 17. Logs obligatorios
- `[DEBUG] Stealth Engine L2 CEX: Master Requested Dump of $500k SOL O(1) L2. Slicing into 15 Asymmetric Random Chunks L2. Dispensing over 2.5 seconds HFT to avoid triggering Enemy OBI/CVD Scanners L2.`
- `[INFO] Deception Ghost Wall Triggered L2 DEX L1 O(1). Placed $1M Fake Buy Wall on Uniswap V3 L1 out of range. Enemy MEV Searcher Front-Ran the Wall O(1) Pumping Price 0.5% L1. CEX Mux (Skill 64) Executed Target Arbitrage 0.5% Cheaper L2 HFT O(1). Cancelled Fake Wall O(1).`
- `[CRITICAL] CEX Penalty Ratio Approaching Limits O(1) L2! Cancellation/Fill Ratio at 8500. Disabling ALL Quote Stuffing and Spoofing L2 Tactics for 12 Hours to Cool-down API Keys Institutional L2.`

## 18. Métricas obligatorias
- `average_order_slicing_count_l2` (Para monitorear fragmentación L2 CEX O(1)).
- `stealth_saved_slippage_bps_l2` (Slippage Cripto L2 comparado con el Slippage que hubiera ocurrido L1 O(1) si la Ballena O(1) atacaba de frente L2).
- `api_unfilled_to_cancel_ratio_l2` (Vida o Muerte CEX L2 API KEY O(1)).

## 19. Tests unitarios
- Randomized Slice Generator O(1): Pasar $100k Order Size L2. Limitador HFT Random `[5%, 15%]`. El Módulo In-Memory C++ DEBE devolver Array `[$8k, $14k, $5k, $12k...]` que Sumen EXACTAMENTE `$100,000` O(1) sin Desperdiciar 1 Centavo L2 HFT (Conservation of Mass Test L2).
- Fuzzing Time-Delay Check L2: El Output Async Payload Array DEBE contener delays de ejecución `(setTimeout/Threads HFT O(1))` No-Lineales `(Ej. 10ms, 45ms, 120ms, 25ms)` O(1). Si los Delays resultan Lineales Fijos `(100ms, 200ms, 300ms)`. Falla Test por Patrón Predecible Cripto L2 Enemigo O(1).
- Cancel-Ratio Safety Governor O(1): Forzar Inyección Artificial O(1) al Módulo Local L2 de `10,000 Cancelaciones L2 CEX O(1) Falsas`. El Governor (Límite L2) DEBE interceptar la ejecución del Siguiente Spoof L2 O(1) arrojando Excepción Segura `API_RATIO_EXCEEDED_HALT_O1` bloqueando la Operativa Peligrosa HFT L2 In-Memory Cripto O(1).

## 20. Tests de integración
- Conectar OrderFlow CEX Mock API L2. Iniciar el "Ping Trading Discovery" L2 O(1). El Bot Envía `$10 Compra L2`. El Servidor Mock L2 CEX responde con un Latency L2 de `15ms`. El Mock se configura con "Fake Enemy Bot L2" que reacciona a los $10. El Modulo L2 Local In-Memory Capta L2 O(1) que el "Mercado L2 Reacciona 20ms Despues". Integra esta Constante de Latencia L2 a Skill 57 CEX L2, Confirmando el Feedback-Loop L2 de Cripto Decepción Táctica O(1) L1 L2 HFT.

## 21. Tests E2E
- El agente HFRC detecta una inmensa asimetría en un Par Iliquido CEX L2 HFT (Skill 12 L2 CEX). El Spread es masivo (2%), pero el Orderbook L2 es frágil como cristal L1 O(1). Si el Agente golpea el CEX L2 con $1 Millón, el slippage L2 Cripto será del 5% y perderá 3% Neto HFT L2. El Agente Activa Skill 69 Deception L2 O(1). Manda una Iceberg Order Fragmentada L2 CEX con Algoritmo Poisson-Arrival L2 Cripto. Los bots enemigos de Market Making L2 no ven el Muro L2 HFT O(1). Proveen liquidez pasivamente asumiendo "Retailers Random CEX L2 comprando de a poquito L1 L2 O(1)". El Agente consume $1 Millón entero HFT O(1) a lo largo de 35 segundos con Disparos Micro-Ocultos L2 CEX. El Spread masivo de 2% se Extrae Completo L2 O(1) con solo 0.1% de Slippage Fantasma (Skill 59 O(1)). La cuenta maestra L2 engorda colosalmente mientras el Cripto-Mercado Enemigo O(1) L2 se pregunta a dónde fue a parar toda la Liquidez Base L1 L2. La Invisibilidad de Ejecución es Alpha Puro HFT O(1).

## 22. Checklist de producción
- [ ] Compliance MUX Firewall CEX L2 (Legal Constraints HFT): En Exchanges Altamente Regulados CEX L2 (Ej. Coinbase Pro L2, Kraken Institutional L1 L2), Desactivar TOTALMENTE el Módulo L2 de `Ghost Walls O(1)` y `Spoofing Cripto L2` desde la Configuración C++ YAML L1 O(1). Limitarlo a DEX L1 On-Chain AMMs O(1) o Exchanges Extranjeros Sin Marco Estructural L2 O(1) (Burbuja Offshore L2 HFT) para prever Congelación de Fondos CEX Fiat/Cripto HFRC Institucional O(1).
- [ ] Incorporar "Hidden Order Types" Nativos API CEX L2. Algunos CEX API L2 (Como Bitfinex/Binance) te permiten poner `Hidden: True` L2 CEX JSON. Si el CEX te Da La Invisibilidad Legal L2, USARLA O(1) en lugar de Gastar CPU Rust L2 Local Emulando el Iceberg O(1). Menos Rate Limit O(1), Más Alpha L2 O(1) Cripto.

## 23. Ejemplo de configuración no hardcodeada
```yaml
deception_and_spoofing_engine_l2_o1:
  enable_iceberg_and_camouflage_l2_o1: true
  enable_ghost_walls_spoofing_l2_o1: false # High Ban Risk on CEX, Disabled for Safety O(1)
  randomized_poisson_arrival_fuzzing_l2_o1: true
  max_api_penalty_ratio_limit_l2_o1: 5000 # Stop deceiving if Ratio hits 5k (Safe buffer from 10k CEX ban limit)
  camouflage_minimum_order_size_usd_l2_o1: 50000.0 # Don't hide small fish. Only camouflage whales HFT
  use_native_hidden_orders_if_supported_l2_o1: true
```

## 24. Ejemplo de pseudocódigo
```javascript
class HFTCamouflageEngine {
    constructor(apiManager) {
        this.api = apiManager;
    }

    async executeIcebergMuxedL2(asset, totalAmount, side) {
        // Fast Pre-Flight Legal Check O(1)
        if (totalAmount < CONFIG.min_order_size_camouflage) {
            return await this.api.submitOrder(asset, totalAmount, side); // Plain Execution
        }

        let remainingSize = totalAmount;
        let executions = [];

        log.info(`Applying O(1) HFT Iceberg Cloaking L2 to $${totalAmount} ${asset} ${side}...`);

        while (remainingSize > 0) {
            // Fuzz Size (Cryptographic Random)
            const randomChunkPct = Csprng.randomBetween(CONFIG.chunk_min, CONFIG.chunk_max);
            let chunkSize = totalAmount * randomChunkPct;
            
            if (chunkSize > remainingSize) chunkSize = remainingSize;
            
            // Fuzz Delay (Poisson or Uniform Random L2 O(1))
            const delayMs = Csprng.randomBetween(CONFIG.delay_min, CONFIG.delay_max);
            
            executions.push(new Promise(resolve => {
                setTimeout(async () => {
                    const res = await this.api.submitOrder(asset, chunkSize, side);
                    resolve(res);
                }, delayMs);
            }));

            remainingSize -= chunkSize;
        }

        return await Promise.all(executions);
    }
}
```

## 25. Criterio final de excelencia
Las Tácticas de Decepción y Camuflaje L2 (Order Flow Spoofing) otorgan la cualidad suprema de "Sigilo Táctico" HFT. Diferencian al Bot HFRC Básico (Que es ordeñado L2 por Ballenas debido a que exhibe todo su peso HFT Cripto de frente) del Bot Predator Apex L1 L2 (Que absorbe liquidez ajena como un Fantasma O(1) sin alertar los Modelos ML de Impacto CEX de Jane Street). Esta Criptografía Micro-Estructural L2 asegura la supervivencia contable del fondo al ejecutar arbitrajes titánicos en estanques CEX/DEX pequeños.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: API Rate Limit Suicide CEX L2 O(1). Si el Bot O(1) se la pasa mandando Fuzzing MUX Ping L2, la API CEX HTTP/FIX colapsa `429 Too Many Requests L2` O(1). Matar el Rate Limit significa que el Agente HFRC NO PODRÁ EJECUTAR HEDGING PERP L2 L1 O(1) cuando venga un Cisne Negro. Solución Exclusiva: Limitador Agresivo Global (Skill 35 L2 O(1)) que apaga la Guerra Cripto L2 si los Rates tocan 80% L2.
- Dependencias: API Rate Limiter (Skill 35 L2), Microstructure State (Skill 62 L2 O(1)).
- Próxima skill: Machine Learning Feature Store (Data Engineering) (Skill 70).
