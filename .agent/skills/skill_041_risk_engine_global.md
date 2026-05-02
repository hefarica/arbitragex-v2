# SKILL 041 — Risk engine global (Circuit breakers)

## 1. Propósito superior
Proteger la integridad del fondo como un firewall de última instancia frente a Black Swans (Cisnes Negros), Flash Crashes, hacks de red, o fallos en el código del propio bot. Este motor monitoriza métricas macro de exposición, caída de balance (Drawdown) y anomalías estadísticas de mercado, disparando "Circuit Breakers" (Interruptores de Emergencia) que pausan o detienen en seco toda actividad comercial antes de que la ruina algorítmica pueda consumarse en milisegundos.

## 2. Nivel de conocimiento requerido
Gestor de Riesgos Cuantitativo Institucional (CRO). Entendimiento profundo de métricas de Drawdown, Value at Risk (VaR), Volatilidad Histórica Estocástica, Toxic Order Flow, correlación de activos, y diseño de sistemas "Fail-Safe" y "Dead Man's Switch" (Sistemas que se detienen solos si el controlador deja de latir).

## 3. Capacidades principales
1. Maximum Daily Drawdown Limit: Detener todo el trading si el Net Asset Value (Skill 38/40) cae por debajo del 1% o 2% de su marca inicial del día. Previene el desangre por bugs no detectados (Death by a thousand cuts).
2. Volatility Halt (Circuit Breaker): Pausar operaciones de un par específico si su precio sube o baja >15% en 5 minutos. En volatilidades absurdas, los spreads falsos y la asfixia de liquidez provocan pérdidas severas en slippage, mejor apagar motores y esperar calma.
3. API Exploit / Unauthorized Withdraw Halt: Si el sistema de contabilidad registra un retiro de fondos (Withdrawal) que NO fue orquestado por el bot de rebalanceo, asumir que la API Key fue comprometida, paralizar operaciones y alertar vía SMS/Llamada de emergencia al administrador.
4. Consecutive Loss Limit: Si 5 trades de arbitraje consecutivos resultan en PnL neto negativo (algo que estadísticamente en este bot no debería pasar nunca), el sistema asume que la fórmula de simulación está rota y se auto-suspende.
5. Exposure Cap Enforcement: Bloquear el Optimizador de Tamaño (Skill 2) si intenta lanzar un trade que represente más del X% del capital total del fondo, evitando poner "Todos los huevos en la misma canasta atómica".
6. Toxic Orderbook Detection: Si el libro de órdenes muestra una caída de liquidez bid-side del 90% en menos de 1 minuto (Rug Pull preparation), desactivar arbitraje de ese activo.
7. RPC / Exchange Ban Cascade: Si 3 llamadas API consecutivas retornan `403 Forbidden` o `IP Ban`, activar "Stealth Mode" (Halt de 1 hora) para evitar un Ban permanente o congelamiento de cuenta por el departamento de Compliance del Exchange.
8. Time-To-Live (TTL) Kill Switch: Un contrato Proxy en L1 tiene una función de "Pausa" o el Bot tiene un Flag global. Si un trade tarda demasiado en completarse (Stuck Tx), no se emiten nuevos trades hasta aclarar el estado del bloque.
9. Oráculo de Correlación Rota: Detectar si el precio de BTC y WBTC divergen bruscamente más del 2%. Indica vulnerabilidad o hack en un puente cross-chain. Paralizar arbitraje envolvente.
10. Dead Man's Switch: El Orquestador manda latidos (Pings) al Risk Engine cada segundo. Si el Risk Engine no recibe latido en 5 segundos, dispara una transacción de "Cierre Total" liquidando inventario asimétrico y apagando el bot desde otro hilo o proceso (Watchdog Timer).

## 4. Entradas requeridas
- `realtime_pnl`: (Skill 38) Para evaluar Drawdown.
- `trade_receipts`: Para evaluar tasa de victorias (Win/Loss ratio) temporal.
- `market_volatility_metrics`: Desviaciones estándar desde el módulo de datos.
- `watchdog_pings`: Pulsos de vida del proceso principal.

