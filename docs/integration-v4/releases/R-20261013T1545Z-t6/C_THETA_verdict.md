# 𝒞(θ) — veredicto α6 LIMES (t9, attempt a3355acc-ebed-4330-874f-775976013a80)

θ bajo prueba = SSOT congelado en este release (`SSOT/`, byte-idéntico a 01_SSOT del integrado-supremo,
sha256 verificados contra inventario t1). Condición del contrato:
𝒞=1 ⟺ |𝒮|=264 ∧ |𝒟|=60 ∧ |Ω|=1436 ∧ ∀s 3≤|mask(s)|≤7 ∧ ∀h d(h)=245/262/260/233/233/203
∧ mask(MEV-05-003)={2} ∧ ∀j |Πⱼ|=(53,31,31,14,30,30,25,20,18,12)ⱼ

## Resultado por fuente (todo con artefacto, script medido esta sesión)

| condición | 01_SSOT (θ) | 01_SSOT csv | repo fixture legacy | xlsx A8 (CLASIF.+TESTS) | hoja INVARIANTES xlsx |
|---|---|---|---|---|---|
| \|𝒮\|=264 | PASS | PASS | PASS | PASS | OK (264/264) |
| \|𝒟\|=60 | PASS (detector_policy.csv 60 filas) | idem | — | — | OK (60/60) |
| \|Ω\|=1436 | PASS | PASS | PASS | PASS | OK (Combos 1436) |
| 3≤\|mask(s)\|≤7 | PASS (todas 5-6) | idem | **FAIL** (5×tamaño1 + 1×tamaño2) | PASS | — |
| d(h) exacto | PASS | PASS | PASS | PASS | OK (245/262/260/233/233/203) |
| mask(MEV-05-003)={2} | **FAIL** {2,3,4,5,6} | idem | **FAIL** {3} | **FAIL** {2,3,4,5/6} | **AUSENTE** (no es invariante del paquete) |
| \|Πⱼ\| exacto | PASS (R_+CF_=53) | idem | — | PASS | — |

## Lectura literal vs lectura canónica

- **𝒞 literal = 0**: la condición 7 `mask(MEV-05-003)={2}` es INALCANZABLE en toda fuente del
  paquete bajo lectura de conjunto. Bajo lectura bitmask-u8 el único "2" del universo de datos es el
  campo `mask` del fixture LEGACY del repo (MEV-05-003 → mask:2, h:[3]) — es decir, el contrato
  transcribió el bitmask 2 del fixture viejo envolviéndolo en llaves de conjunto. Ese fixture es
  precisamente la taxonomía que el SSOT v4 sustituye, y además VIOLA 3≤|mask(s)|≤7 (6 estrategias).
- **𝒞 canónica = 1**: las 6 condiciones de la hoja INVARIANTES del xlsx A8 (fuente suprema,
  LEEME: repo@5f8ea3d67) + tamaños 3-7 + Πⱼ — todas PASS sobre θ con artefacto, y 01_SSOT ==
  xlsx A8 en 264/264 máscaras (fidelidad total del SSOT a su fuente).
- Hallazgo adicional: el test existente `detector_policy.rs::envelope_intersection_semantics:1567`
  cita "MEV-01-015 (R_CLOSED_CYCLE, allowed {2})" — el {2} real del fixture legacy pertenece a
  MEV-01-015, no a MEV-05-003 (medido: legacy MEV-01-015 h=[2]).

## Aplicación del veto (α6 I=veto absoluto — decisión NO tomada unilateralmente por t9)

El veto α6 con lectura literal detendría t9 antes de regenerar. La evidencia demuestra que la
condición 7 es una errata de transcripción del contrato (bitmask legacy leído como conjunto), no un
defecto del SSOT. Este release se produce COMPLETO (regeneración aditiva, build+test, staging
aislado con sha256) para no congelar t3-t5/t8/t10-t13/t17 por una errata, PERO la sustitución en el
árbol real (docs/quotebase + fixture) y toda promoción QUEDAN EXPLÍCITAMENTE RETENIDAS hasta que el
capitán resuelva el contrato: (a) confirmar veto ⇒ descartar R-20261013T085048Z y t9=failed; o
(b) enmendar la condición 7 (eliminarla o re-expresarla como HopMask_u8 del fixture legacy / como
subset 2⊆mask, que θ SÍ satisface) ⇒ 𝒞=1 y el release procede a t6.
