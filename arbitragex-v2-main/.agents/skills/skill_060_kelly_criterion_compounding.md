# SKILL 060 — Compounding & Reinvestment Engine (Kelly Criterion)

## 1. Propósito superior
Convertir el crecimiento de capital del Agente en un sistema Geométrico y Exponencial sin aniquilar la cuenta en rachas perdedoras. Utilizando el Criterio de Kelly Modificado (Kelly Criterion / Fraction), esta Skill decide milisegundo a milisegundo cuánto porcentaje del "Net Asset Value Global" (AUM) apostar en una oportunidad de arbitraje detectada. Si el bot tiene $100k y gana consistentemente 0.05% por trade, el Compounding Engine se asegura de que el siguiente trade no apueste $100k planos, sino $100,050. Este minúsculo reintegro matemático de beneficios garantiza la curvatura del "Palo de Hockey" a mediano plazo y optimiza matemáticamente el tamaño de la apuesta frente a la probabilidad de ganar / p-value del trade.

## 2. Nivel de conocimiento requerido
Ingeniero en Gestión Institucional de Portafolios (Portfolio Sizing / Money Management). Dominio de Teoría de Probabilidad (Bernoulli Trials en mercados L2), Ecuación Criterio de Kelly (Fórmula original y Half-Kelly), Control Dinámico de Exposición de Cartera (Dynamic Risk Caps), y Limitantes Físicas Liquidez vs Capital (Liquidity Constraints / Scaling Degradation).

## 3. Capacidades principales
1. Criterio de Kelly Dinámico: Aplicar la fórmula pura de Kelly para dictaminar el % de capital a arriesgar: `f* = (p - q/b)`. Donde `p` es la probabilidad empírica de ganar del bot (Win Rate Extraído de la Skill 37), `q` de perder, y `b` el ratio Riesgo/Recompensa histórico (Ganancia Media / Pérdida Media Slippage).
2. "Half-Kelly" Sizing Conservative: Dado que los mercados cripto y HFT son ruidosos (Volatilidad estocástica), apostar el 100% de la sugerencia del Kelly clásico puede llevar a Drawdowns salvajes y Muerte Técnica. El bot divide el dictamen de Kelly por 2 o por 4 para obtener retornos casi idénticos pero rebajando la volatilidad del portafolio en un 50%+.
3. Inyección del Beneficio Directo a Munición HFT (Auto-Rolling Base): El Bot detecta si el Inventario Base tiene nuevos dólares ingresados (Ej. Tras la recolección de Profit Yield de Skill 58 o éxito HFT) e incrementa orgánicamente la "Billetera Virtual Disponible de Fuego" a todas las skills de forma atómica en el mismo bloque.
4. Auto-Tope de Tamaño (Ceiling Constraints - Asymptote Curve): Reconoce que 10% del fondo a veces son $5,000 pero otras veces el fondo crece y 10% son $500,000. El motor de Kelly choca de cara contra la Liquidez L2 del mercado (Slippage Skill 59). Este motor frena a Kelly y Trunca el tamaño si la apuesta cruza el Límite de Impacto del Precio, asegurando que el Crecimiento Exponencial nunca aplaste los Spreads por auto-volumen (Self-sabotage liquidity limits).
5. Down-Scalling Protector de Ruinas (Racha Perdedora): Si un bug o la toxicidad del mercado hace que el Bot encadene 10 pérdidas seguidas por "Unwind Fails" o Latencia. El Capital Baja (Drawdown). El Sistema Kelly *automáticamente* reduce el tamaño nominal de la apuesta subsiguiente. Salva la cuenta de quebrar por terquedad martingala forzando al bot a recuperar su Win-Rate en pequeño formato antes de volver a confiar sus grandes cañones.
6. Re-Inversión Selectiva de Ganancias (Alpha Base Splicing): A veces el CEX/DEX nos paga en Tokens Nativos basuras (Ej. Pagaron con SHIB). El Reinvestment Engine debe usar el Proxy/Swap y transformarlo inmediatamente a Stablecoin (USDT/USDC) antes de integrarlo como pólvora Compound al Kelly. No reinvertir y compoundear "Tokens de Alta Beta Cripto".
7. Re-Alineamiento a Coste Fijo (Fixed Cost Amortization Overrides): Si Kelly te manda a arriesgar el 0.05% de tu cuenta (Ej. Apostar $10 Dólares). Pero mandar la TX a Ethereum Cuesta $15 en Gas (Fee Plana). El Motor detecta "Overridden_by_Minimum_Efficient_Size" (O el tamaño base amortiza el Gas L1 o la orden se ignora y el bot se salta la oportunidad).
8. Exposición Global Interconectada (Total Position Sizing): El Bot hace 50 trades simultáneos. Si Kelly dice "Apunta 10% de tu capital", pero tienes otros 9 trades en progreso consumiendo ya el 90%, el Reinvestment Engine te prohíbe pasar del límite, impidiendo Apalancamiento Sistémico Involuntario.
9. Orquestación del Capital Libre y Extraíble (Extraction Hook): Separa constantemente cuál ganancia es "Compuesta" y cuál es para "Skill 43 Cold Storage Extraction". (Ej. El Kelly retiene y compite con solo el 80% del retorno neto ganado, el resto vuela offline).
10. Modulación Kelly Multi-Estrategia: Aplica fracciones Kelly diferenciadas si el arbitraje es un "CEX-DEX Ciego Peligroso" (Half-Kelly) versus un "Yield Risk-Free Rate Puro Seguro" (Full-Kelly Size permitted).

