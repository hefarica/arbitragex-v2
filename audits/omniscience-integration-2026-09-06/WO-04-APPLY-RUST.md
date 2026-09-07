# WO-04 — APPLY (parte Rust): parametrización tip 2 gwei + GAS_COST_USD=30

> Work-order de APLICACIÓN (mitad Rust de WO-04). Fecha: 2026-09-06.
> Applier: agente Gang Omniscience `rust-topology-engineer` (rubric
> `ecc:rust-patterns`). Diseño fuente: `WO-04-DESIGN.md` §(d) diffs
> **D1, D2, D4, D5, D6, D7, D8, D9, D10, D11** (la mitad TS — D3, D12–D15 —
> ya aplicada y re-verificada en `WO-04-APPLY-TS.md`; archivos disjuntos).
> Estado: **APPLIED_VERIFIED (local)** — D1-D11 aplicados; 9/9 gates EXIT 0
> (check ×2, clippy -D warnings, fmt, test ×5: 1566 passed / 0 failed en los
> 4 crates tocados). Detalle en §4 + §Gates-addendum.
>
> Lexico OMEGA: gas = fricción termodinámica · LP fee = fricción de Variedad
> de Liquidez · tip EIP-1559 = parámetro de mercado de gas (NO fee de pool).

---

## 0. Resolución del borde de claim (documentada, no improvisada)

El dispatch listaba 7 archivos `.rs` como claim, pero el board
(`GOAL-WORKORDERS.md` fila WO-04) asigna "**½ Rust (D1-D11)**" y el applier
TS declaró explícitamente "la mitad Rust — D1-D2, D4-D11 — pertenece al
applier Rust paralelo; archivos disjuntos". Tres archivos del diseño quedaban
fuera de la lista literal del dispatch pero DENTRO de la mitad Rust:

1. `backend/relays-client/src/submit_engine.rs` (D5) — **dependencia dura de
   compilación**: D4 cambia la firma de `build_and_sign`; sin D5 el workspace
   NO compila y el gate `cargo check` es imposible. Call-site único
   productivo (grep verificado: solo `submit_engine.rs:438`).
2. `configs/schemas/app.schema.json` (D2) — espejo obligatorio: `execution`
   es `additionalProperties: false`; sin D2 la clave nueva en TOML sería
   rechazada por `ARBX_VALIDATE_SCHEMA=1` (fail-fast roto, no endurecido).
3. `database/migrations/119_trading_config_lp_fee_default_pct.sql` (D11) —
   la migración de la mitad TS ya aterrizada (commits 97742279/9c561ea8):
   el slot 119 quedó libre porque WO-03 aterrizó como `120_…sql` por ruling
   del orquestador (colisión-119, ver fila WO-03 del board).

Decisión: aplicar D1-D11 completos (serie total, ningún otro agente Rust
activo), cada archivo con marcador `// WO-04 (2026-09-06)`. El borde se
documenta aquí en lugar de dejar la mitad Rust incompleta.

## 1. Cambios aplicados (evidencia file:line post-edición)

### D1 — `backend/shared-rs/src/config.rs` (literal 1, hogar del knob)

- **L138-145**: campo `priority_fee_gwei: f64` en `ExecutionCfg` con
  `#[serde(default = "default_priority_fee_gwei")]` y doc que ancla el
  default 2.0 al literal histórico de `bundle_builder.rs:171`.
- **L165-168**: `fn default_priority_fee_gwei() -> f64 { 2.0 }`.
- Cero constructores literales de `ExecutionCfg` en el repo (grep: solo la
  definición) → el campo es serde-only, sin call-sites que reparar.

### D2 — `configs/schemas/app.schema.json` (espejo schema del literal 1)

- **L50-51**: `"priority_fee_gwei": { "type": "number", "minimum": 0 }` en
  `execution.properties` (con la coma de `priority_fee_increment_pct`).
  Fail-fast idéntico al de sus campos hermano: TOML con valor negativo →
  `ConfigError::Schema` al boot con `ARBX_VALIDATE_SCHEMA=1`.

### D4 — `backend/relays-client/src/bundle_builder.rs` (consumo literal 1)

- **L109**: 8º parámetro `priority_fee_gwei: f64` en `build_and_sign`.
- **L97-100**: `#[allow(clippy::too_many_arguments)]` — 8 args cruza el
  umbral (7) de clippy `-D warnings`; patrón con precedente en el repo
  (`cartridge_boot.rs:940`, `evidence.rs:99`, `chain_supervisor.rs:79`,
  `discovery_workload.rs:282`, `math_evidence.rs:86`).
