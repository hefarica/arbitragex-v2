# Checklist

- [ ] **Requisitos previos**: Acceso root al VPS o consola Cloud.
- [ ] **Validaciones**: Ejecutar test de latencia base antes de aplicar optimizaciones.
- [ ] **Seguridad**: Si se habilita `host network`, bloquear puertos no requeridos.
- [ ] **Testing**: Simular tráfico masivo de red y medir el jitter del proceso de Rust.
- [ ] **Observabilidad**: Monitorizar CPU steal time (en entornos de nube compartidos).
- [ ] **Riesgo**: Entender que el VPS puede quedarse sin conexión temporal si sysctl falla.
- [ ] **Criterios de aceptación**: El sistema de ejecución MEV reside en el mismo centro de datos que el validador o proveedor RPC.
