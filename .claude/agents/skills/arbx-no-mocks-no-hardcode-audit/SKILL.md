---
name: arbx-no-mocks-no-hardcode-audit
description: Auditar que una solución no metió mocks, datos falsos o hardcodes peligrosos.
---
# arbx-no-mocks-no-hardcode-audit

## Purpose
Asegurar inamovible y algorítmicamente la pureza y fidelidad empírica del código base, verificando rigurosamente la inexistencia de datos estáticos engañosos o "mockeos" perversos en capas productivas, forzando la cultura Zero-Mocks y Fail-Honest.

## When to use
Debe invocarse obligatoriamente de forma sistemática al concluir cualquier cambio o rama en un archivo del repositorio, antes de dar un paso por completado, entregar el trabajo o empaquetar un commit.

## Inputs needed
- Ruta del espacio de trabajo en bash (para recorrer `src`, excluyendo `target` o `node_modules`).

## Files usually touched
- N/A. (Tool puramente pasiva y forense).

## Commands
- `grep -R "mock" -n backend/searcher-rs/src --exclude-dir=target && exit 1 || true`
- `grep -R "hardcode" -n backend/searcher-rs/src --exclude-dir=target && exit 1 || true`
- `grep -R "fake" -n backend/searcher-rs/src --exclude-dir=target && exit 1 || true`
- `grep -REn "(Math.random\(\)|dummy|fabricated|placeholder)" frontend/src/components || true`

## Safety rules
- El descubrimiento de tan solo un `mock` semántico o lógico en una ruta funcional que no esté dedicada a pruebas (`tests/`), invalida totalmente el esfuerzo y obliga a repensar.
- Identificar asignaciones de coeficientes directos asumidos (`profit = 0.5`) o fechas irreales (`161111111`) es intolerable.

## Verification steps
1. Ejecutar las herramientas de auditoría con Grep.
2. Hacer revisión de lectura (`sanity check`) manual sobre las inicializaciones de struct o estado que van hacia el endpoint público o a la base de datos PostgreSQL.
3. Observar variables sospechosas como `Date.now()` sin sustento referenciado real.

## Failure modes
- Ser complaciente al excluir las rutas correctas en grep y dar una falsa alarma (false negative).
- Ignorar los fallos de esta auditoría justificando que era "solo para visualizar la página".

## Golden output
Ejecución limpia del script bash, en donde no emerge ni una sola coincidencia de la familia léxica ("fake, mock, dummy, arbitrary") en el árbol productivo.

## Anti-patterns
- Decir: "Voy a mockear esto momentáneamente para chequear cómo se ven los estilos de TailwindCSS". La manera correcta es prever y manejar los estados vacíos y vacantes explícitos.
- Emitir en un payload un `{ total: 0 }` estático cuando el servicio inferior subyacente arrojó un TypeError que apagó el server.

## Example prompt
"Realiza el arbx-no-mocks-no-hardcode-audit exhaustivo con grep antes de cerrar la tarea para asegurar que tu contribución es completamente Fail-Honest en Searcher y API."
