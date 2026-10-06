# GITIGNORE-DOCS-DATA-01 — `docs/data/` deja de ser invisible para `git add`

**Orden:** t52 · **Perfil:** Data · **Intento:** `345fd945-bae5-4b2c-ae56-cd0239a45466`
**Base medida por mí:** clon aislado `C:\Users\HFRC\Desktop\arbx-t49\repo`, `origin/main` = **`c89d21a3`** (`git rev-parse --short=8 origin/main`).
**Alcance:** `.gitignore` + `docs/data/`. No se editó backend, frontend, workflows, scripts ni `implementation-state/`. **NO se mergeó y NO se empujó a `main`.**

---

## 0. Lo que estaba roto

`.gitignore` traía la regla **desanclada** `data/` (bloque `# Local data`):

```
 72: # Local data
 73: data/
 74: logs/
 75: tmp/
```

Sin `/` inicial y sin ancla, `data/` matchea **cualquier** directorio llamado `data` a **cualquier profundidad** — incluido `docs/data/`, el destino de los entregables del perfil Data. Consecuencia medida: `docs/data/` tenía **cero** archivos rastreados en todo el historial y el primer entregable del perfil (`ECON-ACCOUNTING-01`, t49) **sólo pudo entrar con `git add -f`**.

Esa es la trampa silenciosa: `git status` **no muestra el archivo**, `git add` **falla con un hint**, y el entregable simplemente no existe para git. Un directorio de entregables donde el versionado falla en silencio.

---

## 1. EL FIX — una línea funcional, anclada

```diff
 # Local data
 data/
+# GITIGNORE-DOCS-DATA-01: `docs/data/` holds VERSIONED Data-profile deliverables
+# (markdown), not local data. Anchored to the repo-root path only — every other
+# `data/` directory at any depth stays ignored by the rule above.
+!/docs/data/
 logs/
```

**Por qué es mínimo:** el cambio funcional es **una sola línea** (`!/docs/data/`). Las tres de comentario documentan la intención, siguiendo la convención del propio archivo (`.gitignore` ya comenta sus reglas: `# Local data`, `# Playwright`, y las notas largas en `:110`, `:134`, `:152`).

**Por qué ANCLADO (`!/docs/data/` y no `!docs/data/`):** con ancla al raíz del repo, la negación re-incluye **exactamente** la ruta `docs/data/` del repo y nada más. Una negación desanclada habría re-incluido también cualquier `.../docs/data/` a cualquier profundidad. El archivo ya usa este idioma: `!/backend/Cargo.lock` (`.gitignore:35`) y `docs/**/*.zip` con su negación (`.gitignore:102`).

**Lo que NO se hizo:** ninguna negación amplia (`!docs/`, `!*.md`, `!**/data/`), ningún debilitamiento de `data/`, ninguna eliminación de la regla.

**Estabilidad de coordenadas:** la regla `data/` **sigue en la línea 73** (la negación se inserta después), así que toda cita previa a `.gitignore:73:data/` — incluidas las del capitán y las de t49 §8 — **sigue siendo exacta**. La inserción corre 4 líneas el resto del bloque: `*.sqlite` pasó de `:76` a `:80` y `*.db` de `:77` a `:81`; cualquier documento que cite esas líneas necesita el corrimiento declarado.

---

## 2. EL TEST DECISIVO — ANTES y DESPUÉS

Instrumento: `git check-ignore -v --no-index`. **Semántica que hace falta leer bien:** termina con **exit 0 y una línea `.gitignore:<n>:<regla>`** cuando la ruta **ESTÁ** ignorada; termina con **exit 1 y sin salida** cuando **NO** lo está. El exit code es la señal, no la ausencia de texto.

### ANTES (worktree en `origin/main` `c89d21a3`, sin el fix)

```
$ git check-ignore -v --no-index docs/data/README.md
.gitignore:73:data/	docs/data/README.md
exit=0                                     ← IGNORADO

$ git check-ignore -v --no-index docs/data/probe.md
.gitignore:73:data/	docs/data/probe.md
exit=0                                     ← IGNORADO  (comando literal del Verify)

$ git add docs/data/README.md              ← add NORMAL, sin -f
The following paths are ignored by one of your .gitignore files:
docs/data
hint: Use -f if you really want to add them.
exit=1                                     ← NO ENTRA

$ git status --porcelain -- docs/data
(vacio — la ruta no aparece)

$ git ls-tree -r --name-only origin/main -- docs/data
(0 archivos)
```

