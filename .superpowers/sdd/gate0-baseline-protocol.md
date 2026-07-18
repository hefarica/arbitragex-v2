# Gate 0 — Baseline Fijado: Protocolo de Captura y Etiquetado

> **Ámbito:** planificación/scaffolding únicamente. Este documento NO ejecuta cambios en el VPS. Cada comando está marcado como *a ejecutar por el operador* o envuelto en modo `--dry-run`.
>
> **Doctrina aplicada:** `git-url-e2e-auditor-scaffold` (§32), `arbx-skills` (paper/shadow, no-hardcode, verify-against-reality), R7 trazabilidad E2E, R8 fail-honest.

## 1. Objetivo

Crear un punto de referencia inmutable (baseline) del despliegue en VPS que permita, en cualquier momento futuro, determinar si el sistema ha derivado, qué cambios se introdujeron y cómo revertir exactamente a este estado.

## 2. Entradas necesarias

| Variable | Origen | Uso |
|----------|--------|-----|
| `VPS_HOST` | `~/.ssh/config` alias `arbx` | Conexión SSH |
| `DEPLOY_PATH` | `/opt/arbitragex-v2` | Ruta del checkout en VPS |
| `COMPOSE_FILE` | `docker/compose.prod.yml` | Compose real en uso |
| `GPG_KEY_ID` | Clave del operador (`git config --global user.signingkey`) | Firma del tag |
| `GITHUB_REMOTE` | `github` (canonical `hefarica/arbitragex-v2`) | Para validar qué commits faltan |

## 3. Definición de "verdadero commit desplegado"

No basta con `git log -1`. El baseline debe capturar:

1. **Commit HEAD** del checkout (`git rev-parse HEAD`).
2. **Branch activa** (`git branch --show-current`).
3. **Estado del working tree** (`git status --porcelain=v1 -uall`).
4. **Untracked files** (`git ls-files --others --exclude-standard`).
5. **Diff contra el remoto canónico** (`git log --oneline HEAD..github/main` y `github/main..HEAD`).
6. **Container image digests** reales ejecutándose (`docker inspect --format='{{.RepoDigests}}'`).
7. **Compose file version** en uso (`sha256sum` del archivo efectivo).
8. **Nivel de migración PostgreSQL** (`SELECT MAX(version) FROM schema_migrations`).
9. **Estado Redis** (`XLEN arbx:opps:detected`, `INFO persistence`, `LASTSAVE`).
10. **Volúmenes activos**, **networks** y **env key hashes** (sin valores secretos).

## 4. Protocolo de captura

### 4.1 Preparación (operador)

```bash
# Asegurar que el alias SSH funciona y el path existe
ssh arbx "cd /opt/arbitragex-v2 && pwd && git remote -v"

# Verificar clave GPG configurada para firma de tags
gpg --list-secret-keys --keyid-format=long
```

### 4.2 Ejecución del script de captura

```bash
# Modo DRY-RUN: solo imprime lo que haría
ssh arbx "bash /opt/arbitragex-v2/scripts/vps/capture-baseline.sh --dry-run"

# Modo real: genera el baseline y crea el tag firmado (operador confirma previamente)
ssh arbx "bash /opt/arbitragex-v2/scripts/vps/capture-baseline.sh --tag baseline/2026-07-18-gate0"
```

### 4.3 Salidas del script

| Artefacto | Ruta en VPS | Descripción |
|-----------|-------------|-------------|
| Manifest JSON | `/opt/arbitragex-v2/.baseline/BASELINE-<ISO8601>.json` | Commit, digests, migración, Redis, compose hash, volúmenes, networks, env key hash |
| Estado git detallado | `/opt/arbitragex-v2/.baseline/BASELINE-<ISO8601>.git.txt` | Status completo, diff con remoto |
| Lista de untracked | `/opt/arbitragex-v2/.baseline/BASELINE-<ISO8601>.untracked.txt` | Archivos no rastreados con checksum |
| Tag firmado | `baseline/YYYY-MM-DD-gate0` en repo local | Anotación con resumen y hash del manifest |

### 4.4 Publicación del tag (operador)

```bash
# Empujar SOLO el tag al remoto canónico github (nunca a origin)
ssh arbx "cd /opt/arbitragex-v2 && git push github baseline/2026-07-18-gate0"
```

## 5. Verificación de rollback

Dado un baseline tag, verificar que `docker compose down && docker compose up` con los digests anclados reproduce el estado exacto.

### 5.1 Protocolo

1. En VPS, exportar el manifest del tag:
   ```bash
   ssh arbx "cat /opt/arbitragex-v2/.baseline/BASELINE-<TS>.json"
   ```
2. Validar que los image digests del manifest siguen disponibles localmente:
   ```bash
   ssh arbx "docker inspect <digest> >/dev/null && echo OK || echo MISSING"
   ```
3. Verificar disponibilidad local de las imágenes del baseline (modo dry-run):
   ```bash
   ssh arbx "bash /opt/arbitragex-v2/scripts/vps/capture-baseline.sh --verify-rollback baseline/2026-07-18-gate0 --dry-run"
   ```
4. Si el operador aprueba, ejecutar rollback controlado:
   ```bash
   ssh arbx "cd /opt/arbitragex-v2 && git checkout <baseline-commit> && docker compose --env-file .env -f docker/compose.prod.yml down && docker compose --env-file .env -f docker/compose.prod.yml up -d"
   ```
   > Nota: el script no genera automáticamente un compose override con digests anclados; verifica disponibilidad de imágenes y entrega evidencia. El operator debe ejecutar el `down/up` manualmente usando el commit y compose file del baseline.

