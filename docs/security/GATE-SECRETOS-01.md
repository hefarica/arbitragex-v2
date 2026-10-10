# GATE-SECRETOS-01 — endurecer los gates contra la clase E-7

> **DECLARACIÓN OBLIGATORIA (leer antes que nada):**
> **NADA de lo que hay en este documento rota la credencial expuesta.** Sólo la
> **rotación en el emisor** convierte una credencial expuesta en una credencial muerta.
> Destrackear, `.gitignore`, `filter-repo`, borrar ramas y — como se demostró en E-7 —
> que `main` haya borrado la ruta del árbol **no cierran la exposición**. La parte del
> emisor **NO es ejecutable por el equipo**: exige la consola del propietario. Este
> documento cierra el **defecto de clase** (que el mismo secreto pueda volver a pasar sin
> que ningún gate lo vea); **no** cierra el incidente de la credencial viva.

- **Tarea:** `t22` — GATE-SECRETOS-01 (parte ejecutable de D-3 / cierre D-10).
- **Antecedente:** E-7 / SEC-TOKEN-ROTATION-01 → `docs/security/TOKEN-EXPOSURE-ACTION-SHEET.md`.
- **Base medida:** `origin/main` `274f04fd` (rama aislada `security/GATE-SECRETOS-01`,
  creada desde `origin/main` fresco). No se publica desde el candidato `858b943f`.
- **Sin merge, sin deploy, sin force-push, sin push a `main`.**

---

## 1. Huecos demostrados de los 5 gates (con archivo y línea)

Base de las citas: `origin/main` = `274f04fd`.

| # | Gate | Hueco medido | Cita |
|---|------|--------------|------|
| G1 | `.gitignore` | El `.claude/settings.json` **sí** estaba listado (`:129`), pero **no es efectivo sobre un path trackeado**: `git check-ignore -q .claude/settings.json` → **exit 1** en este checkout. Además faltaba el hermano local: `.claude/settings.local.json.pre-*.bak`. | `.gitignore:123-131` (bloque SECRETS-TRACKED-01) |
| G2 | `automation/tools/lint-no-hardcode.sh` | Sólo detecta la forma `${VAR:-literal}` en `*.yml/*.yaml/*.sh`. **No lee JSON** y no cubre `"CLAVE": "valor"`. | `:125` `SECRET_VARS`, `:126` `SECRET_DEFAULT_RE`, `:135` globs, `:129` allow-list |
| G3 | `.github/workflows/omega8-m3-grep-gates.yml` | Sólo bloquea PRs que **AÑADEN** un `.env` trackeado. `.claude/settings.json` no es de esa familia. | `:122` grep `(^\|/)\.env(\..*)?$`, `:129` error, `:121` `--diff-filter=A` |
| G4 | `.github/workflows/security.yml` | (a) El scan es **sólo el delta** `BASE..HEAD` → un secreto que entró antes y sigue trackeado es invisible; (b) si falta el binario, **fail-open**: `echo ... skipping` y exit 0. | `:269` nombre del step, `:274` `if command -v gitleaks`, `:286` detect con `--log-opts`, **`:288` fail-open** |
| G5 | `.gitleaks.toml` | **No exceptuó** el fichero: el allowlist es de addresses `0x`+40 hex y 3 paths documentados. **No lo vio por ALCANCE TEMPORAL, no por allowlist.** | `.gitleaks.toml:11` `useDefault`, `:15` regexes, `:25` paths; `.gitleaksignore:8,11` (2 huellas ajenas) |

Evidencia cruda del punto G1 (comando + salida, sin valor):

```
$ git check-ignore -q .claude/settings.json ; echo exit=$?
exit=1
$ git check-ignore -q .claude/settings.local.json.pre-oauth-isolation-20260918_175656.bak ; echo exit=$?
exit=1
```

---

## 2. Qué se cambió (ejecutable)

### 2.1 `.gitignore`
- **Añadido** `.claude/settings.local.json.pre-*.bak` — hueco medido en este checkout: un
  backup real de settings operator-local estaba **sin ignorar** (`check-ignore` exit 1) y un
  `git add -A` lo habría publicado. Su hermano de harness (`.claude/settings.json.pre-*.bak`)
  ya estaba cubierto: la asimetría era el bug.
