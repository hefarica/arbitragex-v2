# FORK-FRESH-01 — gate de edad del fork: a minutos de la cabeza, con mutante stale que FALLA

**InScope tocado:** `scripts/fork_freshness_gate.sh` (nuevo) + este documento. **Nada más.**
**Paper, cero mainnet, cero firmas, cero broadcast, cero reinicios, cero escrituras en el fork.**

---

## 0. Resumen (cada número con su artefacto)

1. **El gate existe y es ejecutable**: `scripts/fork_freshness_gate.sh` (150 líneas, POSIX `sh`, sólo
   lecturas JSON-RPC), umbral **10 minutos** por defecto, veredicto greppable y **tres exit codes distintos**.
2. **El mutante obligatorio PASA la prueba que importa**: contra el fork REAL stale el gate devuelve
   **`EXIT=1` `verdict=STALE`** con su mensaje (§2, M1). Un gate de edad que no falla con el fork viejo sería
   un adorno; éste falla.
3. **La edad, medida hoy** (§3): `fork_block=26148216` idéntico en 3 lecturas, cabeza `26149929` ⇒
   **1713 bloques = 342,60 min = 5,71 h**. Cross-check: `StartedAt→now` = **5,694 h**, desvío **0,480 %**.
4. **La causa, con path y línea** (§4): `docker/compose.prod.yml:183` ejecuta
   `anvil --fork-url "$$ANVIL_FORK_URL" --host 0.0.0.0 --port 8545` — **sin `--fork-block-number`** — y ése es
   *verbatim* el comando del contenedor vivo. `compose.dev.yml:127` **sí** lo pasa.
5. **Bloqueo estructural declarado** (§5): el arreglo vive en `docker/`, y `docker/` está en el `inScope` de
   **t128** (`pending`, `attempt=0`, `dependencies=[t124]`, con **3 dependientes**). Esta tarea entrega
   **el patch y la prueba aquí**, y **no edita fuera de su scope**.
6. **Lo que esto NO es** (§7): **no cambia el NO económico.**

---

## 1. EL GATE: umbral, comando, salida y qué pasa cuando no se cumple

**Artefacto:** `scripts/fork_freshness_gate.sh` (`sha256 = b171fc7e0b2fee2cc0ddaac9cdc68b4e7fcf5365e0096fa57f24dba360850c10`,
150 líneas; el mismo hash se verificó en el VPS antes de ejecutarlo, §8).

**Umbral: 10 minutos.** Punto exacto donde el gate lo lee:

- `scripts/fork_freshness_gate.sh:47` — `MAX_AGE_MINUTES_DEFAULT=10` (la constante por defecto);
- precedencia declarada: `--max-age-minutes N` > `ARBX_FORK_MAX_AGE_MINUTES` > esa constante (líneas 49-51).

**Comando** (los endpoints son obligatorios: el gate **no** hardcodea ninguno):

```sh
scripts/fork_freshness_gate.sh \
  --fork-rpc "$ARBX_FORK_RPC" --head-rpc "$ARBX_HEAD_RPC" \
  [--max-age-minutes 10] [--seconds-per-block 12] [--timeout-seconds 8]
```

**Salida — una línea greppable** (URLs REDACTADAS, sin secretos):

```
FORK_FRESHNESS verdict=STALE reason=over_threshold fork_block=26148216 head_block=26149929 age_blocks=1713 age_minutes=342.60 threshold_minutes=10 seconds_per_block=12 fork_rpc=http://172.18.0.3:8545/<REDACTADO> head_rpc=https://eth.drpc.org/<REDACTADO>
```

**Exit codes — deliberadamente distintos:**

| code | veredicto | significado |
|---|---|---|
| 0 | `FRESH` | la edad está dentro del umbral |
| 1 | `STALE` | el fork está más viejo que el umbral ⇒ **el llamador DEBE abortar** |
| 2 | `UNMEASURABLE` | no se pudo medir (RPC vacío/ilegible/timeout) o config inválida (mismo endpoint, head < fork, umbral no numérico) |

