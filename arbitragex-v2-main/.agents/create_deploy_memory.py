import os
import json
from datetime import datetime

# 1. CREATE KNOWLEDGE ITEM (Persistent Memory)
ki_dir = r"C:\Users\HFRC\.gemini\antigravity\knowledge\arbitragex_vps_deployment"
ki_artifacts_dir = os.path.join(ki_dir, "artifacts")
os.makedirs(ki_artifacts_dir, exist_ok=True)

metadata = {
    "title": "ArbitrageX VPS Deployment Protocol",
    "summary": "Protocolo de despliegue en producción sobre el VPS 195.201.235.70 usando el alias SSH 'arbx'. Incluye comandos exactos para construir y reiniciar contenedores con Docker Compose, garantizando el flujo de trabajo CI/CD manual en la IP productiva.",
    "created_at": datetime.now().isoformat(),
    "last_accessed": datetime.now().isoformat(),
    "references": ["Conversation ID: 244c566e-eec7-46fc-bc7f-f02aa3d61d9e"]
}

with open(os.path.join(ki_dir, "metadata.json"), "w", encoding="utf-8") as f:
    json.dump(metadata, f, indent=2)

ki_artifact_content = """# ArbitrageX VPS Deployment Protocol

## Contexto Operativo
- **IP del Servidor**: `195.201.235.70`
- **Ruta del Repositorio en VPS**: `/opt/arbitragex-v2`
- **Alias SSH**: `arbx` (Configurado en `~/.ssh/config` del host de desarrollo, usa clave RSA/Ed25519 para acceso sin password).
- **Mecanismo**: Docker Compose (`docker/compose.dev.yml`)

## Flujo de Despliegue Estándar

El ciclo para subir código modificado en Rust, Next.js u otro componente a producción es:

1. **Guardar y Subir al Repositorio Remoto (GitHub)**
```bash
git add .
git commit -m "feat: [descripción]"
git push origin main
```

2. **Orquestar el Pull y Build en el VPS mediante SSH**
```bash
ssh arbx "cd /opt/arbitragex-v2 && git pull origin main && docker compose -f docker/compose.dev.yml build searcher-rs && docker compose -f docker/compose.dev.yml up -d searcher-rs"
```

## Reglas Inmutables (Doctrinas)
- **Zero Mocks Doctrine (`rule_00_zero_mocks_doctrine`)**: No enviar simulacros al VPS.
- **Docker Build Strictness (`rule_03_nextjs_docker_build_strictness`)**: Todo cambio estructural debe compilarse de nuevo (`build`), no basta con reiniciar el contenedor si se altera código binario (Rust) o estático inyectado (Next.js).
"""
with open(os.path.join(ki_artifacts_dir, "vps_deployment.md"), "w", encoding="utf-8") as f:
    f.write(ki_artifact_content)


# 2. CREATE SKILL (ArbitrageX v2 Toolkit)
skill_dir = r"C:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\.agents\skills\vps-automated-deployment-protocol"
os.makedirs(skill_dir, exist_ok=True)

manifest = {
    "skill_id": "vps-automated-deployment-protocol",
    "name": "VPS Automated Deployment Protocol",
    "version": "1.0.0",
    "domain": "DevOps | CI/CD | Infraestructura",
    "level": "Production",
    "created_for_project": "arbitragex_v2_productivo_full",
    "output_path": skill_dir,
    "implementation_languages": ["Bash", "Docker"],
    "requires_real_data": False,
    "hardcode_allowed": True,
    "mocks_allowed": False
}

with open(os.path.join(skill_dir, "manifest.json"), "w", encoding="utf-8") as f:
    json.dump(manifest, f, indent=2)

skill_content = """# VPS Automated Deployment Protocol

## Propósito
Estandarizar el acceso remoto y el control de despliegue sobre el VPS productivo de ArbitrageX (`195.201.235.70`). Esta skill sirve como la puerta de enlace segura y automatizada para empujar cambios del motor (ej. `searcher-rs`) a producción.

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
"""
with open(os.path.join(skill_dir, "SKILL.md"), "w", encoding="utf-8") as f:
    f.write(skill_content)

print("Knowledge Item and Skill created successfully.")
