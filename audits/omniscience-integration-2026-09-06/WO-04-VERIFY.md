# WO-04 — VERIFY (adversarial): parametrización de literales económicos

> Work-order de VERIFICACIÓN (kind: verify). Fecha: 2026-09-06.
> Verifier: agente Gang Omniscience `ecc:rust-reviewer` (+ rubric `ecc:typescript-reviewer`
> para la mitad TS). Corre TRAS el apply Rust (WO-04-APPLY-RUST.md) y TRAS el apply
> TS (WO-04-APPLY-TS.md, aterrizado en commit `97742279`).
> Estado auditado: working tree branch `feat/gang-omniscience-2026-09-06` @ `6d9d7830`
> (mitad Rust = 17 archivos modified + migración 119 untracked; mitad TS = commit
> `97742279` ya en historia).
>
> Lexico OMEGA: tip EIP-1559 = parámetro de mercado de gas · GAS_COST_USD =
> fricción termodinámica pre-screen · LP fee = fricción de Variedad de Liquidez.

## VEREDICTO: **APPROVE** (con 2 findings MINOR + 1 hazard operativo para el operador)

Los 6 ítems del checklist charter PASS. Cero defectos CRITICAL: ningún default
derivó del literal previo, la validación es fail-fast al boot (no hot-loop), el
grep del valor antiguo está limpio en los archivos tocados, los 10 gates
re-ejecutados dieron EXIT 0 (exit codes capturados contra `$LASTEXITCODE`
PowerShell, ver §Meta-verificación), los hunks WO-02 de scanner.rs están
intactos y §34.3 permanece intocado.

---

## 1. Checklist charter — matriz file:line

### (1) Defaults parametrizados == literales previos BYTE-VALOR — **PASS**

| # | Literal previo | Hogar nuevo (file:line verificado por lectura) | Default | Igualdad byte-valor |
|---|---|---|---|---|
| L1 | `U256::from(2_000_000_000u64)` (tip 2 gwei, bundle_builder.rs:171 pre) | serde `default_priority_fee_gwei()` → `shared-rs/src/config.rs:166-168`; consumo `bundle_builder.rs:178` `(priority_fee_gwei * 1e9).round() as u64` | 2.0 | `2.0f64 × 1e9 = 2_000_000_000.0` EXACTO en IEEE754 (2e9 < 2^53, entero representable); `.round()` idempotente; `as u64` = 2000000000 → `U256::from` idéntico al literal histórico |
| L1-espejo zod | — | `shared-ts/src/config/index.ts:108` `.default(2.0)` | 2.0 | idem (plano espejo) |
| L1-espejo schema | — | `configs/schemas/app.schema.json:51` `minimum: 0` (sin default — lo aporta serde) | — | TOML no lista la clave (`configs/app.toml:18-27` termina en `priority_fee_increment_pct = 10`, verificado por lectura) → serde default gobierna |
| L2 | `const GAS_COST_USD: f64 = 30.0` (liquidation_worker.rs:147 pre) | `pub const DEFAULT_GAS_COST_USD: f64 = 30.0` `liquidation_worker.rs:151`; `resolve_gas_cost_usd()` `liquidation_worker.rs:154-172` (env ausente → default) | 30.0 | mismo f64 30.0; boot log `liquidation_worker.rs:763` imprime `self.gas_cost_usd` = 30.0 (antes la constante 30.0 — mismo valor) |
| L3 | espejo privado `const GAS_COST_USD: f64 = 30.0` (liquidation_engine.rs:60 pre) | espejo ELIMINADO (diff verificado); engine recibe `gas_cost_usd` en `new()` `liquidation_engine.rs:78-89`; uso vivo `liquidation_engine.rs:235` `self.gas_cost_usd` | 30.0 | `scanner.rs:448-453` inyecta `resolve_gas_cost_usd()` → 30.0 con env ausente. Riesgo de deriva eliminado por construcción (una sola fuente) |
| L4 | `const LP_FEE_FRACTION_DEFAULT = 0.003` (computeSimulatedNet.ts:139 pre) | 4 planos: serde `trading_config.rs:426-428` (0.003) · zod PUT `trading-config.ts:178` `.default(0.003)` · PG migración 119 `DEFAULT 0.0030` NUMERIC(6,4) · snapshot TS `tradingConfigSnapshot.ts:176` `num(o[...], 0.003)` | 0.003 | fracción idéntica en los 4 planos (0.0030 NUMERIC ≡ 0.003 double). Nota R8: `Math.round(0.003 × 10_000) === 30` → `"lp-fee=30bps-proxy"` byte-idéntica — **pinneada por test** (computeSimulatedNet.test.ts:199, Caso 1) |

