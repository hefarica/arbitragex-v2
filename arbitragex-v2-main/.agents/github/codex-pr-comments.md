# OBLIGATORIO: CODEX BOT COMMENTS DIRECTIVE

REGLA INQUEBRANTABLE — CODEX BOT COMMENTS FIRST

Antes de tocar cualquier archivo, ejecutar cualquier fix, modificar workflows, editar migraciones, hacer commit, hacer push o declarar una tarea como resuelta, el agente debe sincronizar, leer, comprender, interpretar y atender ABSOLUTAMENTE TODOS los comentarios emitidos por:

chatgpt-codex-connector[bot]

Esto incluye:
1. Comentarios generales del PR.
2. Reviews del PR.
3. Inline review comments.
4. Comentarios por archivo y línea.
5. Comentarios P1/P2/P3.
6. Comentarios antiguos no resueltos.
7. Comentarios nuevos que aparezcan durante la sesión.
8. Comentarios asociados al commit actual y a commits anteriores del mismo PR.

Fuente obligatoria:
.agents/github/codex-pr-comments.md

Flujo obligatorio:
1. Ejecutar scripts/sync-codex-comments.sh antes de iniciar.
2. Leer completo .agents/github/codex-pr-comments.md.
3. Convertir cada comentario de chatgpt-codex-connector[bot] en una tarea trazable.
4. Clasificar por severidad: P1, P2, P3, INFO.
5. Priorizar P1 antes que cualquier otra tarea.
6. Para cada comentario, identificar:
   - archivo afectado,
   - línea afectada,
   - texto exacto del comentario,
   - causa raíz,
   - riesgo,
   - cambio requerido,
   - validación obligatoria,
   - estado.
7. Resolver o justificar cada comentario.
8. Si un comentario es falso positivo, demostrarlo con evidencia del código actual.
9. No hacer commit hasta que todos los comentarios Codex tengan estado:
   - RESUELTO,
   - FALSO_POSITIVO_JUSTIFICADO,
   - BLOQUEADO_CON_CAUSA,
   - PENDIENTE_POR_DEPENDENCIA_EXTERNA.
10. Después de modificar código, volver a ejecutar scripts/sync-codex-comments.sh para verificar que no apareció feedback nuevo.
11. Si aparece feedback nuevo, repetir el ciclo.
12. Nunca ignorar comentarios de chatgpt-codex-connector[bot].
13. Nunca cerrar un P1 con explicación cosmética.
14. Nunca silenciar tests, linters o guards para “resolver” un comentario.
15. Nunca declarar éxito sin validación ejecutada.

El agente debe tratar los comentarios de chatgpt-codex-connector[bot] como una cola de trabajo obligatoria y viva.

---

MODO CODEX BOT COMMENT RESOLVER — OBLIGATORIO

Antes de tocar el repo, ejecuta:

bash scripts/sync-codex-comments.sh hefarica/arbitragex-v2 103

Después lee completos estos archivos:

.agents/github/codex-pr-103-comments.md
.agents/github/codex-pr-103-tasks.md
.agents/github/codex-pr-103-comments.json

Tu misión no es “revisar comentarios”.
Tu misión es leer, comprender, interpretar, atender y resolver ABSOLUTAMENTE TODOS los comentarios emitidos por:

chatgpt-codex-connector[bot]

Debes tratar esos comentarios como órdenes técnicas de revisión que requieren triage, diagnóstico, fix, validación y evidencia.

Flujo obligatorio:

1. Sincroniza comentarios.
2. Lee todos los comentarios.
3. Extrae todos los P1, P2, P3 e INFO.
4. Crea una matriz viva de resolución.
5. Agrupa por archivo afectado.
6. Detecta duplicados.
7. Detecta comentarios que comparten causa raíz.
8. Prioriza P1.
9. Para cada comentario, responde internamente:
   - ¿Qué está señalando Codex?
   - ¿Qué archivo afecta?
   - ¿Qué línea afecta?
   - ¿Cuál es la causa raíz?
   - ¿Cuál es el riesgo real?
   - ¿Qué cambio mínimo corrige sin romper arquitectura?
   - ¿Qué validación demuestra la corrección?
   - ¿Hay comentarios relacionados?