- La entrada `.claude/settings.json` **ya existía** en `main` (mantenida, no duplicada).
- Verificación: `git check-ignore -v .claude/settings.local.json.pre-oauth-isolation-20260918_175656.bak`
  → `.gitignore:131:...` exit **0**.

### 2.2 `automation/tools/lint-no-hardcode.sh` — dos categorías nuevas BLOQUEANTES
- **§5 `json-secret`** — la forma que E-7 atravesó: par JSON `"CLAVE": "valor"` donde la clave
  **termina** en `token|secret|password|passwd|credential|api_?key|private_key` y el valor
  tiene ≥16 caracteres. Regla de sufijo deliberada: rechaza descriptores (`auth_scheme`,
  `auth_type`, `api_key_id`, `max_tokens`) y acepta `ANTHROPIC_AUTH_TOKEN` / `apiKey` /
  `seller_secret`. Dos alcances: **5a** en `.claude/**` **sin allow-list** (el path exacto que
  falló), **5b** en `*.json/*.jsonc/*.json5` fuera de la allow-list de ejemplos documentados.
- **§6 `tracked-operator-local-secret-path`** — `git ls-files` marcando
  `^\.claude/settings(\..*)?\.json$` como **trackeado**. `git add -f` derrota a `.gitignore`;
  esta categoría derrota a `git add -f`. **Sólo ruta**: el contenido no se abre.
- `ARBX_GATE_ROOT` permite apuntar el gate a un repo-fixture desechable (lo usa el test).
- **Semántica de salida, explícita y medida:** las categorías 1-4 (legacy) siguen siendo
  **informativas y no fatales** — hoy hay **313** en `main`; convertirlas en fatales es una
  decisión aparte con su propio PR, no este. Las categorías **5-6 son fatales** (`exit 1`).

Verificación en `main` (274f04fd) tras el cambio: **BLOCKING=0, exit 0**, 313 legacy informativas.
Un primer intento marcó 1 falso positivo (`auth_scheme` en un ejemplo de `curl` de
`.claude/skills/.../doctrine.md:274`); se corrigió endureciendo la regla a **sufijo de clave**,
no relajando el alcance.

### 2.3 `automation/tools/test-gate-secretos.sh` — NUEVO (test de regresión)
Fixtures desechables en `mktemp -d` (**nada se commitea**; el valor del fixture es una cadena
sintética **derivada en runtime** (`sha256sum | cut -c1-32`), nunca un literal con forma de
secreto commiteado — el primer push llevó un literal y el propio `gitleaks` del repo lo rechazó:
ver §8.1):

| Assertion | Qué prueba |
|-----------|-----------|
| **A1** | `.claude/settings.json` **trackeado** con par JSON secreto → gate **falla** (y reporta §6 **y** §5a) |
| **A2** | `.claude/other-credentials.json` **trackeado** con la misma forma → gate **falla** — *discriminador de la regla de FORMA*: ese path no lo cubre §6, así que sólo §5 puede cazarlo |
| **A3** | Estado corregido (untracked + ignorado + placeholder) → gate **pasa** (exit 0) |
| **A4** | **Auto-chequeo de mutación**: con `JSON_SECRET_RE` neutralizado, A2 debe dejar de fallar. Si el mutante sobrevive, la suite sale ≠ 0 |

### 2.4 `.github/workflows/omega8-m3-grep-gates.yml`
Step nuevo **“E-7 — operator-local credential paths must NOT be tracked”**: `git ls-files`
filtrado por `^\.claude/settings(\..*)?\.json$` → `exit 1` si aparece. Cierra G3 para la familia
de path correcta. Sólo ruta: no lee, compara ni imprime valores.

### 2.5 `.github/workflows/security.yml`
- **G4(b) cerrado (con resolución de binario, evidencia de log):** el step resolvía el binario con
  `command -v gitleaks` y, si fallaba, hacía `echo ... skipping` + exit 0. El log del job
  `111912148621` demuestra que **la acción sí instala el binario** pero **no lo exporta al PATH**:
  `gitleaks version: 8.24.3` / `Version to install: 8.24.3 (target directory: /tmp/gitleaks-8.24.3)`.
  Ahora el step lo resuelve explícitamente (`command -v` → `/tmp/gitleaks-*/gitleaks`) y, si **no**
  hay ejecutable en ninguno de los dos, **falla con `::error::` + exit 1** (fail-closed). Es decir:
  el camino normal vuelve a **escanear de verdad** en vez de saltar en silencio, y el camino
  degradado deja de parecer limpio.
