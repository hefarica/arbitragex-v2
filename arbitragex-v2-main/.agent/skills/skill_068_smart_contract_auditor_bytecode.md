# SKILL 068 — Smart Contract Auditor (On-chain Bytecode Analysis)

## 1. Propósito superior
Actuar como la Barrera Cripto Inmune (Antibody System L1/L2) del Fondo de Alta Frecuencia. Antes de que el Agente invierta millones de dólares en proveer Liquidez V3 L1, Ejecutar Yield Farming DeFi o Arbitrar una nueva Shitcoin/MemeCoin L1, esta Skill descarga el Bytecode Crudo (EVM Opcodes) del contrato L1 y lo analiza asíncronamente en milisegundos O(1) buscando Trampas Mentales (Honeypots), Impuestos Ocultos (Hidden Taxes L1), Modificadores Maliciosos (Malicious Owners), y Funciones Block-Transfer L1 (Anti-Bot Traps) para Vetar la operación CEX-DEX en milisegundos antes del desastre L1.

## 2. Nivel de conocimiento requerido
Auditor EVM Cyber-Sec L1/L2. Deep Reverse-Engineering EVM Bytecode, Decompilación de Solidity/Yul O(1) local In-Memory, Análisis de Taint Flow, Control Flow Graph (CFG) analysis, Detección Signatures O(1), y Maestría en Explotación/Prevención de Flashloans/Re-entrancy Vulnerabilities.

## 3. Capacidades principales
1. Honeypot Detection O(1) (Trampas de No-Venta L1): Inspecciona el Bytecode L1 para funciones anómalas en el `transfer()` L1 (ERC20). Si el Contrato Cripto bloquea a los usuarios de vender L1 L2 (Exigiendo un Flag de Whitelist O(1) que el creador Scammer se guardó para sí), la Skill anula el Arbitraje HFT L1.
2. Hidden Tax Extraction L1 (Impuestos Asimétricos): Las calculadoras (Skill 13 O(1)) asumen que si compras $100 y el CEX cobra 0.1%, recibes $99.9. Pero si el Token EVM tiene un Tax Contract Oculto del 10%, recibirás 90%. El Arbitraje muere. El Auditor simula el "Dry Run Transfer" en Memoria EVM Rust (Revm) detectando el Sangrado Oculto Taxeado L1 e inyectándolo al Optimizador O(1).
3. Owner Privilege Analysis (Centralization Risk L1): Detecta si existe la Función `mint()` L1 o `pause()` L1 sin Timelocks, MultiSigs o Thresholds. Si un Dev Random puede mintear Billones de Tokens L1 e imprimirlos en la cara del Agente HFRC, el Riesgo Direccional HFT L1 es Asimétrico-Terminal y se veta la Liquidez LP L1 O(1).
4. AMM Sync Traps (Liquidity Pool Manipulation L1): Algunos contratos Scams L1 manipulan la variable interna `sync()` del pool de Uniswap V2 L1 tras cada trade para romper los Oráculos de Precios Flashbots L1. El Auditor revisa dependencias L1-EVM anómalas.
5. Re-entrancy Vulnerability Checker L1: Si el Agente HFRC interactúa con un Protocolo Yield Farming Nuevo L1 (Skill 58). El Auditor revisa si el Protocolo cumple el patrón "Checks-Effects-Interactions L1". Si carece del Mutex `nonReentrant` O(1) L1, el riesgo de que roben el protocolo con nuestro dinero dentro L1 aumenta un 500%, degradando el "Trust Score HFT" en la decisión CEX.
6. Bytecode Similarity Matching (Fuzzing O(1)): El Creador de una Moneda Scam "A" L1 lanzó una Moneda Scam "B" L1. Cambió el nombre pero el Bytecode Base EVM es 99% idéntico. La Skill HFT L1 usa Vectores KNN O(1) para detectar similitud de Código Basura L1 Scam y Blacklistear preventivamente la moneda L1 CEX-DEX en 2 milisegundos O(1).
7. Simulación Forense del Ciclo de Vida L1 (Atomic Lifecycle Dry-Run O(1)): No solo lee Código Muerto L1. La Skill inyecta un Bloque Virtual RAM (EVM Clone O(1)). Mintea Tokens L1, Envía L1, Vende L1 (Buy/Sell/Transfer Cycle). Si alguna de estas 3 operaciones lanza `REVERT` o retiene más del `0.5%` de capital L1 O(1) HFT, el Token es Coronado Cripto-Basura L1.
8. Proxy Contract Mismatches (EIP-1967 L1): Valida si el Token O(1) es un Proxy Upgradable. Los Scammers lanzan un Token Limpio L1, consiguen liquidez HFT, y luego "Actualizan (Upgrade)" la Lógica L1 del Proxy inyectando el Robo Tax 100%. La skill monitorea los Eventos EVM `Upgraded` O(1) L1 HFT en Background Async para Desarmar la Cobertura Atómica en cuanto el contrato mute L1.
9. Blacklist API Cross-Check L1: Consulta O(1) Localmente Listas Negras (OFAC, Chainalysis Mocks, Goplus Labs Signatures) de Billeteras/Contratos Peligrosos L1 evitando congelamientos CEX L2 (Compliance Risk HFT).
10. MEV Blocker Interaction Safety O(1): Audita que el Contrato Customizado L1 HFRC Local EVM no tenga lagunas O(1) que permitan a los Searchers enemigos L1 hacer Arbitraje contra nuestra Bóveda de Liquidez Privada O(1). (Self-Audit Continua EVM).