## 5. Salidas esperadas
- `risk_state`: `GREEN` (Normal), `YELLOW` (Restringido), `RED` (Halt absoluto).
- `kill_switch_command`: Alerta dura que mata subprocesos inmediatamente.
- `portfolio_health_report`: Informe emitido por Telegram/Slack.

## 6. Reglas inmutables
- El motor de riesgo está POR ENCIMA del orquestador y de la inteligencia matemática. Si el Algoritmo jura haber hallado un arbitraje de 1 millón de dólares, pero el Risk Engine dice `HALT_EXPOSURE_LIMIT`, la orden se bloquea.
- Un Circuit Breaker "RED" requiere de intervención HUMANA MANUAL para ser reseteado. El bot jamás debe reiniciar operaciones tras un Hard Halt sin que un desarrollador revise por qué el bot perdió la cabeza o detectó un cisne negro.
- Nunca calcular VaR o Volatilidad en el Hilo Principal, externalizar estos cálculos pesados al Data Lake o a Workers aislados, pero evaluar el umbral booleano `< MAX_LIMIT` de manera O(1) en el pipeline de Trade.

## 7. Algoritmos o métodos que debe conocer
- EWMA (Exponential Weighted Moving Average) para Volatilidad.
- Z-Score dinámico de series temporales de PnL.
- Token Bucket para Conteo de Pérdidas Consecutivas.
- Patrones Heartbeat / Watchdog OS Level (systemd / pm2 integrations).

## 8. Fórmulas críticas
- **Daily Drawdown**: `DD_Pct = (Current_NAV - Day_Open_NAV) / Day_Open_NAV`
- **Consecutive Loss Stop**: `if (Losses_Last_N_Trades >= MAX_TOLERANCE) { HALT(); }`
- **Volatilidad Local (5m)**: `StdDev(Prices_last_5m) / Mean(Prices) > MAX_VOLATILITY_PCT`

## 9. Casos extremos
- Flash Crash Generalizado de 50% (Ej. Marzo 2020 COVID Drop): Los diferenciales de precios entre CEXes se vuelven colosales (+5%). El bot lanza órdenes, pero el matching engine de Binance se cae y el de Kraken funciona lento. El arbitraje se "Rompe a la mitad" (Leg risk), comprando caro sin poder vender barato. El Volatility Halt salva al fondo desconectándolo segundos antes de la asfixia de la red.
- Bug Cuantitativo (Negative Slippage Bug): Una actualización de código invierte un signo por error en la Skill 24. El bot empieza a calcular profits masivos en cada trade, pero en realidad pierde $0.5 por trade. A 100 trades por segundo, quema $50/sec. El Drawdown Halt corta el bot a los $1000 de pérdida, previniendo perder la cuenta de $1M entera en minutos.
- Ataque de Phishing/Compromiso de API: El bot detecta una IP foránea retirando USDT. Corta las transacciones, transfiere fondos a "Cold Wallet" configurada de emergencia o lanza llamadas a la API de `Disable Withdrawals` si el CEX lo permite.

## 10. Validaciones obligatorias
- PRE: Todo envío de payload de orden requiere validación del estado `isRiskGreen()`.
- CÁLCULO: Mantener de forma perenne la marca de agua alta (High Watermark) del día. El Drawdown se calcula desde el pico del día, no desde la apertura, para proteger ganancias flotantes consolidadas.
- POST: Registrar cada alerta amarilla o roja en una bitácora forense de inmutabilidad (Skill 37).

## 11. Criterios de aprobación
- La evaluación de riesgos (`checkRiskLimits(tradeObj)`) toma < 0.05ms para no entorpecer el flujo HFT.
- En caso de simulación de pánico, el sistema anula todos los websockets y apaga el pool de workers en < 10ms.

