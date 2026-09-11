# Revisión de GitHub Actions y autodeploy — 2026-09-11

Base: main `53740115e700d67adb97f65ca8b5adbb485a1cc6`.

## Evidencia del incidente

Los 15 workflows de validación del último push terminaron en success. El autodeploy falló después de E2E, en la migración 103, con `canceling statement due to lock timeout` al ejecutar `ALTER TABLE scored_opportunities ADD COLUMN IF NOT EXISTS ...`.

- Ejecución: https://github.com/hefarica/arbitragex-v2/actions/runs/34436938352
- Job: https://github.com/hefarica/arbitragex-v2/actions/runs/34436938352/job/102746361037
- PostgreSQL documenta que ALTER TABLE adquiere ACCESS EXCLUSIVE salvo excepción explícita: https://www.postgresql.org/docs/15/sql-altertable.html

Los logs también muestran `numeric field overflow` en escrituras de opportunities. Es un problema de datos/unidades adicional; esta corrección de CI no inventa ni amplía importes para ocultarlo.

## Cambios

- El autodeploy exige los 15 workflows permanentes actuales y el nuevo gate de regresión de despliegue. Comprueba SHA, rama main, evento push, repositorio, último run/intento y conclusión success. Espera ausentes/pendientes y falla ante errores/cancelaciones. Los workflows adicionales por paths que se hayan iniciado también bloquean si fallan.
- Consulta paginada por SHA; verifica de nuevo justo antes de SSH. Un commit superado por main no despliega ni hace reset del VPS.
- Dispatch manual del autodeploy también requiere gates. Los cartuchos conservan sus pruebas en push, pero el despliegue automático queda en una sola ruta. El despliegue manual de cartuchos comparte exclusión y gates.
- E2E vuelve a ser bloqueante en PR; se retira el comando que ocultaba cualquier fallo de hotpath. Los skips explícitos por infraestructura en los propios tests siguen siendo skips: no acreditan integración live.
- SSH transporta secretos mediante variables de entorno, admite VPS_PORT y tiene límites de conexión. Conserva el método heredado de descubrimiento de host key; no se han configurado credenciales nuevas.
- El autodeploy requiere .env de producción existente. Builds secuenciales con --no-cache y --env-file conforme a AGENTS.md; presupuesto de deploy 120 minutos. El consumo real de disco/tiempo debe verificarse en el VPS.
- Migración 103 consulta catálogo y verifica tipos antes de DDL; un replay con columnas correctas no toma bloqueo exclusivo sobre scored_opportunities. Si falta una columna, conserva el bloqueo y la transacción real; un tipo incorrecto falla y revierte.
- Sólo la migración 103, que es transaccional, reintenta hasta tres veces errores de bloqueo/deadlock. Otros errores fallan inmediatamente. Los logs son temporales privados y el diagnóstico publica estados/PID/bloqueadores sin texto SQL de sesiones.

## Validación

8 pruebas de selección de gates aprobadas localmente. YAML de workflows y sintaxis Bash del autodeploy/migrador comprobadas. El nuevo workflow ejecuta cuatro pruebas sobre PostgreSQL 15 real: instalación/parcial, tipos incorrectos con rollback, replay bajo escritor concurrente y bloqueo real cuando faltan columnas.

Primera ejecución de PR #559: build/typecheck/Rust/contratos e integración aprobados; las cuatro regresiones de PostgreSQL 15 también pasaron. E2E terminó en verde, pero el log reveló una colisión de selector en killswitch.spec.ts (estado DISABLED y confirmación disabled). Se corrigió con selección exacta del título/valor de estado; se retiró el catch que convertía un fallo de armado en skip. Se añaden tres repeticiones del round-trip sin reintentos sobre el mismo stack real de CI. La PR vuelve a validar el commit corregido antes de fusionar.

E2E mantiene exclusiones explícitas de pruebas live o infraestructura ausente; un workflow success no certifica esas rutas excluidas.

## Alcance y pendientes

Esta PR contiene la corrección del despliegue sobre main actual. No incorpora los cambios de motor entregados en `ArbitrageX_v2_Alta_Topologia.zip`; esa integración necesita su propia comparación y validación antes de desplegarse.

No se ha modificado main ni desplegado producción desde esta revisión. No se han alterado permisos de simulación, roles on-chain, configuración live, secretos ni reglas de protección de rama.

Los workflows manuales heredados de recuperación/deploy distintos de autodeploy y cartuchos siguen disponibles. No se ha certificado su equivalencia con el gate nuevo. El migrador heredado aún reproduce otras migraciones: la corrección acota el incidente 103, no constituye una migración completa a un ledger con checksums. Un bloqueo persistente al añadir columnas nuevas requiere revisar las sesiones identificadas, no eliminar el gate ni cancelar sesiones arbitrariamente.

Las validaciones CI no demuestran que el VPS ya ejecute la versión nueva. La promoción requiere el deploy y sus comprobaciones posteriores. El self-heal heredado no es un rollback transaccional de imágenes/base de datos.
