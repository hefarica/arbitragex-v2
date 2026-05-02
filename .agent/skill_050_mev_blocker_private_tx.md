# SKILL 050 — MEV Blocker & Private Transaction Routing

## 1. Propósito superior
Ocultar y blindar los ataques de arbitraje on-chain. Cuando el Bot caza una oportunidad colosal en un DEX (Uniswap/Curve), si transmite la transacción a la mempool pública (P2P Gossip Network), los bots "Predadores MEV" (Maximal Extractable Value) como Searchers, Front-Runners y Sandwich Attackers clonan la operación, sobornan al minero/validador con mayor gas fee, y roban el arbitraje dejando al Bot original con una pérdida de gas o un precio destructivo. Esta skill actúa como un Túnel Encriptado Secreto hacia los Validadores, burlando el Bosque Oscuro (Dark Forest) de Ethereum/L2.

## 2. Nivel de conocimiento requerido
Ingeniero MEV (Maximal Extractable Value), Criptógrafo de Block-Builder. Conocimiento de la Arquitectura Proposer-Builder Separation (PBS) en Ethereum (Flashbots, MEV-Boost, Eden Network, bloXroute). Comprensión de "Bribes" (Sobornos al minero `block.coinbase.transfer`), Bundling de Transacciones Atómicas (Empaquetar transacciones inseparables), y evitación de Sandwich Attacks mediante Tolerancias Estrictas (Slippage Limits on-chain).

## 3. Capacidades principales
1. Ruteo Privado Obligatorio (Flashbots / MEV-Share): En lugar de enviar un `eth_sendRawTransaction` al RPC público (Infura/Alchemy), la skill redirige el payload cifrado usando firmas Flashbots-Auth hacia endpoints RPC privados (`rpc.flashbots.net` o `MEV-Blocker.io`). Las transacciones NUNCA se publican en el Mempool público, volviéndose invisibles a los hackers de MEV.
2. Construcción de Bundles (Transaction Bundling): Permite al bot empacar 2 o 3 transacciones como una "Bala Atómica". O entran todas en el mismo bloque en ese exacto orden, o la red las ignora todas (Previniendo "Leg-Risk" on-chain y liquidaciones parciales erradas).
3. Soborno Optimizado a Validadores (Bribe Optimization): En lugar de gastar fortuna en `PriorityFee/GasPrice` base adivinando, esta skill inyecta una transferencia interna al `block.coinbase` (el minero) al final de la ejecución on-chain del Smart Contract, cediendo (por ejemplo) el 50% de la ganancia teórica para garantizar que el validador acepte y priorice ese bloque sin fallos, reteniendo el 50% de ganancia limpia sin riesgo.
4. Protección Anti-Sandwich Automática (Slippage Auto-Tune): Si por obligación no se puede usar RPC privado (ej. redes L2 exóticas sin MEV builders vivos), el módulo inyecta un parámetro estricto de Slippage (`AmountOutMinimum = Calculated_Out * 0.999`) al contrato proxy, forzando un Revert (Fallo) si un bot sándwich trata de rociar el precio antes de nuestra orden, impidiendo el desangre.
5. Emulación Lógica Off-chain (Bundle Pre-Sims): Usa las APIs de `eth_callBundle` para simular asíncronamente si el minero aceptará o no el bundle y qué ganancia dejará antes de enviar la carga definitiva, no quemando un milisegundo a ciegas.
6. Agregador de Builders (MEV-Boost Relay Multi-Casting): No confía en un solo builder. Envía la transacción privada atómicamente y en paralelo a `Flashbots`, `Titan`, `beaverbuild` y `bloXroute`, obligando a que compitan por quién mina el bloque primero a tu favor.
7. Ocultamiento de Saldo de Origen: Usa redes y contratos proxy para que la billetera original del fondo (Que tiene $1M) no sea detectada fácilmente por escáneres que buscan ballenas ciegas en el Mempool.
8. Re-Envío y Adaptación al Bloque (Block-Aware Retries): Si la oportunidad DEX-DEX dura 2 bloques seguidos y el Bundle falla en el Bloque 100, la skill ajusta el soborno y re-intenta en el Bloque 101 hiper-rápidamente detectando cabeceras (Block headers).
9. Mapeo de Soporte Cross-Chain: Sabe que Ethereum tiene PBS/Flashbots hipermaduro, pero BSC, Arbitrum, Optimism y Solana operan con "First Come First Serve" (FCFS) puro o Sequencers centralizados, adaptando si usa Bribes o Pura Latencia según el ChainID.
10. MEV Refund (MEV-Share/Kickbacks): Si el bot hace un trade "inocente", lo rutea a MEV-Share, forzando a los atacantes a pagarle un soborno al propio bot para hacerle backrunning, convirtiendo la caza en un beneficio financiero indirecto.

