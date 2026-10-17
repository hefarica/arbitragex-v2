# SEC-ENV-EXPOSURE-01 — Material de secreto legible en claro en el entorno de los contenedores

- **run_id**: `arbx-entrega-20261007` · **phase_id**: `implementation`
- **SHA_BASE**: `21d2039cc80b8c47c3eea6b223ce7e663da604fd`
- **Dueño**: Security · **Permisos ejercidos**: SOLO LECTURA sobre el VPS
- **Artefacto**: censo, mecanismo, hoja de acción y frontera de lo no verificado.
- **CERO VALORES DE SECRETO EN ESTE DOCUMENTO.** Se documenta la CLASE, la RUTA y el MECANISMO. Los valores no se volcaron, no se citaron, no se copiaron y no se hashearon en ningún punto del relevamiento: todo comando de lectura de entorno usó un primitivo que descarta el valor *antes* de imprimir (ver Anexo A).
- **Materializa**: F30 ("secret material plaintext en `service_credentials` 7/7, Redis `nopass`, `appendonly yes`") y le agrega la **vía directa de extracción**.

---

## 0. Veredicto en una línea

**El secreto no está "en un archivo protegido": está en el `Config.Env` de 14 contenedores, legible en claro por `docker inspect` desde el host y por `env` desde dentro del contenedor, y la frontera de lectura no es `root` sino el grupo `docker` (que incluye dos cuentas no privilegiadas).** No existe hoy ningún mecanismo de secretos cableado: Vault corre **sellado** y ningún servicio tiene variables `VAULT_*`; el único `secrets:` de Docker está declarado en el compose pero **el archivo que referencia no existe en el host y ningún servicio lo consume**. La única barrera operativa real es *quién está en el grupo `docker`*.

---

## 1. C1 — Censo por servicio

### 1.1 Superficie

`docker ps` → **25 contenedores corriendo**, correspondencia 1:1 con los servicios del compose (`arbitragex-v2-<servicio>-1`).

### 1.2 Contenedores que exponen ≥1 variable de clase secreta

**CONTEO: 14 de 25 contenedores.** Slots totales de clase secreta en el fleet: **129**.

| Contenedor | Slots de clase secreta |
|---|---:|
| `arbitragex-v2-api-server-1` | 12 |
| `arbitragex-v2-edge-1` | 12 |
| `arbitragex-v2-searcher-rs-1` | 12 |
| `arbitragex-v2-sim-ctl-1` | 12 |
| `arbitragex-v2-recon-1` | 12 |
| `arbitragex-v2-selector-api-1` | 12 |
| `arbitragex-v2-relays-client-1` | 12 |
| `arbitragex-v2-anvil-1` | 12 |
| `arbitragex-v2-math-engine-1` | 12 |
| `arbitragex-v2-token-enricher-1` | 13 |
| `arbitragex-v2-postgres-1` | 4 |
| `arbitragex-v2-frontend-1` | 2 |
| `arbitragex-v2-grafana-1` | 1 |
| `arbitragex-v2-minio-1` | 1 |

**Sin ninguna (11):** `promtail`, `prometheus`, `redis`, `alertmanager`, `loki`, `socket-proxy`, `node_exporter`, `thanos-query`, `thanos-sidecar`, `thanos-store`, `vault`.

> **Dos falsos positivos que descarté explícitamente** (y que un conteo ingenuo por patrón de texto reportaría como exposición):
> - `vault` aparecía con 1 por una coincidencia dentro del **valor multilínea** de `VAULT_LOCAL_CONFIG` (JSON con `"tls_key_file": "/vault/tls/…"`). Es una **ruta a un archivo**, no un secreto. Con coincidencia exacta de nombre el conteo de `vault` es **0**.
> - El bloque común tiene **16** coincidencias por patrón pero **12 reales**: cuatro son parámetros, no secretos (`ARBX_TOKEN_ICON_DEXSCREENER`, `ARBX_TOKEN_ICON_DEX_TIMEOUT_MS`, `ARBX_TOKEN_ICON_NEG_TTL_SECS`, `ARBX_TOKEN_ICON_TTL_SECS`). En `minio`, cinco de sus seis son **apuntadores a archivo** (`*_FILE`) o una **clave pública** (`MINIO_UPDATE_MINISIGN_PUBKEY`).

