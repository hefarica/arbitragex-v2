# SKILL 061 — Orquestador de Operaciones en Futuros Perpetuos (Hedge & Execution)

## 1. Propósito superior
Dominar la ejecución, el margen cruzado (Cross-Margin) y las coberturas (Hedging) direccionales dentro de los mercados de Futuros Perpetuos y Contratos por Diferencia (CFDs Cripto). A diferencia del Spot, operar en Futuros involucra Apalancamiento, Riesgo de Liquidación, Tasa de Financiación (Funding Rate) y Aislamiento de Colateral. Esta Skill es el brazo armado que el Agente utiliza para "Shortear" (Vender al descubierto), aislar el Riesgo Direccional (Delta Hedging) y operar el Arbitraje de Cash & Carry con precisión quirúrgica, asegurando que las liquidaciones algorítmicas jamás toquen el capital base del fondo.

## 2. Nivel de conocimiento requerido
Quant Trader Especialista en Derivados. Conocimiento experto de Mecánica de Contratos Inverse/Linear, Cross/Isolated Margin Modes, Liquidaciones Parciales (ADL - Auto Deleveraging), Index Price vs Mark Price Oracles, Initial/Maintenance Margin Limits y API FIX/REST de Binance Futures / Bybit / Hyperliquid.

## 3. Capacidades principales
1. Delta Hedging Instantáneo (Cobertura a Mercado): Si el bot hace Arbitraje Triangular L1 y se queda atascado con 10 ETH porque falló la tercera pata, esta skill inmediatamente abre una posición "Short" de 10 ETH en Perpetuos. Congela el valor en dólares en ese exacto milisegundo (Delta Neutral), dándole al bot tiempo infinito para salir del ETH físico sin sufrir riesgo de mercado si Ethereum colapsa.
2. Gestión de Margen (Margin Health Monitor): Monitorea constantemente el "Maintenance Margin Ratio". Si el precio se acerca peligrosamente al precio de Liquidación CEX, inyecta Colateral extra (Add Margin) desde la Tesorería General (Skill 40) para alejar la bancarrota (Margin Call Defense).
3. Cash and Carry Arbitrage Execution: Detecta si los Futuros Perpetuos cotizan 2% más caros que el Mercado Spot. Compra Spot, Shortea Perpetuo simultáneamente y se queda cobrando la "Tasa de Financiación" (Funding Rate - Skill 16) masivamente hasta que el precio converge, asegurando retornos astronómicos de Risk-Free rate.
4. Auto-Deleveraging (ADL) Defense: Sabe que si gana mucho dinero en un exchange, el CEX puede "Expropiar" sus posiciones rentables para pagar las deudas de los traders quebrados (Auto-Deleveraging). Monitorea el indicador ADL (Luces rojas) y reduce posiciones proactivamente cerrando contratos antes de ser despojado por el Exchange.
5. Modificador de Apalancamiento Dinámico (Auto-Leverage Tuner): Jamás opera a 100x de Leverage fijo. Ajusta el Apalancamiento al volumen nominal. Si necesita shortear $1000, usa 5x Margin ($200 colateral). Si la volatilidad aumenta (HMM Skill 48), reduce el leverage a 2x para expandir el "Colchón de Liquidación" (Liquidation Buffer).
6. Conversión Spot-Inverse: Maneja perfectamente la matemática de los contratos "COIN-M" (Inversos, liquidados en Bitcoin físico) vs "USDS-M" (Lineales, liquidados en Dólares/Tether), normalizando PnL y Size Commands (Contract Sizes, ej: 1 Contrato = 100 USD) para que el Orquestador HFT dispare "Size=1.5 BTC" y este motor lo traduzca legalmente a la API del Exchange.
7. Unwind Sintético (Desarme de Cobertura): Cuando el Agente logra vender los 10 ETH Físicos en Spot para salir de la trampa, esta Skill debe *exactamente a la vez* recomprar (Buy to Cover) los 10 ETH en Futuros. Cierre Atómico Perfecto.
8. Gestión de TWAP/VWAP en Futuros: Evita liquidar o abrir coberturas de $1 Millón de dólares de un golpe con Market Orders, destruyendo la rentabilidad por Slippage. Rebana las órdenes en pedazos de $10,000 en el tiempo óptimo.
9. Oráculo Interno de Divergencia Index/Mark (Mark Price Arb): Sabe que las liquidaciones se basan en el "Mark Price" (Índice base), no en el "Last Price" local. Evita el pánico ciego de cerrar posiciones si el precio local explota por un flash crash irreal pero el Index Price sigue a salvo.
10. Sincronización Multi-Exchange de Positions (Portafolio Consolidado): Lee y centraliza `GET /positionRisk` de Bybit, Binance, OKX y DEXes como GMX/Hyperliquid, otorgando una Visión Maestra del "Net Delta" Global del Bot. (Ej. Estoy Short 5 BTC en Binance y Long 3 BTC en Bybit. Delta = -2 BTC).

