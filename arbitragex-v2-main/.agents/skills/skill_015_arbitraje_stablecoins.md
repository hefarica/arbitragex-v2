# SKILL 015 — Arbitraje de stablecoins

## 1. Propósito superior
Explotar micro-desviaciones (De-pegs) entre monedas estables (USDT, USDC, DAI, TUSD, FDUSD) en entornos ultra-líquidos, como Curve Finance, Uniswap o pares de Stable-fiat en CEX. Dado que las estables convergen teóricamente a $1.00, esta skill permite arbitrajes masivos de altísimo volumen y bajo riesgo de contraparte mediante algoritmos optimizados de reversión a la media.

## 2. Nivel de conocimiento requerido
Experto en Microestructura de Stablecoins y DeFi Stableswaps. Comprensión matemática profunda del invariante "Stableswap" de Curve (fórmula compleja que combina producto constante y suma constante), algoritmos de Market Making pasivo para estables, y conocimiento de eventos de redención (Mint/Burn on-chain con Tether/Circle).

## 3. Capacidades principales
1. Detección de De-pegs menores (0.01% - 0.5%) y mayores (1%+) en tiempo real.
2. Interacción específica con pools "Stableswap" (Curve 3pool, Uniswap V3 0.01% fee tiers).
3. Monitoreo de "Amplification Parameter (A)" en los pools de Curve para calcular el slippage de estables correctamente.
4. Ruteo de liquidez masiva (Múltiples millones de dólares por trade) gracias al bajísimo slippage de estos pools.
5. Identificación de estables "sintéticas" o "sobre-colateralizadas" (DAI, FRAX, sUSD) vs "fiat-backed" (USDC, USDT) ajustando perfiles de riesgo.
6. Capacidad para arbitrar stable-fiat (ej. vender USDT por USD bancario real si está sobre el valor de $1.00 en Kraken).
7. Cálculo de fees especiales (0.01% a 0.04%) característicos de los mercados de estables.
8. Retención de activos cruzados (ej. mantener USDT asumiendo que volverá a 1.00 en vez de hacer ciclo cerrado).
9. Filtro de eventos apocalípticos: Suspender el algoritmo si la desviación supera un límite de pánico (ej. USDC a $0.88 durante crisis de SVB), activando alertas humanas.
10. Gestión de Liquidity Mining o incentivos de Yield integrados en el spread de la stablecoin.

## 4. Entradas requeridas
- `stable_pairs`: Streams de pares específicos (USDT/USDC, TUSD/USDT, DAI/USDC) en DEX y CEX.
- `curve_pool_state`: Invariantes, balances totales, virtual prices y parámetro A de los pools de stables.
- `fiat_onramp_fees`: Comisiones de retiro fiat en caso de arbitraje contra USD real.
- `depeg_threshold_config`: Configuración de alarma (cuándo es ineficiencia y cuándo es colapso sistémico).

## 5. Salidas esperadas
- `stable_arb_signal`: Oportunidad matemática (Comprar TUSD, Vender USDT).
- `projected_repeg_time`: Estimación probabilística del retorno a $1.00 (opcional, para modelos estadísticos).
- `systemic_risk_alert`: Evento de pánico disparado hacia el módulo Risk Engine si el de-peg es muy violento.

## 6. Reglas inmutables
- Nunca operar una "algoritmic stablecoin" (ej. UST, Frax sin colateral duro) con los mismos parámetros de riesgo que una "fiat-backed" (USDC/USDT). Requerir un margen de seguridad 10x superior.
- No ejecutar en picos de crisis (desviaciones bruscas > 3%) asumiendo reversión a la media garantizada. El mercado podría "saber algo que nosotros no".
- Ajustar de manera microscópica los Fees (los trades de stablecoins mueren si el bot asume 0.1% de fee cuando el exchange cobra 0.01%).
- Aprovechar los flash loans (Skill 28) dado que las oportunidades de stablecoins permiten inyectar capital casi infinito sin desplomar el mercado base.

## 7. Algoritmos o métodos que debe conocer
- Fórmulas analíticas de Curve Stableswap Invariant.
- Arbitraje Estadístico (Reversión a la media, Co-integración, Modelos de Ornstein-Uhlenbeck).
- Optimal Order Routing en pools de alta liquidez fragmentada.
- Mecanismos de Redención Institucional (Tether peg dynamics).

## 8. Fórmulas críticas
- **Stableswap Invariant (Simplificado)**: `An^n * sum(x_i) + D = ADn^n + D^(n+1) / (n^n * prod(x_i))`
- **Diferencial Real (Spread)**: `|Price - 1.0000|`
- **ROI Operable**: Debido a que los fees en CEX para stables suelen ser ínfimos (o cero en promociones), una desviación de `$0.9995` es matemáticamente explotable si el fee de taker es `< 0.02%`.

## 9. Casos extremos
- De-Peg estructural (Ej. Colapso de UST o pánico temporal de USDC-SVB). El bot intentaría arbitrar asumiendo reversión, comprando el activo hundiéndose y yendo a la bancarrota.
- Promociones CEX de "Fee Cero": Un CEX pone Maker/Taker 0% para el par USDC/USDT. Bots inundan el exchange operando ineficiencias de 1 céntimo.
- Cambio del Amplification Parameter de Curve por gobernanza on-chain, alterando súbitamente el slippage calculado de la pool.