### 1.3 Variables de clase secreta (NOMBRES) y cuántos contenedores las exponen

**14 variables distintas.** Cero valores.

| Variable (NOMBRE) | Contenedores | Consumidores principales (paths) |
|---|---:|---|
| `ARBX_ADMIN_TOKEN` | 11 | `backend/api-server/src/index.ts`, `routes/dexes.ts`, `websocket.ts`; `frontend/app/settings/credentials/*`, `frontend/lib/onboarding-phases.ts` |
| `ARBX_EDGE_TOKEN` | 11 | `backend/api-server/src/index.ts`; `frontend/lib/api-client.ts`, `frontend/app/settings/credentials/*` |
| `ARBX_MIGRATOR_PASSWORD` | 11 | roles de Postgres + clientes vía `DATABASE_URL` |
| `ARBX_RW_PASSWORD` | 11 | roles de Postgres + clientes vía `DATABASE_URL` |
| `ARBX_RO_PASSWORD` | 11 | roles de Postgres + clientes vía `DATABASE_URL` |
| `POSTGRES_PASSWORD` | 11 | `postgres` (bootstrap) + clientes |
| `MINIO_ROOT_PASSWORD` | 11 | `docker/compose.prod.yml`, `scripts/dr-drill.sh` |
| `JWT_SECRET` | 10 | `backend/api-server/src/routes/auth-siwe.ts` (SIWE), `routes/operator-selftest.ts` |
| `ARBX_SERVICE_TOKEN` | 10 | `backend/api-server/src/index.ts` (servicio↔servicio) |
| `GOPLUS_API_KEY` | 10 | `backend/selector-api/src/token_safety/goplus.ts`, `token_safety/client.ts` |
| `HONEYPOT_IS_API_KEY` | 10 | `backend/api-server/src/routes/operator-selftest.ts` |
| `GRAFANA_ADMIN_PASSWORD` | 10 | `docker/compose.prod.yml` (origen para Grafana) |
| `GITHUB_TOKEN` | 1 | `backend/token-enricher/src/main.rs`; `docker/compose.prod.yml`; `scripts/arbx-env-deploy/arbx_remote.sh` |
| `GF_SECURITY_ADMIN_PASSWORD` | 1 | `arbitragex-v2-grafana-1` (variable efectiva de Grafana) |

**Las tres de alto valor señaladas por el capitán — `ARBX_ADMIN_TOKEN`, `ARBX_EDGE_TOKEN`, `ARBX_MIGRATOR_PASSWORD` — están cada una en 11 contenedores**, no en uno.

### 1.4 ¿Puede leerlas un proceso no privilegiado? — sí, y nombro cuál

Medido, no supuesto:

| Hecho | Medición |
|---|---|
| Socket | `/var/run/docker.sock` → `srw-rw---- root:docker` |
| Grupo con acceso | `docker:x:988:arbx,deploy` |
| Usuarios con shell de login | `arbx` (1000), `deploy` (1001), `auditor` (1002) |

- **`arbx` y `deploy` son cuentas NO-ROOT que pueden extraer TODOS los secretos** con `docker inspect` o `docker exec` — sin autenticación adicional (el acceso al socket *es* la autenticación). Ninguno de los dos necesita ser root.
- **`auditor` (1002) queda FUERA del grupo `docker`** y por esa vía no accede. Es una separación real y hay que preservarla al rotar (ver C4, Fase 5).
- Por la vía *dentro* del contenedor (`env`): funciona en 24 de 25. **`socket-proxy` no tiene shell** (`sh` no existe en la imagen) → su `env` no es legible *desde dentro*; sí por `inspect` desde el host (donde su conteo de clase secreta es 0).
- **Paridad de las dos vías verificada** en tres contenedores de control, con **dos primitivos independientes** (`env` filtrado dentro vs `inspect` desde el host): `sim-ctl` 16 = 16 · `postgres` 4 = 4 · `minio` 6 = 6. Para `minio` el filtrado interno se hizo con builtins POSIX puros porque la imagen **no tiene `grep`**.