## 4. Entradas requeridas
- `raw_signed_tx_payload`: Transacción HEX compilada por el Orquestador (Skill 45 y 36).
- `chain_id`: Red donde va a operar (1: Mainnet, 42161: Arbitrum, etc.).
- `gross_profit_usd_projected`: Profit neto estimado.
- `miner_bribe_pct`: Porcentaje del Profit cedido al Validador (Generalmente ~50-90% en Flashbots para oportunidades evidentes).

## 5. Salidas esperadas
- `dispatch_receipt`: Estado de envío del Bundle.
- `bundle_inclusion_status`: Callback indicando si la transacción entró (Landed) o falló (Dropped).
- `bribe_cost_log`: Resta contable del soborno pagado.

## 6. Reglas inmutables
- JAMÁS enviar una transacción de Arbitraje DEX puro en Capa 1 de Ethereum Mainnet a la Mempool Pública sin protección Flashbots/Private RPC. Será robada en el 100% de los casos (Garantía estadística de ruina por MEV Bot Sniping y Gas Wars).
- El Soborno (Bribe) transferido en el Smart Contract mediante `block.coinbase.transfer(monto)` debe estar acondicionado a la condición estricta: `if (profit < minProfit) revert();`. Si el bloque no rinde dinero, el soborno nunca se paga, blindando la cuenta a Cero-Riesgo (Risk-Free).
- Si la Red es una Capa 2 con un Secuenciador Centralizado asimétrico puro y ciego (como Arbitrum One One-Sequencer), la protección es innecesaria y el ruteo privado puede causar pérdida de latencia crítica. La skill debe saltar la restricción Flashbots dependiendo del ChainID.

## 7. Algoritmos o métodos que debe conocer
- MEV-Boost / Flashbots Builder API (JSON RPC `eth_sendBundle`, `eth_sendPrivateTransaction`).
- Firmas ECDSA paralelas para `X-Flashbots-Signature` Headers exigidos por el Relay.
- Game Theory en Subastas al primer precio cerrado (First-Price Sealed-Bid Auctions - FPA).

## 8. Fórmulas críticas
- **Cálculo del Bribe (Soborno)**: `Miner_Bribe = Gross_Arb_Profit * Competition_Multiplier (0.50 a 0.99)`
- **Condición Atómica EVM**: `require(balanceAfter - balanceBefore > MinimumExpectedProfit, "Arbitrage MEV Stolen/Slippage");`
- **Tasa de Exito de Inclusión**: `(Bundles_Landed / Bundles_Sent)`

