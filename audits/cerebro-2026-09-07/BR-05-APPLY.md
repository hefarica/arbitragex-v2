# BR-05 — APPLY: activar la calibración (port-back WO-07: Stage 2b writer + Stage 2c §IV fold)

- **WO**: BR-05 · kind: **apply** · **Agente**: ecc:rust-reviewer (Gang Omniscience, IA OMEGA)
- **Fecha**: 2026-09-07/08 · **NO-GIT** (0 commits/push/PR/deploy) · **VPS**: 0 sesiones (baseline ya
  verificada por pares — §1) · **HTTP dominio público**: 0/5 requests.
- **Charter**: APPLY del port-back WO-07 YA VERIFICADO (`audits/omniscience-integration-2026-09-06/WO-07-DESIGN.md`,
  veredicto VIABLE). Portar POR HUNKS según tabla §3.1 — cherry-pick ciego PROHIBIDO (colisión
  migración 111 resuelta por DROP consciente). 3 ediciones de reconciliación exactas. Flips =
  OPERADOR-ONLY §34.3 — este apply solo aterriza código DORMANTE.
- **Lexicon**: Topological Yield (profit) · Variedad de Liquidez (pool) · Decoherencia de Estado.

## 0. Veredicto: **APPLIED + VERIFICADO local (check/tests/clippy/fmt/tsc)** — flips y PR = orquestador/operador

| Entregable (tabla §3.1 del diseño) | Estado | Evidencia |
|---|---|---|
| 1. `recon/src/stage2_calibration.rs` — NUEVO (393 ln blob + reconciliación) | ✅ | blob `4c413ecd` verbatim + 5 ediciones (§3) → 414 ln, 6 tests |
| 2. `searcher-rs/src/priors_cache.rs` — NUEVO (267 ln) | ✅ | blob `94a5eed3` BYTE-IDÉNTICO (`git hash-object` = hash citado), 6 tests |
| 3. `recon/src/main.rs` — mod + clones + spawn dormante | ✅ | `main.rs:18,327,369-381` |
| 4. `searcher-rs/src/lib.rs` — `pub mod priors_cache;` | ✅ | `lib.rs:80-82` (hunk CB-02 ajeno preservado) |
| 5. `searcher-rs/src/main.rs` — `mod priors_cache;` | ✅ | `main.rs:86` (diff total +27 = 1 mío + 3 CB-02 + 23 CB-02 census — ajenos intactos) |
| 6. `searcher-rs/src/opportunity_emitter.rs` — §IV fold + 2 campos wire + 3 tests | ✅ | `:35` import, `:132-137` campo, `:159-163` spawn_opt, `:187-189` disabled, `:554-556` fold, `:704-705` wire, `:928-946` tests |
| 7. `api-server/.../scored-opportunities-archiver.ts` — 2 campos Zod (XLANG-01) | ✅ | `:59-68` |
| 8. `.env.example` — SUBSET (6 knobs; 2 knobs de backoff EXCLUIDOS) | ✅ | `:196-200,308-326` |
| — `drift_tracker.rs` / `cartridge_boot.rs` / `111_drift_tracker_backoff.sql` | **NO PORTADOS (DROP consciente)** | `git status` de los 3 = limpio; migración 111 del árbol = SOLO la de main (`111_paper_trade_runs_calibration_eligibility.sql`) |
| README.md (§3.5 opcional) | OMITIDO | fuera del claim del charter (P-∅ quirúrgico); nota §6.3 |

## 1. Sincronía de mesa redonda (leído ANTES de editar, construido SOBRE los pares)

