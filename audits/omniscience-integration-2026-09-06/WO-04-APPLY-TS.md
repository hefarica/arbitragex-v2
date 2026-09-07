# WO-04 — APPLY (parte TS): parametrización del literal 30 bps LP fee

> Work-order de APLICACIÓN (mitad TypeScript de WO-04). Fecha: 2026-09-06.
> Applier: agente Gang Omniscience `ecc:typescript-reviewer` (rubric
> `ecc-backend-patterns`). Diseño fuente: `WO-04-DESIGN.md` §(d) diffs
> **D3, D12, D13, D14, D15** (la mitad Rust — D1-D2, D4-D11 — pertenece al
> applier Rust paralelo; archivos disjuntos, cero .rs tocados).
> Estado: **APPLIED + RE-VERIFIED** — gates propios EXIT 0; suite completa
> EXIT 1 con único failure FUERA de claim (untracked de WO paralelo — §3/§7).

Lexico OMEGA: LP fee = fricción de Variedad de Liquidez · gas = fricción
termodinámica. El literal 4 del charter ("30 bps") era el default del proxy
de fricción de Variedad de Liquidez en la simulación viva del api-server.

---

## 1. Cambios aplicados (evidencia file:line post-edición)

### D3 — `shared-ts/src/config/index.ts` (espejo zod del TOML, literal 1)

- **L104-109**: nuevo campo `priority_fee_gwei:
  z.number().nonnegative().optional().default(2.0)` en `execution`
  (`.strict()`), espejo del `ExecutionCfg::priority_fee_gwei` Rust (serde
  default 2.0) y de `configs/schemas/app.schema.json`. Default idéntico al
  literal 2 gwei del `bundle_builder.rs:171` → sin cambio de comportamiento
  (`.default()` solo materializa el valor que antes estaba hardcodeado).
- Consumidores verificados antes de editar: cero construcción de literales
  `execution` fuera de shared-ts (grep `priority_fee_increment_pct|AppConfigSchema`
  → solo shared-ts/src/config/index.ts); `selector-api` importa `AppConfig`
  como tipo y no construye literales. Frontend no consume AppConfig.

### D12 — `backend/api-server/src/routes/trading-config.ts` (literal 4, plano PUT admin)

| Sitio | file:line | Cambio |
|---|---|---|
| zod schema | L174-178 | `lp_fee_default_pct: z.number().min(0.0).max(0.5).default(0.003)` — **validación fail-fast** (bounds = CHECK de la migración 119; input inválido → 400) |
| `DbRow` | L267-268 | `lp_fee_default_pct: string;` (NUMERIC llega como string) |
| `rowToRedisState` | L378 | `lp_fee_default_pct: Number(row.lp_fee_default_pct)` → espejo Redis JSON |
| SELECT rehydrate | L434 | columna junto a `flashloan_fee_pct` |
| SELECT GET público | L510 | ídem |
| INSERT columnas | L587 | ídem → posición 23 |
| VALUES | L592-596 | `$23` insertado tras `$22`; renumerados `$25::uuid[]..$33` (33 columnas ↔ 33 placeholders ↔ 33 params — alineación verificada por lectura) |
| ON CONFLICT | L619 | `lp_fee_default_pct = EXCLUDED.lp_fee_default_pct` |
| RETURNING | L640 | columna añadida |
| params array | L669 | `body.lp_fee_default_pct` → `$23` |
| SELECT admin list | L854 | columna añadida |

### D13 — `backend/api-server/src/simulation/tradingConfigSnapshot.ts` (plano lectura hot-path)

- **L49-54**: `TradingConfigSnapshot.lp_fee_default_pct: number` (campo
  requerido — espejo verbatim del struct Rust).
- **L174-176**: `parseSnapshot`: `num(o["lp_fee_default_pct"], 0.003)` —
  blob Redis pre-migración-119 (sin el campo) parsea a **0.003** = default
  doctrinal, idéntico al serde default Rust (invariante de primer deploy).

