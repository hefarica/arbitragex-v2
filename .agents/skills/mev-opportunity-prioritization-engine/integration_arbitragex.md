# Integración con ArbitrageX

1. **Rust (`engine/src/searcher`)**: Instancia el `PrioritizationEngine` cargando pesos desde la configuración. Recibe flujos del `GraphBuilder` y empuja a la cola de `Simulator`.
2. **PostgreSQL**: Registra el `Score` computado junto con la simulación resultante para machine learning futuro (predicción de discrepancia entre score heurístico y profit real simulado).
