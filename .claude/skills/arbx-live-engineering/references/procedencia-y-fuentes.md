# Procedencia y fuentes

## Qué es este paquete

Skill nueva, redactada para la solicitud del operador de ArbitrageX del 12 de septiembre
de 2026. Es un procedimiento de ingeniería y unas herramientas locales de apoyo, no
un diagnóstico actual ni una DApp implementada. Los criterios de cobertura y la ventana
local de evidencia son decisiones del paquete, no normas certificadas del proveedor.

## Antecedentes entregados por el operador

`arbx_auditoria_zip_2C3H_2026-09-11(1).txt`, líneas 5–15, aclara que la comparación fue
contra un working tree con cambios sin commit. Líneas 16–20 y 86–96 separan soporte
mainnet explícito, controles conservados y activación financiera del operador.
Líneas 103–156 identifican los fixes que no deben perderse al integrar.

`arbx_vps_diagnostico_2026-09-12(1).txt`, captura 16:00–16:11 UTC, líneas 26–45,
118–122 y 172–179, registra saturación del disco, SQL inaccesible y errores AOF.
Líneas 184–192 identifican alias/contexto, ruta de proyecto y lock de despliegue.
Estos datos justifican el orden read-only, pero NO describen necesariamente el presente.

El informe contiene inferencias que requieren comprobación; no eleva todas sus
conclusiones a hechos de esta skill. Tampoco se incluyen transcripciones crudas,
credenciales, archivos .env ni copias de los informes del usuario en el ZIP.

## Documentación primaria consultada el 12 de septiembre de 2026

[S1] Claude Code — Skills. Formato SKILL.md, ubicación de proyecto e invocación manual.
https://code.claude.com/docs/en/skills

[S2] Claude Code — Permissions. Las reglas se aplican por el cliente; el prompt no
sustituye su mecanismo de autorización.
https://code.claude.com/docs/en/permissions

[S3] Desktop Commander — Release v0.2.30. Documenta el comando remote y su autenticación.
Se cita como evidencia del comando, NO como afirmación de que esa sea la última versión.
https://github.com/wonderwhy-er/DesktopCommanderMCP/releases/tag/v0.2.30

[S4] Desktop Commander — Repositorio oficial/README. Herramientas de terminal y archivos,
límites de allowedDirectories y registro local de argumentos/resultados.
https://github.com/wonderwhy-er/DesktopCommanderMCP

[S5] Playwright — Installation/Running tests. Runner, navegadores y ejecución de tests.
https://playwright.dev/docs/intro

[S6] PostgreSQL 15 — psql. -X, ON_ERROR_STOP y solicitudes separadas con -c.
https://www.postgresql.org/docs/15/app-psql.html

[S7] PostgreSQL 15 — Routine Vacuuming. Reutilización de espacio y coste de reescritura.
https://www.postgresql.org/docs/15/routine-vacuuming.html

[S8] EIP-140 — REVERT. Reversión del estado, incluidos logs, distinta de debug traces.
https://eips.ethereum.org/EIPS/eip-140

No se afirma haber ejecutado Desktop Commander, SSH, PostgreSQL, Playwright ni live
por consultar estos documentos. La validación local del paquete se entrega por separado.
