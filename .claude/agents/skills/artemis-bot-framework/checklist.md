# Checklist

- [ ] **Requisitos previos**: Instalar Tokio y familiaridad con `async_trait`.
- [ ] **Validaciones**: Testear las estrategias usando `std::sync::mpsc` local o un bus sincrónico simulado antes de ejecutar `Engine::run()`.
- [ ] **Seguridad**: Verificar que el estado interior de una Strategy no cambie concurrentemente sin candados (Locks).
- [ ] **Testing**: Escribir Unit Tests para cada Estrategia inyectándole un array falso de Eventos y verificando las Acciones salientes.
- [ ] **Observabilidad**: Añadir tracing en las uniones Collector->Estrategia y Estrategia->Executor para detectar cuellos de botella ("lag").
- [ ] **Riesgo**: Aislar el Executor real a nivel de configuración (feature flag `--paper-trade`).
- [ ] **Criterios de aceptación**: El sistema compila y el bot funciona simulando acciones al recibir transacciones.