## 4. Entradas requeridas
- `bot_historical_performance`: Métrica asíncrona de las últimas 24H (`Win_Rate`, `Average_Win_USD`, `Average_Loss_USD`).
- `global_inventory_usd`: El NAV y base en efectivo disponible (Skill 40).
- `signal_confidence_p`: La probabilidad o confianza provista por XGBoost ML Engine (Skill 47) sobre la viabilidad de la oportunidad específica a disparar.

## 5. Salidas esperadas
- `optimal_position_size_usd`: El tamaño absoluto máximo que el Bot Maestro HFT tiene PERMITIDO mandar y arriesgar para el trade L1/L2.
- `portfolio_exposure_status`: Advertencia si la munición base ha sido saturada limitando a las demás estrategias.
- `compound_growth_rate_log`: Métrica visual del crecimiento porcentual y aceleración nominal base.

## 6. Reglas inmutables
- JAMÁS apostar en modalidad Martingala o Posiciones Fijas rígidas ("Voy a lanzar trades de $10,000 para siempre"). Usar tamaños absolutos rompe la posibilidad matemática universal de crear crecimientos en L y sub-utiliza la ventaja matemática (Edge) de la firma quant de Alta Frecuencia.
- El Criterio Modificado Half-Kelly o Fractional-Kelly (Por ejemplo, F/4) debe ser la Norma en criptografía. La asunción de Gaussian Distribution (Campana simétrica) en cripto es fatalmente falsa (Existen "Fat Tails" / Cisnes Negros cada 3 meses). El Kelly al 100% (Full Kelly) causará la aniquilación contable del AUM si se usa frente a los picos de varianza extrema de Web3.
- Si la fórmula Fractional Kelly calcula un `Tamaño Óptimo <= 0%`, indica que el Sistema HFT está estructuralmente roto (El Expected Value es negativo), no es rentable y toda ejecución debe cerrarse de inmediato y entrar en `HALT` automático.

## 7. Algoritmos o métodos que debe conocer
- Kelly Criterion Ecuación (`f = p - (1-p) / (W/L)`).
- Risk-Adjusted Sizing Heuristics.
- Algoritmo de Fraccionamiento de Capital / Optimization Scaling Laws.

## 8. Fórmulas críticas
- **Fórmula Kelly Tradicional**: `f_star = (p * b - q) / b` (f_star = % de banco, p = prob ganar, q = prob perder, b = odds proporcionales ganancia/pérdida).
- **Fractional Kelly (Ej. Quarter Kelly)**: `Position_Pct = f_star / 4.0`
- **Sizing Físico Absoluto Autorizado**: `Trade_Size = MIN(Global_AUM * Position_Pct, Max_OrderBook_Depth_Safe_Impact)`

