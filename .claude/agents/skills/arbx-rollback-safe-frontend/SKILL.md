---
name: arbx-rollback-safe-frontend
description: Protocolo para devolver el frontend a su último estado estable validado de manera segura y sin reescritura de historia.
---

# ROLLBACK SAFE FRONTEND

## Purpose
Garantizar un procedimiento de restauración controlado y determinista para revertir daños o cambios no autorizados en el frontend, sin destruir el historial de la rama main.

## When to use
Cuando el frontend sea modificado de forma invasiva, rompa compatibilidad, cause errores de build o viole el Frontend Freeze Protocol.

## Safety rules (Prohibiciones)
- NUNCA usar `git reset --hard` para volver atrás en la rama principal.
- NUNCA hacer un `force push` (`git push -f`) a `main`.

## Procedimiento de Rollback
1. Mostrar la historia reciente para identificar el commit:
   `git log --oneline -15`
2. Identificar el ÚLTIMO COMMIT BUENO (`GOOD_COMMIT`) antes de las modificaciones invasivas.
3. Restaurar exclusivamente la carpeta frontend desde ese commit:
   `git checkout "$GOOD_COMMIT" -- frontend`
4. Validar el estado limpio del directorio de trabajo y visualizar los cambios traídos:
   `git status`
   `git diff -- frontend`
5. Ejecutar la validación local estricta antes de realizar un nuevo commit:
   `pnpm --filter frontend build` o `npm run build -w @arbx/frontend`
6. Si compila, hacer el commit de restauración:
   `git add frontend`
   `git commit -m "revert(frontend): restore last stable UI state"`
7. Solicitar aprobación explícita antes de cualquier despliegue.

## Verification steps
1. El diff muestra únicamente reversiones de los archivos UI involucrados.
2. El build compila perfectamente con `Exit code: 0`.
3. Ningún archivo del backend, infra (.env, Docker) o scripts ha sido incluido en el rollback accidentalmente.
