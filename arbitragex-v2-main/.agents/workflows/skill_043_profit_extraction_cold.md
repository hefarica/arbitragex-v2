# SKILL 043 — Profit extraction & cold storage

## 1. Propósito superior
Aislar mecánicamente y retirar (Sweep) los beneficios acumulados generados por el HFT hacia "Silos Inexpugnables" (Cold Wallets / Hardware Wallets / Multisig Vaults) de manera periódica. Protege el Patrimonio Neto (Profits) contra ataques de API, hackeos masivos de CEX (Ej. Colapso de FTX) o bugs catastróficos del propio código. Ejecuta el principio de "Paga y Guarda": lo ganado no se arriesga ciegamente de vuelta en el interés compuesto automatizado, sino que se blinda hasta su inyección manual y auditoría.

## 2. Nivel de conocimiento requerido
Especialista en Custodia Institucional, Arquitectura de Smart Contracts (Multisig Safe, Timelocks), Contabilidad Segregada (Watermarking & Drawdowns), y Procedimientos de Seguridad de Retiros API. Comprensión del manejo de llaves ECDSA offline y ruteo de "Monedas Nativas" de forma irrevocable hacia el almacenamiento en frío.

## 3. Capacidades principales
1. Tracking de Marca de Agua de Beneficio (High-Water Mark Tracking): Detectar si el bot ha generado $X de profit limpio y neto verificable desde el último ciclo de extracción.
2. Contabilidad Segregada ("Caja Fuerte"): El dinero designado para ser retirado (Swept funds) se etiqueta lógicamente en la memoria como "Untouchable" para que el Orquestador o el Rebalanceador (Skill 42) no lo utilicen accidentalmente para comprar BTC.
3. Consolidación a Stablecoins (Stablecoin conversion): Antes de retirar a Cold Storage, convertir las minúsculas ganancias retenidas en monedas raras (Ej. Ganar en PEPE, SHIB) hacia una base sólida de preservación de valor (USDC/USDT) minimizando el riesgo de exposición a criptos en la bóveda inactiva.
4. Auto-Sweep Batching (Extracción por Lotes): No gastar Gas de red extrayendo ganancias cada 10 dólares. Esperar a que el beneficio acumulado supere el "Umbral de Eficiencia de Gas" (Ej. Acumular $5000 y pagar $1 en retirar por Tron/Polygon/L2).
5. Interacción con Multisigs Institucionales (Gnosis Safe / Safe{Core}): Enviar el retiro on-chain hacia un contrato inteligente que requiera la firma biométrica de N/M socios humanos para ser movido, protegiendo a la empresa dueña del capital.
6. Protección Anti-Desangre de Retiros (Withdrawal Drain Protection): Asegurar que si el bot enloquece y la función de "Extracción" se vuelve un bucle infinito, el CEX no reciba mil peticiones de extracción vaciando el balance "Principal" operativo del fondo.
7. Informes de Cierre Fiduciarios: Emitir informes PDF/JSON semanales (Profit Ledger) al equipo contable de la firma mostrando los recibos en blockchain exactos (TxHashes) de todas las ganancias extraídas.
8. Fallback a Modos Redundantes de Extracción: Si la red de Ethereum Mainnet tiene Gas en 500 Gwei (carísimo), la extracción espera; si tarda más de 3 días, enruta a la dirección de Arbitrum L2 para no retrasar la seguridad de caja.
9. "Panic Sweep" (Extracción de Pánico): Ante un estado de alerta roja masivo por insolvencia en X exchange (Rumor fuerte de caída estilo Alameda), vaciar el 100% de la liquidez del Exchange (Principal + Profit) hacia la Cold Wallet instantáneamente.
10. Saneamiento de "Dust" (Restos): Evita arrastrar céntimos fraccionarios de stablecoins en los retiros que producen fallos de precisión por decimales on-chain.

## 4. Entradas requeridas
- `net_pnl_tracker`: Totalidad del PnL real desde Skills 38 y 39.
- `cold_storage_address_book`: Diccionario inmutable de las bóvedas seguras (`0xSafe...`).
- `gas_oracle`: Precios de gas actuales de redes L1/L2.

