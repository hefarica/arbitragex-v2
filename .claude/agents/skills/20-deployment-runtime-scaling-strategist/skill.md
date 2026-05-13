# Skill 20: Deployment, Runtime & Scaling Strategist

## 1. Propósito
Empaquetar, construir y desplegar la aplicación React/Next.js de forma inmutable, escalable y observable. Dominar Docker multi-stage builds específicos para NodeJS, inyección de variables de entorno en Build-time vs Runtime (`NEXT_PUBLIC_`), gestión de procesos con `tini` y monitoreo de memoria base (Standalone output).

## 2. Aplicación directa en ARBITRAGEX
El error que impidió a la aplicación conectarse usando `edge-arbx.ape-tv.net` e insistió en buscar `localhost:8787` ocurrió debido a fallos al entender en qué fase se embeben las variables `NEXT_PUBLIC_`. Esta skill garantiza despliegues atómicos sin fugas, asegurando que la imagen de Docker contenga todo lo precompilado correctamente, y que la aplicación Node.js sobreviva en el VPS (IP `195.201.235.70`).

## 3. Problemas que resuelve
- Build-time vs Runtime Env Variables: Errores persistentes de localhost en producción.
- Contenedores de 2 Gigabytes por arrastrar carpetas enteras de dependencias de desarrollo (`node_modules`).
- Node.js bloqueado o zombie porque el PID 1 no es `init` o `tini` (las señales `SIGTERM` fallan y docker tarda 10 segundos en matar).
- Excesivo consumo de RAM del VPS por correr Next.js en modo no optimizado (`standalone`).

## 4. Reglas Inmutables
- **NEXT_PUBLIC_ en Build Time:** Toda variable con prefijo `NEXT_PUBLIC_` en Next.js App Router ES CONGELADA (quemada, inlined) de manera textual en los archivos JavaScript estáticos durante el proceso de compilación `next build`. **SIEMPRE** deben proporcionarse al Docker Builder a través de `ARG`. Alterarlas en el bloque `environment:` de docker-compose para el contenedor runtime NO alterará el Javascript del cliente si ya está construido.
- Utilizar `output: 'standalone'` en el `next.config.js` para reducir agresivamente la cantidad de dependencias subidas a producción (Solo incluye los rastros de Node vitales).
- El comando CMD en Docker de Next.js debe ejecutarse detrás de `tini` (o similar) para manejo de PIDs y cierre limpio. `ENTRYPOINT ["/usr/bin/tini","--"]`.

## 5. Nivel de Madurez
PhD / DevOps Maestro - Despliegues inmutables listos para la batalla.
