# SKILL 038 — Reconciliación de balances (Accounting)

## 1. Propósito superior
Asegurar que el sistema sabe matemáticamente y sin lugar a duda cuánto capital real posee en todas las cuentas, direcciones on-chain, y protocolos en un milisegundo dado. Previene el "Inventario Fantasma" (creer que tienes dinero cuando en realidad está bloqueado o perdido en fees) y garantiza el control contable cruzando los balances locales calculados contra la realidad dictaminada por la blockchain o los exchanges.

## 2. Nivel de conocimiento requerido
Experto en Contabilidad Financiera Distribuida (Double-Entry Ledger Architecture), Atomicidad Transaccional, API Account Management (CEX/DEX) y Web3 State Verification. Dominio de conceptos de `Free Balance`, `Locked Balance`, `Unsettled Funds` y `Collateral Utilization`.

## 3. Capacidades principales
1. Ingesta Multi-Fuente del inventario: Websockets Privados (CEX `executionReport` o `outboundAccountPosition`), Multicalls masivos on-chain (`balanceOf`), y endpoints REST genéricos.
2. Contabilidad de Doble Entrada Estricta: Cada trade ejecutado descuenta `X` de la cuenta de Origen, suma `Y` a la de Destino, y carga el Fee a una cuenta contable separada ("Expenses/Fees").
3. Separación de Saldos (Free vs Locked): Identificar qué parte del saldo de USDT está "Libre" y qué parte está "Bloqueada" por órdenes Limit pendientes de ejecución o márgenes futuros.
4. Auto-Reconciliación (True-Up): Si el bot calculó que ganó +0.5 USDT en un arbitraje, pero el exchange reporta el saldo con +0.48 USDT (probablemente un fee invisible o un rounding error), el sistema detecta la Anomalía de Contabilidad en segundos y corrige (Ajuste de Saldo) a favor del exchange, mandando alerta de "Matemática Defectuosa".
5. Detección de Fondeos Externos: Si un inversor envía 1 Millón de USDC a la wallet on-chain, el Reconciliador detecta el Evento L1, lo acredita a la bóveda operativa y despierta al módulo de Capital Allocation (Aumento automático de tamaño de Trade).
6. Prevención de Dust buildup: Consolida los polvos sueltos (Cantidades < Min Notional) a nivel contable para que el motor no intente arbitrar sobre 0.00000001 BTC.
7. Valoración Consolidada en Moneda Base (MtM - Mark to Market): Traduce todas las posiciones (ETH, SHIB, LINK, USD) a un portafolio unificado en USD usando oráculos de precios en tiempo real para reportar el Valor Liquidativo Total (Net Asset Value - NAV).
8. Detección de Retiros Suspendidos (Delisted/Maintenance Wallets): Mapear el `withdrawEnable` status del CEX para no contar como "Liquidez Arbitrable" un saldo que no se puede mover cross-chain.
9. Contabilidad de Transacciones en Vuelo (Pending/Unconfirmed): Predecir la caída del saldo mientras una TX de Ethereum está esperando ser minada en el Mempool (Evitar doble gasto de la misma moneda en 2 transacciones concurrentes).
10. Rastreo PnL Permanente (Profit and Loss Tracker): Proporcionar respuesta instantánea de "Cuánto dinero hemos ganado/perdido hoy / esta hora / en este trade".

## 4. Entradas requeridas
- `account_snapshots`: Saldos crudos provistos por APIs REST u On-Chain.
- `account_updates`: Deltas provistos por Websockets (Push).
- `internal_execution_receipts`: Acuses de recibo atómicos del Orquestador (Skill 36) dictaminando lo que "Creemos" que acaba de pasar.
- `asset_prices`: (Para el MtM en USD).

## 5. Salidas esperadas
- `unified_ledger`: Diccionario O(1) in-memory con los saldos exactos Listos, Bloqueados y En_Vuelo.
- `net_asset_value_usd`: Métrica global financiera.
- `reconciliation_alerts`: Diferenciales contables (Drift).