**Qué pasa cuando no se cumple: el sistema NO simula con estado viejo en silencio.** El gate **no** degrada un
fallo de medida a verde: un RPC que no responde es `UNMEASURABLE` (2), nunca `FRESH`. Y `STALE` es **1**: el
contrato de uso es `scripts/fork_freshness_gate.sh … || abortar` — un llamador que ignore el código de salida
tendría que hacerlo **explícitamente**. La **conexión** de ese contrato al camino de simulación vive en
`docker/` (healthcheck/entrypoint del anvil o preflight del simulador) ⇒ **bloqueada** (§5) con el patch en §6.

**Guarda anti-adorno incorporada** (líneas 82-86): si `--fork-rpc` y `--head-rpc` apuntan al **mismo
endpoint**, la edad daría 0 siempre ⇒ el gate **rechaza** con `exit 2` y `reason=same_endpoint`. Es
exactamente el adorno que la tarea prohíbe, cerrado en el propio instrumento.

---

## 2. ★★ EL MUTANTE: CON EL FORK STALE, EL GATE **FALLA**

Batería completa ejecutada **en el VPS** contra el artefacto committeado (mismo sha256 verificado):

| # | caso | veredicto | **EXIT** |
|---|---|---|---|
| **M1** | **fork REAL stale vs cabeza real, umbral 10 min** | **STALE** | **1** |
| M2 | umbral bajado a **1 min** (mismo fork stale) | STALE | 1 |
| M3 | par **fresco** real (dos RPCs independientes de mainnet) | FRESH | **0** |
| M4 | guarda de vacuidad: mismo endpoint para fork y cabeza | UNMEASURABLE `same_endpoint` | 2 |
| M5 | endpoints cruzados (fork=head real, head=anvil) ⇒ `head < fork` | UNMEASURABLE `head_below_fork` | 2 |
| M6 | fork **inalcanzable** | UNMEASURABLE `fork_rpc_no_answer` | 2 |
| M7 | M1 bajo `/bin/sh` (dash) — portabilidad POSIX | STALE | 1 |

**M1, salida literal (el caso obligatorio):**

```
FORK-FRESH-01 STALE: el fork esta a 1713 bloques (342.60 min = 5.71 h) de la cabeza, POR ENCIMA del umbral de 10 min.
FORK-FRESH-01 STALE: fork_block=26148216 head_block=26149929. Simular contra este estado seria medir el pasado, no el libro de ordenes actual: el llamador NO debe continuar.
FORK_FRESHNESS verdict=STALE reason=over_threshold fork_block=26148216 head_block=26149929 age_blocks=1713 age_minutes=342.60 threshold_minutes=10 seconds_per_block=12 fork_rpc=http://172.18.0.3:8545/<REDACTADO> head_rpc=https://eth.drpc.org/<REDACTADO>
EXIT=1
```

**M3 (salida, la rama verde existe y con datos reales):**

```
FORK-FRESH-01 OK: el fork esta a 0 bloques (0.00 min) de la cabeza, dentro del umbral de 10 min.
FORK_FRESHNESS verdict=FRESH reason=within_threshold fork_block=26149930 head_block=26149930 age_blocks=0 age_minutes=0.00 …
EXIT=0
```

**M4 (la trampa del adorno, cerrada):**

```
FORK-FRESH-01: fork-rpc y head-rpc apuntan al MISMO endpoint (eth.drpc.org) — comparar el fork consigo mismo daria FRESH siempre; se rechaza (exit 2).
FORK_FRESHNESS verdict=UNMEASURABLE reason=same_endpoint …
EXIT=2
```

---

## 3. ★★ LA EDAD, MEDIDA (antes y después)

### Antes (baseline t184, re-medido aquí, no heredado)

| lectura | valor | artefacto |
|---|---|---|
| fork block (3 lecturas idénticas) | `0x18efd78` = **26148216** | `eth_blockNumber` sobre `172.18.0.3:8545`, 3 veces |
| cabeza real | `0x18f041c` = **26149916** → después 26149929/26149931 | `https://eth.drpc.org` |
| edad en bloques | **1700 → 1713 → 1715** | resta |
| edad | **340,00 → 342,60 → 343,00 min** = 5,67 → 5,71 → 5,72 h | `age_blocks × 12 s` |
| `anvil StartedAt` | **2026-10-08T14:16:01.110230687Z** | `docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-anvil-1` |
| `sim-ctl StartedAt` | **2026-10-08T14:16:07.1195963Z** (6,01 s después, mismo deploy) | idem `arbitragex-v2-sim-ctl-1` |
| **cross-check** `StartedAt → now` | **5,694 h** | `python3` sobre los valores anteriores |
| **desvío bloques vs reloj** | **0,480 %** | `abs(age_h_bloques − age_h_reloj)/age_h_reloj` |