## 4. Entradas requeridas
- `target_contract_address_l1`: Hash del Contrato `0x...` del nuevo Token / Protocolo L1 L2 HFT a operar.
- `local_evm_state_simulator`: El Módulo Rust `Revm` (Compartido con Skill 67 L1 O(1)) listo para ejecutar Dry-Runs O(1) sin consumir Red.
- `threat_intelligence_signatures`: Base de datos Offline O(1) In-Memory de Bytecodes Maliciosos L1 Históricos Cripto.

## 5. Salidas esperadas
- `contract_safety_score_0_to_100`: Entero. (Ej. 99% Seguro HFT O(1), 15% Peligro Scammer L1).
- `hidden_buy_tax_bps` / `hidden_sell_tax_bps`: Retenciones ocultas descubiertas empíricamente por EVM Dry-Run O(1) In-memory L1.
- `security_veto_signal`: Flag absoluto (`TRUE / FALSE`) que apaga atómicamente el Dispatcher (Skill 64 / 36 L1) O(1).

## 6. Reglas inmutables
- Nunca operar una Transacción de Extracción CEX-DEX (Skill 64 HFT L1) sin que la Bóveda de Auditoría de Código O(1) EVM retorne `SAFE`. Ignorar esta regla garantiza estadísticamente perder el Capital Base L1 en menos de 4 semanas en el inframundo de las Altcoins L1/L2 (Honeypot Trap 100% Lethal O(1)).
- El Análisis EVM DEBE realizarse sin dependencia de APIs Externas L1 (Etherscan, GoplusAPI). Si Etherscan se cae, el Bot Queda Ciego L1 O(1) HFT o asume seguridad ciega. Se exige Ingeniería de Reversa O(1) Local en C++/Rust y Trace Simulators EVM Puros L1 HFT In-Memory.
- Para Tokens de Alto Market Cap Histórico (Blue-chips como WBTC, USDT, UNI L1 O(1)), la Skill asigna Score 100 y aplica Cache O(1) Permanente. El Auditor se centra recursos computacionales estrictamente en Oportunidades "Long-Tail" (Tokens Exóticos de 5 minutos de vida con Arbitrajes Mágicos Irreales L1 HFT).

## 7. Algoritmos o métodos que debe conocer
- Symbolic Execution O(1) Algorithms (Mythril/Oyente logic ported to fast FFI L1).
- EVM Opcodes HFT (SSTORE, SLOAD, DELEGATECALL, SELFDESTRUCT Traps L1).
- State-Diff Tracing EVM Local HFT.

## 8. Fórmulas críticas
- **Cálculo Real de Tax L1 O(1)**: `Simulated_Tax = (Expected_Received_Amount - Actual_Received_Amount_In_EVM_Memory) / Expected_Received_Amount` (Detecta si el Owner está drenando L1 en cada Tx O(1)).
- **Safety Threshold Veto L1**: `if (Simulated_Tax > Config.Max_Tax_Tolerance || Is_Honeypot == True) { VETO_TRADE() }`

