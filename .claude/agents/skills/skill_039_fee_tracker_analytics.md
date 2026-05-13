# SKILL 039 — Fee tracker & maker/taker analytics

## 1. Propósito superior
Rastrear, proyectar y auditar dinámicamente el inmenso y laberíntico esquema de comisiones y descuentos de los exchanges centralizados (CEX VIP Tiers, BNB/KCS deductions, Maker Rebates) y protocolos DeFi (Swap fees, Protocol fees, Flash loan fees). El objetivo primordial de esta skill es garantizar que la Rentabilidad Matemática proyectada (Skill 1) jamás asuma un costo erróneo, evitando ejecutar arbitrajes donde el spread bruto parece enorme pero el costo final destructivo (Fee Trap) vuelve al trade netamente negativo.

## 2. Nivel de conocimiento requerido
Analista Financiero Cuantitativo, Microestructura de Cobros (Fee Structures), Manejo Avanzado de API Account Status y Descuentos Cruzados por Nivel VIP. Dominio de cobro por token base (Pagar en BNB/KCS vs pagar en el Token transado), cálculo iterativo de descuentos (Referral Kickbacks, Rebates en Maker Orders que pagan al usuario en vez de cobrarle), y contabilidad de "Negative Fees".

## 3. Capacidades principales
1. Ingesta Dinámica de Fee Tiers: Mantener actualizado el "Taker Fee" y "Maker Fee" real de la cuenta. Un usuario VIP-3 en Binance Spot paga ~0.04% y no el 0.1% por defecto, abriendo ineficiencias exclusivas no accesibles para el público masivo.
2. Descuento de Moneda de Intercambio (Platform Tokens): Ajustar el Fee real si la cuenta tiene BNB activado para pago de comisiones (-25% de descuento en Binance Spot, -10% en Futuros), leyendo asíncronamente si el balance de BNB es suficiente para cubrir el trade inminente.
3. Tratamiento Maker/Taker: El bot es 99% Taker en arbitrajes atómicos cruzados, pero para estrategias como Arbitraje Estadístico o Market Making (Skill 55), la ejecución pasiva gana "Rebates" (Cobrar por proveer liquidez). El tracker modela matemáticamente ese reembolso como ganancia neta.
4. Ajuste por Promociones de Costo Cero ("Zero-Fee Pairs"): Identificar si pares como `USDC/USDT` o `BTC/USDT` están bajo promociones globales de 0% Maker/Taker, forzando a la matemática a aprovechar desviaciones de micro-centavos invisibles bajo fees normales.
5. Inclusión de Funding Fees Realizados: Auditar si la ganancia del Funding Rate (Skill 16) fue cobrada exactamente a la cuenta según lo proyectado por el Premium Index.
6. Gas Tracker Nativo y Cross-Chain: Convertir Gas (Ej. 30 gwei, 50,000 gas limit) a la denominación en moneda cotizada (Ej. USD u ETH) para integrarlo sinérgicamente en el Spread de costo.
7. Cálculo de Withdraw/Deposit Fees (Puertas de Entrada/Salida): En arbitraje espacial masivo (rebalanceo entre CEX), el costo de red por extraer $1M USDT por la red Tron (TRC20) es ~$1.00 USD, lo que debe restarse al ROI Global de Arbitraje de Intercambio (Skill 12+19).
8. Detección Temprana de Asfixia por Tarifa (Fee Starvation): Si el bot usa "BNB" para pagar comisiones, pero el saldo de BNB en Binance cae a $0.05, el tracker dispara una "Alerta Crítica", forzando al orquestador a recomprar BNB automáticamente, o de lo contrario el Fee sube repentinamente al 0.1% arruinando matemáticamente la rentabilidad.
9. Contabilidad de Cuentas Maestras y Subcuentas. (Fees unificados bajo programas institucionales).
10. Rastreador de Deslizamiento Positivo en DeFi: Descontar el porcentaje de slippage si el AMM (Uniswap) generó deslizamiento positivo debido a un front-running a favor de nuestro trade por arbitraje MEV accidental.