## 4. Entradas requeridas
- `directional_exposure_alerts`: Señal de la Tesorería o Skills de Ejecución indicando: "Atrapados con $10,000 en SHIB, CUBRIR!".
- `futures_orderbook_and_mark_price`: Websockets del contrato y de la tasa de financiamiento (Skill 16 y 33).
- `margin_balances_stream`: El capital libre en la billetera cruzada de Futuros (`Cross Wallet Balance`).

## 5. Salidas esperadas
- `futures_order_commands`: API REST/FIX comandos (`PlaceOrder: Short 15.5 BTC, Type: Market/Limit, ReduceOnly: true`).
- `margin_transfer_requests`: Llamadas internas a Skill 40 para transferir fondos de la Billetera Spot a la Billetera de Futuros CEX.
- `global_delta_telemetry`: Float reportando Exposición Direccional Neta (`0.0` si todo está debidamente cubierto).

## 6. Reglas inmutables
- TODAS las órdenes para cerrar una cobertura (Buy-to-Cover o Sell-to-Close) DEBEN usar obligatoriamente la bandera `ReduceOnly=true` de los Exchanges. Esto previene un error fatal donde el bot intenta cerrar 10 Shorts, se laguea, lo envía dos veces, y accidentalmente abre 10 Longs direccionales exponiendo el fondo a liquidación (Flip position error).
- NUNCA exceder un Apalancamiento Efectivo de Cartera Global superior a `3.0x` (Margin Ratio Mantenimiento Ultra Seguro). Aunque el CEX permita 125x, el Bot jamás usa más del Capital necesario para tener una protección de Flash Crash del 30% en contra.
- En Coberturas de Arbitraje Estadístico (Skill 55) usar SIEMPRE el modo "Cross-Margin" (Margen Cruzado) para que el PnL positivo del Long financie la pérdida no realizada del Short (Floating Loss), evitando liquidaciones asimétricas de piernas aisladas (Isolated Margin Suicide).

## 7. Algoritmos o métodos que debe conocer
- Ecuaciones de Liquidación Exacta de Binance/Bybit (Cálculo de Liquidation Price a mano O(1)).
- Funding Rate Arbitrage Mechanics.
- TWAP (Time-Weighted Average Price) Slicing Algorithms.

## 8. Fórmulas críticas
- **Liquidation Price (Linear Long)**: `Liq_Price = Entry_Price * (1 - Initial_Margin + Maintenance_Margin)` (Simplificado, requiere añadir Funding / Fees).
- **Delta Neutral Math**: `Total_Spot_Value_USD + Total_Futures_Notional_Value_USD = 0` (Para estar blindado).
- **Hedge Conversion to Contracts**: `Quantity_Contracts = Total_Spot_Amount / Contract_Multiplier_Size`