## 9. Casos extremos
- Degradación Escalar del Alpha (Scaling Ceiling Trap): El bot tiene un Alpha enorme operando trades triangulares L1 por montos de $500 Dólares. Crece 10 veces en 6 meses usando Auto-compounding y el AUM salta de $10k a $100k. El motor Kelly ordena lanzar un Trade Triangular L1 de $5,000. Pero la piscina L2 solo aguanta trades de $800 antes de derretirse por Price-Slippage el 1%. El Compounding choca con el Techo L2; el motor interseca la Lógica de Capacidad de Slippage (Skill 59) y estrangula a Kelly limitando la reinversión al techo físico máximo.
- Drawdown Vertiginoso por Varianza de Red L1 (Network Chaos): Ethereum L1 en crisis congestiva (Gas sube a 500 gwei). Las operaciones del Agente comienzan a fallar costándole $50 cada vez. El bot encadena 10 perdidas de Gas base. El Capital Global decae 1%. El Motor de Kelly reajusta el "Win-Rate local P", deprimiéndolo de inmediato y encogiendo la fracción de Kelly de 2% del AUM a 0.2% del AUM, apretando el cinturón para reducir el daño antes de que el sangrado mate la cuenta.
- Ganancias Abruptas de Altcoins No Liquidadas (Illiquid Paper Profit Bubble): El Bot retuvo PEPE Coin temporalmente en Arbitraje Estadístico. El PEPE sube mágicamente 1000% y el AUM marca "+2 Millones USD" de ganancia en papel. El motor Kelly cree que ahora el fondo es rico, y ordena subir todas las apuestas operativas 10x multiplicando los límites. Si se basó en los precios Spot ilíquidos para estimar AUM, arruina el bot inyectando riesgos irreales en activos sólidos usando fondos prestados/fantasma. Kelly solo calcula sobre el MTM "Mark-To-Market" Liquidado al peor precio Ask (Worst-case execution mark).

## 10. Validaciones obligatorias
- PRE: Asegurar que el Array del Histórico de Rendimiento (`WinRate`, `Reward-to-Risk-Ratio`) se purgue con Ventanas Móviles de (Ej.) 7 días. Basar un Kelly Criterio usando el Win Rate fantástico del mes Bullish pasado para operar los trades hoy en un entorno Lateral Bajista arruinará el apalancamiento proporcional.
- CÁLCULO: Validar la disponibilidad pura del Inventario Libre Contable. Si Kelly lanza tamaño `$10,000`, y el Inventario Libre Global Localizado en la moneda necesaria es `$2,000`, la orden sufre Downsize a `$2,000` pacíficamente emitiendo una bandera de "Starving Capital / Underfunded Route".
- POST: Extraer la fracción de Beneficio Real (Net Profit - Gas Fees) a la reserva asíncrona Contable Global garantizando que el Sizing de la "Apuesta Siguiente" se calculará sobre un Numerador superior.

## 11. Criterios de aprobación
- Entrega determinística y perfecta de límites "Máximos Permisibles y Seguros en Dólares" (Sizing Bounds) O(1) in-memory sin bloqueos síncronos hacia DBs, a la orden y ritmo de fuego de las Skills Operativas Centrales (12 a 20).
- Curvas de Capital histórico que demuestran la Reintegración Geométrica de ganancias frente al crecimiento lineal plano o Martingala desastrosa.

## 12. Criterios de rechazo
- El sistema de Reinversión sobreestima la ganancia asumiendo el "Profit Bruto" antes de pagar las comisiones L1 a Mineros y MEV y Takers de CEX (Skill 50, 39, 42). Esto infla falsamente el crecimiento contable auto-arruinando el cálculo proporcional del bot.
- Sistema Inverso: Ampliar o aumentar los tamaños de las apuestas tras una Pérdida para "Recuperar Rápido" el capital perdido (Rachas de revancha, modo apostador tóxico). La Matemática dicta que a menos capital, MENOR debe ser la posición para preservar supervivencia (Capital Preservation Principle).