- **Step nuevo** que ejecuta `automation/tools/test-gate-secretos.sh`: es **independiente del
  alcance temporal** (path + forma, sin ventana de historia), así que no se puede evadir por tiempo.
- **G4(a) — decisión declarada, no omisión:** NO se activa un `gitleaks` de historia completa por
  defecto. El token de E-7 **sigue existiendo en la historia**, así que un scan de historia
  completa pondría **toda** corrida en rojo para siempre sin cerrar nada (la cura es la rotación;
  reescribir historia está prohibido por doctrina y es inefectivo frente a forks/clones). Queda
  como opción explícita del operador, documentada en el propio step.

### 2.6 `.gitleaks.toml` — **deliberadamente intacto**
No se añadió allowlist (añadirla **enmascararía** la clase: dirección contraria) y **no se
añadieron reglas nuevas** porque en este entorno **no hay binario `gitleaks`** para ejecutarlas:
una regla de scanner no verificada es una declaración, no un gate (fail-honest). El hueco de
alcance se cierra con las comprobaciones deterministas de 2.2/2.4/2.5, que sí se ejecutaron.

---

## 3. Sensibilidad a mutación: DEMOSTRADA

Dos corridas de la misma suite, cambiando **sólo** el gate bajo prueba:

```
$ bash automation/tools/test-gate-secretos.sh
PASS: A1 gate rejects a tracked .claude/settings.json with a JSON secret pair (exit 1)
PASS: A1 reports the tracked-path rule
PASS: A1 reports the JSON-form rule
PASS: A2 gate rejects a tracked .claude/other-credentials.json with a JSON secret pair (exit 1)
PASS: A2 JSON-form rule is the catcher
PASS: A3 corrected state (untracked + ignored + placeholder) passes (exit 0)
PASS: A4 MUTATION-DETECTED: neutering the JSON-form rule makes A2 pass again (rule is the cause, not the fixture)
PASS: A4 mutant emitted no json-secret-claude finding (mutation effective)
test-gate-secretos: all assertions passed      [EXIT=0 — 8 PASS / 0 FAIL]

$ sed 's|^JSON_SECRET_RE=.*|JSON_SECRET_RE="THIS_PATTERN_NEVER_MATCHES"|' \
      automation/tools/lint-no-hardcode.sh > /tmp/mutant.sh
$ GATE_BIN=/tmp/mutant.sh bash automation/tools/test-gate-secretos.sh
PASS: A1 gate rejects a tracked .claude/settings.json with a JSON secret pair (exit 1)
PASS: A1 reports the tracked-path rule
FAIL: A1 did not report BLOCKING[json-secret-claude]
FAIL: A2 gate ACCEPTED the JSON secret pair shape (exit 0) — the E-7 hole is open
FAIL: A2 BLOCKING[json-secret-claude] absent
PASS: A3 corrected state (untracked + ignored + placeholder) passes (exit 0)
PASS: A4 MUTATION-DETECTED ...
test-gate-secretos: 3 assertion(s) FAILED      [EXIT=1 — la suite MATA al mutante]
```

Lectura: el mutante (que elimina exactamente la regla nueva) **sobrevive al gate** y la suite lo
**detecta y falla**. La suite no es una declaración de cobertura: es un discriminador.

---

## 4. Verificación de aceptación (comandos y salida)

```
$ bash -n automation/tools/lint-no-hardcode.sh         # exit 0
$ bash -n automation/tools/test-gate-secretos.sh       # exit 0
$ bash automation/tools/lint-no-hardcode.sh            # exit 0, BLOCKING=0, 313 legacy informativas
$ bash automation/tools/test-gate-secretos.sh          # exit 0, 8 PASS / 0 FAIL
$ node -e "js-yaml.load(<3 workflows>)"                # OK: no-hardcode (1 job), omega8 (1 job), security (4 jobs)
```

Flip exigido por el contrato, medido **en el checkout de trabajo** (path declarado por ruta;
el valor nunca se imprime):

