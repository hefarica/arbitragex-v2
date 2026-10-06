# POOL-CIRCULARITY-02 — C2 cableado y empujado; el barrido NO corre porque no está desplegado

**Orden:** t81 · **Perfil:** Backend · **Intento:** 1 · `fff98d77-a14f-44ca-99e5-9504ec4882f9`
**Base:** `origin/main` = `bceb31ef7c0fd6d4b50bcdee6fc8d33858334822` (no se movió) · **Modo:** paper. Sin firma, sin broadcast, sin capital.
**Control `SELECT 1` → `1` pasado antes de CADA cero reportado.**

---

## 1. C2 CABLEADO — la línea

`backend/selector-api/src/index.ts` (+17 líneas, único path tocado):

```ts
import { startTokenSafetySweep } from "./token_safety/sweep.js";       // L31

const stopTokenSafetySweep = startTokenSafetySweep({                    // arranque
  pool, cb: mustCb("token_safety_api"), cfg, logger,
});                                                                     // tras consumer.startWithRetry()

stopTokenSafetySweep();                                                 // en shutdown()
```

**Contexto:** va justo después de `consumer.startWithRetry()` y usa `mustCb("token_safety_api")`, el MISMO CircuitBreaker que el consumidor. En `shutdown()` se detiene antes de cerrar pool/redis. Es la línea que hace que el barrido corra solo.

**Intervalo real:** `ARBX_TOKEN_SAFETY_SWEEP_SECS`, **defecto 900 s (15 min)**; `0` = desactivado. **Valor efectivo en el contenedor: NO DEFINIDO** → si el código estuviera desplegado, aplicaría el defecto de 900 s.

## 2. JUEGO LOCAL COMPLETO (ordenado antes de empujar) — sin regresión

```
typecheck SIN mi cambio : 15 errores
typecheck CON mi cambio : 15 errores     <- IDENTICO
tests (misma corrida)   : 15 failed | 69 passed   <- == baseline de t73
```

Los 15 errores son `Cannot find module 'pg'|'pino'|'express'` — **deps npm ausentes en este árbol, en archivos que no toqué**; el baseline sin mi cambio da exactamente los mismos 15. **Cero regresión introducida.** (En CI, con el árbol de deps completo, `tsc` y `vitest` sí corren.)

## 3. PUSH AUTORIZADO — verificado por el REMOTO

```
git ls-remote origin refs/heads/feat/pool-circularity-01
  261b8566930f40c091094e97147639f90e7958c7

git ls-remote origin refs/heads/main
  bceb31ef7c0fd6d4b50bcdee6fc8d33858334822   <- ANTES y DESPUÉS: no se movió
```

Commits: `50165a7f` (sweep.ts + doc t80) y **`261b8566`** (el cableado). **Exactamente 1 run de CI** emitido, como se declaró: **`37510095702` — `ethics-guard` (`push`)**, in_progress. **NO se mergeó. `main` intacto.**

## 4. LO QUE DECIDE TODO: **NO SE MOVIÓ. La circularidad NO se rompió todavía.**

| medición | ANTES | DESPUÉS | ¿cambió? |
|---|---|---|---|
| `token_safety_cache` frescos (`ttl_expires_at > NOW()`) | **0** | **0** | **NO** |
| `token_safety_cache` total | 1732 | 1732 | NO |
| `pools` activos / total | 1197 / 3935 | **1197 / 3935** | **NO** |
| `simulations` | 0 | **0** | **NO** |
| `entries-added` de `arbx:opps:validated` | 12247832 | **12247832** | **NO** |

**Con esas palabras: el barrido no descongeló la cache, el universo no reaccionó, y `arbx:opps:validated` NO recibió su primera entrada desde 2026-09-17T09:26:12Z.** El 0 se declara como 0, no como éxito por haber corrido el código.

### LA RAZÓN, medida y no supuesta

```
docker exec arbitragex-v2-selector-api-1 grep -rl 'token_safety.sweep' /app/dist
  -> NO ENCONTRADO

docker inspect arbitragex-v2-selector-api-1 --format '{{.State.StartedAt}}'
  -> 2026-10-06T14:53:51.729970792Z          (ANTERIOR a mi push)
```

**El barrido NO está desplegado.** Un push a una rama **no reconstruye el contenedor**: `auto-deploy-vps.yml` dispara sólo con `push: branches: [main]` (verificado en t66 leyendo el YAML), y esta orden prohíbe tocar `main`. **El contenedor vivo corrió desde antes de mi push y su `/app/dist` no contiene el string del barrido.**

⇒ **Falta un DEPLOY, no código.** La línea está, está empujada, está verificada por el remoto y no introduce regresión. Lo único que falta es que la imagen se reconstruya y el contenedor se recree.

## 5. DECLARACIONES

**El gate NO se saltea:** `TOKEN_SAFETY_FLOOR` sin tocar; **cero filas fabricadas**; el único camino de escritura sigue siendo `checkToken`→`upsertCached` (el módulo `sweep.ts` no contiene ni un `INSERT` ni un `UPDATE`, verificado por regex en t80). El cableado sólo **arranca** ese barrido.

**No se firma, no se emite, no se toca capital.** `ARBX_TRADE_MODE` y `ARBX_LIVE_EXEC_ENABLED` **intactos** (no leídos ni escritos).

**Riesgo de la orden a tener presente:** recuperar cobertura da flujo **EVALUABLE, no rentable** (`t48`: **10,82×** corto contra el hurdle). Cuando el barrido corra y la cache descongele, el motor puede **volver a rechazar** esas oportunidades por economía. Eso será una respuesta correcta, no un fallo del barrido.

---

*Cableado hecho, juego local corrido sin regresión, push verificado por el remoto y `main` intacto. Las tres mediciones que deciden —cache descongelada, universo reactivado, `validated` con entrada nueva— dan 0, y el 0 viene con su causa medida: el contenedor vivo no contiene el barrido. Un deploy lo cierra.*
