# TRACK-PROBE-01 — La sonda de identidad del runtime, trackeada como fuente única auditable

- **Tarea:** t41 · **Intento:** 1 · **attempt_id:** `00ad5c44-dcc9-42f1-a1c1-8d44f87c17e5`
- **Rol:** SRE · **Rama:** `sre/track-identity-probe-01` · **Base:** `origin/main` = `a38e677923ad76e0179547ed662bb904a374f7af`
- **In-scope tocado:** `tools/` (1 archivo nuevo) + `docs/sre/` (este documento). **Cero archivos fuera de scope.**
- **Clon aislado:** sparse checkout (`--filter=blob:none --no-checkout`), sin tocar el checkout compartido.

---

## §0 — RESULTADO

`tools/arbx_identity_probe.sh` **pasa a estar trackeado**, con el **mismo contenido** que el workflow de t37 embebe y declara. La sonda deja de ser un archivo untracked de un disco local: cualquiera que clone el repo la tiene, y su contenido queda auditable contra un commit por su hash.

| | |
|---|---|
| bytes | **3703** |
| sha256 | `0FA9E0411D847B25FDD5B311005294E76DEA1BF3326CA992D05D31676084876B` |
| blob git | `a25b3ce47ce1c5cff3a1e0352d309f31d647e0ca` |
| modo | `100755` (declarado en §5) |
| sintaxis | `bash -n` → exit **0** |
| verbos docker | `image, inspect` (solo) |

**El contenido NO se modificó.** Es una copia binaria del archivo medido; no se editó ni un byte.

---

## §1 — Estado ANTES y DESPUÉS, con los comandos que lo prueban

### Antes (medido en esta estación, `origin/main` = `a38e6779`)

```
$ git cat-file -e origin/main:tools/arbx_identity_probe.sh
fatal: path 'tools/arbx_identity_probe.sh' does not exist in 'origin/main'
exit=128

$ git ls-files --error-unmatch tools/arbx_identity_probe.sh     # en el checkout compartido
error: pathspec 'tools/arbx_identity_probe.sh' did not match any file(s) known to git
Did you forget to 'git add'?
```

### Después (en la rama de esta tarea)

```
$ git ls-files tools/arbx_identity_probe.sh
tools/arbx_identity_probe.sh                      # exit 0

$ git ls-files -s tools/arbx_identity_probe.sh
100755 a25b3ce47ce1c5cff3a1e0352d309f31d647e0ca 0	tools/arbx_identity_probe.sh

$ git diff --cached --stat origin/main
 tools/arbx_identity_probe.sh | 83 ++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 83 insertions(+)
```

### Y en `origin/main`: **todavía NO**, y se declara con la razón

```
$ git cat-file -e origin/main:tools/arbx_identity_probe.sh
fatal: path 'tools/arbx_identity_probe.sh' does not exist in 'origin/main'      # exit 128
```

El contrato de esta tarea instruye **NO mergear**, así que el árbol de `origin/main` sólo cambia cuando un humano mergee el PR. **Qué falta exactamente:** el merge del PR de esta rama. Después de eso, `git cat-file -e origin/main:tools/arbx_identity_probe.sh` pasa a exit 0 sin ninguna otra acción. Se adjunta la medición sobre el ref de la rama para que el hecho quede verificado *antes* del merge, no prometido:

```
$ git -C <clon> cat-file -e origin/sre/track-identity-probe-01:tools/arbx_identity_probe.sh
(exit 0)   # tras el push de la rama
```

---

## §2 — Reconciliación: el archivo trackeado == lo que el workflow declara y ejecuta

Tres piezas, tres hashes, **los tres iguales**:

| pieza | cómo se midió | bytes | sha256 |
|---|---|---|---|
| archivo local (el que se trackea) | `Get-FileHash` sobre los bytes | 3703 | `0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876b` |
| cuerpo EMBEBIDO en el workflow de t37 | extraído del YAML (`yaml.safe_load` + corte del heredoc) | 3703 | `0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876b` |
| constante que el workflow **declara** | `EXPECTED_PROBE_SHA256` en `.github/workflows/runtime-identity-probe.yml` (rama `sre/runtime-identity-probe-01`, commit `04ead11`) | — | `0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876b` |

Prueba de la igualdad byte a byte, no de parecido:

```
$ python <validador> <workflow-de-git>.yml tools/arbx_identity_probe.sh
body_bytes= 3703  orig_bytes= 3703
body_sha256= 0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876b
orig_sha256= 0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876b
BODY_IDENTICAL= True

$ (constante declarada en el workflow)
EXPECTED_PROBE_SHA256: 0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876b # lowercase (sha256sum output)
```

**No hay discrepancia que declarar.** El `probe_sha256=0fa9e041…` / `probe_bytes=3703` que el run `37397569735` imprimió en el runner corresponde exactamente a los bytes que ahora viven en el repo.

Correspondencia adicional con la evidencia de t7, que citó la sonda sin poder trackearla: el archivo es **LF puro** (`CR presente=False`), empieza con `#!/usr/bin/env bash` y tiene **83 líneas** — idéntico al que se leyó en t7 y al que se ejecutó en t37.

---

## §3 — ¿El workflow sigue necesitando el cuerpo embebido? **No por necesidad técnica; sí hoy, por dos razones declaradas**

**La respuesta técnica:** con el archivo trackeado, el cuerpo embebido deja de ser el único camino. El workflow puede hacer `actions/checkout` y usar `tools/arbx_identity_probe.sh`, conservando **la misma garantía**:

```yaml
# lo que ya existe y NO debe perderse al cambiar de fuente:
ACTUAL="$(sha256sum tools/arbx_identity_probe.sh | awk '{print $1}' | tr 'A-F' 'a-f')"
[ "$ACTUAL" = "$EXPECTED" ] || { echo "::error::provenance mismatch"; exit 1; }
ssh ... 'bash -s' < tools/arbx_identity_probe.sh
```

El `sha256sum` contra la constante **pinned** es lo que sostiene la propiedad: sin él, una rama podría ejecutar una sonda modificada sin que nadie lo note. Si se conserva ese guard, cambiar de fuente **reduce** la superficie (una sola copia) y no debilita nada.

**Por qué no lo hice en esta tarea:**

1. **`.github/workflows/` está fuera del in-scope de t41** (in-scope declarado: `tools/`, `docs/sre/`). Tocar el workflow sería salirme del contrato.
2. **El workflow ni siquiera está en `main` todavía**: vive en el PR #808 (rama `sre/runtime-identity-probe-01`), abierto y sin mergear. Editar la fuente de un workflow que aún no está en `main` generaría dos cambios entrelazados en dos PRs distintos.

**Estado hasta que el dueño de `.github/workflows/` lo cambie:** el cuerpo embebido sigue siendo **necesario** — es lo que el runner ejecuta hoy — y la reconciliación de §2 garantiza que esa copia y el archivo trackeado **no pueden divergir sin que se note**: si alguien edita uno de los dos, los hashes dejan de coincidir y esta tabla queda falsa de forma verificable.

**Hallazgo asociado, declarado y NO corregido aquí:** el contenido de la sonda (líneas 22-27) afirma que *"`verified` es INALCANZABLE hoy"* porque ningún Dockerfile hornea el label. **t37 midió lo contrario**: `built_from_sha = a38e6779…` en 10/10 imágenes y `api-server` = `verified`. Esa frase está desactualizada. **No la toqué** por dos razones explícitas: (a) el objetivo de t41 es trackear el archivo *como está*, no reescribirlo, y (b) cambiarla rompería la identidad byte a byte con el cuerpo embebido (§2), que está fuera de mi in-scope. Queda para el dueño de `tools/` + `.github/workflows/`, con la evidencia de t37 (runs `37397425099` y `37397569735`) como sustento.

---

## §4 — El comportamiento read-only NO cambió

No es una afirmación de intención: es el mismo archivo, verificado por tres vías independientes.

```
$ (inventario de verbos docker en el archivo trackeado)
image, inspect

$ (control de mutación)
OK: ningun verbo docker mutante

$ bash -n tools/arbx_identity_probe.sh
exit=0

$ (guard del workflow de t37, corrido en el runner del run 37397569735)
docker verbs found in the probe: image inspect
read-only guard OK: verb inventory is a subset of {inspect,image}
```

Sigue leyendo identidad y nada más: `docker inspect` de estado/StartedAt/Image/labels y `docker image inspect` del label horneado. **No escribe en el host, no ejecuta comandos docker mutantes, no toca `.env`, no firma y no hace broadcast.** El único acceso a variables de entorno es interno y filtra exclusivamente `ARBX_DEPLOY_SHA` y `ARBX_DEPLOYED_AT` (`grep '^ARBX_DEPLOY_SHA='`, L46-48) — es decir, identidad.

---

## §5 — El modo `100755` es una decisión, y se declara

El archivo medido vive en un sistema de archivos sin bits POSIX, así que **el modo no es medible** desde esta estación. Se commitea como `100755` porque:

- el archivo empieza con `#!/usr/bin/env bash` (declara intérprete: está pensado para poder invocarse),
- el workflow de t37 le aplica `chmod +x` al materializarlo,
- permite `./tools/arbx_identity_probe.sh` a quien lo audite en Linux.

**El contenido no cambia con el modo** (3703 B, mismo sha256). Si el dueño prefiere `100644`, es un cambio de modo sin efecto sobre el hash ni sobre el uso documentado (`ssh … 'bash -s' < tools/arbx_identity_probe.sh`, que no necesita el bit de ejecución).

---

## §6 — Alcance: esto NO mueve P/N y no acredita nada más

**P/N permanece en 0/115.** Esta tarea **no mueve P/N**, no cierra ningún criterio de negocio y **no convierte en verificado ningún otro criterio**. Lo que hace es una sola cosa: que un artefacto del que ya dependían informes (t7, t37) sea **auditable contra un commit** en vez de vivir sólo en un disco local. No re-mide identidad, no re-ejecuta la sonda, no valida el runtime: eso ya lo hizo t37 y su resultado no cambia.

---

## §7 — Reproducción

```
# verify del contrato — rama de esta tarea
git ls-files tools/arbx_identity_probe.sh                       # -> tools/arbx_identity_probe.sh  (exit 0)
git cat-file -e origin/sre/track-identity-probe-01:tools/arbx_identity_probe.sh   # -> exit 0

# verify del contrato — origin/main (requiere el merge; hoy exit 128, declarado en §1)
git cat-file -e origin/main:tools/arbx_identity_probe.sh        # -> fatal: ... does not exist in 'origin/main'

# integridad del contenido
git cat-file -p :tools/arbx_identity_probe.sh | sha256sum       # -> 0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876b
git cat-file -p :tools/arbx_identity_probe.sh | wc -c           # -> 3703

# reconciliación con lo que el workflow ejecuta
python <validador> <workflow>.yml tools/arbx_identity_probe.sh  # -> BODY_IDENTICAL=True

# read-only
bash -n tools/arbx_identity_probe.sh                            # -> exit 0
```

**Comandos crudos y salidas:** todas las cifras de este documento provienen de comandos corridos en esta estación sobre el clon aislado y de `git show` sobre `origin/sre/runtime-identity-probe-01`; ninguna se copió de otro informe.
