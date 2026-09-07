# WO-07 — DESIGN: Port-back del writer de calibración bayesiana (Stage 2b/2c) del branch PR#472 a main

- **WO**: WO-07 · kind: **design** (READ-ONLY — cero edición de código de producción, cero git write, cero deploy)
- **Agente**: strategy-architect (Gang Omniscience §9) · 2026-09-06
- **Anomalía origen**: informe §2.6 — calibración bayesiana inerte: `math_operator_calibration` vacío, `flat_prior` eterno, 0 claves `*calib*` en Redis, 31 operadores matemáticos sin señal (board `GOAL-WORKORDERS.md:15`).
- **Lexicon**: Topological Yield (profit), Variedad de Liquidez (pool/DEX), TLS (flash loan), Decoherencia de Estado (slippage).

---

## 0. Línea base verificada (evidencia, no aserción)

Leído del VPS por ssh read-only (2026-09-06, `arbitragex-v2-postgres-1` / `arbitragex-v2-redis-1`):

| Hecho | Valor | Implicación |
|---|---|---|
| `SELECT COUNT(*) FROM math_operator_calibration` | **0** | store vacío — el §IV motor colapsa a `flat_prior` |
| `COUNT(*) paper_trade_runs WHERE actual_timestamp IS NOT NULL` | **0** | **cero labels Y** en TODO el historial |
| `COUNT(*) paper_trade_runs WHERE sim_block_number IS NOT NULL` | **0** | ni siquiera existen filas simuladas: el terminus `arbx:opps:simulated` nunca fluyó (consistente con `05-simulator-family.md:5` — 0 aprobadas en 1.000.718 filas) |
| `COUNT(*) paper_trade_runs WHERE calibration_eligible` | 598.878 | todas las filas históricas son default-TRUE (S4-03) |
| Redis `--scan --pattern '*calib*'` | **0 claves** | confirma informe §2.6 |

**Corolario estructural**: el drift-tracker (Y-oracle) hoy no tendría NADA que resolver aunque se encienda — su SELECT exige `sim_block_number IS NOT NULL` (`backend/recon/src/drift_tracker.rs:190`). La cadena labels→calibración está bloqueada aguas arriba en el flip del operador (P1-3), no en código de calibración. El writer que este WO diseña es la pieza DORMANTE que falta aguas abajo.

---

## 1. Qué contiene el branch PR#472 (fuente del port)

