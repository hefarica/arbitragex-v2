Ejecuta el Procedimiento Obligatorio de Fixing (§11 CLAUDE.md) para el problema que el usuario describe.

Secuencia de 9 pasos (NO saltear ninguno):

1. **PAUSAR** — No tocar código aún. Entender el problema primero.
2. **REPRODUCIR** — Ejecutar el comando/test que muestra el error. Capturar output exacto.
3. **TRAZAR** — Seguir el flujo de datos desde el origen: searcher-rs → Redis → PG → API → Frontend. ¿Dónde se rompe?
4. **AUDITAR** — Revisar archivos relacionados. Buscar violaciones de RULE 00-04 y R1-R8.
5. **CORREGIR** — Aplicar el fix mínimo necesario. No refactorizar de más. YAGNI.
6. **COMPILAR** — `cargo check` (Rust) y/o `npx tsc --noEmit` (TS). Cero errores antes de continuar.
7. **DESPLEGAR** — Si aplica, seguir RULE 01 (LOCAL → GIT → VPS) + RULE 03 (--no-cache --env-file).
8. **VERIFICAR EN PRODUCCIÓN** — No en local. En el VPS real. Mostrar evidencia (curl, docker logs, SQL query).
9. **DOCUMENTAR** — Agregar el incidente a `.agents/memory/anti_reincidencia.md` si es un patrón nuevo.

Usa systematic-debugging de Superpowers: 4 fases de root-cause analysis.
Loop autónomo (OMEGA PROTOCOL) hasta que TODOS los pasos estén verdes.
