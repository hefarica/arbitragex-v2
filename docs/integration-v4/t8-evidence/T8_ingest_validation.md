# t8 — t₆-datos: Validación de ingesta A2 (csv) + A8 (xlsx) + asserts vs SSOT congelado

- **Productor:** data-ingest (α-datos), team prices-v4-integration · 2026-10-13
- **Entradas (extraídas de `C:\Users\HFRC\Downloads\arbitragex-prompt-supremo-INTEGRADO.zip` a `%TEMP%\t8_ingest`, sha256 re-medido):**

| Input | sha256 medido | == inventario t1 | == ℛ t9 |
|---|---|---|---|
| `06_CLASIFICACION/clasificacion_264.csv` (A2) | `38f038a82b5d16979e17d002c406d249d9979d164166e2cdaac063d0b80a7786` | ✓ | n/a |
| `ArbitrageX_v4_264_60hojas.xlsx` (A8) | `eb5b803cb66d0dcbb5c17c9885e9a3dc041e01e94c5bc22a37ef0482e4e09cc3` | ✓ (== MANIFEST:sha256(xlsx)) | n/a |
| `01_SSOT/strategy_hop_map.json` | `7fb529c607071a210a0b3bc627dd27b475008f2031445e9acccf317884861122` | ✓ | ✓ (ℛ.tsv:4) |

ℳ (comando reproducible): `python docs/integration-v4/t8-evidence/t8_validate.py` (ronda 1, 38s) y `python docs/integration-v4/t8-evidence/t8_validate_r2.py` (ronda 2); salidas íntegras en `T8_output_full.json` y `T8_output_r2.json`.

---

## (1) Ingesta A2 — clasificacion_264.csv — VEREDICTO: **OK**

| Check esperado | MEDIDO | Estado |
|---|---|---|
| header == `MEV_ID,GRUPO,FAMILIA,DETECTOR,SUPERFICIE,HOPS,BASE_POS,IMP_POS,DELTA` | exacto | OK |
| 264 filas de datos | 264 | OK |
| orden DELTA descendente | no-creciente ✓ (78.0 → 8.0); **no estricto** (empates existen) | OK (descendente; estricto NO afirmado) |
| sin columna índice | 9 columnas, col0=MEV_ID, sin Unnamed/index | OK |
| MEV_IDs únicos | 264/264 únicos | OK |

Extra medido: 58 detectores distintos, 10 superficies distintas, HOPS = máscaras multi-hop (`2/3/4/5/6` estilo).

## (2) Validación A8 xlsx — VEREDICTO: **OK con 2 divergencias declaradas**

| Check | MEDIDO | Estado |
|---|---|---|
| Hojas fijas presentes | 59 hojas, nombres y orden == lista esperada exacta (`T8_output_full.json: sheet_names`) | OK |
| `|hojas| ≥ 60` (tras A7/t4-t5 REGISTRO_INCLUSIONES) | **59 < 60**, `registro_inclusiones_present=false` | **PENDIENTE** (esperado: la hoja la agregan t4/t5) |
| TESTS_ATOMICOS columnas == las 13 especificadas | exacto | OK |
| TESTS_ATOMICOS filas = 264×1000 | 264.000 filas; 264 MEV_IDs × 1000 c/u | OK |
| ASSERT | `BASE<=IMP` en 264.000/264.000 | OK |
| CLASIFICACION_264 (hoja) vs A2 csv | header igual ∧ set MEV_ID igual ∧ orden fila-a-fila igual | OK (idénticos) |
| AGREGADOS_SUPERFICIE vs A2 csv | 10 filas, conteos por superficie 100% iguales | OK |
| AGREGADOS_DETECTOR vs A2 csv | 60 filas, Σ estrategias=264; coincidencia 58/58 detectores reales; **excepción: `OBSERVE01`,`OBSERVE02` existen solo en AGREGADOS con ESTRATEGIAS=0** | OK con excepción declarada |
| INVARIANTES (hoja) | 6/6 OK según la hoja; **pero su `DERIV_POS OBSERVADO=1982` contradice MANIFEST `deriv_pos=3560`** (ver §4) | DIVERGENCIA INTERNA DECLARADA |

## (3) Asserts vs SSOT congelado (t9, ℛ.tsv) — VEREDICTO: **OK con definiciones declaradas**

