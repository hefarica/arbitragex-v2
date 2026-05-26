# [CORE] Arquitectura de Sincronización de Estado (HFT)
**Componente:** ArbitrageX-v2 Core Engine
**Dominio:** High-Frequency State Reconciliation

### 1. TOPOLOGÍA DEL SISTEMA
El sistema es un motor de latencia ultrabaja diseñado para leer deltas de estado en el mempool y emitir transacciones de corrección topológica. Opera bajo un modelo de ejecución condicional estricto: ninguna transacción se emite al validador (ePBS) si el diferencial termodinámico ($\Delta_{profit}$) no es estrictamente mayor que el costo base de ejecución ($\Delta_{gas}$).

### 2. COMPONENTES DEL HOT-PATH (RUST)
* **Ingesta (searcher-rs):** Escucha eventos de transacciones pendientes en nodos locales (Geth/Reth).
* **Simulador de Estado (revm-engine):** Recrea la máquina virtual de Ethereum en memoria aislando el estado actual ($S_0$). Aplica la transacción pendiente para proyectar el estado futuro ($S_1$).
* **Optimizador de Ruta (math-engine):** Calcula el vector de nivelación necesario para estabilizar $S_1$ basándose en las reservas de los Automated Market Makers (AMMs).
* **Ejecutor (Yul Assembly):** Contratos inteligentes desprovistos de abstracciones de Solidity. Optimizados a nivel de *opcode* para garantizar un consumo mínimo de gas y un *revert* de bajo costo si la condición atómica falla.