### DESPUÉS (mismo worktree, con el fix)

```
$ git check-ignore -v --no-index docs/data/README.md
exit=1                                     ← NO ignorado (sin salida = ninguna regla lo matchea)

$ git check-ignore -v --no-index docs/data/probe.md
exit=1                                     ← NO ignorado  (comando literal del Verify)

$ git add docs/data/README.md              ← add NORMAL, sin -f
exit=0                                     ← ENTRA

$ git status --porcelain -- docs/data
A  docs/data/README.md
```

**El test decisivo se dio vuelta: de `exit 0 + .gitignore:73` a `exit 1 + sin salida`.** Y el `git add` normal pasó de `exit 1` con hint a `exit 0` staged.

---

## 3. LA PRUEBA DEL `git add` SIN `-f`

Los **dos archivos nuevos** de esta orden entraron al índice con un `git add` **normal**, y su propio contenido es el entregable:

```
$ git add .gitignore docs/data/README.md docs/data/GITIGNORE-DOCS-DATA-01.md
git add exit=0                             ← sin -f, exit 0, sin hint

$ git status --porcelain
M  .gitignore
A  docs/data/GITIGNORE-DOCS-DATA-01.md
A  docs/data/README.md
```

**Ningún `-f` en esta orden.** Si el fix no hubiera cerrado, estos `git add` habrían fallado con el hint de §2 — y no fallaron.

---

## 4. QUÉ SIGUE IGNORADO DESPUÉS DEL CAMBIO (declaración explícita)

Barrido de sondas, todas con `git check-ignore -v --no-index` (exit 0 = ignorado). El punto es demostrar que la negación re-incluye **una ruta** y no debilita la intención de la regla.

| Sonda | Estado DESPUÉS | Regla que la ignora |
|---|---|---|
| `data/local.json` | **IGNORADO** | `.gitignore:73:data/` |
| `foo/data/x.json` (cualquier profundidad) | **IGNORADO** | `.gitignore:73:data/` |
| `docs/assets/data/x.json` (otro `data` bajo `docs/`) | **IGNORADO** | `.gitignore:73:data/` |
| `sub/docs/data/x.md` (prueba de anclaje) | **IGNORADO** | `.gitignore:73:data/` |
| `docs/data/dump.sqlite` | **IGNORADO** | `.gitignore:80:*.sqlite` |
| `docs/data/dump.db` | **IGNORADO** | `.gitignore:81:*.db` |
| `docs/data/run.log` | **IGNORADO** | `.gitignore:44:*.log` |
| `.env.local` (coherencia del archivo) | **IGNORADO** | `.gitignore:6:.env.*` |
| `docs/data/x.md` | no-ignorado (exit 1) | — (re-incluido a propósito) |

**La intención de `data/` sigue viva:** todo `data/` que no sea el `docs/data/` del raíz queda ignorado, y **dentro** de `docs/data/` siguen ignorándose los datos por tipo (`.sqlite`, `.db`, `.log`). El fix no abre la puerta a datos locales: la negación es de directorio para permitir markdown versionado, y las reglas de tipo siguen protegiendo el contenido.

---

## 5. DESTINO CANÓNICO DE LOS ENTREGABLES DEL PERFIL DATA

**Decisión: `docs/data/` ES el destino canónico. No se mueve nada en esta orden.**

Razones, medidas:

1. Es la ruta que el contrato de t49 fijó (`In scope: docs/data/`) y la que el capitán confirmó.
2. El problema **nunca fue la ubicación, era la regla** que la volvía invisible: la prueba es que `docs/backend/`, `docs/sre/`, `docs/quant/` y `docs/review/` **no** están ignorados, y sin embargo `docs/backend/` tiene **0** archivos rastreados por una razón distinta — **nada se mergeó todavía**.
3. Mover los entregables a otra ruta escondería el defecto en vez de arreglarlo, y dejaría un `data/` desanclado esperando a la próxima víctima.

### 5.1 Corrección de mi propia evidencia de apoyo (t49 §8)

