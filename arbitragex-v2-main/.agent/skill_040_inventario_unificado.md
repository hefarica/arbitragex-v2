# SKILL 040 — Inventario multi-exchange unificado

## 1. Propósito superior
Consolidar el ecosistema entero del Arbitraje y Market Making en una única fotografía del mundo (Global State). Convierte las 50 wallets frías, 20 cuentas CEX, Subcuentas y bóvedas en protocolos DeFi en una base de datos atómica, centralizada e hiper-rápida. Su fin es dictaminar a los motores matemáticos (Graph searchers, Arbitrage Evaluators) la capacidad máxima instalada del fondo para mover dinero sin tener que fragmentar consultas, actuando como el Administrador Maestro de Inventario en tiempo real.

## 2. Nivel de conocimiento requerido
Experto en Arquitectura de Estado Distribuido Global, Estructuras de Datos Combinacionales (Global Hashes), Gestión de Memoria Interprocesos (IPC/Shared State), Finanzas Institucionales y Cash Management (Rebalanceo y Manejo de Reservas). Entendimiento profundo de Múltiples Redes (EVM, Solana, CEX DBs) y Riesgo de Exposición de Divisa Cruzada (Cross-Currency Exposure).

## 3. Capacidades principales
1. Centralización de Estado O(1): Compila saldos brutos de Accounting (Skill 38) en un índice de consulta ultra-veloz, organizando la liquidez por `Activo` independientemente del `Lugar` (Exchange). (Ej. El orquestador pregunta "¿Cuánto USDT total operable tenemos globalmente?").
2. Segmentación de Exposición Base: Evaluar instantáneamente la relación de Riesgo Global. Si el sistema debe estar Delta Neutral en USD, verificar que la suma global de todas las posiciones equivalga a dólares (o Delta-0 respecto al criptoactivo subyacente).
3. Motor de Disponibilidad Distribuida (Knapsack Planner): Si la Skill Matemática calcula que hay un arbitraje de 10 BTC disponible, este gestor informa: "Tenemos 5 BTC en Binance y 5 BTC en Kraken. La operación gigante debe partirse y ejecutarse en paralelo usando ambas bóvedas".
4. Alarmas de Desbalance Estructural (Portfolio Skew Alerts): Si a causa de decenas de operaciones atómicas en CEX, Binance quedó sin USDT (pero lleno de BTC) y Kraken se quedó sin BTC (pero lleno de USDT), el Inventario Global activa una bandera de emergencia pidiendo rebalanceo (Skill 42) al cruzar los umbrales operativos (Ej. 90% desviación).
5. Evaluación del Límite de Fuego (Max Firing Limit): Proporcionar al Orquestador (Skill 36) el tamaño máximo absoluto de un trade que las cuentas pueden tolerar hoy, asegurando que no se sobrepase el límite de Margin Cross-Collateral global.
6. Aislamiento de Reservas Frías (Cold Storage Vaults): Mantener registro de capital inyectado pero no operable ("Dry Powder"). Monedas apostadas (Staking) o guardadas en Hardware Wallets que son parte del AUM (Assets Under Management) pero que no deben exponerse al algoritmo HFT.
7. Valoración Consolidada de Posiciones Complejas: Si se tienen posiciones Abiertas en Perpetuos (Ej. Short 10 BTC en Bybit), el inventario cuenta matemáticamente ese colateral restándolo del delta de exposición Spot (+10 BTC de colateral en Binance Spot), validando la neutralidad global (Ver Skill 16).
8. Actualización Concurrente Lock-Free: Actualizar el estado global instantáneamente desde el Event Loop Maestro usando operaciones Atómicas sin bloquear lecturas de otros threads.
9. Contabilidad Multi-Red: Entender que USDC en Arbitrum no es directamente fusionable con USDC en Mainnet para operaciones atómicas inmediatas, salvo que se use Modo Inventario (Skill 19).
10. Detección de Fondos Huérfanos: Activos varados (Dust u oscuros) en CEX que no pertenecen a estrategias activas. Reportarlos para su reciclaje automático.

## 4. Entradas requeridas
- `accounting_ledger`: Estado local reconciliado de cada cuenta individual (Provisto por Skill 38).
- `pricing_oracles`: Precios mark-to-market en USD/EUR para unificación de exposición monetaria.
- `portfolio_rules`: Reglas estrictas de gestión de riesgos para desbalances.