| Assert | MEDIDO | Veredicto |
|---|---|---|
| **264** estrategias | A2 csv 264 únicos == SSOT json 264 (`n_strategies=264`) == hoja 264 | OK |
| **60** detectores | A2 csv = **58** distintos == SSOT json = 58 (sets idénticos); 60 = filas de AGREGADOS_DETECTOR (58 reales + OBSERVE01/02 sintéticos con 0 estrategias) | OK **solo bajo definición AGREGADOS**; DIVERGENCIA DE DEFINICIÓN DECLARADA |
| **1436** combos | SSOT json Σ|mask| = **1436** exacto; Σ A2 HOPS = 1436 (doble fuente independiente) | OK |
| **0** divergencias MEV_ID | set(A2 csv) == set(SSOT json), diff vacío en ambos sentidos (0/0) | OK |
| hop_dist 245/262/260/233/233/203 | SSOT json: {2:245, 3:262, 4:260, 5:233, 6:233, 7:203} EXACTO; A2 csv HOPS: idéntico | OK (doble fuente) |

Nota de transparencia: `T8_output_r2.json:ssot.hop_dist_matches` dice `false` por un bug de comparación str-vs-int del propio script r2 (`hop_counter.get("2")` sobre claves int); los valores impresos en `ssot.hop_dist` vs `hop_dist_expected` son idénticos entrada por entrada — la igualdad real es la impresa, no el flag.

## (4) Cifras A1 (fuente única de g1-g6) — MEDIDO vs CLAIM

| Figura | CLAIM (MANIFEST.txt:6-11) | MEDIDO (ℳ: t8_validate*.py) | Veredicto |
|---|---|---|---|
| base_pos | 19744 | Σ BASE_POS TESTS_ATOMICOS = **19744.0** exacto | **MEDIDO OK** |
| imp_pos | 31744 | Σ IMP_POS = **31744.0** exacto | **MEDIDO OK** |
| delta_pct | +60.8% | 31744/19744 − 1 = **+60.778%** ≈ +60.8% | **MEDIDO OK** |
| hop_dist | 245/262/260/233/233/203 | igual (SSOT json y A2 csv, doble fuente) | **MEDIDO OK** |
| deriv_pos | 3560 (>0 OK) | **NO REPRODUCIBLE** desde A2/A8: Σ filas DELTA>0 = **12000** (DELTA∈{0,1}: 10 kinds × ~1200); INVARIANTES del propio xlsx dice **1982** — el artefacto se contradice con su MANIFEST (3560 vs 1982). >0 se sostiene (12000>0). | **CLAIM sin ℳ** + DIVERGENCIA INTERNA 3560↔1982↔12000 DECLARADA |
| monotonic | true | Como orden DELTA del A2 csv: **no-creciente ✓** (78→8). Como serie DELTA por (MEV_ID×TEST_KIND): **FALSE** (2556/2640 series violan). Único invariante monotónico medible que sostiene al 100%: ASSERT BASE<=IMP 264000/264000. | **MEDIDO OK bajo interpretación A2-orden; interpretación serie = FALSE** — declarado, no corregido |
| **+0.636 PROHIBIDO (§10)** | — | **0 apariciones** en 792.528 valores escaneados (264000×3 TESTS + 264 A2 DELTA + celdas A01); delta_pct real 60.778 ≠ 63.6 | **OK (sin contaminación)** |

## (5) RG-01/R8 — base de cada cifra

- MEDIDO: todo lo marcado MEDIDO, con ℳ = scripts + outputs JSON adjuntos (reproducibles contra los sha256 de entrada listados arriba).
- CLAIM: `deriv_pos=3560` (MANIFEST.txt, sin ℳ reproducible desde A2/A8/SSOT); `monotonic=true` (MANIFEST; medible solo bajo la interpretación de orden del A2).
- NO COMPUTADO: nada en esta tarea quedó sin computar que el contrato pidiera; lo no computable (deriv_pos por definición ausente) queda como CLAIM citado, jamás como cero.

## Divergencias totales declaradas (NO corregidas en silencio)

1. `detectores`: MANIFEST/INVARIANTES=60 vs A2/SSOT medido=58 reales (60 = 58+2 OBSERVE con 0 estrategias) — diferencia de definición, datos consistentes.
2. `deriv_pos`: MANIFEST=3560 vs INVARIANTES(xlsx)=1982 vs MEDIDO DELTA>0=12000 — contradicción interna del paquete; g1-g6 NO deben consumir 3560 como MEDIDO.
3. `monotonic`: cierto como orden A2; falso como serie por MEV×KIND.
4. `|hojas|=59 < 60` — pendiente REGISTRO_INCLUSIONES (t4/t5), ya declarado por t1.

**Veredicto t8: OK** — ingesta/validación ejecutada completa, asserts 264/1436/0 en verde medido, 60-verde bajo definición AGREGADOS, 4 divergencias declaradas, 0.636 ausente.
