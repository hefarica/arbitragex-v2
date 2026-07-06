# SKILL 072 — Generador de PnL y Tax Compliance (Auditoría Legal Cripto)

## 1. Propósito superior
Automatizar la Consolidación Contable Universal (Accounting & Tax Compliance L1 L2) del fondo. Un Agente que ejecuta 50,000 operaciones HFT diarias a través de 5 CEXes y 10 Blockchains generará un caos de datos. Esta Skill procesa Atómicamente O(1) la conciliación contable de Fees CEX, Gas L1, Deslizamientos O(1), e Impermanent Loss, generando un PnL (Profit and Loss) Exacto al Céntimo Cripto, y exportando Formularios de Conformidad Fiscal (Tax Ledgers, LIFO/FIFO Cripto L2) para Auditoría Legal del Mundo Real, protegiendo al Fondo Cuantitativo de los Reguladores Fiscales.

## 2. Nivel de conocimiento requerido
Auditor Financiero Cuantitativo (Quant Accountant L2 O(1)). Modelos de Valoración Forense Cripto Mark-To-Market (MTM O(1)), Cálculo FIFO (First-In, First-Out) / HIFO (Highest-In, First-Out) para Criptomonedas L1 L2. Deducción de Tasas de Gas L1 como "Capital Loss", Contabilidad de Activos Derivados (Funding Rates, PnL No Realizado HFT), e Integración de Reportes Tributarios Estándar (CSV/PDF para Firmas Auditoras L2 O(1)).

## 3. Capacidades principales
1. Conciliación MTM HFT (Mark-To-Market O(1)): Calcula el Valor Neto de los Activos (AUM L2) del fondo milisegundo a milisegundo. Si el fondo tiene 50 Shitcoins L1 L2 atascadas O(1) de Arbitrajes Ciegos, esta Skill hace fetch del Precio Exacto O(1) al momento del cierre diario, marcándolas a mercado para PnL Real No-Dilusivo Cripto L2.
2. Deducción Taxable Asimétrica (Gas y Comisiones L1 L2): Extrae los Taker Fees O(1) del CEX L2 (Skill 38) y TODO el Gas L1 (EVM BaseFee, MEV Bribes Skill 67 L1) y los consolida como "Costo de Venta (COGS Cripto L1 O(1))" para rebajar matemáticamente la base imponible del Fondo Cuantitativo (Tax Deduction Engine HFT O(1)).
3. Metodología FIFO/LIFO/HIFO Engine O(1): Si el bot compra 1 BTC a $50k, 1 BTC a $60k, y luego vende 1 BTC HFT a $65k Cripto L2 O(1). El Orquestador Contable asume HIFO (Highest In, First Out) vendiendo la fracción de $60k para minimizar la carga fiscal de "Ganancias de Capital a Corto Plazo" (Short-term Capital Gains Optimizer L2 O(1)).
4. Reconocimiento Atómico de PnL Sintético L2: Integra el Yield extraído por Funding Rates CEX (Skill 16 L2) y Market Making Fees AMM V3 (Skill 63 L1) como "Ingreso de Renta Fija Cripto L1/L2", separándolo contablemente de las Ganancias Taker Capital L2, facilitando Estructuración Offshore Legal HFT O(1).
5. Descalce de Airdrops y Hard Forks L1 (Windfall Profits O(1)): El Bot estaba en Ethereum y cae el Airdrop de una L2 Cripto O(1). Este Módulo lo clasifica Atómicamente L2 O(1) como Ingreso Ordinario (Costo Base $0 Cripto), aislando su cálculo de los ratios Sharpe/Sortino O(1) del modelo Predictivo XGBoost L2 (Skill 47) para no confundir al ML con Falso Alpha HFT L1.
6. Auto-Generador de CSV Audit L2: Exporta Logs estandarizados `(Date, Pair, Side, Size, Price, Fee, Venue, TxHash L1)` L2 O(1) formateados para software Fiscal Cripto (CoinTracking, Koinly, TurboTax) evadiendo auditorías humanas Cripto pesadas O(1).
7. Cálculo de "Wash Sale Rule" L2 O(1): Advierte contablemente si el Bot HFT hizo Arbitraje Cripto perdiendo $1 L2 O(1) en el Activo X y lo recompró 1 segundo después L2 (Generando Loss). El Módulo HFT O(1) marca eso como "Wash Sale Penalty L2" anulando el Loss Dedicible fiscal L2 HFT.
8. Balance Sheet Multi-Cadena Cripto L1 L2 O(1): Emite en Tiempo Real la Hoja de Balance General (Activos CEX, Pasivos Préstamos Aave Skill 28 L1, Colateral Futuros Skill 61 L2, Liquidez Atascada Puentes Skill 66 L1). Net Asset Value (NAV) Unificado L1 L2 O(1).
9. Oráculo de Precios Tributarios Cripto L1 (Daily Close L2): Fija los precios Fiscales oficiales Cripto del Mercado (CoinGecko / Oráculos CEX Oficiales L2) a las `00:00 UTC` Cripto O(1) O(1) exigidos por normativas Legales HFT Cripto O(1).
10. Shadow PnL Debugging (Detección O(1) Fallo Ledger): Cruza la Matemática C++ Local O(1) HFT con el Endpoint CEX `GET /account`. Si el Exchange dice que tienes $10,000 y el Agente cree que tiene $10,050. El Auditor Cripto O(1) emite `RECONCILIATION_ERROR_L2` para investigar "Deslizamientos Fantasma Cripto O(1)" (Skill 59 L2).

