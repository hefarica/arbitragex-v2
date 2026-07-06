# Checklist

- [ ] **Requisitos previos**: Tener cuenta de Alchemy y variables en ENV.
- [ ] **Validaciones**: Verificar que el proveedor retorna `chainId` esperado.
- [ ] **Seguridad**: Validar que la API key no se imprime en los logs de error (enmascaramiento).
- [ ] **Testing**: Simular HTTP 429 con mock server y verificar que la app aplica backoff.
- [ ] **Observabilidad**: Emitir métricas de desconexiones WSS y latencia de respuestas.
- [ ] **Riesgo**: Mitigar `stale reads` al hacer transacciones secuenciales.
- [ ] **Criterios de aceptación**: El sistema soporta una desconexión de red de 30 segundos y vuelve a escuchar el mempool sin intervención manual.
