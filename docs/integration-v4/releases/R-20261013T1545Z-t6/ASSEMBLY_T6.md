# ASSEMBLY_T6 — ensamblado del release NUEVO sobre el estado RESUELTO

> t6 (α9 VIGIL + α10 SCRIBA) · 2026-10-13T15:45Z · base limpia `feat/exp-20261013 @ 21feefb32`.
> Orden del capitán: el paquete R-20261013T085048Z PRE-enmienda NO se publica; se corta R-nuevo
> sobre el estado resuelto (enmienda cond7 + promoción t10 YA en árbol + ADM-n admitidos).

## Receta (comandos literales)

```powershell
$r = 'docs\integration-v4\releases\R-20261013T1545Z-t6'
# copia de procedencia (byte-exacta):
Copy-Item R-20261013T085048Z\SSOT\* $r\SSOT\                       # SSOT congelado t9
Copy-Item R-20261013T085048Z\gen\{gen_ssot_v4_rs.py,detector_policy.rs(+.diff),
       strategy_hop_map.v4.fixture.json,hop_cycle_bridge.verify.txt} $r\gen\
Copy-Item R-20261013T085048Z\{x_ref.json,C_THETA_verdict.md} $r\    # antecedente α6
Copy-Item R-20261013T13HI-t13\adm\{econ_sizing,staleness_gate,nonatomic_gate,test_adm}.py $r\adm\
Copy-Item t17-evidence\c_evaluator.py $r\tools\                     # 𝒞 EJECUTABLE persistido
# propios de t6: tools/{vmap_delta.py,phi_static.py}, ci/phi-t7.yml, V0_baseline.md,
# V5_shadow.md, Q1_Q8.md, V1..V3, R_report_T6.md, ASSEMBLY_T6.md (este), ℛ.tsv (cierre)
```

## Diff intencional del release vs base limpia (verificación del worktree de build)

```
 M backend/searcher-rs/src/detector_policy.rs   # promoción t10 (5698976f…) — EN el árbol compartido,
                                                # replicado al worktree de V3 para compilar el conjunto
?? docs/integration-v4/                          # releases + evidence + A1/A3/A7/A8 + ESCALERA_REPORT
?? vps-map/ ?? qa-baseline/ ?? XREF_BASELINE.*   # A0 (t15) + x_ref crudo (t16)
```

## No-pérdida (auditoría §17 del contrato)

Diff del árbol compartido vs HEAD 21feefb32 = HEAD limpio + lista intencional, SIN pérdida:
ASSEMBLY t9 · seam SSE t4 (+14B) · changedPaths t4/t3/t5/t13 · ADM-n (t13 adm/) · manifest 264
(t2) · t12/t17-evidence · detector_policy.rs (t10) · A7/A8 (t14) · vps-map (t15) · XREF_BASELINE
(t16) · t6-evidence + R-20261013T1545Z-t6 (esta tarea). Los 4498 M pre-existentes (t1 T1_modified.txt)
permanecen INTACTOS — cero bytes de esa masa entra al release (worktree detached para compilar).

## Regla de hash §11-bis

sha256 de A1..A9 + A0 registrados en `docs/integration-v4/A1_MANIFEST.md`; sha256(A1 regenerado),
sha256(A3), sha256(A9=V1_hashes.txt) y del release → ℛ.tsv (filas t6).