## 4. Entradas requeridas
- `global_ledger_transactions_l2_o1`: Un Stream infinito HFT O(1) de todas las Órdenes Fildeadas L2 (CEX/DEX/L1).
- `external_tax_rules_config_o1`: "Método FIFO, Tax Year 2026, Región Fiscal Offshore/USA".
- `daily_closing_prices_oracle_l2`: Precios de Cierre MTM Cripto L1 L2 O(1) para inventario flotante.

## 5. Salidas esperadas
- `nav_net_asset_value_realtime_o1`: Un Float Unificado de Riqueza O(1) HFT Cripto que gobierna el Risk Sizing Cripto (Skill 60 Kelly L2).
- `crypto_tax_reports_csv`: Reportes de Transacciones HFT Cripto O(1) Consolidados para Contadores L2 O(1).
- `reconciliation_alerts_l2`: Mismatch Audit Errors (Fugas Contables L1 L2 O(1)).

## 6. Reglas inmutables
- Separación Lógica del PnL O(1) (Trade vs Fees): Al medir la Inteligencia del Bot XGBoost HFT L2 (Skill 47), usar el *Gross PnL* (Sin Comisiones L1). Al medir la eficiencia del Despliegue HFT Real (La Plata Cripto L2), Usar *Net PnL* L1 L2 O(1). La Confusión de estos Valores HFT L2 Cripto engaña a las IA HFT y las lleva a Operar Modelos Takers L2 Quebrados L1.
- No Contabilizar Transferencias L1 L2 como Operaciones Taxables HFT O(1) Cripto. (Skill 42 L1/L2 Mover USDC de Binance a Arbitrum es un `Transfer` O(1), NO un `Sell` HFT. Si la BD lo cuenta como Venta, destruye el PnL Tributario L1 Cripto HFT inflando Capital Gains).
- Precisión Decimal Extrema O(1) Cripto. Prohibido usar `float` estándar de JavaScript (IEEE 754 precision errors O(1)). La Contabilidad Fiscal Cripto O(1) DEBE procesarse en `BigNumber.js / Rust BigDecimal L2 O(1)` evitando que `$100.00 - $99.99 = $0.00999999998` HFT arruine los ledgers Institucionales Cripto L1 L2.