### D14 — `backend/api-server/src/simulation/computeSimulatedNet.ts` (consumo)

- **L138-142**: `LP_FEE_FRACTION_DEFAULT = 0.003` **ELIMINADO** (tombstone
  comment documenta el nuevo hogar único por lado). Cero literales de
  operador en el simulador (RULE 00).
- **L239-249**: Component 2 consume `cfg.lp_fee_default_pct`; la nota R8
  pasa a dinámica: `` notes.push(`lp-fee=${Math.round(cfg.lp_fee_default_pct * 10_000)}bps-proxy`) ``.
  Con default 0.003 → `Math.round(30.000000000000004) === 30` → nota
  `"lp-fee=30bps-proxy"` **byte-idéntica** a la era hardcodeada (cero
  consumidores del string literal fuera de este archivo — re-verificado por
  grep en este apply).
- **L383**: `varCostRateFromCfg` usa `cfg.lp_fee_default_pct` en vez de la
  constante → el sizer inverso y el forward quedan alimentados por el MISMO
  knob (imposible diverger).

### D15 — `backend/api-server/src/simulation/computeSimulatedNet.test.ts` (tests de invariante)

- **L38**: fixture `baseCfg()` + `lp_fee_default_pct: 0.003` (escenario
  canónico existente queda intacto: lp_fees $7.05, nota 30bps).
- **L158-199 Caso 1 (default / primer deploy)**: blob Redis construido a
  mano SIN el campo (como lo persistía un PUT pre-migración-119) →
  `getTradingConfigForChain` con doble de cliente Redis (frontera de
  infraestructura; los DATOS atraviesan el `parseSnapshot` REAL — no hay
  mock de resultados, RULE 00) → `snapshot.lp_fee_default_pct === 0.003`,
  `lp_fees_usd ≈ 7.05`, nota exacta `"lp-fee=30bps-proxy"`.
- **L201-234 Caso 2 (override 0.001 = 10 bps)**: `lp_fees_usd ≈ $2.35`,
  nota `"lp-fee=10bps-proxy"`, y el delta de `varCostRate` (fn privada)
  observado vía `inverseSize`: varCostRate 0.0099 → 0.0079 ⇒ el monto
  requerido para piso neto $50 CAE (≈$1557 < ≈$1661 default, aserción de
  rango + comparación estricta contra el caso default).

## 2. Invariante de primer deploy (sin cambio de comportamiento)

1. Blob Redis sin la clave → `num(..., 0.003)` (D13) → mismo 0.003 que la
   constante borrada (D14). Cubierto por el **Caso 1** del test.
2. PUT del frontend `/strategies` NO envía `lp_fee_default_pct` (el form no
   conoce el campo — ver §Declaraciones) → zod `.default(0.003)` → misma
   columna persistida. Protected por diseño.
3. GET público responde con la clave nueva; `TradingConfigConfiguredSchema`
   del frontend es `z.object` NO `.strict()` (frontend/lib/schemas.ts:504-507)
   → clave desconocida se descarta, parse sigue verde. Edge worker proxya
   verbatim (edge/worker/src/index.ts:1056-1081). Verificado por lectura.
4. Consumidor Rust del Redis JSON: `TradingConfigState` tolera claves
   extra (serde sin deny_unknown_fields) incluso ANTES de que el applier
   Rust aterrice D10; con D10 aterrizado, `#[serde(default)]` = mismo 0.003.

## 3. Gates (comandos exactos + EXIT codes)

Dos corridas: (i) la del apply original y (ii) la **re-verificación
independiente** del agente que cierra este WO (misma working tree, sin
cambios de código entre ambas — ver §7). Números de (ii):

