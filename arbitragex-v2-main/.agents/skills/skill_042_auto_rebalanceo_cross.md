# SKILL 042 — Auto-rebalanceo cross-chain / cross-exchange

## 1. Propósito superior
Automatizar la redistribución de la liquidez del fondo cuando se acumulan desequilibrios estructurales (Inventory Skew). Dado que el Arbitraje Espacial suele drenar activos de un exchange para acumularlos en otro, este módulo "Barredor" (Sweeper) identifica asimetrías de saldos, calcula la ruta de rebalanceo on-chain/off-chain más barata (considerando Gas, Fee de retiro y Latencia) y despacha las transferencias asíncronas de manera autónoma para restablecer la potencia de fuego equilibrada del bot (Resupply).

## 2. Nivel de conocimiento requerido
Especialista en Tesorería y Enrutamiento Inter-bancario (Treasury Logistics). Dominio de redes blockchain de Capa 1 y Capa 2, tiempos de bloqueo, seguridad de finalidad (Finality - Bloques de confirmación de depósitos CEX), estructuras de `Withdrawal APIs`, y optimización de costes de puenteo (Bridges vs CEX withdrawals).

## 3. Capacidades principales
1. Detección de Desbalance (Skew Tracking): Evalúa continuamente las asimetrías reportadas por el Inventario Global (Skill 40). Si Binance tiene 90% del USDT y Kraken 10%, lanza un rebalanceo hacia la media (50/50).
2. Routing de Mínimo Coste (Cheapest Path): Evalúa si es más barato retirar dinero de Binance a Arbitrum (USDC), o si es mejor convertir USDC a XRP, enviar XRP a Kraken pagando 0 fees, y vender XRP por USDC en Kraken para recomponer el saldo original (Synthetic Rebalancing).
3. Estimación de Riesgo de Mercado durante Rebalanceo Sintético: Si usa XRP como "Moneda de Transporte", asume el riesgo direccional de que XRP baje 5% en los 2 minutos que tarda en llegar y venderse. El bot calcula si el ahorro en fees de red justifica el riesgo de mercado temporal (Hedge o Unhedged Rebalance).
4. Auto-Whitelist y Gestión de Direcciones L1/L2: Mantener el "Address Book" local enlazado a los Whitelists de Retiro de los CEX y enviar solicitudes firmadas a través de sus APIs de Retiro (Withdrawal API).
5. Monitoreo de Transacciones en Vuelo (In-Flight Tx Tracker): Rastrear hashes de transacciones L1 en la mempool, o estatus de CEX (`Processing`, `Completed`) sin bloquear a los workers matemáticos.
6. Restricción de Bloques Confirmados (Deposit Confirmation Wait): Conocer heurísticamente que Kraken requiere 64 bloques de Ethereum (12 mins) y Binance 12 bloques (2 mins), para no disparar alertas de "Fondos Perdidos" antes de tiempo.
7. Evitación de Horas Pico (Gas Optimization): Si la asimetría de balance no es crítica, el Auto-rebalanceo pospone el retiro en Ethereum hasta las 02:00 AM UTC cuando el Gas Gwei baja en un 70%.
8. Pila de Consolidación (Dust Sweeping): Usar las APIs nativas del exchange para consolidar remanentes ("Convert small balances to BNB/OKB") semanalmente.
9. Contabilidad de Pérdida por Peaje (Toll Fee Amortization): Restar el coste del rebalanceo (Retiro CEX + Gas) directamente del PnL Global para evitar falsos profits inflados.
10. Pausa Activa de Cuentas Rebalanceando: Avisar al Orquestador que la cuenta X está bajo "Restock" y su capital en vuelo NO debe usarse para calcular Delta Exposure direccional.

## 4. Entradas requeridas
- `inventory_skew_alerts`: Eventos disparados por la Skill 40 (Inventario Unificado).
- `gas_price_oracles`: Datos de costo Gwei en las L1/L2.
- `exchange_withdrawal_fees`: Tabla estática/dinámica de costos de extracción.
- `network_congestion_status`: Estado actual de puentes inter-cadena (Bridges).