## 7. Algoritmos o métodos que debe conocer
- Métodos de Base de Costo (HIFO, LIFO, FIFO, Specific Identification L2 O(1)).
- Time-Weighted Return (TWR L2 O(1)) y Money-Weighted Return (MWR) para Performance Institucional.
- Accounting Double-Entry Ledger (Partida Doble) Aplicado a Blockchain L1 L2.

## 8. Fórmulas críticas
- **Unrealized MTM PnL L2 O(1)**: `Sumatoria( (ClosePrice - AvgEntryPrice) * InventorySize )`
- **TWR Performance HFT L2 O(1)**: Aísla el Rendimiento Cripto del Fondo O(1) independiente de si inyectaste $1M Dólares ayer L2 O(1). El Alpha puro O(1) Cripto L1 L2 HFT.

## 9. Casos extremos
- Explosión Contable MEV L1 (Bribes L1 como Fee HFT): El Bot ejecuta Sandbox MEV L1 (Skill 67). Paga $10,000 Dólares al Minero Cripto EVM L1 como "Propina (Bribe O(1))" para ejecutar un trade de $11,000 L1. El Bot cree que compró barato L1 Cripto, pero obvió el Bribe L1 O(1). Contablemente asume que debe impuestos sobre $11,000 de Ganancia HFT L1. Solución O(1): El Auditor Parseador L1 O(1) Extrae el campo EVM `block.coinbase.transfer()` y lo anota sagradamente como `DEDUCTIBLE_L1_NETWORK_FEE`, blanqueando la matemática Tributaria L1 O(1).
- Impermanent Loss Tax Deduction L1 (Uniswap V3 LP HFT L2 O(1)): Tienes ETH en un Pool L1. Baja el ETH, tienes Pérdida Impermanente O(1). ¿Es Taxable L2? NO hasta que saques la Liquidez HFT (Burn LP NFT L1). El PnL Engine O(1) separa estrictamente "Paper Loss L1" (No deducible) de "Realized Loss L1" (Deducible tras el Burn O(1) HFT L1 Cripto).
- Errores de Base Moneda L2 (Cross-Pairs HFT): Arbitras ETH/BTC L2 O(1). Ganas 0.1 BTC L2. ¿Cuánto dinero "Fiat" ganaste L1 L2? La Ley obliga tasar todo en Dólares O(1) en el momento exacto del Fill HFT O(1). El Módulo HFT O(1) hace Join Asíncrono O(1) con el precio BTC/USDT en ese Milisegundo O(1) para sellar el CSV con el equivalente Fíat exigido por los reguladores L2 O(1).

## 10. Validaciones obligatorias
- PRE: Chequeo de Ledger Atómico (Skill 38 L2 O(1)). Validar que no hay "Trades Huérfanos L2 Cripto". Si falta 1 Pata de un Triangular (Skill 53 L2), el PnL se dispara a -100% L2. Filtrado O(1) HFT.
- CÁLCULO: Incorporación del Costo de Funding de Futuros (Skill 16 L2 O(1)). Si la estrategia Cash and Carry HFT Cripto gana $1,000 L2 O(1) de Tasa. Se computan Atómicamente O(1) como Intereses L2 Cripto, no Ganancias de Capital.
- POST: Reconciliación Asíncrona L2 (Nightly Job O(1) Cripto). A las 2 AM HFT, el Bot extrae vía REST API O(1) el Historial del CEX, lo Cruza con su Base Local TSDB (Skill 37 L2) y emite Certificado de Cuadratura Matemática L2 O(1).

## 11. Criterios de aprobación
- Generación in-memory O(1) de Reportes de Capital Gains (HIFO O(1) L2) a través de +1,000,000 de Trades HFT en menos de 5 segundos O(1) (Batch Processing C/Rust L2 O(1) Cripto).
- Net Asset Value (NAV L2) Actualizado milisegundo a milisegundo HFT cruzando todos los puentes L1, Exchanges L2 CEX, Aave Lending L1 L2 y Futuros Perps L2.

