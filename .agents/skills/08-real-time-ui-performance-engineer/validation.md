# Validación y Auditoría

## 1. Criterios de Validación
- Habilitar la opción "Highlight updates when components render" en las herramientas React DevTools Profiler.
- Observar el WebSocket funcionando en vivo. Solo deben destellar de color verde las filas específicas que reciben actualización, NO el contenedor entero ni los subcomponentes adyacentes.

## 2. Cómo Auditar
- Inyectar 10,000 eventos sintéticos por WebSocket en 2 segundos.
- La memoria JS no debe crecer más de 50MB ni arrojar picos insalvables de Main Thread Blocking largos en el Profiler del navegador.