## 9. Casos extremos
- Dynamic Tax Scams L1 (Tax Dinámico de Bloque L1): El Scammer crea un código L1 donde en los bloques impares el Tax es 0%, pero en bloques pares es 99% L1. El Bot hace Dry-Run HFT O(1) en el Bloque Par y el bot dice "Es Seguro, 0% Tax". Ejecuta HFT en bloque impar y el Gas L1 choca perdiendo 99% Cripto. Solución FFI L1: El Simulador O(1) inyecta Mutaciones (Fuzzing Local EVM O(1)) alterando `block.number` y `block.timestamp` L1 al azar en 5 corridas de 1 milisegundo HFT, asegurando que el contrato es Estático EVM O(1) y no Determinista Temporal EVM Trap L1.
- Blacklist Trap L1 CEX-DEX: El Token "Pepe3.0 L1" prohíbe que el Contrato HFRC (La Bóveda Proxy Skill 51) pueda Vender. Permite a billeteras normales, pero castiga a Contratos Inteligentes L1 (Anti-Bot MEV Flag L1). Tu Bot CEX HFT no lo sabe L2. Compra L2 CEX, Mueve L1 DEX, Intenta Vender L1 DEX EVM. Revierte O(1). Pierdes Delta CEX HFT O(1). Solución O(1): El Dry-Run HFT EVM L1 se realiza EXPRESAMENTE suplantando (Impersonating) la Dirección L1 Criptográfica O(1) del Proxy HFRC L1, desnudando la discriminación Contractual MEV O(1) antes del Dispatch.
- Gas Limit Griefing L1 (Bucle de Quemado de Gas L1 EVM): El Smart Contract Scammer no tiene Tax. Pero su función `transfer()` L1 contiene un loop EVM O(1) inútil que consume 50 Millones de Gas L1. El Bot HFRC no lo sabe. Su Orquestador Híbrido (Skill 64 L1 L2) cree que el envío es barato. La transacción HFT falla por `Out Of Gas L1` (Gas Griefing EVM) o si se aprueba, devora el Alpha HFT L1 Cripto en comisión quemada. Solución: El Auditor L1 devuelve exactamente el "Gas Used O(1) Mínimo Empírico L1" de su simulación Local RAM HFT asíncrona, VETANDO si gasta > 300,000 unidades de Gas HFT Irreal.

## 10. Validaciones obligatorias
- PRE: Chequeo Cache O(1) L1. Si el contrato ya fue auditado en los últimos 5 minutos L1 y su `codeHash` L1 EVM no ha mutado, BYPASS Auditoría L1 (Salva 2ms de Latencia Crítica CEX HFT O(1)).
- CÁLCULO: Mapear la interacción L1 del DEX Router. A veces el Token L1 no tiene Tax nativo en su código, PERO el Orquestador CEX HFT llama al DEX "PancakeSwap L1", y el Pool de PancakeSwap L1 FUE HACKEADO para robar fondos O(1). El Auditor NO SOLO simula el Token L1, sino la Transacción Completa L1 End-to-End HFT (Route Dry-Run Complejo L1 O(1)).
- POST: Si el Auditor EVM emite una "Luz Amarilla L1" (Ej. Tax de 2% Fijo), No mata el Arbitraje. Se lo envía Matemáticamente al Ruteador CEX/DEX (Skill 64 L1) O(1) para que decida si el Spread de 5% Cripto sobrevive al castigo de 2% L1. (Comunicación Asimétrica Paramétrica O(1) HFT).

## 11. Criterios de aprobación
- Capacidad de Descubrir y Vetar "HoneyPots L1 EVM Standard" (Aprobar compras, Bloquear Ventas L1) usando simulación EVM RAM Local en menos de 3.5 milisegundos O(1) de procesamiento Multihilo C/C++ FFI sin trabar Event Loop de Node/Rust.
- Tolerancia Cero O(1) EVM L1 Cripto a los Tokens "Pausados" Temporalmente o con Blacklist Contractual, devolviendo Veto Absoluto Operativo O(1).