§34.1/§34.4 (carga de la prueba): la matemática no cambió en ningún modo — cada
knob materializa exactamente el literal previo cuando el operador no actúa.
Conversión f64→u64 del L1 satura (Rust ≥1.45) sin panic; schema acota `v ≥ 0`.

### (2) Config validada fail-fast AL BOOT, no en hot-loop — **PASS**

- **L1 (TOML)**: `AppConfig::load_from` `shared-rs/src/config.rs:205` valida schema
  cuando `ARBX_VALIDATE_SCHEMA=1` (`config.rs:212`) — boot, pre-spawn. Clave
  negativa en TOML → `ConfigError::Schema` al arranque (mismo fail-fast que los
  campos hermano). El consumo es 1× por bundle desde config ya cargada
  (`submit_engine.rs:446` → struct field read), sin parse en hot-loop.
- **L2/L3 (env)**: `resolve_gas_cost_usd()` se invoca EXACTAMENTE en 2 sitios,
  ambos boot-time: `scanner.rs:451` (dentro de `build_orchestrator`, constructor
  del camino vivo) y `main.rs:1022` (spawn del worker legacy, gated). Censo
  workspace completo de constructores (`LiquidationEngine::new` /
  `LiquidationWorker::new`): 2 sitios src + 3 tests — todos verificados, cero
  hot-loop. El kernel `estimate_liquidation_profit` (`liquidation_worker.rs:283-292`)
  rechaza gas no-finito/negativo → None (R8) — el check defensivo del camino de
  riesgo sigue vivo ADEMÁS del warn del resolver (`liquidation_worker.rs:163-170`).
- **L4 (PUT admin)**: zod `z.number().min(0.0).max(0.5).default(0.003)`
  (`trading-config.ts:178`) = fail-fast 400 en la frontera de escritura; DB CHECK
  `>= 0 AND <= 0.5` (migración 119) como segunda capa. Lectura hot-path:
  `num()` (`tradingConfigSnapshot.ts:93-100`) rechaza no-finitos → fallback
  0.003 — misera el patrón exacto de los campos hermano (`flashloan_fee_pct`
  et al.). Único escritor del blob = el PUT validado.

### (3) Cero hardcode residual del valor antiguo en archivos tocados — **PASS**

Grep del valor antiguo sobre los 8 archivos Rust + 3 TS tocados:

- `2_000_000_000` → **0 ocurrencias** en archivos tocados. Única ocurrencia del
  repo: `relays-client/src/executor/gas_oracle.rs:37` — gemelo MUERTO (verificado:
  cero referencias a `GasOracle` fuera de su propio archivo, grep exit 1), NO
  tocado y declarado fuera de charter por diseño §Gemelos + apply §5.4. Correcto.
- `30.0;` → única ocurrencia `liquidation_worker.rs:151` = el NUEVO default
  canónico `DEFAULT_GAS_COST_USD` (el hogar declarado del knob, no residuo).
- `0.003` → solo los planos canónicos: serde default (trading_config.rs:427), su
  doc (362), fixture de test (775); TS: zod default (trading-config.ts:178),
  snapshot default (tradingConfigSnapshot.ts:176), comentarios tombstone
  (computeSimulatedNet.ts:138/140/242). Cero literal de cómputo.
- `LP_FEE_FRACTION_DEFAULT` → solo 2 comentarios tombstone (el propio archivo y
  el test que documenta el origen). Identificador de código: 0.
- String `"lp-fee=30bps-proxy"` → únicamente en el test que pinnea el contrato
  (computeSimulatedNet.test.ts:168/199/208). Cero consumidores en src/frontend
  (la nota es ahora dinámica).

### (4) Gates re-ejecutados (exit codes REALES — ver §Meta) — **PASS**

