# CONTRACT-CI-HOLE-01 — el job de contratos nunca corrió: `forge build` y `forge test` en `skipped` y un `|| true` encima

**Alcance tocado:** `.github/workflows/foundry.yml` (+ este documento en `docs/ci/`). Nada más.
**Paper, cero mainnet, cero despliegues, cero reinicios, cero escrituras en producción.**
Entrega en **PR en DRAFT** (`#898`): el aterrizaje es otro ciclo.

---

## 0. Resumen (cada número con su artefacto)

1. **El defecto es real y está medido en el run `37792419955`**: el step de dependencias muere con
   `Error: failed to create dir ".../contracts/lib/openzeppelin-contracts": File exists (os error 17)` y
   deja `forge build --sizes` y `forge test -vv` en **`skipped`**.
2. **Por qué**: los tres paths de `contracts/lib` son **gitlinks (modo 160000)** declarados en `.gitmodules`,
   y el job hacía checkout **sin `submodules:`** ⇒ git los materializa como **directorios vacíos** ⇒
   `forge install` se niega a reutilizarlos.
3. **El enmascaramiento está en `foundry.yml:38`** (y `:103` en `test-fork`, según `origin/main`): el step se
   **llama** `forge install dependencies || true` mientras su `run:` **no toleraba nada**. El nombre prometía
   tolerancia; el step moría.
4. **Corrección de la premisa, medida**: la cobertura de contratos **NO era una ilusión**. `ci.yml:90-118`
   (`lint-and-test-contracts`, con `submodules: recursive`) corre **`forge test` BLOQUEANTE** y es requerido a
   través de **`ci-gate`** — que es un check **requerido en `main`**. Lo que faltaba era el job redundante de
   `foundry.yml` y, con él, `forge build --sizes` (que compila **todo, incluido `script/`**) y **`test-fork`**.
5. **Control bidireccional**: (a) contrato sano → job **success** con `forge build` y `forge test`
   **EJECUTADOS** (run `37794955928`); (b) contrato **roto a propósito** → job **FAILURE** (§4).

---

## 1. ★ LA CAUSA, REPRODUCIDA (comando, salida, exit code)

**Artefacto**: log del job `113363009504` del run `37792419955` (`gh api repos/hefarica/arbitragex-v2/actions/jobs/113363009504/logs`),
verbatim:

```
2026-10-08T14:32:06.7413029Z forge install OpenZeppelin/openzeppelin-contracts@cd05883078060e0cd8a7bd36636944570dbe1722 --no-git --no-commit
2026-10-08T14:32:06.7413880Z forge install OpenZeppelin/openzeppelin-contracts-upgradeable@fa525310e45f91eb20a6d3baa2644be8e0adba31 --no-git --no-commit
2026-10-08T14:32:06.7414703Z forge install foundry-rs/forge-std@b090968353a209833d4a6f1383230477e96d8438 --no-git --no-commit
2026-10-08T14:32:06.7781311Z Installing openzeppelin-contracts in /home/runner/work/arbitragex-v2/arbitragex-v2/contracts/lib/openzeppelin-contracts (url: https://github.com/OpenZeppelin/openzeppelin-contracts, tag: cd05883078060e0cd8a7bd36636944570dbe1722)
2026-10-08T14:32:06.7785006Z Error: failed to create dir "/home/runner/work/arbitragex-v2/arbitragex-v2/contracts/lib/openzeppelin-contracts": File exists (os error 17)
2026-10-08T14:32:06.7805644Z ##[error]Process completed with exit code 1.
```

**Comando exacto**: los tres `forge install … --no-git --no-commit` del step (arriba) · **salida**: las dos
líneas de `Installing`/`Error` (arriba) · **exit code**: **1**.

**La cadena completa, medida** (mismo run, `gh api .../runs/37792419955/jobs`):

| step | conclusion |
|---|---|
| 4 · `forge install dependencies \|\| true` | **failure** |
| 5 · `forge build (with sizes)` | **skipped** |
| 6 · `forge test (verbose)` | **skipped** |

### 1.1 El porqué, con archivo y modo de árbol

```
$ git ls-tree HEAD contracts/lib/
160000 commit b090968353a209833d4a6f1383230477e96d8438	contracts/lib/forge-std
160000 commit cd05883078060e0cd8a7bd36636944570dbe1722	contracts/lib/openzeppelin-contracts
160000 commit fa525310e45f91eb20a6d3baa2644be8e0adba31	contracts/lib/openzeppelin-contracts-upgradeable
```