## 5. Salidas esperadas
- `global_inventory_snapshot`: Estructura en RAM de todo el poder de fuego global y sus cuellos de botella por Exchange.
- `skew_warning`: Señal asíncrona ("Binance está seco de USDT, rebalancear").
- `exposure_matrix`: Delta Neutrality Report. Qué tan descubiertos o cubiertos estamos matemáticamente frente a fluctuaciones de mercado.

## 6. Reglas inmutables
- Ninguna Skill Ejecutora (Motor de Arbitraje o Optimizador de Tamaño) puede intentar calcular un Trade sin primero leer sincrónicamente el `Inventario Global` para establecer los límites máximos disponibles en `Exchange A` y `Exchange B`.
- El sistema NO mezcla la valoración teórica de tokens en puente cross-chain en el grupo de "Tokens Disponibles para Arbitraje Atómico". El dinero en vuelo o bloqueado en Vaults es tratado estrictamente como inoperable para HFT.
- Mantener la Segregación de Moneda: WBTC (Wrapped Bitcoin) NO se mezcla con BTC (Bitcoin nativo CEX) en la bolsa de liquidez, a menos que un módulo matemático justifique su conmutabilidad 1:1 en esa transacción específica.

## 7. Algoritmos o métodos que debe conocer
- Vectorización y Agregación In-Memory.
- Portfolio Risk Optimization Models (Markowitz base).
- Identificadores de Co-ubicación de liquidez.

## 8. Fórmulas críticas
- **Global Available Volume (por activo y red)**: `Sum(Ledger[Venue].Free_Balance) WHERE Asset = Target_Asset AND Network = Target_Network`
- **Global Delta (USD Exposure)**: `Sum(Spot_Holdings_USD) - Sum(Futures_Short_Holdings_USD) + Sum(Futures_Long_Holdings_USD)` (Debe ser estricto = 0.00 en Delta Neutral strategies).
- **Skew Ratio (Desbalance de Cuentas)**: `Max(Venue_Asset_Bal) / Min(Venue_Asset_Bal)` (Para detectar qué pata CEX quedó coja tras excesivos arbitrajes unidireccionales).

## 9. Casos extremos
- Unilateral Drain (El Drenaje de una pata): El spread de BTC entre OKX y Binance se mantiene persistente por 5 horas siempre a favor de comprar en OKX y vender en Binance. El Bot agota el 100% de los USDT en OKX y el 100% del BTC en Binance. La oportunidad sigue allí, pero el Inventario Global declara "Inventario Asimétrico, Capacidad 0". Detiene la operativa y solicita un rebalanceo on-chain/API.
- Cuentas Cautivas (Frozen Funds): Un CEX suspende los retiros temporalmente en un momento donde su cuenta acapara el 80% de toda nuestra liquidez de un activo. El bot debe conocer este estado para penalizar el riesgo sistémico de continuar mandando capital a ese exchange (Limitar la exposición).
- Falsos Positivos de Exposición: El oráculo de precios para valuar el inventario sufre un flash-crash temporal, haciendo creer al sistema que el Net Asset Value (NAV) global cayó un 40%. La Skill 40 debe aislar al oráculo de precios y no disparar alertas de pánico que bloqueen cuentas inofensivas.

## 10. Validaciones obligatorias
- PRE: Validar que todos los nodos contables locales (Accounting LEDGER) enviaron sus latidos (heartbeats) sanos antes de consolidar el Inventario Global. (Si un CEX está desconectado, no sumar su viejo caché asumiéndolo vivo).
- CÁLCULO: Mantener un Index invertido para velocidad O(1). No es `Exchange => Activos`, sino `Activo => { ExchangeA: Vol, ExchangeB: Vol }`. Cuando el Arbitrajista busca "USDT", no itera 20 exchanges, accede al map global.
- POST: Realizar una comprobación de sanidad de suma 0: El Balance_Libre + Balance_Bloqueado + Balance_En_Vuelo = Balance_Total. Sin excepción.

## 11. Criterios de aprobación
- Los subsistemas matemáticos pueden solicitar la "Profundidad máxima disponible en CEX Y" en menos de 0.05ms (Caché puro).
- El sistema reporta el Delta Exposure global continuamente para los módulos de Gestión de Riesgo Institucional.

