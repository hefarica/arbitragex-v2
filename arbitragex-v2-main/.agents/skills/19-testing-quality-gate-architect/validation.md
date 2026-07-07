# Validación y Auditoría

## 1. Criterios de Validación
- Romper el código agregando un `any` erróneo o eliminando el `<button>` del JSX.
- El servidor de integración continua (CI) debe interrumpir el proceso de dockerizacion o despliegue y mostrar un Warning color rojo.

## 2. Cómo Auditar
- Ejecutar la pirámide completa: `npm run lint`, `npm run typecheck`, `npm run test`, `npm run test:e2e`. Todos deben terminar exitosamente en entornos limpios.