## 12. Criterios de rechazo
- Promediar Comisiones HFT L2 (Ej. "Asumir que pagamos 0.05% CEX O(1)"). Las Comisiones CEX varían por Slippage, VIP Tiers (Descuentos BNBB), y Gas L1 Dinámico HFT. Contabilidad Cripto O(1) HFT Exige lectura real del campo `Commission` o `Gas_Used * BaseFee L1 O(1)` por el Tick Exacto O(1). Asumir valores Destroza la Conformidad Fiscal L1 L2.
- Dejar Activos HFT "Perdidos L1 O(1) Cripto". Si se enviaron 1000 USDC por el Puente (Skill 66 L1 O(1)) y están "Viajando L1". El AUM baja 1000 USDC. El Bot detiene su operativa creyendo que "Perdió dinero". (Fallo de In-Transit Ledger Cripto L2 O(1)).

## 13. Riesgos que mitiga
- Muerte Fiscal y Descalces de Capital L2 O(1) (Tax Wipeout Cripto). Un Bot Cuantitativo HFT Cripto O(1) puede ganar $100,000 O(1) HFT al final del año. Pero si generó Ganancias Brutas L2 por $1,000,000 y Pérdidas Brutas HFT por -$900,000 Cripto (Wash Trading/HFT Spread), los Gobiernos en algunos países L2 O(1) te Cobran Impuestos sobre el $1 Millón, ignorando tus Pérdidas HFT. Esta Skill O(1) alerta matemáticamente si el Bot está "Cosechando Pérdidas Tóxicas L2 O(1)" y reestructura los Tiempos HFT (Ej. Hold 31 Días O(1) Cripto) para Optimizar la Rentabilidad Institucional Cripto O(1).

## 14. Integración con otras skills
- Validador L2 O(1) HFT de los datos en Crudo del Ledger (Skill 38 L2 O(1)).
- Fuente de Verdad Financiera (AUM Cripto O(1)) para el Gestor de Kelly Criterion (Skill 60 L2 O(1)).
- Absorbedor de Métricas del Orquestador L1 CEX-DEX (Skill 64 L1 L2 O(1)).

