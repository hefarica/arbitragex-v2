# SKILL 030 — Detección de honeypots / scam tokens

## 1. Propósito superior
Filtrar e invalidar instantáneamente contratos inteligentes fraudulentos (Honeypots, Rug-Pulls, Fee-on-Transfer abusivos, Proxies Mutantes). Dado que el 90% de las "oportunidades de arbitraje ridículamente altas" provienen de tokens falsos con liquidez trampeada, esta skill funciona como un Analizador de Contratos de Grado Forense que protege el portafolio de operar basura, erradicando transacciones inútiles, retenciones de fondos (Cannot Sell) y estafas matemáticas complejas.

## 2. Nivel de conocimiento requerido
Auditor de Smart Contracts Senior / Security Researcher. Dominio profundo de Solidity Bytecode Analysis, EVM Opcodes, técnicas comunes de fraude (Whitelisting dinámico, Blacklisting, Pausable modifiers, funciones de Mint escondidas, Ownership privileges), herramientas especializadas (Honeypot is) y análisis estocástico heurístico del comportamiento del token.

## 3. Capacidades principales
1. Detección Estática (Bytecode Parsing): Leer el código del contrato y buscar firmas maliciosas conocidas (ej. `selfdestruct`, `delegatecall` no auditable, re-entrancy bugs ocultos).
2. Detección Dinámica (Shadow TX): Ejecutar un swap en el entorno de Simulación (Skill 29) e intentar realizar inmediatamente un Swap de reversión (`Buy -> Sell`). Si la Venta falla, es un Honeypot estricto.
3. Análisis de "Fee on Transfer" (Tax): Diferenciar entre un token que indica "AmountOut 10" pero recibe "Amount 8" en la billetera. Si el `Buy Tax` + `Sell Tax` excede el beneficio matemático del spread, se tacha el arbitraje por inviabilidad.
4. Escaneo de Propiedad (Ownership check): Determinar si el Owner o el Contrato tiene la función de Pausar transferencias (`pause()`), confiscar balances, o acuñar infinitamente (`mint()`).
5. Monitoreo de "Liquidity Lock": Inspeccionar si la Liquidez de la pool está bloqueada en servicios como Unicrypt o PinkSale, o si el desarrollador puede drenar los fondos a placer (Rugpull inminente).
6. Revisión de Proxies UPPS/Transparentes: Entender que un token "Limpio" puede ser un proxy que re-apunta a un contrato "Malicioso" modificado a voluntad por un admin multisig.
7. Almacenamiento perenne de un "Global Blacklist Cache": Si un token es identificado como veneno, su dirección entra permanentemente a la lista negra guardada en Memoria/Redis, ahorrando computo de escaneo en futuras peticiones.
8. Control de Ticks Manipulados (Uniswap V3): Detección de concentraciones de liquidez en solo 1 Tick usadas como carnada para engañar cotizadores, vaciándose en Ticks adyacentes.
9. Uso combinado de Oráculos Externos (GoPlus Security, Honeypot.is API) para añadir consenso multi-capa al análisis estático propio del bot.
10. Comprobación de Ticker Spoofing (Falsificación de Nombre): Contratos que se hacen llamar "USDC" o "WETH" pero tienen otro Address y un creador diferente al de Circle/MakerDAO.

## 4. Entradas requeridas
- `token_address`: Dirección del contrato bajo sospecha/operación.
- `router_address`: La dirección del DEX donde supuestamente está la liquidez.
- `buy_tax_threshold` / `sell_tax_threshold`: Umbral de comisiones de transferencia aceptables (Usualmente < 1%).
- `blockchain_id`: Red donde opera el token para cotejo contextual.

## 5. Salidas esperadas
- `is_safe`: Booleano global que da luz verde para arbitrar.
- `threat_level`: `"CRITICAL"`, `"HIGH"`, `"LOW"`, `"SAFE"`.
- `detected_taxes`: `{ buy: 0.1, sell: 5.0 }`.
- `rejection_reason`: Cadena detallando la estafa (Ej. "Cannot sell token. Revert thrown on transferFrom").

## 6. Reglas inmutables
- JAMÁS inyectar liquidez de Arbitraje en un Token desconocido sin que haya cruzado exitosamente este filtro con un estado `"SAFE"`. Un ROI de 200% significa que es una trampa.
- Para la comprobación dinámica (Dry-Run de Compra y Venta), se DEBEN usar los nodos RPC de simulación (Skill 29) de forma obligatoria, no asumiendo que los oráculos externos siempre tienen la razón.
- Si el "Sell Tax" (Impuesto de Venta) detectado de forma dinámica altera la fórmula matemática de ganancia de modo que el `Net_Profit < 0`, el token se rechaza por asimetría, independientemente de si es "estafa" intencional o solo mala tokenómica.
- Se debe manejar un registro rígido "Whitelist" de Bluechips (WETH, WBTC, DAI, USDC, UNI). Jamás perder milisegundos analizando estos tokens sagrados.