En t49 §8 escribí que `docs/backend`, `docs/sre` y `docs/quant` "**sí** están rastreados". El portador correcto son **dos instrumentos distintos**, y los mezclé. Medido por mí sobre `origin/main` (`c89d21a3`):

| Directorio | `git ls-tree -r --name-only origin/main -- <dir>` | `check-ignore --no-index` (worktree **sin** el fix) |
|---|---|---|
| `docs/sre` | **3** | no-ignorado |
| `docs/release` | **1** | no-ignorado |
| `docs/backend` | **0** | **no-ignorado** |
| `docs/quant` | **0** | **no-ignorado** |
| `docs/review` | **0** | no-ignorado |
| `docs/data` | **0** | **IGNORADO** `.gitignore:73:data/` |

**La lección, que es la misma de esta orden:** *"0 archivos rastreados"* y *"está ignorado"* son afirmaciones **distintas** y necesitan **portadores distintos** — `ls-tree` cuenta lo merged; `check-ignore` decide lo ignorado. Un cero de `ls-tree` significa "nada mergeado", **no** "ignorado". Mi conclusión de t49 era correcta; la evidencia de apoyo ahora es la de arriba.
**Esta corrección vive acá** (y no editando `docs/data/ECON-ACCOUNTING-01.md` en su rama, que abriría un segundo PR sobre el mismo path). Debe viajar a `#817` o a la orden que lo cierre.

---

## 6. DISPOSICIÓN DEL HALLAZGO DE ESQUEMA `actual_*` (no se arregla acá)

**El hallazgo (de t49, verificado en `main` `c89d21a3`):** el repo tiene columnas `actual_*` en `paper_trade_runs` (`actual_gas_cost_usd`, `actual_profit_usd`) y un path de breaker que las lee, pero **su propio string de procedencia declara que NO son settlement on-chain**:

```
backend/api-server/src/routes/risk-circuit-breakers.ts:171
const ACTUAL_GAS_PROVENANCE = "sim-ctl replay via drift_tracker (not on-chain settled)";
```

y el productor que las llena **es un replay**, no una liquidación, y hoy **no corre**: `backend/recon/src/main.rs:344` (`if drift_mode == "on"`) + `.env.example:347` (`ARBX_DRIFT_TRACKER_MODE=off`) ⇒ el gas realizado es **NULL**, y **NULL ≠ 0** (R10).

**Riesgo nombrado:** un lector mapea el **nombre** `actual_*` → REALIZADO. Es el mismo error de la familia que esta célula persigue (una etiqueta leída como otra cosa), con consecuencia económica: presentar un replay como costo liquidado.

### Partición propuesta (nada de esto se ejecuta en esta orden)

| # | Acción propuesta | Dónde exactamente | ¿Toca código? | Dueño | Orden propuesta |
|---|---|---|---|---|---|
| D1 | **Renombrar la AFIRMACIÓN, no (necesariamente) la columna**: en la superficie de API/UI el path ya se llama `paths.actual`, y el breaker ya emite `provenance` con el texto de replay. El débito es de **nombre**: exponerlo como `paths.replayed` (o renombrar el campo de wire) y dejar la columna de DB como está para no migrar consumidores. | `backend/api-server/src/routes/risk-circuit-breakers.ts:134-171` (tipos + provenance) y su eco en `frontend/features/risk/RiskCircuitPanel.tsx` | **SÍ** | Backend + Frontend | **Orden aparte** (fuera de `docs/`) |
| D2 | **Productor, si algún día hay broadcast**: hoy la procedencia puede ser un **string global** porque hay **un solo escritor y es replay**. El día que exista un **segundo escritor** (recibos on-chain), el string global se vuelve falso para esas filas ⇒ hace falta una **columna marcadora por fila** (`actual_gas_source`), y ese es el gate de entrada. | `database/migrations/051_paper_trade_runs.sql` (esquema) + `backend/recon/src/drift_tracker.rs:213` (writer) | **SÍ** | Data + Backend | **Condicionada**: sólo si aparece el segundo escritor |
| D3 | **Declarar NO COMPUTADO de forma PERMANENTE lo que es permanente**: en modo paper **nunca** habrá gas realizado — no es "pendiente de backfill", es **inexistente por construcción**. La instrumentación correcta ya existe (el ledger `gas_measurement_state` de la migración 126 con sus estados `not_applicable`/`impossible`): lo que falta es **declararlo cerrado** en el contrato de categorías, para que ninguna UI prometa una convergencia que no puede ocurrir. | `docs/data/` (contrato de categorías, este perfil) + el `reason` del breaker | **NO** (documental) | **Data** | **Esta línea de trabajo** — sin dependencia de código |

