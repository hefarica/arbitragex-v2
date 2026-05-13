---
name: arbx-vps-verification-runbook
description: Verificar deploy real en VPS de forma segura.
---
# arbx-vps-verification-runbook

## Purpose
Proporcionar una lista sistemática e infalible de verificaciones posteriores al despliegue para garantizar conectividad, exposición controlada y estabilidad directamente en el entorno de producción (VPS `arbx`).

## When to use
Inmediatamente después de ejecutar un comando de despliegue (`docker compose up`) y antes de dictaminar cualquier refactorización o corrección como completada y exitosa.

## Inputs needed
- Conocimiento de los puertos TCP/UDP esperados en Compose (e.g., 8080 API, 3002 Selector).
- Dominios de exposición públicos en Cloudflare (e.g., `https://edge-arbx.ape-tv.net`).

## Files usually touched
- Actúa principalmente sobre línea de comandos. No modifica ficheros del repositorio.

## Commands
- Estado general: `ssh arbx "docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'"`
- Telemetría de logs: `ssh arbx "docker logs arbitragex-v2-api-server-1 --since 10m"`
- Curl Interno (bypass Firewall): `ssh arbx "curl -s http://localhost:8080/api/v1/strategies/runtime-status | jq"`
- Curl Público (Edge): `curl -s "https://edge-arbx.ape-tv.net/api/strategies/runtime-status" | jq`

## Safety rules
- Nunca utilizar ni sugerir el uso de vinculación pública 0.0.0.0 a nivel de Docker (`ports: ["8080:8080"]`) fuera del scope del túnel seguro de Cloudflare, arriesgando exposición del VPS.
- No utilizar purgas generalizadas `flushall` para corroborar actualizaciones.

## Verification steps
1. Revisa que el status en Docker sea `Up x minutes`, rechazando reinicios constantes (`Restarting x`).
2. Verifica ausencia de trazas tipo Pánico / Unhandled Promise Rejection en logs del servicio en cuestión.
3. Evalúa la respuesta en texto plano vía localhost del VPS (asegurando validez de Backend directo).
4. Evalúa la misma respuesta desde el Endpoint público exterior en Edge.

## Failure modes
- Validar el log local o la compilación exitosa y desentenderse de que el proxy de Cloudflare falla devolviendo un 502 Bad Gateway no monitoreado.

## Golden output
Comandos regresando estado del contenedor sano (Up 2 minutes) y curl desde red externa devolviendo un JSON HTTP 200 perfecto, idéntico al emitido vía localhost.

## Anti-patterns
- Decir "La compilación local fue exitosa, doy la tarea por terminada" sin efectuar la auditoría visual y funcional sobre el VPS.
- Abrir un puerto del firewall de servidor (ej. UFW o Hetzner) para poder hacer un curl de "verificación rápida" con una IP.

## Example prompt
"Ejecuta el protocolo arbx-vps-verification-runbook en terminal para comprobar que la API y el Edge Proxy estén devolviendo el runtime-status luego de actualizar."
