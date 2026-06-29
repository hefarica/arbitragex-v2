---
name: arbx-vps-production-gate
description: Barrera de protección para evitar despliegues accidentales, reinicios y modificaciones inseguras en el VPS productivo.
---

# VPS PRODUCTION GATE

## Purpose
Establecer un flujo estricto e inquebrantable de autorización y validación previa antes de interactuar con el entorno de producción (VPS).

## When to use
Antes de ejecutar CUALQUIER comando que modifique el estado de la máquina VPS (`<VPS_IP>`).

## Comandos Permitidos SIN Aprobación Previa (Solo Lectura)
- `git status`, `git log`, `git diff`
- `curl` a endpoints internos/públicos para diagnóstico
- `docker ps --format ...`
- `docker logs <contenedor> --tail N`
- Consultas read-only a PostgreSQL/Redis.

## Comandos Prohibidos SIN Aprobación Previa
- `ssh arbx` para ejecutar comandos destructivos o mutativos.
- `docker compose build`, `up -d`, `restart`, `down`
- Editar o imprimir los secretos del `.env` (`cat .env` está prohibido)
- `git pull` en la máquina VPS.
- Hacer deploy "en background".
- Decir "listo" sin evidencia en el reporte.

## Flujo de Despliegue Idempotente (Una vez Aprobado)
1. `ssh arbx 'cd /opt/arbitragex-v2 && git pull'`
2. `ssh arbx 'cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.prod.yml build --no-cache <service>'`
3. `ssh arbx 'cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.prod.yml up -d <service>'`
4. Verificar mediante logs y curl que el servicio subió adecuadamente y no presenta errores.

## Verification steps
1. Existe un diff validado y aprobado.
2. Existe compilación (build) local exitosa.
3. Se generó un commit claro y se subió (`git push`).
4. Existe autorización explícita del usuario para desplegar al VPS.
