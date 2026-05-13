# Validación y Auditoría

## 1. Criterios de Validación
- Ejecutar `docker build`. El tamaño final de la imagen Runner (Standalone) no debería exceder los 250MB - 350MB. (A diferencia de los 1.5GB de una imagen base completa).
- Buscar un componente cargado en el navegador del cliente, inspeccionar en DevTools Sources y verificar que las variables estáticas están "quemadas" (Hardcoded by Webpack) con la IP/Dominio de producción y no de desarrollo.

## 2. Cómo Auditar
- Inspeccionar `docker-compose.yml`. Todo `NEXT_PUBLIC_` en un servicio de frontend obligatoriamente debe estar dentro de la cláusula `build: args:`.
