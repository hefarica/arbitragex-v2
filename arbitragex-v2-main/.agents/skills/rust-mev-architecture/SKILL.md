# Arquitectura MEV Bot en Rust

## Nivel
Nivel experto avanzado.

## Propósito
Diseñar y mantener la arquitectura interna de bots de extracción de valor (MEV) usando Rust, priorizando la concurrencia asíncrona, seguridad de la memoria y simulación local rápida para predecir rentabilidad antes del envío on-chain.

## Fuente de aprendizaje
https://pawelurbanek.com/rust-mev-bot

## Conocimiento interiorizado
- **Rendimiento Financiero**: El ecosistema MEV es altamente competitivo. Un bot básico (como triangular arbitrage en Uniswap V2) a menudo no es rentable ("unprofitable") porque bots más optimizados siempre ganan las subastas de bloque, o el costo del gas + bribes supera el beneficio.
- **Flujo de Ejecución Rápida**: Un MEV bot necesita decodificar transacciones en mempool instantáneamente, estimar el estado futuro, inyectar su propia transacción (ej. un sandwich o backrun) y enviarla al Flashbots Relay.
- **Rust y Concurrencia**: Rust es ideal porque `tokio` permite escuchar múltiples WebSockets (mempools, oráculos) y disparar tareas de cálculo en paralelo sin garbage collection pauses.
- **Smart Contracts Helper**: La lógica compleja on-chain se debe delegar a un contrato ejecutor propio (usualmente escrito en Solidity o Yul) que verifica los balances finales y hace un `revert` si el beneficio esperado no se alcanza (protección contra frontrunning del propio bot).

## Cuándo activar esta skill
- Al auditar o extender `backend/searcher-rs`.
- Al escribir lógica de decodificación de transacciones entrantes (ABI parsing).
- Al implementar lógica de puja (bidding) para el Flashbots relay.

## Cuándo no activar esta skill
- Tareas de frontend o dashboards operativos.
- Desarrollo de contratos inteligentes estándar (no orientados a MEV).

## Entradas necesarias
- ABIs de los pools DEX objetivo.
- Proveedor RPC local o de baja latencia.
- Definición de la estrategia matemática.

## Procedimiento paso a paso
1. Iniciar un proceso principal asíncrono con `tokio`.
2. Lanzar "Collectors" que escuchan el mempool vía WebSockets o nodos locales.
3. Decodificar la transacción y enviarla a un canal (MPSC channel).
4. El "Strategy Engine" procesa el evento, hace la simulación local del estado (Reth o EVM forked) y calcula beneficio.
5. Si `beneficio > costo_gas`, construir paquete (bundle) y enviarlo al "Executor" para su ruteo a constructores (builders).

## Salidas esperadas
- Archivos `.rs` optimizados y concurrentes.
- Tests unitarios de simulación local.

## Aplicación al proyecto actual
Se aplica directamente al desarrollo de los componentes internos de `searcher-rs` de ArbitrageX, estructurándolo para que no muera ante picos de red.

## Aplicación a futuros proyectos
Desarrollo de indexadores de datos on-chain de alta velocidad o bots de liquidación.

## Buenas prácticas
- Descartar rápido transacciones irrelevantes (filtros a nivel de topic/hash) para ahorrar CPU.
- Usar un contrato de fallback en Solidity que tenga el modifier `onlyOwner`.

## Errores comunes
- Fallar en el cálculo del gas, resultando en que la transacción pierde dinero (saldo en negativo).
- "Spamear" el RPC con cálculos de estado para cada transacción del mempool.

## Riesgos técnicos
- Deadlocks en canales MPSC en Rust si la cola se llena porque el motor de estrategia es más lento que el ritmo de entrada del mempool.

## Riesgos legales, éticos o financieros
- Sandbox de pruebas obligatorio. Ejecutar este tipo de bots sin `revert` condicional en el contrato quemará fondos reales (ETH) en gas fallido.
- Se debe configurar de manera defensiva, sin atacar usuarios normales (solo realizar arbitrajes saludables o liquidaciones autorizadas).

## Controles de seguridad
- Validar beneficio `> 0` en local y `> 0` dentro del smart contract.
- Utilizar testnets o "Paper Trading" (simulación) por defecto.
- Gestión estricta de claves: usar AWS KMS, HashiCorp Vault o enclaves seguros para firmar transacciones; nunca dejar `.env` con claves en texto claro en repositorios.

## Checklist operativo
- [ ] Canal de eventos implementado asíncronamente (Tokio MPSC).
- [ ] Simulador local de EVM configurado para estimar sin llamar al RPC externo.
- [ ] Contrato ejecutor tiene `require(balanceAfter > balanceBefore + gasCost)`.
- [ ] Variables de entorno protegidas para las *Private Keys*.

## Ejemplo seguro
Ver `examples.md`.

## Dependencias
- `tokio`, `ethers-rs` o `alloy`, `revm` (opcional, para simulación EVM rápida).

## Métricas de calidad
- Tiempo desde recepción de evento en el mempool hasta emisión de respuesta: <10ms.
- 0% de pérdidas de fondos por bugs matemáticos (garantizado por el smart contract).

## Criterios de finalización
- El bot puede ingestar el mempool de Mainnet en tiempo real sin saturar la CPU ni crashear.
