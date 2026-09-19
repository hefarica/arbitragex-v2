# WO-02d-FIX-G1 · CONVERGENCIA DUAL — conteo shared-rs 24 verificado por segundo fixer independiente

> Fixer gang Omniscience ronda 1 · 2026-09-17. Charter idéntico al de
> `WO-02d-FIX-G1-20260917.md` (despacho paralelo, patrón convergencia dual —
> precedente WO-02b-FIX-MATHMAP/WO-02b-FIX2). Regimen §32/§33 read-only,
> doc-only, cero git/cargo/npm/VPS/HTTP (0/5 requests).

## 0. Estado encontrado (FAIL-HONEST primero)

Al iniciar, el fix YA ESTABA APLICADO en disco por el fixer paralelo:

- `02d-RELAYS-CLIENT-SHARED.md` §3 header (:198) corregido a "24 archivos .rs en
  src/ — 25 crate-wide" + blockquote de procedencia `// WO-02d-FIX-G1`.
- §6 (:275-278) corregido a "23/24 archivos src documentados" con marcador.
- §ERRATA-01 append-only (:333-365) completo.
- `GOAL-WORKORDERS.md` :51-55 (resumen DONE WO-02d) y :235-248 ("Reglas de esta
  mesa", entrada `// WO-02d-FIX-G1 APLICADA`) corregidos.
- Reporte del par en disco: `WO-02d-FIX-G1-20260917.md` (6,052 bytes, 02:39).

ESTE fixer NO re-editó ninguna corrección ya aterrizada (cero duplicación de
errata, cero riesgo de doble-marcador). El valor agregado de este despacho es la
**verificación independiente completa** del trabajo del par, que queda
DOBLE-VERIFICADO.

## 1. Recompute independiente (RULE 00 — no heredado)

Comandos ejecutados sobre el árbol de trabajo local (`backend/`):

```
$ find shared-rs/src -name "*.rs" | wc -l                      → 24
$ find shared-rs/tests -name "*.rs" | wc -l                     → 1
$ find relays-client/src -name "*.rs" | wc -l                   → 19
```

- **Diff de listas = 0**: los 24 archivos src encontrados por `find` son BYTE-
  IDÉNTICOS (mismo conjunto, sort) a la lista de 24 declarada en §ERRATA-01
  (:29-34) del par. Verificado con `diff` de listas → "LISTS IDENTICAL".
- **Tabla §3 = 19 filas** (grep de filas `| \`` entre §3 y §4): coincide con el
  reclamo de cobertura del par "19 filas → 23 de 24 archivos src" (`lib.rs` solo
  en preámbulo; subdirectorio `oracle_snapshot/configured_rpc.rs` agrupado en la
  fila del módulo `oracle_snapshot`).
- **"19 relays-client" re-verificado CORRECTO** (find propio = 19) — el charter
  preservaba esta cifra y el fix del par efectivamente no la tocó.

Clasificación: 24 src / 25 crate-wide / 19 relays-client = PRIMARY_SOURCE
(find propio + diff de listas contra la errata del par).

## 2. Auditoría post-fix de residuos claim-bearing (grep de todo el dir)

`grep -rn "52 fichas|33 archivos|33 shared|33/33" --include="*.md" .`:

- **En alcance WO-02d (ficha + board): CERO ocurrencias vigentes.** Las únicas
  apariciones de "33"/"52 fichas" en `02d-RELAYS-CLIENT-SHARED.md` y
  `GOAL-WORKORDERS.md` viven dentro de erratas/marcadores/quotes históricos
  (§ERRATA-01 :339-340, board :53/:237/:246) — ninguna presenta 33 o 52 como
  cifra vigente.
- **Contaminación residual en archivos de otros owners — CONFIRMADA presente y
  CORRECTAMENTE no pisada** (idéntica a la tabla §4 del par):
  - `WO-02a-verify-VERIFY.md:16` y `:309` ("52 fichas") — owner 02a-verify.
  - `WO-02c-CROSS-EXAM.md:68` ("los 33 shared-rs") — owner cross-exam 02c.
  - `WO-02d-verify-CROSS-EXAM.md:43` — cita el 33 como EVIDENCIA del defecto
    (registro del refutador, intacto por diseño).
  Descontaminar los dos primeros = mini-errata append-only de SUS owners
  apuntando a `WO-02d-FIX-G1-20260917.md`. Este fixer NO la aplicó (claims de
  archivo: no se pisan WOs ajenos).

## 3. Evaluación de la desviación vs el hint del charter ("52→43 fichas")

El hint sugería sustituir "52 fichas: 19 + 33" por 43. El fixer par ELIMINÓ la
suma en vez de re-aritmética (reporte §3.3). **CONCUERDO con la decisión del
par**, por dos razones RULE 00:

1. "Fichas" (filas de tabla) ≠ "archivos": 19 fichas relays + 19 filas
   módulo-tipo shared-rs ≠ 43 de ninguna lectura reproducible; 19 fichas + 24
   archivos src mezcla unidades. Escribir "43 fichas" fabricaría un nuevo tally
   con la misma clase de defecto que el original.
2. La cifra load-bearing para el censo del /goal es la cobertura real: 19/19
   relays-client y 23/24 archivos src shared-rs (24/24 contando la cita de
   `lib.rs` en preámbulo) — ambas ahora vigentes y reproducibles.

## 4. Veredicto

**GAP G-1 (DEFECTO 1 del cross-exam 02d): CERRADO y DOBLE-VERIFICADO.** El fix
del par es correcto, completo dentro de su alcance declarado, append-only donde
correspondía, y sin colisión con fixers paralelos. Cero acción adicional
requerida en superficie WO-02d.

Fuera de alcance (intacto, declarado): DEFECTO 2 del cross-exam (TTL
validated_plan 300 s vs 60 s, ficha §4) — espera su propio fixer; erratas
append-only de descontaminación en WO-02a-verify/WO-02c-CROSS-EXAM — owners
respectivos.

## 5. Cambios aplicados por ESTE fixer

- Este reporte (nuevo).
- Una línea de convergencia dual añadida al board (`GOAL-WORKORDERS.md`, entrada
  WO-02d-FIX-G1, append-only) — precedente CONVERGENCIA DUAL WO-02a.

Cero .rs tocados, cero git/cargo/npm/build/VPS/HTTP (0/5 requests).

— FIXER WO-02d-FIX-G1-CONVERGENCE · fin.