**Lo que NO propongo:** borrar ni renombrar las columnas `actual_*` de la DB (rompería consumidores por un problema de nombre), ni "arreglar" el switch `ARBX_DRIFT_TRACKER_MODE` (es una decisión del operador, no un defecto), ni tocar el breaker en esta orden.

**D3 ejecutado acá:** para la categoría **REALIZADO** de una corrida paper, el estado honesto es **NO REALIZADO / NO COMPUTADO permanente** — no un `0` y no un "pendiente". Es la misma regla que `ECON-ACCOUNTING-01` fijó para la ruta de t48, ahora con su consecuencia de esquema declarada.

---

## 7. P/N NO SE MUEVE

**`P/N` sigue en `0/115`.** Este trabajo **no** lo mueve, **no** cierra ningún criterio de negocio y **no** convierte en verificado ningún otro criterio.
Lo que hace es una sola cosa: **un entregable del perfil Data vuelve a ser versionable con `git add` normal**, y deja declarada la disposición del hallazgo `actual_*` para que otra orden la ejecute. Un `.gitignore` no mueve un criterio económico.

---

## 8. CLON AISLADO Y `auto-deploy-vps.yml`

**No se mergeó y no se empujó a `main`.** Rama: `fix/gitignore-docs-data-01`, creada desde `origin/main` `c89d21a3`; el push es de la rama y el PR queda abierto para revisión independiente.

**Estado del workflow medido ANTES de empujar** (`gh run list --workflow=auto-deploy-vps.yml --limit 5`): hay un run **`in_progress`** en `main` — creado `2026-10-06T02:48:51Z`, título *"Merge pull request #815 from hefarica/sre/runtime-identity-repeat-01"*.

**Por qué la rama no puede tocarlo** (verificado leyendo el trigger, no asumido):

```yaml
# .github/workflows/auto-deploy-vps.yml
on:
  push:
    branches: [main]
  workflow_dispatch:
```

Dispara **sólo** con `push` a `main` o dispatch manual. Un push de rama y la apertura de un PR **no lo alcanzan**. El cumplimiento de la restricción de §22.20 es por construcción del trigger, no por suerte de timing.
**Precisión fail-honest:** el documento `REGLAS-OPERATIVAS.md` **no está en el árbol** (`git ls-files | Select-String REGLAS-OPERATIVAS` → 0 coincidencias; glob → sin archivos), así que cumplo la regla **tal como la cita la orden** y **no cito un `file:line` que no leí**.

---

## 9. REPRODUCCIÓN

```bash
# 1. Test decisivo (el que manda) — BEFORE: exit 0 + ".gitignore:73:data/" / AFTER: exit 1 sin salida
git check-ignore -v --no-index docs/data/probe.md ; echo "exit=$?"

# 2. Rastreados bajo docs/data en main (portador correcto para "mergeado")
git ls-tree -r --name-only origin/main -- docs/data

# 3. La intención de data/ sigue viva (todas deben dar exit 0 + .gitignore:73:data/)
git check-ignore -v --no-index data/local.json
git check-ignore -v --no-index foo/data/x.json
git check-ignore -v --no-index docs/assets/data/x.json
git check-ignore -v --no-index sub/docs/data/x.md

# 4. Los datos dentro de docs/data siguen ignorados por tipo
git check-ignore -v --no-index docs/data/dump.sqlite   # .gitignore:80:*.sqlite
git check-ignore -v --no-index docs/data/dump.db       # .gitignore:81:*.db
git check-ignore -v --no-index docs/data/run.log       # .gitignore:44:*.log

# 5. La prueba de que cerró: add NORMAL (sin -f) de un archivo nuevo bajo docs/data/
git add docs/data/README.md && git status --porcelain -- docs/data
```

---

*Una línea funcional en `.gitignore`, anclada a la ruta del contrato, con el test decisivo citado antes y después. Sin `-f`, sin merge, sin push a `main`, sin tocar código. `P/N` sigue en `0/115`.*
