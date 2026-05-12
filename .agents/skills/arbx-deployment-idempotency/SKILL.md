---
name: arbx-deployment-idempotency
description: Hacer deploys repetibles y seguros en VPS productivo.
---
# arbx-deployment-idempotency

## Purpose
Abastecer un marco procedimental para empujar, construir y poner en vivo las modificaciones del repositorio hacia el entorno remoto (VPS Hetzner) garantizando mínima fricción, reversibilidad y compilación limpia, asegurando persistencia correcta del contexto ambiental.

## When to use
Cada vez que exista un cambio de código previamente auditado que deba propagarse a la nube/VPS para pasar de un estado "shadow local" a un "live operation" seguro y expuesto.

## Inputs needed
- Rama o commit activo validado localmente.
- Identificación plena de los contenedores/microservicios involucrados (`api-server`, `edge-worker`, `frontend`, etc.).

## Files usually touched
- Archivo maestro `.env` y el recetario `docker-compose.yml` (para confirmaciones paramétricas, si fuese necesario).

## Commands
- Pre-Stage local:
  `git commit -am "chore: deploy updates"` -> `git push`
- Actualización Remota VPS:
  `ssh arbx "cd /opt/git/arbitragex-v2 && git pull"`
- Invocación de Build Implacable:
  `ssh arbx "cd /opt/git/arbitragex-v2 && docker compose build --no-cache --env-file .env <service>"`
- Despliegue en Background:
  `ssh arbx "cd /opt/git/arbitragex-v2 && docker compose up -d <service>"`

## Safety rules
- Nunca abusar de los labels directos genéricos como `:latest` en la inicialización sin presionar `--no-cache`, previendo falsas actualizaciones del Docker daemon.
- La inserción del parámetro `--env-file .env` es un mandamiento ineludible, indispensable para el embebido correcto de directrices estáticas de Next.js.
- Comprobar logs y `Up` tras levantar. Contemplar retroceder (checkout a SHA anterior y reconstruir) si el nuevo despliegue genera crashes cíclicos inmediatos.

## Verification steps
1. Pull resuelto con exactitud sobre la ruta.
2. Build sin cache reflejando descarga e hidratación de paquetes de Pnpm/Cargo vigentes.
3. Up inyectando el contenedor regenerado.
4. Consulta rápida de `docker ps` visualizando el tag `Running` superior a 15 segundos sostenidos.

## Failure modes
- Efectuar `docker compose up -d` de manera perezosa sin pasar la bandera `--build`, perpetuando fallas al levantar una imagen residual.
- Olvidar la propagación de entorno (`--env-file`) resultando en apps intentando consultar a `localhost:8080` ciegamente en lugar del dominio enmascarado en Cloudflare.

## Golden output
Compilación integral limpia y un comando contenedor que transiciona al estado `Running` asimilando el contexto real, propiciando un downtime transicional menor a escasos segundos.

## Anti-patterns
- Intervenir artesanalmente el contenedor (Ej. copiando ficheros con SCP para eludir procesos largos de build), perdiendo la trazabilidad inmutable de Docker.
- Acarrear o reiniciar componentes stateful delicados (Postgres/Redis) para "acompañar un pequeño deploy", aniquilando valiosas caches del mercado sin razón.

## Example prompt
"Ejecuta arbx-deployment-idempotency para desplegar las correcciones visuales de React en arbx forzando la directiva --no-cache y reabriendo el proxy."