## 12. Criterios de rechazo
- El sistema no logra aislar o ignorar alertas temporales y detiene el bot "Para siempre" ante fluctuaciones estándar, causando el Síndrome del Bot Asustadizo (Perdida de oportunidad inmensa).
- El sistema confía en APIs externas lentas para verificar su propio estado. (El Risk debe nutrirse estrictamente del entorno local ya validado).

## 13. Riesgos que mitiga
- Riesgo Estructural y de Ruina por Fat-Finger (Código Roto): La mayor parte de los fondos Cuantitativos en la vida real quiebran por un bug en producción liberado un viernes, no por "movimientos normales del mercado" (Ej. Caso Knight Capital perdiendo $440M en 45 minutos).
- Riesgo Sistémico (Cisnes Negros): Eventos no correlacionados que rompen la lógica matemática normal (Despegs masivos tipo LUNA/UST).

## 14. Integración con otras skills
- Es la cerradura que gobierna al Orquestador (Skill 36).
- Lee datos contables y de PnL provenientes de Skill 38.

## 15. Modelo de datos sugerido
```json
{
  "RiskEngineState": {
    "status": "GREEN",
    "daily_drawdown_pct": -0.15,
    "consecutive_losses": 0,
    "last_trade_pnl": 5.4,
    "volatility_flags": {
      "BTC_USDT": false,
      "ETH_USDT": false,
      "LUNA_USDT": true
    },
    "watchdog_health": "ALIVE",
    "kill_switch_engaged": false
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Background como Watchdog Process (preferiblemente un proceso de SO diferente al proceso de Node/Rust, monitorizándolo vía RPC).
- Interceptor Sincrónico In-Memory (`checkPreTradeRisk()`) inyectado en el pipeline del Orquestador.

## 17. Logs obligatorios
- `[DEBUG] Pre-Trade Risk Check PASSED. Drawdown: -0.1%. Exposure: 2%.`
- `[WARN] High Volatility Detected on LUNA_USDT (+20% in 1m). Placing pair on YELLOW cooldown (No trading for 5 mins).`
- `[CRITICAL] MAX DRAWDOWN LIMIT BREACHED (-1.5%). PANIC HALT INITIATED. ALL WORKERS KILLED. NOTIFYING ADMINS.`

## 18. Métricas obligatorias
- `risk_engine_cpu_overhead_ms` (Debe ser microscópico).
- `circuit_breaker_trips_monthly`.
- `consecutive_losses_counter_realtime`.

## 19. Tests unitarios
- Drawdown Kill: Inyectar datos donde el saldo pasa de 100,000 a 98,500 (1.5% drop). El config permite 1.0%. La función `evaluateDrawdown` debe retornar INMEDIATAMENTE `HALT`.
- Consecutive Loss Tracker: Simular 4 trades en -$0.10 y 1 trade en +$0.50. El contador debe resetear a 0. Luego simular 5 trades en -$0.10. El contador debe retornar `HALT`.
- Exposure Cap Constraint: Capital 1 Millón. Optimizador pide invertir 500k (50% exposición). Config dice Max Exposición = 10%. El Risk Engine debe truncar el trade a 100k y retornar advertencia o rechazarlo totalmente.

## 20. Tests de integración
- Levantar un nodo Watchdog auxiliar. El Orquestador simula un ciclo infinito (Infinite Loop, CPU 100% frozen) en Node.js. El Watchdog nota la ausencia del latido de 1 segundo (Ping OS). A los 3 segundos, el Watchdog hace un `kill -9` del PID del Orquestador y llama al script `/emergency/cancel_all_orders.sh`.

## 21. Tests E2E
- Entorno: Mainnet Forcada. Se inyecta un bug a propósito que invierte las lógicas de compra y venta en el bot de prueba. El bot empieza a mandar transacciones perdedoras a toda velocidad. Al alcanzar la transacción perdedora #5 o acumular $500 de pérdida virtual, el Risk Engine levanta el Firewall, sella los WebSockets, cancela órdenes Limit en curso y destruye la ejecución en menos de 50 milisegundos aislando el bug al 100%.

## 22. Checklist de producción
- [ ] Incorporación de Alertas Out-of-Band: Enviar un PagerDuty, llamada automatizada de Twilio, o SMS. Telegram no sirve porque a las 4 AM en un cisne negro no vas a despertar con un mensaje de Telegram.
- [ ] Implementar un Botón de Pánico Hardwired (Un endpoint HTTP oculto `/panic` al que el admin puede pegarle con un curl si ve el mercado colapsando en la TV y no quiere esperar al bot).
- [ ] Configurar el "Auto-Cancel on Disconnect" en Binance/OKX (Kill Switch Exchange Level): Si el bot de Node.js crashea, Binance cancelará todas las órdenes automáticamente tras 10 segundos de perder la conexión del WebSocket de usuario.

## 23. Ejemplo de configuración no hardcodeada
```yaml
risk_engine:
  max_daily_drawdown_pct: 1.0
  max_consecutive_losses: 5
  max_single_trade_exposure_pct: 5.0
  high_volatility_halt_threshold_pct: 10.0
  watchdog_ping_timeout_ms: 3000
  enable_twilio_alerts: true