## 5. Salidas esperadas
- `profit_transfer_receipt`: Hashes y constancias de red de las extracciones exitosas.
- `locked_sweep_funds`: Flag local que disminuye el "Capital de Trabajo" activo.
- `accounting_reports`: Registros inmutables en la BD para fiscalidad (Skill 37).

## 6. Reglas inmutables
- Las direcciones del Cold Storage (Destination Addresses) DEBEN estar "Hardcodeadas" en una configuración génesis segura cargada desde Variables de Entorno encriptadas (AWS KMS / Hashicorp Vault) y jamás actualizables vía API o en Runtime para evitar ataques de Man-In-The-Middle donde se cambia la address del tesoro.
- La ejecución de una transferencia de Profit hacia el exterior debe deducir el Capital Base Interno del bot. Si el bot gana $1000 y se retiran, el bot no debe seguir asumiendo que tiene una base de AUM + $1000 para sus cálculos de Risk Drawdown (Skill 41). El High Watermark se reinicia proporcionalmente.
- NUNCA mandar fracciones de centavo como retiro a red principal de Ethereum (Gastar $10 de red para enviar $0.50). Requiere control riguroso de Batching.

## 7. Algoritmos o métodos que debe conocer
- Safe (anteriormente Gnosis Safe) Smart Contract architecture e interacciones proxy.
- Principios "Zero-Trust" en Criptografía.
- Cálculos impositivos marginales (FIFO/LIFO aplicados a extracción si es aplicable legalmente).

## 8. Fórmulas críticas
- **Cálculo de Fondos Extraíbles**: `Extractable_Profit = Current_NAV - Base_Capital - Target_Compounding_Reserve`
- **Condición de Lote (Batch Trigger)**: `if (Extractable_Profit > Minimum_Sweep_Threshold_USD && Gas_Fee < Extractable_Profit * Max_Sweep_Fee_Pct) { Sweep() }`

## 9. Casos extremos
- API Compromised + Sweep Exploit: Un hacker consigue robar el código fuente del bot e inyecta una dirección propia en la configuración del Cold Storage, luego detona un comando manual de "Panic Sweep". Regla de mitigación: Los CEX institucionales exigen que las Withdraw Addresses sean puestas en una Whitelist manual desde la web con 2FA e IP Lock; el hacker recibirá Revert desde el CEX.
- Atasco Temporal (Rebase/Yield Tokens): Beneficios cobrados en tokens que alteran su balance solitos como aTokens (Aave) o stETH (Lido). El bot intenta mandar un string de "Monto Exacto Fijo" `100.5123`, y para cuando la orden L1 se ejecuta, el balance es `100.5124`. Puede causar errores de polvo (Dust Error); se recomienda extraer "Balance Total" de ganancias, no "Cantidades absolutas".
- Exchange Insolvency Rumor: FTX 2.0. El bot lee (integración externa/noticias/API) un rumor crítico o el admin activa el `PANIC_SWEEP_MODE`. El código debe priorizar este barrido por encima de todas las tareas, vendiendo a mercado sin importar el slippage de -3% de todo el inventario de Spot a USDT y enviando los USDT por cualquier red L2 viva a la Safe externa.

## 10. Validaciones obligatorias
- PRE: Chequear las llaves Criptográficas Maestras del archivo seguro. Si las Address destino no pasan los Checksums `EIP-55`, cancelar e invalidar el retiro por completo.
- CÁLCULO: Reservar un % del Profit a favor del Compounding (Interés Compuesto). (Ej. El fondo ganó $10k. Enviar $8k al Cold Storage. Reservar $2k en las cuentas para aumentar geométricamente el poder de compra base).
- POST: Validar las direcciones de contrato de Stablecoins. Enviar USDT y USDC al mismo contrato proxy o dirección no-soportada puede sepultar y quemar el profit para siempre.