| Gate | Comando (cwd) | Resultado | EXIT |
|---|---|---|---|
| vitest suites afectadas | `npx vitest run src/simulation/computeSimulatedNet.test.ts src/routes/trading-config.test.ts` (`backend/api-server`) | 2 files / **30 passed** (26 sim + 4 route) | **0** |
| vitest suite completa unit | `npx vitest run` (`backend/api-server`, excluye `test/` integración) | 59 files / **725 passed + 1 failed** — ver abajo | **1** |
| typecheck api-server | `npx tsc --noEmit -p tsconfig.json` | sin errores | **0** |
| typecheck shared-ts | `npx tsc --noEmit` (`shared-ts`) | sin errores | **0** |

**El único failure de la suite completa está FUERA del claim de WO-04**:
`src/credentials/crypto.test.ts > "rejects version current-1 without PREV…"`
(`AssertionError: expected function to throw an error, but it didn't`, L206).
Ese archivo es **untracked** en git y pertenece a un WO paralelo en curso
(credentials/crypto — junto a `websocket-wo10.test.ts` y
`websocket-hot-streamer.test.ts`, también untracked). Desacoplamiento
verificado por grep: cero imports de `tradingConfigSnapshot` /
`computeSimulatedNet` / `trading-config` / `config/index` en esos 4 archivos
(exit 1). En la corrida del apply original la suite dio 56 files / 689
passed / EXIT 0 — la delta (56→59 files, 689→726 tests) son exactamente los
3 archivos de test del WO paralelo que aterrizaron DESPUÉS en la working
tree. No lo toco (§3 Surgical / claim discipline); su dueño lo repara.

Observación honesta: el banner de vitest resolvió **v1.6.1** (workspace
hoisted), no el `^3.2.6` declarado en devDependencies del paquete — mismo
estado pre-existente del repo, no introducido por este WO.

## 4. Declaraciones fail-honest (R8)

1. **El form frontend no expone aún el knob** (`frontend/components/trading-config-form.tsx`
   no lista `lp_fee_default_pct`). NO está en mi claim → no lo toqué. Sin
   efecto de comportamiento (zod default), pero el operador hoy solo puede
   girar el knob vía PUT admin crudo. Follow-up natural: WO frontend.
2. **Migración 119 + `shared-rs/trading_config.rs` (D10-D11) = mitad Rust**,
   applier paralelo. Orden de deploy documentado en el diseño: migración 119
   ANTES del deploy del api-server nuevo (el SELECT lista la columna; el zod
   default protege el PUT, no el SELECT).
3. **`selector-api` NO fue re-typecheckeado** (fuera de claim): consume
   `AppConfig` como tipo; verificado por grep que no construye literales
   `execution` (riesgo de compilación nulo), pero el gate formal no corrió.
4. Paridad de defaults 4 planos del literal 4: **0.003** (zod PUT, L178) ==
   **0.003** (snapshot TS, L176) == plano PG **0.0030** y plano Rust serde
   **0.003** (ambos del applier Rust — citados por diseño, no ejecutados aquí).
   Los planos TS quedan pinneados por el Caso 1 del test.

## 5. Restricciones cumplidas

- CERO archivos `.rs` / `scanner.rs` tocados (diff confinado a los 5 archivos
  claimados — `git diff --stat` en §6).
- CERO git write (sin commit/push/PR), CERO VPS, CERO requests a dominio
  público (presupuesto usado: 0/5).
- Diffs marcados `// WO-04 (2026-09-06)` en cada hunk (grep §6).
- RULE 00: sin datos fabricados — el doble de Redis en el Caso 1 es un doble
  de CLIENTE en la frontera de infraestructura; el JSON de config es un
  fixture de input y atraviesa el `parseSnapshot` real bajo prueba.
- §32/§33: modo audit/scaffold — nada de executor/wallets/capital/firma/
  broadcast. §34.3: ni un flag de modo fue tocado.

## 6. Evidencia de confinamiento del diff

