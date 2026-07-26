# SSH CI/CD Protocol — Paridad con GitHub Actions (sin Actions)

> **Propósito:** Operar el VPS por SSH con el **mismo rigor** que un pipeline CI/CD:
> identidad fijada, lock, dry-run obligatorio, apply tipado, verify fail-closed,
> rollback determinista, artefacto de auditoría.  
> **No** es un atajo. Es CI/CD ejecutado por el operador con disciplina de máquina.

| Campo | Valor |
|-------|--------|
| Driver local | [`scripts/vps/ssh-cicd-driver.sh`](../../scripts/vps/ssh-cicd-driver.sh) |
| Verify | [`scripts/vps/verify-deploy.sh`](../../scripts/vps/verify-deploy.sh) |
| Baseline Gate 0 | [`scripts/vps/capture-baseline.sh`](../../scripts/vps/capture-baseline.sh) |
| Deploy selectivo VPS | [`scripts/vps/deploy-efficient.sh`](../../scripts/vps/deploy-efficient.sh) |
| Referencia Actions | `.github/workflows/hardened-vps-deploy.yml`, `deploy-vps.yml`, `ops-paper-mode.yml` |
| Host | SSH alias `arbx` → `/opt/arbitragex-v2` |
| Postura capital | **paper/shadow only** — mainnet live físicamente fuera de este protocolo |

---

## 1. Principios (inviolables)

1. **Fail-closed.** Cualquier duda → abort. No “sigue y ya vemos”.
2. **Dry-run por defecto.** Mutar requiere flags explícitos **y** confirmación de SHA.
3. **Identidad fijada.** Se despliega un **SHA de 40 hex** conocido, no “lo último que haya”.
4. **Un solo writer.** Lock en VPS; segundo deploy concurrente = abort.
5. **Nunca ciego.** Baseline pre-estado → apply → verify post-estado → comparar.
6. **Nada de live.** Este protocolo **no** toca `ARBX_LIVE_EXEC_ENABLED`, no pone
   `ARBX_TRADE_MODE=live`, no mete signer de mainnet, no hace broadcast.
7. **No borrar estado.** Prohibido `docker compose down`, `docker system prune -a`,
   `git clean -fdx` sobre datos, drop de DB, flush Redis de streams de producción
   salvo runbook de incidente aparte con OK explícito.
8. **Secrets fuera de git.** `.env` del VPS no se pisa con el del laptop. Upserts de
   env solo con runbook `ops-paper` y nombres de clave, nunca pegar secretos en el historial.
9. **Código canónico = GitHub.** El commit debe existir en `https://github.com/hefarica/arbitragex-v2.git`
   (o el remote canónico del clone). No rebasar ni “reset” a un mirror bare stale.
10. **Audit trail.** Cada corrida deja un directorio de artefacto con logs, SHAs, diffs, verify.

---

## 2. Mapa mental: Actions ↔ SSH

| Gate de Actions (hardened / deploy-vps) | Equivalente SSH |
|------------------------------------------|-----------------|
| `workflow_dispatch` + inputs | Flags del driver + confirm SHA |
| `confirm_token == target_sha` | `--confirm-sha` debe ser idéntico a `--sha` |
| `environment: production` reviewer | El operador es el reviewer (no hay auto) |
| Lock `/tmp/arbitragex_deploy.lock` | Mismo lock (o el del efficient-deploy) |
| Baseline pre-state artifact | `pre_state.*` en `repo-vps-audits/SSH-CICD-<ts>/` |
| `git fetch` + `cat-file` target | Fetch canónico + verify object exists |
| `git reset --hard <sha>` solo tras verify | Solo en fase APPLY, nunca en dry-run |
| `docker compose ... up -d --build` | Selective via `deploy-efficient` o lista explícita |
| Health curl api/edge | `verify-deploy.sh` + paper-path R7 extendido |
| Auto-rollback on health fail | `deploy-efficient` ya rollback; driver también expone `--rollback` |
| No secret/config via workflow | Driver bloquea change_type `secret` |
| Concurrency group | Lock file + abort si existe |

