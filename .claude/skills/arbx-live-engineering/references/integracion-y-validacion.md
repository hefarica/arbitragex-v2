# Integración, CI y validación observada

## Trabajar sobre el sistema existente

Inspecciona repo, rama, HEAD, cambios locales y sus instrucciones antes de cambiar nada.
Identifica build scripts, módulos, esquemas y tests de cada flujo. No presupongas que
las rutas o símbolos de un informe antiguo siguen existiendo ni que un PR abierto
está desplegado. La descripción de una PR no reemplaza sus resultados de pruebas.

Conserva el código no publicado del equipo autorizado. Registra original y destino,
SHA o hash de archivo y pruebas asociadas. Para integración amplia utiliza comparación
base + delta; las regresiones silenciosas pueden compilar sin referencias huérfanas.

Un hallazgo no es una orden automática. Verifícalo contra el código actual; registra
CONFIRMADO, RESUELTO, NO_APLICA_CON_EVIDENCIA, NO_REPRODUCIDO o PENDIENTE, con razón.
No marques cerrado un HIGH porque desapareció del grep o se omitió su archivo.

## Edición de instrucciones del proyecto

Lee primero el `CLAUDE.md` efectivo y los archivos `.claude` relacionados. Añade una
extensión coherente con la decisión del operador utilizando la plantilla proporcionada.
Mantén referencias a la política anterior y explica el cambio de producto: soporte
mainnet explícito en vez de veto perpetuo; control financiero del operador conservado.

No copies una lista de “allowed tools” amplia para saltarte confirmaciones. En Claude
Code, las herramientas y archivos están sujetos a reglas efectivas de permisos [S2].
Cambiar instrucciones de ingeniería no modifica esas reglas de manera mágica.

## Estrategia de validación

Descubre comandos reales de package.json, Cargo, Foundry y configuración Playwright.
Evita actualizaciones incidentales de versiones. Usa las versiones fijadas y lockfiles.
Prueba tanto los módulos afectados como el flujo integrado y regresiones de seguridad.

Ejemplos de clases de prueba, no afirmaciones de ejecución:

- Formato/lint/typecheck/build, unitarias y propiedades matemáticas.
- Contratos y adaptadores con casos positivos/negativos y cantidades exactas.
- PostgreSQL y Redis reales aislados, sin nombres ni mounts de producción.
- API e integración con configuración real del entorno de prueba.
- Navegador con controles, errores, reconexión y relectura del estado persistido.
- Testnet identificada sin valor, más simulaciones/forks mainnet sin envío.

No cambies la regla zero-mocks productiva por una prohibición de fixtures de tests:
una fixture declarada prueba lógica, pero nunca prueba disponibilidad de producción.
No uses esa fixture como evidencia de patrimonio, rentabilidad, reservas o ejecución live.

## Desktop Commander y Playwright

Descubre herramientas MCP y usa la terminal de la máquina autorizada para levantar
los procesos de prueba. Playwright ejecuta pruebas en navegadores reales y genera
informes/trazas según la configuración elegida [S5]. Guardar un script sin correrlo
no es E2E. Una captura sola no demuestra que una mutación llegó al backend.

Si Playwright ya está instalado y configurado, usa su runner local fijado; verifica
que `npx` no vaya a descargar una versión inesperada. No ejecutes init sobre el repo
para sobrescribir config existente. Instalar navegador o dependencias es escritura
local autorizable, no una acción válida durante read-only del VPS.

Por test registra entorno y datos, acción esperada, observada, aserciones, SHA, versiones,
exit code y evidencias. Prueba registro/edición de redes, DEX, factories y pools,
validaciones de dirección/cadena, permisos admin, filtros/paginación, conexión WS,
fallos de RPC/DB, reinicio y recuperación. Los toggles financieros reales no se activan
con un robot de UI como parte de estos tests.

## Gates de despliegue

Los gates requeridos deben terminar exitosamente sobre el SHA que se va a desplegar.
Un resultado skipped/cancelled/pending no es aprobado. No escondas fallos con
`continue-on-error`, `|| true`, allow_failure ni salidas cero fabricadas en checks críticos.

Confirma la precedencia de configuración y la identidad del artefacto: SHA de código,
run de CI, digest/ID de imagen y evidencia runtime deben concordar. `git rev-parse`
por sí solo solo describe el checkout, no el binario que atiende solicitudes.

Una vez mapeado y autorizado el VPS, utiliza el flujo canónico, el lock compartido y
verificaciones previas de capacidad/persistencia. Conserva imágenes y configuración
necesarias para revertir. El rollback de código no deshace automáticamente migraciones
ni transacciones blockchain: documenta sus fronteras antes de ejecutar.

## Estados de informe

`PASS`: aserción ejecutada y satisfecha. `FAIL`: ejecutada y fallida. `NOT_RUN`: no se
lanzó. `BLOCKED`: falta un requisito. `SKIPPED`: el runner la omitió. `PENDING`: falta
resultado terminal. Reporta los conteos separados, no metas todo dentro de “verificado”.

No afirmes que la DApp está lista para operar por haber validado el instalador de esta
skill o un manifiesto. Cada capa tiene evidencia propia. Usa la plantilla de pruebas.

Fuentes S2 y S5: [procedencia y fuentes](procedencia-y-fuentes.md).
