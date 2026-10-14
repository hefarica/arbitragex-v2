# SELECTOR-PARSE-01 — la causa es un `.strict()` de zod que tira el 100% del stream

**Orden:** t73 · **Perfil:** Backend · **Intento:** 1 · `2a02c7ce-17ba-4623-9e7f-b7c5ff6f9c1c`
**Base:** `origin/main` = `2d22ce7083e940e93f7818c51247cf68e69e9b47` · **Modo:** paper. Sin firma, sin broadcast, sin capital.
**Canal:** `C:\Program Files\Git\usr\bin\ssh.exe` (Git-for-Windows) — **el de Windows (`System32\OpenSSH`) da exit 255; éste funciona.** Control `SELECT 1` → `1` en todas las corridas.

---

## 1. EL PAYLOAD REAL (crudo, sin editar)

```
$ ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli XREVRANGE arbx:opps:detected + - COUNT 1"
1791290286529-0
json
{"id":"0dcd67a1-2982-43de-a64a-5f6e2f9adf80","chain_id":1,"strategy_kind":"triangular",
 "dex_a":"uniswap-v2","dex_b":"uniswap-v3","pair_symbol":"698250(4-hop cycle)",
 "token_in":"0x6982508145454ce325ddbe47a25d4ec3d2311933",
 "token_out":"0x6982508145454ce325ddbe47a25d4ec3d2311933",
 "amount_in_wei":"0","expected_profit_usd":null,"net_expected_profit_usd":null,
 "roi_pct":null,"risk_score":null,"block_number":26133382,
 "rejection_reason":"v3_quote_unavailable","cartridge_id":null,
 "detector_id":"hop_cycle_bridge","pipeline_latency_ms":945,
 "economics":{"computation_status":"error","error_reason":"v3_quote_unavailable",
   "amount_in_wei":null,"amount_out_wei":null,"amount_in_usd":null,"amount_out_usd":null,
   "gross_profit_usd":null,"gas_usd":null,"dex_fees_usd":null,"flash_fee_usd":null,
   "bribe_usd":null,"slippage_usd":null,"other_costs_usd":null,"total_cost_usd":null,
   "net_profit_usd":null,"roi_pct":null,"target_net_usd":null,"target_delta_usd":null,
   "meets_target":null,"quote_block":26133382,"simulation_block":null,"legs":[],
   "not_computed_reasons":{}},
 "detected_at":"2026-10-06T12:38:05.578591982Z","trace_id":"30a11ea8-8a09-42e4-883f-5d57ba8b39c1"}
```

## 2. EL ERROR EXACTO (no el conteo)

```
$ ssh arbx "docker logs arbitragex-v2-selector-api-1 --since 20m 2>&1 | grep consumer.invalid_message | tail -1"
{"level":"warn","time":"2026-10-06T12:38:18.429Z","service":"selector-api",
 "event":"consumer.invalid_message","id":"1791290298428-0",
 "err":"[{\"code\":\"unrecognized_keys\",\"keys\":[\"economics\"],\"path\":[],
         \"message\":\"Unrecognized key(s) in object: 'economics'\"}]"}
```

## 3. LA CAUSA EXACTA — lado a lado

| | |
|---|---|
| **lo que `selector-api` espera** | `shared-ts/src/contracts/index.ts:15` `export const OpportunitySchema = z.object({ … })` … **`:66` `.strict();`** |
| **lo que el searcher publica** | el mismo objeto **+ una clave de nivel superior no declarada: `economics`** (el desglose de 7 costes) |

**`consumer.ts:329`** `opportunity = OpportunitySchema.parse(JSON.parse(json));`

Con `.strict()`, zod rechaza **cualquier** clave desconocida → `unrecognized_keys` → **`consumer.ts:330`** la captura → **`:331` `consumer.invalid_message`** → **`:334` `XACK`** → **el mensaje se pierde para siempre**.

**Es la TERCERA vez que este defecto exacto ocurre**, y el propio archivo lo documenta dos veces: `:41-47` (`cartridge_id`) y `:49-54` (`detector_id`) — *"MUST be declared in the strict schema: the Rust producer always serializes this key … omitting it here makes OpportunitySchema.parse reject every published Opportunity"*. El productor Rust añade claves **unilateralmente**; el schema TS va un paso por detrás y el precio es el **100% del stream**.

## 4. EL PARSER REAL CONTRA EL PAYLOAD REAL

No una reimplementación: `SelectorOpportunitySchema` es el const realmente exportado por `consumer.ts`, y el payload es el JSON completo de §1.