```
 backend/api-server/src/routes/trading-config.ts          | 19 ++++-
 backend/api-server/src/simulation/computeSimulatedNet.test.ts | 80 ++++++++++++-
 backend/api-server/src/simulation/computeSimulatedNet.ts  | 19 +++--
 backend/api-server/src/simulation/tradingConfigSnapshot.ts |  9 +++
 shared-ts/src/config/index.ts                             |  4 ++
 5 files changed, 123 insertions(+), 8 deletions(-)
```

(`git diff --stat -- <los 5 archivos claimados>`, working tree local, sin
commit — protocolo operador 2026-08-23.)

## 7. Re-verificación independiente (agente que cierra — 2026-09-06)

La working tree llegó a este agente con las 5 ediciones ya aplicadas
(diff 123+/8− idéntico al §6 — patrón RESPAWN-2: verificación NUNCA hereda
del reporte previo). Se re-verificó TODO desde cero:

1. **Contenido vs diseño D3/D12/D13/D14/D15** — re-leído diff completo de
   los 5 archivos: zod `lp_fee_default_pct: z.number().min(0.0).max(0.5).default(0.003)`
   (fail-fast 400), campo `DbRow`, `rowToRedisState`, 4 SELECT + INSERT +
   ON CONFLICT + RETURNING + params, snapshot interface+`num(...,0.003)`,
   consumo en los 2 sitios del simulador, tests Caso 1/2. Todo presente,
   todo marcado `// WO-04 (2026-09-06)`.
2. **Alineación INSERT contada a mano**: 33 columnas ↔ 33 placeholders
   ($1..$24, `$25::uuid[]`, `$26::jsonb`, $27..$33) ↔ 33 params;
   `body.lp_fee_default_pct` = `$23` emparejado con la columna 23
   `lp_fee_default_pct` (trading-config.ts:587/594/669).
3. **Gates re-ejecutados** (§3): targeted vitest EXIT 0 (30 passed), tsc
   api-server EXIT 0, tsc shared-ts EXIT 0; suite completa = único failure
   fuera de claim (untracked paralelo, desacoplado por grep).
4. **Sin referencias colgantes**: `LP_FEE_FRACTION_DEFAULT` sobrevive solo
   en comentarios (tombstone computeSimulatedNet.ts:138 + test L160);
   el string `lp-fee=30bps-proxy` existe ÚNICAMENTE en el test que pinnea
   el contrato (cero consumidores en api-server src y frontend — si algo
   lo matcheara, la nota dinámica lo rompería).
5. **Exports que usa el test existen**: `getTradingConfigForChain`
   (tradingConfigSnapshot.ts:201) y `_clearSnapshotCacheForTests` (L315,
   test-only helper pre-existente).
6. **Consumidores del tipo `TradingConfigSnapshot`**: 4 archivos; el único
   fuera del claim (`routes/opportunities-live.ts`) solo importa el tipo
   (L133, Map en L822) — sin construcción literal; tsc verde lo pinnea.
7. **Claim del reporte sobre frontend re-verificada por lectura**:
   `frontend/lib/schemas.ts:504` — `TradingConfigConfiguredSchema` es
   `z.object` plano (sin `.strict()`): zod DESCARTA claves desconocidas →
   el GET público con `lp_fee_default_pct` extra parsea verde. (El schema
   base tampoco conoce el campo — L468 lista `flashloan_fee_pct`, sin
   `lp_fee_default_pct`: stripped, sin efecto.)

## Estado

**APPLIED + RE-VERIFIED** — mitad TS de WO-04 completa según diseño
D3/D12/D13/D14/D15; gates propios EXIT 0 (targeted vitest 30 passed, tsc
api-server, tsc shared-ts); suite completa EXIT 1 con único failure FUERA
de claim (untracked de WO paralelo, desacoplado por grep); invariante de
primer deploy pinneada por test (Caso 1: blob sin campo ⇒ 0.003 ⇒ nota
`"lp-fee=30bps-proxy"` byte-idéntica).