- **Board completo**: `GOAL-WORKORDERS.md` (BR-05 = "Apply del diseño verificado + flip wiring
  (activación = operador si aplica)").
- **`BR-05+06-VERIFY.md` (math-validator)** — pre-verificó la matemática al 100% y me dejó 3
  acciones que INCORPORÉ TODAS:
  - **MINOR-1** (cota del clamp): el blob decía "log-LR bounded ±~9.2" — CORRECTO es
    \|log_lr\| ≤ 2·ln(9999) ≈ **18.42** (9.21 unilateral; contraejemplo θ₀ pineado ⇒ +10.82).
    Corregido en `stage2_calibration.rs:79-83`.
  - **MINOR-2** (predicado del watermark): Edición 3 completada con
    `AND actual_profit_usd IS NOT NULL` para que el disparador cuente EXACTAMENTE lo que el fold
    consume (filas unvalued ya no disparan re-consolidaciones redundantes). `:168-177`.
  - **MINOR-4** (errata blob-hash `94a5aed3`): confirmada errata — `git hash-object` del archivo
    extraído = `94a5eed33b78…` EXACTO (verificado §4).
- **`BR-03+04+07-VERIFY.md` (ecc:security-reviewer)** — documentó BR-04 BLOCK/ausente
  (`flood_gate.rs` no existe). Mi charter decía "CORRE DESPUÉS de BR-04 (comparten
  opportunity_emitter.rs)": **BR-04 NO aterrizó** — llegué primero al emitter sobre el archivo
  limpio en HEAD. AVISO al builder BR-04: debe rebasar sobre mi diff (el emitter ahora tiene
  campo `priors` en el struct + `fold` como 8º arg de `build_score_record`).
- **`BR-02-APPLY.md` (ecc:rust-reviewer, mi encarnación previa)** — baseline de tests que no
  puedo romper: `cargo test -p searcher-rs --lib` 1158 passed / 3 ignored. Hoy: **1166/0/3**
  (+8 = +7 BR-05 [6 priors_cache + 1 emitter] + 1 de un par paralelo — el árbol es canvas
  compartido; `orchestrator.rs` mostró modificación concurrente durante mi corrida, claim ajeno
  no tocado).
- **`BR-00-REVERIFY.md`** — PASS-WITH-ADVISORIES del gate estructural sim; sin overlap con mis
  archivos (sim-ctl vs recon/searcher/api-server archiver).

## 2. Fuente del port (verificación de alcanza y exactitud)

```
git cat-file -e 4c413ecd11abff8d2c0765fff4a31e618beb08a1   → OK
git cat-file -e 94a5eed33b78a47b93eb9fe282af439eda2db007   → OK  (design §8; errata MINOR-4 desmentida)
git cat-file -e 113145e80e18bf18e702d1d89b417f519f7ab6b6   → OK  (commit core; b8600895 = solo migración — DROP)
git show <blob> > <destino>; git hash-object <destino>     → hash IDÉNTICO pre-edición (los 2)
```

Merge-base 20c93917. Diferencia vs diseño §3.1 fila 6: el emitter SÍ tuvo drift main↔merge-base
(WO-10 latency spans, ~40 ln) — por eso el port fue **a mano por hunks contra el archivo actual**
(no `git apply`), exactamente lo que el charter exige. Los hunks de lib.rs/main.rs/recon-main
aplicaron con contexto idéntico + hunks ajenos (CB-02) respetados.

## 3. Ediciones de reconciliación sobre el blob `4c413ecd` (diseño §3.2 + MINORs)

| # | Edición | file:line |
|---|---|---|
| 1 | Doc-semántica labels S4-03: PASS→yield real, ECONOMIC/MARKET→Y=0 EXACTO, STRUCTURAL→`calibration_eligible=false` (el branch solo conocía el path PASS) | `stage2_calibration.rs:56-63` |
| 2 | Gate `AND ptr.calibration_eligible` en `consolidate()` (no-contaminación defense-in-depth vs `drift_tracker.rs:192`) | `:217-222` |
| 3 | Gate `AND calibration_eligible AND actual_profit_usd IS NOT NULL` en `tick()` — cuenta EXACTAMENTE lo que el fold consume (Edición 3 del design + MINOR-2) | `:168-177` |
| M1 | Cota del clamp corregida: ±9.21 por logit ⇒ \|log_lr\| ≤ 2·ln(9999) ≈ 18.42 general | `:79-83` |

Todo lo demás (κ=20, watermark `MAX(calibrated_at)`, upsert UNNEST idempotente, clamp θ, 6 tests)
queda verbatim del blob. `priors_cache.rs` = 100% verbatim (tabla §3.1 fila 2: "NO — verbatim").

## 4. Verificación local (target caliente, árbol principal — NO worktree, §36.4)