## 4. Entradas requeridas
- `account_info`: Endpoint de datos del usuario, balances de monedas VIP (BNB, OKB, BGB).
- `exchange_info`: Metadata de los pares indicando reglas promocionales de Fee.
- `network_gas_oracle`: (Skill 21) Módulo de lectura on-chain con el estado del Gas nativo.
- `executed_trades_logs`: (De la Skill 38 - Accounting), donde figuran los montos exactos deducidos como comisión post-trade.

## 5. Salidas esperadas
- `dynamic_fee_matrix`: Estructura en RAM consultada globalmente en tiempo O(1) con el Fee real instantáneo para (Exchange, Pair, Side).
- `true_pnl_tracker`: Resumen real neto post-todo del fondo.
- `fee_balance_alarms`: Notificación de que las monedas utilitarias están bajas (Ej. "Buy more BNB for fees").

## 6. Reglas inmutables
- JAMÁS configurar los "Fees de Taker/Maker" harcodeados en un config de YAML si se es VIP o se usa moneda de plataforma (Ej. BNB). La estructura de Fees debe ser leída directamente por API al arrancar el bot y re-cacheada cada 24 horas. Los números hardcodeados envejecen y provocan ruina.
- Toda deducción por Fee en Arbitraje (Ecuación de Rentabilidad) DEBE asimilar el impacto cascada: Pagar un 0.05% al entrar al Trade 1 de CEX, y un 0.05% al salir por el Trade 2 en DEX, requiere aplicar la matemática compuesta `(1 - F1) * (1 - F2)` y no la suma burda `0.1%`, para mantener precisión milimétrica (Especialmente crítico en Arbitraje Triangular, Skill 14).
- Las comisiones DeFi en V2 son del 0.3%, pero en V3 pueden ser 1%, 0.3%, 0.05% o 0.01%. El rastreador DEBE exigir el "Fee Tier" del Pool en V3 como parámetro de simulación obligatorio.

## 7. Algoritmos o métodos que debe conocer
- Aritmética de Deducción Cruzada de Fees (Cross-Token Fee Calculations).
- API Account/Commission Status parsing.
- Alertas predictivas de agostamiento de saldos (Watermark levels).

## 8. Fórmulas críticas
- **Cálculo Real del Fee (CEX VIP con BNB)**: `Final_Fee_Bps = (Base_VIP_Fee * Discount_Platform_Token) + Kickback_Rate`
- **Impacto Total de Fees (Triangular)**: `Volumen_Salida = Volumen_Entrada * (1 - Fee1) * (1 - Fee2) * (1 - Fee3)`
- **Umbral de Relleno Utilitario (BNB Replenish)**: `if (Balance_BNB < Max(Trade_Volume) * Fee_Pct * 20) { Buy_BNB() }`

## 9. Casos extremos
- Descuido de Moneda de Plataforma (Platform Coin Starvation): El bot calcula arbitrajes esperando un costo del 0.075%. El balance de BNB se acaba. El exchange empieza a cobrar 0.1% desde el activo en trade (USDT). El bot empieza a perder dinero por 0.025% en cada ciclo HFT, creyendo falsamente que es rentable. Ruina Financiera Silenciosa.
- Negative Fee Promotion Trap: Un exchange anuncia "0% Fees para el Par X". El API Information (Metadata) se actualiza de noche, pero el bot lo cacheó por la mañana. El bot no ejecuta operaciones gigantescas ultraseguras porque asume matemáticamente un 0.1% de fee. Pierde oportunidad institucional (Opportunity Cost).
- Tarifa de Custodia (Maintenance Fee) aplicadas sin piedad en el fondeo de perpetuals o de opciones que barren las micro-ganancias retenidas en balance.

