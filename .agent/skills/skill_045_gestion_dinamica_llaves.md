# SKILL 045 — Gestión dinámica de llaves API/Private keys

## 1. Propósito superior
Proteger los secretos criptográficos, inyectar "Claves Privadas" (Private Keys) para la firma de transacciones On-Chain, y administrar "API Keys / Secrets" de CEX de manera inviolable, efímera y rotativa en memoria viva. Esta skill asegura que NINGÚN secreto de alto rango toque el disco duro físico del servidor (Hard Drive) en texto plano, neutralizando vectores de ataque como Local File Inclusions, Log Poisoning o Infecciones de Ransomware/Trojans sobre la máquina virtual anfitriona (VPS).

## 2. Nivel de conocimiento requerido
Ingeniero de Seguridad Cloud (DevSecOps), Criptógrafo de Clave Pública/Privada. Entendimiento avanzado de Secret Managers (AWS KMS, HashiCorp Vault, Azure KeyVault), Inyección en Memoria Efímera (tmpfs/Ramdisk), Aislamiento de Variables de Entorno, Cifrado AES-256-GCM, algoritmos ECDSA (Curva secp256k1) para firmado de Ethereum (ethers.js/viem), y el protocolo HMAC-SHA256 para CEX Auth.

## 3. Capacidades principales
1. Inyección Dinámica al Arranque (Bootstrapping): Al iniciar el bot, el módulo autentica contra un AWS KMS o HashiCorp Vault usando un rol IAM y descarga en memoria RAM (Zero-Disk) todas las llaves secretas.
2. Limpieza estricta de Logs (Log Sanitization): Interceptador implacable que sobre-escribe cualquier variable en el objeto `console.log` o framework de logging que parezca un formato de clave privada (`0x[a-f0-9]{64}`) o HMAC Token (`***SECRET***`).
3. Rotación de Llaves de CEX (API Key Rotation): Mantiene un pool de 5-10 Claves API por exchange. El bot distribuye el peso HTTP entre ellas, pero si una clave es expuesta o banneada por el exchange (Rate Limit ban skill 35), el Módulo elimina esa clave de la memoria asíncronamente y usa una de respaldo sin detener el motor.
4. Hot Wallet Signer Abstraction: Aisla el objeto `Signer` / `Wallet` de Web3. El código del bot de Arbitraje (Skill 13 DEX-DEX) NUNCA debe poder leer la propiedad `.privateKey` directa; sólo puede pasar un hash a la función `signTransaction()` del Módulo Custodio. Si un atacante inyecta código, no puede volcar las llaves por consola, solo enviar firmas.
5. Inactivación Remota (Remote Revocation): El Administrador puede desactivar un Secret Manager Master desde AWS. El Bot, que realiza Heartbeats de Auth cada hora, al no poder renovar, entra en Pánico, cierra inventarios y hace `process.exit()` eliminando toda clave en RAM y borrando la memoria (Scrubbing).
6. Split-Key Generation (Opcional Multipar/Threshold): Uso de firmas de Computación Multipartita (MPC) o firmas KMS aisladas donde el bot NUNCA ve la clave privada, sino que AWS KMS firma remotamente el payload de la EVM o se emite para una Smart Contract Wallet local.
7. Cifrado Interno In-Memory: Mantener las llaves cifradas con AES en RAM usando una llave maestra en el código, y desencriptar un milisegundo antes de firmar la orden HMAC para Binance, borrando la variable resultante inmediatamente, para dificultar Memory Dumps maliciosos de RAM (Scraping de C).
8. Detección de Compromiso del Repo: Evitar que alguien haga push al repositorio Git por error de una `.env` con las claves maestras usando ganchos (Hooks) pre-commit agresivos en el CI/CD, o alertas en el boot si el entorno huele a texto plano de riesgo.
9. Separación de Roles (Least Privilege): Cargar solo `READ_ONLY` API keys en los módulos de Inventario y OrderBooks, y cargar `TRADE_ONLY` keys con Withdrawals (Retiros) desactivados en el Orquestador HFT. Las Keys con Withdraw activado van segregadas en un entorno de Cold Sweep protegido por IP específica.
10. Hardware Enclave Support (Ej. AWS Nitro Enclaves): Soporte preparativo para ejecutar el firmador dentro de un enclave inaccesible incluso para el usuario "root" de Linux de la máquina virtual.