```
ANTES  : git check-ignore -q .claude/settings.json            -> exit 1
ACCIÓN : git rm --cached .claude/settings.json                -> rm '.claude/settings.json'
         (el fichero SIGUE en disco: Test-Path .claude/settings.json = True)
DESPUÉS: git check-ignore -v .claude/settings.json            -> .gitignore:126:.claude/settings.json   exit 0
         Select-String -LiteralPath .gitignore -Pattern '\.claude'
           -> line 126: .claude/settings.json
```

---

## 5. Qué NO hace este cambio (y qué sigue siendo del operador)

- **NO rota la credencial.** Es la única acción que cierra el incidente y vive en la consola del
  emisor: **no es ejecutable por el equipo**. Ver el action sheet de E-7.
- **NO borra la historia** ni la reescribe (`filter-repo`/BFG/force-push: prohibidos e inefectivos).
- **NO destrackea por sí solo** en otras ramas/copias: el `git rm --cached` se aplicó **en este
  checkout**; `main` ya tenía el path fuera del árbol desde `91957994` (2026-09-30) — y eso, como
  demuestra E-7, **no** cerró nada.
- **NO toca `.gitleaks.toml`**, ni configuración de secretos, ni `.github/workflows` ajenos al
  alcance (sólo los 3 citados).
- **NO convierte en fatales las 313 findings legacy** de las categorías 1-4: decisión separada,
  con su número medido aquí para que quien la tome no parta de cero.
- **NO hace merge, deploy, force-push ni push a `main`.**

## 6. Riesgo residual declarado

1. **`security.yml` ahora es fail-closed**: si un runner no tiene el binario `gitleaks`, el job
   **falla** (antes pasaba). Es intencional (un scan que no corre no puede verse como limpio),
   pero cambia el modo de fallo de "verde silencioso" a "rojo ruidoso". Si eso ocurre, la acción
   correcta es **proveer el binario**, no reintroducir el `skip`.
2. **La historia sigue conteniendo la credencial** (G4a). Mientras el operador no rote, el objeto
   es recuperable de cualquier clon. Este documento no cambia ese hecho.
3. Las categorías 5b (`*.json` fuera de allow-list) podrían marcar un JSON legítimo con una clave
   que termine en `key`/`token` y un valor ≥16 chars. Mitigación medida: en `main` el gate da
   **BLOCKING=0**; el primer falso positivo detectado (`auth_scheme`) se corrigió en la regla.
4. **`main` tiene el scan de secretos en ROJO por hallazgos HISTÓRICOS** en la familia
   `.claude/settings*.bak` (§8.2). **No lo causa este PR** y **no se cierra con gates**: exige la
   rotación del operador y una decisión explícita sobre la historia. Este PR **no lo enmascara**.

## 7. Trazabilidad

- Rama: `security/GATE-SECRETOS-01` (desde `origin/main` `274f04fd`), PR abierto **sin merge**;
  manifestación verificada con `git ls-remote origin refs/heads/security/GATE-SECRETOS-01`.
- Worktree aislado: `arbx-gate-secretos-01` (el checkout compartido **no** se cambió de rama; la
  única mutación en él es el `git rm --cached .claude/settings.json` declarado en §4).
- Archivos de este cambio: `.gitignore`, `automation/tools/lint-no-hardcode.sh`,
  `automation/tools/test-gate-secretos.sh` (nuevo), `.github/workflows/no-hardcode.yml`,
  `.github/workflows/omega8-m3-grep-gates.yml`, `.github/workflows/security.yml`,
  `docs/security/GATE-SECRETOS-01.md` (este documento).

## 8. Fix-forward medido en el primer push (y un hallazgo PREEXISTENTE)

### 8.1 El gate del repo rechazó mi propio test — correctamente
El primer push (commit `38b36387`) puso el job `gitleaks (secrets scan)` en **failure** a los 18 s.
Diagnóstico por log del job `111912148621`, con el valor **enmascarado**:

```
Finding:  FIXTURE_SECRET="<REDACTED>"
RuleID:   generic-api-key
Entropy:  4.954196
File:     automation/tools/test-gate-secretos.sh
Line:     41
```

Era **mi propia constante de fixture**: un literal de 32 caracteres de alta entropía. La respuesta
correcta es **fix-forward, no allowlist**: el valor ahora se **deriva en runtime**
(`sha256sum | cut -c1-32`) y no queda ningún literal con forma de secreto en el repositorio. **No**
se añadió nada a `.gitleaks.toml` ni a `.gitleaksignore`: un repo no debe necesitar una excepción
del scanner para testear su propio scanner.