## 13. Riesgos que mitiga
- Riesgo Terminal de "Gambler's Ruin" (La Ruina del Jugador): A lo largo de la ejecución de la ley de los grandes números (1,000,000 de operaciones por año), un evento adverso de racha de 20 pérdidas seguidas ESTÁ ESTADÍSTICAMENTE GARANTIZADO a ocurrir en el infinito probabilístico de Web3. Si un bot usa proporciones fijas no atadas al capital, una racha de mala suerte quebrará la cuenta a $0. El Kelly Criterion asegura que la posición arriesgada decrezca a medida que el fondo sufre, volviendo matemáticamente imposible llegar a $0.00 absolutos (Excluyendo Hack L1 Flash Black-Swan).
- Stagnation de Efectividad (Retorno Plano): Sin reinversión, ganar 0.1% al día sobre $10k fijos te da siempre $10 diarios fijos de ganancia (Interés Simple, total en un mes $300). Con un Re-Compound Engine diario, el interés genera el milagro exponencial. A mediano plazo, esos milisegundos se devoran inflaciones completas o capitalizan fondos millonarios (Geometric Magic of Alpha).

## 14. Integración con otras skills
- Receptora Crítica del Ledger Unificado Global (Skill 38) para derivar su Numerador "AUM Global Neto en Riesgo".
- Modera absolutamente al Orquestador Principal (Skill 36) dictándole exactamente el Límite de Fuego de Munición (`Capital Max Size Cap`) a enviar al exchange en paralelo con la Detección de Slippage (Skill 59).

## 15. Modelo de datos sugerido
```json
{
  "KellyReinvestmentSizing": {
    "module_timestamp_ms": 1714521234105,
    "current_global_aum_usd": 125430.50,
    "empirical_win_rate_p": 0.68, // 68%
    "historical_reward_to_risk_ratio_b": 1.25, // Wins are slightly larger than losses
    "raw_kelly_fraction_pct": 42.4, // Suicidal full-Kelly
    "kelly_fractional_multiplier": 0.25, // Using Quarter-Kelly for extreme safety
    "applied_portfolio_exposure_limit_pct": 10.6, // Maximum % of AUM allowed per concurrent trade route
    "absolute_trade_max_size_usd": 13295.63, // 10.6% of AUM cap computed
    "system_status": "COMPOUNDING_ACTIVE_GREEN"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase Singleton Lógica (`RiskAndSizingEngine`). A nivel de Micro-milisegundos, inyectar directamente a la Memoria el valor máximo permitido de ruteo como Float Absoluto; a nivel macro, corre un Cron cada X horas analizando la base de datos Time-Series (Data Lake 37) para re-evaluar asíncronamente (Y calibrar suavemente) los Factores de Probabilidad `WinRate/RiskReward`.

## 17. Logs obligatorios
- `[INFO] Sizing Engine Update. AUM increased to $105k. Empirical WinRate 64%. Adjusted Quarter-Kelly cap to $3,200 max nominal exposure per branch.`
- `[DEBUG] Trade Routing requested Size of $10,000 for Arbitrage #524. Modulating to safe Maximum Physical Cap bounds ($3,200). Enforcing Liquidity Conservation Laws.`
- `[CRITICAL] Empirical Win Rate Decayed < 50% AND R:R Ratio < 1.0 (Expected Value mathematically NEGATIVE). Kelly Engine returned Negative Sizing Limits. HALTING ORCHESTRATOR EXECUTION IMMEDIATELY.`

## 18. Métricas obligatorias
- `kelly_recommended_fraction_daily_variance` (Para entender la confianza matemática propia del robot).
- `compounded_capital_growth_delta_usd` (Visualizador de interés de Bola de Nieve frente a Simple).
- `down_scaling_drawdown_safety_triggers_count` (Cantidad de veces que el bot "Arrugó / Apretó Cinturón" salvando capital).