10. Implementa fixes por lotes coherentes.
11. Ejecuta validaciones.
12. Actualiza .agents/github/codex-pr-103-tasks.md con estados.
13. Vuelve a sincronizar comentarios.
14. Si aparece nuevo feedback, repetir.
15. Solo después de resolver o justificar todo, preparar commit.

Estados permitidos:
- RESUELTO
- FALSO_POSITIVO_JUSTIFICADO
- BLOQUEADO_CON_CAUSA
- PENDIENTE_POR_DEPENDENCIA
- NO_APLICA_CON_EVIDENCIA

Prohibido:
- Ignorar P1.
- Resolver un comentario sin tocar el archivo afectado o justificarlo.
- Apagar tests para pasar CI.
- Cambiar guards de seguridad para ocultar fallas.
- Relajar lint sin autorización.
- Marcar como resuelto sin validación.
- Commitear sin re-sincronizar comentarios.
- Atender logs de CI ignorando comentarios Codex.
- Atender comentarios generales ignorando inline comments.
- Atender solo el último commit ignorando reviews anteriores del mismo PR.

Validación mínima por tipo:
- Workflows YAML: revisar sintaxis, rutas, secrets, working-directory, env, servicios.
- TypeScript: npm run typecheck --workspaces --if-present.
- Rust: cargo fmt --check, cargo clippy, cargo test desde el workspace correcto.
- SQL migrations: aplicar secuencia de migraciones en DB limpia y DB actualizada.
- Nginx/Docker: docker compose config, nginx -t si aplica.
- Security: gitleaks, no-hardcode, npm audit, cargo audit según aplique.
- Tests: ejecutar el test exacto afectado antes del test global.

Entrega obligatoria:
1. Total comentarios Codex leídos.
2. Total P1/P2/P3.
3. Tabla de comentarios por archivo.
4. Tabla de resolución.
5. Archivos modificados.
6. Validaciones ejecutadas.
7. Comentarios resueltos.
8. Comentarios pendientes y razón.
9. Riesgos.
10. Commit sugerido.

No declares éxito hasta demostrar que todos los comentarios de chatgpt-codex-connector[bot] fueron leídos, interpretados y atendidos.

---

ACTIVA MODO ABSOLUTE CODEX REVIEW COMPLIANCE.

Objetivo:
Que ningún comentario de chatgpt-codex-connector[bot] quede sin leer, interpretar, atender, validar o justificar.

Antes de cualquier acción:
1. Ejecuta scripts/sync-codex-comments.sh.
2. Lee .agents/github/codex-pr-103-comments.md.
3. Lee .agents/github/codex-pr-103-tasks.md.
4. Lee el JSON bruto.
5. Cuenta comentarios.
6. Cuenta P1/P2/P3.
7. Agrupa por archivo.
8. Genera plan de resolución.

Durante la tarea:
- Mantén abierta la matriz de comentarios.
- Marca cada comentario con estado.
- No cambies de tema hasta resolver todos los P1.
- No hagas fixes cosméticos.
- No ocultes fallas.
- No cambies doctrina para satisfacer CI.
- No elimines comentarios del código sin comprender por qué existían.
- No relajes gates de seguridad.

Después de cada fix:
- Ejecuta validación específica.
- Actualiza matriz.
- Re-sincroniza comentarios.
- Verifica que no aparecieron nuevos comentarios de Codex.

Regla final:
Si hay un solo comentario P1 de chatgpt-codex-connector[bot] sin estado final, el trabajo está incompleto.

Los comentarios de chatgpt-codex-connector[bot] no son ruido: son una cola técnica obligatoria. Cada comentario debe convertirse en una tarea con archivo, línea, causa raíz, fix, validación y estado. Si el agente no puede demostrar que leyó y atendió todos los comentarios Codex, no está autorizado a modificar, commitear ni declarar éxito.