## 9. Casos extremos
- Uncle Block Risk / Reorgs (Reorganización L1): El Bundle de Flashbots gana, se inserta en el Bloque N, pero 12 segundos después, un nodo validador corrompido de la red propone un bloque más largo (Reorg) que invalida el Bloque N, publicando tu transacción en texto plano a todos y robándola en el N+1 (Uncle Bandit Attack). Muy raro post-Merge (Ethereum PoS), pero el Bot debe manejar el re-despliegue veloz y bloquear saldos de cuenta hasta confirmación profunda (Skill 38 Finality Check).
- Soborno Ciego Fallido: El bot envía una orden asumiendo que 60% de soborno es suficiente. Un competidor ruso MEV-Bot ofrece 61%. El bot pierde. El bot debe incorporar una lógica de retroalimentación probabilística (Game Theory, Bayes, Skill 9) que aprenda qué % es necesario según la hora, la liquidez y el token. (Ej. Gaps en Shiba requieren 99% soborno, gaps en USDC/DAI requieren 10% soborno).
- Censura en Relays (OFAC Sanctions): Algunos Relays rechazan procesar transacciones provenientes de direcciones sancionadas. El MEV Multi-Caster debe incluir Relays Neutros o "No-Censurables" (`Agnostic Relays`) asegurando inclusión infinita sin barreras jurisdiccionales que entorpezcan el libre comercio on-chain del Agente.

## 10. Validaciones obligatorias
- PRE: Chequear dinámicamente si el ChainID y el Smart Contract están soportados en el Router RPC privado (Endpoint de conectividad HTTP a `<builder-url>`).
- CÁLCULO: Incorporar en el Orquestador maestro que firmar la cabecera del Flashbot RPC es obligatorio. Flashbots exige firmar el payload JSON con una Llave Privada *Independiente* (Authentication Key) para crear un historial de reputación al bot, desvinculada de la cuenta de billetera (Skill 45 segregación de keys).
- POST: Si el bundle "Droppea" (No se mina tras 2 bloques seguidos), el bot debe matar lógicamente el proceso del Trade, liberar los Locks Contables de USDC (Skill 38) y pasar a otra ruta.

## 11. Criterios de aprobación
- Ruteos L1 (Ethereum) envían la petición cifrada `eth_sendBundle` que completa en un tiempo de RTT en red menor a 150ms.
- Ninguna operación de Arbitraje On-chain genera registros crudos pendientes en Etherscan / Mempool (100% invisibilidad P2P).

## 12. Criterios de rechazo
- Intento de ruteo MEV-Share en redes como Solana, que no usan Mempool global, quemando ciclos de procesamiento en APIs no nativas o no soportadas (Requiere validación de ecosistema).
- Rechazo del Bundle por el Relay indicando `Simulated Revert` (Generalmente por error matemático en la inyección del Bribe al minero en Solidity).

## 13. Riesgos que mitiga
- Riesgo de Dark Forest Assassination: Sin esto, los Arbitrajistas On-Chain puros (On-chain to On-chain, ej. Uniswap a Sushiswap) pierden sistemáticamente cada uno de sus trades a favor de Searchers más grandes (MEV Bots experimentados) que tienen la infraestructura de ruteo directo.
- Ataques tipo "Salmonella" o "Honeypot Poisoning" inyectados por bots que leen tus intenciones en la mempool pública.
- Costes de Gas Fallidos: En transacciones directas, si el Trade Falla on-chain, pierdes la tarifa base del Gas ($15-$150 USD de pérdida pura en Reverts). Usando Flashbots, si el trade falla en simulación, o la red lo rechaza, Pagas CERO USD de penalización (Risk-Free Reverts).

## 14. Integración con otras skills
- Receptora primaria del Payload pre-listo del Orquestador (Skill 36) y de las firmas de Seguridad (Skill 45).
- Informa los costos totales puros de `Bribe + Gas` al Tracker Analítico de Riesgo y Comisiones (Skill 39).

