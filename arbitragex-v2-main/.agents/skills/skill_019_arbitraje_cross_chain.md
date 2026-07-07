# SKILL 019 — Arbitraje cross-chain controlado

## 1. Propósito superior
Detectar y ejecutar ineficiencias de precio entre activos idénticos o envueltos que residen en distintas blockchains (Ej. ETH en Ethereum vs WETH en Arbitrum vs ETH en Polygon). Gestiona el inmenso riesgo de la asincronía y el tiempo de confirmación de los "Bridges" (Puentes intercadena), evaluando si el spread sobrevive al tiempo de viaje o si debe ejecutarse usando inventario pre-balanceado (Inventory Cross-Chain Arbitrage).

## 2. Nivel de conocimiento requerido
Experto en Arquitectura Blockchain L1/L2, Seguridad de Bridges (Lock/Mint, Burn/Release, Liquidity Networks), Criptografía cross-domain y Finalidad de Bloques. Dominio profundo del cálculo estocástico de latencia para estimar la probabilidad de supervivencia del gap durante los minutos/segundos de ruteo.

## 3. Capacidades principales
1. Identificación de diferenciales de precio (Spread) de un mismo activo base en diferentes ecosistemas (DeFi L1 vs DeFi L2).
2. Cálculo unificado de "Costos Cross-Chain": Gas en cadena origen + Fees de Bridge + Gas en cadena destino.
3. Modo "Inventory Arb": Ejecución instantánea vendiendo/comprando en ambas cadenas simultáneamente asumiendo que el bot ya tiene inventario en ambas (Zero Bridge Delay).
4. Modo "Bridge Arb": Ejecución ruteada usando puentes de liquidez instantánea (ej. Stargate, Across Protocol, Hop) o puentes nativos.
5. Inferencia probabilística del "Slippage de Tiempo" (Qué tanto se moverá el precio en la cadena B mientras los fondos cruzan desde la cadena A).
6. Monitoreo de "Reorgs" y "Finality": Diferenciar entre "Soft Finality" (Rollups) y "Hard Finality" (Ethereum L1).
7. Manejo de Smart Contracts especializados que automatizan el trade de origen, ejecutan la llamada cross-chain (LayerZero/CCIP) y triggerean el trade en destino.
8. Estimación de liquidez de salida en la cadena destino del bridge (Si envías 100 ETH por el puente, el puente debe tener 100 ETH en el smart contract de destino).
9. Mapeo de tokens homólogos yWrapped-Tokens (`USDC.e` vs `USDC` nativo en Arbitrum).
10. Detección y prevención de hackeos de bridges en tiempo real (Si el TVL de un puente se desploma en un bloque, paralizar ruteos).

## 4. Entradas requeridas
- `cross_chain_oracles`: Feeds de precios sincronizados de las cadenas objetivo.
- `bridge_liquidity`: Liquidez actual en los contratos del puente seleccionado.
- `bridge_fees_and_gas`: Costo de red de la cadena A, costo del puente, y gas de la cadena B.
- `latency_model`: Distribución de probabilidad del tiempo de confirmación del puente (Ej. Stargate tarda ~45 segundos de L1 a L2).
- `inventory_balances`: Fondos disponibles del bot en todas las cadenas monitoreadas.

## 5. Salidas esperadas
- `cross_chain_opportunity`: Detalles del arbitraje (Red A -> Red B, Profit).
- `execution_mode`: `INVENTORY_SYNC` (Instantáneo) o `BRIDGE_ROUTE` (Asíncrono).
- `projected_gas_total_usd`: Suma de todos los costos de infraestructura intercadena.
- `bridge_survival_probability`: Probabilidad % de que la ineficiencia siga existiendo al llegar los fondos.

## 6. Reglas inmutables
- Nunca rutar fondos a través de un Bridge sin verificar programáticamente primero la liquidez del contrato en la cadena destino. (Evitar fondos trabados por días).
- En el Modo `BRIDGE_ROUTE`, si el tiempo esperado de cruce es mayor a 3 minutos, la operación es RECHAZADA, sin importar el margen de ganancia (El mercado de crypto es demasiado volátil).
- La prioridad máxima SIEMPRE es el Modo `INVENTORY_SYNC` (Ejecutar pata A y pata B a la vez usando saldos preexistentes). El bridge se usa solo para rebalancear el portafolio a posteriori con calma.
- Todo spread aparente derivado de activos "Pegged" no oficiales (Ej. madUSDC) debe someterse a análisis de de-peg estructural (Skill 15).