## 15. Modelo de datos sugerido
```json
{
  "TaxAndPnLComplianceEngineO1": {
    "report_timestamp_ms_o1": 1714521234105,
    "global_nav_usd_o1": 545210.45, // Total Wealth of the HFT Fund
    "daily_metrics_l2_o1": {
      "realized_gross_profit_usd_o1": 1500.20,
      "total_cex_fees_usd_o1": -145.50,
      "total_l1_gas_bribes_usd_o1": -250.00,
      "funding_rates_earned_usd_o1": 85.50,
      "net_pnl_usd_o1": 1190.20
    },
    "tax_optimization_o1": {
      "hifo_short_term_gains_usd_o1": 850.20, // Minimized via HIFO Math L2 O(1)
      "wash_sales_flagged_count_l2_o1": 4, // 4 HFT trades ignored for loss harvest
      "unrealized_paper_loss_harvestable_o1": 5000.0 // Suggestion to sell/rebuy for Tax Harvesting L2
    },
    "reconciliation_status_o1": "PERFECT_MATCH_L2_CEX_AND_L1_RPC"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Background Cripto `QuantitativeAuditor_O1`. Ejecuta Rutinas `calculateHIFO()` HFT asincrónicamente y Mantiene el `SharedArrayBuffer` Nav_L2 O(1) actualizado al ms O(1) para el Agente Principal L2.

## 17. Logs obligatorios
- `[INFO] Tax & PnL Engine O(1) L2: Reconciled 15,204 Trades from Binance L2. Total Net PnL Today: +$1,420.50 USD. Exporting CSV to Cloud Storage for Compliance.`
- `[DEBUG] Tax Loss Harvesting Opportunity L2 O(1): ETH bag sitting at -$5,000 Paper Loss L2. Selling and Re-buying via Options Synthetic L2 to harvest Tax Loss without triggering Wash-Sale Rules L2 HFT O(1).`
- `[CRITICAL] LEDGER RECONCILIATION FAILED L2 CEX O(1)! Local HFT SQL TSDB says we have 15 ETH. Binance API says we have 14.8 ETH. Halt Trading L2 HFT. Investigating 0.2 ETH Leak (Possible Fee Extraction or Hack L2 O(1) HFT).`

## 18. Métricas obligatorias
- `sharpe_ratio_twr_l2_o1` (Rendimiento ajustado al Riesgo Real Cripto O(1)).
- `fee_to_gross_profit_ratio_l2_o1` (Si pagamos $1000 a CEX L2 para ganar $1100, el Bot HFT está quebrado asintóticamente L2 O(1)).
- `unrealized_mtm_pnl_l2_o1`.

## 19. Tests unitarios
- HIFO Accounting Engine O(1): Input: `Buy 1 BTC a 50k, Buy 1 BTC a 60k`. Input: `Sell 1 BTC a 55k`. Output Esperado O(1) C++: El Lote vendido fue el de 60k. Pérdida Declarada: `-$5k`. (Lógica HIFO). Fallar si el Motor arroja Ganancia de `+$5k` (FIFO Logic L2 Cripto O(1)).
- Precision Math Loss L2: Ingresar `Amount = 0.00000001 BTC`. Precio: `65000`. Tax: `0.05%`. Operaciones en Floats JS dan Underflow NaN. La Librería BigNumber Rust O(1) DEBE devolver Computación Contable HFT Exacta sin Destruir la Base Imponible O(1).

## 20. Tests de integración
- Levantar un Servidor Mock CEX L2 con 1,000 Transacciones L2 Históricas pre-computadas (Incluyendo Fees, Rebates L2 CEX, Bribes L1, Funding L2 O(1)). Procesar la Noche Contable `ReconcileNightlyBatch()`. El Motor PnL L2 HFT DEBE emitir un CSV cuyo Total PnL O(1) coincida hasta la milésima de céntimo (Precision Cripto O(1)) con la hoja Excel Maestra Precalculada. Tolerancia Cero O(1).

## 21. Tests E2E
- El agente HFRC Cripto O(1) opera como un Demonio HFT 24/7 durante el Q4 2026 L2. Ejecuta 2.5 Millones de trades Arbitrando L1 (Bridges) y L2 (Binance/OKX). El 31 de Diciembre a las 23:59 UTC, El Motor `Skill 72 O(1)` compila y Sella Atómicamente O(1) HFT el Año Fiscal Cripto. Deduce los $150,000 en Gas MEV O(1) gastados como Gastos Operativos. Cruza todas las Operaciones Triangulares a Dólares Usando Oráculos MTM Milsisegundo O(1). Optimiza con HIFO Cripto las Ventas CEX O(1). Genera el TurboTax/CoinTracking CSV Cripto de Inmediato O(1). Todo auditado L1 L2, permitiendo a los Managers del Fondo Cuantitativo Firmar Legalmente Cripto Offshore O(1) sin el suplicio mortal contable HFT Manual L2.

## 22. Checklist de producción
- [ ] Detección Automática de Airdrops y Hard Forks L2 O(1): Usar Escáner Blockchain L1 (Skill 21 L1) y CEX API `GET /asset/assetDividend` L2 O(1) para no perder ingresos mágicos Cripto O(1). Todo el Airdrop debe sumarse al AUM L2 HFT Cripto O(1) pero bajo la Categoría de Ingreso "Zero-Cost-Base", blindado para el IRS/HMRC Cripto O(1).
- [ ] Mapeo de Sub-cuentas Sybil L2 O(1): Las 50 Subcuentas HFT CEX (Skill 71) NO PUEDEN reportarse como 50 Agentes Aislados L2 O(1). El Generador de Tax DEBE Consolidarlas bajo un mismo ID Fiscal (Corporate Entity L2 Cripto O(1)), unificando los inventarios O(1) y neutralizando Wash Trades Cross-Accounts L2 HFT.

## 23. Ejemplo de configuración no hardcodeada
```yaml
pnl_tax_compliance_engine_l2_o1:
  accounting_method_l2_o1: "HIFO" # Highest In First Out optimizes crypto taxes heavily
  fiat_base_currency_l2_o1: "USD"
  enable_automated_nightly_reconciliation_l2_o1: true
  export_format_compliance_l2_o1: "COINTRACKING_CSV_O1"
  auto_harvest_tax_losses_at_year_end_l2_o1: true # Advanced Module HFT L2 Cripto
  strict_bignumber_precision_l2_o1: true
