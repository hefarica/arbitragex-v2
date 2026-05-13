# Validación y Auditoría

## 1. Criterios de Validación
- Deshabilitar red en DevTools. La UI debe mantener la última caché. Rehabilitar la red, el socket debe reconectar, y NO deben aparecer objetos duplicados con la misma Key/ID en la vista de la tabla.
- Iniciar la aplicación en un puerto limpio; de inmediato se deben visualizar registros históricos (Snapshot de Redis). 

## 2. Cómo Auditar
- Buscar en `/frontend` las palabras clave `.on(` o `socket.addEventListener(`. 
- Verificar dentro del callback si la actualización de estado incluye una validación explícita (ej. `.some` o `.findIndex`).