## 4. Entradas requeridas
- `iam_credentials` / `vault_tokens`: Credenciales inofensivas/temporales para acceder a los secretos maestros en la nube.
- `unsigned_transaction`: Payload raw HTTP (para CEX) o Raw Tx Hex (para DEX) que necesita firma criptográfica.
- `venue_identifier`: (Binance, Arbitrum, etc.) para aplicar la clave correcta.

## 5. Salidas esperadas
- `signed_payload`: Transacción HMAC (CEX) o ECDSA (L1/L2) inyectable a la red.
- `key_health_status`: Alertas en caso de que una API Key expire pronto (90-day CEX mandatory rotations).
- `access_audit_log`: Registro inmutable (Skill 37) de QUIÉN y PARA QUÉ pidió firmar una transacción (Ej. "Arbitraje 212 firmó para extraer $15k en Uniswap").

## 6. Reglas inmutables
- JAMÁS guardar archivos `.env` en producción en disco duro o pasar claves privadas mediante argumentos CLI explícitos (ej. `node index.js --key 0x123...`). Esos vectores exponen las claves a comandos como `ps aux` visibles por cualquier usuario del sistema operativo.
- El módulo que posee la Private Key (Skill 45) NUNCA debe tener lógica de toma de decisión de Trading. Sólo acepta comandos estrictamente estructurados, audita, firma y devuelve. (Separation of Concerns).
- Las API Keys del exchange deben configurarse desde la consola de Binance/OKX obligatoriamente asociadas a la IP Fija de AWS/VPS (`IP Whitelisting`). Así, si las API Keys se filtran en texto plano en Pastebin, son inútiles para los hackers.

## 7. Algoritmos o métodos que debe conocer
- ECDSA Secp256k1 & Keccak256 Hashing.
- HMAC-SHA256 y HMAC-SHA512.
- Memory Encryption & Garbage Collection manipulation (para sobreescribir bytes en 0 en lugar de esperar la recolección natural, si el lenguaje subyacente -Rust/C++- lo permite. En JS usar buffers atómicos).

## 8. Fórmulas críticas
- **CEX Signature (Binance HMAC)**: `HMAC_SHA256(Secret_Key, "timestamp=123&symbol=BTC&side=BUY")`
- **EVM Signed Tx**: `RLP_Encode(Nonce, GasPrice, GasLimit, To, Value, Data, V, R, S)`

## 9. Casos extremos
- Compromiso Total del Sistema Operativo de la VPS (Root Escallation Exploit): El atacante gana permisos de Root en la máquina EC2 y hace un "Memory Dump" del proceso Node/Rust. Si el bot guarda las variables secretas planas en el objeto `process.env`, el atacante saca los millones del fondo. Solución: Cifrado en RAM y descifrado "Just in time", minimizando la ventana de bytes expuestos.
- Expiración de Certificados / API CEX: Binance avisa por mail que por seguridad rotará obligatoriamente todas las llaves creadas hace >90 días en 24 horas. El bot, mediante API de cuenta, debe poder verificar la salud y edad de su llave.
- Desync de Time-Server local vs CEX: La firma HMAC requiere que el Timestamp del Payload (String concatenado) y la firma secreta lleguen en un tiempo exacto (RecvWindow 5000ms). Si el reloj difiere (Ver Skill 34), la firma criptográfica es validada criptográficamente por Binance, pero escupirá error `Outside RecvWindow`. El módulo inyecta la hora maestra compensada pre-firma.

## 10. Validaciones obligatorias
- PRE: Validar que el Orquestador maestro esté en Estado Operativo `GREEN` o que la orden sea mandatoria. (No firmar órdenes falsas en estado HALT).
- CÁLCULO: Validar el tamaño del buffer que aloja la firma, limpiar los Arrays de Bytes (`crypto.randomFillSync()` con ceros) de los secretos efímeros una vez derivados o usados.
- POST: Validar la expiración cruzada. Evitar enviar al Mempool transacciones o llamadas REST asíncronas con firmas que vencerán antes de tocar el enrutador enemigo.

## 11. Criterios de aprobación
- Las transacciones REST / WSS (Websocket Login `listenKey`) / EVM devuelven siempre 200 OK de Autenticación sin excepciones misteriosas.
- La latencia del firmado Local / Cifrado toma < 0.2ms. No usa KMS externo cada segundo para evitar el RTT (Lag) asfixiante de AWS.

