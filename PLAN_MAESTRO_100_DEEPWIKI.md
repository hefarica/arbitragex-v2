# PLAN MAESTRO — repo-vps-deepwiki al 100%

> **Scan objetivo:** `SCAN-20260716T181807Z` → Próximo scan ≥ 95% (100% ideal)
> **Fecha:** 2026-07-16
> **Estado actual corregido:** ~87% (scanner reportó 64.2% con falsos positivos)
> **Duración estimada total:** 4–6 horas distribuidas
> **Riesgo:** Bajo (solo read-only audit; fixes en scanner local, no en producción)

---

## RESUMEN EJECUTIVO

El scanner `repo_vps_deepwiki.py` v1.0.0 contiene **4 bugs de lógica** que deflacionaron artificialmente el score del scan anterior de ~87% a 64.2%. Adicionalmente, el VPS tiene **1 servicio degradado** (`thanos-store`) y un **deploy gap de ~21 commits** que impiden el 100%.

Este plan divide el trabajo en **5 fases** con verificación incremental. Cada fase es independiente salvo la secuencia de deploy.

---

## TABLA RESUMEN DE FASES

| Fase | Objetivo | Tiempo | Riesgo | Rollback |
|------|----------|--------|--------|----------|
| 1 | Corregir 4 bugs del scanner | 2h | Bajo | Git checkout del .py original |
| 2 | Reparar `thanos-store` en VPS | 30min | Medio | `docker compose restart` previo |
| 3 | Sincronizar deploy (299b8ac → aa28fee) | 1h | Medio | VPS git stash + checkout 299b8ac |
| 4 | Pre-scan validation | 30min | Nulo | N/A |
| 5 | Ejecutar scan y verificar 100% | 30min | Nulo | Re-ejecutar desde Fase 4 |

---

## FASE 1: CORRECCIÓN DE BUGS DEL SCANNER (repo_vps_deepwiki.py)

### Bug 1 — Falsos MISSING en contenedores con Dockerfiles existentes

**Síntoma:** 9 servicios marcados `MISSING` a pesar de que sus Dockerfiles existen en el repo.

**Causa raíz (líneas 876–878 del scanner):**
```python
dockerfile = str(service.get("dockerfile", ""))
dockerfile_exists = True if not dockerfile else dockerfile in files
```

El parser de Compose extrae `build.dockerfile` del YAML pero **no resuelve el path relativo al contexto de build**. Ejemplo: en `compose.prod.yml`:
```yaml
searcher-rs:
  build:
    context: ../backend
    dockerfile: searcher-rs/Dockerfile
```

El scanner almacena `dockerfile: "searcher-rs/Dockerfile"` en el contrato, pero el inventario del repo tiene `"backend/searcher-rs/Dockerfile"`. La comparación `dockerfile in files` falla porque los paths no coinciden.

**Fix:**
```python
# En la función que parsea compose (descubre el contract),
# cuando se extrae build.dockerfile, prefijar con el directorio
# relativo del contexto de build al repo root.

build_context = service_build.get("context", ".")
dockerfile_rel = service_build.get("dockerfile", "Dockerfile")
# Resolver path absoluto desde el compose file, luego relativo al repo
compose_dir = Path(compose_path).parent
dockerfile_abs = (compose_dir / build_context / dockerfile_rel).resolve()
dockerfile_repo_rel = dockerfile_abs.relative_to(repo).as_posix()
# Almacenar dockerfile_repo_rel en el contrato
```

**Verificación:**
```bash
python3 repo_vps_deepwiki.py doctor
python3 repo_vps_deepwiki.py scan --repo "c:/Users/HFRC/Desktop/arbitragex-v2-main (17)" \
  --output-root "./test-scan-bug1"
# Verificar que NINGÚN servicio del contrato tenga dockerfile=False
```

---

### Bug 2 — `remote_hash=UNKNOWN` para TODOS los archivos críticos

**Síntoma:** 400+ archivos con `remote_hash=UNKNOWN`, forzando `DRIFT` en todos.

**Causa raíz (líneas 714–718 del scanner):**
```python
quoted = " ".join(shell_quote(path) for path in safe_paths)
hash_cmd = (
    f"cd {shell_quote(repo_path)} 2>/dev/null && "
    f"for p in {quoted}; do if [ -f \"$p\" ]; then sha256sum -- \"$p\"; "
    "else printf 'MISSING  %s\\n' \"$p\"; fi; done"
)
```