## 6. Reglas inmutables
- El motor de Arbitraje (Skill 1, Skill 12, etc.) JAMÁS lee su saldo haciendo una petición HTTP/WSS. Siempre consulta asincrónicamente el `Unified Ledger` (RAM Local). El Reconciliador es el único que mantiene actualizado ese Ledger.
- Si la anomalía de saldos (Local vs Exchange) excede un límite de seguridad estricto (e.g. `$10 USD`), se dispara un `PANIC_HALT_TRADING`. Significa que una de las fórmulas de Fee o Slippage del bot tiene un fallo crítico y está perdiendo fondos silenciosamente.
- Toda deducción por Fees, Gas, Préstamos y Spread se registra aisladamente. El sistema debe responder ¿Por qué perdimos $1 en este trade? desglosado matemáticamente.

## 7. Algoritmos o métodos que debe conocer
- Arquitectura Event Sourcing & CQRS (Command Query Responsibility Segregation) para reconstrucción del Ledger desde 0 en caso de crash de RAM.
- Optimistic Concurrency Control (OCC) en memoria para lock de fondos multi-hilo.
- Precisión Decimal EVM compatible (BigInt y Fixed-Point mathematics).

## 8. Fórmulas críticas
- **Cálculo de Fondos Operables**: `Usable_Amount = Total_Balance - Locked_Balance - In_Flight_Tx_Balance`
- **Tolerancia Reconciliatoria**: `|Balance_API - Balance_Calculated_Local| < Max_Acceptable_Dust_Drift`
- **Total NAV (USD)**: `Sum(Asset_i_Usable * Price_i) + Sum(Asset_i_Locked * Price_i)`

## 9. Casos extremos
- Interrupción de Socket CEX Privado: El Websocket de usuario de Binance se cae en medio de un trade de altísima frecuencia. Binance ejecutó 10 órdenes. El Bot en RAM no supo nada, su saldo está "congelado" artificialmente. Debe detectar la caída de Socket (Skill 31), pausar operativa con ese saldo, pedir el Snapshot REST (Skill 35 Bypass), resincronizar, y continuar.
- Gas Fee no contable: Una transacción de ETH falla por OutOfGas, consumiendo $15 dólares de la wallet pero sin generar ningún recibo de arbitraje positivo. El bot debe contabilizar y conciliar esa pérdida y descontar el balance de ETH nativo.
- "Airdrops" sorpresivos o Rebase Tokens: Saldo de cuenta aumenta mágicamente (Airdrop) o disminuye mágicamente (Elastic Supply Tokens, Rebase). El módulo acata la realidad Blockchain sobre su cálculo estricto local sin entrar en bucle de pánico si el token tiene esa cualidad.

## 10. Validaciones obligatorias
- PRE: Chequear Locks. Si el saldo de USDC es 1000, y una rutina asíncrona inicia un trade por 800 USDC, se adquiere un Lock (Pending) para que otro hilo no vea los 1000 disponibles.
- CÁLCULO: Validar la conversión de String (CEX Payload `"0.0050"`) a Float/BigInt con exactitud extrema.
- POST: Cada 60 minutos, ejecutar "True-Up" silencioso forzando snapshot masivo de On-Chain (Multicall) y CEX para confirmar que el contador local sigue alineado a la perfección.

## 11. Criterios de aprobación
- La lectura del saldo `Free` es instantánea en microsegundos y está siempre respaldada por eventos reales.
- El Net PnL calculado coincide exactamente con la diferencia histórica de fondeos vs balance actual.

## 12. Criterios de rechazo
- El sistema detecta una divergencia de Saldo (Balance Drift) no explicable que supera los umbrales operativos de seguridad (Posible API hack, robo externo o matemática rota). Activa el Kill-Switch General.
- Inconsistencia de Token (La API del CEX da saldos de `LUNA` pero el on-chain es `LUNC`).

