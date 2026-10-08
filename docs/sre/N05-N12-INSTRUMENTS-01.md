# N05 + N12 — dos instrumentos que se saboteaban a sí mismos

**Tarea:** t116 · **run_id:** `arbx-entrega-20261008` · **`main` medido:** `8414e51211d0a26d664b7e669af2eacbee8d1dd4` (= SHA_BASE del briefing).
**Alcance:** `.github/workflows/` (2 archivos) + este documento. **Ningún merge, ningún deploy, ninguna corrida sobre `main`: las dos corridas de validación fueron sobre la rama.**

---

## 0. Veredicto en una línea

**Los dos instrumentos quedan reparados y probados con corridas reales**: la evidencia del Public DApp ahora **sobrevive al checkout** (y si no sobreviviera, el step **FALLA**), y la sonda de la capa 3 ahora **distingue LENTA de CAÍDA** — lo demostró sola en la corrida real, reproduciendo el modo de fallo del operador y clasificándolo como degradación, no como caída.

## 1. N05/W1 — El instrumento destruía su propia evidencia

### 1.1 El patrón medido (run 37727448265, del operador), confirmado acá por sus conclusions

```
2 success  :: Register the three SHAs separately and decide whether this run evaluates anything
3 success  :: actions/checkout@93cb6efe…                        <-- LIMPIA el arbol
5 failure  :: Record tested source, without production secrets
6 skipped  :: Build the liquidity-catalog model the browser audit consumes
7 skipped  :: Install locked browser
8 skipped  :: Actual public browser, GET and read-only socket subscriptions     <-- el navegador real
9 failure  :: Read the served SHA from the audit report, separately from the other two
10 success :: Publish the verdict of this audit run             <-- veredicto VACIO en verde
11 failure :: Preserve failures as well as screenshots
```

**La causa:** el step 2 hacía `mkdir -p public-evidence` y escribía `sha-register.txt` **dentro del workspace**; el step 3 (`actions/checkout`) trae el comportamiento por defecto de **limpiar el árbol** (`git clean -ffdx` + `git reset --hard`) y **borra `public-evidence/`**; el step 5 intenta escribir ahí y muere con `source-sha.txt: No such file or directory`; los steps 6-8 se saltan (incluido el navegador real) y el step 10 **publicaba "el gate no llegó a correr" con `process.exit(0)`** — un veredicto vacío en verde. **Un SKIP silencioso es el fallo, y el arreglo lo trata como tal.**

### 1.2 El arreglo (3 piezas)

1. **La evidencia se escribe TAMBIÉN fuera del workspace**: `mkdir -p public-evidence "$RUNNER_TEMP/public-evidence"` y el registro se escribe con `| tee` en los dos sitios. **`$RUNNER_TEMP` no lo limpia el checkout.**
2. **Step nuevo inmediatamente después del checkout** — `Restore the evidence the checkout wiped, and FAIL if it did not survive` — que restaura el registro y **FALLA** (`::error::EVIDENCIA PERDIDA`, `exit 1`) si no sobrevivió. *Un paso que no corre no certifica nada.*
3. **El step del veredicto ahora FALLA** cuando `evaluate=true` y el registro no existe (`process.exit(1)` + `::error::`), en vez de publicar vacío con `exit 0`. La condición está disponible vía `env: EVALUATE: ${{ steps.gate.outputs.evaluate }}`.

**Orden resultante** (`grep -n '^      - '`): L47 gate → **L112 checkout** → **L122 guard** → L135 setup-node → L141 `Record tested source` → L149 build → L159 browser install → L164 **browser real** → L177 served SHA → L210 veredicto → L274 artefactos.

### 1.3 La prueba: corrida REAL sobre la rama

**Run `37735028949`** (`workflow_dispatch`, ref `sre/n05-n12-instruments-01`, head `3a786119`) = **`completed/success`**:

```
2 success  :: Register the three SHAs separately and decide whether this run evaluates anything
3 success  :: Run actions/checkout@93cb6efe…
4 success  :: Restore the evidence the checkout wiped, and FAIL if it did not survive   <-- el guard NUEVO
5 success  :: Run actions/setup-node@…
6 success  :: Record tested source, without production secrets        <-- ANTES: failure
7 success  :: Build the liquidity-catalog model the browser audit consumes   <-- ANTES: skipped
8 success  :: Install locked browser                                  <-- ANTES: skipped
9 success  :: Actual public browser, GET and read-only socket subscriptions  <-- ANTES: SKIPPED
10 success :: Read the served SHA from the audit report, separately from the other two
11 success :: Publish the verdict of this audit run
12 success :: Preserve failures as well as screenshots                <-- ANTES: failure
```

Salidas literales de esa corrida:

```
[step 4] evidence restored into the workspace: 221 bytes
[step 9] { "checks": { "status_before": { "status": "passed" }, "pages": { ...
[step 10] served_sha=8414e51211d0a26d664b7e669af2eacbee8d1dd4   reason=leido de report.json .after.sha
[step 11] verdict=DIAGNOSTICO (sin expected_sha: NO certifica version candidata)   certifies_candidate=false
[step 9 env] DAPP_ORIGIN=https://arbx.ape-tv.net
```

⇒ **El navegador real CORRIÓ** (antes era `skipped`), leyó el SHA servido `8414e512…`, y el veredicto se publicó con la semántica correcta: una corrida manual sin `expected_sha` **diagnostica y no certifica** (`certifies_candidate=false`). **N05 queda cerrado con una corrida, no con una promesa.**

## 2. N12/W2 — La sonda no distinguía lenta de caída

### 2.1 Por qué 10 s no alcanzaban (medido en t103)

- `limit=1` es **BIMODAL**: n=12 en la ventana del api-server, **7 abortos a ~300,4-300,7 s** y 5 completos en 74-117 ms.
- El tope del aborto está en la **capa HTTP**: `api-server/src/index.ts:2051` (`httpServer.listen`) con **0 ocurrencias** de `requestTimeout|headersTimeout|keepAliveTimeout` ⇒ defaults de Node (**300 s**).
- Con `-m 10`, una degradación lenta se reportaba **`HTTP 000`** ⇒ **indistinguible de una caída**.

### 2.2 El arreglo: dos superficies, dos presupuestos, dos formas — con el criterio DECLARADO en el log

| eje | antes | ahora |
|---|---|---|
| superficies | 1 (edge 8788) | **2**: DIRECTO `8080/api/v1/…` (**sin caché**) + EDGE `8788/api/…` |
| presupuestos | 10 s | **10 s** y **reintento a 45 s** |
| formas | 1 (`limit=1`) | **2** (`limit=20` y `limit=1`) |
| criterio | `HTTP 000` | **PASS exige LAS DOS en 200** · solo-presupuesto-largo → **LENTA** (`api_slow`) · ninguna ni con 45 s → **CAÍDA** (`api_down`) · edge sí / directo no → **`edge_masks_dead_api`** (caché NO descartable) · directo sí / edge no → **`edge_not_serving`** |

### 2.3 La prueba: la corrida REAL reprodujo el modo de fallo del operador y lo clasificó

**Run `37735021940`** (ref `sre/n05-n12-instruments-01`, head `3a786119`) = **`completed/success`**, **10/10 steps en success**. Log literal de la capa 3:

```
probe surface=direct8080 shape=limit20 budget=10s http=200 time=0.149508s bytes=119791
probe surface=edge8788   shape=limit1  budget=10s http=000 time=10.002046000s bytes=0      <-- el modo de fallo del operador, reproducido
probe surface=edge8788   shape=limit20 budget=45s http=200 time=0.729189s bytes=120068     <-- y contesta con el presupuesto largo
cache_discarded_by=direct_8080 — el api-server contesto DIRECTO, asi que la respuesta no es cache del edge
##[warning]capa 3 DEGRADADA (LENTA, no caida) en: edge8788 | latencias finales: directo=0.149508s edge=0.729189s. Contesto solo con el presupuesto de 45s.
evidence: http_direct=200 http_edge=200 bytes=119791 keys=… source=direct8080
API /opportunities/live returns 20 items (source=direct8080)
```

**Lectura:** con el instrumento VIEJO, esa misma corrida habría dicho `HTTP 000` → `PIPELINE ROJO capa 3: la API respondio HTTP 000, no 200` — **exactamente el rojo que el operador vio en el run 37713413446** con Redis/PG/WS/Docker sanos. Con el instrumento nuevo: **`000` en la forma vieja, `200` en 0,729 s con otra forma y otro presupuesto, y `200` en 0,1495 s por la superficie directa** ⇒ **LENTA, no caída.**

**DECISIÓN DE DISEÑO DECLARADA (para el capitán):** una capa **LENTA** deja la corrida en **verde con un `::warning::` nombrado** (las dos superficies acabaron en 200). El criterio pedido era **distinguir**, y se distingue con el nombre, la forma, el presupuesto y las latencias. Si el operador quiere que *lento también sea rojo*, es **una línea** (`exit 1` en vez del warning) — no la tomé por mi cuenta porque cambiar la severidad de un gate es decisión suya, no reparación.

## 3. W3 — La caché del borde, declarada con método

El borde sirve de caché (medido en t103: `0,121938 → 0,002147 → 0,002453`, y `0,097747` tras 4 s de pausa), así que **un 200 del edge no prueba que el api-server haya contestado**.

**El método para descartarlo:** la sonda consulta **también el api-server DIRECTO** (`127.0.0.1:8080/api/v1/…`), que **no pasa por el borde y no tiene caché**. Si el directo contesta, la respuesta no puede venir de la caché del edge — y el log lo declara: **`cache_discarded_by=direct_8080`** (visible en la corrida real, §2.3).