El error del scan: `bash: -c: line 1: unexpected EOF while looking for matching '''`

`shell_quote()` usa `shlex.quote()` que envuelve en comillas simples. Cuando un path contiene caracteres especiales o espacios, la concatenación en el `for p in {quoted}` genera un string bash malformado. Específicamente, el cierre de comillas simples dentro del heredoc de comando SSH colisiona con las comillas del `for` loop.

**Fix — Reescribir `hash_cmd` para usar un array bash:**
```python
paths_quoted = " ".join(shell_quote(path) for path in safe_paths)
hash_cmd = (
    f"cd {shell_quote(repo_path)} 2>/dev/null && "
    f"paths=({paths_quoted}); "
    "for p in \"${paths[@]}\"; do "
    "if [ -f \"$p\" ]; then sha256sum -- \"$p\"; "
    "else printf 'MISSING  %s\\n' \"$p\"; fi; done"
)
```

Alternativa más segura: escribir un script temporal en el VPS:
```python
script_lines = [
    "#!/bin/bash",
    f"cd {shell_quote(repo_path)} 2>/dev/null || exit 1",
]
for path in safe_paths:
    script_lines.append(
        f'if [ -f {shell_quote(path)} ]; then sha256sum -- {shell_quote(path)}; '
        f'else printf "MISSING  %s\\n" {shell_quote(path)}; fi'
    )
# Escribir script a /tmp via SSH, ejecutar, eliminar
```

**Verificación:**
```bash
# Ejecutar scan y verificar que remote_hash != UNKNOWN para archivos existentes
# en el VPS
grep "remote_hash=UNKNOWN" 02_REPO_VPS_PARITY.md | wc -l
# Debe ser 0 (o muy bajo para archivos realmente ausentes)
```

---

### Bug 3 — VPS SHA vacío / `branch="DIRTY\t0"`

**Síntoma:** `vps_sha=""`, `branch="DIRTY\t0"`, score de parity = UNKNOWN.

**Causa raíz (líneas 610–624 del scanner):**
```python
meta_cmd = (
    "set +e; "
    "if [ -r /etc/os-release ]; then . /etc/os-release; "
    "printf 'OS\t%s %s\n' \"${NAME:-Linux}\" \"${VERSION_ID:-unknown}\"; fi; "
    "printf 'UNAME\t'; uname -srmo; "
    ...
    f"cd {shell_quote(repo_path)} 2>/dev/null && "
    "printf 'SHA\t'; git rev-parse HEAD 2>/dev/null; "
    "printf 'BRANCH\t'; git branch --show-current 2>/dev/null; "
    "printf 'DIRTY\t'; git status --porcelain=v1 2>/dev/null | wc -l; "
    ...
)
```

Problemas:
1. Si `cd` falla (repo_path no existe o no es git repo), los comandos `git` siguen ejecutándose desde `$HOME`, donde no hay repo git → SHA vacío.
2. `git branch --show-current` puede retornar vacío si HEAD está detached → línea `BRANCH	` sin valor.
3. El parser de líneas (líneas 626–651) asume que cada `printf` produce exactamente una línea con un `\t`. Si algún comando no imprime nada o imprime múltiples líneas, el parsing se desalinea.

**Fix:**
```python
meta_cmd = (
    "set +e; set -u; "
    "REPO_PATH=" + shell_quote(repo_path) + "; "
    "if [ -d \"$REPO_PATH/.git\" ]; then "
    "  cd \"$REPO_PATH\" || exit 1; "
    "  printf 'SHA\t%s\n' \"$(git rev-parse HEAD 2>/dev/null)\"; "
    "  printf 'BRANCH\t%s\n' \"$(git branch --show-current 2>/dev/null || echo detached)\"; "
    "  printf 'DIRTY\t%s\n' \"$(git status --porcelain=v1 2>/dev/null | wc -l)\"; "
    "else "
    "  printf 'SHA\t\n'; printf 'BRANCH\t\n'; printf 'DIRTY\t\n'; "
    "fi; "
    "if [ -r /etc/os-release ]; then ... fi; "
    # ... resto de comandos independientes del repo
)
```