## 7. Algoritmos o métodos que debe conocer
- Interoperability Messaging Protocols (Cross-Chain Interoperability Protocol - CCIP, LayerZero).
- Algoritmos de "Rebalancing Routing" (Problema del transporte/Knapsack problem modificado).
- Monitoreo concurrente de eventos Web3 en múltiples nodos RPC.

## 8. Fórmulas críticas
- **Costo Total Cross-Chain**: `Gas_Origen_USD + Protocol_Fee_USD + Dest_Gas_Oracle_USD`
- **Condición de Inventario (Instantáneo)**: `Balance_A >= Size` AND `Balance_B >= Size` (Permite hedge atómico).
- **Time Slippage Discount (Para Bridge Mode)**: `Profit_Proyectado_USD - (Volatilidad_BPS_por_segundo * Bridge_Delay_Seconds * Capital)`

## 9. Casos extremos
- Hackeo del Bridge en pleno cruce (Fondos quemados en A, imposibles de redimir en B). Pérdida del 100%.
- Latencia del Secuenciador (L2 Sequencer down): La red destino (Arbitrum) se cae, los fondos llegan al smart contract destino pero el DEX de destino está paralizado.
- Liquidez fragmentada: Un bot ve spread en `USDC -> USDT` en Polygon, y cruza desde Ethereum con `USDC.e` (Bridged). Llega y se da cuenta que el DEX requería USDC nativo, destruyendo la ruta.

## 10. Validaciones obligatorias
- PRE: Validar rigurosamente el contrato del Token Address en la cadena B.
- CÁLCULO: Incorporar un "Buffer de Finalidad" al tiempo. Si Polygon requiere 128 bloques para finalidad, el tiempo de cruce no es 2 segundos.
- POST: Si se usa el Modo Bridge, instanciar un Worker asíncrono que escuche eventos en la Blockchain B durante horas si es necesario, alertando al orquestador cuando los fondos aterricen.

## 11. Criterios de aprobación
- Modo Inventario: Los fondos locales son suficientes y el ROI Neto Post-Gas > Límite.
- Modo Bridge: El Spread neto supera holgadamente el castigo estocástico de volatilidad temporal (`Time Slippage Discount`).

## 12. Criterios de rechazo
- El Gas en Ethereum L1 es de > 100 GWEI, destruyendo cualquier margen operable en el paso por el puente.
- El puente seleccionado tiene alertas de seguridad recientes o su liquidez de salida es menor al `Size * 1.5`.

## 13. Riesgos que mitiga
- Riesgo de Asincronía Absoluta: Convertir una oportunidad matemática segura en una especulación direccional ciega por culpa de un puente lento de 20 minutos.
- Riesgo de "Fake Arbitrage" intercadena: Distinguir tokens envueltos no líquidos de tokens nativos hiper-líquidos.

## 14. Integración con otras skills
- Apoyado fuertemente por Optimización Estocástica (Skill 6) para el cálculo de decaimiento del spread temporal.
- Requiere Arbitraje DEX-DEX (Skill 13) como motor de ejecución en origen y destino.

