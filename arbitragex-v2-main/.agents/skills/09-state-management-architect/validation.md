# Validación y Auditoría

## 1. Criterios de Validación
- Filtrar la tabla del dashboard principal, ordenar las columnas por ROI y actualizar la página con `F5`. Los filtros y el ordenamiento deben preservarse (URL State o LocalStorage persist).
- Observar el React Profiler cuando se hace click en el botón "Toggle Theme" o "Toggle Compact View". Únicamente deben repintarse los componentes suscritos a esos modificadores estéticos, no la data entera.

## 2. Cómo Auditar
- Identificar en `/frontend` instancias de `createContext`. Evaluar si es justificable. Recomendar cambiar a parámetros de URL o Zustand atomizado si se descubre alto tráfico de re-renders.