**Y el límite se declara cuando NO se puede descartar:** si el edge contesta y el directo no (ni con 45 s), el veredicto es **`edge_masks_dead_api`** con el aviso explícito de que **la respuesta del edge puede ser caché y no se puede descartar que el API esté caído**. La ambigüedad no se esconde: se nombra.

## 4. W4 — Lo que no se toca, y por qué el gate no se ablanda

- **Capas 1 (Redis), 2 (PG), 4 (WS) y 5 (Docker): intactas.** Prueba de la corrida real: **los cinco steps de capa + preflight + verdict = 10/10 en success**, con las capas 1,2,4,5 sin cambios de texto.
- **El gate NO se vuelve más fácil:** antes bastaba **una** superficie (posiblemente **cacheada**); ahora **PASS exige las dos en 200**, y los cuatro casos nuevos son **rojos nombrados**, no pases: `api_down`, `api_slow`, `edge_masks_dead_api`, `edge_not_serving`. El único caso que pasa es "la API contesta de verdad **y** el consumidor tiene dato".

## 5. W5 — Las corridas reales (sobre la rama, nunca sobre `main`)

| run | workflow | ref | head | conclusion | steps |
|---|---|---|---|---|---|
| **`37735021940`** | Pipeline Integrity Gate | `sre/n05-n12-instruments-01` | `3a786119` | **`completed/success`** | **10/10** success (incluye Layer 3) |
| **`37735028949`** | Public DApp verified journeys | `sre/n05-n12-instruments-01` | `3a786119` | **`completed/success`** | 25/25 success (incluye el guard nuevo y el navegador real) |

**Hazard declarado (es mío, y lo medí yo en t64):** `pipeline-integrity.yml` tiene `concurrency: group: pipeline-integrity / cancel-in-progress: true`. Mi `workflow_dispatch` sobre la rama **pudo cancelar la corrida anterior de ese mismo grupo** (posiblemente un `schedule` de `main`). No toca producción ni cambia el SHA de `main`, pero **no es inocuo**: el grupo es compartido y el instrumento puede cancelarse a sí mismo su propia medición. *(El `public-dapp-verification.yml` tiene `cancel-in-progress: false`: ese no cancela nada.)*

## 6. W6 — Alcance, y la no-regresión declarada (no asumida)

- **PR propio: #864**, `state=OPEN`, **`merged_at=null`**, 2 archivos, **SIN merge**. Es el único path tocado junto con este documento.
- **¿Puede afectar la DApp o el deploy?** Los dos archivos son **`.github/workflows/`**, no la DApp.
  - **`public-dapp-verification.yml`**: por diseño es una **condición POSTERIOR** al deploy (su encabezado lo dice: *"NO un requisito del despliegue del que depende para arrancar: convertirla en requisito del deploy sería una dependencia circular"*). **Merging no agrega ningún gate al deploy.** Lo que cambia es **después**: la auditoría pasa de *saltarse el navegador en silencio* a **correrlo y publicar un veredicto real**. ⇒ **No hay regresión de deploy; hay más certificación.**
  - **`pipeline-integrity.yml`**: corre por `schedule` y manual; **no es un gate del deploy**. Se vuelve **más estricto** (dos superficies donde antes bastaba una) y agrega una clase de aviso. Podría poner rojo un caso que hoy pasaría **en cacheado**: es el objetivo, no un efecto colateral.
  - **Riesgo residual declarado:** si el borde queda sirviendo y el api-server directo no, el gate dirá `edge_masks_dead_api` (**rojo**) donde antes decía verde **con una respuesta de caché**. Es la severidad que se pidió.
- **Nada de la DApp se tocó**: `backend/`, `frontend/`, `monitoring/`, `docker/` sin cambios (`git diff --stat` = 2 archivos, ambos en `.github/workflows/`).

## 7. Reproducción

```bash
# W5: las dos corridas reales, sobre la RAMA
gh run view 37735021940 --repo hefarica/arbitragex-v2 --json jobs | jq -r '.jobs[].steps[] | "\(.number) \(.conclusion) :: \(.name)"'
gh run view 37735028949 --repo hefarica/arbitragex-v2 --json jobs | jq -r '.jobs[].steps[] | "\(.number) \(.conclusion) :: \(.name)"'
# W2: el criterio y la evidencia de la sonda, literal
gh run view 37735021940 --repo hefarica/arbitragex-v2 --log | grep -E 'probe surface=|cache_discarded_by|DEGRADADA|returns .* items'
# W1: el guard y el navegador
gh run view 37735028949 --repo hefarica/arbitragex-v2 --log | grep -E 'evidence restored|served_sha=|verdict=|certifies_candidate='
# validacion estatica de los dos archivos
python -c "import yaml;[yaml.safe_load(open(p,encoding='utf-8')) for p in ['.github/workflows/pipeline-integrity.yml','.github/workflows/public-dapp-verification.yml']];print('yaml_ok')"
```