Además, añadir validación post-parse:
```python
if result["sha"] == "" and result.get("configured"):
    rec.log("VPS", "warning", "git-sha-empty",
            f"VPS repo at {repo_path} has no git SHA", note="Check if path is correct or repo is initialized")
```

**Verificación:**
```bash
ssh root@195.201.235.70 "cd /opt/arbitragex-v2 && git rev-parse HEAD"
# Debe retornar: 299b8ac2b784da32c06c978795064fe5943cc874
```

---

### Bug 4 — Worktrees inflan el inventario a 46,144 archivos

**Síntoma:** El inventario incluye archivos duplicados bajo `.claude/worktrees/agent-*/` y `.claude/worktrees/agent-*/arbitragex-v2-main/`.

**Causa raíz (líneas 69–72 del scanner):**
```python
IGNORE_DIRS = {
    ".git", "node_modules", "target", ".next", "dist", "build", ".venv",
    "venv", "coverage", ".cache", "__pycache__", ".idea", ".vscode",
}
```

`.claude/worktrees/` no está en la lista de exclusión. Cada worktree de agente es un clon completo del repo (o casi), duplicando miles de archivos.

**Fix:**
```python
IGNORE_DIRS = {
    ".git", "node_modules", "target", ".next", "dist", "build", ".venv",
    "venv", "coverage", ".cache", "__pycache__", ".idea", ".vscode",
    ".claude",  # ← AÑADIR: excluye worktrees, settings, agents, skills temporales
}
```

Consideración: `.claude/` puede contener `CLAUDE.md` que ES parte del contrato canónico. Si se excluye completamente, perderíamos archivos legítimos.

**Fix refinado — solo excluir worktrees:**
```python
IGNORE_DIRS = {
    ".git", "node_modules", "target", ".next", "dist", "build", ".venv",
    "venv", "coverage", "__pycache__", ".idea", ".vscode", ".cache",
}
# En inventory_repo, filtrar paths que contengan .claude/worktrees/
for root, subdirs, names in os.walk(repo):
    subdirs[:] = [d for d in subdirs if d not in IGNORE_DIRS]
    root_path = Path(root)
    rel_root = root_path.relative_to(repo).as_posix()
    # Excluir paths bajo .claude/worktrees/
    if ".claude/worktrees/" in rel_root:
        subdirs.clear()  # No descender más
        continue
    ...
```

**Verificación:**
```bash
python3 repo_vps_deepwiki.py scan --repo "..." --output-root "./test-scan-bug4"
# Verificar que repo-inventory.json tenga file_count < 15,000 (vs 46,144 actual)
```

---

## FASE 1 — CHECKLIST DE IMPLEMENTACIÓN

- [ ] **1A.** Hacer backup del scanner original:
  ```bash
  cp repo_vps_deepwiki.py repo_vps_deepwiki.py.v1.0.0.backup
  ```
- [ ] **1B.** Implementar Fix Bug 1 (context path resolution en compose parser)
- [ ] **1C.** Implementar Fix Bug 2 (hash_cmd con array bash o script temporal)
- [ ] **1D.** Implementar Fix Bug 3 (meta_cmd robusto + validación SHA)
- [ ] **1E.** Implementar Fix Bug 4 (excluir `.claude/worktrees/` del inventario)
- [ ] **1F.** Ejecutar `python3 repo_vps_deepwiki.py doctor` — debe pasar
- [ ] **1G.** Ejecutar scan local-only (sin VPS):
  ```bash
  python3 repo_vps_deepwiki.py scan \
    --repo "c:/Users/HFRC/Desktop/arbitragex-v2-main (17)" \
    --output-root "./test-phase1"
  ```
- [ ] **1H.** Verificar: NINGÚN servicio con Dockerfile real tenga `dockerfile=False`
- [ ] **1I.** Verificar: `file_count` < 15,000 (sin worktrees)
- [ ] **1J.** Commit del scanner corregido (separado del repo del proyecto)

---

## FASE 2: REPARACIÓN DE `thanos-store` EN VPS

### Diagnóstico

