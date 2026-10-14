# RELEASE-MANIFEST-LABELS-01 — Delta de identidad de runtime: materializado, verificado y **ya en `main`**

**Tarea:** `t23 PUBLICAR-LABELS-01` · **Intento:** 2 (`1c0cbe0f-8075-42a5-8f51-d164f99fe865`) · **Perfil:** Data
**Base medida por mí:** `origin/main` = `c89d21a3` · HEAD del checkout compartido = `858b943fd80c8b5e1606d220aa87d63b86f4ba15` (sin cambio) · Sin firma, sin broadcast, sin deploy, **sin merge**.

> **CORRECCIÓN DE ESTADO.** La versión de este documento que vivía **sin trackear** en el checkout compartido (sha256 `978ecd8cd6e7f8c06d8a78224929a02170f4d80097c6a75f8cde676c722a719f`) declaraba **«BLOQUEADO — NO PUBLICADO»** con el candidato `166caeb6`, y era correcta **en su momento**: ese candidato revertía `main` en 8 de 15 archivos. **Ese defecto está CERRADO** (§2) por la re-emisión desde blob de `main` (`7bc24110`), que sí se publicó y **ya está mergeada**. Este documento es el manifiesto **vigente**; la copia local previa queda **superada**.

---

## 0. VEREDICTO EN UNA LÍNEA

El delta de identidad son **exactamente 15 archivos**, con **+66 / −0** contra `274f04fd` (inserción pura: dos líneas de identidad por Dockerfile y el wiring del build-arg en los dos compose); sus **15 blobs son byte-idénticos a los de `origin/main` actual** (cero revert, cero drift post-merge); está **publicado** en la rama `release/runtime-identity-labels` (remoto = `7bc24110`) vía **PR #800, MERGED**. **El mecanismo de labels está en `main`. Lo que este documento NO acredita es que funcione en un build real: `docker build` nunca se ejecutó (§4).**

---

## 1. MANIFIESTO — los 15 archivos con su hash

Verificación: para cada fila, el blob de `7bc24110` se extrajo a disco, se **re-hasheó** (debe devolver el mismo blob ⇒ los bytes son los del objeto) y se calculó su **sha256**; en paralelo se comparó el blob contra `origin/main:<path>`.

| path | blob `7bc24110` (SHA-1) | sha256 del contenido |
|---|---|---|
| `backend/api-server/Dockerfile` | `c39f45296e752763083bf29660a9c203c37b4ce2` | `113a2869939a1d1a0d3d8a2b34b3fa32213590484fa2cd36fb22258e19a08c26` |
| `backend/math-engine/Dockerfile` | `58b4841e5be2a4044c4a0672350b9ea9af1fbeb0` | `70596d60bf8027c9b39d1999c1e02173778fbd21c353d48f2e4334ad6ad4a3b4` |
| `backend/recon/Dockerfile` | `bec5d7d9f314f61ebc069078d142d550f86dcd4d` | `65d94219e8336482f3263ddc692fb0995b731fe1cb02df0bc72d1f2f78d4f997` |
| `backend/relays-client/Dockerfile` | `4ca1fa6682fe63a267003ab924db0d1df5baacbf` | `711d3bd9eedc165feb172b4c4e1c78f49828bba926dbee95b58c71d33bba83bb` |
| `backend/searcher-rs/Dockerfile` | `47078bbac1cc4067aa99dfa71ee944b859ab1165` | `2f2031ed8a9d9da5298dcd96c7641a67f4466a533bb4a8ba5836bcab3854b3e2` |
| `backend/searcher-rs/Dockerfile.edge` | `45bf544163a5a7c6716540e7522686efa64522c4` | `a81e505f0264170b71d42be18844cf3662564ee142961460c2806badaad17176` |
| `backend/selector-api/Dockerfile` | `af824280a99ea7598b73a63b91f61ebe4de8e8ae` | `57fc7975078fa64573350d970329eb90a269cdbd6138c88c7184d6be074e3ad6` |
| `backend/sim-ctl/Dockerfile` | `84396635e88058dcfaf1f8cef7535e31da5c779d` | `a08d59b4bafce491f05effcecd7e255e3921a2b78e7af6e683ba856ba51a57b5` |
| `backend/token-enricher/Dockerfile` | `667afba61820fa4ae4018c313b0ff7f0def940eb` | `c38ff4594d73e3e89b56c6ad40bf8bc8ffdd8057448e325faaeb9870bdf4d834` |
| `docker/compose.dev.yml` | `0ce01f582c3339aaf52be32ac054d338d81c544b` | `d8f44b31561f8aacb9bebf2157457770d71147be414a10bc78f92e0f9d48477f` |
| `docker/compose.prod.yml` | `0bcd2162086c2ce00980e7b25bdce4adfe9e64f0` | `c730433252e2a9ddf8b2255b93f69f0203083581f6415879d71e8da27a33fe53` |
| `docker/socket-proxy/Dockerfile` | `b24f53d3e6e01068f273471d43624c84fcfbb347` | `f4cd27a2486a752f045333bc3ff04b79ce8a79218a78524d45d68525636ccfe1` |
| `edge/dev-local/Dockerfile` | `e204b37a0e097a207b089a419e6b099d8b01c7ad` | `52062fbad9687ef5d699539fdac1ccb59ba3a212ed1663850251fc5c8fe4beb0` |
| `edge/worker/Dockerfile.node` | `3ca3f5edd7d21a280bc3438e46dd588e1abab263` | `feabf7cdd5c03b144d3e4557490d8d54e5b6a793aaac2bd0b6b04a9f5d6d212c` |
| `frontend/Dockerfile` | `c8940d3b2a3a647956ad1edcf0982c2c971e55aa` | `2de5b7373466e3bac7b024e610ff2e4b7145588d81d092a53b5fffedc4484565` |

