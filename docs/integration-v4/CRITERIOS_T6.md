# A3 — CRITERIOS T6 (montado por t6, regla de hash §11-bis)

> Procedencia: `.tmp-inbox/integrado-supremo/00_PLAN/CRITERIOS_T6.md` (145 B, semilla del paquete
> INTEGRADO; hash de origen verificado en `t1-evidence/T1_inventario_sha256.txt`). Montado a
> `docs/integration-v4/CRITERIOS_T6.md` por t6 (estado F0: "pendiente t6" → MONTADO).
> Contenido SEMILLA íntegro; la evaluación de cada criterio se agrega debajo con su evidencia.

## Semilla (verbatim, 7 criterios)

```
# CRITERIOS T6
1 strategies==264 | 2 detectors==60 | 3 combos==1436 | 4 DERIV_POS>0 | 5 IMP>=BASE | 6 delta>=+58% | 7 cap nuevas +3 (INSPECCION)
```

## Evaluación t6 (cada criterio con instrumento)

| # | Criterio | Valor medido | Instrumento | Veredicto |
|---|---|---|---|---|
| 1 | strategies==264 | 264 | A1 origen `MANIFEST.txt:5` (counts) + 𝒞 cond1 sobre `SSOT/strategy_hop_map.json` (`python tools/c_evaluator.py SSOT --canonical` → rc=0) | PASS |
| 2 | detectors==60 | 60 | A1 origen :5 + 𝒞 cond2 sobre `SSOT/detector_policy.csv` | PASS |
| 3 | combos==1436 | 1436 | A1 origen :5-6 (asserts OK) + 𝒞 cond3 (Ω=Σhops) | PASS |
| 4 | DERIV_POS>0 | deriv_pos=3560 | A1 origen :7 (`deriv_pos: 3560 (>0 OK)`); definición F04/t13: 3560 = IMP familia D_ post-fix (1982=BASE pre-fix; 12000=filas delta>0 en TESTS — tres métricas distintas, t13 F04) | PASS |
| 5 | IMP>=BASE | imp 31744 >= base 19744 | A1 origen :8-9 | PASS |
| 6 | delta>=+58% | +60.8% (A1) · **+60.778% MEDIDO** (t8: (31744-19744)/19744) | A1 origen :10 + medición t8 | PASS |
| 7 | cap nuevas +3 (INSPECCIÓN) | 3 capacidades nuevas implementadas y admitidas: ADM-52 sizing-AMM-cerrada+Kelly, ADM-54 staleness-oráculo, ADM-55 ventanas-no-atómicas (`adm/*.py`, unittest 10/10 t13 re-ejecutado por t6) | inspección t6 sobre `adm/` + `python -m unittest adm/test_adm.py` | PASS |

**Φ(θ-paquete) = g1∧…∧g7 = 1** (g1..g6 SOLO del MANIFEST/A1 — anti-circularidad §7; g7 por INSPECCIÓN).

> Regla de hash §11-bis: sha256(ESTE archivo A3) queda registrado dentro de A1 regenerado
> (`docs/integration-v4/A1_MANIFEST.md`) y en ℛ.tsv.
