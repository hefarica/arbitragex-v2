# TOKEN-EXPOSURE-ACTION-SHEET — rotación del token vivo de `.claude/settings.json`

- **Incidente:** SEC-TOKEN-ROTATION-01 (E-7)
- **Estado:** **ABIERTO — requiere acción del OPERADOR.** Ningún cambio de código lo cierra.
- **Clase de credencial:** token de autenticación de la superficie de modelos
  (`ANTHROPIC_AUTH_TOKEN`, consumido por el cliente `claude-code` / gateway `CCR`).
- **No es:** clave de firmante, seed, ni credencial on-chain. No habilita broadcast ni capital.
- **Valor:** NUNCA se imprime en este documento, en logs, en mensajes ni en chat. Todo el
  documento identifica la credencial por **huella** (`sha256` truncado), que permite
  comparar identidad sin revelar el secreto.
- **Autor del análisis:** Security (t3, `E-7`). **Decide y ejecuta:** el operador.
- **Base medida:** candidato `858b943fd80c8b5e1606d220aa87d63b86f4ba15` (rama
  `fix/perhop-reserves-01`) **y** `origin/main` `274f04fd` — ver §3, se declaran por
  separado porque difieren.

---

## 1. Reproducción de la exposición (sin imprimir el valor)

| # | Ruta | Blob sha (git) | Línea / clave | Huella del valor |
|---|------|----------------|---------------|------------------|
| **P1** | `.claude/settings.json` | `15a78fe98bdf29220f8f91901e84f0d781c8a1f0` (árbol de `HEAD` `858b943f`) | `.env.ANTHROPIC_AUTH_TOKEN` | `sha256_12=6c45ce87b547`, `len=50`, `uniq=33` |
| P1' | `.claude/settings.json` (disco, sin commitear) | `ccf75ea50b9fa480ac0d34b947fc87a7d921dded` | `.env.ANTHROPIC_AUTH_TOKEN` | **misma huella** `sha256_12=6c45ce87b547` |

Comandos reproducibles (ninguno imprime el valor):

```bash
git rev-parse HEAD:.claude/settings.json            # -> 15a78fe98bdf29220f8f91901e84f0d781c8a1f0
git hash-object .claude/settings.json               # -> ccf75ea50b9fa480ac0d34b947fc87a7d921dded
git ls-files -s .claude/settings.json               # -> 100644 15a78fe9... 0  .claude/settings.json
# parseo JSON + huella (nunca imprime el valor):
git show HEAD:.claude/settings.json | python -c "import sys,json,hashlib;p=json.load(sys.stdin);v=p['env']['ANTHROPIC_AUTH_TOKEN'];print('len',len(v),'sha256_12',hashlib.sha256(v.encode()).hexdigest()[:12])"
# -> len 50 sha256_12 6c45ce87b547
```

**Hito del disco:** el token que vive hoy en el checkout es **la misma unidad** que el
commiteado (huella idéntica `6c45ce87b547`). No es una copia vieja: es la credencial viva,
trackeada **y** en disco.

### 1.1 Hallazgo de método (por qué un grep normal no lo ve)
El patrón habitual `TOKEN\s*[:=]\s*"..."` **falla** en JSON: la clave va entrecomillada, así
que tras `ANTHROPIC_AUTH_TOKEN` viene `"`, luego `:`. Un barrido con ese patrón devuelve
`NO-LITERAL-MATCH` sobre este fichero — falso negativo verificado en esta misma sesión. La
detección correcta es *parsear el JSON* (o barrer literales entrecomillados ≥16 chars), no
grepear `clave: valor`.

---

## 2. Refs alcanzables — medido **por base**