## 11. Criterios de aprobación
- Extracción autónoma transita libremente del Estado "Ganancia Flotante" a "Depósito Confirmado Cold Wallet".
- El Ledger Central (Skill 38) asimila la sustracción en sus activos Libres sin declarar "Inconsistencia" ni disparar Kill-Switches.

## 12. Criterios de rechazo
- Intento de enviar capital Extraíble hacia un address de Testnet desde un CEX de Mainnet, o cruces letales de formato de red.
- CEX reporta `Withdrawal Quota Exceeded` (Limites de $10M diarios superados) - el barrido es fraccionado para los días subsiguientes.

## 13. Riesgos que mitiga
- Riesgo Catastrófico de Custodia (Tercera Parte/CEX Hack): El arbitraje obliga al operador a dejar cientos de miles de dólares en Binance, KuCoin y Uniswap Contracts, bajo altísimo riesgo inherente. La "Barrer" ganancias asiduamente asegura que, en el peor de los casos, la firma solo pierde el "Capital de Trabajo Mínimo" pero el bot logró proteger meses y años de trabajo puro.
- Autocombustión Algorítmica: Un bug que destruye la cuenta. Si el bot retiraba cada semana sus profits, el daño del bug se limita a la semana actual.

## 14. Integración con otras skills
- Receptora del estado de riqueza final de Unified Ledger (Skill 38 / 40) y Profit metrics.
- Coordinación fina con Limitadores de Red (Rate Limits Skill 35) y Gas Oracles.

## 15. Modelo de datos sugerido
```json
{
  "ColdSweepJob": {
    "timestamp": 1714521234105,
    "amount_usd_swept": 15400.0,
    "asset_transferred": "USDC",
    "network_used": "arbitrum_one",
    "destination_cold_address": "0xSafeMultisig123...",
    "fee_paid_usd": 0.45,
    "compounding_amount_left_behind": 5000.0,
    "trigger_reason": "SCHEDULED_WEEKLY_SWEEP"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Cron-Job aislado (Ej. Sábados a las 02:00 AM UTC cuando la volatilidad y el gas caen) y un endpoint de Webhook protegido/autenticado internamente para detornar `POST /admin/panic-sweep`.

## 17. Logs obligatorios
- `[INFO] Profit Sweeper active. Extracting 10,000 USDC from Bybit to Gnosis Safe Mainnet. Preserving 5,000 USDC for algorithmic compounding.`
- `[DEBUG] Cold Sweep queued but Gas on Ethereum is 145 Gwei. Postponing until Gas < 30 Gwei for maximal profit preservation.`
- `[CRITICAL] PANIC SWEEP COMMAND RECEIVED. Liquidating all active portfolios and firing raw withdrawal txs across 5 exchanges bypassing optimization.`

## 18. Métricas obligatorias
- `total_all_time_profit_swept_to_cold_usd`.
- `time_since_last_sweep_hours`.
- `compounded_capital_left_in_cex_usd`.

## 19. Tests unitarios
- Compounding Ratio Calculation: Inyectar un NAV base de $100k y un NAV actual de $120k. Límite de Sweep: Retirar 80% ganancias. El script debe ordenar extraer exactamente `$16,000` y dejar la base en `$104,000`.
- Address Sanity Check: Intentar alimentar la configuración de Cold Storage con `0x123` (Corto / Inválido) y con una dirección L2 correcta. La función de inicialización debe lanzar error fatal en la mala y pasar en la buena, no permitiendo que el bot se encienda con destinos corruptos.
- Sweeping Threshold Optimization: Simular un beneficio de $50 y un Gas Cost de $15 (30% merma). El módulo debe saltarse la operación de barrido argumentando ineficiencia logística.

## 20. Tests de integración
- Proveer permisos Read-only a una API de Sandbox de CEX y emitir simulación de llamado de retiro (`withdraw()`). Validar en el historial de eventos del testnet que el payload viaja con el Memo y Tags (Ej. Ripple/Stellar requieren un código ID destino) bien estructurados para evitar pérdida L1.

## 21. Tests E2E
- Escenario: Día 30 de operación del fondo cuantitativo HFRC. El bot generó un total bruto de +14.5% ($14,500 en $100k). El reloj toca la marca de extracción quincenal. Se auto-suspenden las órdenes maestras por 1 minuto. Se re-consolidan los remanentes de Altcoins oscuras (doge, pepe) vendiéndolas a USDT a mercado. El balance asienta $14,500 netos. Se emiten 3 peticiones API de extracción masiva. Los CEX liberan los retiros hacia el Smart Contract Gnosis Safe del fondo. Las firmas de directores humanos se requieren offline en la UI de Safe para disponer del dinero real y auditar éxito final. El bot vuelve al trabajo.

## 22. Checklist de producción
- [ ] Whitelist Manual CEX: Habilitar y fijar la IP Estática (Elastic IP de AWS) obligatoria para el endpoint de retiro y bloquear cualquier retiro de API para IPs nuevas o dinámicas.
- [ ] Confirmar Contratos Proxy. Muchas Billeteras Cold Storage como Gnosis son Smart Contracts. Muchos CEXes advierten "DO NOT send funds to Smart Contracts, only EOAs". Revisar documentación de cada CEX para asegurarse de que soportan envío a contratos de tesorería L1.
- [ ] Regla Legal & AML (Anti-Money Laundering): Barrer volúmenes gigantes en fracciones estructuradas para no gatillar una alerta automática de la Unidad Financiera del exchange que congela cuentas por "Movimientos Sospechosos" (Structuring Suspicion).

## 23. Ejemplo de configuración no hardcodeada
```yaml
profit_sweeper:
  extraction_frequency_hours: 168 # Weekly
  profit_compounding_retention_pct: 20.0  # Keep 20% in the exchange to grow base capital
  min_extraction_threshold_usd: 5000.0
  max_acceptable_withdrawal_fee_usd: 25.0
  panic_mode_convert_to_stable: true
  cold_addresses:
    erc20_stablecoins: "0xColdMainVaultAddress..."
    spl_tokens: "SolanaColdVaultAddress..."