## 5. Salidas esperadas
- `rebalance_execution_receipt`: ID de orden de retiro y TxHash.
- `inventory_lock_command`: Bloqueo virtual en la contabilidad del saldo en movimiento.
- `replenish_complete_event`: Señal final cuando el exchange B acredita el fondo enviado por el exchange A.

## 6. Reglas inmutables
- JAMÁS retirar el 100% del saldo de una moneda L1 (Ej. ETH en Ethereum, MATIC en Polygon). Se DEBE dejar un remanente intocable para pagar futuras transacciones de Gas (Gas Buffer Reservoir), de lo contrario la wallet principal del agente quedará "Brickeada" e inoperante.
- Todas las direcciones destino para Rebalanceos deben provenir de una CONSTANTE estricta y pre-configurada (Address Book inyectado en RAM al inicio). El bot NUNCA debe generar o concatenar direcciones destino dinámicamente como prevención ante Hacks de inyección en código.
- Los Rebalanceos Sintéticos (Vender USDT por XRP -> Enviar XRP -> Vender XRP por USDT) sólo se permiten si el Costo proyectado (Slippage Buy + Fee Withdraw + Slippage Sell) es MENOR al Withdrawal Fee directo de la Stablecoin AND la exposición temporal de mercado se cubre o es de volatilidad pyme.

## 7. Algoritmos o métodos que debe conocer
- Shortest Path / Minimum Cost Flow (Para rutear dinero en un grafo CEX-CEX-DEX).
- Confirmaciones estocásticas de bloques (Finalidad Probabilística de PoW/PoS).
- Aritmética de Deducción Continua (Saber que si envías 100 USDT y cobran 5 USDT, llegará 95 USDT al otro lado y preparar la contabilidad para ese valor).

## 8. Fórmulas críticas
- **Costo de Rebalanceo Directo**: `Cost = Withdrawal_Fixed_Fee + Network_Gas_Fee`
- **Costo de Rebalanceo Sintético**: `Cost = (Vol * Slippage_Buy) + (Vol * Slippage_Sell) + Taker_Fees + Cheap_Network_Fee`
- **Fórmula de Disparo**: `if (Skew_Ratio > Max_Tolerated AND Cost_To_Rebalance < Historic_Arb_Profit_Of_That_Venue)`

## 9. Casos extremos
- Exchange Paused Withdrawals (Retiros de Mantenimiento): Un exchange congela los retiros de Solana temporalmente. El rebalanceo lo intenta, obtiene un `Http 400 Withdrawals Suspended`. El bot debe cancelar el proceso, marcar la ruta como Degradada por N horas y no hacer bucles de reintento que gasten cuota API (Skill 35).
- Despeg del Activo de Transporte (Transport Asset Depeg): Se usa XRP para mandar valor barato. Mientras el XRP está en vuelo de 5 minutos, la SEC lanza una demanda y XRP cae un 20%. El rebalanceo "barato" costó $100,000 en pérdidas. El Rebalanceo Sintético debe quedar restringido a stablecoins o L1s colosales protegidas (ETH).
- Missing Deposit (Depósito Atrapado): Binance retira los USDT, la tx es válida on-chain, pero Kraken no los acredita por "Revisión de AML/Compliance". El capital queda en el Limbo. El Rebalanceador, al superar 24 horas en In-Flight, debe escupir un log "ADMIN_INTERVENTION_REQUIRED_FOR_FUNDS".

## 10. Validaciones obligatorias
- PRE: Chequear que la red destino (Network ID) es absolutamente idéntica y soportada entre CEX A y CEX B (Por ejemplo, USDT en Polygon POS (ERC20) hacia USDT en Solana (SPL). El bot NUNCA debe mezclar redes, previniendo pérdida de fondos permanente).
- CÁLCULO: Mantener métrica de "Threshold Cost". No gastes $15 dólares en Gas de Ethereum para rebalancear $20 dólares de inventario sesgado. El mínimo tamaño de rebalanceo debe amortizar los costos al 1%.
- POST: Validar con la Skill 38 que el Ledger reflejó `-100` en cuenta de Origen, `+0` en la red (Dinero Quemado por fee) y `+99` en la cuenta Destino, logrando Suma Cero exacta en el portafolio Global.