Branch local `feat/stage2-calibration-closure` (= PR#472, NO mergeado), 2 commits sobre el merge-base `20c93917` (post-#471):

- `113145e8` (2026-08-29 11:53 -0500) — `feat(calibration): STAGE2-A/B/C — drift-tracker Capa B + log-LR store writer + IV priors fold`
- `b8600895` (2026-08-29 12:06 -0500) — `fix(migration): 111 index build CONCURRENTLY` — toca **solo** `database/migrations/111_drift_tracker_backoff.sql` (verificado con `git show --stat`)

Diff completo `20c93917..feat/stage2-calibration-closure`: 12 archivos, +1023/−39.

### 1.1 El corazón estadístico (lo que hay que portar)

`backend/recon/src/stage2_calibration.rs` (blob `4c413ecd`, 393 líneas, 6 tests): consolidación recompute-from-source de labels `(evidence_vector, Y)` → store por operador con shrinkage jerárquico:

```
θ_k = (κ·θ₀ + wins_k) / (κ + n_k)        log_lr_k = logit(θ_k) − logit(θ₀)
```

- κ = 20 pseudo-eventos (default `ARBX_CALIBRATION_PRIOR_KAPPA=20`): operador con n=3 es ~87% prior; n=150 es ~88% empírico; **n=0 ⇒ log_lr=0** (LR=e⁰=1, contribución nula — estado honesto near-flat).
- Consolidación cada `ARBX_CALIBRATION_CONSOLIDATE_EVERY=100` labels NUEVAS (watermark = `MAX(calibrated_at)` = max `actual_timestamp` realmente consolidado — nunca salta una fila).
- Upsert idempotente de las 31 filas (`sample_count = EXCLUDED.sample_count` — recompute absoluto, inmune al counter-drift de la lección tally-board).
- OFF por default (`ARBX_STAGE2_CALIBRATION_MODE=off`).

`backend/searcher-rs/src/priors_cache.rs` (blob `94a5eed3`, 267 líneas, 6 tests): lado lectura (Stage 2c) — espeja el store en memoria cada `ARBX_PRIORS_REFRESH_SECS=30` y aplica el fold §IV en el hot-path de emisión vía la primitiva REAL que ya vive en main:

- `backend/searcher-rs/src/math_evidence.rs:393` — `evidence_posterior_log_odds(prior_log_odds, evidence, calibration) -> (f64, &'static str)` con `source_context = "calibrated" | "flat_prior"` (verificado en main actual).
- Semántica honesta: store vacío ⇒ `calibration() = None` ⇒ fold saltado ⇒ `posterior_log_odds = null`, `calibration_applied = false` en cada score record; PG caído ⇒ retiene el ÚLTIMO slice bueno (stale-pero-real > fabricado-vacío); PG no configurado ⇒ `disabled()`.

**Este par es el gap real**: el informe §2.6 constató que las primitivas §IV (`evidence_posterior_log_odds`) NO tenían call-site productivo — el emitter capturaba el vector pero jamás aplicaba calibración. Sin writer no hay store; sin consumidor el store sería invisible. El branch trae ambos.

---

## 2. Resolución de la colisión de migración 111 (GOTCHA CRÍTICO)

### 2.1 La colisión, en exacto

| | Branch PR#472 | main actual (aterrizado por #474/#475, S4 labels program) |
|---|---|---|
| Archivo | `111_drift_tracker_backoff.sql` | `111_paper_trade_runs_calibration_eligibility.sql` |
| Columnas backoff | `actual_attempt_count INT`, `actual_next_attempt_at TIMESTAMPTZ` | `sim_attempts INT`, `sim_last_attempt_at TIMESTAMPTZ` |
| Columnas S4-03 | — (no existían cuando se escribió) | `sim_fail_family TEXT CHECK(structural\|economic\|market)`, `calibration_eligible BOOL DEFAULT TRUE` |
| Índice | `idx_paper_trade_runs_next_attempt` (CONCURRENTLY, fix b8600895) | `idx_ptr_pending_calibration` (CONCURRENTLY, parcial) |
| Estilo DDL | `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` desnudo | DO-blocks catalog-guarded (rerun-lock-safety GEN-CI-FAIL 2026-08-30) |

**Semánticamente son el MISMO backoff con nombres distintos**: PR#472 defiere guardando `next_attempt_at` explícito; main calcula la deferral desde `sim_last_attempt_at < now() − 30·2^min(attempts,7)` (`drift_tracker.rs:198-202`) con techo `ARBX_DRIFT_TRACKER_MAX_ATTEMPTS=10`. Horarios equivalentes; el de main es el que YA ESTÁ DESPLEGADO y con taxonomía S4-02 integrada.

### 2.2 Resolución decidida (y por qué NO es "renombrar a 119")

**NO se porta migración alguna. Cero DDL nuevo.** El número 111 NO se re-numera a 119 porque no hay columnas que aportar: todo lo que el port toca ya existe en el schema desplegado de main:

| Superficie que Stage 2b/2c toca | Migración que la creó (main) | Evidencia |
|---|---|---|
| `math_operator_calibration (operator_id PK, log_lr, sample_count, calibrated_at)` | `103_math_evidence_scoring.sql:24` | leído en main |
| `scored_opportunities.evidence_vector JSONB` | `103_math_evidence_scoring.sql:21` | leído en main |
| `paper_trade_runs.actual_profit_usd / actual_timestamp / opportunity_id UUID` | `051_paper_trade_runs.sql` (+091/100) | `051:16-18` |
| Gate S4-03 `calibration_eligible` (consumidor) | `111_paper_trade_runs_calibration_eligibility.sql` | ya en VPS (598.878 filas TRUE) |

Las columnas huérfanas del branch (`actual_attempt_count`, `actual_next_attempt_at`) **jamás existieron en DB alguna** (el branch nunca se mergeó ni deployó — memory 2026-09-06: "writer κ=20/backoff = SOLO branch PR#472 sin merge"). No hay reconciliación física que hacer; la reconciliación es de DISEÑO:

1. **DROP del archivo** `database/migrations/111_drift_tracker_backoff.sql` del port — el commit b8600895 completo queda fuera (solo tocaba ese archivo).
2. **DROP del diff de backoff en `drift_tracker.rs`** (+123 líneas del branch): main ya tiene la versión S4-03 SUPERSET — 5 estados (`Resolved`, `NotPassed(family)`, `StructuralNotEligible`, `Pending`, `Failed`), taxonomía `shared_rs::sim_taxonomy`, gate `calibration_eligible`. Portar el backoff del branch sería un regression semántico (perdería S4-02/S4-03) y rompería columnas.
3. **NO se porta el diff de `cartridge_boot.rs`** (Gap 1 route_metadata, +87 líneas del branch): ya aterrizó en main SUPERIOR vía HOPS-EMIT-01 (#534, `9fdebb01`) + HOPS-LEDGER-04 (#537, `324a0beb`) — main construye el route desde los plan-legs ÚNICAMENTE con gate estructural documentado (`cartridge_boot.rs:1552-1580`), que corrige el enfoque merge-candidate+plan del branch (los pares aplanados del candidate NO son fuente válida). Los 9 sitios de emisión ya pasan `route_ref` (`cartridge_boot.rs:1368,1463,1588,1603,1637,1656,1664,1674`).
4. **Las 3 ediciones de reconciliación** de §3.1 alinean el writer con la semántica de labels que main SÍ tiene (y el branch no conocía).

**MIGRATION_HISTORY.md**: sin entrada nueva (no hay migración). Si el reviewer quiere rastro, una línea opcional en la sección de notas — no requerida.

---

## 3. Port-back: inventario exacto de archivos

### 3.1 Tabla de decisión

| # | Archivo (ruta en main) | Acción | Procedencia | ¿Editado vs branch? |
|---|---|---|---|---|
| 1 | `backend/recon/src/stage2_calibration.rs` | **PORT — archivo NUEVO** (393 ln + 6 tests) | blob `4c413ecd` | **SÍ — 3 ediciones de reconciliación (§3.2)** |
| 2 | `backend/searcher-rs/src/priors_cache.rs` | **PORT — archivo NUEVO** (267 ln + 6 tests) | blob `94a5eed3` | NO — verbatim |
| 3 | `backend/recon/src/main.rs` | PORT — 2 hunks (mod + spawn) | `113145e8` | NO — verbatim (contexto en main idéntico, verificado `recon/src/main.rs:318-352`) |
| 4 | `backend/searcher-rs/src/lib.rs` | PORT — 1 hunk (`pub mod priors_cache;`) | `113145e8` | NO — verbatim |
| 5 | `backend/searcher-rs/src/main.rs` | PORT — 1 hunk (`mod priors_cache;`) | `113145e8` | NO — verbatim |
| 6 | `backend/searcher-rs/src/opportunity_emitter.rs` | PORT — fold + 2 campos wire + 2 tests | `113145e8` | NO — verbatim (archivo sin drift main↔merge-base, verificado) |
| 7 | `backend/api-server/src/routes/scored-opportunities-archiver.ts` | PORT — 2 campos Zod | `113145e8` | NO — verbatim |
| 8 | `.env.example` | PORT — SUBSET | `113145e8` | **SÍ — se excluyen 2 knobs de backoff (§3.4)** |
| 9 | `README.md` | PORT OPCIONAL (tabla Stage 2a/2b/2c) | `113145e8` | adaptación menor (§3.5) |
| — | `backend/recon/src/drift_tracker.rs` | **NO PORT** (supersede S4-03 en main) | — | — |
| — | `backend/searcher-rs/src/cartridge_boot.rs` | **NO PORT** (supersede HOPS #534/#537) | — | — |
| — | `database/migrations/111_drift_tracker_backoff.sql` | **NO PORT** (colisión §2 — DROP) | — | — |

### 3.2 Ediciones de reconciliación sobre `stage2_calibration.rs` (diffs exactos)

El archivo se toma verbatim del blob `4c413ecd` (`git show feat/stage2-calibration-closure:backend/recon/src/stage2_calibration.rs`) y se aplican EXACTAMENTE estas 3 ediciones, marcadas `// WO-07 (2026-09-06)`:

**Edición 1 — doc-semántica de labels (el branch escribió "solo re-exec passing"; main S4-03 también etiqueta el rechazo ECONOMIC/MARKET con Y=0 exacto):**

```diff
--- a/backend/recon/src/stage2_calibration.rs   (blob 4c413ecd)
+++ b/backend/recon/src/stage2_calibration.rs   (port WO-07)
@@
-//! - A label exists ONLY where `actual_profit_usd IS NOT NULL` (the
-//! drift-tracker writes it solely on a passing re-exec). Win ⇔ > 0.
+//! - A label exists ONLY where `actual_profit_usd IS NOT NULL`. On main
+//! (S4-03) the drift-tracker writes terminal labels on TWO paths: a PASSING
+//! re-exec (realized Topological Yield) and an ECONOMIC/MARKET reject
+//! (`actual_profit_usd = 0` EXACTLY — the market rejected the trade at the
+//! settled block; a rejected execution realized nothing). Win ⇔ > 0 covers
+//! both honestly. STRUCTURAL rows never carry a label
+//! (`calibration_eligible = false`).
+//! // WO-07 (2026-09-06): reconciled with main's S4-03 label semantics.
```

**Edición 2 — gate S4-03 en el query de consolidación (no-contaminación defense-in-depth: el consumidor también excluye filas estructurales, no solo el scan del tracker):**

```diff
@@ async fn consolidate(db: &PgPool, cfg: &Stage2Config) -> anyhow::Result<()> {
         ) lat ON true
         WHERE ptr.actual_timestamp IS NOT NULL
           AND ptr.actual_profit_usd IS NOT NULL
+          AND ptr.calibration_eligible
+          -- WO-07 (2026-09-06): S4-03 no-contamination gate at the consumer —
+          -- a structurally-failed fixture must never feed the priors even if
+          -- a label ever landed on it (defense-in-depth vs drift_tracker.rs:192).
         "#,
```

**Edición 3 — mismo gate en el conteo de watermark del `tick()` (coherencia: lo que cuenta hacia `consolidate_every` es lo mismo que el fold consume):**

```diff
@@ async fn tick(db: &PgPool, cfg: &Stage2Config) -> anyhow::Result<()> {
         SELECT COUNT(*)
         FROM paper_trade_runs
         WHERE actual_timestamp IS NOT NULL
           AND actual_timestamp > $1
+          AND calibration_eligible
+          -- WO-07 (2026-09-06): count exactly what consolidate() folds (S4-03).
         "#,
```

Todo lo demás — κ, watermark, upsert UNNEST, clamp θ, los 6 tests unitarios puros — queda verbatim.

### 3.3 Hunks verbatim del port (referencia exacta)

Cada hunk es recuperable con `git diff 20c93917..feat/stage2-calibration-closure -- <ruta>`; se reproducen los críticos:

**`backend/recon/src/main.rs`** (contexto main verificado en `main.rs:318-352` — aplica limpio):

```diff
@@ recon/src/main.rs
 mod consumer;
 mod drift_tracker;
 mod persistence;
 mod pnl_engine;
+mod stage2_calibration;
 mod variance;
@@ (dentro del spawn del drift-tracker, tras `let ks_drift = killswitch.clone();`)
+                // Clones for the Stage 2b calibration job (log-LR store writer).
+                let db_for_stage2 = db_for_aggregator.clone();
+                let ks_stage2 = killswitch.clone();
@@ (tras el bloque `drift_tracker.dormant`)
+                // Stage 2b: calibration job — consolidates the drift-tracker's
+                // Y-labels + archived evidence vectors into the per-operator
+                // log-LR store (`math_operator_calibration`) every N new labels
+                // (hierarchical shrinkage; the §IV motor's `calibrated`
+                // source_context). OFF by default.
+                let stage2_mode = std::env::var("ARBX_STAGE2_CALIBRATION_MODE").unwrap_or_default();
+                if stage2_mode == "on" {
+                    let stage2_cfg = stage2_calibration::Stage2Config::from_env();
+                    tokio::spawn(async move {
+                        stage2_calibration::run_periodic(db_for_stage2, ks_stage2, stage2_cfg)
+                            .await;
+                    });
+                } else {
+                    info!(event = "stage2_calibration.dormant", mode = %stage2_mode);
+                }
```

**`backend/searcher-rs/src/lib.rs`**:

```diff
@@ searcher-rs/src/lib.rs
 pub mod pool_candidate;
 pub mod pool_discovery;
 pub mod pool_sources;
+// Stage 2c (§IV read side): per-operator log-LR cache + the posterior fold.
+pub mod priors_cache;
 pub mod publisher;
```

**`backend/searcher-rs/src/main.rs`**:

```diff
@@ searcher-rs/src/main.rs
 mod metrics;
 mod patterns;
 mod persistence;
+mod priors_cache;
 pub mod publisher; // (nota: en bin es `mod publisher;` — hunk exacto del branch)
```

**`backend/searcher-rs/src/opportunity_emitter.rs`** — 5 hunks verbatim del branch (archivo sin drift en main):
1. `use crate::priors_cache::{section_iv_fold, PriorsCache};` + campo `priors: PriorsCache` en `OpportunityEmitter` (`opportunity_emitter.rs:126` área).
2. Constructor real: `let priors = PriorsCache::spawn_opt(&pool);` (el campo `pool: Option<PgPool>` existe en main, `opportunity_emitter.rs:113`).
3. Constructor dry-run/shadow: `priors: PriorsCache::disabled()` ("No PG in dry-run — the §IV fold stays honest-null").
4. En `score_and_publish` (~`opportunity_emitter.rs:493-516`, tras capturar `evidence_vector`): `let calibration = self.priors.calibration(); let fold = section_iv_fold(score.prior_log_odds, &evidence_vector, &calibration);` y `fold` como nuevo arg de `build_score_record`. Incluye el comentario ampliado de por qué el Beta-side prior sigue `None` (schema `bayesian_priors` keyed `token_pair UNIQUE` pre-STRAT-IDENT-01 — alimentarlo re-introduciría el colapso de identidad; el surface §IV per-operator ES el que tiene writer).
5. `build_score_record(..., fold: crate::priors_cache::SectionIvFold)` + 2 claves nuevas en el JSON:

```diff
         "bayesian_accepted": score.bayesian_accepted,
         "prior_log_odds": score.prior_log_odds,
+        "posterior_log_odds": fold.posterior_log_odds,
+        "calibration_applied": fold.calibration_applied,
         "chain_id": chain_id_i64,
```

más los 2 tests nuevos/extendidos (`score_record_accepted_variant_has_null_rejection_reason` extendido con assertions `posterior_log_odds.is_null()` + `calibration_applied == false`; test nuevo `score_record_calibrated_fold_is_carried`).

**`backend/api-server/src/routes/scored-opportunities-archiver.ts`** (schema Zod — `.passthrough()` NO se usa, campos desconocidos se DROPEAN; sin este hunk los 2 campos nuevos se perderían silenciosamente en el archivo — XLANG-01):

```diff
@@ api-server/src/routes/scored-opportunities-archiver.ts (tras prior_log_odds, línea ~58)
   prior_log_odds: z.number().finite().nullable().optional(),
+  /**
+   * §IV fold (Stage 2c): prior_log_odds + Σ (log_lr_k · e_k) from the
+   * calibrated per-operator store (`math_operator_calibration`, mirrored by the
+   * searcher's PriorsCache). null when either side (evidence snapshot,
+   * calibration slice) is absent. NOT persisted yet — no PG column; parsed to
+   * keep the Rust↔Zod wire contract explicit (XLANG-01).
+   */
+  posterior_log_odds: z.number().finite().nullable().optional(),
+  /** §IV: true when any |log_lr_k| > ε participated in the fold. Not persisted. */
+  calibration_applied: z.boolean().optional(),
   chain_id: z.number().int().nullable().optional(),
```

### 3.4 `.env.example` — SUBSET (diff exacto)

Se portan SOLO los knobs del writer/lector. **Se excluyen** `ARBX_DRIFT_TRACKER_BACKOFF_BASE_SECS` / `ARBX_DRIFT_TRACKER_BACKOFF_MAX_SECS` del branch — el backoff de main es el S4-03 (`30s·2^min(n,7)` hardcodeado + `ARBX_DRIFT_TRACKER_MAX_ATTEMPTS`, `drift_tracker.rs:53-64`); introducir knobs duales de un backoff muerto confunde al operador. `// WO-07 (2026-09-06)` en el bloque nuevo:

```diff
--- a/.env.example
+++ b/.env.example
@@ (tras ARBX_SCORING_HARD_GATE=false, línea ~196)
 ARBX_SCORING_HARD_GATE=false
+# Stage 2c — §IV priors cache: how often the searcher re-mirrors the per-operator
+# log-LR store (math_operator_calibration, written by recon's stage2_calibration
+# job) that feeds the §IV posterior fold in the scoring record.
+# WO-07 (2026-09-06): port-back PR#472 (feat/stage2-calibration-closure).
+ARBX_PRIORS_REFRESH_SECS=30
@@ (tras ARBX_DRIFT_TRACKER_SETTLE_LEAD_SECS=15, línea ~306 — ANTES de SIMCTL_URL)
 ARBX_DRIFT_TRACKER_SETTLE_LEAD_SECS=15
+
+# ------------------ Stage 2b: log-LR calibration job (§IV store writer) -------
+# Consolidates the drift-tracker's Y-labels + scored_opportunities.evidence_vector
+# into math_operator_calibration (per-operator log-LR, shrinkage κ=20 pseudo-
+# events) every N newly labeled events. This is what flips source_context from
+# flat_prior to calibrated — from REAL labeled data only (RULE 00). OFF by
+# default; enable together with the drift-tracker once labels flow.
+# Dependency: P1-3 (SIM_BACKEND=revm flip, operator-only) MUST be live first —
+# without passed sims there are no labels, and without labels this job writes
+# NOTHING (invariant §4).
+# WO-07 (2026-09-06): port-back PR#472.
+ARBX_STAGE2_CALIBRATION_MODE=off
+ARBX_STAGE2_CALIBRATION_INTERVAL_SECS=60
+ARBX_CALIBRATION_CONSOLIDATE_EVERY=100
+ARBX_CALIBRATION_PRIOR_KAPPA=20
 SIMCTL_URL=http://sim-ctl:3003
```

Nota de colisión de namespace: `ARBX_CALIBRATION_MIN_SCORED=100` / `ARBX_CALIBRATION_MIN_OBSERVATIONS=30` ya existen en `.env.example:202-203` — son del dashboard A.5 (consume `backend/api-server/src/routes/scoring-status.ts:137`) y NO colisionan con `ARBX_CALIBRATION_CONSOLIDATE_EVERY` / `ARBX_CALIBRATION_PRIOR_KAPPA` (prefijos distintos, consumidores distintos). Documentado aquí para que el reviewer no los confunda.

### 3.5 `README.md` — OPCIONAL (doc-honestidad)

Hunk del branch adaptable: filas 2b/2c de la tabla Stage. En main la fila "Gap 1" ya está resuelta (HOPS #534) — el texto del port debe decir `✅ (closed by HOPS-EMIT-01 #534)`, no `🟡`. Se incluye como edición del apply-WO solo si el reviewer lo aprueba (P-∅: un PR = un ID; este WO es calibración, el README es documentación del mismo port — aceptable dentro del mismo ID).

---

## 4. INVARIANTE (R8): cero escrituras `*calib*` hasta que existan labels reales

**Enunciado**: hasta que ≥100 labels Y reales (`actual_timestamp IS NOT NULL AND calibration_eligible`) existan en `paper_trade_runs`, el sistema NO escribe una sola fila en `math_operator_calibration` ni emite señal calibrada alguna. La calibración NO puede fabricar señal con flat prior eterno.

**Verificación formal por capas** (cada una independiente — fail-closed en cascada):

| # | Capa | Mecanismo exacto | Evidencia file:line (post-port) |
|---|---|---|---|
| 1 | Job no spawneado | `ARBX_STAGE2_CALIBRATION_MODE=off` default → rama `stage2_calibration.dormant` | `recon/src/main.rs` (hunk §3.3); `.env.example` port |
| 2 | Umbral de disparo | `tick()`: `new_labels < consolidate_every (100)` → `stage2_calibration.waiting`, return — **cero writes** | `stage2_calibration.rs::tick` (blob 4c413ecd) |
| 3 | Guard de vacuidad | `consolidate()`: `total_n == 0` → `stage2_calibration.skipped_no_pairs`, store intacto | `stage2_calibration.rs::consolidate` |
| 4 | Matemática anti-fabricación | operador n=0 ⇒ `log_lr=0` (LR=1, contribución nula); shrinkage κ=20: n=3/w=3 vs θ₀=0.5 ⇒ θ=13/23≈0.565 (log_lr≈+0.26 nats — 3 eventos NO son evidencia); clamp θ∈[1e-4, 1−1e-4] acota \|log_lr\|≲9.2 | tests `shrinkage_prior_dominates_sparse_operator`, `zero_n_operator_contributes_nothing`, `all_wins_clamps_to_finite_log_lr` |
| 5 | Labels reales solamente | `actual_*` los escribe EXCLUSIVAMENTE el drift-tracker tras re-execución real vía sim-ctl en el bloque asentado (PASS → yield realizado; ECONOMIC/MARKET → 0 exacto; STRUCTURAL → sin label, `calibration_eligible=false`; PENDING ≠ label — ausencia de veredicto jamás se imputa) | `drift_tracker.rs:285-418` (main, S4-03); gate del consumidor Edición 2 |
| 6 | Lectura honesta | store vacío ⇒ `PriorsCache::calibration() = None` ⇒ fold saltado ⇒ cada score record lleva `posterior_log_odds: null, calibration_applied: false` — **la honestidad es visible en el wire** | `priors_cache.rs::section_iv_fold` + test `fold_absent_calibration_is_honest_none` |
| 7 | Contrato wire estricto | Zod sin `.passthrough()` — los 2 campos existen SOLO porque el hunk §3.3 los declara; ausencia = drop, nunca invención | `scored-opportunities-archiver.ts:49-51` (main) |

**Superficie observable del invariante** (para el verifier L4 post-deploy del apply-WO):
- PG: `SELECT COUNT(*) FROM math_operator_calibration` — debe seguir **0** mientras `COUNT(paper_trade_runs WHERE actual_timestamp IS NOT NULL AND calibration_eligible) < 100`.
- Redis: `--scan --pattern '*calib*'` — permanece 0 (el store es PG; PriorsCache es memoria de proceso — NO introduce claves Redis).
- Wire: score records con `posterior_log_odds: null` y `source_context: "flat_prior"` mientras el store esté vacío.
- Logs recon: `stage2_calibration.dormant` (flag off) o `stage2_calibration.waiting` (flag on, labels<100). NUNCA `stage2_calibration.consolidated` sin labels previos.

**Anti-patrón cubierto explícitamente**: no hay path que escriba log_lr≠0 sin datos — ni default, ni seed, ni bootstrap. El único camino al store es `consolidate()` y este exige pares (e,Y) reales.

---

## 5. GATES de verificación (apply-WO)

Todo LOCAL (Windows, `target/` caliente §36.4 — NO worktree frío). **Cero git/deploy en este WO** (protocolo operador 2026-08-23; NO-GIT-FINAL-GATE).

1. **Compilación**: `cargo check -p recon -p searcher-rs` (workspace) — verde.
2. **Tests unitarios nuevos**: `cargo test -p recon stage2` → 6/6 (shrinkage, zero-n, log-LR positivo, clamp, logit simetría, θ shrink denso); `cargo test -p searcher-rs priors_cache` → 6/6 (fold honesto None×2, all-zero computable, shift con calibración, cache disabled/roundtrip); emitter: los 3 tests de record (2 extendidos + 1 nuevo).
3. **Lint**: `cargo clippy --workspace` + `cargo fmt --check` (los hunks del branch ya pasaban fmt en su día; las ediciones WO-07 respetan formato).
4. **TS**: `tsc --noEmit` del workspace api-server (hunk Zod es aditivo-optional — sin cambios de tipos requeridos por consumidores).
5. **CI (14 required checks)** al abrir el PR del apply-WO: contract tests + paridad + guardian smoke — sin impacto esperado (campos aditivos, flag off).
6. **Invariante §4 verificado post-deploy** (del apply-WO, con drift-tracker y stage2 aún OFF): `math_operator_calibration` = 0 filas; score records con `posterior_log_odds: null`.
7. **NO-GIT**: este WO no commitea nada. El apply-WO (agente applier Rust, serie) ejecuta los pasos 1-5 y abre PR con ID WO-07 + anomalía informe §2.6.

---

## 6. Dependencia explícita: orden P1-3 → WO-07 (flips = operador)

### 6.1 La cadena causal completa (cada eslabón con su flip)

```
[P1-3 · OPERADOR §34.3]  SIM_BACKEND=revm + REVM_RPC_URL (RPC DEDICADO de pago)
        │                  en .env VPS + cableado en docker/compose.prod.yml
        │                  (hoy NO cableados en ningún docker/*.yml — 05-simulator-family.md:45)
        ▼
  primer `passed=t` en `simulations`  +  XLEN arbx:opps:simulated > 0
        ▼
  terminus consumer → paper_trade_runs CON sim_block_number          [hoy: 0 filas — §0]
        ▼
[OPERADOR]  ARBX_DRIFT_TRACKER_MODE=on  (S4-03 ya desplegado; 5 estados)
        ▼
  labels reales: PASS → actual_profit_usd>0 · ECONOMIC/MARKET → =0 · STRUCTURAL → ineligible
        ▼
[OPERADOR]  ARBX_STAGE2_CALIBRATION_MODE=on   ← ESTE WO-07 (writer)
        ▼
  ≥100 labels → primera consolidación → math_operator_calibration ≠ vacío
        ▼
  PriorsCache.updated → score records con posterior_log_odds ≠ null,
  calibration_applied=true, source_context="calibrated"   ← fin del flat_prior eterno
```

### 6.2 Orden de ejecución de los WOs

- **El CÓDIGO de WO-07 puede aterrizar ANTES que P1-3** — es 100% dormante (3 flags OFF: mode del job, drift-tracker, y el fold se auto-desactiva con store vacío). Aterrizarlo antes es lo SEGURO: cuando lleguen las labels, el writer ya está auditado. **Recomendación: apply-WO inmediato, flips después.**
- **La ACTIVACIÓN de WO-07 exige P1-3 vivo** — sin `passed=t` no hay labels (§0: hoy 0), y sin labels el invariante §4 mantiene el store en 0 por diseño. Encender `ARBX_STAGE2_CALIBRATION_MODE=on` hoy produciría únicamente `stage2_calibration.waiting` cada 60s (debug log) — honesto e inofensivo, pero sin sentido operativo.
- **Sin WO-07, P1-3 produce labels que nadie consume**: los 31 operadores seguirían sin señal aunque todo el pipeline simulara (el informe §2.6 es exactamente ese gap). Por eso el roadmap fija "labels que alimenten calibración (P1-3/4)" como la pareja que cierra la brecha (`00-PREDATOR-ROADMAP.md:210`).
- Advertencia heredada del round-table: el flip P1-3 ejercerá POR PRIMERA VEZ el path consumer→persistence→paper_trade_runs del terminus (runtime nunca probado, 0 entregas históricas — `06-exec-terminus-CROSS.md:52`) — la ventana post-flip debe observar ese path, no solo sim-ctl.

### 6.3 Límites doctrinales del flip

- Flips (`SIM_BACKEND`, `ARBX_DRIFT_TRACKER_MODE`, `ARBX_STAGE2_CALIBRATION_MODE`) = **operador-only**, jamás inferidos de flags ni chat (§34.3 análogo; precedente `arbx-live-flip-chat-refused`).
- El writer es read/compute-only sobre datos de simulación paper: no toca executor, wallets, firma ni broadcast (§32/§33 intactos). Sin riesgo de capital.
- `arbx-simulation-mandatory` + `arbx-paper-trade-first` aplican al flip P1-3, no a este port (que no cambia ninguna ruta de ejecución).

---

## 7. Riesgos residuales y mitigaciones

| Riesgo | Severidad | Mitigación en el diseño |
|---|---|---|
| Reintroducir columnas duales de backoff (colisión 111) al hacer cherry-pick mecánico | ALTA | §2.2: port = 113145e8 **menos** migration/drift_tracker/cartridge_boot/backoff-knobs. El cherry-pick ciego está PROHIBIDO en el apply-WO; se aplica por hunks según tabla §3.1 |
| Semántica de labels divergente (branch: solo-PASS vs main S4-03: PASS + rechazo-económico/market con Y=0) | MEDIA | Edición 1 (doc) + verificación de que `win = y > 0.0` cubre ambas; gate `calibration_eligible` (Ediciones 2-3) |
| Primer post-P1-3: 100 labels podrían llegar sesgadas al rechazo económico (el flood XEN/AGLD es 100% rejected — taxonomy 2026-09-06) | MEDIA | DOCUMENTADO, no bloqueante: θ₀ pooled absorbe el base-rate y el shrinkage κ=20 mantiene a los operadores cerca del flat hasta n≫κ. Follow-up (fuera de WO-07): estratificar θ₀ por `sim_fail_family` cuando haya datos |
| Escalar e_k ≠ bool amplifica contribución (caveat #3 del branch) | BAJA | Ya documentado en el módulo (shrinkage + clamp acotan); follow-up logistic-weighted con datos reales |
| `actual_profit_usd` NULL en PASS sin precio USD (unvalued) | BAJA | Ya contado como `unvalued` en el log y excluido del fold — nunca imputado (R8) |
| Contrato Rust↔Zod (XLANG-01): campos dropeados silenciosamente | BAJA | Hunk §3.3 Zod incluido en el mismo port; archiver tests corren en CI |

---

## 8. Trazabilidad

- Fuente: branch local `feat/stage2-calibration-closure` (refs verificadas read-only; CERO checkout/commit — protocolo NO-GIT).
- Commits: `113145e80e18bf18e702d1d89b417f519f7ab6b6` (core) + `b8600895f817db0a2afc536b08eb95960d6a385e` (solo migración — DROP del port).
- Blobs exactos: `stage2_calibration.rs` = `4c413ecd11abff8d2c0765fff4a31e618beb08a1`; `priors_cache.rs` = `94a5eed33b78a47b93eb9fe282af439eda2db007`.
- Merge-base con main: `20c93917` (post-#471). Drift main↔merge-base en archivos portados: SOLO `drift_tracker.rs` y `cartridge_boot.rs` (ambos NO-PORT por supersede) — los 8 archivos del port están sin drift o son nuevos.
- Evidencia VPS read-only (ssh `arbx`, 1 sesión): §0.
- Presupuesto HTTP dominio público: **0/5 requests usados** (toda la evidencia es git-local + ssh-readonly).
- Board: `GOAL-WORKORDERS.md:15` (WO-07 PENDIENTE → este design habilita el apply).
- Cross-refs: `05-simulator-family.md:5,36,45` (P1-3/D-6), `05-simulator-family-CROSS.md:95` (flip P0), `00-PREDATOR-ROADMAP.md:121,210` (orden P1-3→labels→calibración), `06-exec-terminus-CROSS.md:52` (path terminus nunca ejercido).

**Veredicto del diseño**: VIABLE. Port quirúrgico de 8 archivos (2 nuevos, 6 hunks), cero migraciones, cero cambios de comportamiento runtime (todo dormante), colisión 111 resuelta por DROP-CONCIENTE (no por renombrado), invariante R8 verificado por 7 capas independientes, dependencia P1-3→WO-07 documentada con cadena causal completa y flips delegados al operador.