**Totales medidos:** `total=15 · extracción+rehash OK=15/15 · blob == origin/main = 15/15`.
**Mecanismo (estático, sobre los blobs):** `13/15` portan `LABEL org.opencontainers.image.revision` (los 13 Dockerfiles) y **`15/15`** portan `ARBX_BUILD_SHA` (los 13 Dockerfiles + los 2 compose como wiring del build-arg). Los compose no llevan el LABEL — correcto: **el LABEL vive en el Dockerfile y el compose sólo pasa el `--build-arg`**. Conteo del wiring: `docker/compose.prod.yml` = **11** ocurrencias, `docker/compose.dev.yml` = **10**.

Las 15 rutas son **exactamente** el conjunto del contrato: 10 Dockerfiles de servicio (`api-server`, `math-engine`, `recon`, `relays-client`, `searcher-rs`, `searcher-rs/Dockerfile.edge`, `selector-api`, `sim-ctl`, `token-enricher`, `frontend`) + `edge/worker/Dockerfile.node` + `docker/socket-proxy/Dockerfile` + `edge/dev-local/Dockerfile` + `docker/compose.prod.yml` + `docker/compose.dev.yml`. **FALTANTES=0, EXTRA=0.**

---

## 2. EL DEFECTO DEL INTENTO 1, Y SU CIERRE (el hallazgo que importa)

**Intento 1 tomó los bytes del disco compartido** — como el contrato ordenaba («del working tree actual, no de una reconstrucción») — y produjo el candidato `166caeb6`. Medido entonces: **8 de 15 archivos revertían trabajo auditado de `main`**, incluidos una marca de seguridad y un binding de red:

| Archivo | Qué perdía |
|---|---|
| `backend/{math-engine,recon,relays-client,searcher-rs,sim-ctl,token-enricher}/Dockerfile` | `--mount=type=cache` 1→0 y el staging `build/out` |
| `frontend/Dockerfile` | `--mount=type=cache` 1→0 |
| `docker/compose.prod.yml` | **`SEC-EDGE-BYPASS-01` 1→0** y **`127.0.0.1:8788` 1→0** |