`thanos-store` está `UNHEALTHY`. Es un componente de la stack de observabilidad (métricas a largo plazo). No afecta el pipeline de procesamiento de oportunidades, pero sí el score del scanner.

**Comandos de diagnóstico:**
```bash
ssh root@195.201.235.70 "docker logs arbitragex-v2-thanos-store-1 --tail 50"
ssh root@195.201.235.70 "docker inspect arbitragex-v2-thanos-store-1 --format='{{.State.Health}}'"
ssh root@195.201.235.70 "docker exec arbitragex-v2-thanos-store-1 wget -qO- http://localhost:10902/-/healthy 2>&1 || echo 'NO_RESPONSE'"
```

### Causas probables y fixes

| Causa probable | Síntoma | Fix |
|---------------|---------|-----|
| MinIO bucket no existe | Logs con `bucket not found` | Crear bucket `thanos` en MinIO |
| Credenciales MinIO incorrectas | `Access Denied` | Verificar `MINIO_ROOT_USER/PASSWORD` en `.env` |
| Store gateway sin bloques | `no blocks found` | Normal si no hay datos históricos; ajustar healthcheck |
| Puerto conflictivo | `bind: address already in use` | Verificar `netstat -tlnp | grep 10902` |
| Imagen incompatible | CrashLoop en startup | Verificar tag de imagen thanos |

### Acción recomendada (read-only audit primero, luego fix)

```bash
# 1. Diagnóstico read-only
ssh root@195.201.235.70 "docker logs arbitragex-v2-thanos-store-1 --tail 100 2>&1 | head -50"

# 2. Si es healthcheck demasiado estricto (sin datos aún):
#    Editar compose para relajar healthcheck o esperar datos
# 3. Si es MinIO bucket faltante:
ssh root@195.201.235.70 "docker exec arbitragex-v2-minio-1 mc alias set local http://localhost:9000 \$MINIO_ROOT_USER \$MINIO_ROOT_PASSWORD && mc mb local/thanos --ignore-existing"

# 4. Restart del servicio (último recurso):
ssh root@195.201.235.70 "cd /opt/arbitragex-v2/docker && docker compose -f compose.prod.yml restart thanos-store"
```

### Rollback
```bash
ssh root@195.201.235.70 "cd /opt/arbitragex-v2/docker && docker compose -f compose.prod.yml down thanos-store && docker compose -f compose.prod.yml up -d thanos-store"
```

---

## FASE 3: SINCRONIZACIÓN DE DEPLOY (299b8ac → aa28fee)

### Análisis del delta

```
299b8ac feat(live-testnet): scaffold real TS route, config overlay, tests honestos
57b1665 fix(semiotic-bridge): resolve Rust compilation errors
20d57ae feat(semiotic-bridge): complete Fact-Forcing Gate
aa28fee feat(live-testnet-v2): SSE endpoint, executor stub, E2E token from env, CI secrets
```

**Cambios introducidos:**
1. **SSE endpoint** — nuevo endpoint Server-Sent Events en api-server o edge
2. **Executor stub** — nuevo módulo de ejecución (relays-client o spine)
3. **E2E token from env** — tests end-to-end ahora leen token de variables de entorno
4. **CI secrets** — nuevos secrets en GitHub Actions (no afecta VPS directamente)
5. **Fact-Forcing Gate** — infraestructura de validación semiótica
6. **Rust compilation fixes** — correcciones en código Rust (backend)

### Secuencia de deploy segura

```bash
# === EN VPS (195.201.235.70) ===
cd /opt/arbitragex-v2

# 1. Verificar estado actual
git log -1 --oneline
# → 299b8ac

# 2. Fetch desde github (origin es el VPS mirror, usar github remote)
git fetch github main

# 3. Ver diff (solo lectura)
git diff 299b8ac..github/main --stat

# 4. Stash de cambios no trackeados (precaución)
git stash push -m "pre-deploy-stash-$(date +%s)"

# 5. Checkout del nuevo SHA
git checkout aa28fee

# 6. Verificar que los compose files no cambiaron de forma incompatible
diff <(git show 299b8ac:docker/compose.prod.yml) docker/compose.prod.yml

# 7. Build sin cache de servicios afectados
#    (identificar cuáles cambiaron sus Dockerfiles o dependencias)
docker compose --env-file .env -f docker/compose.prod.yml build --no-cache thanos-store api-server edge frontend

# 8. Up con recreación de contenedores modificados
docker compose --env-file .env -f docker/compose.prod.yml up -d --remove-orphans

# 9. Verificar health de todos los servicios
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Health}}"

# 10. Verificar logs de servicios críticos
docker logs arbitragex-v2-api-server-1 --tail 20
docker logs arbitragex-v2-edge-1 --tail 20
```

