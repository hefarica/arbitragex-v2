# G-PAGE-INV-01 — Cardinalidad del inventario apex (nuevo, cierra D-06)

- **Tipo:** gate anti-regresión estático.
- **Script:** `scripts/gates/g-page-inv-01.sh` (exit 0 = PASS, exit 1 = FAIL).
- **Estado en HEAD `fe62cdce` + P3:** **PASS** (evidencia abajo, reproducible).

## Qué fija

1. **INV-1 se cumple:** `EXPECTED_PAGE_COUNT == PAGES.length` en `frontend/lib/apex/page_inventory.ts`. La invariante estaba **documentada en el propio archivo pero no ejecutada por nada**.
2. **El conteo de descriptores es estable** en el valor que fija esta iteración (38).
3. **La brecha descriptores↔páginas vivas no puede olvidarse:** el gate la reporta en cada ejecución y exige que esté declarada en el inventario como **D-08**. Una brecha no declarada falla el gate.

## Corrección de la cifra de la iteración 1

La iteración 1 reportó **39** entradas en el array `PAGES`. Recuento a profundidad de objeto sobre el mismo HEAD (`fe62cdce`): **38** descriptores (`route:` × 38, sin *spreads*). La constante declaraba **37**. El hallazgo D-06 se mantiene —constante ≠ array ≠ páginas vivas—, pero con la cifra correcta.

## Evidencia (comando reproducible + salida real)

```
$ ./scripts/gates/g-page-inv-01.sh
G-PAGE-INV-01 — apex page-inventory cardinality gate
  info  EXPECTED_PAGE_COUNT = 38, PAGES descriptors = 38
  PASS  INV-1: EXPECTED_PAGE_COUNT == PAGES.length
  PASS  PAGES descriptors == 38 (value pinned by this iteration)
  info  live App Router page.tsx = 59, descriptors = 38, gap = 21
  PASS  descriptors-vs-live gap is declared in the inventory (D-08)

G-PAGE-INV-01: PASS
```

## Lo que este gate NO cierra

El hueco real sigue abierto como **D-08**: el inventario describe 38 páginas y la app tiene 59. Completar los 21 descriptores que faltan es un paso propio (cada entrada necesita ruta, archivo, categoría, propósito, auth, rol, gates de operador, mutaciones, stream y acciones críticas).