- **L160-161**: comentario de fee actualizado ("operator-configured priority
  tip", default 2 gwei — WO-04).
- **L177-178**: `let priority_fee = U256::from((priority_fee_gwei * 1e9).round() as u64);`
  — el literal `U256::from(2_000_000_000u64)` queda ELIMINADO; conversión
  gwei→wei con redondeo; cast float→int satura (Rust ≥1.45) y el schema
  acota `v ≥ 0`.

### D5 — `backend/relays-client/src/submit_engine.rs` (call-site literal 1)

- **L446**: `self.cfg.execution.priority_fee_gwei,` como 8º argumento —
  exactamente el mismo enhebrado que ya fluyen `max_value_eth` y
  `target_block_offset` (L444-445).

### D6 — `backend/searcher-rs/src/workers/liquidation_worker.rs` (literales 2)

- **L143-151**: `const GAS_COST_USD` → `pub const DEFAULT_GAS_COST_USD: f64 = 30.0`
  con doc del override env.
- **L153-172**: `pub fn resolve_gas_cost_usd() -> f64` — patrón
  `LIQUIDATION_WORKER_INTERVAL_SECS` del mismo archivo endurecido: env
  ausente → default; no-finito/negativo → `warn!` (event
  `liquidation.gas_cost_usd_invalid`, raw, fallback) + default. Un gas
  negativo inflaría el Topological Yield neto → check defensivo obligatorio
  (camino de riesgo). Coherente con el hardening del kernel:
  `estimate_liquidation_profit` ya rechazaba gas no-finito/negativo
  (`liquidation_worker.rs:285-292`), y la resolución al boot adelanta ese
  rechazo con warn explícito.
- **L716-721 + L725-733**: `LiquidationWorker` gana `pub gas_cost_usd: f64`;
  `new(interval_secs, chain_id, gas_cost_usd)`.
- **L763**: log de boot `gas_cost_usd = self.gas_cost_usd` (reporta el knob
  real, mismo 30.0 con env ausente).
- **L984**: uso en el loop → `self.gas_cost_usd`.
- Fixups de consistencia de renombre (solo comentarios, cero código):
  doc de módulo L36-37 y comentario H2 L1072-1073 referenciaban el
  identificador `GAS_COST_USD` eliminado.

### D7 — `backend/searcher-rs/src/engines/liquidation_engine.rs` (literal 3)

- **L56-60 (original) DELETE**: el espejo privado `const GAS_COST_USD` y su
  doc que confesaba el riesgo de deriva ("Must match the value in
  `liquidation_worker::GAS_COST_USD` … by convention") — eliminado completo.
- **L42-44**: import de `DEFAULT_GAS_COST_USD` desde el worker (una sola
  fuente de verdad).
- **L68-91**: `LiquidationEngine` gana `pub gas_cost_usd: f64` recibido en
  `new(indexer, chain_id, gas_cost_usd)`.
- **L235**: uso vivo → `self.gas_cost_usd`.
- **L526, L547, L591** (tests): `GAS_COST_USD` → `DEFAULT_GAS_COST_USD`
  (mismo valor 30.0, cero cambio de expectativa).

### D8 — `backend/searcher-rs/src/scanner.rs` (constructor del camino VIVO)

- **L446-453**: `LiquidationEngine::new(liq_indexer, chain_id,
  crate::workers::liquidation_worker::resolve_gas_cost_usd())`.
- Los hunks WO-02 (emit_simulated, familia latency WO-10) están INTACTOS:
  la edición es un bloque autocontenido en L446; `git diff` muestra solo el
  hunk esperado.

### D9 — `backend/searcher-rs/src/main.rs` (worker legacy, gated)

- **L1019-1023**: `LiquidationWorker::new(liquidation_period_secs,
  liquidation_chain, workers::liquidation_worker::resolve_gas_cost_usd())`.
  Worker y engine leen el MISMO knob al boot → imposible diverger
  (doble lectura documentada en el diseño §Riesgos.2).

### D10 — `backend/shared-rs/src/trading_config.rs` (plano Rust del literal 4)

- **L359-370**: campo `lp_fee_default_pct: f64` con
  `#[serde(default = "default_lp_fee_default_pct")]`, doc que cita la
  migración 119 (CHECK 0–0.5), el precedente schema-drift
  `enabled_dex_ids`, y la doctrina ROUTES_CROWN_JEWEL regla 4 (fee on-chain
  per-leg = verdad; esto es el proxy default gobernable).
- **L424-427**: `fn default_lp_fee_default_pct() -> f64 { 0.003 }`.
- Paridad de defaults del literal 4, 4 planos: **0.003** (serde Rust) ==
  **0.003** (zod PUT admin, aterrizado por la mitad TS) ==
  **0.0030** (PG DEFAULT migración 119) == **0.003** (snapshot TS
  `num(..., 0.003)`).

### D11 — `database/migrations/119_trading_config_lp_fee_default_pct.sql` (NUEVO)

- `ALTER TABLE trading_config ADD COLUMN IF NOT EXISTS lp_fee_default_pct
  NUMERIC(6,4) NOT NULL DEFAULT 0.0030 CHECK (>= 0 AND <= 0.5)` — verbatim
  del diseño. `ADD COLUMN ... DEFAULT` constante = metadata-only (PG ≥ 11);
  tabla diminuta (1 fila/chain).

## 2. Colateral del apply — defectos latentes del diseño encontrados y reparados

El diseño fue verificado por lectura, no por compilación; dos clases de
call-sites fuera de su scope de grep rompían la compilación de tests. Se
repararon con el cambio mínimo (cada línea marcada `// WO-04 (2026-09-06)`):

1. **D10-collateral (6 fixtures `TradingConfigState` exhaustivos)**: el
   struct ganó un campo; los 6 constructores literales de test (sin
   `..Default::default()`) no compilaban. +1 línea `lp_fee_default_pct:
   0.003,` en cada uno:
   `prioritization-spine/src/config_aware.rs:1257`,
   `prioritization-spine/src/strategy_config_gate.rs:399`,
   `shared-rs/src/price_oracle.rs:338`,
   `shared-rs/src/trading_config.rs:775` (sample_state),
   `searcher-rs/src/size_optimizer.rs:1871`,
   `searcher-rs/src/engines/triangular_engine.rs:890`.
   Valor 0.003 = default canónico → cero cambio de comportamiento de tests.
2. **D7-collateral (3 tests de integración)**: `LiquidationEngine::new`
   ganó 3er parámetro; el diseño solo censó call-sites en `src/`
   (`scanner.rs:446`, `main.rs:1019`). Tres tests de integración en
   `searcher-rs/tests/` también construían el engine:
   `tests/cartridge_shadow_replay.rs:190-196`,
   `tests/orchestrator_parallel_run.rs:166-172`,
   `tests/v2_shadow_replay.rs:136-142`. Pinnearon
   `searcher_rs::workers::liquidation_worker::DEFAULT_GAS_COST_USD`
   (siempre 30.0, SIN dependencia del env) para determinismo — los
   shadow-replay/orchestrator tests no deben mutar su semántica si un
   operador exporta `LIQUIDATION_GAS_COST_USD` en el shell de CI.
3. **clippy `too_many_arguments`** (D4): ver §1.D4 — allow con precedente.

## 3. Invariante de primer deploy (cero cambio de comportamiento, §34.1)

- **L1 (tip)**: `configs/app.toml [execution]` NO lista `priority_fee_gwei`
  (verificado por lectura, sección actual termina en
  `priority_fee_increment_pct = 10`) → serde default 2.0 →
  `(2.0 * 1e9).round() as u64 == 2_000_000_000` → tip byte-idéntico al
  literal histórico. Schema con `minimum: 0` acepta el default →
  `ARBX_VALIDATE_SCHEMA=1` sigue verde. zod shared-ts idem (mitad TS).
- **L2/L3 (gas USD)**: env ausente → `DEFAULT_GAS_COST_USD = 30.0` en worker
  (boot log L763) y engine (L235), misma resolución única → imposible
  diverger (el espejo privado con riesgo de deriva fue ELIMINADO).
- **L4 (lp fee)**: blob Redis sin la clave → serde default 0.003; migración
  119 `DEFAULT 0.0030` preserva filas existentes y nuevas.
- **Mode-invariant §34.1**: la matemática de detección/evaluación no cambió
  en NINGÚN modo; los tres knobs son fricción/config, no topología. Ningún
  flag de modo, ni `live_exec_policy.rs`, ni el default-deny, ni
  `MainnetRefused` fueron tocados (§34.3 intacto — `git status` vacío para
  esos archivos).

## 4. Gates (comandos exactos + EXIT codes; cwd `backend/`, target caliente)

| Gate | Comando | Resultado | EXIT |
|---|---|---|---|
| cargo check (lib+bins) | `cargo check -p shared-rs -p relays-client -p searcher-rs -p prioritization-spine` | 0 errores, 0 warnings (43.65s) | **0** |
| cargo check all-targets (incluye tests+fixtures) | `… --all-targets` | 0 errores, 0 warnings | **0** |
| cargo clippy | `cargo clippy -p shared-rs -p relays-client -p searcher-rs -p prioritization-spine --all-targets -- -D warnings` | limpio (46.87s, tras el allow §1.D4) | **0** |
| cargo fmt | `cargo fmt -p …4 crates -- --check` | CLEAN (1 hunk auto-formateado por `cargo fmt` write: test L547 del engine, línea >100 cols) | **0** |
| cargo test searcher (comando del diseño) | `cargo test -p searcher-rs liquidation` | ok — 3 passed / 0 failed (lib, filtro) + los 6 binarios de integración compilan y corren (0 matching) — **Windows AppControl NO bloqueó** (dev profile) | **0** |
| cargo test searcher lib completa | `cargo test -p searcher-rs --lib` | **1150 passed / 0 failed / 3 ignored** (1.04s) — cubre los 3 tests del engine editados (L526/547/591), worker, size_optimizer y triangular_engine (fixtures D10-collateral) | **0** |
| cargo test shared-rs | `cargo test -p shared-rs` | **217 passed / 0 failed** (0.06s) + 1 doc/integración passed (9.41s) — cubre config.rs, trading_config.rs (sample_state + legacy-blob deserialización con el campo nuevo ausente → serde default) y price_oracle | **0** |
| cargo test prioritization-spine | `cargo test -p prioritization-spine` | **121 passed / 0 failed** (+2 ignored integración) — cubre config_aware + strategy_config_gate (fixtures D10-collateral) | **0** |
| cargo test relays-client | `cargo test -p relays-client` | **77 passed / 0 failed / 1 ignored** (0.18s) — cubre bundle_builder (tests del propio archivo; `build_and_sign` completo es integration-level deferred M5, diseño §(c).1) y submit_engine | **0** |

## 5. Declaraciones fail-honest (R8)

1. **Sin tests nuevos del lado Rust** — el diseño D15 marcaba el test de
   `resolve_gas_cost_usd` como "(Opcional Rust … si Oleada 4 no lo incluye,
   declararlo)": DECLARADO no incluido. Razón: parseo de env en test requiere
   mutación serializada de proceso (patrón ENV_LOCK de
   `shared-rs/src/config.rs:300`) — la validación del path negativo está
   cubierta por compilación (match exhaustivo) + el kernel ya endurecido
   (`estimate_liquidation_profit` devuelve None para gas no-finito/negativo,
   tests existentes `estimate_nan_inputs_return_none` /
   `estimate_inf_inputs_return_none` en el propio worker). El warn R8 del
   helper es observable en logs, no asertado por test.
2. **MIGRATION_HISTORY.md sin entrada para 119** — el diseño D11 no la
   incluye; precedente mixto en el repo (118 sin entrada, 120 con entrada).
   DECISIÓN-DEL-OPERADOR documentada pendiente; la nota de orden de deploy
   (migración 119 ANTES del deploy del api-server nuevo) vive en el diseño
   §Invariante y en `WO-04-APPLY-TS.md` §4.2.
3. **`app.toml` no lista la clave nueva** — agregar `priority_fee_gwei = 2.0`
   explícito al TOML es decisión de estilo del operador (default serde ya
   cubre). Ídem `LIQUIDATION_GAS_COST_USD` en el `.env` del searcher (VPS =
   territorio operador, §32/§33).
4. **gas_oracle.rs NO cableado** (quinta copia del tip, `executor/gas_oracle.rs:37`):
   el diseño lo declara fuera de Oleada 4 (gemelo no consumido). Intacto. Si
   algún día se cablea, DEBE consumir este mismo `AppConfig`.
5. **Fallback base fee 30 gwei** (`bundle_builder.rs:174-176`) — adyacente
   fuera de charter; intacto por diseño §Gemelos.

## 6. Restricciones cumplidas

- **CERO git write**: sin commit/push/PR (protocolo operador 2026-08-23);
  edición local + verificación solamente.
- **CERO VPS**, cero requests a dominio público (presupuesto HTTP: 0/5).
- **§34.3 INTACTO**: `live_exec_policy.rs`, default-deny, `MainnetRefused`
  sin tocar (`git status --porcelain` vacío para esos paths).
- **CERO archivos TS** tocados (mitad del applier TS; diff confinado a
  §1+§2 de este reporte).
- **RULE 00**: sin datos fabricados — defaults declarativos idénticos a los
  literales históricos; fail-fast documentado por plano.
- Diffs marcados `// WO-04 (2026-09-06)` en cada hunk de código.
- scanner.rs: hunks WO-02/WO-10 preservados (verificado por diff §1.D8).

## 7. Confinamiento del diff (git diff --stat, working tree, sin commit)

```
 backend/prioritization-spine/src/config_aware.rs        |  1 +
 backend/prioritization-spine/src/strategy_config_gate.rs|  1 +
 backend/relays-client/src/bundle_builder.rs             | 11 +++-
 backend/relays-client/src/submit_engine.rs              |  1 +
 backend/searcher-rs/src/engines/liquidation_engine.rs   | 35 ++++++-----
 backend/searcher-rs/src/engines/triangular_engine.rs    |  1 +
 backend/searcher-rs/src/main.rs                         |  1 +
 backend/searcher-rs/src/scanner.rs                      |  8 ++-
 backend/searcher-rs/src/size_optimizer.rs               |  1 +
 backend/searcher-rs/src/workers/liquidation_worker.rs   | 44 ++++++++---
 backend/searcher-rs/tests/cartridge_shadow_replay.rs    |  8 ++-
 backend/searcher-rs/tests/orchestrator_parallel_run.rs  |  8 ++-
 backend/searcher-rs/tests/v2_shadow_replay.rs           |  8 ++-
 backend/shared-rs/src/config.rs                         | 12 ++++
 backend/shared-rs/src/price_oracle.rs                   |  1 +
 backend/shared-rs/src/trading_config.rs                 | 19 ++++++
 configs/schemas/app.schema.json                         |  3 +-
 17 files changed, 139 insertions(+), 32 deletions(-)
```

(+ untracked: `database/migrations/119_trading_config_lp_fee_default_pct.sql`.
`.claude/settings.json` y submódulos contracts aparecen dirty en el tree
PRE-EXISTENTES a este apply — no son de WO-04.)

## 8. Pendientes del operador (no de agentes)

1. Orden de deploy L4: **migración 119 ANTES** del deploy del api-server con
   la mitad TS (el SELECT lista la columna; el zod default protege el PUT,
   no el SELECT). Ya documentado en `WO-04-APPLY-TS.md` §4.2.
2. Knobs listos para girar (sin acción = comportamiento idéntico):
   `configs/app.toml [execution] priority_fee_gwei` (2.0 default) ·
   env `LIQUIDATION_GAS_COST_USD` (30.0 default) · PUT admin
   `lp_fee_default_pct` (0.003 default, bounds 0–0.5).
3. Decisión: entrada en `MIGRATION_HISTORY.md` para la 119 (§5.2).
4. Follow-up natural del diseño (fuera de WO-04): hot-reload del gas-cost
   vía `trading_config` (alternativa descartada documentada) y consumo de un
   oráculo de tip en relays-client (jerarquía §(e).2 del diseño).

## Gates-addendum — veredicto final

**Los 9 gates EXIT 0.** Suma de tests PASSED sobre los 4 crates tocados:
**1566 / 0 failed** — searcher-rs lib **1150** (los 3 del filtro
`liquidation` son subconjunto del lib, no se suman dos veces; los binarios
de integración corren con 0 tests matching) + shared-rs **217+1** +
prioritization-spine **121** + relays-client **77** (ignored = pre-existentes,
no de WO-04). Windows AppControl 4551 **NO bloqueó** los
binarios de test (dev profile, target caliente del árbol principal §36.4) —
el fallback fail-honest documentado en el charter no fue necesario. Sin
necesidad de declarar APPLIED_UNVERIFIED: tests EJECUTADOS.

## Estado

**APPLIED_VERIFIED (local)** — mitad Rust de WO-04 completa según diseño
D1-D11 + colateral mínimo documentado (§2); invariante de primer deploy
preservada por construcción (defaults idénticos en los 4 planos de cada
literal); check/clippy/fmt/test EXIT 0 sobre los 4 crates tocados; §34.3 y
prohibiciones intactos; CERO git write. Deploy y giro de knobs = operador
(§8).