## 13. Riesgos que mitiga
- Riesgo de Invalidación Dinámica ("Insufficient Funds Error"): Si el bot no descuenta su saldo internamente antes de emitir un trade, enviará 10 peticiones asíncronas para gastar los mismos 100 dólares, las últimas 9 rebotarán con error, llenando logs, quemando API limits y estropeando métricas.
- Pérdida Silenciosa por Fees: Muchos bots fracasan porque calculan un profit falso sin registrar el fee de Taker real cobrado por el CEX, creyendo ser rentables cuando están secando la cuenta gota a gota.

## 14. Integración con otras skills
- Proporciona el límite duro y estricto de la Optimización de Tamaño de Trade (Skill 2) mediante la métrica `Usable_Amount`.
- Consume lecturas de Websockets CEX (Skill 31) y de Lectura On-Chain Multicall (Skill 21/23).

## 15. Modelo de datos sugerido
```json
{
  "UnifiedLedgerAsset": {
    "venue": "BINANCE",
    "asset": "USDT",
    "total": 54200.50,
    "free": 40200.50,
    "locked": 10000.00,
    "in_flight": 4000.00,
    "usd_value": 54200.50,
    "last_reconciled_timestamp_ms": 1714521234105,
    "drift_status": "ALIGNED"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase Global de Contabilidad In-Memory. Maneja funciones críticas: `lockFunds(venue, asset, amount)`, `commitTrade(receipt)`, `releaseFunds(venue, asset, amount)`.

## 17. Logs obligatorios
- `[DEBUG] Funds Locked: 5000 USDC on Arbitrum. In-flight flag set. Free Balance remaining: 1500 USDC.`
- `[INFO] Reconciled. Binance WSS User Data matches Local Ledger for BTC. True-up diff: 0.00000000.`
- `[CRITICAL] Accounting DRIFT Exception! Local expected 10 ETH, Alchemy RPC reports 9.5 ETH. HALTING ALL TRADING to prevent mathematical ruin.`

## 18. Métricas obligatorias
- `total_nav_usd_realtime`.
- `accounting_drift_usd_amount`.
- `asset_utilization_pct` (Qué porcentaje del dinero está parado en "Free" sin trabajar, indicando ineficiencia algorítmica).
- `locked_funds_timeout_count` (Si los fondos quedan en "in-flight" mucho tiempo y hay que forzar su liberación).

## 19. Tests unitarios
- Event Sourcing: Simular un saldo inicial de 0. Pasar 3 eventos: Depósito (100), Trade Buy (-50), Trade Fee (-1). Comprobar que Total=49 y Free=49.
- Optimistic Lock Release: Hilo A llama `lockFunds(20)`. Hilo B falla al pedir `lockFunds(40)` (Saldo 49). El Hilo A termina con error de red y llama a `releaseFunds(20)`. Hilo B reintenta y adquiere el lock exitosamente.
- Auto-Reconciliation Math: Simular `Local_Balance = 100.50` y Forzar `API_Snapshot = 100.48`. La función debe insertar un movimiento contable en negativo de `0.02` clasificado como "Reconciliation Ajustment" sin crashear el bot (Márgen de polvo).

## 20. Tests de integración
- Levantar servidor CEX Mock que emita websockets de actualización de cuenta. El Agente enviará órdenes que restan saldo; el Mock confirmará su estado, y el Reconciliador empatará las firmas en microsegundos validando el ciclo PnL de extremo a extremo.

## 21. Tests E2E
- El bot lee un spread, deduce que la ruta usará 5 CEXes y 2 Redes On-Chain simultáneas. Antes de disparar las patas de arbitraje, la Skill 38 realiza 7 Locks Atómicos en la memoria local, bloqueando toda la liquidez, asegurando que si otra oportunidad aparece en el siguiente milisegundo, la cuenta no sea sobregirada ("Overdraft"). Ejecuta los trades, y al final, los WS del exchange devuelven los tickets, desbloqueando el capital y asentando el beneficio exacto.

## 22. Checklist de producción
- [ ] Incorporación de Listeners exclusivos a los Streams de Datos de Usuario (Binance: `listenKey`, OKX: `Private Channels`) manejando la criptografía segura de login WSS.
- [ ] Implementación del "In-Flight Auto-Unlock Timeout". Si un Trade se cae y la orden asíncrona "muere" en silencio, un temporizador global debe liberar los fondos reservados (`locked`) tras 60 segundos de inactividad, evitando congelar el bot para siempre por un Lock huerfano.
- [ ] Filtro implacable contra activos "Scam/Airdrops" (Tokens basura que se depositan solos en billeteras de Ethereum). Si no están en la Whitelist contable, se ignoran y no se suman al `Total NAV USD`.

## 23. Ejemplo de configuración no hardcodeada
```yaml
accounting_engine:
  max_acceptable_drift_usd: 5.0  # Drift beyond this causes panic halt
  in_flight_timeout_ms: 10000    # Release internal locks if no confirmation arrives
  true_up_reconciliation_interval_seconds: 600
  ignore_dust_values_usd: 0.1