## 11. Criterios de aprobación
- API de Retiro (CEX Withdraw) devuelve éxito y TxHash L1.
- Contabilidad local refleja el salto de fondos hacia la canasta de `in_flight`.

## 12. Criterios de rechazo
- La red elegida está congestionada (Gwei por encima de umbrales máximos configurables) y el Skew Ratio aún no es "Crítico", optando por esperar.
- La API Keys carece del Permiso de Retiro (Withdrawal Permission Enabled/IP Restricted) por seguridad manual.

## 13. Riesgos que mitiga
- Muerte por Sequía (Liquidity Drying): El bot gana en cada trade, pero acumula todo el USD en un solo lugar y todo el BTC en otro. Llegado el punto límite, la matemática sigue identificando maravillas, pero el bot no puede operar porque está "Asimétricamente Seco". Esta skill le devuelve el combustible infinito.
- Error Manual de Operador (Fat Finger Transfer): Si un humano mueve el dinero manualmente cruzando puentes, hay probabilidad estadística del 1% de error a lo largo de 10,000 transferencias. Si el bot rebalancea automatizado usando un Whitelist y Network Mappers, el riesgo de equivocación de red o copy-paste es estadísticamente 0%.

## 14. Integración con otras skills
- Reacciona a la señal de Inventario Unificado (Skill 40).
- Descuenta sus costes desde el Tracking de Comisiones (Skill 39) e interacciona on-chain (Skill 21).

## 15. Modelo de datos sugerido
```json
{
  "RebalanceOperation": {
    "job_id": "REBAL-USDT-BIN-KRAK-1044",
    "asset": "USDT",
    "network_transport": "TRC20",
    "source_venue": "binance",
    "dest_venue": "kraken",
    "amount_to_move": 45000.0,
    "projected_cost_usd": 1.0,
    "status": "AWAITING_DESTINATION_DEPOSIT",
    "tx_hash": "0xabc123...",
    "elapsed_time_minutes": 4.5
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Background (Sweeper Process). Corre una evaluación CRON cada 5 minutos sobre la topología del inventario. Invoca endpoints específicos como `/api/v3/withdraw` en CEX.

## 17. Logs obligatorios
- `[INFO] Rebalance Triggered. Skew > 5.0x. Moving 50k USDT from Binance to Bybit via Polygon.`
- `[DEBUG] Rebalance Synthetic Evaluated: Buying XRP on A -> Sending -> Selling XRP on B. Projected cost: $4. Direct withdraw cost: $15. Synthetic selected.`
- `[WARN] Withdrawal API for Solana network is marked MAINTAINANCE on Kraken. Postponing rebalance engine 60 minutes.`

## 18. Métricas obligatorias
- `total_capital_rebalanced_monthly_usd`.
- `average_rebalance_transit_time_minutes`.
- `total_costs_spent_in_rebalancing_usd` (Peaje logístico, debe ser restado al PnL final).

## 19. Tests unitarios
- Cost Optimization Logic: Proporcionar `Direct_Cost = $20`. Proporcionar `Synthetic_Cost = $5`, pero añadir una volatilidad extrema al activo sintético. La heurística DEBE rechazar la ruta sintética si la volatilidad excede la tolerancia al riesgo (Hedge cost).
- Gas Reservation (Gas Buffer): Solicitar enviar "El 100% del Balance" de ETH en Arbitrum (Ej. 1.0 ETH). El bot debe reservar y recortar exactamente 0.05 ETH para futuras comisiones, enviando sólo 0.95 ETH al CEX.
- Re-Entry Attack Prevention: Asegurar que mientras el proceso `withdraw()` esté en promesas asíncronas, el módulo adquiere un Lock de mutex para evitar disparar un segundo retiro de balance fantasma en la misma cuenta un milisegundo después.

## 20. Tests de integración
- Forcar Mainnet. Disparar transferencia nativa USDC desde Wallet EOA hacia Wallet CEX destino, leer logs on-chain para asegurar la detección del Hash en la red correcta.

## 21. Tests E2E
- El bot satura el capital de la Bóveda A (Bybit) dejándola a 0 USD. El inventario lo capta, llama a la Skill 42. La Skill deduce que la Bóveda B (Binance) tiene excedente masivo. Ordena transferir $100k vía BSC BEP20. Llama a la API. El saldo en B cae y entra en "In Flight". 3 minutos después, Bybit recibe el bloque de red BSC, acredita $100k - fee, lanza Websocket `Deposit Success`. Skill 38 lee el websocket, libera In-Flight. Skill 40 lee el nuevo balance, Skill 42 da por terminado el job, y el bot maestro reanuda el trading atómico en Bybit.

## 22. Checklist de producción
- [ ] Seguridad Suprema de Claves: La API Key de Binance que se utiliza para Withdrawals DEBE estar en una Subcuenta separada, o requerir estricto Whitelist de IP y Address CEX. Si la Key de Tradeo normal se filtra, los hackers no podrán drenar vía Withdrawals.
- [ ] Incorporación de un Limite Absoluto Diario (Daily Rebalance Quota). Para prevenir un bucle infinito que queme el capital en "Fees de transferencia", limitar el rebalanceo a máximo 5 veces al día por par.
- [ ] Evitar Transferencias de "Tokens Rebase" o Tokens con Tax a la hora de hacer rebalanceo sintético, lo que restaría 10% del dinero a la basura durante la transferencia on-chain (Skill 30 Honeypot Check).

## 23. Ejemplo de configuración no hardcodeada
```yaml
auto_rebalance_engine:
  enable_automated_withdrawals: false # FALSE by default in production. Enable manually after extreme security checks.
  target_skew_ratio: 1.5
  max_rebalance_cost_pct: 0.05
  allow_synthetic_rebalancing: false
  daily_withdrawal_limits_usd: 500000.0
  address_book:
    binance:
      USDT_ERC20: "0xColdWalletAddressBinance..."
      USDT_TRC20: "TBinanceColdAddress..."
