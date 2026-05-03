# Validación y Auditoría

## 1. Criterios de Validación
- Lanzar un Next.js App en modo `next start` (Producción).
- Deshabilitar el caché del navegador (`Disable Cache`).
- Abrir consola. Un render limpio NO DEBE emitir advertencias de hidratación (`Warning: Text content did not match. Server: "A" Client: "B"` ni los minificados #418, #423, #425).

## 2. Cómo Auditar en ARBITRAGEX
- Inspeccionar todos los archivos con `.tsx` en la ruta `/frontend/app`.
- Buscar regex: `useState.*Date\.now\(\)`, `useState.*Math\.random\(\)`. Si se encuentran fuera de contextos controlados, marcar como falla.
- Buscar llamadas a `.toLocaleTimeString()` o `.toLocaleString()` en el JSX sin que dependan de una variable de estado que confirme el montaje del cliente.

## 3. Señales de Fallo en Producción
- El usuario reporta ver un layout que "parpadea" de un estado a otro en la carga inicial (Flash of Unstyled Content / Flash of Server Content).
- La consola del navegador arroja errores que terminan provocando que la aplicación caiga en un Client-Side rendering completo (pérdida de beneficios SSR).