---

## 2. C2 — El mecanismo: es inherente a la inyección por `environment:`, y el vector real es `env_file:`

Archivo: **`/opt/arbitragex-v2/docker/compose.prod.yml`** (1013 líneas). Working dir del proyecto: `/opt/arbitragex-v2/docker`.

### 2.1 Inyección por archivo completo (la causa del bloque uniforme)

**10 servicios** declaran `env_file:` apuntando al archivo completo de entorno:

| Línea de `env_file:` | Ruta inyectada |
|---|---|
| 138, 177, 209, 251, 286, 325, 368, 407, 486, 713 | `../.env` → `/opt/arbitragex-v2/.env` |

Consecuencia: **todo `/opt/arbitragex-v2/.env` entra como variables de entorno de esos contenedores**, y de ahí el bloque común de 12 variables de clase secreta que aparece idéntico en 10 servicios. El propio compose lo deja escrito (comentario en la línea 473).

### 2.2 Inyección por interpolación explícita `environment:`

Sin un solo literal. Lo medido: las 11 líneas que mencionan una variable de clase secreta en el compose usan interpolación `${...}`; **la búsqueda de candidatos literales devolvió CERO líneas**. Líneas concretas: `58` (`POSTGRES_PASSWORD`), `60` (`ARBX_MIGRATOR_PASSWORD`), `62` (`ARBX_RW_PASSWORD`), `63` (`ARBX_RO_PASSWORD`), `420` (`ARBX_ADMIN_TOKEN`, `${VAR:?required}`), `490` (`ARBX_EDGE_TOKEN`), `551`/`558`, `650` (`GF_SECURITY_ADMIN_PASSWORD`, `GRAFANA_ADMIN_PASSWORD`), `720` (`GITHUB_TOKEN`), `881` (`MINIO_ROOT_PASSWORD`).

**Conclusión de C2:** la exposición **es inherente a la inyección** — `env_file` + `environment:` entregan el secreto como *variable de entorno en claro* dentro del contenedor, y Docker la persiste en `Config.Env` (visible desde el host por `inspect`, sin entrar al contenedor). No hay un error de configuración puntual: es el diseño de inyección.

### 2.3 Origen del material

- `/opt/arbitragex-v2/.env` → **`600 root:root`**, 11071 bytes.
- Hay **27 archivos** que matchean `/opt/arbitragex-v2/.env*`: el activo + **26 copias** (20 de ellas `.env.bak*` con nombre de fecha/rotación previa).
- Modos: **24 × `600 root:root`** · **1 × `600 deploy:deploy`** (`.env.crucible`) · **2 × `644 root:root`** (`.env.example`, `.env.mcp.example`).
- Los dos `644` son **plantillas** (`.example`) y su contenido **no fue leído** por la prohibición de esta tarea: lo declarado es su modo, su tamaño y que nombran 14 y 1 variables de clase secreta respectivamente. Si una plantilla contuviera valores reales sería un hallazgo aparte — **NO VERIFICADO** (ver C5).

> **Nota de mecánica** (documentada, **no medida en esta tarea**): `env_file`/`environment` se aplican al **crear** el contenedor. Cambiar `.env` sin recrear el contenedor no cambia su entorno (un `restart` no re-lee el archivo; `up -d` sí recrea). Impacta directo en la ventana de rotación de C4.

---

## 3. C3 — ¿Existe ya un mecanismo de secretos y está cableado? **No.**