## 9. Casos extremos
- Limit Up / Limit Down (Velas Aguja): Un token explota +400% en Futuros pero 0% en Spot (Squeeze Masivo). El bot está Short en futuros y Long en Spot. El PnL total es Cero. Pero como el Margin es cruzado solo en Futuros, el Exchange Liquida la posición de Futuros robándose el colateral, dejando el Spot Long desnudo. Cuando el precio de futuros cae de vuelta a la realidad, perdiste dinero por culpa de Separación Fáctica de Colaterales. Solución estricta: Orquestador de Margen (Skill 42 y 61) que detecta el estrangulamiento L2 (Squeeze) y manda colateral de emergencia desde Spot antes de la guillotina.
- Funding Rate Mortal (Whipsaw Funding): El Bot entra en un Cash and Carry Trade buscando ganar 0.05% cada 8 horas (El perp paga al Short). De repente, los Toros capitulan, y el Funding Rate se invierte violentamente (-2.00% al día). Ahora el Short PAGA al Long. El Bot está sangrando capital pasivo. La Skill monitorea las tasas en Predictivo (Next Funding / Premium Index). Si la Tasa Nominal Invierte signo (Funding Rate Reversal), ejecuta Unwind de la estrategia de acarreo inmediatamente protegiendo la media mensual.
- Rechazo de APIs Límite de Apalancamiento (Notional Brackets): CEXes te bajan el Leverage si la posición es gorda. (De 0 a 50k = 50x. De 50k a 200k = 20x). El Bot calcula el `Initial Margin Required` dinámicamente según la Bracket List pública del Exchange en memoria, de lo contrario un envío Multi-millonario devolverá `INSUFFICIENT_MARGIN` rompiendo la atómicidad del Arbitraje Estadístico en milisegundos.

## 10. Validaciones obligatorias
- PRE: Asegurar concordancia de Activo Base. "Estoy atascado con `WBTC` on-chain L1". La Cobertura DEBE ser en `BTCUSDT` Perpetuo. Mapear el `Wrapper` exacto en los Diccionarios Lógicos. No shortear `BCH` por accidente al leer mal el Ticker.
- CÁLCULO: Validar si la Tasa de Préstamo (Borrow Rate) Spot + Funding Rate Futuros devora el "Expected Arbitrage Spread". (Si el Spread de convergencia da $15, pero mantener el Short 1 semana cuesta $30, Rechazo Operativo Total).
- POST: Vigilar que la Exposición Neta Global se mantenga cerca a Cero. Si el Sistema indica "Hedged = True" pero el Dashboard dice "Delta: +50 ETH", un error de "Contract Size Multiplier" se ha colado (Compraste 50 Contratos que valían 0.01 ETH en vez de 1 ETH cada uno). Fallo catastrófico de Cuantización.

## 11. Criterios de aprobación
- Respuesta Sub-milisegundo para levantar "Emergency Shorts" (Coberturas de emergencia) tan pronto la Skill Principal HFT reporta un Fallo de Leg (Broken Leg).
- Cálculo in-memory predictivo del Nivel de Liquidación Exacto de toda la sub-cuenta (< 0.1% de margen de error respecto al Exchange UI).

## 12. Criterios de rechazo
- El uso de Órdenes a Mercado Gigantescas sin VWAP para abrir o cerrar posiciones direccionales. El Impacto de Precio (Slippage Skill 59) te arrancará 1% a 2% de rentabilidad neta matando tu PnL HFT (Que lucha por ganar 0.05% a la vez). Se debe escalar la salida (Drip out) usando TWAP en segundos/minutos.
- Olvidar la Configuración de Modo "One-Way" vs "Hedge-Mode". Si el bot requiere tener un Long y un Short abiertos al mismo tiempo en el mismo exchange (Arbitraje Complejo), la API fallará bloqueando órdenes si la cuenta no fue setada a "Hedge Mode" en los Settings del Perfil Base. (Initial Setup Blocker).