```
$ npx tsx --tsconfig tsconfig.repro.json repro-real.ts
ANTES (OpportunitySchema, .strict()): RECHAZA
    unrecognized_keys ["economics"]
DESPUES (SelectorOpportunitySchema, el EXPORTADO REAL): PASA
CONTROL NEGATIVO (clave basura): RECHAZA (bien: sigue estricto)
```

**Y a escala, sobre 120 payloads REALES del stream vivo** (capturados read-only con `XRANGE`, sin consumir). Se regenera con:

```bash
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli --json XRANGE arbx:opps:detected - + COUNT 120" > live-payloads.json
```

(`live-payloads.json` es dato crudo de producción: **no se commitea**, se regenera con esa línea. El repro de un payload de `repro-selector-parse.ts` es autocontenido y no lo necesita.)

```
$ npx tsx --tsconfig tsconfig.repro.json repro-batch.ts
payloads reales parseados: 120
ANTES  (OpportunitySchema .strict())          : 0 / 120 PASAN
DESPUES(SelectorOpportunitySchema, el real)   : 120 / 120 PASAN
fallos residuales del fix: (ninguno)
```

**0/120 → 120/120.** El `0/120` reproduce en laboratorio, con payloads reales, el mismo 100% de pérdida que el `2044 invalid_message / 20 min` de producción.

## 5. EL FIX

`backend/selector-api/src/consumer.ts` — un schema local que **AÑADE la clave**, sin dejar de ser estricto:

```ts
export const SelectorOpportunitySchema = OpportunitySchema.extend({
  economics: z.object({}).passthrough().nullish(),
});
```

y `:329` pasa a usarlo. Por qué así:

1. **Se mantiene ESTRICTO.** `.extend()` conserva la política `strict` — lo prueba el **control negativo** de §4: una clave basura (`totally_unknown_key`) **sigue siendo rechazada**. Esto **no** degenera en "aceptar cualquier cosa".
2. **No se relaja ningún gate.** `economics` **no lo consume ninguna decisión**: `prefilter`, `safety`, `score`, `decide` y `persist` leen sólo los campos ya declarados. Aceptarlo **no cambia umbrales, ni el gate de viabilidad, ni el veredicto económico**. Sólo evita que el parser tire el mensaje entero por un campo que no usa.
3. **El fix canónico pertenece a `shared-ts/`** (declarar la clave en el schema compartido, como se hizo con `cartridge_id` y `detector_id`). Está **FUERA DE ALCANCE** (`backend/selector-api/`, `docs/backend/`), así que la corrección se aplica en el consumidor y **se declara la deuda**: mientras el productor siga añadiendo claves, `shared-ts` debería declararlas para no repetir esta cuarta vez.

## 6. NO ROMPE NADA — baseline comparado (R11)

`vitest` en este árbol da `3 failed | 6 passed`, `15 failed | 69 passed`. **NO es mi cambio**: los fallos son deps npm ausentes (`pg`, `pino`, `express`, `smol-toml`) en archivos que **no toqué**. Probado con baseline:

```
BASELINE (sin mi fix): Test Files 3 failed | 6 passed | Tests 15 failed | 69 passed
CON mi fix           : Test Files 3 failed | 6 passed | Tests 15 failed | 69 passed
```

**Idéntico.** Los 15 fallos son **preexistentes y de entorno**, no míos. El typecheck falla sólo por `Cannot find module 'pg'|'pino'|'express'`.

## 7. ESTADO MEDIDO DEL SISTEMA (ANTES, con control vivo)

```
CONTROL   SELECT 1                              -> 1                     [canal VIVO]
invalid_message ventana 3m                      -> 303                   [BEFORE, mi baseline]
simulations            count(*)                 -> 0
simulations            n_tup_ins / upd / del    -> 0 / 0 / 0             [DE POR VIDA]
opportunities          n_tup_ins               -> 13619205
executions / paper_trade_runs  n_tup_ins        -> 0 / 0
stats_reset (arbitragex)                        -> (nunca)  => contadores de por vida
XLEN arbx:opps:validated                        -> 10001
sim-ctl-g0  last-delivered-id                   -> 1789637172953-0  (ANTIGUO)  lag 0  pending 0
```

### ALCANCE DEL DEFECTO — medido por mí

Grupos sobre **`arbx:opps:detected`**:

| grupo | consumers | pending | entries-read | **lag** |
|---|---|---|---|---|
| `enricher` | 1 | 65 | 84.161.771 | **215.189** |
| `paper-archiver-g0` | 209 | 329 | 83.684.266 | **692.694** |
| **`selector-g0`** | 1 | 0 | **83.504.387** | **872.573** |