| Mecanismo | Estado medido | ¿Cableado a los servicios? |
|---|---|---|
| **Vault** (`arbitragex-v2-vault-1`) | `Initialized=true`, **`Sealed=true`** (`Unseal Progress 0/2`, shamir 3-de-2, `v1.18.1`, storage `file`, `HA=no`) — `vault status` sale con código 2 | **NO.** Las únicas referencias en el compose son `VAULT_LOCAL_CONFIG` (841), `VAULT_ADDR` (847, 862), `VAULT_CACERT` (848), `VAULT_SKIP_VERIFY` (862) — **todas dentro del bloque del propio servicio `vault`**. Ningún contenedor de aplicación tiene una sola variable `VAULT_*` (verificado en el censo completo). |
| **Docker secrets** | Declarado en `/opt/arbitragex-v2/docker/compose.prod.yml:1011-1013` (`grafana_admin_password` → `file: /run/secrets/arbx/grafana_admin_password`) | **NO.** El archivo **no existe**: `ls /run/secrets/arbx/` → *No such file or directory*. Y **ningún servicio declara `secrets:`**: la única aparición de `secrets:` en el compose es la declaración top-level. Es un **secreto fantasma**: declarado, inexistente e inyectado igual por `environment:` en claro. |
| **Redis auth** | `redis-cli PING` → **`PONG` sin `AUTH`** ⇒ **`nopass` confirmado por ejecución** (corrobora F30) | n/a — no hay credencial que rotar; es una superficie abierta análoga. |

**Frase para el operador:** el proyecto **tiene** el mecanismo desplegado (Vault corriendo) y **hasta declaró** un Docker secret, pero **ninguno de los dos está en uso**: el 100% del material viaja hoy como variable de entorno en claro. Antes de rotar conviene decidir el destino de inyección, o se rotará dos veces.

---

## 4. C4 — Hoja de acción de rotación

> **Nada de esto se ejecutó.** La rotación es acto **irreversible del operador**. Esta hoja es una orden de trabajo, no un registro.

### 4.0 Precondiciones (ventana)

1. **Sin deploy en vuelo** y sin merges a `main` durante toda la ventana (un merge invalida el deploy por el check de same-SHA — regla vigente del pipeline).
2. Anotar el `SHA` de `main` y el estado del último `Auto-Deploy VPS (Post-E2E)` **antes** de empezar.
3. **Backup del `.env` vigente fuera del host** (el material viejo sigue siendo válido hasta que se revoque en el proveedor).
4. Tener a mano la vía de emergencia a Postgres por socket local (`docker exec` + `psql`) **antes** de tocar roles: es lo único que evita un lockout total si la rotación de PG queda a medias.

### 4.1 Orden exacto, dependencias y qué se rompe

