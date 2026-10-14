# R-20261013T085048Z — Release t9 (α2 FABER + α6 LIMES) · attempt 2 fcfef188

**Productor:** ssot-eng (t9) · **Base:** feat/exp-20261013 @ 21feefb32 (línea base limpia; árbol
compartido con 4092 M JAMÁS tocado — ver ASSEMBLY.md) · **Veredicto del paso: OK con 2 escalados.**

## Entregable (para t6)

Staging local completo bajo `docs/integration-v4/releases/R-20261013T085048Z/`:
SSOT congelado (3 archivos, sha256 == inventario t1) · ℛ.tsv (registro con productor/verificador) ·
x_ref.json (t16, δ_abs=0.005 con derivación) · C_THETA_verdict.{json,md} · gen/ (generador
gen_ssot_v4_rs.py + detector_policy.rs regenerado +134/−0 + strategy_hop_map.v4.fixture.json (264
rows) + hop_cycle_bridge.verify.txt + detector_policy.rs.diff) · V2_static.txt · V3_build/ (salidas
literales cargo build+test base limpia + exoneración HEAD puro + restores verificados) ·
V4_dryrun.md (DEGRADADO documentado + probe ssh Git-bash, destino leído /opt/arbitragex-v2,
docs/integration-v4 AUSENTE ⇒ creación limpia) · ASSEMBLY.md · Q1_Q8.md · V1_hashes.txt ·
este reporte. Promoción al árbol REAL retenida (ver escalado 1).

## Resultados por paso del contrato

| paso | resultado | evidencia |
|---|---|---|
| (1) congelar SSOT + sha256→ℛ | OK | ℛ.tsv; V1_hashes.txt; == T1_inventario_sha256.txt:7-9 |
| (2) x_ref t16 + δ_abs | OK | x_ref.json (XREF_BASELINE.md 7a03e242…/json 089ba911… verificados) |
| (3) DFAC['D_']:=0.80 + regenerar Rust | OK | gen/detector_policy.rs: const DFAC_D_UNDERSCORE=0.80 + dfac() + tabla 60 + 2 tests ok; stub hop_cycle_bridge satisfecho por código existente (MIN=3/MAX=7); 04_DIFFS = stubs, integración manual por especificación (t2 §c) |
| (4) 𝒞(θ) α6 | **ESCALADO 1** | C_THETA_verdict.md: literal=0 SOLO por condición 7 (errata bitmask legacy); canónica=1 (hoja INVARIANTES xlsx 6/6 OK + 01_SSOT==xlsx 264/264) |
| (5) V1..V4 | OK (V4 DEGRADADO; V3 con exoneración) | V1_hashes.txt, V2_static.txt, V3_build/*, V4_dryrun.md |
| (6) R-<ts> staging para t6 | OK | este directorio |
| (7) §I Q1-Q8 | OK | Q1_Q8.md (escrituras solo R-<ts>/+t9-evidence; tmp→fsync→rename; restores verificados) |

## V3 — build/test en línea base limpia (worktree HEAD 21feefb32, CARGO_TARGET_DIR compartido)

- `cargo build --release -p searcher-rs` → **exit=0** (20m45s; log V3_build/cargo_build_release_cleanbase.txt).
- `cargo test --release -p searcher-rs --lib` → **1431 passed; 3 failed; 5 ignored**
  (log V3_build/cargo_test_cleanbase.txt). Tests NUEVOS de t9: `dfac_fix_is_materialized` ok,
  `ssot_v4_detector_table_matches_ssot` ok (V3_build/v4_tests_result.txt).
- **ESCALADO 2 (riesgo línea base):** los 3 fallos (`liquidation_snipe_engine::{address_from_word…,
  calldata_encoding…, fetch_positions_real_pipeline_wiremock}`) son PRE-EXISTENTES en HEAD puro:
  exoneración en worktree restaurado (sha256 verificado) reproduce los MISMOS 3 fallos sin mi
  archivo (V3_build/exoneration_pure_head.txt, exit=101 idéntico). La línea base compila (build
  exit=0) pero arrastra 3 tests rojos — t6/t10 deben conocerlo; NO es de t9 (diff +134/−0 solo
  detector_policy.rs).

## Escalados al capitán (decisiones NO tomadas unilateralmente)

1. **Veto α6 condición 7**: `mask(MEV-05-003)={2}` es inalcanzable en TODA fuente del paquete bajo
   lectura de conjunto (01_SSOT=xlsx={2,3,4,5,6}; legacy fixture={3}); el "2" existe SOLO como
   HopMask_u8/bitmask del fixture legacy (y además el {2} real de legacy es MEV-01-015, citado por
   el test existente detector_policy.rs:1567). Recomendación: enmendar el contrato (eliminar la
   condición o re-expresarla como HopMask_u8 legacy / como {2}⊆mask, que θ SÍ satisface) ⇒ 𝒞=1.
   Mientras tanto: promoción al árbol RETENIDA (sustitución de docs/quotebase+fixture NO aplicada);
   el release vive completo en staging. Si el capitán confirma el veto literal: descartar este
   release y t9=failed.
2. **3 tests pre-existentes en HEAD** (liquidation_snipe_engine): documentados + exonerados;
   exigen decisión de quién los repara (no t9, fuera de scope).

## Fail-honest (instrumentos)

- ssh NATIVO: exit=255 stderr 0B (t9-evidence/, replicado 1 vez). ssh Git-bash: exit=0 (V4 probe,
  solo lectura). sql_query: NO usado (t16: sin filas). PG/Redis: no requeridos por t9.
- Formatos: todos los números de este reporte citan su artefacto (columna evidencia / rutas).
