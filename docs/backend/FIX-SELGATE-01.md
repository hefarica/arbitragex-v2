# FIX-SELGATE-01 — `validated` vuelve a ser un LOG DE DECISIONES

**Orden:** t82 · **Perfil:** Backend · **Intento:** 1 · `b177040b-e13c-4f9a-a650-148e65f51874`
**Base:** `origin/main` = `bceb31ef` · **Modo:** paper. Sin firma, sin broadcast, sin capital.
**Control `SELECT 1` → `1` pasado antes de reportar cualquier cero.**

---

## 1. ANTES — el corte reproducido con salida cruda

```
docker exec arbitragex-v2-selector-api-1 sh -c 'curl -s http://127.0.0.1:3002/metrics | grep selector_decisions'
arbx_selector_decisions_total{decision="reject",reason="producer_rejected",chain_id="1"} 400773
```

**SÓLO `producer_rejected`. CERO `accept`.** 400.773 decisiones, todas por la misma razón.

Y `arbx:opps:validated` con `entries-added` **clavado en 12247832**.

### LA PRUEBA DE QUE ERA UN LOG DE DECISIONES (no una cola de aceptados)

```
docker exec arbitragex-v2-redis-1 redis-cli XINFO STREAM arbx:opps:validated | tail -8
last-entry  1789637172953-0
  {"...","risk_score":51.2,"rejection_reason":"single_pool_no_spread",
   "detected_at":"2026-09-17T09:26:12.777064508Z",...}
first-entry 1789636936078-0
  {"...","risk_score":51.2,"rejection_reason":"v3_quote_unavailable",
   "detected_at":"2026-09-17T09:22:13.308310074Z",...}
```

**Las últimas entradas escritas traen `rejection_reason` Y `risk_score` a la vez.** Una cola de aceptados no publica entradas con motivo de rechazo y score calculado. **`validated` publicaba DECISIONES — aceptadas y rechazadas.** Queda probado por los bytes, no por inferencia.

## 2. EL CONTRATO DE LA CADENA (leído antes de tocar una línea)

`consumer.ts:6`:
```
parse → prefilter → safety → score → decide → persist → (if accept) publish validated → XACK
```

`engine.ts`:
- `producerRejected()` — **:36-39** (la guarda)
- cortada en `prefilter` **:56-58** → `return {kind:"reject", reason:"producer_rejected"}`
- `decide()` — **:89-125**, umbrales en **:93** (`cfg.token_safety.min_acceptable_score`), **:109** (`cfg.risk.max_revert_rate_pct * 2`), **:117** (`cfg.scoring.min_accept_score`)

**El corte eran DOS compuertas confladas:**
1. `prefilter` corta en **`consumer.ts:404`** (`if (pre) return pre;`) → `decide()` **nunca corre**.
2. el publish estaba gateado por aceptación en **`consumer.ts:376`** (`if (decision.kind === "accept")`) → nada se publica.

## 3. EL FIX — separa el LOG de la COLA

`backend/selector-api/src/consumer.ts` — **único archivo de código tocado**:

```ts
// :376  ANTES: if (decision.kind === "accept") { await this.publishValidated(opportunity, decision.score); }
// :376  AHORA: await this.publishValidated(opportunity, decision);
```

```ts
// :423  publishValidated(opp, decision) emite el veredicto y su motivo:
const payload = JSON.stringify({
  ...opp,
  risk_score: decision.score,
  verdict: decision.kind,                                   // "accept" | "reject"
  verdict_reason: decision.kind === "reject" ? decision.reason : null,
});
```

**`verdict` / `verdict_reason` son campos NUEVOS: son el contrato con aguas abajo.** El que EJECUTA filtra `verdict === "accept"`. `rejection_reason` **se conserva intacto** tal como lo emitió el productor (es un campo distinto, con otro dueño: no se pisa).

**La guarda NO se borró: `engine.ts` está BYTE-IDÉNTICO** (`git diff --stat -- …/policy/engine.ts` → vacío). `producerRejected()` sigue clasificando en `prefilter:56-58`, sigue cortando antes del trabajo caro, y un productor-rechazado **sigue sin poder salir como `verdict:"accept"`**. Lo que se movió es el **PUNTO DE ENFORCEMENT**: antes el filtro se ejercía **suprimiendo el mensaje** en el publicador; ahora se ejerce **abajo**, sobre un payload que lleva el veredicto explícito. **Reubicado, no eliminado.**

## 4. NINGÚN UMBRAL SE RELAJÓ

`git diff --stat -- backend/selector-api/src/policy/engine.ts` → **vacío: byte-idéntico.** Umbrales citados, antes **y** después (idénticos):