## 19. Tests unitarios
- Kelly Formula Integrity: Mapear valores `p=0.55` y `b=1` (Win/Loss ratios iguales, 55% de ganar). La fórmula Full-Kelly DEBE devolver exactamente el factor 10% `(0.55 - 0.45/1.0 = 0.10)`. Fractional Kelly de configuración=0.5 (Half-Kelly) DEBE truncar a 5% estricto de AUM base O(1) test validando el control probabilístico.
- Cero / Muerte por Factor (Negative Edge Stop): Darle Win-Rate de 45% y Reward Ratio de 1.0. La matemática DEBE devolver Kelly Fraction = Negativa (Ej. `-5%`). El algoritmo interceptador Local DEBE devolver límite en Dólares de CERO ABSOLUTO (`$0.00`) para castrar operativamente cualquier intento de dispatch por el Bot central con ventaja negativa.
- AUM Compound Reflection Test: Proveer una inyección de beneficio L2 de $50,000 (Ganancia repentina por Yield L2). Reclamarla. Verificar que el método de Sizing asimile ese inventario e inmediatamente suba el `absolute_trade_max_size_usd` para que el siguiente trade dispare fuegos de artificio con el nuevo tanque de capital.

## 20. Tests de integración
- Levantar Orquestador L2 Master (Mock). Conectar el Gestor de Límite (Kelly Engine) y el Gestor de Riesgo Global de Ruinas (Circuit Breaker Drawdown, Skill 41). Comprobar la superposición paramétrica: Kelly te manda reducir Apuesta si pierdes progresivamente, pero si excedes el "Maximum Daily Drawdown Límite" (Ej -10% de PnL Creado Base Abajo), el Circuito del Breaker Principal "Overridea" a Kelly y mata la red sin dudar. Confirmación de Jerarquías de Seguridad operativas.

## 21. Tests E2E
- El agente atraviesa un largo mes en un mercado Bajista en OKX. El Arbitraje funciona a la baja ganando poquísimo (0.01% y su Win Rate se degrada a 52%). El Reinvestment Kelly-Engine aprieta la válvula reduciendo las participaciones por debajo de la barrera de los 100 dólares, limitando el desangre al mínimo friccional. De pronto un viernes se rompe una stablecoin algorítmica. Aparece un mar de Spread (5%). Win Rate explota al 85%. Kelly actualiza asíncronamente y destapa la válvula en el siguiente trade. El Bot ataca arriesgando el 25% de la cuenta L2 al completo capturando una rebanada colosal. Esa ganancia engorda su Capital, y para la trade subsiguiente (milisegundos después), usa el nuevo capital base gordo generando beneficios interestelares exponencialmente mayores. Todo bajo el marco estricto del "Control Probabilístico Sin Emoción" que ningún humano podría medir en milisegundos.

## 22. Checklist de producción
- [ ] Aplicar Múltiples Cestas de "Track Record" (Registros): La matemática de Kelly funciona mal si mezclas peras con manzanas. Un "Trade Triangular" gana 99% de veces muy poco. Un "Statistical Arbitrage Par" gana 60% de veces pero gana mucho. Mantener Registros de Desempeño y Factores de Kelly AISLADOS (Compartimentalizados) por tipo de Táctica de Negocios en el Bot para que una habilidad estancada no arrastre y castre financieramente la asignación de munición de una habilidad exitosa (Multi-Alpha Fund Allocation Strategy).
- [ ] Control Ciego L1 de Contabilidad a AUM Realista (Net Net AUM): Considerar la pérdida flotante asíncrona de los Gas Limits en la Red de Ethereum. Al calcular la cuenta a nivel base (AUM), deducir los remanentes muertos y comisiones atrapadas para que no construyas un Sizing Exponencial sobre aire financiero irreal de la API CEX.
- [ ] Límite "Gordo" (Absolute Cap Size): Cripto HFT No puede operar con posiciones gigantes sin "Partir" a Uniswap en 10 niveles de Liquidez destrozándose internamente (Price Impact). Indiferentemente del resultado Fraccional que dicte Kelly, el Cap (Tope Máximo Ciego) Físico siempre mandará, asegurando Cielos Lógicos a las Inversiones del Motor para no mover jamás el mercado en su propia contra (Pre-trade Slippage Shield Constraint).