```

## 24. Ejemplo de pseudocódigo
```javascript
async function executeRebalance(asset, sourceVenue, destVenue, amountNeeded) {
    if (!CONFIG.enable_automated_withdrawals) return; // Read-only recommendation mode
    
    // 1. Calculate best network to route the asset
    const bestNetwork = await determineCheapestNetwork(asset, sourceVenue, destVenue);
    if (!bestNetwork) throw new Error("No shared network between venues found");
    
    // 2. Estimate transit fee
    const withdrawFee = await fetchWithdrawFee(sourceVenue, asset, bestNetwork);
    if (withdrawFee > CONFIG.max_acceptable_fee_usd) {
        log.warn("Rebalance fee too high. Postponing.");
        return;
    }

    // 3. Obtain immutable Whitelisted Address for Destination CEX
    const destAddress = CONFIG.address_book[destVenue][`${asset}_${bestNetwork}`];
    
    // 4. Update Internal Ledger Accounting (Lock Funds)
    Ledger.reserveFunds(sourceVenue, asset, amountNeeded, "REBAL_JOB");
    
    try {
        // 5. Fire external CEX API Request
        const txReceipt = await cexApi.withdraw(sourceVenue, asset, bestNetwork, destAddress, amountNeeded);
        
        // 6. Monitor in background queue (Track TxHash)
        InFlightMonitor.add(txReceipt.hash, destVenue, asset, amountNeeded);
    } catch (e) {
        Ledger.releaseFunds(sourceVenue, asset, amountNeeded); // Rollback locally on API failure
    }
}
```

## 25. Criterio final de excelencia
El Rebalanceador transforma el arbitraje cerrado y asfixiante en un sistema de Tuberías Infinitas (Perpetual Motion Machine). Unifica la infraestructura on-chain y los datacenters centralizados en una sola piscina de agua contigua, garantizando que el frente de batalla siempre tenga munición donde la oportunidad asoma.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: APIs de Retiro expuestas (Riesgo de seguridad de la API Key, que debe ser mitigado con IP Whitelisting estricto dictado por el exchange).
- Dependencias: API de Retiros habilitada, Inventario Global Unificado.
- Próxima skill: Profit extraction & cold storage (Skill 43).