---

## 3. Clasificación de cambio (`change_type`)

Elegir **uno**. Define qué se puede tocar y qué approvals extras hacen falta.

| change_type | Qué toca | Extra |
|-------------|----------|--------|
| `docs-only` | Nada de containers (opcional sync git only) | Ninguno |
| `frontend-only` | `frontend` (+ `--no-cache` si `NEXT_PUBLIC_*`) | RULE 03/04 |
| `edge-only` | `edge` | — |
| `api-server-only` | `api-server` | hot-path flag |
| `searcher-rs/hot-path` | `searcher-rs` (+ deps rust compartidas) | `--hot-path-ok` |
| `relays-sim-selector` | `relays-client` `sim-ctl` `selector-api` | paper-path focus |
| `docker/compose` | rebuild amplio | `--hot-path-ok` |
| `database/migrations` | **BLOQUEADO** en driver salvo `--db-backup-ok` + runbook aparte | backup obligatorio |
| `secret/config-sensitive` | **SIEMPRE BLOQUEADO** en este protocolo | manual runbook |
| `mixed-change` | varios servicios | `--hot-path-ok` |

---

## 4. Prerrequisitos

### Local (Windows + Git Bash)

- `ssh` alias `arbx` funciona: `ssh -o BatchMode=yes arbx 'echo OK'`
- Repo clone limpio o con SOLO cambios que vas a commitear
- `git` + acceso push al remote canónico (GitHub)
- El SHA a desplegar **ya está en el remote canónico** (`git push` hecho)

### VPS

- Path `/opt/arbitragex-v2`
- Compose: `docker/compose.prod.yml`
- `.env` presente (no lo sobrescribe el driver)
- Docker healthy; disco con margen (baseline lo reporta)
- `ARBX_TRADE_MODE=paper` (o shadow) — verify falla si no

### Mental

- Tienes 30–90 min sin interrupciones para apply real
- Sabes el rollback SHA actual del VPS (el driver lo captura)

---

## 5. Fases operativas

### Fase 0 — Preflight LOCAL (antes de SSH mutante)

```bash
# En el clone local
git fetch origin          # o: git fetch github  (si tu canónico se llama así)
git status -sb
git log -1 --oneline
# Tests del área tocada (ejemplos)
# cargo check -p relays-client -p searcher-rs
# cd frontend && npx tsc --noEmit
```

**Abort si:** working tree sucio no relacionado, tests rojos, SHA no pusheado.

### Fase 1 — Pin de identidad

```text
TARGET_SHA = full 40-char commit que YA está en el remote canónico
```

Comprueba en GitHub UI o:

```bash
git cat-file -t "$TARGET_SHA"   # commit
git branch -r --contains "$TARGET_SHA"
```

### Fase 2 — Dry-run (OBLIGATORIO, default)

Desde la raíz del repo local:

```bash
bash scripts/vps/ssh-cicd-driver.sh \
  --sha <40hex> \
  --change-type relays-sim-selector \
  --dry-run
```

El driver:

1. Crea `repo-vps-audits/SSH-CICD-<UTC>/`
2. SSH read-only: hostname, HEAD, status, docker ps, disk, paper mode
3. Verifica que el objeto `$TARGET_SHA` es alcanzable tras `git fetch` en VPS
   (si el remote del VPS no tiene el commit → **FAIL** y te dice que hagas push/fetch primero)
4. Diff `VPS_HEAD..TARGET_SHA` (nombres de archivo)
5. Clasifica servicios que **se tocarían**
6. Corre verify-deploy en modo informe si pides `--verify-now` (read-only)
7. **No** adquiere lock de apply, **no** reset, **no** compose build
8. Escribe `DRY_RUN_REPORT.md` y sale 0 solo si preflight OK