Causa raíz, medida y **generalizable**: el checkout compartido está en `858b943f`, **174 commits por detrás** de `main`. **Tomar bytes del disco sólo es seguro si el disco es POSTERIOR a la base; ahí el disco es ANTERIOR.** Obedecer la instrucción al pie de la letra fabricaba el revert. El intento 1 **se negó a publicar** y lo declaró: correcto.

**Cierre — verificado por mí sobre la re-emisión `7bc24110` (bytes desde `274f04fd:<path>` + reinserción de las 2 líneas de identidad):**

| Símbolo | `origin/main` | `7bc24110` |
|---|---|---|
| `--mount=type=cache` en los 6 backend Dockerfiles | 2 | **2** |
| `--mount=type=cache` en `frontend/Dockerfile` | 1 | **1** |
| `SEC-EDGE-BYPASS-01` en `compose.prod.yml` | 1 | **1** |
| `127.0.0.1:8788` en `compose.prod.yml` | 2 | **2** |
| `/build/out` en `backend/recon/Dockerfile` | 2 | **2** |

Y el `git diff --numstat 274f04fd 7bc24110` es **inserción pura en los 15**: trece archivos `+2/−0` (las dos líneas de identidad), `compose.dev.yml` `+19/−0`, `compose.prod.yml` `+21/−0`. **Total `+66 / −0`.** Cero líneas borradas ⇒ **cero revert**.

---

## 3. MANIFESTACIÓN VERIFICADA POR EL REMOTO (R13)

```
$ git ls-remote origin refs/heads/release/runtime-identity-labels
7bc24110de7cc0a0c588fedb97d5f8a76e8a7dda    refs/heads/release/runtime-identity-labels
exit=0
```

El remoto **lista** la ref (no sólo respondió OK el push): manifestado.

```
$ git cat-file -p 7bc24110 | head -2
tree 136a49e8bd78888ae85f9ab9e69292b24b2741d4
parent 274f04fd8d89692fd4b0169130ede74264e31167      ← base exigida = origin/main de la orden
```

```
$ gh pr list --head release/runtime-identity-labels --state all --json number,url,state,mergedAt
[{"number":800,"url":"https://github.com/hefarica/arbitragex-v2/pull/800","state":"MERGED","mergedAt":"2026-10-05T18:34:02Z"}]
```

**PR #800 — MERGED el 2026-10-05T18:34:02Z.** `git merge-base --is-ancestor 7bc24110 origin/main` → **exit 0**. Auditoría del set contra el conjunto exacto de 15: **FALTANTES=0, EXTRA=0** (§1).

> **DECLARACIÓN SOBRE EL MERGE.** El merge de #800 **no lo ejecuté yo**, y ninguna de mis acciones en esta tarea mergeó, desplegó ni hizo push a `main`. El contrato de este intento prohíbe mergear; el merge de #800 es un hecho previo y ajeno (t26 / la célula), y lo reporto como estado, no como acto propio.

---

## 4. LO QUE ESTE MANIFIESTO **NO** ACREDITA

1. **`docker build` nunca se ejecutó.** Lo verificado es **corrección estática de la configuración**: que los 15 blobs llevan el `ARG`/`LABEL` y el wiring del build-arg, que el conjunto es exacto y que el contenido coincide con `main`. **No** que la imagen construida hornee el SHA, ni que la sonda devuelva `verified`.
2. **El runtime hoy no corre este contenido.** El despliegue medido (2026-10-06T02:33Z) reporta `built_from_sha=3f00b359…`; el label sólo puede aparecer en un **rebuild posterior**. Este manifiesto habilita ese rebuild, no lo promete.
3. **No cierra D-12 por sí solo:** D-12 necesita un build real con el label horneado y una sonda que lo lea.

---

## 5. ESTADO DEL CHECKOUT COMPARTIDO (medido, y por qué agrava el §2)