Modo **160000 = gitlink**: son **submódulos** (`.gitmodules` declara los tres). `actions/checkout` en
`foundry.yml` **no** llevaba `submodules:`, así que git deja esos tres paths como **directorios vacíos**.
`forge install` se niega a instalar sobre un directorio existente ⇒ `File exists (os error 17)`.

**Y los tres SHAs son exactamente los que el install re-bajaba**: el install era una forma cara (red) de pedir
lo que los gitlinks **ya fijan**. Eso es lo que hace correcto el arreglo de §3.

### 1.2 ★ EL ENMASCARAMIENTO, con path y línea

```
.github/workflows/foundry.yml:38:      - name: forge install dependencies || true        # job `test`
.github/workflows/foundry.yml:103:     - name: forge install dependencies || true        # job `test-fork`
```

(líneas según `origin/main`; en la rama del arreglo ese step ya no existe). El **nombre** declara tolerancia
(`|| true`) sobre un `run:` que **no la tenía**: si el install fallaba, el step moría y el job quedaba en rojo
*por la razón equivocada*, dejando build/test en `skipped`. El propio comentario del archivo lo admitía
(`foundry.yml:50`: *"so `forge build` failed — masked until now by `|| true`"*), y el comentario de
`foundry.yml:63-65` documenta el incidente anterior del mismo mecanismo (un test stub + 33 tests rojos en
`main` bajo un check verde).

⇒ **Un skip no es un pase, y un nombre que promete tolerancia no es un diseño.** Es la misma clase que este
proyecto paga una y otra vez: una ausencia vestida de presencia.

**Reproducción local: NO disponible, y se declara.** `forge` local está **bloqueado por Device Guard**
(`'C:\Users\HFRC\.foundry\bin\forge.exe' ha sido bloqueado por la directiva de Device Guard de su
organización`) y el VPS **no tiene `forge`** (`command -v forge` → `MISSING`). Por eso la reproducción es el
**log del run** — un artefacto de ejecución real con su exit code, no una hipótesis.

---

## 2. ★ EL ALCANCE, CONTADO (ventana declarada)

Método: `gh run list --workflow foundry.yml --limit 30` + `gh api .../runs/<id>/jobs` por cada run
(30 llamadas), mirando los steps `forge build*` / `forge test*` del job `forge build + test`.

**Ventana: los 30 runs más recientes de `foundry.yml`, de `2026-08-30T01:35:10Z` a `2026-10-08T14:26:04Z`.**

| firma en el run | runs | lectura |
|---|---|---|
| `build/test = [skipped,skipped]` | **6** | **este defecto**: el step de dependencias murió *antes* de compilar |
| `build/test = [failure,skipped]` | 4 | firma **distinta**: `forge build` **corrió y falló** (cascada correcta) |
| `build/test` ejecutados (success/failure) | 20 | build y test se ejecutaron |

Los **6 con la firma del defecto**: `37744529752` (07:36Z), `37745302367` (07:44Z), `37747910062` (08:08Z),
`37748858956` (08:17Z), `37750589752` (08:33Z), `37792419955` (14:26Z) — **todos del 2026-10-08**.

**Desde cuándo: no se puede fechar, y se dice.** El último run con ambos ejecutados fue `35938837616`
(`2026-09-24T00:31:58Z`, success) y el siguiente run del workflow es ya del `2026-10-08T07:36:58Z`: **entre
esos dos no hubo ningún run** (los disparadores tienen `paths:` `contracts/**` y `foundry.yml`). El defecto
estuvo presente en **todos** los runs desde el primero posterior al hueco; su inicio exacto queda **dentro de
un intervalo sin observaciones** y no se estima.

**Lo que NO se deriva y por tanto NO se dice:** cuántos bugs pasaron por este agujero. **No es computable** —
no hay artefacto que ligue defectos de contrato a runs en los que el build no corrió. Decir un número sería
inventarlo.

➡️ **Consecuencia honesta del conteo**: *"los contratos están testeados en CI"* **no** era una creencia vacía
(§2.1), pero el job redundante de `foundry.yml` **no aportaba nada** y, mientras tanto, `forge build --sizes`
y los **fork tests** no corrían en absoluto. No se sigue que hubiera bugs: **no se sabe** por esa vía.

### 2.1 La corrección a la premisa (lo que la medición obligó a cambiar)

El enunciado de la tarea afirma que *"toda la confianza depositada en 'los contratos están testeados en CI' es
NO COMPUTADA"*. **La medición no lo sostiene**, y hay que decirlo:

