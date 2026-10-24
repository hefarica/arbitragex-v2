# Deploy-tree guard + watchdog teardown (VPS-DEPLOY-GUARD)

Incidente que este guard cierra: **2026-10-24 14:08Z**. Dos fallos independientes en el mismo deploy.

## 1. El árbol de deploy no era el SHA

`/opt/arbitragex-v2` tenía un fichero **NO trackeado**:

```
backend/api-server/src/routes/internal-am.ts
sha256 9476187bb38cf595db11e75777f8dc31f2c0c18302a273970c7c0b8672cba6e6
  import { Router } from "express";
  import { requireBearerToken } from "@arbx/shared";   <-- NO existe en @arbx/shared
```

`git reset --hard` (paso `[2/9]` del deploy) **no borra** ficheros untracked, así que el fichero sobrevivió, entró al
contexto Docker y rompió el `tsc` del api-server ⇒ el build abortó **~30 min** dentro, con un rojo que culpaba al SHA
cuando el SHA estaba perfectamente verde en CI. Tras ponerlo en cuarentena, los 12 servicios construyeron y el deploy terminó `success`.

Y hay una segunda variante del mismo mal: un fichero **trackeado sucio** en el árbol lo **destruye** el
`git reset --hard` del deploy — trabajo no commiteado que se pierde en silencio (medido 2026-10-24:
`M frontend/lib/hooks/usePricesStream.ts`).

## 2. El watchdog quedaba huérfano y colgaba el job

El suelo de disco del paso `[6/9]` corre en un subshell en background:

```bash
( while true; do …prune si el disco baja de 6 GB…; sleep 15; done ) &
DISK_FLOOR_PID=$!
for service in …; do docker compose … build --no-cache "$service"; done
kill "$DISK_FLOOR_PID"; wait "$DISK_FLOOR_PID"      # ← sólo en el camino feliz
```

Si **cualquier** cosa del medio falla (un build, un `docker compose`, un `exit`), el script muere antes del `kill`, el
subshell queda huérfano… y **hereda los pipes del `ssh`**. El runner nunca ve EOF ⇒ el job **se cuelga** hasta el
timeout (120 min) en vez de fallar rápido. En el incidente: **2 intentos colgados**.

**Fix**: `stop_disk_floor_watchdog()` (TERM → 2 s → KILL → `wait`) llamado desde `deploy_completion_guard`, que es la
función por la que **terminan los dos `trap … EXIT`** del script (el temprano y el de `deploy_self_heal`), más el
`::warning::` si el watchdog seguía vivo. El camino feliz usa la misma función (una sola ruta de limpieza).

## 3. Qué bloquea el guard (`deploy_tree_guard`)

Corre **dos veces** en el deploy, ambas antes de tocar Docker:

| Momento | Qué detecta |
|---|---|
| `pre-anchor` (antes de `git fetch`/`git reset --hard`) | trabajo local que el `reset` **destruiría** o que entraría al contexto Docker |
| `post-anchor` (tras verificar `HEAD == TARGET_SHA`) | cualquier resto untracked que sobrevivió al reset |

**Bloquea** ⇒ `::error::DEPLOY-TREE-GUARD` + salida con código ≠ 0 (falla rápido):

- cualquier fichero **trackeado sucio** (`M`, `A`, `D`, `R`…) en cualquier ruta;
- cualquier **untracked** que no esté en la allowlist de infraestructura.

**No bloquea** (por diseño):

- ficheros **gitignored** (`.env`, `monitoring/thanos/objstore.yml`, `archives/`, `.secrets/`…): son **parte esperada**
  del runtime del VPS y no viajan al contexto de build por git — el guard usa `git status` (ignorados excluidos).
- las **11 entradas untracked de infraestructura** medidas en el árbol el 2026-10-24 (allowlist explícita, ver §4).

## 4. Allowlist de infraestructura (11 entradas, medidas — no inventadas)

```
.secrets/                                                  archives/
docker/compose.dev.yml.pre-f1-20261018                     docker/compose.prod.yml.pre-r2.1792813353
monitoring/alertmanager/alertmanager.yml.pre-r2.1792813353 scripts/disk_guard.sh.pre-guard02-20261018
scripts/gsim1_variance_export.sql.bak                      scripts/pg_retention.sh.pre-f1p-20261018
scripts/pg_retention.sh.pre-fk02-20261018                  scripts/pg_retention.sh.pre-part03-20261018
scripts/price_seeder.sh.pre-pull-20260930T104945Z
```

Son backups/prefijos de infraestructura que ya vivían ahí. **Añadir una entrada a esta lista es una decisión de
operador** y debe justificarse en este documento: cada línea nueva es un fichero que puede volver a entrar al contexto
Docker sin que nadie lo mire. `scripts/ci/test_deploy_tree_guard.py` compara la allowlist del workflow con esta lista
documentada y **falla si divergen**.

## 5. Cuarentena REVERSIBLE (procedimiento)

Cuando el guard aborta, el mensaje del job ya imprime estos pasos. **Nada se borra**: se mueve.

```bash
# 0) entrar al árbol de deploy
cd /opt/arbitragex-v2

# 1) ver qué hay (el guard ya lo listó; esto es para inspeccionar)
git status --porcelain=v1 --untracked-files=all
git diff -- <ruta-trackeada-sucia>
cat <ruta-untracked>

# 2) cuarentena (una sola carpeta, con timestamp UTC)
install -d -m 700 /root/arbx-quarantine
TS=$(date -u +%Y-%m-%dT%H%MZ)
for f in <rutas reportadas>; do
  mv "$f" "/root/arbx-quarantine/$(basename "$f").$TS"
done

# 3) re-deploy (el guard pasa). Si el trabajo era legítimo:
#    commitear en una rama y mergear — el árbol de producción NO es un workspace.

# 4) DESHACER la cuarentena (reversibilidad)
mv /root/arbx-quarantine/<nombre>.<TS> <ruta original>
```

Precedente real: `internal-am.ts` se movió a `/root/arbx-quarantine/internal-am.ts.2026-10-24T1408Z` y el deploy
terminó en verde sin tocar ni un byte del fichero.

## 6. Modo emergencia (logged, nunca silencioso)

```
ARBX_DEPLOY_TREE_GUARD=warn   # exportado en el entorno remoto
```

Continúa el deploy con el árbol sucio y emite `::warning::DEPLOY-TREE-GUARD en modo warn: … deploy no reproducible`.
**No es un bypass**: no cambia el código, no relaja el gate por defecto (`enforce`) y deja traza en el log del job.
Usarlo sólo para desbloquear producción mientras se prepara la cuarentena o el commit.

## 7. Cómo verificarlo

```bash
# regresiones (sin SSH, sin Docker, sin cadena): ejecutan el guard REAL extraído del workflow
python3 -m unittest discover -s scripts/ci -p 'test_deploy_tree_guard.py' -v
```

Cubre: árbol limpio ⇒ pass · `internal-am.ts` untracked ⇒ **aborta rápido** con la ruta y el procedimiento ·
allowlist ⇒ pass · backup nuevo fuera de la allowlist ⇒ aborta · trackeado sucio ⇒ aborta · **tras cuarentena ⇒ pass** ·
`warn` ⇒ pass con warning · el guard corre **antes** del `reset` y del `build` · el `trap` de salida mata al watchdog
(probado matando un hijo real) · la allowlist del workflow == esta lista.

## 8. Rollback del propio guard

`git revert <sha>` y listo: el guard no toca datos, no migra nada y no persiste estado. El único efecto permanente es
el `::error::`/`::warning::` en el log del deploy.
