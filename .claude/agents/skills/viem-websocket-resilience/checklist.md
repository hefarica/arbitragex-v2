# Checklist

- [ ] **Requisitos previos**: Actualizar `viem` a la última versión estable (ideal > 2.18.x) si es posible.
- [ ] **Validaciones**: Verificar si los datos en la UI se actualizan al reconectar el internet.
- [ ] **Seguridad**: Asegurar que las API keys de WSS en Frontend estén protegidas por CORS.
- [ ] **Testing**: Test E2E donde el navegador pierde conexión y la recupera.
- [ ] **Observabilidad**: Console.log o Sentry error si la reconexión automática falla repetidamente.
- [ ] **Riesgo**: Recargar toda la página web (`window.location.reload()`) es una mala UX, usar reseteo de estado interno.
- [ ] **Criterios de aceptación**: Frontend muestra estatus de error de red sin tener memory leaks.
