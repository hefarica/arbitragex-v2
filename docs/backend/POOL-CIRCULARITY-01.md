# POOL-CIRCULARITY-01 — C2 implementado y verificado; C1 NO es "falta quien la llame"

**Orden:** t80 · **Perfil:** Backend · **Intento:** 1 · `49ae2834-755d-443d-9c6e-1c2baa709722`
**Base:** `origin/main` = `bceb31ef` (congelamiento vigente; el remoto pasó de `2d22ce7` a `bceb31e` durante el trabajo) · **Modo:** paper. Sin firma, sin broadcast, sin capital.
**Canal:** `C:\Program Files\Git\usr\bin\ssh.exe` (R77). **Control `SELECT 1` → `1` pasado antes de cada cero reportado.**
**NO SE EMPUJÓ NADA.** La orden lo prohíbe explícitamente (`ethics-guard.yml` no filtra rama: cualquier push emite un run) y todo el trabajo es local en clon aislado.

---

## 0. VEREDICTO CORTO

| | |
|---|---|
| **C2** (barrido de `checkToken` desacoplado) | **IMPLEMENTADO y VERIFICADO** localmente; **sin cablear** (`index.ts` está fuera de alcance) |
| **C1** (consumidor de `observed_unindexed_pairs`) | **NO IMPLEMENTADO — y la premisa de la orden no se sostiene** (ver §3) |
| Mediciones "después" (aceptación 3/4/5) | **NO COMPUTADAS** — requieren deploy, y el deploy y el push están prohibidos por la orden |

---

## 1. LAS DOS COMPROBACIONES "ANTES" — ambas confirmadas por barrido del árbol

**A) `observed_unindexed_pairs` tiene CERO lectores.** Barrido de todo el árbol buscando el literal:

| aparición | archivo:línea |
|---|---|
| comentario de doc | `pool_discovery.rs:6` |
| **escritor** (`INSERT … ON CONFLICT`) | **`pool_discovery.rs:409-414`** |
| migración (CREATE TABLE + índices) | `migrations/059_pool_discovery_tables.sql:9,25,29` |

**Ningún `SELECT`.** Confirmado: **sólo el escritor.**

**B) `checkToken` tiene EXACTAMENTE dos call sites, ambos en el paso de candidato:**

```
consumer.ts:411  const safetyIn  = await checkToken(this.deps.pool, ..., opp.chain_id, opp.token_in);
consumer.ts:412  const safetyOut = await checkToken(this.deps.pool, ..., opp.chain_id_out ?? opp.chain_id, opp.token_out);
```

Todo lo demás son la definición (`client.ts:24`), el `import` (`consumer.ts:21`) y comentarios/tests. **Ninguna producción de veredictos independiente de un candidato.** Confirmado.

## 2. LÍNEA BASE MEDIDA (toda con control `SELECT 1` → `1`)

| medición | valor ANTES |
|---|---|
| `token_safety_cache` total / **frescos** | **1732 / 0** |
| `pools` activos / total | **1197 / 3935** |
| pares con ≥2 pools | **223** |
| `observed_unindexed_pairs` | **7494** |
| `entries-added` de `arbx:opps:validated` | **12247832** |
| `simulations` | **0** |
| `ARBX_POOL_ENUM_MODE` (contenedor vivo) | **shadow** |

## 3. C1 — LA PREMISA DE LA ORDEN NO SE SOSTIENE (medido)

La orden dice: *"La función YA EXISTE … **Falta únicamente quien la llame**."*

**Medí el contenido real de la tabla y no alcanza con un llamador:**

```
total 7494 | is_resolved 775 | resolved_pool_addr NOT NULL 775 | SIN resolver 6719
```

- De los **775** con `resolved_pool_addr`, **746 YA están en `pools`** → un consumidor que sólo llame a `enumerate_and_persist_pool` alcanzaría **29 filas útiles**.
- Los **6719 restantes (89,7%)** tienen `resolved_pool_addr IS NULL`: **no hay dirección de pool que pasarle a `enumerate_and_persist_pool`**. Su firma es `enumerate_and_persist_pool(pool_addr, token0_hint, token1_hint, fee_bps, activate)` (`pool_discovery.rs:1063-1070`) — **exige un `pool_addr` que esas filas no tienen.**

⇒ **Falta una PATA DE RESOLUCIÓN**, no un llamador: hay que derivar la dirección del pool desde el par de tokens contra las factories sembradas (`IUniswapV2Factory::getPair` `pool_discovery.rs:229`, `IUniswapV3Factory::getPool` `:274`), persistir `resolved_pool_addr` + `is_resolved`, y **recién entonces** hidratar. Esa pata no existe en el árbol.

**No inventé esa pata a medias.** Escribir Rust sin poder compilarlo lo dejaría como una hipótesis con formato de parche — y con `main` congelado eso es peor que no tenerlo. Se declara el hallazgo con su número en vez de entregar código no verificado.

