---
name: arbx-frontend-freeze-protocol
description: Reglas y protocolo inquebrantable para prevenir daños no autorizados al frontend productivo.
---

# FRONTEND FREEZE PROTOCOL

## Purpose
Garantizar la estabilidad visual y operativa del frontend productivo, evitando modificaciones, "mejoras" y rediseños no autorizados o innecesarios.

## When to use
SIEMPRE que una tarea implique o sugiera tocar el código del frontend (React, Next.js, Tailwind, componentes, vistas, etc.).

## Safety rules (Prohibiciones)
Queda estrictamente prohibido tocar frontend sin aprobación.
No se pueden modificar, mover, rediseñar o reinterpretar:
- `frontend/app/operations`
- `frontend/app/opportunities`
- `RuntimeStatusCards.tsx`
- `OperationsClient.tsx`
- `OpportunitiesClient.tsx`
- `page.tsx`
- `PipelineFunnelCard.tsx`
- `KPICard.tsx`
- Componentes UI compartidos, estilos globales, layouts, headers, cards, wrappers, colores, tipografía, espaciados, o navegación.

## Flujo antes de cambios
1. Mostrar el archivo exacto a modificar.
2. Explicar por qué es necesario.
3. Mostrar riesgo.
4. Esperar aprobación explícita del usuario.
5. Hacer cambio mínimo.
6. Mostrar diff.
7. Ejecutar build (`pnpm --filter frontend build` o `npm run build -w @arbx/frontend`).
8. Pedir aprobación antes de deploy.
9. Desplegar solo si el usuario aprueba.

## Anti-patterns
- Mover cards o reestructurar layout sin aprobación explícita.
- Rediseñar vistas existentes basándose en criterios estéticos propios.
- Cambiar colores o clases de Tailwind (ej. `text-success` a `bg-warning`) de componentes productivos.
- Tocar layout general.
- Desplegar código en VPS sin haber mostrado un diff y recibido aprobación.

## Verification steps
1. Validar que la solución no se pueda hacer puramente en backend/API.
2. Comprobar que cualquier edición propuesta en frontend tiene confirmación del usuario.
3. Compilación exitosa localmente antes de hacer commit.