### 8.2 El scan de historia completa YA está rojo en `main` — y NO es de este PR
El run **programado** de `security.yml` sobre `main` (`274f04fd`, `event=schedule`,
2026-10-05T13:38Z, run `37318433734`, job `111791011451`) está en **failure**, con hallazgos
(valores enmascarados):

```
Finding:  "ANTHROPIC_API_KEY": "<REDACTED>"
RuleID:   generic-api-key
Entropy:  4.846370
File:     .claude/settings.json.<REDACTED len=35>.bak
Line:     16   (y otro hallazgo en la línea 27)
```

Tres consecuencias que cambian el encuadre de esta tarea:

1. **La clase E-7 no es un solo fichero.** La familia `.claude/settings*.json` **y sus `.bak`**
   también lleva credenciales. Por eso `.gitignore` ahora cierra **la familia entera**
   (`.claude/settings*.json*`) en vez de una variante por vez.
2. **El scan de historia completa ya está activo** (el run programado corre sin `--log-opts`).
   Esto **confirma con evidencia** la decisión de §2.5/G4(a) de no activarlo también en el camino
   PR/push: habría puesto ese camino en rojo igualmente, sin cerrar nada.
3. **El rojo es PREEXISTENTE y no lo causa este PR.** Y no se cierra con gates: esos `.bak`
   contienen credenciales reales de historia, así que la acción correcta es la misma que en E-7 —
   **rotación por el operador** (más una decisión explícita sobre qué hacer con la historia, que
   este documento **no** toma). Un PR de gates no debe enmascarar este hallazgo: se reporta.

### 8.3 El hallazgo del fixture se re-reporta en el commit que lo ELIMINA
Tras el fix-forward, el job **volvió a fallar con el mismo hallazgo** (mismo archivo, línea 41,
entropy 4.954) — en el commit `3720eb3b`, que es el que **quita** el literal. Motivo: el rango que
usa la acción es `--log-opts=--no-merges --first-parent <SHA>^..<SHA>` (log del job, verbatim), es
decir **el parche del commit**: un parche de *borrado* también contiene el texto del secreto, y
gitleaks lo marca mientras ese commit esté en la ventana.

Dos consecuencias, declaradas sin adornos:

- **Es la misma limitación de ALCANCE que documenta G4(a)**, vista desde el otro lado: el scan por
  ventana no ve lo viejo, y además castiga el commit que limpia. Un gate por ventana no puede
  distinguir "secreto presente" de "secreto eliminado en este parche".
- **El remedio estándar (huella en `.gitleaksignore`) queda FUERA del alcance de esta tarea.**
  `.gitleaksignore` vive en la **raíz** del repo y el alcance asignado es
  `.gitignore`, `automation/tools/`, `.github/workflows/`, `docs/security/`. Añadirlo por mi cuenta
  habría sido salirme del scope, así que **no se hizo**: se reporta como follow-up con el formato
  exacto (`<commit>:automation/tools/test-gate-secretos.sh:generic-api-key:41`) y precedente en el
  propio repo (`.gitleaksignore:2-11` documenta el mismo caso: cita reescrita que el scan por rango
  sigue viendo). El valor nunca fue una credencial: era un fixture sintético.

### 8.4 Verificación LOCAL de la lógica del step de `security.yml`
El job de seguridad **aborta en el step 3** (la acción de gitleaks), así que los steps 4-5 —
incluida mi resolución de binario — **no se ejecutan en CI en este PR**. No se declara verificado
por fe: se ejecutó la lógica extraída del YAML con stubs, simulando la expansión `${{ ... }}`:

```
extracted 30 lines of step logic
PASS: A: binary on PATH is used (exit=0)
PASS: B: /tmp/gitleaks-*/gitleaks is resolved (exit=0)   <- el layout real del runner (log)
PASS: C: no binary anywhere -> fail-closed (exit=1)      <- antes: echo 'skipping' + exit 0
step-logic: all cases passed
```

El caso C es exactamente el fail-open que se cierra: sin binario, el step **falla** en vez de
aparentar un scan limpio.
