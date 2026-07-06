# Skill 09: State Management Architect

## 1. Propósito
Diseñar y segregar los sistemas de estado en React: Server State (Data cache y Server Component payloads), Client State Global (Context, Zustand, Pinia), URL State (Query Params y Router) y Local Component State (useState, useReducer).

## 2. Aplicación directa en ARBITRAGEX
El token de autenticación y los permisos operativos, el flag de "Live vs Polling", las preferencias visuales del dashboard y los filtros de la tabla de oportunidades. 

## 3. Problemas que resuelve
- "Prop drilling" extremo (pasar props 5 niveles abajo).
- Renderizados globales masivos por poner estado rápido y frecuente (ej. animaciones o texto ingresado) en el Provider global.
- Desincronización: Guardar datos asíncronos en estado de UI (Ej. hacer un global Store para guardar JSONs de base de datos en lugar de usar TanStack Query).
- Fricción de compartición de URLs (Copiar la URL y enviarla a otro usuario, y que se pierdan los filtros aplicados).

## 4. Reglas Inmutables
- **URL First:** Todo estado de filtro, sort, paginación o búsqueda, DEBE ser un parámetro URL (`?search=ETH&page=2`). Nunca un estado local `useState`, para poder copiar y pegar links de estados específicos y retener el History del navegador.
- **Server Data in Server Context:** No copies datos provenientes de la API o la Base de datos en un Store global de Zustand a menos que los vayas a transformar altamente offline. Usa TanStack Query o Next.js RSC payload para eso.
- **Zustand/Jotai > Context:** Para estados globales del cliente altamente interactivos (Preferencias, toggles globales, temas, flags de WebSockets), prefiere Zustand (o Jotai) antes que React Context, por sus optimizaciones de render y selectores moleculares.

## 5. Nivel de Madurez
Senior / Architect - Garantiza modularidad y evita monopolios de estado.