## 10. Validaciones obligatorias
- PRE: Validar contra el "Systemic Risk Circuit Breaker". Si el spread cruzó el 1.5% o 2%, detener toda compra automática por riesgo de insolvencia de la stablecoin.
- CÁLCULO: Para DEX, el iterador de Newton para resolver la ecuación de Curve debe lograr precisión `1e-18` o fallar.
- POST: Monitorizar la liquidez del CEX para retiro. Comprar USDT barato con USD no sirve si el CEX tiene los retiros en ERC20 pausados.

## 11. Criterios de aprobación
- Existe ineficiencia matemática neta > Fee Total.
- El spread está dentro de la banda histórica natural "Safe De-Peg" (0.01% - 0.4%).
- Se usa capital escalado, dado el bajo impacto de mercado (Slippage casi nulo).

## 12. Criterios de rechazo
- El spread cruza la banda de riesgo sistémico (ej. "Tether a 0.96 USD. Do not catch falling knife").
- Falsa liquidez en un par obsoleto (Ej. BUSD) que no se puede arbitrar en el mundo real ni quemar on-chain.

## 13. Riesgos que mitiga
- Riesgo Estructural (Terra Luna Event): Bloquea inteligentemente la recolección de stablecoins fallidas previniendo la aniquilación del portafolio.
- Riesgo de Slippage Invisible: Al entender la fórmula de Curve, evita mandar una transacción masiva esperando un precio lineal que termina sufriendo slippage asintótico.

## 14. Integración con otras skills
- Alimentado por Flash Loans (Skill 28) dado que requiere capital colosal para rendir.
- Depende drásticamente del Circuit Breaker Financiero (Skill 44).
- Aporta rentabilidad ultra-estable a los Dashboards de Riesgo (Skill 84).

## 15. Modelo de datos sugerido
```json
{
  "StablecoinArbitrage": {
    "pair": "USDC-USDT",
    "venue_a": "curve_3pool",
    "venue_b": "binance_spot",
    "depeg_magnitude_bps": 25,
    "volume_usd": 500000.0,
    "projected_profit_usd": 125.0,
    "systemic_panic_mode": false
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en bucle con resolución milimétrica leyendo pools tipo Curve/UniV3 y los pares fiat/stable de Kraken/Coinbase.

## 17. Logs obligatorios
- `[INFO] Stable Arb: USDC/USDT gap detected (0.9990). Executing $50k flash swap via Uniswap V3. Net Profit: $35.`
- `[CRITICAL] DAI/USDC depeg exceeded 150 bps! Halting all stable-arb operations. Possible systemic shock.`

## 18. Métricas obligatorias
- `stablecoin_spread_gauge`
- `stable_arb_win_rate_pct` (Debe ser > 99%, las estables son altísimamente predecibles bajo parámetros normales).
- `curve_pool_imbalance_ratio`

## 19. Tests unitarios
- Solución de invariante Curve: Alimentar la fórmula con `x_balances`, `A`, calcular `D`, e inyectar delta `dx`. El `dy` resultante debe ser idéntico al del Smart Contract de Curve auditado.
- De-Peg limits: Testear que un de-peg de 0.2% pasa, pero uno de 2.5% dispara `Systemic Risk Alert` bloqueando todo.

## 20. Tests de integración
- Integración en Testnet Ethereum llamando a un Fork de la 3pool, con préstamos flash de Aave para realizar arbitraje por 1 millón de USDC.

## 21. Tests E2E
- El agente lee datos históricos del de-peg de SVB (Marzo 2023), y el módulo debe demostrar que bloquea el sistema en lugar de perder todo el capital persiguiendo USDC.

## 22. Checklist de producción
- [ ] Incorporación en código del iterador de Newton (`get_y` y `get_D` de Vyper pasados a Rust/JS con precisión `BigInt`).
- [ ] Separación semántica en configuración: "Safe Stables" (USDC, USDT) vs "Algo Stables" (FRAX) vs "CDP Stables" (DAI).
- [ ] Fee tracker ultra-preciso (Cuidado con los pares de CEX donde el exchange quita/pone comisiones dinámicamente).

## 23. Ejemplo de configuración no hardcodeada
```yaml
stablecoin_engine:
  safe_pegs: ["USDT", "USDC"]
  max_tolerable_depeg_bps: 80         # 0.8% drop is the panic limit
  curve_max_iterations: 255           # Newton solver limit
  min_notional_usd: 10000             # Stable arb only makes sense with high volume
```

## 24. Ejemplo de pseudocódigo
```python
def evaluate_stable_opportunity(pool_price, cex_price, config):
    # Stablecoin logic
    spread = abs(pool_price - cex_price)
    deviation_from_peg = abs(1.0 - pool_price)
    
    # Circuit Breaker Check
    if deviation_from_peg > config.panic_limit:
        log.error("SYSTEMIC SHOCK DETECTED. DO NOT BUY.")
        return False, 0
        
    # Standard arbitrage validation
    if spread > CONFIG.min_stable_spread_bps:
        profit = calculate_heavy_volume_arb(pool_price, cex_price)
        if profit > config.gas_cost:
             return True, profit
             
    return False, 0
```

## 25. Criterio final de excelencia
El motor extrae consistentemente rentabilidad garantizada y monótona de ineficiencias de $0.0005, apoyándose en pools masivos de Curve, pero reacciona instantáneamente bloqueando la billetera cuando detecta una ruptura estructural en el mercado crypto fiat, protegiendo el 100% del portafolio.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Pérdida del Peg estructural de Tether (Bancarrota sistémica no predecible por precio).
- Dependencias: Circuit Breakers, AMM Mathematics.
- Próxima skill: Arbitraje por funding rate (Skill 16).