| Gate | Comando (cwd) | Resultado | EXIT |
|---|---|---|---|
| cargo check --all-targets | `cargo check -p shared-rs -p relays-client -p searcher-rs -p prioritization-spine --all-targets` (`backend/`) | Finished, 0 err/0 warn (54.79s) | **0** |
| cargo clippy | `… --all-targets -- -D warnings` | limpio (4m31s) | **0** |
| cargo test searcher-rs --lib | | **1150 passed / 0 failed / 3 ignored** (0.92s) | **0** |
| cargo test shared-rs | | **217 passed / 0 failed** + 1 doc/integración passed (30.86s) | **0** |
| cargo test prioritization-spine | | **121 passed / 0 failed** + 0/2 ignored integración | **0** |
| cargo test relays-client | | **77 passed / 0 failed / 1 ignored** (0.07s) | **0** |
| cargo test searcher-rs liquidation (filtro del diseño) | | 3 passed (lib) + integración 0 matching | **0** |
| vitest dirigido WO-04 | `npx vitest run src/simulation/computeSimulatedNet.test.ts src/routes/trading-config.test.ts` (`backend/api-server/`) | **2 files / 30 passed** (26 sim + 4 route) | **0** |
| tsc api-server | `npx tsc --noEmit -p tsconfig.json` | sin errores | **0** |
| tsc shared-ts | `npx tsc --noEmit` (`shared-ts/`) | sin errores | **0** |
| vitest suite completa unit api-server (regresión) | `npx vitest run` | **59 files / 729 passed / 0 failed** (624.38s) | **0** |

Suma tests PASSED re-ejecutados sobre los 4 crates: **1566 / 0 failed** —
coincide 1:1 con lo reportado por el applier (1150+218+121+77). Windows
AppControl 4551 NO bloqueó (dev profile, target caliente del árbol principal).
Cobertura de colateral confirmada por compilación: `cargo check --all-targets`
incluye los 3 tests de integración y los 6 fixtures D10-collateral; censo
workspace-wide: `ExecutionCfg {` se construye literal SOLO en su definición
(shared-rs/src/config.rs); `TradingConfigState {` en exactamente los 6 archivos
del colateral — cero constructor huérfano en sim-ctl/recon/math-engine/
simulator-v2/sim-core/token-enricher/mcp-sim-engine.

### (5) scanner.rs — hunks WO-02 intactos tras el apply WO-04 — **PASS**

`git diff backend/searcher-rs/src/scanner.rs` contiene UN solo hunk: el bloque
WO-04 en L443-453 (constructor de LiquidationEngine). Verificación por grep de
los marcadores WO-02/WO-10 en el archivo actual: `emit_simulated` cable en
scanner.rs:2651-2661, 5º elemento del stream en 2411/2916/2940/3061, mapping
verbatim 3221/3334, spans de latencia WO-10 en 1507/1529/1600 — **todos
presentes e intactos**. Ninguna línea WO-02/WO-10 aparece en el diff WO-04.

### (6) §34.3 intacto — **PASS**

`git status --porcelain backend/relays-client/src/live_exec_policy.rs
backend/relays-client/src/executor/` = **vacío** (cero modificación). Lectura
directa: default-deny `ARBX_LIVE_EXEC_ENABLED != 'true'` (live_exec_policy.rs:12/32/53),
`MainnetRefused` en :37/:85/:124/:141/:154 — el terminus PHYSICALLY REFUSES
mainnet como antes. La barrera M1 de `build_and_sign` (primer statement,
default-deny + testnet-only, bundle_builder.rs:111-113) también intacta. Ningún
flag de modo fue tocado por WO-04 (los 3 knobs son fricción/config, no terminus).

---

## 2. Verificación adversarial adicional (más allá del charter)

- **Alineación INSERT contada a mano (independiente)**:
  `trading-config.ts:577-596` columnas = 33 (`lp_fee_default_pct` posición 23);
  placeholders `$1..$33` con casts `$6/$8/$9::jsonb`, `$25::uuid[]`, `$26::jsonb`;
  array params `trading-config.ts:647-680` = 33 entradas con
  `body.lp_fee_default_pct` en la posición 23 ↔ `$23` ↔ columna 23. Casts
  emparejados con su param JSON.stringify correspondiente. **Alineación exacta.**
- **Nota R8 dinámica**: `computeSimulatedNet.ts:246-248` — `Math.round(cfg.lp_fee_default_pct * 10_000)`;
  override 0.001 → `lp-fee=10bps-proxy` (Caso 2 del test, aserción negativa contra 30bps).
- **`varCostRateFromCfg`** (computeSimulatedNet.ts:383): mismo knob que el forward
  → sizer inverso y simulación no pueden diverger.
- **Fail-honest del parser**: blob corrupto → `parseSnapshot` null → snapshot
  doctrinal por defecto (patrón pre-existente); `num()` finito-guard ya citado.
- **Orden de migración**: 118 (existente) → **119 (nueva, WO-04)** → 120 (WO-03,
  ya committed) — sin colisión de slot.