⇒ El fork **nació con el contenedor** (6 s de diferencia con `sim-ctl`) y **no volvió a leer**: la edad en
bloques y la edad del contenedor coinciden dentro del **0,48 %**, que es lo que se espera de un fork clavado
en el bloque que era cabeza al arrancar (los dos relojes, el de bloques y el de pared, avanzan juntos).

### Después: **sigue viejo, y el gate lo dice en ROJO**

El tramo de configuración que haría el fork fresco vive en `docker/` ⇒ **bloqueado** (§5). Medido en esta
sesión: la edad **creció de 340,00 a 343,00 min** mientras trabajaba (el fork no se movió ni un bloque; la
cabeza avanzó 15). **La edad que sigue midiéndose es 5,72 h** y el gate queda **armado y en rojo**:
`EXIT=1 verdict=STALE`. *Armado y en rojo es mejor que ausente* — y es exactamente lo que la tarea pedía para
este caso.

---

## 4. LA CAUSA, SIN ADORNOS, CON PATH Y LÍNEA

```
docker/compose.prod.yml:183:    - exec anvil --fork-url "$$ANVIL_FORK_URL" --host 0.0.0.0 --port 8545
docker/compose.dev.yml:126:      --fork-url ${ANVIL_FORK_URL}
docker/compose.dev.yml:127:      --fork-block-number ${ANVIL_FORK_BLOCK:-latest}
```

**Hallazgo que afina el enunciado**: el defecto **no** es "en el repo nadie pasa `--fork-block-number`". En
`compose.dev.yml:127` **sí** se pasa (con default `latest`) — o sea, la dirección correcta ya está escrita en
el repo. Lo que corre en producción es **`compose.prod.yml:183`**, que **no** lo pasa: y el comando del
contenedor vivo, leído con `docker inspect` (redactado, §8), es **verbatim**
`exec anvil --fork-url "$ANVIL_FORK_URL" --host 0.0.0.0 --port 8545`. ⇒ Confrontación directa entre el archivo
y el proceso: **el que manda es prod, y a prod le falta el flag.**

`--fork-url` **sin** `--fork-block-number` ⇒ anvil toma el bloque que es cabeza **en ese instante** y **no
vuelve a leer** (no hay re-fork periódico). Eso es lo que clava el fork y envejece con el reloj.

---

## 5. ★★ BLOQUEO DE RUTA: DECLARADO, CON SU CAUSA Y SU PORQUÉ (no se esquiva)

**El arreglo del fork vive en `docker/`.** `docker/` **no** está en el `inScope` de esta tarea
(`scripts/`, `docs/sre/`). El motivo declarado es que **t128 lo tiene tomado**, y el estado del equipo
(leído de `.agent-teams/arbx-publicacion-desbloqueo-02/team.json`) lo confirma y lo matiza:

| hecho de t128 | valor medido |
|---|---|
| `status` | **pending** |
| `attempt` | **0** (nunca reclamada) |
| `dependencies` | `[t124]` ⇒ **está esperando a t124**: por eso está deadlocked |
| `inScope` | `backend/sim-ctl/src/signer_funding.rs`, `backend/sim-ctl/src/sim_engine.rs`, `backend/sim-ctl/tests/`, **`docker/`**, `docs/backend/` |
| dependientes reales (campo `dependencies`) | **t130 (pending)**, **t134 (pending)**, t146 (**cancelled**) |

⇒ **Cancelar t128 envenenaría a t130 y t134** (ambas pending y ambas declarando `t128` en `dependencies`).
El bloqueo de ruta es **real**: el path `docker/` está tomado por una tarea activa con dependientes vivos.

**Y la corrección que hay que decir: t128 ya NO es el mismo sujeto.** Su propio texto lo declara —
*«El subject "FORK-TRUST-01" es legado y ya no describe la tarea; la premisa del fork está refutada»* — y su
objetivo actual es `balance_of` enviando 24 bytes en vez de 32 en `backend/sim-ctl/src/signer_funding.rs`
(un defecto de calldata del **signer funding**, no del fork). Su propio objetivo añade: *«NO toques la fuente
del fork, CANDIDATE_SLOTS, umbrales, gas, sizing ni el gate de paper»*.