## 12. Criterios de rechazo
- Basarse en API de Terceros para Auditoría L1 HFT. Usar `GoPlus Security API` o `TokenSniffer` por REST HTTPS tardará 1500 milisegundos en L1. En ese tiempo 50,000 Bots MEV L1 devoraron el Arbitraje. La Infraestructura HFT de HFRC requiere Integración Off-Grid In-Memory L1 Autárquica Absoluta.
- Enviar transacciones ciegas On-chain L1 O(1) y confiar en que si la Blockchain revierte, no importa. Cada Revert L1 gasta Capital Base HFT O(1) Cripto (Gas Fees Drenantes L1). El Auditor reduce el "Revert Rate HFT L1" a < 0.01% Factual Institucional O(1).

## 13. Riesgos que mitiga
- La Mortalidad del Long-Tail Arbitrage L1 (Riesgo Cripto O(1)). En Tokens Blue Chip (BTC, ETH L1), ganar el 0.01% de Spread requiere 100 Millones HFT L1 O(1). El Agente gana márgenes brutales (Spreads de 5% a 20%) cazando Monedas Meme/Micro-caps recién listadas. Pero el 80% de estas monedas nuevas son SCAMS DISEÑADOS por hackers L1 Cripto para robar a los Bots HFT Ciegos L1 O(1). El Smart Contract Auditor funciona como un Traje Anti-Radiación L1 O(1) HFT, permitiendo al Agente operar en las Zonas Tóxicas más lucrativas del criptoverso CEX-DEX L1 sin sufrir envenenamiento colateral Cripto O(1).

## 14. Integración con otras skills
- Escudo Defensivo del Hybrid Router CEX-DEX (Skill 64 L1 L2) y MEV Arbitrage (Skill 67 L1).
- Usa Instancias de EVM Simulador Local O(1) de Skill 67 L1 HFT.
- Actualiza permanentemente la Blacklist Histórica Cripto del Risk Engine Global (Skill 41 L1 L2).