| # | Objetivo | Dependencias | Qué se rompe si se hace mal | Reversible |
|---|---|---|---|---|
| **1** | **Claves de proveedores EXTERNOS**: `GOPLUS_API_KEY`, `HONEYPOT_IS_API_KEY`, `GITHUB_TOKEN`, `ARBX_TOKEN_ICON_DEXSCREENER` | Revocar+regenerar **en el proveedor** (GoPlus / honeypot.is / GitHub / DexScreener) | En cuanto se revoca, **cualquier consumidor con el valor viejo deja de funcionar al instante**: `selector-api` (sanidad de tokens), `api-server` (selftest), `token-enricher` (metadatos) | **NO.** Es el paso irreversible: el valor viejo muere en el proveedor y no se puede volver atrás. **ACTO DEL OPERADOR.** |
| **2** | **Pares atómicos de token** (deben cambiar en el MISMO `up -d`): `ARBX_ADMIN_TOKEN` (11 contenedores) y `ARBX_EDGE_TOKEN` (11) | `.env` + recreación **simultánea** de `api-server`, `frontend`, `edge` (y los 8 restantes del bloque) | Si se recrea un lado y no el otro: **401 en las rutas admin** y **fallo del camino frontend→edge→api-server** hasta que ambos coincidan. Con `ARBX_ADMIN_TOKEN` desincronizado el operador **pierde su propia UI de administración** | Sí (re-poner el valor anterior en `.env` + recrear), **pero el valor viejo queda comprometido para siempre** |
| **3** | `ARBX_SERVICE_TOKEN` (10), `JWT_SECRET` (10) | Igual que #2 | `JWT_SECRET`: **invalida todas las sesiones/JWT vigentes** → re-login forzado (impacto deseable, no incidente). `ARBX_SERVICE_TOKEN`: rompe servicio↔servicio hasta recrear todos | Sí |
| **4** | **Plano de datos: los 4 de Postgres** — `POSTGRES_PASSWORD`, `ARBX_MIGRATOR_PASSWORD`, `ARBX_RW_PASSWORD`, `ARBX_RO_PASSWORD` (11 contenedores cada uno) | `ALTER ROLE … PASSWORD` **dentro** de PG usando la vía de emergencia + `.env` + recreación de los 10 clientes | **Es el paso de mayor blast radius.** `docker-entrypoint-initdb.d` sólo corre con el directorio de datos **vacío**: no sirve para rotar; hay que hacer `ALTER ROLE`. Entre el `ALTER ROLE` y la recreación de los 10 clientes **hay una ventana en la que los clientes siguen presentando la credencial vieja y fallan autenticando**. Un error acá **bloquea todos los servicios** | Sí, **sólo** si se conservó la vía de emergencia por socket local; sin ella, no |
| **5** | **Hojas**: `GRAFANA_ADMIN_PASSWORD` / `GF_SECURITY_ADMIN_PASSWORD` (Grafana), `MINIO_ROOT_PASSWORD` (11 contenedores + `scripts/dr-drill.sh`) | Recrear `grafana`, recrear `minio` + cualquier cliente con credencial root | Grafana: se resetea la clave admin al recrear (esperado). MinIO: atención a `MINIO_ROOT_PASSWORD_FILE` / `MINIO_*_KEY_FILE` presentes **además** de las variantes en claro — **no verificado cuál gana** (ver C5) | Sí |
| **6** | **Higiene de copias**: purgar/destruir las **26 copias** `.env*` (20 `.env.bak*`, `.env.crucible` en `deploy:deploy`) | — | **Sin este paso la rotación es INCOMPLETA**: el valor viejo sigue en claro en 26 archivos `600 root:root` que el propio tooling de rotación podría restaurar por error | **NO** para las copias (destruirlas es el objetivo); conservarlas es lo que no se puede |

### 4.2 El paso irreversible, explícito

**`#1` (revocación/regeneración en los proveedores externos) y la destrucción de las 26 copias `.env*` son irreversibles y son ACTO DEL OPERADOR.** El resto es reversible *si y sólo si* se preserva la vía de emergencia a Postgres.

### 4.3 Fix de inyección (elegir UNO antes o junto con la rotación)

- **Opción A — Docker secrets.** Convertir el `secrets:` de la línea 1011 en real: crear el archivo en el host (`/run/secrets/arbx/<nombre>`, `0640`, dueño del grupo que corre el servicio) y declarar `secrets:` **por servicio** con el montaje en `/run/secrets/…`. Ventaja: elimina el paso 6 del problema de raíz. **Verificar el flujo de deploy**: el archivo debe existir en cada host y sobrevivir a los despliegues.
- **Opción B — Vault, que ya está desplegado.** Requiere, en este orden: **desellarlo** (shamir 3-de-2: hacen falta 2 de las 3 shares; hoy `Unseal Progress 0/2`), crear el engine y las políticas, **y cablear** las variables `VAULT_*` + el agente/entrypoint en los servicios. Es más trabajo que A y hoy agrega una dependencia dura al arranque de cada servicio: si Vault queda sellado tras un reinicio, los servicios no arrancan.
- **Recomendación de esta tarea (fundamentada, no vinculante):** **A ahora, B después.** Docker secrets corta la vía de exposición con el mínimo cambio y sin nueva dependencia de arranque; Vault ya desplegado se aprovecha cuando exista el procedimiento de desellado automático (hoy **no verificado**: no hay evidencia de un mecanismo de auto-unseal — `Seal Type shamir`).

### 4.4 Verificación al cerrar la rotación