## 15. Modelo de datos sugerido
```json
{
  "MevProtectedPayload": {
    "chain_id": 1,
    "strategy_type": "DEX_DEX_ARBITRAGE",
    "bundle_hash": "0xabc123fed456...",
    "target_block_number": 19450302,
    "projected_profit_eth": 0.50,
    "miner_bribe_eth": 0.35,
    "bribe_percentage": 70.0,
    "relays_notified": ["flashbots", "titan", "beaver"],
    "bundle_status": "LANDED_SUCCESSFULLY",
    "latency_submission_ms": 42
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Multiplexor HTTP concurrente (`Promise.all` para `fetch` en POST o RPC Sockets) que inyecta la firma en el header y re-envía el payload JSON de Array de Tx a 5 Relays constructores masivos (Agnósticos) simultáneamente sin esperar bloqueos I/O.

## 17. Logs obligatorios
- `[INFO] L1 Arbitrage Constructed. Projected Profit: 0.5 ETH. Bribe: 0.35 ETH (70%). Dispatching Private Bundle to 4 Relays.`
- `[DEBUG] Bundle Simulation on Eden Network Passed. Waiting for block 19450302 consensus...`
- `[CRITICAL] BUNDLE DROPPED (Competition Bribe Out-Bid). A competitor MEV bot paid a higher bribe. Unlocking balances and aborting safely (Loss: $0.00).`

## 18. Métricas obligatorias
- `mev_bundles_submitted_count`.
- `mev_bundles_landed_win_rate_pct` (Vital para el algoritmo de aprendizaje automático - Skill 47).
- `bribe_efficiency_ratio` (Mide cuánto profit se regala innecesariamente a los mineros. Ideal: Optimizar a la baja).

## 19. Tests unitarios
- Multi-Cast Failure Resistance: Purgar red simulada con 4 Relays. Mockear Relay 1 y 2 con Fallo HTTP (502 Bad Gateway), Relay 3 con Timeout, Relay 4 con Éxito. El Orquestador Multiplexor DEBE sobrevivir, resolver favorablemente, y reportar Éxito si al menos 1/4 Relays asimiló el Bundle sin crashear el proceso.
- Dynamic Chain Recognition: Lanzar petición de protección MEV con ChainID = 42161 (Arbitrum). El módulo debe detectar que "Arbitrum no necesita Flashbots" y auto-recortar (Short-circuit) la petición para enviar la transacción como una RawTx encriptada clásica a Sequenciador Alchemy privado nativo, sin quemar cabeceras extra ni firmas falsas.
- Flashbots Auth Signer: Validar que el header `X-Flashbots-Signature` coincida exactamente con la función de firmado Hash(Payload) requerida usando una Private Key random, sin revelar la cuenta CEX privada.

## 20. Tests de integración
- Conexión a la red `Goerli` o `Sepolia` Testnet Flashbots endpoint. Intentar someter un Bundle falso vacío. La respuesta HTTP JSON RPC debe arrojar un formato estructurado y el bot debe decodificarlo correctamente (`Simulation error` / `Revert`).

## 21. Tests E2E
- El agente Maestro HFRC capta un desajuste del 5% entre Uniswap V3 y Curve Finance (Skill 25 y 26). Skill 13 levanta alerta. El Orquestador crea un payload Cifrado. La Skill 50 envuelve la transacción en un Bundle. Inyecta el Soborno al validador (Ej. `block.coinbase.transfer(profit * 0.8)`). El Multiplexor inyecta el paquete a 5 constructores de bloques Flashbots/Titan/Bloxroute. Dos segundos después, la red Mainnet Ethereum crea un nuevo bloque L1. La transacción se mina en primer lugar del bloque (Index 0). El spread es absorbido sin dejar huella en la mempool, el minero se queda con el 80% y el fondo absorbe el 20% pasivamente libre de cualquier riesgo transaccional sin pagar Gas al fallar, probando la "Invisibilidad Táctica de Combate" On-chain.

## 22. Checklist de producción
- [ ] Inyección a la Config de Relays "No-OFAC" y Neutros que acepten el tráfico global para maximizar el "Inclusion Rate" (Probabilidad estadística de que tu bloque se escoja primero).
- [ ] Eliminación Rígida de Endpoints Públicos de Infura o Alchemy (WSS/REST públicos) para la función POST `eth_sendRawTransaction`. Si ese código queda vivo en el pipeline de Ethereum, el esfuerzo del Módulo 50 será inútil por filtración de Mempool (Leaking).
- [ ] Optimización de Slippage Limits Hardcodeado (`minAmountOut`) en el Smart Contract On-chain. No confíes que la invisibilidad privada te defiende al 100%. Si un constructor de bloque corrupto te "Desempaqueta" (Unbundles), el Slippage Revert `require` del Smart Contract es la última pared de titanio protegiendo el capital de la expropiación algorítmica.

## 23. Ejemplo de configuración no hardcodeada
```yaml
mev_protection_engine:
  enable_private_routing_l1: true
  enable_private_routing_l2: false   # Sequencers like Arbitrum are FCFS currently
  default_bribe_percentage_target: 75.0 # Give 75% of profit to miners to guarantee inclusion
  auth_signer_private_key_env_var: "FLASHBOTS_AUTH_KEY"
  target_relays_mainnet:
    - "https://relay.flashbots.net"
    - "https://rpc.titanbuilder.xyz"
    - "https://builder.weversetech.com"
    - "https://rpc.beaverbuild.org"
