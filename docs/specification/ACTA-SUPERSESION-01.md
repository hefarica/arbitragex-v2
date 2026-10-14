# ACTA-SUPERSESION-01 — Qué pide cada tarea fallada y dónde está entregado

- **Rol:** Specification · tarea **t181** · `attempt_id 5b2eb78c-056e-4379-840d-9240faa2cafb`
- **Autoridad:** el OPERADOR autorizó expresamente documentar la supersesión. **NO autorizó declarar superseded algo que no se puede probar.**
- **Naturaleza:** solo lectura y documentación. **CERO cambios de código.** No reabre tareas, no cambia veredictos, no reescribe la historia.
- **SHA de `main` al medir:** `d5114eddf59ac1f9ae81be58edc22fffd75b65ee`
- **Método:** todo artefacto se verifica **por el REMOTO** (`gh pr view`, `git ls-remote`, `git rev-parse <sha>:<ruta>`), nunca por la respuesta de un push. Cada verificación con su **exit code**.

---

## 0. La regla que ordena esta acta

**No generalizar más allá de lo que se midió.** Está **prohibido** escribir «todos los fallos viejos están superseded».

**El resultado, contado:**

| Conjunto | Total | Con artefacto MERGED/pass | De rama publicada (no mergeada) | `PARCIAL` | `NO EVIDENCIADO` |
|---|---|---|---|---|---|
| Los **9** `failed without a follow-up repair` del bloque `Delivery` | **9** | **6** | **2** (t80, t81 — mismo objeto) | **1** (t79) | **0** |
| `pending/not completed` citados por la orden | 3 (t128, t130, t134) | 1 (t134 → #884 OPEN) | 0 | 0 | **2** (t128, t130) |
| Canceladas con razón ya declarada por el capitán | 2 (t144, t146) | 2 | 0 | 0 | 0 |

**Verificación 6+2+1 = 9.** Los **6** con artefacto MERGED/pass son **t5, t66, t73, t74, t76, t82** (concuerda con la tabla de §1.10).
**Los 3 `NO EVIDENCIADO` (t79 parcial + t128 + t130) quedan ABIERTOS.** **No se cierra nada por conveniencia.**

---

## 1. Los 9 `failed without a follow-up repair` — uno por uno

### 1.1 t5 — `E-3 REV-IND-01` — ✅ **SUPERSEDED, ARTEFACTO VERIFICADO**

| | |
|---|---|
| **Qué pedía** | Dictamen independiente sobre la estrategia de integración (E-2) → Reviewer |
| **Estado en el tablero** | `[failed]` verification · verdict **needs_revision** · attempt 1 |
| **Artefacto que lo supersede** | **t14 `REV-IND-01-R4`**, veredicto **`pass`** — *«FIRMA DEL GATE 2. El GATE 2 QUEDA FIRMADO; E-4 (t6) queda desbloqueada»* |
| **Cadena** | t5 (needs_revision) → t9 → t11 → t13 → **t14 (pass)** |
| **Verificación** | Estado y veredicto leídos **del tablero**, no de memoria |

**Nota de mecanismo (importante):** `t14` es de kind `review`, **no** `repair`. Por eso el tablero **sigue listando t5** aunque su sujeto esté entregado y firmado (§5).

### 1.2 t66 — `PRODUCTOR-SIM-01` — ✅ **SUPERSEDED, ARTEFACTO VERIFICADO**

| | |
|---|---|
| **Qué pedía** | `sim-ctl` lee 12,2M entradas con lag 0 y escribe CERO filas: el corte está adentro → Backend |
| **Estado** | `[failed]` implementation attempt 2 |
| **Artefacto que lo supersede** | **PR #835** — verificado por el remoto: `state=MERGED`, `mergedAt=2026-10-07T16:24:44Z`, `headRefOid=6d0ed76285915a3f5f47ec7fffa6f1fa91ae5108`, `changedFiles=1` |
| **Causa del `failed`** | la tarea cerró como fallo **por autoridad de medición**, no por ausencia de arreglo: *«CERO código tocado. CERO filas escritas en `simulations`»* |

**Lo que NO se declara:** que el corte «esté arreglado y midiendo» en producción. **El artefacto entregado es el PR #835 MERGED; la medición post-fix no está evidenciada en esta acta.**

### 1.3 t73 — `SELECTOR-PARSE-01` — ✅ **SUPERSEDED, ARTEFACTO VERIFICADO**

| | |
|---|---|
| **Qué pedía** | `selector-api` tira 100% de lo que el searcher publica: 2.043 `invalid_message`/20min y cero publicaciones → Backend |
| **Estado** | `[failed]` implementation attempt 1 |
| **Artefacto que lo supersede** | **PR #838** — `state=MERGED`, `mergedAt=**2026-10-06T13:52:41Z**`, `headRefOid=079deb3f879f734374f9f595f8b81b32deb71c89`, `changedFiles=7` |
| **Causa del `failed`** | *«CAUSA RAIZ ENCONTRADA Y FIX VERIFICADO AL 100%»*, pero falló **en las tres mediciones post-fix por falta de autoridad de deploy** |

### 1.4 t74 — `KELLY-GUARD-CLIPPY-01` — ✅ **SUPERSEDED, ARTEFACTO VERIFICADO**

| | |
|---|---|
| **Qué pedía** | La guarda dispara `clippy::neg_cmp_op_on_partial_ord` bajo `-D warnings`: opción (A) ordenada → Security |
| **Estado** | `[failed]` **repair r1** attempt 2 |
| **Artefacto que lo supersede** | **PR #827 `security/kelly-guard-01`** — `state=MERGED`, `mergedAt=2026-10-07T06:21:35Z`, `mergeCommit.oid=3e63a2ca67ac4896b2c76317348bde3fd3492dbe`, `headRefOid=8111f5899ce6a5417dc8ba5c8288fdf58de089cf`, `changedFiles=1` |
| **Causa del `failed`** | *«arreglo APLICADO y verificado localmente (las tres verdes en WSL2)»*; falló porque **el veredicto de CI quedó SIN MEDIR (encolado)** |

**El head del PR coincide con el head de la rama en el remoto**: `git ls-remote … refs/heads/security/kelly-guard-01` → `8111f5899ce6a5417dc8ba5c8288fdf58de089cf`, **idéntico** al `headRefOid` del PR. Verificación cruzada por dos vías.

### 1.5 t76 — `KELLY-GUARD-LAND-01` — ✅ **SUPERSEDED, ARTEFACTO VERIFICADO**

| | |
|---|---|
| **Qué pedía** | Medir el veredicto de CI de #827 y aterrizarlo: es la precondición de capital → Security |
| **Estado** | `[failed]` **verification r1** attempt 1 |
| **Artefacto que lo supersede** | **PR #827 MERGED** (id. §1.4) — el aterrizaje que la tarea pedía **ocurrió**, por otra vía |
| **Causa del `failed`** | *«Cierro como `failed` porque el criterio de aterrizaje no se ejecutó — **no porque CI falle**»*; el veredicto existía y era **VERDE en los dos contexts requeridos**; no se mergeó **por la orden de congelamiento del capitán (deploy en vuelo)** |

**t74 y t76 son la pareja medida:** t74 no pudo leer el veredicto de CI; t76 lo leyó y era verde; ninguna de las dos aterrizó; **#827 lo aterrizó después.**

### 1.6 t79 — `POOL-COVERAGE-01` — ⚠️ **SUPERSEDED — PARCIAL. Nadie declaró el artefacto de t79**

| | |
|---|---|
| **Qué pedía** | El motor rechaza 25,40% SIN LLEGAR A EVALUAR: **6.017 pares observados y nunca indexados** → Data |
| **Estado** | `[failed]` work attempt 1 — *«FALLIDA EN SU CRITERIO DE EJECUCIÓN, con el bloqueo localizado y medido»* |
| **Artefacto que la tarea declara** | `docs/data/POOL-COVERAGE-01.md` (15.892 B, sha256 `f5daa440ab279bf965331cf712fb60fd75f23eb8cb905ba48b1b4698962d0ef8`) — **sin merge, sin CI, sin push** |
| **Sujeto entregado después** | **SÍ, por cadena distinta de tareas Data**, no por un artefacto que cite a t79: **t33 `POOL-UNIVERSE-01`** (1.305 de 4.207 pools, 6.017 pares) → **t136 `POOL-UNIVERSE-02`** (los 41 pares: *«la respuesta es NO, cerrada por tres puertas medidas»*) |

**Dictamen honesto:** el **sujeto** (6.017 pares nunca indexados) **fue medido después y cerrado por tres puertas** en t33/t136. Pero **el artefacto de t79 no está acreditado en el repo por este acta** (quedó sin push). Se declara **`PARCIAL`**: sujeto cubierto, artefacto propio `NO EVIDENCIADO`.

### 1.7 t80 — `POOL-CIRCULARITY-01` — ✅ **SUPERSEDED, ARTEFACTO VERIFICADO (rama publicada)**

| | |
|---|---|
| **Qué pedía** | Círculo cerrado: sin candidatos no hay veredictos, y sin veredictos no hay pools → Backend |
| **Estado** | `[failed]` implementation attempt 1 — *«C2 IMPLEMENTADO Y VERIFICADO; C1 NO IMPLEMENTADO porque su premisa está REFUTADA por medición»* |
| **Artefacto que lo supersede** | rama **`feat/pool-circularity-01`**, commit `50165a7f9d…` (trabajo local, clon aislado) → **publicada por t81** |

### 1.8 t81 — `POOL-CIRCULARITY-02` — ✅ **SUPERSEDED, ARTEFACTO VERIFICADO**

| | |
|---|---|
| **Qué pedía** | Cablear C2 (una línea) y **empujar**: el barrido es lo único que rompe el círculo → Backend |
| **Estado** | `[failed]` implementation attempt 1 — *«C2 CABLEADO Y EMPUJADO, verificado por el remoto sin regresión. FALLIDO en las 3 mediciones que deciden: NO SE MOVIÓ»* |
| **Artefacto que lo supersede** | rama en el remoto: **`git ls-remote … refs/heads/feat/pool-circularity-01` → `1130a6e78cf98d7737b1f2126c4563652776fb5e`** — coincide con el head `1130a6e7` que t81 declaró |
| **Estado de `main`** | t81 declaró `main` intacto en `bceb31ef`; hoy `main` = `d5114edd`. **El push de la rama ocurrió; `main` avanzó después por otros merges, no por este.** |

### 1.9 t82 — `FIX-SELGATE-01` — ✅ **SUPERSEDED, ARTEFACTO VERIFICADO**

| | |
|---|---|
| **Qué pedía** | Restaurar el log de decisiones: `validated` vacío por construcción desde 2026-09-17 → Backend |
| **Estado** | `[failed]` implementation attempt 1 — *«EL FIX ESTÁ HECHO Y VERIFICADO LOCALMENTE; las mediciones que deciden quedan pendientes del despliegue de t86. FALLIDO sólo en la aceptación 6 (`entries-added` no avanzó)»* |
| **Artefacto que lo supersede** | **PR #843** — `state=MERGED`, `mergedAt=**2026-10-07T20:20:46Z**`, `headRefOid=c78cdea3f56b0cfb35c6a74bb1cb5263700f6dc2`, `changedFiles=3` — coincide con el head `c78cdea3` que t82 declaró |

**Nota:** el `failed` de t82 era **una sola aceptación** (`entries-added` no avanzó). No es un fallo de arreglo: es un fallo **de medición post-despliegue**, y el despliegue sigue bloqueado (§3).

### 1.10 Recuento de los 9

| # | Tarea | Sujeto | Artefacto |
|---|---|---|---|
| 1 | t5 | entregado | **t14 pass** |
| 2 | t66 | entregado | **#835 MERGED** |
| 3 | t73 | entregado | **#838 MERGED** |
| 4 | t74 | entregado | **#827 MERGED** |
| 5 | t76 | entregado | **#827 MERGED** |
| 6 | t79 | entregado | **PARCIAL** — sujeto por t33/t136; artefacto propio `NO EVIDENCIADO` |
| 7 | t80 | entregado | rama publicada (por t81) |
| 8 | t81 | entregado | rama `1130a6e7` en remoto |
| 9 | t82 | entregado | **#843 MERGED** |

> **`6 de 9` con artefacto MERGED/pass verificado · `1 de 9` PARCIAL (t79) · `2 de 9` con artefacto de rama publicada, no mergeada (t80/t81 comparten el mismo objeto).**
> **`6 + 2 + 1 = 9`.** **`1 de 9` (t79) queda con su artefacto propio `NO EVIDENCIADO`.** **No se declara «todos superseded»: no está medido.**

---

## 2. `pending · not completed` citados por la orden

| Tarea | Qué pedía | Estado | Artefacto |
|---|---|---|---|
| **t128** `FORK-TRUST-01` | El fork devuelve estado vacío y el sistema lo lee como un valor: **la métrica entera está contaminada** | `[pending]` **attempt 0** | **`NO EVIDENCIADO`** — **ABIERTA** |
| **t130** `PRODUCER-IDENTITY-01` | El gate descarta por nombre: **`triangular` declarado vs `triangular_arb` implementado** | `[pending]` **attempt 0** | **`NO EVIDENCIADO`** — **ABIERTA** |
| **t134** `N11-SHAREDRS-02` | Declarar `verdict`/`verdict_reason` cubriendo el ALCANCE REAL: **52 literales en 29 archivos** | `[pending]` **attempt 0** | **PR #884** — `state=OPEN`, `isDraft=**false**` (**READY FOR REVIEW**), `headRefOid=**7e059208206e9ef54cdc2aa88e6eb6bd94aa3224**` (coincide con el `7e059208…` declarado), `changedFiles=5`, `mergedAt=**null**` |

**Dictamen:** de los tres, **t134 SÍ** tiene sujeto entregado (#884, abierto sin merge). **t128 y t130 NO** tienen artefacto identificable en esta acta ⇒ **`NO EVIDENCIADO`, ABIERTAS.**

**Mecanismo que las mantiene colgadas:** las tres declaran `(deps: t124)` y **`t124` está `cancelled`**. En este harness **`cancelled` cuenta como «unfinished» en el gate de dependencias**, así que **t128/t130/t134 quedan deadlocked** — no por falta de trabajo, por un ancestro cancelado.

---

## 3. `t132` y `t144`/`t146` (canceladas por el capitán)

| Tarea | Sujeto | Artefacto |
|---|---|---|
| **t132** `GSIM-ENCODER-COVERAGE-01` | Ya cancelada por el capitán | **PR #885** — `state=OPEN`, `isDraft=**true**` (DRAFT), `headRefOid=a6769b94a9527a7cbfa4d798689defc880f3cca7`, `changedFiles=9`, `mergedAt=null` |
| **t144** `LAND-874-01` | Nunca reclamada (attempt 0, **ninguna línea escrita**); cancelada por dependencia | Superseded por **t157 `SSOT-VERSIONADO-02`** (pending) — **no es un fallo del trabajo** |
| **t146** `PRODUCER-PRICE-FAILCLOSED-01` | Cancelada por **FALSACIÓN CIENTÍFICA**: la hipótesis que existía para arreglar **murió con evidencia**, en **t160 (PR #889)** | **No hay nada que entregar: el sujeto no existe.** Cancelar fue correcto |

---

## 4. El artefacto de la orden — `t138` vs `t172`

**t138** (`W12-F25-CALLBACK-BINDING-01`, `[failed]` implementation): cadena **t141 → t148 → t149 → t153 → PR #878**. Verificado por el remoto:

| Verificación | Comando | Resultado |
|---|---|---|
| PR #878 | `gh pr view 878 --json state,mergedAt,mergeCommit,changedFiles` | `state=**MERGED**`, `mergedAt=**2026-10-08T15:49:13Z**`, `changedFiles=4`, `headRefOid=937b3b12dafa33b22ce4196e6cc1bd2233c0ac98` · **exit=0** |
| `main` | `git ls-remote … refs/heads/main` | **`d5114eddf59ac1f9ae81be58edc22fffd75b65ee`** (no `77b42b3d`: #878 aterrizó) · **exit=0** |
| Contrato en `main` | `git rev-parse d5114edd:contracts/src/FlashLoanExecutor.sol` | **`cfdf5b93822b2aac4aae2c7cc7f985a702e658bf`** — coincide · **exit=0** |
| `ci-gate` tras el merge | `gh api …/commits/d5114edd/check-runs` | `{"conclusion":"success","name":"ci-gate","status":"completed"}` · **exit=0** |

> **`t172` (`LAND-878-02`, `[failed]`) es el segundo intento del mismo sujeto, y su primera mitad SÍ ocurrió** (#878 aterrizó). Falló en la segunda mitad: **F25 sigue abierto porque el deploy quedó bloqueado.**

**El deploy bloqueado — se declara, no se oculta:**

```
gh api repos/hefarica/arbitragex-v2/actions/runs/37803874672/jobs
  {"conclusion":"failure", "name":"Wait for all deployment gates", "status":"completed"}
  {"conclusion":"skipped", "name":"Deploy to VPS",              "status":"completed"}
  exit=0
```

**Runtime servido (`curl -s http://195.201.235.70/api/status`, exit=0):** `deploy.sha = **77b42b3dccc001455fda3e8d4437d973c9e98c4b**` · `deploy.at = 10/08/2026 13:47:10`.

**Contraste medido:** `main` = `d5114edd` **≠** runtime servido = `77b42b3d`. **El contrato arreglado está en `main` pero NO se está sirviendo.** Por eso t172 es `failed`: su criterio era *«cerrarlo leyendo el bytecode SERVIDO»* y el bytecode servido no cambió.

---

## 5. ★ EL MECANISMO — ¿esta acta desbloquea `Delivery`?

**NO. Esta acta NO desbloquea `Delivery` por sí sola.** Decirlo de otro modo sería presentarle al operador un desbloqueo que no ocurre.

**Lista verbatim del tablero** (25 items):

```
Delivery: blocked (team requires escalation resolution;
  t79 (work) is not completed; t177 (work) is not completed; t179 (work) is not completed;
  t181 (work) is not completed;
  t5 failed without a follow-up repair;  t66 failed without a follow-up repair;
  t73 failed without a follow-up repair; t74 failed without a follow-up repair;
  t76 failed without a follow-up repair; t80 failed without a follow-up repair;
  t81 failed without a follow-up repair; t82 failed without a follow-up repair;
  t128 (implementation) is not completed; t130 (implementation) is not completed;
  t134 (implementation) is not completed;
  t138 failed without a follow-up repair; t156 failed without a follow-up repair;
  t157 (integration) is not completed;   t162 (integration) is not completed;
  t165 failed without a follow-up repair;
  t166 (integration) is not completed;   t169 (integration) is not completed;
  t172 failed without a follow-up repair;
  t178 (integration) is not completed;   t180 (review) is not completed)
```

### 5.1 El predicado, leído de la lista y no inferido

**El bloqueo se computa desde los ESTADOS DE TAREA**, y la frase **`failed without a follow-up repair`** nombra su predicado: *un `failed` sin un `repair` de seguimiento queda listado*. Corroborado por el dato duro:

**`t5` tiene cadena de supersesión completa hasta `t14` con veredicto `pass` y FIRMA DEL GATE 2 — y el tablero LO SIGUE LISTANDO.** Luego **la evidencia documental no satisface el predicado**: lo que lo satisfaría es un **`repair` de seguimiento**, y `t14` es de kind `review`, no `repair`.

### 5.2 Qué haría falta mecánicamente

`Delivery` **no se desbloquea con un acta: se desbloquea con ESTADO.**

1. **Un `repair` de seguimiento por cada `failed`** que llegue a `completed` — 9 casos (`t5, t66, t73, t74, t76, t80, t81, t82, t138`), más **t156, t165, t172** que también están en la lista y **no estaban en el encargo de esta acta** (los declaro: existen, están listados, y **no los evidencié**).
2. **Completar las `pending`**: `t128, t130, t134, t157, t162, t166, t169, t178, t180` — **9 casos**.
3. **Desbloquear el deadlock de ancestro**: `t124` está `cancelled` y **`cancelled` cuenta como unfinished**, así que t128/t130/t134 no arrancan por más que se quiera.
4. **Cerrar la propia cola viva**: `t79, t177, t179, t181` — **t181 es esta acta**; al completarse sale de la lista, los otros **3 no**.
5. **`team requires escalation resolution`** — condición de equipo, **no** satisfacible por una tarea individual.

### 5.3 Efecto medido de esta acta sobre la lista

| | |
|---|---|
| Items en la lista antes de t181 | **25** |
| Items que t181 retira al completarse | **1** (`t181`) |
| Items que t181 desbloquea por evidencia documental | **0** |
| Items que siguen bloqueando después de t181 | **24** |

> **Esta acta convierte trabajo ya hecho en trabajo LOCALIZADO. No cambia ningún estado, y `Delivery` se computa de estados.** El valor entregado es que **nadie tenga que re-derivar** qué pedía cada tarea fallada ni dónde quedó — no que el gate se abra.

---

## 6. Lo que esta acta NO es

- **No es una reparación.** No se tocó una línea de código; cero escrituras.
- **No reabre ninguna tarea.** No reclamé ninguna de las listadas.
- **No cambia ningún veredicto.** Los `failed` siguen `failed`; los `pass` siguen `pass`.
- **No reescribe la historia.** Las listas de §1–§4 son lo que el tablero y el remoto dicen hoy, con su exit code.
- **No declara superseded nada que no pueda probar**: los 4 `NO EVIDENCIADO` (t79 artefacto propio, t128, t130, y los 3 de §5.2 punto 1) quedan **ABIERTOS**.

---

## 7. Instrumento — defectos del capitán ya medidos, respetados

Estos se declaran como **heredados y no re-verificados en esta acta** (el encargo los da por medidos, y yo no los medí):

- `forge fmt --all --check` **NO EXISTE** (exit 2) — es **`forge fmt --check`**.
- `quote_block` y `gross/net` viven en el **JSONB `economics`**.
- `fail_reason` va **prefijado** (`LIKE 'prefijo%'` con control positivo, **nunca** `IN (...)`).
- El techo de mercado vive en **`opportunities.rejection_reason`**.
- `:9090` externo da **`000`**.
- La frontera pre/post es **`docker inspect StartedAt`**.
- A `localhost:8545` **no se llega desde el host** (`172.18.0.3:8545`, o `cast` dentro del contenedor).

**Ninguno de estos se ejercitó en esta acta**: su materia es documental. Se listan para que quien siga no los re-descubra.

---

## 8. Reproducción — cada verificación con su exit code

```bash
# Los 3 PRs del encargo (por el REMOTO, nunca por la respuesta de un push)
gh pr view 884 --repo hefarica/arbitragex-v2 \
  --json state,isDraft,headRefOid,changedFiles,mergedAt ; echo "exit=$?"
#   -> {"changedFiles":5,"headRefOid":"7e059208…","isDraft":false,"mergedAt":null,"state":"OPEN"}  exit=0
gh pr view 885 --repo hefarica/arbitragex-v2 \
  --json state,isDraft,headRefOid,changedFiles,mergedAt ; echo "exit=$?"
#   -> {"changedFiles":9,"headRefOid":"a6769b94…","isDraft":true,"mergedAt":null,"state":"OPEN"}   exit=0
gh pr view 878 --repo hefarica/arbitragex-v2 \
  --json state,mergedAt,mergeCommit,changedFiles ; echo "exit=$?"
#   -> {"changedFiles":4,"headRefOid":"937b3b12…","mergedAt":"2026-10-08T15:49:13Z","state":"MERGED"}  exit=0

# main, por el remoto
git ls-remote https://github.com/hefarica/arbitragex-v2.git refs/heads/main ; echo "exit=$?"
#   -> d5114eddf59ac1f9ae81be58edc22fffd75b65ee  refs/heads/main   exit=0

# El contrato de t138 EN MAIN
git rev-parse d5114eddf59ac1f9ae81be58edc22fffd75b65ee:contracts/src/FlashLoanExecutor.sol ; echo "exit=$?"
#   -> cfdf5b93822b2aac4aae2c7cc7f985a702e658bf                     exit=0

# ci-gate tras el merge
gh api repos/hefarica/arbitragex-v2/commits/d5114edd…/check-runs \
  --jq '.check_runs[]|select(.name=="ci-gate")|{name,status,conclusion}' ; echo "exit=$?"
#   -> {"conclusion":"success","name":"ci-gate","status":"completed"} exit=0

# El DEPLOY BLOQUEADO (se declara, no se oculta)
gh api repos/hefarica/arbitragex-v2/actions/runs/37803874672/jobs \
  --jq '.jobs[]|{name,status,conclusion}' ; echo "exit=$?"
#   -> {"conclusion":"failure","name":"Wait for all deployment gates","status":"completed"}
#      {"conclusion":"skipped","name":"Deploy to VPS","status":"completed"}            exit=0

# PRs de las tareas falladas (todos MERGED, verificado por el remoto)
for n in 827 835 836 838 843; do gh pr view $n --repo hefarica/arbitragex-v2 \
  --json number,state,headRefOid,changedFiles,mergedAt ; echo "exit=$?"; done
#   #827 MERGED 2026-10-07T06:21:35Z  head 8111f589…  1 file
#   #835 MERGED 2026-10-07T16:24:44Z  head 6d0ed762…  1 file
#   #836 MERGED 2026-10-07T17:20:54Z  head 12a0e539…  1 file
#   #838 MERGED 2026-10-06T13:52:41Z  head 079deb3f…  7 files
#   #843 MERGED 2026-10-07T20:20:46Z  head c78cdea3…  3 files

# Ramas citadas por t80/t81 y t74
git ls-remote https://github.com/hefarica/arbitragex-v2.git refs/heads/feat/pool-circularity-01 ; echo "exit=$?"
#   -> 1130a6e78cf98d7737b1f2126c4563652776fb5e  exit=0
git ls-remote https://github.com/hefarica/arbitragex-v2.git refs/heads/security/kelly-guard-01 ; echo "exit=$?"
#   -> 8111f5899ce6a5417dc8ba5c8288fdf58de089cf  exit=0   (= headRefOid de #827)

# Runtime servido vs main (contraste declarado)
curl -s --max-time 25 http://195.201.235.70/api/status ; echo "exit=$?"
#   -> deploy.sha = 77b42b3dccc001455fda3e8d4437d973c9e98c4b   ≠ main d5114edd   exit=0

# Integridad de ESTA acta (blob de git, NUNCA Out-File)
git hash-object docs/specification/ACTA-SUPERSESION-01.md
```

---

## 9. Cierre

- **`6 de 9`** fallos viejos con artefacto **MERGED/pass** verificado (t5, t66, t73, t74, t76, t82) · **`1`** PARCIAL (t79) · **`2`** con artefacto de rama publicada pero **no mergeada** (t80, t81 — mismo objeto). **`6+2+1 = 9`.** **Nada se declara «todos superseded».**
- **`t134` SÍ** tiene sujeto entregado (#884). **`t128` y `t130` quedan `NO EVIDENCIADO` y ABIERTAS.**
- **El deploy sigue bloqueado** y el runtime sirve `77b42b3d`, no `d5114edd`: **se declara.**
- **`Delivery` NO se desbloquea con esta acta.** Se retira **1 de 25** items (`t181`); quedan **24**, y se dice **qué haría falta mecánicamente** para que caigan.

*Acta de localización, no de reparación. El trabajo que esas tareas buscaban está localizado en la medida en que se pudo verificar; lo que no se pudo verificar queda abierto con nombre.*