## 13. Riesgos que mitiga
- Riesgo Direccional Absoluto (Naked Exposure Risk): Estar "Atascado" en cripto es fatal. Compras LUNA a $100 esperando un arbitraje, el arbitraje falla, LUNA baja a $0 en horas. Pierdes el 100% de capital. Al tener el Módulo Perpetuos armado como Gatillo Rápido, el Agente presiona el botón "Delta Neutral". Pierdes la comisión del exchange ($0.50), pero te desconectas totalmente del riesgo del abismo, convirtiendo a la Inteligencia Artificial en Virtualmente Inmune a Crashes (Flash Crash Immunity).
- Falla de Convergencia Larga (Duration Risk): Las estrategias estadísticas a veces tardan semanas en revertirse. Sin "Cash and Carry / Funding Rate Monitoring", el Bot muere por los "Costes de Almacenamiento Sintéticos". (Bleeding alpha).

## 14. Integración con otras skills
- Socio de Acción de la Skill 16 (Arbitraje de Funding Rate).
- Herramienta de Salvavidas para el Triángulo Interrumpido (Skill 53 y 54 - Broken Leg Hedging).
- Alimentador crucial de métricas a la Skill 41 (Global Risk Engine / Margin Limit Dashboard).

## 15. Modelo de datos sugerido
```json
{
  "FuturesHedgeExecution": {
    "hedge_id": "EMERGENCY_COVER_ETH_442",
    "timestamp_ms": 1714521234105,
    "trigger_source": "TRIANGULAR_ARB_LEG_FAILURE",
    "spot_physical_exposure_usd": 45000.0, // We are stuck Long $45k physical ETH
    "action": {
      "exchange": "binance_futures",
      "pair": "ETHUSDT",
      "side": "SELL",
      "notional_size_usd": 45000.0,
      "leverage_configured": 2.0,
      "margin_type": "CROSSED"
    },
    "current_mark_price": 3000.55,
    "estimated_liquidation_price": 4500.22, // Highly safe (Needs 50% jump to liquidate)
    "reduce_only_flag": false,
    "status": "HEDGE_LOCKED_AND_ACTIVE"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase `FuturesOrchestrator` centralizada con Websockets privados a los endpoints `/ws/userData` de Futuros. Contiene el método `emergencyDeltaNeutralize(asset, amount)`, invocable por el Event Bus Global.

## 17. Logs obligatorios
- `[INFO] Hedging Orchestrator: Spot Arbitrage Leg 3 Failed. Trapped in Long SHIB ($50k). Instantly firing 50k Notional Short SHIBUSDT on Perpetual Markets. Delta Neutrality restored.`
- `[DEBUG] Margin Monitor Warning: Maintenance Margin cross 80% boundary on OKX Perps due to localized squeeze. Auto-Transferring 50,000 USDT from Spot Wallet to Futures Wallet to defend Liquidation threshold.`
- `[CRITICAL] Funding Rate Reversal Detected! Short Leg paying Longs heavily. Immediate VWAP Unwind of Statistical Arbitrage initiated to prevent synthetic bleeding.`

## 18. Métricas obligatorias
- `average_time_to_hedge_ms` (Tiempo entre el fallo del trade físico y la colocación del Short sintético).
- `global_delta_usd` (El Santo Grial visual: Debe orbitar cerca del 0.0 constante, si sube a $1M, el bot se volvió "Apuesta Simple").
- `accumulated_funding_fees_pnl` (Para saber si el seguro nos está costando o pagando).

## 19. Tests unitarios
- Liquidation Price Math: Inyectar un colateral (Billetera cruzada = `$1,000`). Entrar Short por `$10,000` Notional (Apalancamiento efectivo 10x). Precio de Entrada `$100.00`. Maintenance Margin Tier CEX = `0.40%`. El simulador C++ / Node DEBE devolver en el cálculo local el `LiqPrice` exacto (Aprox. `$109.60`). Comprobar que encaja al centavo con la Fórmula API oficial del exchange, previniendo Falsas Alarmas o Liquidaciones Ciegas.
- Leverage Bracket Auto-Downsize: Proveer un envío de `$1,000,000` de volumen de Short. El Exchange dicta que Volúmenes > $500k máximo aceptan `10x`. Si el código fuente lo mandaba por defecto a `20x`, el Test DEBE fallar en el interceptor local y Auto-Modificar el JSON API payload a `"leverage": 10` sin hacer round-trip inútil que sea rebotado en HTTP 400 Bad Request por la API del Exchange.
- Reduce Only Rejection: Forzar al bot a querer cerrar una posición `Long de 5 BTC`. Mandar `Sell 15 BTC (ReduceOnly=True)`. El orquestador DEBE validar que el Exchange SÓLO venderá 5 BTC y cortará la orden ahí mismo, previniendo quedarse Short de `10 BTC` desnudo, garantizando inmunidad a "Fat Finger" / Errores lógicos Multi-Threading.

## 20. Tests de integración
- Levantar servidor Falso con API de Binance Futuros (Mock L2). Enviar el "Hedge Atómico". Ejecutar subprocesos donde un simulador sube el precio (Pump and Dump) del mercado falso. Comprobar que el Motor de Defensa (Margin Monitor) del Bot en segundo plano dispara llamadas REST a la API Falsa de `POST /sapi/v1/asset/transfer` moviendo fondos Spot->Futures correctamente ANTES de que el Liquidation Engine del simulador falso decrete la bancarrota de la posición.

## 21. Tests E2E
- El agente HFRC detecta y ejecuta Triangular intra-exchange en Binance: Compra USDT->PEPE->BTC->USDT. Por infortunio, la liquidez de PEPE->BTC colapsa instantáneamente antes del trade por bots institucionales. El Bot compra los PEPE con USDT, pero falla al intentar comprar el BTC. Se queda "Dormido" direccionalmente Long con $100,000 en la moneda meme PEPE, la cual empieza a desangrarse -5%. El Agente (Event Bus) detecta el Parcial Fill Lógico de la pata faltante, invoca al Futures Orchestrator (Skill 61). En 8 milisegundos, el Orquestador dispara una "Market Order" de Short PEPE-USDT Perpetual por $100,000 Notional. El Delta se aísla de inmediato. PEPE sigue cayendo al -20%. La Billetera Spot pierde -$20,000 en papel, pero la Billetera de Futuros gana +$20,000 en el Short. El PnL neto global es -$10 de comisiones del Exchange. El Fondo salva $20,000 de capital y sobrevive el Flash Crash ileso en Misión de "Damage Control Atómico".

## 22. Checklist de producción
- [ ] Universalización de Size (Contract Multipliers): Bybit a veces usa "1 Contrato = 1 USD". Binance a veces usa "1 Contrato = 1 BTC". OKX "1 Contrato = 100 USD". Jamás pasar dólares crudos (`size_usd`) al Dispatch. El Normalizador debe dividir y "Redondear/Truncar" los lotes según el `stepSize` y `lotSize` base, de otra forma la urgencia del Hedge rebotará con errores sintácticos arruinándote la vida.
- [ ] Hedge Liquidity Sanity Check: A veces te quedaste "Largo" en el Spot de una Shitcoin, y quieres Shortearla en Perpetuos. PERO la moneda Perps "PEPE-USDT" no tiene $100k de liquidez en el orderbook para vender en corto. El Orquestador tiene que "Comprar Sintético Correlacionado" (Ej. Shortear SHIB-USDT como proxy) o hacer TWAP Lento; no enviar Mkt Order al vacío destruyendo la cuenta con 10% de slippage sintético defensivo.
- [ ] Margin Mode Initialization Script: El bot NUNCA debe arrancar su motor central sin antes enviar peticiones REST POST a todos sus CEXes forzando: `MarginType=Cross` y `PositionMode=HedgeMode`. Los CEXes a veces actualizan y resetean tus configuraciones a "One-Way Isolated". Forzar este Setup Programático en el Bootstrap (Arranque) salva Vidas Corporativas Cuantitativas.

## 23. Ejemplo de configuración no hardcodeada
```yaml
perpetual_futures_orchestrator:
  enabled: true
  exchanges_configured: ["binance_futures", "bybit_perps", "hyperliquid"]
  global_leverage_target: 2.0 # Operate at 2x Effective Leverage to sleep peacefully
  maximum_acceptable_slippage_for_emergency_hedge_bps: 200 # Accept up to 2% slip if avoiding total ruin
  margin_defense_mechanisms:
    maintenance_ratio_trigger_auto_deposit: 0.85 # If margin ratio hits 85%, pull funds from Spot to defend
    margin_add_chunk_usd: 10000.0
  fallback_correlated_hedging_enabled: true # If no direct perp exists, short highly correlated asset (Skill 55 stats)