### Validación post-deploy

```bash
# Health check E2E
curl -s http://195.201.235.70:8787/health || echo "EDGE FAIL"
curl -s http://195.201.235.70:8080/health || echo "API FAIL"

# Redis check
docker exec arbitragex-v2-redis-1 redis-cli PING

# Postgres check
docker exec arbitragex-v2-postgres-1 pg_isready -U postgres
```

---

## FASE 4: PRE-SCAN VALIDATION

### Checklist antes de ejecutar el scan final

- [ ] **4A.** Scanner corregido (Fase 1 completa)
- [ ] **4B.** `thanos-store` health = `HEALTHY` o `UP` (Fase 2)
- [ ] **4C.** VPS SHA = `aa28fee` (Fase 3)
- [ ] **4D.** Todos los contenedores en estado `running`:
  ```bash
  ssh root@195.201.235.70 "docker ps --format '{{.Names}}: {{.Status}}' | grep -v 'Up' || echo 'ALL_UP'"
  ```
- [ ] **4E.** No hay contenedores `EXTRA` inesperados (solo `friendly_jones` es conocido — Exited)
- [ ] **4F.** Repositorio local limpio (sin cambios no commiteados que afecten compose):
  ```bash
  git -C "c:/Users/HFRC/Desktop/arbitragex-v2-main (17)" status --short
  # Debe mostrar solo .claude/settings.json y .claude/settings.local.json
  # (o estar limpio)
  ```
- [ ] **4G.** `docker compose config` válido en ambos entornos:
  ```bash
  # Local
  docker compose -f docker/compose.prod.yml config > /dev/null && echo "VALID"
  # VPS
  ssh root@195.201.235.70 "cd /opt/arbitragex-v2/docker && docker compose -f compose.prod.yml config > /dev/null && echo 'VALID'"
  ```
- [ ] **4H.** Espacio en disco VPS > 10% libre:
  ```bash
  ssh root@195.201.235.70 "df -h / | tail -1"
  # Actual: 82% usado = 18% libre → OK pero monitorizar
  ```

---

## FASE 5: SCAN FINAL Y VERIFICACIÓN DEL 100%

### Comando de scan

```bash
python3 "C:/Users/HFRC/.claude/skills/repo-vps-deepwiki/bin/repo_vps_deepwiki.py" scan \
  --repo "c:/Users/HFRC/Desktop/arbitragex-v2-main (17)" \
  --vps-host "195.201.235.70" \
  --vps-user "root" \
  --ssh-key "/c/Users/HFRC/.ssh/arbx_hetzner" \
  --vps-repo-path "/opt/arbitragex-v2" \
  --compose auto \
  --contract auto \
  --output-root "c:/Users/HFRC/Desktop/arbitragex-v2-main (17)/repo-vps-audits"
```

### Criterios de aceptación para 100%

| Criterio | Valor esperado | Tolerancia |
|----------|---------------|------------|
| **Score weighted** | 100% | ≥ 98% |
| `VERIFIED` count | ≥ 25 | Todas las CORE + infra |
| `MISSING` count | 0 | Ningún falso positivo |
| `BROKEN` count | 0 | `thanos-store` reparado |
| `UNKNOWN` count | 0 | VPS SHA capturado |
| `DRIFT` count | ≤ 2 | Solo archivos legítimamente modificados en hot-path |
| `repo_sha == vps_sha` | `aa28fee` | Exacto |
| `file_count` | < 15,000 | Sin worktrees |
| Todos los E2E flows | `VERIFIED` | F1–F4 + deps |

### Verificación post-scan