## 10. Validaciones obligatorias
- PRE: Chequear la Matriz Dinámica cada vez que el Orquestador llama a la Matemática de Spread, garantizando usar el `Taker_Fee` adecuado.
- CÁLCULO: Mantener lógica para comisiones deducidas del activo cotizado (Quote Asset, ej. USDT) vs activo base (Base Asset, ej. BTC). Las comisiones que reducen el activo base requieren comprar más activo base para que el rebalanceo Delta-Neutral (Skill 16) no quede descuadrado ("Dust Unhedged").
- POST: Realizar una auditoría cruzada post-trade (Audit true-up). Tomar el balance de Exchange menos Saldo Calculado, el diferencial debe ser EXACTAMENTE el Fee pagado en el registro. Si la diferencia es > $0.01 USD, levantar Bandera Roja Forense.

## 11. Criterios de aprobación
- Consulta O(1) de comisiones a la matriz local demora < 0.01ms.
- El PnL reportado asimila el Fee cobrado con un Drift (Error contable) de 0%.

## 12. Criterios de rechazo
- Fallo de lectura del endpoint `/api/v3/account` o similar para constatar el Nivel VIP al inicio (El bot debe negarse a iniciar hasta confirmar comisiones o forzar Hard-Fallback a Fee Estándar castigado).
- Divergencia contable descubierta por el rastreador de comisiones ejecutadas.

## 13. Riesgos que mitiga
- La asfixia de márgenes minúsculos: Arbitrajes institucionales suelen rendir un 0.15% bruto por operación. Si las comisiones sumadas son 0.10%, el bot gana 0.05% y el fondo hace dinero billonario. Si las comisiones están mal calculadas y suman 0.16%, el bot destruye el portafolio en una espiral de muerte milisegundo a milisegundo. Este rastreador es el Auditor Ciego que detiene el algoritmo antes de la ejecución.
- Riesgo de Agotamiento de Colateral Operativo: Perder el nivel de descuento y ser empujado a una categoría de Fee minorista (Retail Tier).

## 14. Integración con otras skills
- Proporciona las Constantes Dinámicas a todas las Matemáticas Fundamentales (Skills 1 a 11 y 24 a 28).
- Trabaja de la mano con la Reconciliación de Balances (Skill 38) para cuadrar cuentas.

## 15. Modelo de datos sugerido
```json
{
  "FeeMatrixRecord": {
    "exchange": "binance_spot",
    "symbol": "BTC_USDT",
    "maker_fee_pct": 0.000,
    "taker_fee_pct": 0.075,
    "discount_asset_enabled": true,
    "discount_asset": "BNB",
    "discount_asset_health": "SUFFICIENT_FOR_500_TRADES",
    "last_updated_ms": 1714521234105,
    "promotional_zero_fee_mode": false
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Background que actualiza parámetros vía REST cada hora, emparejado con un `Struct / Hash Map` ultra optimizado global de Solo Lectura.

## 17. Logs obligatorios
- `[INFO] Fee Matrix Synchronized. Account VIP Tier 4 detected on OKX. Maker: 0.010%, Taker: 0.030%. Arbitrage Math thresholds updated.`
- `[WARN] Binance BNB Balance CRITICALLY LOW. (Expected 0.5 BNB, got 0.01 BNB). Executing Market Buy for 1 BNB to sustain fee discounts.`
- `[DEBUG] Detected ZERO-FEE Campaign active for TUSD/USDT. Temporarily routing 100% of stable-arb flow through this pair.`

## 18. Métricas obligatorias
- `average_taker_fee_paid_usd_per_trade`.
- `total_fees_saved_via_platform_tokens_usd` (Métrica dorada para convencer inversores).
- `fee_divergence_errors_count`.

## 19. Tests unitarios
- Taker Fee Compounding: Calcular entrada 100 y Fee 0.1%. Validar que la salida en cascada descuenta de forma multiplicativa la base menguada, no sobre la entrada original. (Asegurar que la matemática devuelve el valor tras fee, con BigInts).
- Discount Fallback: Setear `Balance_BNB = 0`. El tracker debe arrojar al Orquestador la comisión Taker Cruda `(0.1%)` y cancelar la asimilación asimétrica VIP de `0.075%`.
- Promotional Parser: Inyectar un JSON del Exchange Info indicando fee base 0 y fee VIP 0. Validar que la matriz se ajusta sin fallar cálculos (Divisiones por Cero).

## 20. Tests de integración
- Conexión vía SDK/API Oficial al entorno Testnet/Mainnet en modo sólo lectura (Read-Only) usando la Llave API. Corroborar que extrae y asigna correctamente el Nivel VIP (CommissionRate en Binance).

## 21. Tests E2E
- El agente opera durante 10,000 bloques. La suma en USD de todos los Spreads de Precio crudos arrojó +$5,000 USD. El modulo de Account True-Up (Skill 38) arrojó un ingreso neto final real en Bóveda de +$3,850 USD. El Tracker de Comisiones (Skill 39) debe demostrar y auditar en registro que la diferencia exacta de $1,150 USD fue consumida por Fees Taker en Exchanges y Gas Costs On-chain sin faltar un céntimo de desajuste inexplicable.

## 22. Checklist de producción
- [ ] Incorporación de un Módulo Analítico Visual: Graficar los "Fees Pagados vs Profit Bruto Obtenido". Un porcentaje sano suele ser 30-50% del profit bruto cedido a comisiones en mercados ultra-competitivos.
- [ ] Contabilización de Fee por Depósito/Retiro a la hora de hacer rebalanceos Cross-Exchange. Extraer $1M USDT por la red ETH cuesta $15 (ERC20), pero en BSC (BEP20) cuesta $0.30. Incorporar ese costo logístico oculto a la rentabilidad sistémica global.
- [ ] Activar rutinas de conversión de Dust (Polvo): El exchange genera fracciones en las monedas operadas que son inoperables (Dust). Habilitar la función CEX `Convert Dust to BNB` de forma automática cada 24 horas usando API.

## 23. Ejemplo de configuración no hardcodeada
```yaml
fee_tracking_engine:
  update_fee_tiers_interval_minutes: 60
  auto_replenish_discount_tokens: true
  min_discount_token_balance_usd: 50.0  # Buy BNB if value drops below $50
  preferred_withdrawal_network: "TRC20" # Cheapest for Cross-CEX arbitrage