1. Re-ejecutar el censo **por conteo** (Anexo A) y confirmar que los 14 contenedores siguen exponiendo las mismas **clases** (el censo no prueba la rotación: los nombres no cambian).
2. Confirmar que los contenedores fueron **recreados** (no sólo reiniciados) y que su `Config.Env` refleja el `.env` nuevo.
3. Confirmar `docker-entrypoint-initdb.d` **no** fue la vía usada en PG (habría sido un no-op sobre datos existentes).
4. Repetir `redis-cli PING` → si sigue dando `PONG` sin `AUTH`, **la superficie Redis sigue abierta** (es un hallazgo aparte de F30, no lo cierra esta rotación).

---

## 5. C5 — Frontera de lo NO VERIFICADO (declarado, no asumido)

Todo lo siguiente **no fue verificado** en esta tarea, con su razón:

| # | No verificado | Razón |
|---|---|---|
| 1 | Si el material almacenado en Postgres (`pg_authid`) coincide con el valor del entorno | Leerlo exige manipular material de credencial. **Prohibido en esta tarea.** |
| 2 | Si las **26 copias** `.env.bak*` / `.env.crucible` contienen los **mismos** valores que `.env` | Requiere leer valores. (Sus tamaños difieren: 5037–19253 B ⇒ son generaciones distintas, pero **eso no prueba** coincidencia ni divergencia.) |
| 3 | Si `.env.example` / `.env.mcp.example` (modo `644`, world-readable) contienen **valores reales o placeholders** | Requiere leer valores. Por nombre son plantillas y nombran 14 y 1 variables de clase secreta; **el contenido no se leyó**. |
| 4 | Si Vault almacena alguna de estas credenciales | Vault está **sellado**: no se puede leer. Irrelevante mientras no esté cableado. |
| 5 | Si un pipeline de logs (promtail→loki) u otra exportación ya persistió un volcado de entorno | Requeriría buscar material de secreto en los logs. No es parte de esta tarea. |
| 6 | Si existe un authorization plugin que restrinja `docker exec` | No se inspeccionó el daemon. Lo medido es el **acceso por permisos de socket**, que hoy alcanza. |
| 7 | Si las claves de terceros siguen activas/válidas | Requeriría llamar a los proveedores. No se hizo ninguna llamada externa. |
| 8 | Si el historial de git (tracked) contiene alguno de estos valores | Fuera del alcance de esta tarea (un hallazgo análogo se trató en `GATE-SECRETOS-01`). |
| 9 | En MinIO, cuál de los dos gana: `MINIO_ROOT_PASSWORD` (en claro) o `MINIO_ROOT_PASSWORD_FILE` | Ambos nombres están presentes; no se probó precedencia (probar exigiría manipular credenciales). |
| 10 | Si `restart` vs `up -d` re-lee `.env` | Se declara como **mecánica documentada de Docker** aplicada al plan, **no medida** en esta tarea. |

---

## 6. C6 — Auto-verificación de CERO valores en este artefacto

Ejecutada **sobre este mismo archivo, antes de entregarlo**, con los resultados crudos:

1. **Líneas tipo `NOMBRE=valor`** — patrón `^[A-Z][A-Z0-9_]{2,}=` → **0 coincidencias**. No hay ni una asignación de secreto en el documento.
2. **Runs alfanuméricos largos (≥20 chars), candidatos a valor** — patrón `[A-Za-z0-9+/]{20,}` → **6 distintos, los 6 clasificados y benignos**:
   - `/run/secrets/arbx/grafana` — **ruta**
   - `21d2039cc80b8c47c3eea6b223ce7e663da604fd` — el **`SHA_BASE` citado del encabezado** (un SHA de commit, público por diseño; aparece una sola vez, como cita)
   - `frontend/app/settings/credentials/`, `frontend/lib/onboarding`, `server/src/routes/auth`, `server/src/routes/operator` — **paths de código**
   → **cero runs de tipo valor de credencial.**