| Magnitud | Medido | Nota |
|---|---|---|
| `HEAD` | `858b943fd80c8b5e1606d220aa87d63b86f4ba15` | **sin cambio** respecto del baseline |
| Archivos rastreados **borrados** del working tree (` D`) | **1208** | 918 `frontend/`, 240 `backend/`, 26 `shared-ts/`, 24 `edge/` |
| De los 15 del delta, ausentes en disco | **5** | `backend/api-server/Dockerfile`, `backend/selector-api/Dockerfile`, `edge/dev-local/Dockerfile`, `edge/worker/Dockerfile.node`, `frontend/Dockerfile` |
| Untracked | 313 | incluye esta carpeta `docs/identity/` antes de este PR |
| Entradas **staged** en el índice real | 1 (`.claude/settings.json`) | **ajena** (trabajo de otro nodo sobre E-7); no la toqué |
| `git worktree list` | **44** entradas | la orden espera **48** → invariante **desactualizada**; **no creé ni podé ninguna** |
| `.git/index.lock` | **ausente** | |
| `git write-tree` (índice real) | `7e0d6d18e194b08548e09ec8f2553eb97ec777c1` | **≠** `HEAD^{tree}` `622d32c0…` — porque hay 1 entrada staged ajena; **no escribí el índice** |

**Consecuencia dura para la letra del contrato:** el criterio «cada uno verificado **por hash contra el disco**» es **irrealizable para 5 de los 15** — esos Dockerfiles **no existen** en el working tree compartido. La verificación que sí cierra el criterio es la que hice: **hash contra los objetos git** (§1) + igualdad con `main`. Declarar lo contrario sería afirmar un chequeo que no se puede correr.

**Y la lección del intento 1, generalizada** (§2): en este checkout, *materializar desde el disco* no es una operación neutra — el disco está 174 commits atrás de `main` y además le faltan 1208 archivos rastreados. **El origen seguro de bytes para cualquier delta es `<base>:<path>`, no el disco.**

---

## 6. PROHIBICIONES — CUMPLIMIENTO DECLARADO

Sin merge · sin deploy · sin force-push · sin push a `main` · sin `workflow_dispatch` · sin `worktree add/prune/remove` · sin tocar HEAD, el índice real ni el working tree del checkout compartido. Toda la verificación fue de lectura (`rev-parse`, `diff`, `cat-file`, `grep`, `hash-object`, `write-tree`); el documento que estás leyendo se compuso en un **clon aislado** y se publicó desde allí.

---

## 7. REPRODUCCIÓN

```bash
R="C:/Users/HFRC/Desktop/arbitragex-v2-main (17)"

# 1. Manifestación en el remoto (el criterio decisivo)
git -c safe.directory="$R" -C "$R" ls-remote origin refs/heads/release/runtime-identity-labels
#   -> 7bc24110de7cc0a0c588fedb97d5f8a76e8a7dda

# 2. El set exacto y la ausencia de revert
git -c safe.directory="$R" -C "$R" diff --name-only 274f04fd 7bc24110        # 15 rutas
git -c safe.directory="$R" -C "$R" diff --numstat 274f04fd 7bc24110          # +66 / -0 en 15 filas
git -c safe.directory="$R" -C "$R" grep -c -F -- 'SEC-EDGE-BYPASS-01' 7bc24110 -- docker/compose.prod.yml   # 1

# 3. Igualdad de cada blob con main (cero drift post-merge)
for p in $(git -c safe.directory="$R" -C "$R" diff --name-only 274f04fd 7bc24110); do
  a=$(git -c safe.directory="$R" -C "$R" rev-parse "7bc24110:$p")
  b=$(git -c safe.directory="$R" -C "$R" rev-parse "origin/main:$p")
  [ "$a" = "$b" ] && echo "OK  $p" || echo "DRIFT $p"
done

# 4. Invariantes del checkout compartido
git -c safe.directory="$R" -C "$R" rev-parse HEAD          # 858b943f… (sin cambio)
git -c safe.directory="$R" -C "$R" worktree list | wc -l   # 44 (la orden dice 48: desactualizado)
```

---

*Verificación de lectura sobre el checkout compartido; documento compuesto y publicado desde un clon aislado. `docker build` NO ejecutado. `P/N` sigue en `0/115`: este manifiesto no mueve ningún criterio por sí solo — habilita el rebuild que D-12 necesita para medirse.*