| Gate | Comando | Resultado |
|---|---|---|
| Compilación | `cargo check -p recon -p searcher-rs` | **PASS** (4m10s; lib+bins ambos crates) |
| Tests writer | `cargo test -p recon stage2` | **6/6** (shrinkage prior/dense, zero-n, log-LR positivo, clamp, simetría logit) |
| Tests reader | `cargo test -p searcher-rs --lib priors_cache` | **6/6** (fold honesto None×2, all-zero computable, shift con calibración, disabled, roundtrip) |
| Tests emitter | `cargo test -p searcher-rs --lib score_record` | **3/3** (2 extendidos + 1 nuevo `score_record_calibrated_fold_is_carried`) |
| Suite completa | `cargo test -p searcher-rs --lib` | **1166 passed / 0 failed / 3 ignored** (baseline BR-02: 1158/3 — cero regresión) |
| Lint | `cargo clippy -p recon -p searcher-rs --lib --bins` | **PASS, 0 warnings** (22m18s — contención de build-lock con agentes hermanos compilando en paralelo, precedente BR-02 §5.1) |
| fmt | `rustfmt --edition 2021 --check` sobre los 6 archivos Rust | **0 diffs** (standalone = misma config que cargo fmt: no hay rustfmt.toml, defaults) |
| TS | `npm run typecheck` (api-server, tsc --noEmit) | **PASS** (0 errores) |
| TS tests | `npx vitest run scored-opportunities-archiver.test.ts` | **3/3** (schema parsea old+new shapes — backlog-compat) |
| Recon full | binario de test compilado (ver §4.1) | **24 passed / 0 failed / 0 ignored** (6 stage2 + 18 pre-existentes) |

### 4.1 Nota de método (fail-honest)

- La corrida `cargo test -p recon` (suite completa) quedó bloqueada ~10 min en el file-lock del
  build directory (múltiples agentes hermanos compilando en paralelo — mismo patrón que BR-02
  §5.1). Se detuvo y se ejecutó el BINARIO de test ya compilado directamente
  (`target/debug/deps/recon-ddb329790019b380.exe`, el mismo hash que cargo corrió para el filtro
  stage2 — mismo código, cero recompilación): **24 passed / 0 failed / 0 ignored** en 0.01s.
- `cargo` no estaba en el PATH del shell de esta sesión — invocado como `$HOME/.cargo/bin/cargo.exe`
  (target caliente del árbol principal).
- La salida de clippy pasó por `tail` (lección BR-02 de máscara de exit-code): verificado por
  contenido (línea `Finished` + `[exited with code 0]` del harness, 0 líneas `warning`/`error`).

## 5. INVARIANTE §4 (R8): cero escrituras hasta ≥100 labels reales — verificado capa por capa

| # | Capa | Evidencia en ESTE apply (file:line) |
|---|---|---|
| 1 | Job no spawneado | `recon/main.rs:372-381`: default `off` → `stage2_calibration.dormant`; `.env.example:324` `ARBX_STAGE2_CALIBRATION_MODE=off` |
| 2 | Umbral de disparo | `stage2_calibration.rs:156-192`: watermark `MAX(calibrated_at)`; `new_labels < consolidate_every` → `waiting` + return (cero writes) |
| 3 | Guard de vacuidad | `:271-279`: `total_n == 0` → `skipped_no_pairs`, store intacto |
| 4 | Matemática anti-fabricación | `:358-364` n=0 ⇒ (0.0, 0) doble-seguro (short-circuit + analítico); shrinkage κ=20; clamp `:337-340`; cota documentada correcta (§3-M1); tests `zero_n_operator_contributes_nothing`, `shrinkage_prior_dominates_sparse_operator`, `all_wins_clamps_to_finite_log_lr` |
| 5 | Labels reales solamente | drift_tracker.rs de main S4-03 PRE-EXISTENTE (sin diff — DROP consciente §0) + gate consumidor Edición 2 (`:217`) |
| 6 | Lectura honesta | `priors_cache.rs:176-206` store vacío ⇒ `calibration()=None` ⇒ fold saltado ⇒ wire `posterior_log_odds: null, calibration_applied: false` (`opportunity_emitter.rs:704-705`); test `fold_absent_calibration_is_honest_none` |
| 7 | Contrato wire estricto | `scored-opportunities-archiver.ts:59-68`: 2 campos `nullable().optional()` declarados; SIN `.passthrough()` (campos no declarados se dropean, nunca se inventan); vitest 3/3 |