3. **Asignaciones fuera de interpolación** — patrón `=\s*[A-Za-z0-9]` → **5 coincidencias, las 5 explicadas**: dos son aritmética en prosa (`16 = 16`, `4 = 4`), una son **campos de estado de Vault** (`Initialized=true`, `HA=no` — estado del servicio, no material de credencial), una es el texto citado de mi propio patrón de `grep`, y dos son fragmentos de `awk` del Anexo A.
4. **Garantía estructural:** ningún valor fue capturado en ningún punto del relevamiento. Todos los comandos de entorno usaron `cut -d= -f1` o `while IFS="=" read -r k _`, que **descartan el valor antes de imprimir** (Anexo A). No hay valor que pueda filtrarse a este documento porque **ninguno pasó por stdout**.

---

## Anexo A — Comandos reproducibles (todos de lectura, todos de conteo/nombres)

```bash
# Superficie
docker ps --format '{{.Names}}'

# CENSO 1 — conteo por contenedor (patrón laxo; incluye falsos positivos)
for c in $(docker ps --format '{{.Names}}'); do
  n=$(docker exec "$c" sh -c 'env | cut -d= -f1 | grep -c -i -E "TOKEN|PASSWORD|SECRET|KEY"' 2>/dev/null)
  echo "$c|${n:-NA}"
done

# CENSO 2 — NOMBRES por contenedor, vía host (cubre los que no tienen shell).
#   cut -d= -f1 descarta el VALOR antes de que grep vea nada.
for c in $(docker ps --format '{{.Names}}'); do
  echo "$c :: $(docker inspect "$c" --format '{{range .Config.Env}}{{println .}}{{end}}' \
    | cut -d= -f1 | grep -i -E "TOKEN|PASSWORD|SECRET|KEY" | sort -u | paste -sd, -)"
done

# CENSO 3 — primitivo POSIX puro (para imágenes SIN grep, p.ej. minio).
#   IFS="=" read parte en el primer '=' y tira el resto a _ => el valor NUNCA se imprime.
docker exec <container> sh -c 'env | while IFS="=" read -r k _; do
  case "$k" in *TOKEN*|*PASSWORD*|*SECRET*|*KEY*) echo "$k";; esac; done'

# Mecanismo de inyección (archivo + línea, sin valores)
grep -n 'env_file' /opt/arbitragex-v2/docker/compose.prod.yml | cut -d: -f1
awk '/^ *env_file:/{ln=NR; getline; gsub(/^[ \t]+/,""); print ln": "$0}' /opt/arbitragex-v2/docker/compose.prod.yml
grep -n -o -E '[A-Za-z_][A-Za-z_0-9]*(TOKEN|PASSWORD|SECRET|KEY)[A-Za-z_0-9]*' /opt/arbitragex-v2/docker/compose.prod.yml
grep -c -E '(TOKEN|PASSWORD|SECRET|KEY)' /opt/arbitragex-v2/docker/compose.prod.yml   # vs cuántas usan ${

# ¿Existe el mecanismo?
docker exec arbitragex-v2-vault-1 vault status          # exit 2 => sealed
ls -la /run/secrets/arbx/                               # no existe
grep -n -E '^ {0,4}secrets:' /opt/arbitragex-v2/docker/compose.prod.yml

# Frontera de lectura
stat -c '%A %U:%G %n' /var/run/docker.sock
getent group docker
getent passwd | awk -F: '$3>=1000 && $3<65534'

# Redis
docker exec arbitragex-v2-redis-1 redis-cli PING        # PONG sin AUTH => nopass

# Higiene de copias
find /opt/arbitragex-v2 -maxdepth 1 -name '.env*' -type f -printf '%m %u:%g %s %f\n' | sort | uniq -c
```

**Regla que estos comandos respetan y que debe seguirse en cualquier repetición:** nunca `docker exec <c> env` sin filtrar. Un volcado crudo reexpone el material en la terminal, en el historial y en los logs del agente.

---

## Anexo B — Qué NO hace este artefacto

- No rotó nada. No reinició nada. No escribió nada en el VPS.
- No contiene ningún valor de secreto (C6).
- No afirma que el material esté comprometido: afirma que es **legible por una frontera más amplia que `root`**, que es un hecho distinto y suficiente para rotar.
- No cierra F30: `Redis nopass` **sigue abierto** y se confirmó por ejecución en esta tarea.