```

## 24. Ejemplo de pseudocódigo
```javascript
class UnifiedLedger {
    constructor() {
        this.balances = new Map(); // venue_asset -> UnifiedLedgerAsset
        this.locks = new Map(); // UUID -> LockInfo
    }

    reserveFunds(venue, asset, amount, tradeId) {
        const key = `${venue}_${asset}`;
        const account = this.balances.get(key);
        
        if (!account || account.free < amount) {
            return false; // Insufficient internal funds
        }
        
        // Optimistic locking (in a single thread or using Atomics in multithreading)
        account.free -= amount;
        account.in_flight += amount;
        
        this.locks.set(tradeId, { venue, asset, amount, timestamp: Date.now() });
        return true;
    }

    commitTrade(tradeId, actualDeltas) {
        const lock = this.locks.get(tradeId);
        if (!lock) return;

        // Apply true deltas (usually containing exact fees deducted)
        for (let delta of actualDeltas) {
             const key = `${delta.venue}_${delta.asset}`;
             let account = this.balances.get(key) || createEmptyAccount();
             
             account.total += delta.amount; // Pos or Neg
             account.free += delta.amount;
        }

        // Release the exact lock amounts we held speculatively
        const origAccount = this.balances.get(`${lock.venue}_${lock.asset}`);
        origAccount.in_flight -= lock.amount;
        // Notice we don't add back to 'free' directly here, the actualDeltas already adjusted total/free accurately.

        this.locks.delete(tradeId);
    }
    
    // Asynchronous listener to WebSockets
    onUserDataUpdate(venue, asset, absoluteBalanceFromExchange) {
         const key = `${venue}_${asset}`;
         const account = this.balances.get(key);
         const expectedTotal = account.total;
         
         if (Math.abs(expectedTotal - absoluteBalanceFromExchange) > CONFIG.drift_tolerance) {
              log.error(`Drift detected in ${key}. Fixing locally to match reality.`);
         }
         
         // Hard sync (True Up)
         account.total = absoluteBalanceFromExchange;
         // Recalculate free based on our internal locks
         account.free = account.total - account.locked - account.in_flight;
    }
}
```

## 25. Criterio final de excelencia
El reconciliador de balances convierte al bot en un Auditor Financiero a Nivel Máquina. Rastrea cada céntimo que se mueve por el éter en microsegundos, asegurando el 100% de la integridad de los fondos y previniendo que un bug algorítmico agote el capital asumiendo saldos falsos creados por latencia de red.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: APIs Privadas que envían Snapshots desactualizados (CEX Bug). El bot empataría con una "Realidad Falsa" del CEX y descartaría su propia historia correcta.
- Dependencias: API de CEX/DEX Account, Websockets de Usuario.
- Próxima skill: Fee tracker & maker/taker analytics (Skill 39).