## 15. Modelo de datos sugerido
```json
{
  "SmartContractAuditReport": {
    "token_address_l1": "0xBadScamTokenAddress000...",
    "timestamp_ms": 1714521234105,
    "audit_latency_ms": 2.1,
    "security_status": "HONEYPOT_DETECTED_L1_O1",
    "evm_simulation_o1": {
      "buy_simulation_success": true,
      "buy_gas_used_wei": 145000,
      "sell_simulation_success": false, // Fails when we try to sell back
      "sell_revert_reason_decoded_l1": "TRANSFER_FROM_FAILED: NOT_WHITELISTED_O1",
      "hidden_tax_discovered_pct": 100.0 // Scam Trap Total Wipeout O(1)
    },
    "contract_flags": ["UPGRADABLE_PROXY_RISK", "OWNER_MINT_DETECTED_L1"],
    "action": "VETO_AND_BLACKLIST_PERMANENTLY_HFT_L1"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Singleton C++/Rust Worker `EVM_Forensics_Auditor_O1`. Contiene un método `verifyLifecycleSafety(tokenContract, amount)` que es llamado sincrónicamente por el MUX (Skill 64 L1) si el Token Address no está en la Whitelist Cache In-Memory Segura.

## 17. Logs obligatorios
- `[DEBUG] L1 EVM Auditor O(1): Scanning New Token Pair on PancakeSwap L1. Dry-Run Lifecycle complete in 1.5ms. Taxes: Buy 0.5%, Sell 1.0%. Proxy = Safe. Dispatching OK Signal to MUX Router HFT L1.`
- `[INFO] HoneyPot Trap Sniffed L1! Token PEPE10X allows Buy but disables Sell function O(1) L1 EVM Memory via custom Owner Modifier. 14 MEV Bots just got trapped. We bypassed. VETO ACTIVE.`
- `[CRITICAL] Infinite Gas Loop Griefing EVM Detect L1 O(1)! Contract consumes 15 Million Gas on simple Transfer. Arbitrage Router HFT aborted. Blacklisting Dev Address associated L1 O(1).`

## 18. Métricas obligatorias
- `average_audit_time_ms_o1` (Debe orbitar el 1ms-3ms L1 In-Memory O(1) HFT Limit).
- `scam_tokens_vetoed_lifetime_count` (Auditoría visual del dinero salvado al Fondo HFRC por evitar Trampas Cripto L1 O(1)).
- `false_positive_rejection_rate_l1` (Si se asusta mucho y rechaza tokens seguros L1 O(1), el CEX MUX pierde dinero base Alpha HFT).

## 19. Tests unitarios
- Honeypot Bypass RAM EVM O(1): Inyectar EVM Bytecode L1 real del Scam "Squid Game Token" (Aquel que no dejaba vender a nadie L1). El Simulador C/Rust DEBE compilar L1, instanciar Balance Ficticio, intentar Transferir L1 Cripto al DEX Router, Recibir Excepción de Revert L1 Cripto y arrojar Fuego Rojo (`VETO_TRUE`) en menos de 2 milisegundos O(1) Thread Async.
- Hidden Tax Calibration O(1): Inyectar EVM L1 Bytecode del Contrato "SafeMoon" (Con 10% de Impuesto On-Chain). El Simulador manda Transferir "100 Tokens L1". La Billetera de Destino Ficticia Cripto recibe "90 Tokens L1". El Módulo Resta (100-90 = 10 O(1)) y escupe Float al Orquestador L2 HFT: `hiddenTaxBps: 1000`. Comprobado Aislamiento Analítico L1.
- Impersonation Fuzzing EVM L1: Simular Token Anti-Bot L1. El Contract `transfer()` L1 contiene una trampa L1: `if (msg.sender == MyBotAddress) { revert(); }`. El Fuzzer Local O(1) suplanta el Payload Cripto con Orígenes Random L1 vs La Firma Local L1 del Bot HFT. Detecta Discriminación Algorítmica L1 Cripto y veta O(1) asíncronamente L1 CEX-DEX.

## 20. Tests de integración
- Levantar Anvil Mainnet L1 Node Mock. Inyectar Pipeline L2 Async HFT del Routing Híbrido (Skill 64). Alimentarlo con una Lista Ciega L1 O(1) de 10 Tokens (5 Reales L1, 5 Scams Creados localmente con Solc). Ordenar Arbitraje ciego a Todos. Validar en los logs del MockRPC L1 HFT que NINGUNA de las transacciones hacia los 5 Scams L1 Cripto tocó la Blockchain Falsa, comprobando que el Filtro EVM Cortafuegos L1 (Antibody System O(1)) aisló el Virus L1 Cripto con Precisión Aséptica.

## 21. Tests E2E
- El agente HFRC detecta que "DOGE_INU_MARS" (Token desconocido) acaba de cotizarse en un exchange Asiático L2 centralizado (CEX HFT). Al mismo tiempo, en Uniswap L1 EVM, el token cuesta un 30% menos O(1). ¡Oportunidad Gigante MUX CEX/DEX Arbitrage L1-L2 (Skill 64 O(1))! El MUX se prepara para comprar $50,000 Dólares L1 en Uniswap EVM. Skill 68 salta en Memoria (0.5ms EVM Audit L1 O(1)). Compila el Bytecode On-Chain EVM. Fuzzea la lógica de venta L1. Descubre que el Creador Asiático puso una función Oculta L1 que le permite Poner una Tarifa de Venta del 99% si él llama a un botón Admin L1 O(1). La Skill L1 Veta el Ruteo HFT O(1). El MUX Híbrido se Cancela Asíncronamente. 5 minutos después, el Desarrollador asiático presiona el Botón de Tax 99% L1, robando millones a otros Bots Takers HFT L2 ciegos. El Bot HFRC se queda con los Dólares Cripto intactos, inmune a las tretas asimétricas del Hacking Social DeFi Cripto L1, blindado por Matemática EVM Forense L1 In-Memory.

## 22. Checklist de producción
- [ ] Oráculo de Firmas Anti-Exploit O(1) Externo: Integrar una conexión P2P Cripto a un Módulo Forta/Goplus L1 Async O(1). (Pero con Caída Silenciosa a Bypass). Si el Servidor de Auditoría Externa responde en < 5ms, añadir su Veredicto Humano/AI al Veredicto Empírico Local EVM. Si Tarda más de 5ms O(1), Matar Timeout y Guiarse solo por la Inteligencia Local FFI Rust HFT Cripto O(1) para mantener Latencia SLA O(1).
- [ ] Cache O(1) de Expiración Condicional: Si hoy auditaste PEPE L1 EVM O(1) y era "SAFE". Almacenarlo. Pero SI Y SOLO SI el contrato L1 PEPE NO ES UPGRADABLE (No Proxy O(1) L1). Si es Upgradable Proxy L1 (EIP-1967 L1), la caché NO se fía ciega y re-evalúa Hash L1 Lógico EVM de Fondo O(1) para evitar el "Cambio de Reglas EVM" (Bait and Switch HFT O(1)).

## 23. Ejemplo de configuración no hardcodeada
```yaml
smart_contract_auditor_engine_evm_o1:
  enable_live_bytecode_inspection_l1: true
  evm_simulation_max_latency_ms_o1: 5 # Do not delay HFT routes by more than 5ms
  max_acceptable_hidden_tax_bps_l1_o1: 50 # Ignore tokens with > 0.5% internal scam taxes
  auto_veto_on_upgradable_proxies_l1_o1: false # Disallow if True (Safer but limits opportunities in V2 tokens)
  fuzzing_simulation_passes_o1_evm: 2 # Number of EVM dry-runs per token to spot conditional L1 anomalies
  cache_ttl_safe_contracts_minutes_l1_o1: 1440 # Remember safe contracts for 24h