## 15. Modelo de datos sugerido
```json
{
  "CrossChainArbitrage": {
    "asset": "USDC",
    "source_chain": "ethereum",
    "dest_chain": "arbitrum",
    "spread_bps": 250,
    "execution_mode": "INVENTORY_SYNC",
    "net_profit_usd": 45.50,
    "bridge_used_for_rebalance": "across_protocol",
    "status": "APPROVED"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Un Multi-Chain RPC Manager que mantenga suscripciones Websocket paralelas a los nodos completos (Full Nodes) de L1 y L2 simultáneamente.

## 17. Logs obligatorios
- `[INFO] Cross-chain arb detected: ETH -> Optimism. Executing INVENTORY_SYNC to bypass 2-minute bridge delay. Profit locked.`
- `[WARN] Spread found ETH -> Polygon, but Bridge Mode is required. Bridge latency (15 mins) exceeds safety threshold. Arb rejected.`
- `[CRITICAL] Dest_chain liquidity dried up on Across Protocol. Re-routing rebalance via Stargate.`

## 18. Métricas obligatorias
- `inventory_sync_success_rate`.
- `bridge_latency_prediction_error`.
- `cross_chain_gas_expenditure`.

## 19. Tests unitarios
- Conversión de Address Multi-cadena: Validar que el token USDC de Ethereum se mapea algorítmicamente contra el address correcto de USDC nativo en Arbitrum, ignorando los clones falsos o bridged deprecados.
- Selector de modo: Si `balance_A >= 100` y `balance_B >= 100`, debe forzar `INVENTORY_SYNC`. Si `balance_B == 0`, debe evaluar `BRIDGE_ROUTE`.

## 20. Tests de integración
- Conectar con la API de LayerZero o Stargate para hacer poll real de las reservas de la cadena destino antes de firmar cualquier transacción simulada.

## 21. Tests E2E
- Simular un gap de precio entre un Fork L1 y un Fork L2. El bot debe abrir transacción atómica en L1, y en L2, usando inventarios locales. Luego, disparar asincrónicamente el rebalanceo de fondos mediante puentes por detrás para restaurar el estado inicial.

## 22. Checklist de producción
- [ ] Incorporación de un catálogo determinista de "Contratos Seguros" (Token Whitelist multichain) para evitar ataques de tokens falsos con el mismo Ticker.
- [ ] Monitoreo del "Gas Oracle" en la cadena de destino para evitar mandar fondos a una L2 que justo en ese momento entró en pico de congestión extrema.
- [ ] Auditoría local automatizada: Comprobar firmas RPC TLS.

## 23. Ejemplo de configuración no hardcodeada
```yaml
cross_chain_engine:
  preferred_execution: "INVENTORY_SYNC"
  bridge_max_wait_seconds: 120
  allowed_chains: ["ethereum", "arbitrum", "optimism", "polygon", "base"]
  bridge_liquidity_safety_multiplier: 2.0
```

## 24. Ejemplo de pseudocódigo
```javascript
async function evaluateCrossChainArb(chainA, chainB, amount) {
    const profit = calculateSpread(chainA, chainB);
    
    // Check Inventory Mode first
    if (inventory[chainA].has(amount) && inventory[chainB].has(amount)) {
        const gasCost = getGas(chainA) + getGas(chainB);
        if (profit > gasCost * 1.5) {
            return { mode: "INVENTORY_SYNC", executable: true, expected_profit: profit - gasCost };
        }
    }
    
    // Fallback to Bridge Mode
    const bridgeInfo = await checkBestBridge(chainA, chainB);
    if (bridgeInfo.estimated_time_s > CONFIG.max_bridge_time) {
         log.warn("Bridge too slow for HFT. Opportunity decayed.");
         return { executable: false };
    }
    
    if (bridgeInfo.dest_liquidity < amount * 1.5) {
         log.warn("Insufficient liquidity on destination bridge contract.");
         return { executable: false };
    }
    
    const totalCost = getGas(chainA) + bridgeInfo.fee + getGas(chainB);
    const timeDiscountedProfit = applyTimeDecay(profit, bridgeInfo.estimated_time_s);
    
    if (timeDiscountedProfit > totalCost) {
         return { mode: "BRIDGE_ROUTE", executable: true, expected_profit: timeDiscountedProfit - totalCost, bridge: bridgeInfo };
    }
    
    return { executable: false };
}
```

## 25. Criterio final de excelencia
El motor cross-chain abstrae completamente las fronteras entre blockchains, operando como un ente ubicuo que explota gaps usando inventario propio a velocidades sub-segundo, relegando a los Bridges de terceros a ser meros re-balanceadores lentos y asíncronos que corren en background fuera de la línea de riesgo de la operación.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Bridge Hacks (Evitado operando principalmente en Inventory Sync mode y usando puentes solo para rebalanceo pasivo bajo límites de exposición baja).
- Dependencias: Gestión Multichain RPC, Inventario global unificado.
- Próxima skill: Arbitraje entre agregadores (Skill 20).