```

## 24. Ejemplo de pseudocódigo
```javascript
class FeeTrackerMatrix {
    constructor() {
        this.matrix = new Map();
        this.utilTokens = new Map(); // BNB, OKB, etc
    }

    // Fast O(1) synchronous function called millions of times
    getFee(exchange, symbol, isMaker) {
        const record = this.matrix.get(`${exchange}_${symbol}`);
        if (!record) return FALLBACK_WORST_CASE_FEE; // Protect mathematically
        
        let fee = isMaker ? record.maker_fee_pct : record.taker_fee_pct;
        
        if (record.discount_asset_enabled && this.hasSufficientUtilityToken(exchange)) {
             fee = fee * (1 - record.discount_rate_pct); // Apply 25% platform token discount
        }
        
        return fee;
    }

    // Async background task
    async checkUtilityTokenHealth() {
        for (let [exchange, config] of this.getExchangesUsingUtilTokens()) {
             const balance = Ledger.getFreeBalance(exchange, config.asset);
             if (balance < CONFIG.min_discount_token_balance) {
                 log.warn(`Utility token ${config.asset} starvation on ${exchange}. Auto-replenishing...`);
                 await orderManager.executeMarketBuy(exchange, `${config.asset}_USDT`, CONFIG.replenish_amount);
                 this.utilTokens.set(exchange, 'HEALTHY');
             }
        }
    }
}
```

## 25. Criterio final de excelencia
El Fee Tracker es un actuario implacable. Convierte un bot de arbitraje teórico ingenuo (que asume que el mercado no tiene costos) en una estructura financiera purista, prediciendo centavo a centavo cuánto se llevarán los datacenters y los creadores de mercado antes de que el dinero pise las cuentas de banco propias, garantizando que todo beneficio sea verdaderamente neto y líquido.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Cambios de Fee-Tiers institucionales repentinos por el Exchange (API Endpoint sin documentar). (Mitigado por el Auto-Ajuste de Drift Contable Skill 38).
- Dependencias: API Rest Account endpoints.
- Próxima skill: Inventario multi-exchange unificado (Skill 40).