### 5.2 Criterio de éxito del rollback

- `git rev-parse HEAD` coincide con `manifest.git.commit`.
- `docker ps --format '{{.Image}}'` muestra únicamente imágenes cuyo digest está en `manifest.docker.images[].digest`.
- `SELECT MAX(version) FROM schema_migrations` coincide con `manifest.database.schema_migration_max` (o es mayor si hubo migraciones reversibles).
- `XLEN arbx:opps:detected` coincide dentro de una tolerancia documentada (Redis es volátil; se anota `observation` si difiere).
- Los healthchecks de todos los servicios hot-path pasan (`scripts/vps/verify-deploy.sh --strict`).

## 6. Triage de untracked files

**NUNCA eliminar archivos sin aprobación explícita del operador.**

### 6.1 Clasificación

| Categoría | Acción | Ejemplos esperados |
|-----------|--------|--------------------|
| Datos operativos | Añadir a `.gitignore` y documentar en baseline | logs/, .audit/, .baseline/ (esta carpeta) |
| Cartridge runtime | Decisión del operador: commit o ignorar | `scripts/cartridge-deployment/` desplegados por otro pipeline |
| Secretos/Env reales | IGNORAR, rotar si aparecen | `.env` real, `killswitch.json` con claves |
| Cache/build | Ignorar y, si es necesario, limpiar | `frontend/.next/`, `backend/target/` |
| Desconocido | Documentar como `untracked_unknown` en baseline | Cualquier archivo no clasificable |

### 6.2 Protocolo de decisión

1. Generar lista ordenada con tamaño y checksum.
2. Para cada archivo, clasificar según la tabla anterior.
3. Si un archivo contiene un patrón de secreto (regex de gitleaks), marcar como `SECRET_POSSIBLE` y alertar al operador para rotación.
4. Si el operador aprueba, aplicar cambios en `.gitignore` o commit.
5. Re-ejecutar `capture-baseline.sh` después de cualquier modificación para obtener un baseline limpio.

## 7. Formato del manifest JSON (ejemplo)

```json
{
  "schema_version": "gate0-v1",
  "captured_at": "2026-07-18T12:00:00Z",
  "hostname": "arbx-vps",
  "git": {
    "commit": "661724a...",
    "branch": "main",
    "canonical_remote": "github",
    "behind_canonical": 21,
    "ahead_canonical": 0,
    "working_tree_clean": false,
    "porcelain": "M .superpowers/sdd/progress.md\n?? tmp-vps-wip/..."
  },
  "docker": {
    "compose_file": "docker/compose.prod.yml",
    "compose_sha256": "abc123...",
    "images": [
      {
        "service": "api-server",
        "image": "arbitragex-v2/api-server:main",
        "digest": "sha256:def456...",
        "container": "arbitragex-v2-api-server-1"
      }
    ],
    "volumes": ["arbitragex-v2_postgres_data", ...],
    "networks": ["arbitragex-v2_arbx-net"]
  },
  "environment": {
    "env_keys_hash": "abc123..."
  },
  "database": {
    "schema_migration_max": "098",
    "migration_files_count": 84
  },
  "redis": {
    "xlen_arbx_opps_detected": 1523,
    "lastsave": 1721299200,
    "persistence": "rdb_enabled"
  },
  "untracked": {
    "count": 7,
    "files": [
      {"path": "tmp-vps-wip/...", "size": 4096, "sha256": "...", "category": "unknown"}
    ]
  },
  "observations": [
    {"category": "git", "severity": "warn", "detail": "21 commits behind github/main"}
  ]
}
```

## 8. Riesgos residuales

| Riesgo | Mitigación |
|--------|------------|
| Image digest no disponible tras `docker system prune` | Guardar manifest + considerar `docker save` para imágenes críticas; imágenes local-only se marcan `local_only` |
| Redis es volátil; baseline de streams puede divergir rápidamente | Capturar `XLEN` como observación, no invariante absoluto |
| Untracked files contienen secretos | Escaneo con gitleaks; rotar si se detecta |
| Operador ejecuta script con `--tag` real sin revisar dry-run | El script exige confirmación interactiva a menos que `--yes` |
| Tag se empuja a `origin` en lugar de `github` | Script usa `git push github <tag>` explícito; documentado en protocolo |

## 9. Dependencias con siguientes Gates

| Gate | Qué necesita de Gate 0 |
|------|------------------------|
| Gate 1 — Upgrade Path | Commit exacto del baseline, digests de imágenes actuales, nivel de migración previo |
| Gate 2 — Rollback Harness | Tag firmado, manifest JSON, compose hash, protocolo de verificación de rollback |

## 10. Comandos de validación post-captura

```bash
# Verificar que el tag existe y está firmado
ssh arbx "cd /opt/arbitragex-v2 && git tag -v baseline/2026-07-18-gate0"

# Verificar que el manifest es parseable
ssh arbx "python3 -m json.tool /opt/arbitragex-v2/.baseline/BASELINE-*.json >/dev/null && echo JSON_OK"

# Comparar contra estado actual
ssh arbx "bash /opt/arbitragex-v2/scripts/vps/capture-baseline.sh --compare baseline/2026-07-18-gate0"
```