## 7. Algoritmos o métodos que debe conocer
- Heurística de detección de Fraude EVM (Pausable Token Standards EIP).
- Deserialización y Matching de EVM Bytecode OpCodes (Identificar variables privadas como `mapping(address => bool) isBlacklisted;`).
- Simulación estocástica (Intentar vender 1 wei, luego intentar vender toda la tenencia simulada).

## 8. Fórmulas críticas
- **Cálculo de Buy Tax Empírico**: `Tax_Buy = 1 - (Saldo_Recibido_Real_Simulado / Saldo_Esperado_Teórico_Amm)`
- **Cálculo de Sell Tax Empírico**: `Tax_Sell = 1 - (ETH_Recibido_Real_Simulado / ETH_Esperado_Teórico_Amm)`
- **Condición de Viabilidad de Spread Modificado**: `Profit_Gross_Calculado * (1 - Tax_Buy) * (1 - Tax_Sell) > Minimum_Viable_Profit`

## 9. Casos extremos
- Anti-Bot Honeypots (Bloqueo en Bloque 0): Tokens que permiten comprar y vender, EXCEPTO si detectan que vendes en el mismo bloque en el que compraste (La rutina estándar de este Agente de Arbitraje). Simular la venta en bloque `N+1` oculta el problema que aparecerá en producción en el bloque `N`.
- Whitelist Honeypots: El desarrollador compra el token en el contrato oficial en la red real, aparentando volumen "orgánico", pero nadie más puede hacerlo. Al simular sin permisos especiales, el bot recibe `Revert`.
- Modificación de Tax In-Flight: Un token legítimo (Taxes al 1%) que, de repente, gracias a su Proxy, actualiza el "Sell Tax" a 99% a través del owner en el momento de un gran volumen transaccional (Tax Rug).

## 10. Validaciones obligatorias
- PRE: Chequear si la dirección del token está en la Memoria RAM Caché `Global_Whitelist`. Si es "WETH", saltar chequeos inmediatamente (0 ms overhead).
- CÁLCULO: Validar la proporción entre Reservas de Liquidez DEX vs Total Supply. Si un solo bot puede comprar el 99% del Supply, es un token trampa generado por un creador para falsificar feeds de agregadores.
- POST: Incorporar el valor porcentual del Tax a los costos fijos (Gas + Fees) para actualizar la métrica de ejecución.

## 11. Criterios de aprobación
- La comprobación dinámica permite Comprar y Vender inmediatamente con retención de fondos (Tax) de < Límite Seguro.
- El Bytecode no contiene firmas obvias de Proxy malicioso u operaciones de Mint ilimitadas.

## 12. Criterios de rechazo
- Fallo de "Dry Run" inverso. (La EOA de prueba compró, pero al hacer call para el sell, lanza error).
- Respuesta de Oráculos externos (GoPlus API) alerta `"Is_Honeypot": true`.

## 13. Riesgos que mitiga
- Riesgo de Agujero Negro "Hotel California": Capital entra, paga entrada cara y nunca puede salir (Imposibilidad estructural de Venta). Es la forma más rápida en la que bots novatos queman $10,000 en 1 día persiguiendo APYs/Gaps inflados artificialmente.
- Riesgo de Slippage invisible: La matemática del AMM aprueba, pero los taxes invisibles del código drenan la transferencia vaciando el balanceo contable.

## 14. Integración con otras skills
- Requisito condicional anterior a la Simulación de Pre-Trade (Skill 29) para tokens exóticos no mapeados.
- Contribuye masivamente a la reducción de ruido en la Detección de Ciclos (Skill 4). (Si se limpia la basura primero, el grafo matemático halla rutas puras).

## 15. Modelo de datos sugerido
```json
{
  "TokenSecurityAudit": {
    "address": "0xBadC0de...",
    "status": "CRITICAL",
    "threats_detected": ["CANNOT_SELL", "OWNERSHIP_NOT_RENOUNCED", "HIGH_SELL_TAX"],
    "empirical_buy_tax": 0.05,
    "empirical_sell_tax": 0.99,
    "cached": true,
    "source": "SIMULATED_DRY_RUN"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Sub-módulo forense independiente. Puede conectarse asincrónicamente a la API de GoPlus Security (`https://api.gopluslabs.io/api/v1/token_security/...`) y simultáneamente ejecutar el "Dynamic Simulation Checker" on-chain local.

## 17. Logs obligatorios
- `[DEBUG] Token 0xPEPE... checked. Taxes: 0/0. Sell allowed. Status: SAFE. Caching in RAM.`
- `[CRITICAL] Honeypot detected! Cannot execute Sell simulation. Sell Tax calculated at 99%. Adding to permanent Blacklist.`
- `[WARN] Suspicious liquidity structure. Token appears to be spoofing USDC (Fake Token). Abandoning Arbitrage route.`

