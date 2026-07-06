# Checklist

- [ ] **Requisitos previos**: Definir `ARBX_PAPER_TRADE` en `.env`.
- [ ] **Validaciones**: Probar un log con una clave privada falsa y verificar que el redactor funcione.
- [ ] **Seguridad**: Verificar que el botón de Kill Switch requiere un token de admin o red privada.
- [ ] **Testing**: Testear el Circuit Breaker con respuestas falsas del RPC (ej. latencia inyectada).
- [ ] **Observabilidad**: Dashboard central configurado y apuntando a los logs estructurados.
- [ ] **Riesgo**: Que el sistema de logs se llene el disco (implementar log rotation).
- [ ] **Criterios de aceptación**: Toda ejecución debe ser auditada y enmascarar llaves privadas o secrets.
