# VPS Automated Deployment Protocol

## Propósito
Estandarizar el acceso remoto y el control de despliegue sobre el VPS productivo de ArbitrageX (`<VPS_IP>`). Esta skill sirve como la puerta de enlace segura y automatizada para empujar cambios del motor (ej. `searcher-rs`) a producción.

## Conocimiento Esencial (Memoria Inyectada)
- El entorno remoto cuenta con un alias SSH configurado localmente llamado `arbx`.
- El uso de este alias puentea la restricción de contraseñas ya que el identity key correcto está referenciado en el SSH Agent de Windows.
- El proyecto en el servidor remoto reside en `/opt/arbitragex-v2`.

## Comando Principal de Despliegue
Para aplicar cambios luego de un `git push`, se debe lanzar el proceso en cadena usando conectores lógicos de bash (`&&`) sobre el túnel SSH:

```bash
ssh arbx "cd /opt/arbitragex-v2 && git pull origin main && docker compose -f docker/compose.dev.yml build <service_name> && docker compose -f docker/compose.dev.yml up -d <service_name>"
```

Sustituir `<service_name>` por el microservicio correspondiente (ej. `searcher-rs`, `frontend`, `api-server`).

## Criterios de Producción
- Nunca compilar fuera del Docker en el VPS (para evitar contaminación de librerías en el SO host).
- Toda compilación de Rust se realiza dentro del entorno aislado del container en modo `release`.