## 18. Métricas obligatorias
- `honeypots_blocked_count` (Tokens falsos prevenidos, salva vidas financieras).
- `security_audit_latency_ms` (Se espera < 50ms al recaer fuertemente en simulación local, para no interrumpir el flujo HFT).
- `tax_impact_cancellations` (Veces que un tax arruinó un spread matemáticamente perfecto).

## 19. Tests unitarios
- Ticker Spoof: Cargar un hash con symbol "USDT" pero diferente address del Tether oficial de Ethereum. La skill DEBE tacharlo como SCAM/SPOOFING al instante.
- Decodificador de Tax: Simular transacción local recibiendo 90 tokens tras esperar recibir 100. La skill debe devolver exactamente 0.1 (10% Tax).
- Caching: Solicitar validación del mismo Token 2 veces seguidas. La primera vez demora 50ms, la segunda vez debe demorar 0.01ms desde la memoria.

## 20. Tests de integración
- Consumo API Forense: Configurar llamada paralela (Race) entre la Simulación en el Fork Local (Vender token trampeado) y la llamada REST a GoPlus. Asegurar el rechazo unánime.

## 21. Tests E2E
- El agente descubre un spread monumental de +450% en un Token listado hace 5 minutos (Token trampa clásico). Envía la dirección a la Skill 30. El emulador inyecta saldo simulado, compra 1000 tokens, intenta vender 1000 tokens y la llamada revierte en Mempool por función `onlyOwner`. Se bloquea la ruta al momento y el agente sigue escaneando a otra parte de manera impune.

## 22. Checklist de producción
- [ ] Incorporación de Whitelist de Bluechips con inyección estática (Hardcoded Genesis Config) para obviar la capa forense totalmente en operaciones institucionales entre WETH, WBTC y Stables.
- [ ] Detección de Modificadores Proxy: Incluso si el código es seguro, comprobar si es actualizable (Upgradable Contract). Si es upgreadable sin `Timelock`, marcar como Riesgo Medio.
- [ ] Comprobación estricta del contrato del Router (PancakeswapV2 Router oficial vs Contratos de Router clónicos oscuros que secuestran la ejecución).

## 23. Ejemplo de configuración no hardcodeada
```yaml
security_scanner:
  max_acceptable_buy_tax_pct: 1.5
  max_acceptable_sell_tax_pct: 1.5
  require_ownership_renounced: false   # Too strict for many real meme tokens
  enable_third_party_oracle: true      # Uses GoPlus/Honeypot.is as second opinion
  trusted_bluechip_whitelist:
    - "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2" # WETH Mainnet
    - "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48" # USDC Mainnet
```

## 24. Ejemplo de pseudocódigo
```javascript
async function auditTokenSafety(tokenAddress, routerAddress) {
    // 1. Check Fast-Path Whitelist / Blacklist Cache
    if (SecurityCache.isWhitelisted(tokenAddress)) return SAFE_RESULT;
    if (SecurityCache.isBlacklisted(tokenAddress)) return HONEYPOT_RESULT;

    // 2. Perform concurrent External API Check and Internal Dry Run
    const [externalAudit, dynamicRun] = await Promise.all([
        fetchGoPlusSecurity(tokenAddress),
        executeDynamicDryRun(tokenAddress, routerAddress)
    ]);

    // 3. Evaluate results
    if (externalAudit.is_honeypot || !dynamicRun.sell_successful) {
        SecurityCache.addToBlacklist(tokenAddress);
        return { status: "CRITICAL", reason: "Cannot sell or marked as honeypot by Oracle" };
    }

    if (dynamicRun.buy_tax > CONFIG.max_tax || dynamicRun.sell_tax > CONFIG.max_tax) {
         SecurityCache.addToBlacklist(tokenAddress);
         return { status: "HIGH", reason: `Toxic taxes. Buy: ${dynamicRun.buy_tax}, Sell: ${dynamicRun.sell_tax}` };
    }

    SecurityCache.addToWhitelist(tokenAddress);
    return { status: "SAFE", taxes: dynamicRun };
}
```

## 25. Criterio final de excelencia
El filtro forense permite al sistema de arbitraje sobrevivir y ser altamente rentable en los peores bajos fondos del ecosistema cripto (Binance Smart Chain Meme-trenches o Solana Degen Swaps), cazando oportunidades de alto riesgo sin jamás caer en trampas destructoras de billeteras, aplicando pensamiento paranoico institucional en microsegundos.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Soft-Rugs silenciosos donde el dev drena lentamente liquidez sin bloquear venta (Solo afecta a estrategias no atómicas. El arbitraje HFT no sufre porque entra y sale instantáneamente).
- Dependencias: API Externa de Seguridad, Entorno Fork local.
- Próxima skill: WebSockets multi-exchange (Skill 31).