```bash
# 1. Leer score
jq '.score' repo-vps-audits/SCAN-*/architecture-state.json

# 2. Verificar que no hay MISSING falsos
grep -c "MISSING" repo-vps-audits/SCAN-*/03_DOCKER_RUNTIME.md
# Debe ser 0 (o 1 si hay un servicio realmente no definido en compose)

# 3. Verificar que remote_hash no es UNKNOWN
grep -c "remote_hash=UNKNOWN" repo-vps-audits/SCAN-*/02_REPO_VPS_PARITY.md
# Debe ser 0 para archivos que existen en VPS

# 4. Verificar VPS SHA
grep "vps_sha" repo-vps-audits/SCAN-*/00_SUPREME_EXECUTIVE_REPORT.md
# Debe mostrar aa28fee

# 5. Verificar E2E flows
grep "State:" repo-vps-audits/SCAN-*/04_E2E_INFORMATION_FLOWS.md
# Todos deben ser VERIFIED o DEGRADED (no BROKEN por falsos positivos)
```

---

## MATRIZ DE RIESGO Y MITIGACIÓN

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|-------------|---------|------------|
| Fix del scanner introduce nuevo bug | Media | Medio | Test local-only antes de VPS; backup del .py original |
| Deploy de aa28fee rompe servicio existente | Baja | Alto | Deploy por servicio (no `--no-cache` global); rollback a 299b8ac documentado |
| `thanos-store` no reparable sin datos | Media | Bajo | Es observabilidad, no crítico para pipelines; marcar como known issue |
| SSH al VPS falla durante scan | Baja | Medio | Verificar conectividad antes; timeout generoso (600s) |
| Windows path separators causan issues | Media | Medio | Usar `.as_posix()` consistentemente; probar en Git Bash |

---

## AGENTES OMEGA ASIGNABLES

Según la matriz de orquestación institucional (§16):

| Fase | Builder | Validador |
|------|---------|-----------|
| 1 — Scanner fixes | `cs-validator` (Python) | `math-validator` (lógica de scoring) |
| 2 — thanos-store | `devops-platform` | `security-auditor` |
| 3 — Deploy | `devops-platform` | `cs-validator` |
| 4 — Pre-scan | `cs-validator` | `security-auditor` |
| 5 — Scan + 100% verify | `cs-validator` | `math-validator` |

---

## ANEXO: COMANDOS DE ROLLBACK RÁPIDO

### Rollback scanner
```bash
cd "C:/Users/HFRC/.claude/skills/repo-vps-deepwiki/bin/"
cp repo_vps_deepwiki.py.v1.0.0.backup repo_vps_deepwiki.py
```

### Rollback VPS deploy
```bash
ssh root@195.201.235.70 "cd /opt/arbitragex-v2 && git stash && git checkout 299b8ac && cd docker && docker compose --env-file .env -f compose.prod.yml up -d --build --no-cache"
```

### Rollback thanos-store
```bash
ssh root@195.201.235.70 "cd /opt/arbitragex-v2/docker && docker compose -f compose.prod.yml stop thanos-store && docker compose -f compose.prod.yml rm -f thanos-store && docker compose -f compose.prod.yml up -d thanos-store"
```

---

## GLOSARIO DE ESTADOS DEL SCANNER (para referencia del operador)

| Estado | Significado real | Qué hacer |
|--------|-----------------|-----------|
| `VERIFIED` | Contenedor/file existe, SHA coincide, health OK | Nada |
| `DRIFT` | Existe pero SHA difiere o compose no alinea | Revisar si es deploy pendiente o hotfix intencional |
| `MISSING` | No existe en repo, compose, o runtime | Si es falso positivo → reportar bug de scanner; si es real → crear componente |
| `BROKEN` | Existe pero no está running o health failed | Investigar logs; restart si es transitorio; fix si es persistente |
| `BLOCKED` | No puede evaluarse porque dependencia falló | Arreglar la dependencia primero |
| `UNKNOWN` | Insuficiente evidencia (SSH falló, path no resuelto) | Re-ejecutar scan; verificar conectividad |
| `EXTRA` | Descubierto fuera del contrato canónico | Auditar si es legítimo (legacy, dev tool) o huérfano |

---

*Documento generado por IA OMEGA — Investigación Cuántica Aplicada.*
*Modo: Read-only audit / Planning. No se ejecutaron mutaciones.*
*SHA del plan: derivado de aa28fee + análisis de scanner v1.0.0*