| Base | ¿El path está en el árbol? | ¿El objeto (blob) sigue alcanzable? | Evidencia |
|------|---------------------------|-------------------------------------|-----------|
| Candidato `858b943f` (`fix/perhop-reserves-01`) | **SÍ** — entrada de índice `100644 15a78fe9…` | **SÍ** | `git ls-files -s` |
| `origin/CATALOG-HYGIENE-01` (ref remota) | **SÍ** (contiene `858b943f`) | **SÍ** | `git branch -r --contains HEAD` → `origin/CATALOG-HYGIENE-01`; `git ls-remote --heads origin` → `7693b6067f95d0455a338c45792365bb839339cc` |
| `origin/main` `274f04fd` | **NO** (`fatal: path '.claude/settings.json' exists on disk, but not in 'origin/main'`) | **SÍ, en HISTORIA**: el commit de untrack `91957994c06c` es ancestro de `origin/main` (`git merge-base --is-ancestor 91957994c06c origin/main` → exit 0) | comando citado |
| `refs/heads/ac-base` | — | **SÍ** | `git for-each-ref --contains HEAD` |
| `refs/dsh/checkpoints/session-*` (locales, 14 refs) | — | **SÍ** | `git for-each-ref --contains HEAD` |

**El matiz que no se puede perder:** `origin/main` **removió el fichero del árbol** (commit
`91957994c06c`, `2026-09-30`, "fix(security): SECRETS-UNTRACK-01 - .claude/settings.json
fuera del indice (token vivo) (#746)"). Remover del árbol **no** es rotar: el objeto del
token sobrevive en la historia de `main` y se sigue sirviendo a cualquiera que clone
`main`. Declarar "main lo resolvió" sería confundir un control (un fichero fuera del
índice) con el cierre del incidente (la credencial muerta).

### 2.1 Publicación desde el candidato = re-publicación del token
`858b943f` está **174 commits detrás** de `origin/main` y conserva el fichero **trackeado**.
Publicar ese candidato (o cualquier forward-port que arrastre su árbol) vuelve a publicar
la credencial. Nota de la célula: la base recomendada pasó a ser un forward-port sobre
`origin/main`, **no** la publicación del candidato; ambas mediciones quedan arriba para que
la decisión no dependa de cuál base se elija.

---

## 3. Re-materialización activa (la exposición no está congelada)

```bash
git log --all --format='%H|%ad|%s' --date=short --find-object=15a78fe98bdf29220f8f91901e84f0d781c8a1f0
# -> 20 commits referencian el blob; 12 de ellos son "dsh-checkpoint session-... turn 1"
#    (2026-09-30 .. 2026-10-05), el resto: 2 del untrack SECRETS-UNTRACK-01 y la historia previa.
```

El arnés de agentes **re-commitea el blob** en cada checkpoint de sesión. Consecuencia
operativa: el supuesto "la historia es un bloque inmutable y ya está medido" es **falso** en
este checkout — la superficie crece mientras se trabaja. Cualquier remediación que dependa
de "enumerar los commits que lo contienen" caduca.

### 3.1 Colisión delete/modify en la base nueva
`.claude/settings.json` es **delete/modify**: `origin/main` lo borró del árbol y en disco
está modificado (`ccf75ea5…` ≠ `15a78fe9…`). Riesgo declarado: **un `git add -A`/`commit -a`
sobre la base nueva reintroduce el fichero con el token vivo**. Antes de cualquier
publicación, `git status --porcelain` debe mostrar ese path **fuera** del índice y
**gitignorado** (§6, paso 6).

---

## 4. Verificación previa del operador (identidad sin revelar el valor)

En cada host que consuma la credencial (VPS `arbx`, cada máquina de desarrollo):

```bash
printf '%s' "$ANTHROPIC_AUTH_TOKEN" | sha256sum | cut -c1-12
```

- Sale `6c45ce87b547` → **es la credencial expuesta**: rotación inmediata (§6).
- Sale otra cosa → la credencial del runtime ya difiere del literal commiteado; **aun así
  se rota** si esa cadena fue alguna vez una credencial, y se cierra el untrack.

Límite honesto declarado: desde el repo **no** puedo determinar si el literal commiteado
sigue siendo **aceptado hoy** por el emisor. No tengo runtime y no debo usar la credencial
para averiguarlo. Eso cambia la **severidad** (¿remediación o higiene?), no la **acción**.

---

## 5. Cobertura de `.gitignore` y gates — citas y huecos

### 5.1 `.gitignore` (HEAD) — **HUECO declarado**
Cubre: `.env` (`:5`), `.env.*` (`:6`), `*Secrets_Config.xlsx` (`:13-14`), `*.key` (`:16`),
`.claude/settings.local.json` (`:117`), `.claude/settings.local.json.bak` (`:118`),
`credentials-manifest*.json` (`:123`), `!.scripts/credentials-manifest.example.json` (`:125`).

**No existe ninguna regla para `.claude/settings.json`** — el hermano `settings.local.json`
está protegido y el que lleva el token, no. Verificado: `git check-ignore -v
.claude/settings.json` → **exit 1, sin salida** (no ignorado). Además el fichero está
**trackeado**, así que `.gitignore` no aplica hasta que se destrackee.

### 5.2 `automation/tools/lint-no-hardcode.sh` — **HUECO declarado**
- `:125` define `SECRET_VARS` (incluye `ARBX_ADMIN_TOKEN`, `ARBX_EDGE_TOKEN`, `JWT_SECRET`, …).
- `:126` patron único: `\$\{(SECRET_VARS):-[^}]+\}` — sólo el idioma de **default de compose/shell**.
- `:135` globs: `*.yml *.yaml *.sh`. `:129` allow-list.

**No cubre** `.json`, no cubre `.ts`/`.md`, y **no cubre la forma `"CLAVE": "valor"`** — que es
exactamente la de P1. Un literal JSON nunca dispara este gate.

### 5.3 `.github/workflows/omega8-m3-grep-gates.yml` — **HUECO declarado**
- `:122` `grep -E "(^|/)\.env(\..*)?$"` sobre los ficheros nuevos del PR.
- `:129` `::error::This PR adds a tracked .env file — STRICTLY forbidden`.

Cubre **la ruta `.env*`**, no `.claude/settings.json` (no es un `.env`). Sí cubre, en cambio,
el route de `.env.crucible` (§7.3) para PRs futuros.

### 5.4 `.github/workflows/security.yml` (gitleaks) — **HUECO declarado**
- `:248` `fetch-depth: 0` (historia completa disponible), `:250` `gitleaks/gitleaks-action@v3`.
- `:257` `gitleaks diff scan (PR/push delta)`.
- `:274` `gitleaks detect --source . --log-opts="${BASE}..${HEAD}" --config .gitleaks.toml --redact --verbose`.
- `:276` si falta el binario: `echo "... skipping diff scan"` → **fail-open**.

Dos huecos: (a) el escaneo es **sólo del delta** `BASE..HEAD`, así que **no re-detecta** un
secreto que entró antes de la ventana — el caso de P1, trackeado desde `2026-07-11`; (b) si
el binario no está, el job **no falla**. `.gitleaks.toml:11` (`useDefault = true`), `:15`
(allowlist limitada a addresses `0x`+40 hex) y `:25` (3 paths documentados) **no** allow-listean
`.claude/settings.json`; `.gitleaksignore:8,11` sólo tiene 2 huellas ajenas a este fichero.
Es decir: la configuración no lo exceptúa — el **alcance temporal del escaneo** es lo que no
lo vio.

**Resumen de gates:** la ruta no está cubierta ni por `.gitignore` ni por ninguno de los tres
patrones prohibidos. Son huecos citados, no supuestos.

---

## 6. ACTION SHEET — pasos exactos (OPERADOR)

> Orden obligatorio: **rotar primero**, higiene después. Los pasos 1-5 son remediación; 6-8 son
> contención y cierre. Nada de esto lo ejecuta Security.

1. **Confirmar identidad (sin imprimir valores).** En cada host con la credencial, §4.
   Anota sólo el resultado booleano (`¿= 6c45ce87b547?`).
2. **Revocar en el emisor.** Si el token es del proveedor de modelos:
   consola de API keys → revocar la clave. Si es de un gateway propio/relay (el
   `ANTHROPIC_BASE_URL` y `CCR_CLAUDE_CODE_MODEL` apuntan a un router interno), revocar en ese
   emisor **y** rotar también la credencial aguas arriba.
3. **Emitir credencial nueva** y guardarla **fuera del checkout**: `~/.claude/settings.local.json`
   (ya gitignorado, `.gitignore:117`) o variable inyectada por el gestor de secretos. Nunca en
   un fichero trackeado.
4. **Propagar** a todos los consumidores (hosts de desarrollo, VPS, secretos de CI con
   `gh secret set`) y reiniciar los consumidores para que tomen el valor nuevo.
5. **Verificar el corte.** La credencial vieja debe fallar (401 / no resuelve). Comando de
   verificación sin imprimir el valor. Criterio de éxito: **la credencial expuesta deja de
   autenticar**.
6. **Destrackear + ignorar** (higiene, *después* de rotar, propiedad del track de release):
   `git rm --cached .claude/settings.json` en cada rama que lo trackee; añadir
   `.claude/settings.json` a `.gitignore` junto a `:117`; commit con el ID del incidente.
7. **Detener la re-materialización** (§3): 12 checkpoints re-commitean el blob. La acción es
   del arnés/operador; **fuera del alcance de E-7** y prohibida para Security.
8. **Cerrar con gate nuevo** (doctrina §37 — sin gate nuevo es una regresión esperando fecha):
   (a) un pase de gitleaks **sobre historia completa** una vez; (b) que el job del
   `security.yml:276` **falle** en vez de saltar si falta el binario; (c) regla de `.gitignore`
   para `.claude/settings.json`. Todo esto lo edita el operador en `.github`/`.gitignore` —
   **Security no toca `.github` ni configuración de secretos**.

---

## 7. RADIO DE EXPLOSIÓN

1. **Qué autentica.** La superficie de modelos del operador: cuota, gasto y tránsito de datos
   por ese gateway. **No** hay firmantes, seeds ni wallets en este fichero. No verifiqué qué
   más autoriza ese token en el emisor: **declarado como no medido**, no como inexistente.
2. **Ventana.** Introducido el `2026-07-11` (commit `2f4c82c9`); presente hoy. El untrack en
   `main` es del `2026-09-30` y **no** rotó nada (el propio mensaje dice "token vivo").
3. **Dónde está.** Cualquier clone/fork/espejo de `origin` (repo bare del VPS + remoto de
   GitHub), runners de CI con `fetch-depth: 0`, 20 commits (12 checkpoints) y los 14 refs
   locales `refs/dsh/checkpoints/*`. Cada uno de esos lugares puede servir el blob.
4. **Irrecuperable.** Forks, clones, caches y objetos inalcanzables en máquinas de terceros,
   y las copias internas del proveedor de git: no se pueden enumerar ni purgar desde aquí.
   Por eso **la única acción que cierra es la rotación**, no la limpieza de objetos.

---

## 8. Qué NO arregla esta exposición

- **Destrackear** (`git rm --cached`): la historia conserva los bytes; el blob sigue
  alcanzable desde `origin/main` (`91957994` es ancestro medido).
- **`.gitignore`**: sólo afecta a ficheros no trackeados. Un secreto que ya entró, no se
  "ignora" hacia atrás.
- **Que `main` haya borrado la ruta del árbol**: es el untrack, punto anterior. Control ≠ cierre.
- **Force-push / reescritura de historia / `filter-repo` / BFG**: **prohibido por doctrina**,
  destructivo e **ineficaz** (los clones/forks/caches no se reescriben). No se propone.
- **Borrar la rama o el checkout**: el objeto ya está replicado fuera.
- **Editar el valor dentro del fichero y commitear**: la historia se queda con el valor
  viejo, el fichero sigue trackeado y el valor nuevo pasa a estar expuesto también.
- **Cerrar el incidente con un PR "de limpieza"**: deja la credencial viva intacta.

## 9. Qué NO hago yo (Security / E-7) — frontera

- **No imprimo el valor** en ningún sitio: ni artefacto, ni log, ni mensaje, ni chat. Todo el
  documento identifica la credencial por huella.
- **No roto** la credencial, ni la uso, ni la valido contra el emisor.
- **No toco `.github/**` ni la configuración de secretos.**
- **No fuerzo push**, no reescribo historia, no ejecuto `filter-repo`/BFG.
- **No edito `.claude/settings.json`** ni lo destrackeo: es un cambio de código del track de
  release, y además no cierra la exposición.
- **No afirmo que `main` resolvió el incidente**: removió del árbol, no rotó.

## 10. Declaración exigida

> **NINGUNA tarea de código cierra esta clase de exposición:** untrack, `.gitignore`, borrado
> de rama, reescritura de historia o un PR de limpieza son higiene posterior; **solo la
> rotación en el emisor** convierte la credencial expuesta en una credencial muerta.

---

## 11. Hallazgos secundarios de la misma clase (rastreados, no todos incidentes)

| Ruta | Blob sha | Huella | Veredicto |
|------|----------|--------|-----------|
| `tests/e2e/cartridge-integration.spec.ts:23` | `cc78044b2f16f8de4df740e4ce90b55c25d98080` | literal de 16 chars, `sha256_12=17d6bfe05d1b`, los 3 segmentos están en vocabulario de placeholder | **placeholder con forma de token** (default de test). No es credencial viva; higiene. |
| `docs/auditoria/OMEGA_MULTICHAIN_GUIDE.md:292` | `ec987f85d9f6541c544dee5b3adb45a6e75db1e1` | literal de 19 chars asignado a `ARBX_ADMIN_TOKEN`, `sha256_12=3ea820a67f1c`, 4/4 segmentos en vocabulario de placeholder | **probable ejemplo documentado**; no verificable desde el repo. Verificación: comparar huella contra `ARBX_ADMIN_TOKEN` del runtime (§4 con la variable de admin). |
| `.env.crucible` | `7a883a8ca850bbd7109c952f6d8b46e58e311f9d` | todos los valores con forma de credencial son placeholder (contienen `CHANGE`; entropía 0.23-5.80; el valor con forma de privkey tiene 3 caracteres únicos) | **sin secreto vivo**; pero es un `.env` **trackeado** pese a `.gitignore:6` (`.env.*`): hueco estructural para el paso 8(c). |
| `docs/operations/SECRETS_POLICY.md:123-125` | `9e18766b77b74a7be5e66f5bbe50db57b96ce61b` | `change_me` presente; entropías 3.57/3.36/0.23 bits | **placeholder documentado** (así lo declara el propio fichero en `:119-121`). |
| 7 workflows en `.github/workflows/*` | — | fragmentos de sustitución de shell (`$(grep -E ... | cut -d= -f2-)`), no literales | **falsos positivos de mi barrido**, declarados para no inflar el informe. |

---

## 12. Evidencia cruda reproducible (resumen; comandos ejecutados en este checkout)

```
git rev-parse HEAD                                  -> 858b943fd80c8b5e1606d220aa87d63b86f4ba15
git rev-parse HEAD:.claude/settings.json            -> 15a78fe98bdf29220f8f91901e84f0d781c8a1f0
git hash-object .claude/settings.json               -> ccf75ea50b9fa480ac0d34b947fc87a7d921dded
git ls-files -s .claude/settings.json               -> 100644 15a78fe9... 0  .claude/settings.json
git rev-parse origin/main:.claude/settings.json     -> fatal: path exists on disk, but not in 'origin/main'
git merge-base --is-ancestor 91957994c06c origin/main -> exit 0 (True)
git branch -r --contains HEAD                       -> origin/CATALOG-HYGIENE-01
git ls-remote --heads origin (extracto)             -> 7693b6067f95d0455a338c45792365bb839339cc refs/heads/CATALOG-HYGIENE-01
git log --all --find-object=15a78fe9... | wc -l     -> 20 (12 con "dsh-checkpoint")
git check-ignore -v .claude/settings.json           -> exit 1 (no ignorado)
git log --diff-filter=A -1 -- .claude/settings.json -> 2f4c82c9 2026-07-11
```

Auditorías internas de esta conclusión: `metacog_audit` → **SOPORTADO 5/5** fragmentos
(0 no soportados, 0 contradichos, 9 items de evidencia con artefacto);
`adversarial_verdict` → **sobrevive TESIS** (peso 23 vs 1, 8 items con artefacto frente a 1).
Único flanco declarado: no se puede medir desde el repo si el literal sigue **aceptado** por
el emisor (§4).

---

## 13. Trazabilidad

- Tarea: `t3` / `E-7` — SEC-TOKEN-ROTATION-01 (equipo `arbx-publicacion-desbloqueo-02`).
- Insumo del capitán (medido por Release en t4): base de publicación cambiada a forward-port
  sobre `origin/main`; colisión delete/modify de `.claude/settings.json`. Ambos incorporados
  en §2.1 y §3.1 **sin** verificar yo el valor de la credencial.
- Alcance respetado: el único fichero escrito es este documento
  (`docs/security/TOKEN-EXPOSURE-ACTION-SHEET.md`).