```

## 24. Ejemplo de pseudocódigo
```javascript
class PnLAndTaxComplianceEngine {
    constructor(dbL2, priceOracleO1) {
        this.db = dbL2;
        this.oracle = priceOracleO1;
    }

    async generateHifoTaxReportO1(yearL2) {
        const allTradesL2 = await this.db.getAllFillsForYear(yearL2);
        let inventoryLots = []; // Stores {amount, price, timestamp}
        let taxEventsCSV = [];
        
        for (const trade of allTradesL2) {
            // Unify pricing to USD Base MTM O(1)
            const fiatPrice = await this.oracle.getHistoricalPriceMs(trade.asset, trade.timestamp);

            if (trade.side === 'BUY') {
                inventoryLots.push({ amount: trade.size, costBase: fiatPrice, time: trade.timestamp });
            } else if (trade.side === 'SELL') {
                // HIFO Math O(1) L2 - Sort lots descending by Cost Base
                inventoryLots.sort((a, b) => b.costBase - a.costBase);
                
                let remainingToSell = trade.size;
                while (remainingToSell > 0 && inventoryLots.length > 0) {
                    const highestLot = inventoryLots[0];
                    const amountToDeduct = Math.min(highestLot.amount, remainingToSell);
                    
                    const capitalGainUsdL2 = (fiatPrice - highestLot.costBase) * amountToDeduct;
                    const feeDeductionUsd = trade.feeUsd; // Include EVM Gas or CEX Fee
                    
                    taxEventsCSV.push(this.formatCsvL2(trade, amountToDeduct, capitalGainUsdL2 - feeDeductionUsd));
                    
                    highestLot.amount -= amountToDeduct;
                    remainingToSell -= amountToDeduct;
                    
                    if (highestLot.amount === 0) inventoryLots.shift(); // Remove depleted lot
                }
            }
        }
        return taxEventsCSV;
    }
}
```

## 25. Criterio final de excelencia
El Motor PnL Cripto Cuantitativo transforma al Agente HFRC (Un hacker de latencia Cripto) en un Fondo Financiero Legal y Auditable Institucional (BlackRock-tier Cripto Compliance L2 O(1)). Al matematizar con precisión Absoluta las Comisiones L2, Impermanent Loss L1, y Tasas de Gas O(1) bajo Métricas Contables Globales (TWR/NAV HIFO Cripto L2), el Bot permite la Escala de Inversión Fiduciaria (Billones L2 O(1)) sin el peligro mortal de las auditorías Fiscales HFT Cripto O(1).

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Flash Loan Tax Implications L1 O(1). Los préstamos flash de millones de dólares (Skill 28 L1 O(1)) devueltos en 1 bloque EVM generan un volumen transaccional de Trillones L1 L2 al final del año. El Engine L2 DEBE ignorarlos y considerarlos puramente PnL Operativo Net O(1) HFT, o Koinly Cripto colapsará por Volúmenes L1 Irreales.
- Dependencias: MTM Price Oracle (Skill 14 L2 O(1)), Ledger TSDB (Skill 38 L2 O(1)).
- Próxima skill: Análisis de Sentimiento NLP (Twitter/News Scraper HFT) (Skill 73).