## 12. Criterios de rechazo
- El Módulo intenta mezclar tokens envueltos (`USDC.e` de Arbitrum con `USDC` Nativo de Arbitrum) asumiendo que son lo mismo operativamente on-chain, destrozando la ejecución de los smart contracts del motor. (Tienen address distinta, se indexan distinto).
- El cálculo de Net Asset Value muestra una caída violenta inexplicable sin ejecución de trades perdedores (Generalmente un problema de Oráculo o Bug contable fatal).

## 13. Riesgos que mitiga
- Fragmentación Estéril de Capital (Dead Liquidity): Evita que el fondo asuma que no puede hacer un trade de $1M, cuando en realidad tiene $1M repartido en 4 cuentas de $250k. Informa a los ejecutadores que armen transacciones batchizadas para movilizar la liquidez fragmentada.
- Pérdida de Neutralidad Direccional: Si el bot gana $10 en arbitraje pero accidentalmente rompió su cobertura y compró $10,000 en BTC sin vender futuros, y BTC baja un 10%, el bot pierde $1,000. El Inventario Global escupe alertas rojas si el "Global Delta" no es cero absoluto en milisegundos para forzar coberturas.

## 14. Integración con otras skills
- Única fuente de verdad validada para el Optimizador de Tamaño (Skill 2) y Motor de Riesgo Global (Skill 41).
- Detonante directo del Rebalanceador Automático (Skill 42).
- Consolidador absoluto del Accounting Node local (Skill 38).

## 15. Modelo de datos sugerido
```json
{
  "GlobalInventoryState": {
    "total_aum_usd": 1504200.50,
    "delta_exposure_usd": 0.00,
    "asset_index": {
      "USDT": {
        "global_free": 500000.0,
        "venues": {
          "binance": 200000.0,
          "okx": 50000.0,
          "arbitrum_wallet_cold": 250000.0
        },
        "skew_ratio": 4.0,
        "health": "SKEWED_NEEDS_REBALANCE"
      },
      "BTC": {
        "global_free": 12.5,
        "venues": { "binance": 8.0, "kraken": 4.5 },
        "skew_ratio": 1.77,
        "health": "HEALTHY"
      }
    }
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase Global inyectada como dependencia a todos los módulos HFT. Con un Bus de Eventos que dispara alertas a los Módulos de Gestión de Riesgo (Risk Engine) cada vez que se superan umbrales de seguridad críticos.

## 17. Logs obligatorios
- `[DEBUG] Global Inventory Synchronized. Total AUM: $1.50M. Delta: Neutral.`
- `[WARN] Liquidity Skew Alert: USDT heavily concentrated on Binance (85%). Firing signal to Auto-Rebalancer Module.`
- `[CRITICAL] DELTA NEUTRALITY BREACHED! Exposure shifted to +$15,000 Long BTC. Suspending trading and executing emergency hedge.`

## 18. Métricas obligatorias
- `total_delta_exposure_usd_realtime`.
- `liquidity_skew_index_by_asset`.
- `global_capital_efficiency_pct` (Qué % del fondo está moviéndose vs qué % está sentado rindiendo nada).

## 19. Tests unitarios
- Index Inversion: Tomar un Ledger con 50 Cuentas x 100 Activos (5000 puntos de datos). La skill debe reconstruir el mapa unificado (Asset -> Venue) en nanosegundos (Indexación óptima pre-generada, no calculada on-the-fly).
- Delta Evaluation: Inyectar datos donde el fondo compró 1 BTC Spot y cortó en Futuros 0.5 BTC. El Validador de Delta DEBE saltar detectando $+0.5 BTC de exposición direccional en riesgo, alertando al orquestador.
- Filter Inactive Exchanges: Simular que Kraken está "Offline/Rate Limited". La Skill 40 DEBE eliminar sus balances virtuales del `global_free` total para que las matemáticas de arbitraje no cuenten con dinero inalcanzable.

## 20. Tests de integración
- Sincronizar el Inventario con un Mock Accounting y Oráculos vivos de Coingecko/Binance. Simular caída violenta del precio del activo B (-20%). Verificar que el NAV (USD) de la bóveda de Delta Neutral se mantiene estable y no fluctúa (Confirmación estructural de la viabilidad HFT institucional).

## 21. Tests E2E
- El agente funciona en producción de Testnet. Mueve 1,000 operaciones en 5 exchanges durante 24 horas. Los fondos fluyen como agua entre bolsas CEX y DEX en 5 cadenas on-chain diferentes. El Dashboard central monitorea que, a pesar de la locura inter-cambiaria, el Net Asset Value subió suavemente por las micro-ganancias, y el Skew de liquidez se autorreguló sin intervención humana.

## 22. Checklist de producción
- [ ] Incorporación de Lógica de Divisa Base Fuerte. Todas las valuaciones MTM (Mark-to-Market) deben estandarizarse estrictamente contra el Oráculo de USD (o EUR) con feeds Redundantes (Chainlink + Binance API).
- [ ] Diseño de Seguridad Segregada (Ring Fencing). Si el Exchange A colapsa por insolvencia estilo FTX, el inventario Global asume esos fondos como Perdidos y ajusta el AUM al instante (Castigo a Cero o Pelo) sin corromper el funcionamiento de los Arbitrajes en el Exchange B.
- [ ] Optimizar recálculo de Skew: Sólo recalcular la asimetría si el movimiento reciente impactó >5% del patrimonio de ese activo para no saturar CPU (Lazy Skew Calculation).

## 23. Ejemplo de configuración no hardcodeada
```yaml
global_inventory_engine:
  base_fiat_currency: "USD"
  delta_neutrality_tolerance_usd: 50.0  # Drift allowed before emergency hedge
  max_skew_ratio_trigger: 4.0           # E.g. Venue A has 4x more than Venue B
  mark_to_market_interval_ms: 1000
