# Integración del ZIP en GitHub

La integración parte de `4edf8405292b9598c3d66a20c3b05b89e66a000f`, que incorpora
la PR #559. Conserva sus gates por SHA y las correcciones de migración y despliegue.

Se compararon los 60 archivos de código de los manifiestos de alta topología y
continuación contra su base anterior a ambos parches. No había cambios de upstream
en conflicto. Se incorporan los archivos de código y los informes de cobertura,
sin sustituir el repositorio completo por una copia antigua del ZIP.

El ZIP de procedencia tiene SHA-256
`bd87f44b42e1fa71e16ae22bc1c83eac81f0f9e29bbfe1bea6771e399fb77b46`.
Los manifiestos y logs de las dos fases son evidencia histórica del ZIP; los
ajustes de Clippy de esta integración no están incluidos en sus hashes originales.
Las validaciones actuales corresponden al SHA de la PR y sus runs de Actions.

## Permisos y ejecución

Los candidatos de análisis conservan su información y sus permisos de simulación.
Pueden entrar al flujo de ejecución, donde el plan se valida nuevamente con el
firmante, contratos, financiación, gas y estado real. Un rol ausente produce
`missing_onchain_role` con contrato y beneficiario. Un permiso añadido en memoria
no modifica los roles del contrato desplegado.

La simulación atómica conectada soporta routers V2 compatibles y una o dos
agrupaciones contiguas de routers. Las demás superficies requieren sus adaptadores,
según `CONTINUACION_ALTA_TOPOLOGIA.md`. No se han concedido roles ni cambiado
credenciales o activadores live en esta integración.

## Regresión de Actions observada tras la primera fusión

El run [34600625472](https://github.com/hefarica/arbitragex-v2/actions/runs/34600625472)
falló con `playwright: not found` en el job de cartuchos. Instalaba dependencias
de la raíz, aunque E2E tiene un paquete y lockfile separados. Además, ese job no
arrancaba API/frontend y descartaba el resultado de Playwright con `|| true`.
El autodeploy de `4edf840` se detuvo antes de SSH, como exige el nuevo gate.

Ahora el job de cartuchos llama al workflow E2E del mismo commit: usa su lockfile,
levanta la aplicación, aplica migraciones y ejecuta pruebas bloqueantes. Los cambios
del propio workflow también disparan la validación en PR. Las pruebas que exigen
RPC y servicios externos conservan sus exclusiones explícitas; no se presentan
como pruebas de integración productiva aprobadas.

La reutilización emplea `workflow_call` y `jobs.<id>.uses`, según la
[documentación de GitHub](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows).

En la PR, el run de simulador `34601918255` aprobó tests y árbol de dependencias,
pero falló al publicar resultados porque la API productiva no respondía en el
puerto 8080. Esa dependencia impedía desplegar una recuperación de la propia API.
Ahora CI conserva los dos payloads como artefactos obligatorios, con SHA y run de
origen; el autodeploy los valida y publica después de comprobar la salud del VPS.
Los tests fallidos, artefactos ausentes o evidencia de otro SHA siguen fallando.
Un fallo de publicación después del despliegue también deja el workflow en rojo.
Las PR no publican resultados en el registro productivo. Cuatro regresiones Python
comprueban identidad, origen y rechazo de intentos obsoletos o fallidos.

Los gates Rust incluyen también `--bin relays-client` para ejecutar las pruebas de
admisión y contabilidad que el comando heredado `--lib` dejaba fuera. La ejecución
local completa aprobó 1.940 tests, con cuatro integraciones externas ignoradas.

## Límites de la evidencia

Los resultados locales y de CI no acreditan ingresos, todos los 264 ejecutores,
transacciones rentables ni el SLA completo. El despliegue solo puede declararse
terminado después de pasar los gates del commit fusionado y comprobar en el VPS
el SHA y la salud de los servicios.

El script heredado `lint-no-hardcode.sh` devuelve código cero incluso cuando
enumera infracciones; por tanto, su estado verde es informativo y no certifica
ausencia de valores hardcodeados. Esta integración no cambia esa política ni
oculta sus mensajes. Los logs históricos también conservan su formato original.