- `ci.yml:90-118` define `lint-and-test-contracts`, que hace checkout con **`submodules: recursive`** y corre
  **`forge test (blocking, authoritative for ci-gate)`** y `forge fmt --check`, **ambos bloqueantes**.
- El comentario del propio job (`ci.yml:97-112`) lo declara uno de los seis requires de `ci-gate`, y **`ci-gate`
  es un check requerido en `main`**: medido con
  `gh api repos/hefarica/arbitragex-v2/branches/main/protection --jq '.required_status_checks.contexts'`
  → `["ci-gate","Verifier policy tests"]`.
- Medido corriendo, run `37791625629` (success): `lint-and-test-contracts` → step 4 `forge test` **success**,
  step 5 `forge fmt --check` **success**; job **success**; `ci-gate` → **success**.

⇒ **Los contratos sí se compilan y se testean en cada PR**, y ese gate es requerido. Lo que el agujero se
llevaba era **otra cosa**: `forge build --sizes` (que compila el proyecto **entero, incluido `script/`**, cosa
que `forge test` no necesariamente cubre) y el job `test-fork`, que por `needs: test` **nunca corrió**.

---

## 3. EL ARREGLO (no es otro `|| true`)

`.github/workflows/foundry.yml`, en **los dos** jobs (`test` y `test-fork`):

1. `actions/checkout` pasa a `with: submodules: recursive` — el **mismo patrón que ya usa `ci.yml:94-95`**, que
   está medido funcionando. Las libs vienen de los commits que los gitlinks **ya** fijan.
2. **Se elimina** el step `forge install dependencies || true` (líneas 38 y 103 de `origin/main`) — el punto
   donde el job moría y el nombre que enmascaraba.
3. Se sustituye por un step **explícito y nombrable**: `Verify contracts/lib is materialised (no forge install,
   no tolerance)`, que recorre las tres libs y, si alguna falta o está vacía, imprime
   `::error::contracts/lib/<lib> MISSING or empty — the checkout must materialise the pinned submodules` y
   **FALLA**.
4. **Cero `|| true` añadidos. Cero `continue-on-error` añadidos.** `forge build --sizes` y `forge test -vv`
   siguen **bloqueantes**, sin tocar su `run:`.

**La diferencia entre «no puede» y «no quiero», escrita en el YAML**: el job **no** depende de la red para las
dependencias (vienen de git, que es su fuente canónica y ya está pinneada); si un checkout no las materializa,
el job lo dice **con el nombre de la lib** y falla. No hay camino en el que falte una lib y el job siga verde.

**Alternativa rechazada y por qué**: mantener `forge install` haciéndolo idempotente (borrar los placeholders
vacíos antes de instalar) habría dejado el job dependiendo de la red para pedir **exactamente los commits que
el repo ya fija**, con más piezas y la misma superficie de fallo. Git es la fuente; la verificación la vigila.

---

## 4. ★★ CONTROL BIDIRECCIONAL

### (a) Contrato sano → job **success** con `forge build` y `forge test` EJECUTADOS

Run **`37794955928`** (`Foundry CI`, event `pull_request`, headSha `f2444d7c1e81e6566427d976146f7a73dd02a18a`),
job **`forge build + test`** = **success** (`14:53:26Z → 14:54:28Z`):

| step | conclusion |
|---|---|
| 3 · `Verify contracts/lib is materialised (no forge install, no tolerance)` | **success** |
| 4 · **`forge build (with sizes)`** | **success** (ejecutado, NO skipped) |
| 5 · **`forge test (verbose)`** | **success** (ejecutado, NO skipped) |

**Bonus medido**: en ese mismo run, el job **`forge fork test (mainnet)`** = **success**
(`14:54:29Z → 15:03:13Z`), con `forge build` **success** y **`forge fork test (testFork_*)` success**: los fork
tests **corrieron de verdad** por primera vez en la ventana medida — antes no podían, porque `needs: test`
apuntaba a un job rojo.

### (b) Contrato roto a propósito → job **FAILURE**

Rama **scratch** `sre/t171-control-broken` (temporal, **NO forma parte del entregable**; se borra al terminar
el control) = la rama del arreglo + **un step temporal** que inyecta en runtime un contrato con error de
sintaxis (`src/__t171_control_broken.sol`). **No se modificó ningún archivo de `contracts/` en el repo**: el
"contrato roto" se materializa dentro del runner.

Run **`37796921196`** (`Foundry CI`, event `pull_request`, headSha `a0ed73a8`), job **`forge build + test`** =
**failure**:

| step | conclusion |
|---|---|
| 4 · `Verify contracts/lib is materialised …` | success |
| 5 · `CONTROL t171 … inyectar un contrato roto a proposito` | success |
| 6 · **`forge build (with sizes)`** | **failure** (ejecutado, NO skipped) |
| 7 · `forge test (verbose)` | skipped (cascada correcta: el build falló antes) |

Evidencia del log del job `113378671936`, verbatim:

```
injected control file:
-rw-r--r-- 1 runner runner 71 Oct  8 15:11 src/__t171_control_broken.sol
Error: Compiler run failed:
Error (6933): Expected primary expression.
 --> src/__t171_control_broken.sol:1:66:
##[error]Process completed with exit code 1.
```

⇒ **El contrato roto NO pasa**: el job falla, y falla **porque `forge build` se EJECUTÓ y detectó el error**,
no porque un step anterior lo saltara. La diferencia con el estado anterior es exactamente esa: antes el job
moría en el install y el build **nunca miraba el código**. Si un contrato roto pasara, el arreglo habría
convertido el skip en un verde ciego; **no lo hizo**.

**Alcance del control, sin inflarlo**: en este run `forge test` queda `skipped` porque `forge build` falla antes
(cascada correcta, no enmascaramiento). Que el step de test **se ejecute** está medido en el control (a)
(`forge test (verbose)` = success) y su bloqueo es estructural: el archivo entregado no contiene `|| true` ni
`continue-on-error` en CÓDIGO: las 4 apariciones de esas cadenas en el archivo entregado están TODAS en comentarios que documentan su eliminación (`git grep` → L47, L50, L71, L135, las cuatro empezando por `#`).

---

## 5. ★ LA PRUEBA DE FUEGO Y EL CICLO

- Esta tarea **entrega en DRAFT** (`#898`): **el aterrizaje es otro ciclo**. No se re-dispara nada ni se fuerza
  ningún gate.
- **Regla de turnos respetada**: `#894` (carrera G4, `auto-deploy-vps.yml`) va **antes** que este si ambos están
  listos. Son **archivos distintos** (`auto-deploy-vps.yml` vs `foundry.yml`), pero el turno se respeta igual.
- **Criterio de aceptación del próximo ciclo natural**: el job `forge build + test` con `forge build (with
  sizes)` y `forge test (verbose)` **ejecutados** (no `skipped`), leído del run. Si volviera a `skipped`, el
  arreglo no funcionó y se dirá.

---

## 6. EL DIFF, COMPLETO Y ACOTADO

```
$ git diff --stat origin/main
 .github/workflows/foundry.yml | 64 ++++-----  (39 inserciones, 25 borrados)
 docs/ci/CONTRACT-CI-HOLE-01.md | 271 +
```

**No se tocó**: `contracts/` (ni sus libs), `docker/`, la carrera G4 (`#894`, `auto-deploy-vps.yml`), la lista
de servicios, la lógica de identidad, ningún otro workflow, ni ningún otro archivo fuera de
`.github/workflows/foundry.yml` y este documento. Si el arreglo hubiera exigido tocar algo más, esta tarea
paraba y lo declaraba: **no lo exigió**.

---

## 7. DEFECTOS DE INSTRUMENTO PROPIOS, DECLARADOS (cazados durante el trabajo)

1. **El commit del arreglo falló** por quoting: el mensaje con `\"` dentro de una cadena de PowerShell se cortó
   y git leyó las líneas siguientes como **rutas** (`did not match any file(s) known to git`). Se rehizo con
   `git commit -F <archivo>`. La rama remota quedó creada en `main` sin el commit durante un momento: se
   declaró y se corrigió en el mismo paso.
2. **El step de control rompió el YAML**: su `name:` contenía `": "` (dos puntos + espacio) dentro de un escalar
   plano ⇒ **workflow inválido** ⇒ GitHub reportó el push como run `failure` **sin jobs** (`37796469268`,
   `37796605885`). Corregido; desde entonces **se valida el YAML antes de cada push**.
3. **El here-string de PowerShell no incluye el salto de línea final**, así que la inyección **concatenó**
   `- name: forge build (with sizes)` al final del `run:` del step de control (línea 70 del archivo de la rama
   scratch), dejando al job sin step de build y el workflow rechazado. Corregido con edición literal y
   re-validado.
4. Herramienta: `forge` local **bloqueado por Device Guard** y ausente en el VPS ⇒ la reproducción es el log
   del run (§1), declarado en vez de simulado.

Ninguno de estos defectos quedó en el artefacto entregado: el `foundry.yml` de `#898` está validado
(`python -c "import yaml; yaml.safe_load(...)"` → OK) y su control (a) corre en verde.