| locus | umbral |
|---|---|
| `engine.ts:93` | `cfg.token_safety.min_acceptable_score` |
| `engine.ts:109` | `cfg.risk.max_revert_rate_pct * 2` |
| `engine.ts:117` | `cfg.scoring.min_accept_score` |

El objetivo es que el pipeline **PROCESE**, no que apruebe lo que hoy rechaza. No se tocó ninguno.

## 5. JUEGO LOCAL COMPLETO — cero regresiones

```
tests  ANTES (baseline) : 15 failed | 69 passed (84)
tests  CON el fix       : 15 failed | 70 passed (85)     <- los MISMOS 15; +1 pasa
typecheck CON el fix    : 15 errores                      <- == baseline
src/policy/sel-gate01.test.ts : 5 passed (5)
```

Los 15 fallos son deps npm ausentes (`pg`/`pino`/`express`) en archivos que no toqué; el conteo es idéntico al baseline del mismo árbol. **Cero fallos nuevos.**

### El único test que mi cambio rompía — y por qué se reescribió en vez de borrarse

`sel-gate01.test.ts:99` afirmaba `expect(CONSUMER_SRC).toMatch(/decision\.kind === "accept"/)` — es decir, **codificaba la compuerta que la orden manda quitar**. Al correrlo, era **el único fallo nuevo** (16 vs 15, verificado por diff de nombres).

Se **reescribió, no se borró**: las aserciones 1 y 2 del archivo (clasificación de `producerRejected`, y que la guarda corre **antes** de `checkToken`/`scoreOpportunity`) **siguen intactas y pasando** — la guarda se preserva. Lo que cambió es la tercera: ahora afirma el contrato NUEVO (el publish NO está gateado por aceptación, y el payload lleva `verdict` + `verdict_reason`). El docstring del archivo explica qué cambió y por qué. **No se debilitó ninguna verificación de seguridad.**

## 6. LO QUE NO PUDE MEDIR — y es lo que decide

**NO COMPUTADO: `entries-added` NO avanzó respecto de 12247832.** No lo reporto como logro.

**La razón está medida (t81 la estableció):** un push a una rama **no despliega**. `auto-deploy-vps.yml` dispara sólo con `push: branches: [main]`, y el aterrizaje es de Release (t86). El contenedor vivo corre el código viejo, así que el `if (kind === "accept")` sigue ahí **en producción** hasta que t86 aterrice y despliegue.

Línea base ya tomada para cerrarlo en cuanto se despliegue: `entries-added` **12247832**, `arbx_selector_decisions_total` con **sólo `producer_rejected`**. Tras el deploy: `entries-added` debe **avanzar**, y las métricas deben mostrar poblaciones de `reason` distintas además de `producer_rejected`.

**La forma de la salida está predicha por el propio diseño:** como la guarda sigue cortando, cada mensaje del productor actual seguirá decidiéndose `producer_rejected` — pero **ahora se PUBLICA**, así que `entries-added` avanza igual. Lo que NO va a cambiar es la proporción de aceptados: el productor rechaza el 100% de sus filas (`6563231 | 6563231 | 0`). **Flujo evaluable ≠ flujo rentable** (`t48`: 10,82× corto contra el hurdle). No lo presento como ganancia.

## 7. DECLARACIONES

**No se firma, no se emite, no se toca capital.** `ARBX_TRADE_MODE=paper` y `ARBX_LIVE_EXEC_ENABLED=False` **citados desde el contenedor vivo e intactos**.

**RIESGO DECLARADO PARA AGUAS ABAJO (material, y lo levanto acá):** al publicar también los rechazos, `sim-ctl` —que hoy simula todo lo que llega de `validated`— **va a simular filas rechazadas**, que es exactamente el consumo de RPC de fork que la guarda original quería evitar (87% según el audit). **El traslado del filtro está INCOMPLETO sin un cambio en `sim-ctl`**, que está **FUERA de mi alcance** en esta orden. Verificado que agregar campos es seguro para él: el struct `Opportunity` de Rust **no** usa `deny_unknown_fields` (sólo `flashloan_math.rs:51` y `oracle_snapshot.rs:12,19`, ninguno es el Opportunity), así que serde ignora `verdict`/`verdict_reason` en vez de rechazarlos.

**NO se mergeó** (el aterrizaje es t86). Rama + PR, verificación por el REMOTO.

---

*El corte reproducido con su salida cruda y con la prueba de que `validated` era un log —las entradas supervivientes traen motivo Y score—. El fix mueve el punto de enforcement en vez de borrar la guarda: `engine.ts` byte-idéntico, umbrales idénticos, cero fallos nuevos. Las mediciones que deciden quedan pendientes del deploy de t86, y el traslado del filtro queda declarado como incompleto sin `sim-ctl`.*