```

## 24. Ejemplo de pseudocódigo
```javascript
class GlobalInventory {
    constructor() {
        this.assetsIndex = new Map(); // asset -> { global_free, venues: Map }
        this.totalAUM = 0;
        this.deltaExposureUsd = 0;
    }

    rebuildIndexFromLedger(ledgerState) {
        let newAssetsIndex = new Map();
        
        for (let [venue, account] of ledgerState) {
            if (isVenueDegradedOrOffline(venue)) continue; // Don't count frozen money
            
            for (let [asset, balInfo] of account.assets) {
                let assetObj = newAssetsIndex.get(asset) || { global_free: 0, venues: new Map() };
                
                assetObj.global_free += balInfo.free;
                assetObj.venues.set(venue, balInfo.free);
                
                newAssetsIndex.set(asset, assetObj);
            }
        }
        
        this.assetsIndex = newAssetsIndex;
        this.evaluateSkewAndHealth();
    }
    
    evaluateDeltaNeutrality(oraclePrices, futuresPositions) {
        let netDelta = 0;
        // Sum Spot exposure
        for (let [asset, data] of this.assetsIndex) {
            if (!isStablecoin(asset)) {
                 netDelta += data.global_free * oraclePrices.get(asset);
            }
        }
        // Subtract Short Futures Exposure
        for (let pos of futuresPositions) {
            netDelta -= pos.shortNotionalUsd;
        }
        
        this.deltaExposureUsd = netDelta;
        
        if (Math.abs(netDelta) > CONFIG.delta_neutrality_tolerance) {
             EventBus.emit('CRITICAL_DELTA_BREACH', netDelta);
        }
    }
    
    // Fast path for math skills O(1)
    getAvailableLiquidity(venue, asset) {
        const assetMap = this.assetsIndex.get(asset);
        if (!assetMap) return 0;
        return assetMap.venues.get(venue) || 0;
    }
}
```

## 25. Criterio final de excelencia
El Inventario Global Unificado transforma la red descentralizada de capitales dispersos en un super-organismo unificado. Dictamina la potencia total del bot en tiempo real, garantizando la seguridad de "Delta 0" mientras moviliza ejércitos de capital en la dirección óptima sin cometer errores logísticos ni estrangularse en ineficiencias de enrutamiento monetario.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Flash Crashes en Oráculos de precios base (Stablecoin depegs o Oráculos de valoración rotos que disparan alertas falsas).
- Dependencias: Accounting Ledger, Price Oracles.
- Próxima skill: Risk engine global (Skill 41).