**Superficie observable post-deploy** (para el verifier L4): PG `math_operator_calibration` = 0
filas mientras labels < 100 · Redis `--scan --pattern '*calib*'` = 0 (el store es PG; PriorsCache
es memoria de proceso — NO introduce claves Redis) · score records con `posterior_log_odds: null`
· logs recon `stage2_calibration.dormant` (flag off). NUNCA `consolidated` sin labels previos.

## 6. Reglas duras cumplidas

- **RULE 00 / R8**: cero mocks, cero hardcode. El único camino al store es `consolidate()` con
  pares (e,Y) reales; None≠0 preservado en cada capa (fold honesto-null; `unvalued`/`no_evidence`
  contados y excluidos, jamás imputados). Gate WO-06 §7.3 sobre mi diff: `grep -icE
  "0x3235|0x0645|0x6982|agld|xen\b|pepe"` = **0 hits**.
- **§32/§33**: 0 executor/wallets/capital/firma/broadcast. El port es read/compute-only sobre
  datos de simulación paper. **0 sesiones SSH** (baseline VPS ya constatada por WO-07-DESIGN §0
  y BR-05+06-VERIFY: store 0 filas, 0 labels, 0 claves calib).
- **§34.3**: `backend/relays-client/` byte-idéntico a HEAD (0 líneas del diff). Los 3 flips
  (`ARBX_STAGE2_CALIBRATION_MODE` / `ARBX_DRIFT_TRACKER_MODE` / `SIM_BACKEND`) quedan documentados
  como OPERADOR-ONLY — este apply aterriza TODO dormante (defaults `off` en código + .env.example).
  §34.1: mode-invariante — el fold aplica la misma matemática en todos los modos.
- **§36**: trabajo en el árbol principal compartido (branch `feat/hops-live-01` @ 27aca289,
  target caliente), diffs ajenos (CB-02 runtime_knobs/census, BR-02 pool_discovery/knobs,
  control-board api-server) preservados byte a byte.
- **NO-GIT**: 0 commit/push/PR/deploy. Diffs viven en el working tree para el PR del orquestador.
- **Marcadores**: 24 comentarios `// BR-05 (2026-09-07)` en los 8 archivos (el charter fijó
  BR-05 como ID del WO; la procedencia WO-07 queda citada inline en cada marker).

## 7. Handoff a la mesa

1. **Al verifier adversarial (re-despacho)**: rubrica = WO-07-DESIGN §5 gates 1-4 + §4 capas 1-7
   (este reporte §5 ya mapea cada capa a file:line del apply). Re-ejecutar:
   `cargo test -p recon stage2` · `cargo test -p searcher-rs --lib priors_cache` ·
   `cargo test -p searcher-rs --lib score_record` · `tsc --noEmit`. Verificar además los 3 DROP
   (§0 tabla) y que los knobs de backoff NO están en `.env.example`.
2. **Al builder BR-04** (cuando despachen): `opportunity_emitter.rs` YA NO está limpio —
   rebasar sobre este diff (struct `priors`, `build_score_record` 8 args con `fold`). El
   security-reviewer ya dejó su baseline del archivo (§3.2 de BR-03+04+07-VERIFY).
3. **Al operador** (cadena causal, WO-07-DESIGN §6.1, TODOS operador-only): P1-3
   `SIM_BACKEND=revm` → labels → `ARBX_DRIFT_TRACKER_MODE=on` → ≥100 labels →
   `ARBX_STAGE2_CALIBRATION_MODE=on`. El código ya está auditado y dormante para cuando lleguen
   las labels. Encender stage2 HOY solo produce `stage2_calibration.waiting` cada 60s (honesto,
   inofensivo — no hay labels).
4. **Follow-up fuera de claim** (documentado, no editado): estratificar θ₀ por `sim_fail_family`
   cuando haya datos (WO-07-DESIGN §7); `README.md` fila 2b/2c (§3.5 opcional) para el PR del
   orquestador si lo desea; PG column para persistir `posterior_log_odds` (hoy parsed-not-persisted
   por diseño XLANG-01).

— ecc:rust-reviewer, Gang Omniscience, 2026-09-07/08. Construido sobre WO-07-DESIGN/VERIFY
(2026-09-06) + BR-05+06-VERIFY + BR-03+04+07-VERIFY + BR-02-APPLY (mesa redonda §1).