```

## 24. Ejemplo de pseudocódigo
```javascript
class RiskEngine {
    constructor(config) {
        this.maxDrawdown = config.max_daily_drawdown_pct;
        this.dailyHighWatermarkUsd = 0;
        this.currentNavUsd = 0;
        this.consecutiveLosses = 0;
        this.isHalted = false;
    }

    updatePnL(latestNav) {
        if (latestNav > this.dailyHighWatermarkUsd) {
             this.dailyHighWatermarkUsd = latestNav;
        }
        this.currentNavUsd = latestNav;
        
        const drawdownPct = ((this.dailyHighWatermarkUsd - this.currentNavUsd) / this.dailyHighWatermarkUsd) * 100;
        
        if (drawdownPct > this.maxDrawdown) {
             this.initiatePanicHalt(`Drawdown breached: ${drawdownPct.toFixed(2)}%`);
        }
    }

    registerTradeResult(pnlNetUsd) {
        if (pnlNetUsd < 0) {
            this.consecutiveLosses++;
            if (this.consecutiveLosses >= CONFIG.max_consecutive_losses) {
                 this.initiatePanicHalt(`Consecutive losses breached: ${this.consecutiveLosses}`);
            }
        } else {
            this.consecutiveLosses = 0; // Reset
        }
    }

    // Called synchronously before EVERY trade
    checkPreTradeRisk(tradeIntent) {
        if (this.isHalted) throw new Error("RISK_HALT");
        
        const exposurePct = (tradeIntent.notionalUsd / this.currentNavUsd) * 100;
        if (exposurePct > CONFIG.max_single_trade_exposure_pct) {
             throw new Error("EXPOSURE_LIMIT_REACHED");
        }
        
        if (MarketMonitor.isHighlyVolatile(tradeIntent.pair)) {
             throw new Error("VOLATILITY_COOLDOWN_ACTIVE");
        }
    }

    initiatePanicHalt(reason) {
        this.isHalted = true;
        log.critical(`PANIC HALT INITIATED: ${reason}`);
        EventBus.emit('GLOBAL_KILL_SWITCH');
        AlertSystem.callTwilioAdmin(reason);
    }
}
```

## 25. Criterio final de excelencia
El Risk Engine es el Cinturón de Seguridad, los Airbags y los Frenos ABS integrados en uno. Su perfección radica en que el 99.9% del tiempo opera de forma invisible (Cero Falsos Positivos) sin causar latencia de cómputo, pero está absolutamente garantizado de cortar el sistema entero en microsegundos si el bot pisa fuera de la línea de supervivencia matemática.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Cortes de energía o kernel panics del servidor (Solucionado usando el Watchdog Dead-Man Switch del Exchange API o Smart Contract).
- Dependencias: Integración con Accounting y Orquestador.
- Próxima skill: Auto-rebalanceo cross-chain / cross-exchange (Skill 42).