**`selector-g0` está 872.573 entradas por detrás** y con `pending 0`: lee, descarta el 100% como inválido, ACKea, y **no alcanza el caudal** del searcher. Los `n_tup_ins` de por vida (13,6M oportunidades contra **0 simulaciones**) acotan el impacto histórico: **13.619.205 oportunidades detectadas, 0 simuladas**.

> El capitán citó `enricher` `entries-read` 84.160.965 / `lag` 215.171; yo medí **84.161.771 / 215.189** — avanzó entre las dos lecturas, consistente con un stream vivo.

## 8. LO QUE **NO** PUDE MEDIR — y por qué (blocker)

Las tres mediciones **DESPUÉS del fix** exigen que el fix esté **DESPLEGADO**. **No tengo autoridad de deploy**: el objetivo del equipo es *"sin firma, sin broadcast, sin deploy"* y los alcances de deploy (`docker/`, `.github/`) están **fuera de mi alcance**.

**No hice ningún workaround sobre producción.** Correr el consumidor parcheado contra el mismo grupo `selector-g0` sería un acto de deploy, competiría con el consumidor vivo y podría robarle entradas — **peligroso, y no lo hice.**

**Queda listo para cerrarse en cuanto se despliegue**, con estos comandos exactos:

```bash
# 4) el conteo debe CAER (baseline propio: 303 en 3m)
docker logs arbitragex-v2-selector-api-1 --since 3m 2>&1 | grep -c consumer.invalid_message

# 5) deben aparecer entradas NUEVAS: last-delivered-id debe AVANZAR
#    (antes de mi fix: sim-ctl-g0 en 1789637172953-0, XLEN 10001)
docker exec arbitragex-v2-redis-1 redis-cli XINFO GROUPS arbx:opps:validated

# 6) simulations debe dejar de estar en 0 (control: SELECT 1)
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT 1"
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT count(*) FROM simulations"
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT n_tup_ins FROM pg_stat_user_tables WHERE relname='simulations'"
```

## 9. RETRACTACIÓN VISIBLE DE MI F4 DE t66

En `PRODUCTOR-SIM-01` (t66) escribí que el comentario `migrations/113:17` (*"~640k rows"*) era plausiblemente cierto y que **`ON DELETE CASCADE` "refuerza (B)"**, concluyendo que *"sin una sola escritura en la historia" sería falso*.

**RETIRO ESE F4. La medición me contradice:**

- `simulations` → **`n_tup_ins = 0`, `n_tup_upd = 0`, `n_tup_del = 0`**. **Nunca entró ni salió una fila.**
- `stats_reset` de `arbitragex` → **`(nunca)`**: son contadores **de por vida**.
- Por lo tanto **la cascada nunca actuó** (no había nada que cascadear) y **`migrations/113:17` es FALSO**: `simulations` nunca tuvo ~640k filas.
- **"Cero escrituras de por vida" queda PROBADO**, no refutado.

**Lo que sí sigue en pie de t66:** que `insert_simulation` **sí es llamada**, y que las hipótesis (A) índice INVALID y (B) FK CASCADE quedaban **sin decidir** — el capitán las corrió y **ambas caen**: `simulations_revm_idempotency_uq indisvalid = true` refuta (A); `n_tup_ins = 0` refuta (B). La causa era **(C), esta**. Se retracta el F4; **no se borra** (§9 queda como registro).

## 10. LÍMITES Y P/N

- **NO se toca el umbral de viabilidad ni el gate de rechazo económico.** Probado por control negativo (§5.1): el schema sigue estricto y sólo se declaró una clave que **ninguna decisión consume**. **No hubo que relajar ningún gate** — el fix no lo exigió, así que no se paró por esa regla.
- **NO se firma, NO se emite, NO se toca capital.** Sin deploy, sin merge.
- **NO mueve P/N (0/115).**
- **Deuda declarada:** el fix canónico va en `shared-ts/` (fuera de alcance). Mientras el productor Rust añada claves unilateralmente, esto puede repetirse — es la **tercera** vez documentada en el propio schema.

*Causa identificada con el payload real y el error real; fix verificado con el parser REAL sobre 120 payloads REALES del stream vivo (0/120 → 120/120) y con control negativo que prueba que el estricto sigue estricto. Las tres mediciones posteriores quedan declaradas NO COMPUTADAS por falta de autoridad de deploy.*