## 3. Findings (ninguno CRITICAL)

| ID | Severidad | file:line | Descripción | Acción |
|---|---|---|---|---|
| F1 | **MINOR** (cosmético) | `trading-config.ts:672` | Comentario stale: dice "JSONB stringified for `$25::jsonb` cast" pero tras el renumerado WO-04 `strategy_configs` es `$26::jsonb` (`$25` es ahora `enabled_dex_ids::uuid[]`). El comentario era correcto pre-WO-04 y quedó desactualizado POR el renumerado. Cero efecto en comportamiento (el cast vive en el SQL, no en el comentario). | 1 línea del applier TS en su próximo touch; NO bloquea |
| F2 | **MINOR** (cosmético) | `liquidation_worker.rs:36` | Doc de módulo expone la fórmula `net_profit_usd = gross_profit_usd − GAS_COST_USD` usando el identificador eliminado; el paréntesis inmediato documenta el knob real (`LIQUIDATION_GAS_COST_USD`, default `DEFAULT_GAS_COST_USD`). Etiqueta de fórmula, no código. | opcional; NO bloquea |
| F3 | **HAZARD OPERATIVO** (acción del operador, NO defecto de código) | `database/migrations/119_...sql` (untracked) vs commit `97742279` | La mitad TS **ya está committed** y sus SELECT listan `lp_fee_default_pct`; la migración 119 está **untracked** en la working tree. Si el operador deploya el estado committed sin llevar la 119, los routes de trading-config rompen (column does not exist) y el mirror Redis queda sin refrescar. Bajo protocolo NO-GIT ningún agente puede commitearla — es territorio operador. | El push que lleve la mitad Rust DEBE incluir la 119 en el MISMO cambio (orden documentado en 3 lugares: diseño §Invariante, apply-Rust §8.1, apply-TS §4.2) |
| F4 | **RESUELTO a favor** (re-verify) | suite completa unit api-server | El apply-TS reportó EXIT 1 con 1 failure fuera de claim (`credentials/crypto.test.ts` "rejects version current-1"). En MI re-ejecución la suite completa da **59 files / 729 passed / 0 failed, EXIT 0** — el dueño de ese WO paralelo lo reparó entre ambos runs. Regresión total: LIMPIA. | ninguna |

Declaraciones del applier re-verificadas como CIERTAS: gas_oracle.rs gemelo muerto
intacto y fuera de charter (F-adjacent, correcto); `MIGRATION_HISTORY.md` sin
entrada para 119 (tampoco para 120 — precedente mixto, decisión operador
documentada); fallback base-fee 30 gwei `bundle_builder.rs:176` adyacente intacto;
`configs/app.toml` sin la clave nueva (serde default gobierna — byte-identidad).

## 4. Restricciones del verifier cumplidas

- CERO git write (sin commit/push/PR): solo lectura de git + edición del propio
  reporte. CERO VPS, CERO requests a dominio público (presupuesto HTTP: 0/5).
- §32/§33: modo audit/read-only — nada de executor/wallets/capital/firma/broadcast.
- RULE 00: este reporte cita solo evidencia leída/ejecutada; los números de gates
  provienen de corridas propias con exit code real.

## Meta-verificación (lección de infraestructura para futuros gates)

El intento inicial de correr cargo en Git Bash produjo `cargo: command not found`
pero los `echo EXIT:$?` tras un pipe (`cmd | tail; echo $?`) reportaban **0 falso**
(exit de `tail`, no del compilador). Falso-verde detectado y corregido: todos los
gates de §1.(4) fueron re-ejecutados vía PowerShell con `$LASTEXITCODE` capturado
inmediatamente tras el comando, sin pipe intermedio. Regla para próximos
verifiers de este gang: `cmd | tail; echo $?` en bash miente sobre el exit del
comando — usar `${PIPESTATUS[0]}` o PowerShell `$LASTEXITCODE`.

## Estado

**APPROVE** — WO-04 completo (mitad Rust working-tree + mitad TS committed
`97742279`) verificado adversarialmente: 6/6 checklist PASS, 1566 tests Rust +
suite TS completa 729/729 (incluye los 30 dirigidos WO-04) + 2 tsc re-ejecutados
EXIT 0, invariante de primer deploy preservada por construcción y pinneada por
test, §34.3/WO-02/RULE 00 intactos. Los 2 findings MINOR no bloquean; el hazard
F3 (migración 119 untracked vs TS committed) requiere acción del operador en el
momento del push.