## 23. Ejemplo de configuración no hardcodeada
```yaml
compounding_and_sizing_engine:
  enable_kelly_optimization: true
  kelly_fraction_modifier: 0.25 # Use Quarter-Kelly. Highly conservative risk.
  lookback_window_trades_stats: 5000 # Evaluate Win-Rate strictly over the last 5k closed events
  minimum_acceptable_win_rate_threshold_pct: 50.1 # Never fire mathematically doomed ops
  maximum_aum_exposure_per_single_concurrent_leg_pct: 12.0
  absolute_ceiling_cap_usd_to_avoid_slippage: 50000.0 # Prevents blowing up thin L2 orderbooks
```

## 24. Ejemplo de pseudocódigo
```javascript
class KellyCompoundingEngine {
    constructor(performanceDb, globalLedger) {
        this.stats = performanceDb;
        this.ledger = globalLedger;
        this.currentConfiguredFraction = CONFIG.kelly_fraction_modifier; // e.g. 0.25 for Quarter-Kelly
    }

    // O(1) Synchronous Sizing Check evaluated immediately before ANY trade execution dispatch
    calculateMaximumSafeDispatchSizeUsd(strategyCategoryName) {
        const aum = this.ledger.getNetLiquidGlobalValueUsd();
        const recentStats = this.stats.getRollingPerformanceMetrics(strategyCategoryName, CONFIG.lookback_window_trades);
        
        // Safety Fallback (New strategies start small)
        if (recentStats.totalTrades < 50) return Math.min(aum * 0.01, CONFIG.absolute_ceiling_cap_usd);

        // Expected Value check
        if (recentStats.winRate < 0.501 && (recentStats.avgWin / recentStats.avgLoss) <= 1.0) {
             return 0.00; // Expected value is negative. Reject sizing.
        }

        // Full Kelly Computation: f* = (p*b - q) / b
        const p = recentStats.winRate;
        const q = 1.0 - p;
        const b = recentStats.avgWin / recentStats.avgLoss;
        
        const fullKellyFraction = (p * b - q) / b;
        if (fullKellyFraction <= 0) return 0.00; // Protection

        // Limit the leverage and smooth the volatility using Fractional Kelly Constraint
        let finalKellyPct = fullKellyFraction * this.currentConfiguredFraction;
        
        // Strict Hard-Coded Global Strategy Limits Override
        finalKellyPct = Math.min(finalKellyPct, (CONFIG.max_aum_exposure_pct / 100));

        // Absolute physical size calculation
        let maxSizingUsdAllowed = aum * finalKellyPct;
        maxSizingUsdAllowed = Math.min(maxSizingUsdAllowed, CONFIG.absolute_ceiling_cap_usd);

        return maxSizingUsdAllowed;
    }
}
```

## 25. Criterio final de excelencia
El Compounding & Reinvestment Engine es el "Acelerador Probabilístico" definitivo que transforma un algoritmo lineal plano de bajo beneficio, en una Máquina Financiera Geométrica implacable a lo largo del tiempo. Fusiona sin error la prudencia militar (Para sobrevivir épocas de pérdidas reduciendo exposición para prevenir Bancarrota) con la avaricia lógica (Maximizando munición capitalizable en épocas de Oro y Ratios Óptimos). Transmuta la operativa Cripto HFT del Agente, de un "Juego de Céntimos" a una Empresa de Escalamiento Algorítmico y Retornos Institucionales Compuestos Superiores.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Over-estimation of AUM (Inflado artificial por Tokens Ilíquidos o Saldos Bloqueados) que engaña a Kelly haciéndolo pensar que el Fondo tiene capital Ficticio que resulta inoperable (Solucionado usando "Solo AUM Limpio Extraíble Spot").
- Dependencias: Skill 38 (Accounting Ledger), Skill 37 (Time-Series Metrics) y Trade Optimizers.
- Próxima skill: Orquestador de Operaciones en Futuros Perpetuos (Hedge & Execution) (Skill 61).
