# SKILL 014 — Arbitraje triangular

## 1. Propósito superior
Detectar y ejecutar ineficiencias de precio dentro de un único exchange (CEX o DEX) triangulando 3 activos (ej. BTC, ETH, USDT) donde la conversión cruzada `USDT -> BTC -> ETH -> USDT` resulta en una cantidad final de USDT mayor a la inicial. Elimina el riesgo de lentitud entre exchanges (Leg Risk estructural) basándose enteramente en la liquidez y microestructura local.

## 2. Nivel de conocimiento requerido
Experto en Álgebra de Tipos de Cambio (Cross-Rates), microestructura de Matching Engines unificados, gestión atómica o ultra-rápida de ruteo de órdenes multi-par, y validación milimétrica de Fees de Taker. Nivel Máster en algoritmia de detección concurrente.

## 3. Capacidades principales
1. Generación y mantenimiento de todas las tripletas posibles (Triangles) operables a partir de los pares base (USDT, USDC, BTC, ETH) del exchange.
2. Cálculo de la tasa cruzada teórica vs la tasa real del libro.
3. Validación estricta del sentido de la orden (Buy/Sell o Bid/Ask cruzados) para las 3 operaciones consecutivas.
4. Estimación de volumen cuello de botella (Bottleneck Volume): El trade se topa con el volumen de la pata más ilíquida del triángulo.
5. Ejecución secuencial ultra-veloz en CEX o ejecución atómica en DEX.
6. Aplicación de Fee compuesto: `Profit = (V1 * R1 * (1-f1) * R2 * (1-f2) * R3 * (1-f3)) - V1`.
7. Ajuste de Step Size / Min Notional combinado (el redondeo en la pata 1 afecta la cantidad ingresada a la pata 2).
8. Mecanismo de cobertura (Hedge de emergencia) en caso de ejecución parcial de la pata 2 (Dejar fondos trabados en altcoin).
9. Mapeo de volatilidad intra-triángulo (para calcular el riesgo temporal de los 10ms que tarda el CEX en completar las 3 órdenes).
10. Filtro de activos suspendidos (Delisted/Maintenance) que anulan un vértice del triángulo.

## 4. Entradas requeridas
- `exchange_tickers`: Flujo L1/L2 consolidado del exchange local.
- `fee_tiers`: Nivel de comisiones actuales del usuario (Maker/Taker) específico para los pares (Algunos pares tienen Zero-Fee campaigns).
- `inventory_balance`: Fondos disponibles en la moneda base del triángulo.

## 5. Salidas esperadas
- `triangle_opportunity`: Tripleta identificada y validada `[Par1(Side), Par2(Side), Par3(Side)]`.
- `expected_profit`: Beneficio neto calculado tras deducción de comisiones.
- `execution_status`: Recibo de los 3 fills.
- `stranded_assets`: Inventario residual si alguna pata no se llenó.

## 6. Reglas inmutables
- Siempre descontar el Fee 3 veces en el cálculo matemático, porque el capital cruza 3 pares distintos.
- En CEX, las órdenes deben enviarse de forma secuencial síncrona si depende del saldo resultante (Cash mode) o simultánea si hay balance preexistente para cubrir todas las patas (Inventory Mode).
- La cantidad a ejecutar NUNCA debe superar la profundidad visible del nivel 1 (Best Bid/Ask) de la pata más ilíquida.
- El tiempo total de vida (TTL) del arbitraje triangular en evaluación debe ser inferior a 2 milisegundos (Las ineficiencias intradiarias se corrigen por Market Makers rapidísimo).

## 7. Algoritmos o métodos que debe conocer
- Detección de ciclos algebraicos limitados a profundidad 3 (Variante optimizada de Skill 3/4).
- Precisión numérica estricta para manejo de redondeos en cascada (Rounding errors mitigation).
- Algoritmia de Ordenamiento para identificar la "Pata Cuello de Botella" al instante.
- Retrying & Rollback logic para operaciones secuenciales fallidas en CEX.

## 8. Fórmulas críticas
- **Cálculo Directo (Side=Buy, Buy, Sell)**: `Salida = ( (Entrada/Ask1) * (1-Fee1) / Ask2 ) * (1-Fee2) * Bid3 * (1-Fee3)` (Nota: La fórmula varía dependiendo si el par base o quote está invertido).
- **Condición Arbitraje**: `Salida > Entrada * (1 + Min_ROI_Limit)`
- **Cuello de Botella**: `Max_Volume_Entrada = Min(Vol1_in, Vol2_in, Vol3_in)` adaptado a la unidad base de inicio.

## 9. Casos extremos
- Un "Triángulo de Polvo" (Dust Triangle): La pata 1 se ejecuta, pero por redondeos el saldo resultante para la pata 2 es 0.00000001 inferior al `Min_Notional` del exchange, cancelando el arbitraje y dejando polvo de altcoin en la cuenta.
- "Zero-Fee Campaigns": El exchange anuncia comisiones 0% en BTC/USDT, alterando masivamente la viabilidad del triángulo.
- Falsa liquidez en un par obsoleto (Ej. BUSD) que no permite salir.

## 10. Validaciones obligatorias
- PRE: Asegurar que el cálculo de Step Size sea perfecto; si requiere $10.123 para el paso 2 pero el exchange recibe $10.12, los $0.003 restantes se pierden en eficiencia.
- CÁLCULO: Mantener un modelo de bid/ask estricto. (Comprar A con B implica mirar el Ask de A/B o el Bid de B/A).
- POST: Verificación cruzada de inventario (Audit check) post-triángulo para certificar que ningún token basura quedó colgado.