```

## 24. Ejemplo de pseudocódigo
```javascript
class EVMSmartContractAuditor {
    constructor(revmSimulatorC) {
        this.simulator = revmSimulatorC;
        this.safetyCache = new LRUCache(CONFIG.cache_ttl);
    }

    async evaluateContractSafetyL1(tokenAddress, sampleAmount) {
        // Fast Cache O(1) Bypass
        const cachedStatus = this.safetyCache.get(tokenAddress);
        if (cachedStatus) return cachedStatus;

        // FFI Call to Rust EVM O(1) Tracer Engine
        const simulatedResult = this.simulator.dryRunLifecycleO1(tokenAddress, sampleAmount);
        
        let report = { isSafe: true, hiddenTaxBps: 0, flags: [] };

        // 1. Revert check (Honeypot Trap / Anti-bot trap L1)
        if (!simulatedResult.buySuccess || !simulatedResult.sellSuccess) {
            report.isSafe = false;
            report.flags.push("HONEYPOT_OR_REVERT_DETECTED_L1_O1");
            return report; // Immediate Veto O(1)
        }

        // 2. Hidden Tax Calibration L1 O(1)
        const expectedReturn = sampleAmount; // Assuming 0% tax
        const actualReturn = simulatedResult.sellAmountReceived;
        
        const hiddenTaxPct = (expectedReturn - actualReturn) / expectedReturn;
        report.hiddenTaxBps = hiddenTaxPct * 10000;

        if (report.hiddenTaxBps > CONFIG.max_tax_bps) {
            report.isSafe = false;
            report.flags.push("HIGH_HIDDEN_TAX_L1_O1");
        }

        // 3. Centralization Analysis (Owner overrides L1)
        if (simulatedResult.hasOwnerMintFunc || simulatedResult.hasPauseFunc) {
            report.flags.push("OWNER_CENTRALIZATION_RISK_L1_O1");
            // Depending on config, we might not Veto, but warn Risk Engine L1
        }

        if (report.isSafe) this.safetyCache.set(tokenAddress, report);
        
        return report;
    }
}
```

## 25. Criterio final de excelencia
El Smart Contract Auditor Cripto EVM inyecta al Agente HFRC el Instinto de Preservación de Nivel Hack-Proof Definitivo. Evita que la codicia matemática del HFT CEX (La obsesión por el Spread a Ciegas) conduzca al sistema por laberintos de Contratos Inteligentes Manipulados L1. Logra un puente de confianza Absoluta HFT Off-chain In-Memory, neutralizando Scams institucionales y Scripts Tóxicos con la misma frialdad algorítmica con la que genera utilidades L1 L2, transformando al fondo Cripto en una Entidad EVM blindada en el Dark Forest Descentralizado.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Advanced State-Dependent EVM Traps L1 O(1). Hay Hacks donde el Token actúa normal 99% de las veces, PERO explota una vulnerabilidad en el Oráculo O(1) de Precios Cripto L1 ajeno solo cuando un usuario ajeno interactúa. Simular todo el Universo EVM RAM en 2ms L1 O(1) es imposible, se simula el Cripto-Ciclo Inmediato HFT L1.
- Dependencias: Skill 67 (Revm Simulator L1 O(1)), Skill 64 (Mux Router CEX-DEX L2 HFT L1).
- Próxima skill: Order Flow Spoofing & L2 Deception Tactics (Skill 69).