## 12. Criterios de rechazo
- El sistema de despliegue en CI/CD o el arranque falla al contactar el KMS o el Secret Engine (Falla Módulos y se niega a abrir operativa, abortando).
- Los "Sanitizers" de Logs detectan una estructura expuesta de clave (Falla forense interna) y matan el proceso para prevenir filtración por CloudWatch/Datadog.

## 13. Riesgos que mitiga
- Riesgo Terminal de Robo Interno / Externo: Evita que ingenieros junior que monitorean los logs en Grafana/Datadog vean claves maestras de retiros CEX. El bot solo emite resultados numéricos, pero el core guarda el tesoro. Es la diferencia principal entre un "Proyecto Hobby de Arbitraje" (Que deja la semilla `.env` expuesta en GitHub y quiebra al día 2) y un "Software Hedge-Fund Grade".
- Riesgo de Ban Cruzado: Distribuye la firma a través de Nllaves API distintas para engañar y aliviar la presión de métricas de uso pesado en los exchanges si es legal y necesario.

## 14. Integración con otras skills
- Proporciona el Payload Final a Rate Limit Bypass (Skill 35) que lo emite por red.
- Funciona como extensión segura de Auto-Rebalanceo (Skill 42) y Profit Extraction (Skill 43).

## 15. Modelo de datos sugerido
```json
{
  "KeyManagerStatus": {
    "module": "SECURE_SIGNER",
    "vault_status": "AUTHENTICATED",
    "active_key_pools": {
      "binance": { "keys_loaded": 3, "status": "HEALTHY" },
      "evm_signers": { "keys_loaded": 1, "network": "MAINNET" }
    },
    "last_vault_sync_ms": 1714521234105,
    "signatures_produced_last_hour": 154020,
    "security_lockdown_triggered": false
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase `WalletCustodyManager` In-Memory Singleton. Solo posee funciones puras de Input/Output: `signHttpExchangeRequest(exchangeId, payloadString)` y `signEvmTransaction(chainId, rawTx)`.

## 17. Logs obligatorios
- `[DEBUG] Payload signed successfully for Bybit Account A. Auth header attached.`
- `[INFO] HashiCorp Vault sync completed. Renewed 2 API Keys successfully.`
- `[CRITICAL] UNAUTHORIZED SIGNATURE ATTEMPT DETECTED! Module X attempted to sign a transaction with destination outside of Whitelist. Rejecting and sending Admin Alert.`

## 18. Métricas obligatorias
- `total_signatures_performed_sec`.
- `key_rotation_events_month`.
- `failed_auth_responses_count` (Si Binance te rechaza la firma, es posible que revocaron tu API Key, forzando a rotar inmediatamente).

## 19. Tests unitarios
- Log Sanitizer Validation: Inyectar una instrucción `log.info("Checking config", { pKey: "0xabc123456789..." })`. El módulo proxy de log debe emitir `Checking config { pKey: "[REDACTED_SECRET]" }` asegurando nula filtración accidental.
- HMAC Math Precision: Alimentar la función con un Secret de test ("my_secret") y un Query String ("timestamp=100"). Debe generar exactamente la cadena HASH en Hex idéntica al Standard oficial de la documentación de Binance para que no haya errores `Invalid Signature`.
- EIP-155 Replay Protection: Validar que el firmador de EVM L1 adhiere estrictamente el `chainId` en el payload RLP de firma, previniendo que la transacción firmada para Arbitrum pueda ser robada (Replay Attack) en Mainnet Ethereum donde el usuario también tiene dinero.

## 20. Tests de integración
- Levantar servidor de AWS KMS simulado (LocalStack). El bot debe arrancar, solicitar IAM Creds, recuperar JSON protegido de las claves cifradas, descifrarlas en RAM, instanciar los firmadores y borrar el string JSON de origen con buffers cero.

## 21. Tests E2E
- El bot principal arranca con `NODE_ENV=production`. Todo el portafolio (millones) requiere operar. La orden de rebalanceo L1 exige firmar on-chain y mandar dinero al exchange. La Skill 45 verifica en O(1) la "Policy" interna, firma el bytecode de Solidity a ciegas usando `ethers.Wallet` aislada, y devuelve el `0xTxSigned`. El Agente lo inyecta a la red pública (RPC Skill 21). El Trade ocurre. Durante todo el proceso, ningún desarrollador con acceso SSH de lectura pudo encontrar las claves privadas en el disco VPS ni en la RAM superficial.

## 22. Checklist de producción
- [ ] Incorporación en entorno Linux VPS del almacenamiento de Ramdisk Efímero (`/dev/shm`) si se usa un binario que exige leer la llave en archivo temporal, y destruir ese archivo en el Shutdown hook (ej. `process.on('SIGTERM')`).
- [ ] Reglas WAF (Web Application Firewall): El nodo que guarda el Vault de HashiCorp / AWS IAM solo debe dejar pasar Peticiones GET desde la IP específica y estática del Servidor VPS que corre el Bot, denegando toda comunicación del resto de internet (Bloqueo en CAPA 3 de Red).
- [ ] Aplicar "Revocación Mágica" en Exchanges (Binance API). Si ocurre un desastre con AWS, llamar al Endpoint de Binance que aniquila inmediatamente las API keys usando otra llave de pánico separada del circuito general de Arbitraje.

## 23. Ejemplo de configuración no hardcodeada
```yaml
security_and_custody:
  secrets_manager_provider: "aws_kms"  # "hashicorp_vault", "local_env_encrypted"
  kms_region: "us-east-1"
  enable_in_memory_encryption: true    # Encrypts the raw private key in RAM
  rotate_keys_automatically_days: 85   # Binance forces 90 days usually
  log_sanitization_strict_mode: true   # Overrides core console.log to redact 64-char strings