```

## 24. Ejemplo de pseudocódigo
```javascript
class MEVBlockerRouter {
    constructor(authSignerKey) {
        this.flashbotsAuthSigner = new ethers.Wallet(authSignerKey); // Only for identification, holds 0 funds.
        this.relays = CONFIG.target_relays_mainnet;
    }

    async dispatchAtomicBundle(rawSignedTransactionsArray, targetBlockNumber) {
        // Construct standard payload
        const payload = {
            jsonrpc: "2.0",
            id: 1,
            method: "eth_sendBundle",
            params: [{
                txs: rawSignedTransactionsArray,
                blockNumber: `0x${targetBlockNumber.toString(16)}`
            }]
        };

        const payloadString = JSON.stringify(payload);
        const signature = await this.flashbotsAuthSigner.signMessage(ethers.utils.id(payloadString));
        
        // Multi-cast to all relays concurrently (Don't wait for one to fail)
        const dispatchPromises = this.relays.map(async (relayUrl) => {
            try {
                return await fetch(relayUrl, {
                    method: "POST",
                    headers: {
                        'Content-Type': 'application/json',
                        'X-Flashbots-Signature': `${this.flashbotsAuthSigner.address}:${signature}`
                    },
                    body: payloadString
                });
            } catch (error) {
                return null; // Swallow individual relay errors
            }
        });

        const responses = await Promise.all(dispatchPromises);
        
        // Analyze success logic...
        if (responses.every(r => r === null)) {
             throw new Error("ALL_RELAYS_REJECTED_OR_TIMEOUT");
        }
        
        return "BUNDLE_SUBMITTED_AND_PENDING_CONFIRMATION";
    }
}
```

## 25. Criterio final de excelencia
El MEV Blocker y Ruteador de Transacciones Privadas es el escudo de invisibilidad (Stealth Cloak) del Agente Supremo. Comprende y manipula las fallas y corrupciones estructurales (Bribes/Sobornos) del ecosistema PoS de Ethereum, explotando las mecánicas de constructores de bloques para ejecutar ataques atómicos "Riesgo Cero" donde o todo sale perfecto o nada ocurre, destruyendo en el proceso la ventaja táctica de todos los francotiradores enemigos en el Bosque Oscuro de Web3.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: "Unbundling" malicioso por validadores enemigos que desarman tu bloque para re-ensamblarlo y robarte (Casi nulo usando Relays confiables, solucionado 100% con Slippage estricto en el Contrato L1).
- Dependencias: Proveedores Flashbots MEV-Boost / Smart Contract atómico con Bribe injection logic.
- Próxima skill: Orquestador de Smart Contracts de ejecución proxy L1/L2 (Skill 51).