⇒ **Lo que se reporta al OPERADOR, con esas palabras**: *una tarea pending deadlocked tiene bloqueada la ruta
de un arreglo cuyo sujeto ya no es el suyo*. El bloqueo de **scope** sigue en pie (no edito fuera de
`inScope`), pero el argumento de "es literalmente el mismo sujeto" **no se sostiene hoy** y conviene que el
operador lo sepa antes de decidir si reasigna `docker/`.

**No se toca `docker/` a escondidas.** El cambio va aquí, como patch y con su prueba.

---

## 6. EL PATCH PARA `docker/` (texto, **NO aplicado**) y las dos vías

### 6.1 La vía BARATA: declarar y fijar el bloque (hace el estado auditable, **no** lo hace fresco)

```diff
--- a/docker/compose.prod.yml
+++ b/docker/compose.prod.yml
@@ -183 +183 @@
-    - exec anvil --fork-url "$$ANVIL_FORK_URL" --host 0.0.0.0 --port 8545
+    - exec anvil --fork-url "$$ANVIL_FORK_URL" --fork-block-number "$$ANVIL_FORK_BLOCK" --host 0.0.0.0 --port 8545
```
más `ANVIL_FORK_BLOCK` provisto por el deploy (hoy `compose.dev.yml:127` usa `${ANVIL_FORK_BLOCK:-latest}`).

**Qué resuelve**: el bloque deja de ser implícito — se puede **auditar** contra qué bloque se simuló.
**Qué NO resuelve**: el fork sigue **clavado** (fijarlo a un número lo congela para siempre). Por eso el gate
sigue siendo necesario: la vía barata **no** alcanza el requisito de "a minutos de la cabeza".

### 6.2 La vía que SÍ da frescura: re-fork periódico, con su impacto declarado

Un supervisor que, cuando el gate devuelve `STALE`, re-forkee el anvil **sin reiniciar el contenedor**
(`anvil_reset` con `forking.jsonRpcUrl`+`blockNumber`) o lo reinicie en una ventana. **Impacto que hay que
declarar antes de activarlo**: un re-fork tira el estado local — **invalida cualquier medición en vuelo** y
cualquier `sim_*` que esté corriendo. Igual que un reinicio silencioso: **no se activa desde aquí.**

### 6.3 La vía CARA (la correcta por rango), nombrada aunque se descarte

**Fork por fila**: `--fork-block-number = opportunities.block_number`. t184 midió que a `FORK+10` la
re-cotización **desde el fork** reproduce la salida guardada del motor con error **`1,866e-10` (DIEZ DÍGITOS)**
mientras la de la **cabeza** está a **`2,190e-03`**; y que hay **cruce** (a `+10` gana el fork; a `+50/+200/+1000`
gana la cabeza) ⇒ **ningún instrumento único sirve para todo el rango**. Si se elige la vía barata se elige
**sabiendo** que **no** resuelve el rango completo: sirve para que el sistema **no mire el pasado**, no para
cotizar cada fila a su bloque.

---

## 7. ★ LO QUE ESTE ARREGLO **ES** Y LO QUE **NO ES** (escrito para que nadie lo lea como promesa)

- **ES**: un instrumento que **mide la edad del fork** y **falla** cuando el estado con el que se iba a
  simular ya no es el mercado actual. Sirve para que **el sistema no mire el pasado**.
- **NO ES**: un cambio del **NO económico**. t184 midió que **el fork es ESTRICTAMENTE MÁS PESIMISTA** que el
  instrumento correcto: contra el fork quedan **5** oportunidades positivas, contra la cabeza **18**. La
  dirección del error **excluye** que el bloque equivocado haya **fabricado** el NO.
- **NO ES**: una licencia para fabricar un edge que el libro de órdenes no tiene. Arreglar la frescura
  **no** crea oportunidades: cambia **contra qué mercado** se mide.

---

## 8. INSTRUMENTO: lo usado, lo que falló y lo que no se tocó