## 11. Criterios de aprobación
- Tripleta validada donde la ineficiencia matemática cubre cómodamente los 3 "Taker Fees".
- El tamaño (size) supera el Min_Notional estricto de las 3 patas.

## 12. Criterios de rechazo
- El triángulo contiene un activo ilíquido (Spread > 1%) que evapora la ganancia en su paso.
- El CEX no ofrece soporte "IOC" (Immediate or Cancel) para uno de los pares.

## 13. Riesgos que mitiga
- Riesgo de Red: Al hacerse 100% dentro de una base de datos de un mismo exchange, no hay tiempos de transferencia, congestiones de mempool, ni errores de RPC (a menos que sea DEX triangular).
- Riesgo de contraparte: Las comisiones son predecibles y fijas, no como el "Gas" dinámico.

## 14. Integración con otras skills
- Reutiliza la Detección de Ciclos (Skill 4).
- Está sujeto a Probabilidad Bayesiana (Skill 9) para evitar pares altamente manipulados (Fake spread).
- Nutre el Dashboard HFT (Skill 81).

## 15. Modelo de datos sugerido
```json
{
  "TriangularExecution": {
    "exchange": "binance",
    "base_asset": "USDT",
    "route": ["BTC-USDT", "ETH-BTC", "ETH-USDT"],
    "sides": ["BUY", "BUY", "SELL"],
    "bottleneck_volume_usd": 1500,
    "executed_volume_usd": 500,
    "net_roi_usd": 1.25,
    "status": "COMPLETED"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Sub-proceso altamente optimizado dedicado exclusivamente a monitorizar un sub-conjunto de 20-30 pares de altísima liquidez usando WebSockets locales paralelos, buscando desviaciones en milisegundos.

## 17. Logs obligatorios
- `[DEBUG] Evaluating triangle: USDT -> BTC -> ETH -> USDT. Spread is -0.1% (Loss).`
- `[INFO] Executed Triangular Arb on OKX. Path: USDT->XRP->ETH->USDT. Profit: +$1.25 (0.25% Net).`
- `[WARN] Triangular Arb sequence failed at Leg 2 (Insufficient Balance error due to truncation). Hanging asset: XRP.`

## 18. Métricas obligatorias
- `triangular_opportunities_detected_per_minute`
- `triangular_execution_latency_ms` (Tiempo entre lanzar Orden 1 y recibir confirmación de Orden 3).
- `hanging_dust_value_usd` (Monitoreo de la eficiencia del truncamiento matemático).

## 19. Tests unitarios
- Mockear precios de 3 pares para forzar una oportunidad. Validar que la fórmula direccional de Buy/Sell detecta la rentabilidad real descontando fees.
- Test de truncamiento: Simular el flujo de saldo asegurando que la Pata 2 y 3 nunca intenten vender 0.000001 de más.
- Test de Bid/Ask invertido: Comprar A/B vs Vender B/A deben ser testeados para no errar el lado del order book.

## 20. Tests de integración
- Sincronizar con un cliente WebSocket simulado de Binance, inyectar el retraso de 10ms entre operaciones y evaluar el comportamiento.

## 21. Tests E2E
- El agente lee datos de la Testnet, identifica una discrepancia en BNB/BTC/USDT, ejecuta la cadena IOC exitosamente y reconcilia el profit en la tabla maestra.

## 22. Checklist de producción
- [ ] Módulo matemático preparado para "Inverse Contracts" o "Stable-Stable" loops (ej. USDT -> USDC -> DAI -> USDT).
- [ ] Optimización de ruteo: Si el exchange soporta "Batch Orders", enviar las 3 órdenes en el mismo JSON payload (ej. Binance Spot API `POST /api/v3/order/cancelReplace`).
- [ ] Filtro implacable contra delisted tokens o suspensiones temporales.

## 23. Ejemplo de configuración no hardcodeada
```yaml
triangular_arbitrage:
  active_exchanges: ["binance", "okx"]
  max_sequence_timeout_ms: 100
  allowed_base_assets: ["USDT", "USDC", "BTC"]
  require_ioc_support: true
```

## 24. Ejemplo de pseudocódigo
```python
def check_triangular_opportunity(p1, p2, p3, fee_rate, initial_capital):
    # Assuming path: USDT -> AssetA -> AssetB -> USDT
    # 1. Buy AssetA with USDT
    assetA_acquired = (initial_capital / p1.ask) * (1 - fee_rate)
    
    # 2. Buy AssetB with AssetA (Assuming p2 is AssetB/AssetA)
    # Be careful with directional price logic based on exact pairs
    assetB_acquired = (assetA_acquired / p2.ask) * (1 - fee_rate)
    
    # 3. Sell AssetB for USDT (Assuming p3 is AssetB/USDT)
    final_usdt = (assetB_acquired * p3.bid) * (1 - fee_rate)
    
    roi = (final_usdt - initial_capital) / initial_capital
    
    if roi > MIN_ROI:
        return True, roi
    return False, roi
```

## 25. Criterio final de excelencia
El motor triangular ejecuta el ciclo completo en CEX en menos de 100 milisegundos de pared a pared, garantizando precisión de BigInt y 0% de polvo varado ("Dust") en la cuenta al terminar cientos de ciclos diarios.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Latencia en la confirmación de CEX que impida emitir la pata 2 a tiempo.
- Dependencias: Gestión de Inventario, Order Routing.
- Próxima skill: Arbitraje de stablecoins (Skill 15).