```

## 24. Ejemplo de pseudocódigo
```javascript
async function executeScheduledSweep() {
    const currentNav = Ledger.getTotalNAV();
    const baseCapital = Ledger.getBaseCapitalMark();
    const pureProfit = currentNav - baseCapital;

    if (pureProfit < CONFIG.min_extraction_threshold_usd) {
        log.info("Profit insufficient for gas-optimized sweep. Continuing accumulation.");
        return;
    }

    const extractionAmount = pureProfit * (1 - (CONFIG.profit_compounding_retention_pct / 100));
    const newBaseCapital = currentNav - extractionAmount;
    
    // Safety lock during sweeping
    RiskEngine.setSystemStatus('YELLOW_SWEEPING');
    
    try {
        // Find best stablecoin and venue to withdraw from
        const { venue, asset, network } = await SweepOptimizer.findCheapestExitRoute(extractionAmount);
        
        // Execute CEX withdrawal
        const tx = await CexApi.withdraw(
             venue, 
             asset, 
             network, 
             CONFIG.cold_addresses.erc20_stablecoins, 
             extractionAmount
        );
        
        // Update High Watermark permanently
        Ledger.updateBaseCapitalMark(newBaseCapital);
        AuditLogger.recordSweep(tx, extractionAmount);
        
    } catch (e) {
        log.error("Sweep failed (Network/API issue). Retrying in next cycle.");
    } finally {
        RiskEngine.setSystemStatus('GREEN');
    }
}
```

## 25. Criterio final de excelencia
El Profit Sweeper sella y materializa en el mundo físico la genialidad teórica del bot matemático. Garantiza que la guerra de latencias y spreads no sea solo números parpadeando en un monitor de AWS, sino flujos de capital neto irreversibles custodiados bajo las medidas militares del más alto nivel financiero.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Bloqueos manuales de soporte humano (Manual KYC Review) en el exchange justo al pedir la extracción (Risk/AML Flag del CEX). (Requiere resolución humana externa).
- Dependencias: API Withdrawal con permisos de Full Trust.
- Próxima skill: Alertas de desvío de pegs (Stablecoins/LSDs) (Skill 44).