## 4. C2 — IMPLEMENTADO Y VERIFICADO

**Archivo nuevo, dentro de alcance:** `backend/selector-api/src/token_safety/sweep.ts`

- `listCatalogTokens(pool, limit)` — catálogo real: `tokens` referenciados por `pools` (`token0_id`/`token1_id` son FK uuid a `tokens.id`). **Medido en el VPS: 3.453 tokens distintos.**
- `sweepCatalogTokens(deps, limit=500)` — una pasada; llama a **`checkToken`** por token. Un token que falla no aborta la corrida (fail-honest: se cuenta, se sigue).
- `startTokenSafetySweep(deps)` — barrido periódico. **Intervalo: `ARBX_TOKEN_SAFETY_SWEEP_SECS`, por defecto 900 s (15 min)**; `0` = desactivado (kill-switch de operador sin tocar código). No solapa corridas.

**Verificación ejecutada** (el módulo carga y exporta; sin el árbol de deps completo, `tsc` no corre — se declara):

```
EXPORTS: listCatalogTokens, startTokenSafetySweep, sweepCatalogTokens
startTokenSafetySweep es funcion: true
sweepCatalogTokens es funcion  : true
listCatalogTokens es funcion   : true
SQL usa tokens+pools (catalogo real): true
NO contiene INSERT/UPDATE propio   : true
```

### EL GATE NO SE SALTEA — y está probado por el propio diff

`sweep.ts` **no contiene ni un `INSERT` ni un `UPDATE`** (verificado arriba, por regex sobre el propio fuente). La **única** escritura la hace `checkToken` → `upsertCached`, dentro del camino real: canónico → cache → proveedor GoPlus bajo CircuitBreaker → heurística interna. **`TOKEN_SAFETY_FLOOR` y `min_acceptable_score` intactos; cero filas fabricadas.** El barrido **LLAMA** al gate.

### LÍMITE DECLARADO DE C2: no está cableado

El contrato acota el alcance a `backend/selector-api/src/token_safety/`. Arrancar el barrido requiere **una línea en `backend/selector-api/src/index.ts`** (`startTokenSafetySweep({...})` en el arranque), y **`index.ts` está FUERA de alcance**. El barrido existe, es correcto y es verificable, pero **hoy no se ejecuta solo**. Se declara en vez de tocar un archivo fuera de alcance.

## 5. LO QUE NO PUDE MEDIR (aceptación 3/4/5) — bloqueado, no fingido

Las tres mediciones "después" exigen que C2 **corra en el sistema vivo**, y eso requiere **deploy**. La orden prohíbe empujar ("NO empujar sin orden explicita") y el objetivo del equipo es "sin deploy". **No hice workarounds sobre producción.** Baseline ya tomado en la MISMA ventana para cerrarlas en cuanto se despliegue: cache **1732/0 frescos**, pools **1197/3935**, ≥2 pools **223**, `entries-added` **12247832**, `simulations` **0**.

## 6. DECLARACIONES OBLIGATORIAS

**Reactivación (medida, no inferida):** `ARBX_POOL_ENUM_MODE=**shadow**` en el contenedor vivo. La reactivación automática **está encendida y alcanza pools CONOCIDOS** — pero **no toca los pares observados-sin-indexar**: de los **7494** de `observed_unindexed_pairs`, **6719 (89,7%) no tienen `resolved_pool_addr`**, y **ninguno** de esos tiene fila en `pools`. Encender el worker no era el arreglo: ya estaba encendido.

**R9 (ventana de logs):** la ventana del contenedor **rota**. Data midió que cubre **17:55:38 → 18:06:36** de un arranque **14:53:51Z**. **No concluyo ausencia de ningún evento** desde `docker logs`: cualquier "no aparece X" queda acotado a esa ventana retenida, no al histórico.

**No se toca `is_active` por SQL a mano, y NO se baja el umbral de spread ni el de net-profit.** Con esas palabras: no se ejecutó ningún `UPDATE pools SET is_active`, y este trabajo no lee ni modifica umbrales económicos.

**No se firma, no se emite, no se toca capital.** `ARBX_TRADE_MODE` y `ARBX_LIVE_EXEC_ENABLED` **intactos** (no se leyeron ni escribieron). **NO se mergea y NO se toca `main`** (sigue en `bceb31ef`). **NO se empujó ninguna rama.**

**Alcance:** único path nuevo, `backend/selector-api/src/token_safety/sweep.ts` (dentro de `backend/selector-api/src/token_safety/`). **Cero cambios en `pool_discovery.rs`** (C1 no implementado), cero cambios fuera de alcance.

---

*Lo medido está medido: las dos comprobaciones "antes" confirmadas por barrido, la línea base completa con control, y la premisa de C1 refutada con 6719/7494. Lo no hecho se declara como no hecho: C1 necesita una pata de resolución que no existe, C2 no está cableado por límite de alcance, y las mediciones "después" requieren un deploy que esta orden prohíbe.*