```

## 24. Ejemplo de pseudocódigo
```javascript
class CustodySignerManager {
    constructor() {
        this.masterAesKey = generateRuntimeEntropy(); // Ephemeral internal encryption
        this.encryptedCexKeys = new Map();
        this.encryptedEvmWallets = new Map();
    }

    async bootAndLoadFromKMS(kmsClient) {
        // Retrieve payload from KMS, decrypts to string in memory temporarily
        const rawSecrets = await kmsClient.fetchSecret('arbitragex/production_keys');
        
        // Encrypt with local ephemeral master key to avoid clear-text RAM scraping
        for (let [exchange, config] of Object.entries(rawSecrets.cex)) {
            this.encryptedCexKeys.set(exchange, aesEncrypt(config.apiSecret, this.masterAesKey));
        }
        for (let [network, pKey] of Object.entries(rawSecrets.evm)) {
            this.encryptedEvmWallets.set(network, aesEncrypt(pKey, this.masterAesKey));
        }
        
        // Zero-fill original strings using Node Buffer manipulation (Garbage Collect safety)
        zeroFillObject(rawSecrets);
    }

    // High performance pure math signature func O(1)
    signHmacPayload(exchange, queryString) {
        const encryptedSecret = this.encryptedCexKeys.get(exchange);
        if (!encryptedSecret) throw new Error("Key not loaded for exchange");
        
        // Decrypt just-in-time
        const rawSecretBuffer = aesDecrypt(encryptedSecret, this.masterAesKey);
        
        // Perform HMAC
        const signature = crypto.createHmac('sha256', rawSecretBuffer).update(queryString).digest('hex');
        
        // Zero out the buffer instantly
        crypto.randomFillSync(rawSecretBuffer); 
        
        return signature;
    }
}
```

## 25. Criterio final de excelencia
La gestión dinámica de llaves convierte al Agente Supremo en una Fortaleza Paranoica de Grado Bancario ("Bank-Grade Fort-Knox"). Otorga la capacidad de disparar miles de balas financieras por segundo confiando ciegamente en que las pólvoras criptográficas subyacentes son indescifrables, invisibles y temporales para cualquier vector de ataque interno o externo.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Robo del Master IAM Token local. Si alguien vulnera el OS del VPS, asume la "Identidad" de la máquina y AWS le permite bajar las llaves maestras en un comando. (Solucionado aislando toda la máquina a IP Whitelist a nivel AWS/Exchange).
- Dependencias: Integración con Infraestructura de Cloud Pública o Privada (Vault).
- Próxima skill: Orquestador de simulaciones y Backtester interno (Skill 46).
