# Checklist

- [ ] **Requisitos previos**: Archivos ABI obtenidos de Etherscan/Sourcify.
- [ ] **Validaciones**: Verificar si los parseadores ABI extraen correctamente la dirección del token y los montos (AmountIn, AmountOutMin).
- [ ] **Seguridad**: Configurar Rust para fallar al momento (Fail-Fast) con `unwrap()` en entorno de test, pero recuperar con `match` en producción para evitar crasheos.
- [ ] **Testing**: Crear Mocks de eventos del Mempool para pasar al engine de simulación.
- [ ] **Observabilidad**: Uso extensivo de `tracing` en lugar de `println!`.
- [ ] **Riesgo**: Probar con alta concurrencia (`cargo test -- --test-threads=10`).
- [ ] **Criterios de aceptación**: Procesamiento de 5,000 transacciones/segundo simuladas sin pérdida de memoria.