- **`172.18.0.3:8545` funciona desde el host del VPS**; **`localhost:8545` no** (declarado, no probado por mí
  más allá de usar la forma correcta). Todas las lecturas del fork salieron de `172.18.0.3`.
- **`:9090` externo** → `external_9090_http_code=000` (control negativo del defecto ya conocido del capitán).
  Para el host se usó **loopback vía `ssh arbx`**.
- **`cast` / `forge` NO existen en el VPS** (`command -v forge` → MISSING). El `Verify` del contrato pedía
  `cast block-number --rpc-url …`: **no ejecutable ahí**; la cabeza se leyó por **JSON-RPC
  `eth_blockNumber`** (mismo dato, misma fuente), y se declara la sustitución en vez de simular el comando.
- **Configuración efectiva, REDACTADA**: el `docker inspect` del anvil devuelve
  `["/bin/sh","-c"]|["exec anvil --fork-url \"$ANVIL_FORK_URL\" --host 0.0.0.0 --port 8545"]` — el nombre de
  la variable, **nunca su valor**. `--fork-block-number` = **0 ocurrencias**, `--fork-url` = **1**.
- **`trading_config` (no se toca, medido)**: chain 1 → `capital_usd=1000.00`, `min_profit_usd=50.0000`,
  `simulation_target_profit_usd=50.0000`. (El "multiplicador 3.0" **no** aparece en esas columnas: se reporta
  lo que la consulta devuelve y **no** se afirma nada sobre un campo que no se leyó.)
- **Control de canal Postgres**: `SELECT 1` → `exit=0`; `SELECT esto_no_existe` → **`exit=1`** + `ERROR:
  column "esto_no_existe" does not exist` (sin tubería, como exige el contrato).
- **El fork no se tocó y no se escribió en él**: el gate **sólo lee**; ninguna llamada mutante
  (`anvil_*` de escritura, `evm_mine`, `anvil_reset`) se ejecutó. Tampoco se reinició ningún contenedor.
- **Ningún control se pipeó a `head`.** Los outputs de los mutantes se capturaron completos.
- **Archivo scratch declarado**: el gate se subió a `/tmp/fork_freshness_gate.sh` del VPS para ejecutarlo,
  con **sha256 verificado idéntico** al local (`b171fc7e…`, 150 líneas) antes de correrlo, y **se borró** al
  terminar.

---

## 9. AJUSTES Y DEFECTOS PROPIOS, CAZADOS Y DECLARADOS

1. **`$args` como parámetro de función en PowerShell** = nombre de una **variable automática** ⇒ la línea
   remota salió mangled y se imprimió el base64 en vez de ejecutar el gate. Corregido subiendo el artefacto
   por `scp` y verificando hash. (El primer intento, con un script remoto anidado, falló por lo mismo.)
2. **Campo de dependencias mal leído**: consulté `dependsOn` cuando el campo real es **`dependencies`** ⇒
   mi primera conclusión ("t128 no tiene dependientes") era **falsa**. Re-consultado con el campo correcto:
   **tiene 3** (t130, t134 pending; t146 cancelled). Se declara el error y el dato corregido.
3. **`report()` llamaba a `redact()` antes de definirla** (en `sh` las funciones no se hoistean) ⇒ corregido
   moviendo las definiciones antes de su primer uso, y **verificado ejecutando el gate** con `bash` y con `sh`.

---

## 10. LÍMITES DECLARADOS (lo que NO se logró y por qué)

- **La edad "después" NO quedó en minutos**: quedó en **5,72 h y creciendo**, porque el arreglo de
  configuración vive en `docker/` (bloqueado, §5). El gate está **armado y en rojo**, que es lo máximo
  honesto alcanzable desde `scripts/` + `docs/sre/`.
- **La conexión del gate al camino de simulación** (que el sistema *no* simule cuando el gate falla) es
  **patch**, no hecho: requiere `docker/` (healthcheck/entrypoint del anvil) o el preflight del simulador.
- **No se midió** el efecto de un re-fork sobre mediciones en vuelo (no se ejecutó ninguno: sería una
  escritura en el fork, prohibida aquí).
- **No se tocó** `capital_usd`, target, multiplicador, la fuente del fork, `CANDIDATE_SLOTS`, gas, sizing ni
  el gate de paper.