### Fase 3 — Apply (explícito)

Solo después de leer el dry-run:

```bash
bash scripts/vps/ssh-cicd-driver.sh \
  --sha <40hex> \
  --confirm-sha <40hex> \
  --change-type relays-sim-selector \
  --apply \
  --hot-path-ok
```

Reglas fail-closed del apply:

- `--confirm-sha` **==** `--sha` (como `confirm_token` de hardened)
- `change_type` no es `secret/config-sensitive`
- migrations → requiere `--db-backup-ok`
- hot-path types → requieren `--hot-path-ok`
- Adquiere lock; si existe → abort
- Escribe baseline pre-state
- Guarda rollback SHA (HEAD actual VPS) en artefacto **y** `.last-known-good-commit` en VPS
- `git fetch` + verify object + `git checkout --detach $SHA` o `reset --hard $SHA`
  **solo** del worktree de código (nunca borra `.env`)
- Build/up **selectivo** según change_type (reutiliza lógica efficient)
- Siempre `--env-file .env -f docker/compose.prod.yml`
- Nunca `compose down`
- Libera lock en EXIT (trap)
- Corre `verify-deploy.sh` (+ paper-path gates del driver)
- Si verify CRITICAL falla → rollback automático al SHA guardado + re-verify

### Fase 4 — Verify (R7 + paper path)

Además de lo que ya hace `verify-deploy.sh` (containers, redis ping, pg ready,
api/edge health, CSP, trade mode paper):

El driver exige (fail en `--strict`, warn si no):

| Check | Criterio |
|-------|----------|
| `XLEN arbx:opps:detected` | legible (0 = warn, no inventar) |
| `XLEN arbx:opps:validated` | reportar; 0 sostenido = **paper-path broken** |
| `XLEN arbx:opps:simulated` | idem |
| `paper_trade_runs` MAX(created_at) | stale > 24h = warn/fail strict |
| rejection mix última hora | top reasons (forense, no mock) |
| `relays_consumer` | log boot: spawned vs skipped |
| Live exec | `ARBX_LIVE_EXEC_ENABLED` no true en mainnet path |
| Watchlist liq | SCARD report only |

### Fase 5 — Rollback

```bash
bash scripts/vps/ssh-cicd-driver.sh --rollback
# o con SHA explícito del artefacto:
bash scripts/vps/ssh-cicd-driver.sh --rollback --sha <previous40hex> --confirm-sha <previous40hex> --apply
```

Rollback:

1. Lock
2. `git reset --hard $ROLLBACK_SHA`
3. `docker compose --env-file .env -f docker/compose.prod.yml up -d` (servicios tocados o full según artefacto)
4. verify-deploy
5. Artefacto `ROLLBACK_*.md`

### Fase 6 — Artefacto de auditoría

Cada corrida deja:

```text
repo-vps-audits/SSH-CICD-<UTC>/
  meta.json              # sha, host, operator, dry_run|apply, change_type
  pre_state.txt
  post_state.txt
  diff_names.txt
  services_selected.txt
  verify_deploy.log
  paper_path_r7.log
  DRY_RUN_REPORT.md | APPLY_REPORT.md
  ROLLBACK_REPORT.md     # si aplica
```

**No** incluye secretos ni volcado de `.env` (solo presencia de claves allowlisted).

---

## 6. Happy path (una sesión)

```bash
# 0) Local: commit + push canónico
git push origin HEAD

# 1) SHA
SHA=$(git rev-parse HEAD)

# 2) Dry-run (siempre)
bash scripts/vps/ssh-cicd-driver.sh --sha "$SHA" --change-type relays-sim-selector --dry-run

# 3) Leer repo-vps-audits/SSH-CICD-*/DRY_RUN_REPORT.md
# 4) Apply solo si el reporte es honesto y aceptable
bash scripts/vps/ssh-cicd-driver.sh \
  --sha "$SHA" --confirm-sha "$SHA" \
  --change-type relays-sim-selector \
  --apply --hot-path-ok --strict

# 5) Si rojo: el driver ya intentó rollback; si no,:
bash scripts/vps/ssh-cicd-driver.sh --rollback --strict
```

