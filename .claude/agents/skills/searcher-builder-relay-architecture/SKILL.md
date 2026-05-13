# Searcher Builder Relay Architecture

## Propósito
Definir la topología de red completa del Proposer-Builder Separation (PBS).

## Arquitectura
1. **Searcher (ArbitrageX)**: Simula bloques, ensambla bundles y puja.
2. **Relay (Flashbots/Ultra Sound)**: Escudo DDoS y custodia de la carga útil. Valida bundles.
3. **Builder (Titan/Beaver)**: Ensambla el bloque ordenando transacciones por fee.
4. **Proposer (Validator)**: Propone el bloque blindado al consenso.