```

## 24. Ejemplo de pseudocódigo
```javascript
class FuturesHedgeOrchestrator {
    constructor(apiManager, ledger) {
        this.api = apiManager;
        this.ledger = ledger;
    }

    // Called asynchronously when the Spot Execution Engine suffers a failed Leg
    async emergencyDeltaNeutralize(assetSymbol, spotExposureUsd, failureSide) {
        // We are trapped. If failureSide is BUY, we hold USDT (Safe). 
        // If failureSide is SELL, we hold Crypto (Danger, must Short it).
        if (failureSide === 'BUY' || isStablecoin(assetSymbol)) return; 

        log.critical(`BROKEN LEG EMERGENCY. Stranded with $${spotExposureUsd} of ${assetSymbol}. Isolating Delta...`);

        const perpSymbol = this.api.getPerpMapping(assetSymbol);
        
        // Slippage & Depth Check before plunging in blindly
        const maxAvailableVolume = await OrderbookMemory.getDepth(perpSymbol, CONFIG.max_slip_bps);
        const actualHedgeSizeUsd = Math.min(spotExposureUsd, maxAvailableVolume);

        const orderCmd = {
            symbol: perpSymbol,
            side: 'SELL', // Shorting to hedge physical Long
            notionalUsd: actualHedgeSizeUsd,
            type: 'MARKET', // Speed is life
            reduceOnly: false,
            leverageTarget: CONFIG.global_leverage_target
        };

        const res = await this.api.submitPerpetualOrder(orderCmd);
        
        if (res.status === 'FILLED') {
             log.info(`DELTA SECURED. Hedged ${actualHedgeSizeUsd} USD. Remaining Unhedged Risk: $${spotExposureUsd - actualHedgeSizeUsd}`);
             this.ledger.registerHedgeAttachment(assetSymbol, perpSymbol); // Tie them logically
        } else {
             // Dispatch fallback correlation hedge if direct fails
             await this.fallbackCorrelatedHedge(assetSymbol, actualHedgeSizeUsd);
        }
    }
}
```

## 25. Criterio final de excelencia
El Orquestador de Futuros Perpetuos es el Paracaídas y el Traje NBQ (Nuclear, Biológico, Químico) del Agente Supremo. Elimina el factor "Suerte" (Adivinar que el mercado no caerá) de la ecuación de Arbitraje, inyectando un escudo de Acero Deltas Neutros. Logra que el Agente HFT interactúe con los mercados de derivados más sanguinarios del planeta Cripto sin sufrir el terror liquidatorio humano, transformando apuestas de riesgo direccional en laboratorios puros de diferenciales sintéticos sin fricción contable letal.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Black-Swan Liquidations (Flash crash de 80% en 1 milisegundo "Scam Wick"). El Margin Call Defense no tendrá tiempo de mover el colateral Spot->Futuros porque la red API CEX se ahoga (Timeout). Solucionado mantiendo SIEMPRE el Cross-Margin con colateral holgado pre-depositado inactivo en la bóveda L2 y manteniendo leverages sub-3x.
- Dependencias: API FIX/REST CEX (Skill 31), Margin Balances LEDGER (Skill 38) y Trianguladores.
- Próxima skill: Análisis de Micro-Estructura (Order Imbalance) (Skill 62).