---

## 7. NEVER-DO (lista de quemaduras reales del proyecto)

| Prohibido | Por qué |
|-----------|---------|
| `git reset --hard origin/main` sin pin SHA | drift; mirror stale; deploy no reproducible |
| Confiar en remote bare VPS como canónico sin verificar | historial paralelo / commits fantasma |
| `docker compose down` / `down -v` | tumba estado y a veces volúmenes |
| `docker compose build` sin `--env-file .env` | RULE 04 — frontend hornea localhost |
| `restart` frontend tras cambiar `NEXT_PUBLIC_*` sin rebuild no-cache | RULE 03 |
| Deploy con working tree VPS sucio sin clasificar | pisas hotfixes manuales |
| Dos deploys SSH a la vez | race en git + compose |
| Meter `FLASHBOTS_SIGNER_KEY` mainnet “para que arranque el consumer” | capital path; paper debe vivir sin signer |
| `ARBX_TRADE_MODE=live` / `ARBX_LIVE_EXEC_ENABLED=true` en este flujo | fuera de alcance |
| Borrar lock ajeno sin forense | puedes matar deploy concurrente a medias |
| `git clean -fd` en `/opt/arbitragex-v2` | borra artefactos/logs/.env locales |
| Aplicar migrations sin backup + flag | pérdida de datos |
| “Solo un sed al .env en caliente” sin backup timestamped | ops-paper ya exige backup |

---

## 8. Qué reutiliza vs qué añade

| Pieza | Estado |
|-------|--------|
| Lock | PRESENT (`deploy-efficient`, hardened) — driver unifica |
| Rollback file | PRESENT `.last-known-good-commit` |
| verify-deploy | PRESENT — driver lo invoca + paper-path extra |
| capture-baseline | PRESENT Gate 0 — opcional `--gate0` |
| deploy-efficient | PRESENT selectivo — driver puede delegar apply |
| confirm_token | PRESENT en Actions — driver lo emula |
| Paper-path R7 (validated/simulated/paper freshness) | **PARTIAL** en verify hoy — driver lo añade |
| Driver orquestador local | **NUEVO** `ssh-cicd-driver.sh` |

---

## 9. Relación con el paper-path roto (contexto 2026-07-26)

El forense R7 mostró: `detected` lleno, `validated=0`, `simulated=0`,
`paper_trade_runs` stale desde 2026-07-17, `relays_consumer.skipped` sin signer.

Este protocolo **no repara** eso solo: repara **cómo** subes el fix.
El fix de código (consumer paper sin signer, price path, etc.) va:

1. Commit local + push GitHub  
2. Dry-run SSH con este driver  
3. Apply pin SHA  
4. Verify paper-path gates (deben ponerse verdes **después** del fix, no antes)

---

## 10. Cuándo SÍ usar GitHub Actions en vez de este driver

- Quieres reviewer environment enforced por GitHub
- El laptop no tiene la llave o la red al VPS es mala
- Auditoría org-level necesita run_id de Actions

Cuando uses Actions, **los mismos principios** de este doc aplican; el driver
es la versión “operador como runner”.

---

## 11. Checklist rápido pre-apply

- [ ] SHA 40 hex pusheado al canónico  
- [ ] Dry-run leído; servicios listados tienen sentido  
- [ ] `change_type` correcto; flags hot-path/db si aplican  
- [ ] Nadie más deployando (lock libre en dry-run report)  
- [ ] Paper mode sigue paper en pre_state  
- [ ] Rollback SHA anotado  
- [ ] Ventana de tiempo y plan B  
- [ ] **No** se van a tocar secrets ni migrations “de pasada”  
