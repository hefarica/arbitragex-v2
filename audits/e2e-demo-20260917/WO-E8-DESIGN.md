# WO-E8 · DESIGN — Síntesis del veredicto unificado honesto (R8)

**WO:** WO-E8 · **kind:** redesign (composición, no recolección)
**Agente:** ecc:code-architect (Gang Omniscience, programa e2e-demo-20260917)
**Fecha:** 2026-09-17 · **Entregable principal:** `audits/e2e-demo-20260917/00-SYNTHESIS.md`
**Reglas duras respetadas:** RULE 00 / §32-§33 read-only / §34.3 intacto / NO-GIT / 0 HTTP
requests gastados del presupuesto de cortesía (2 disponibles).

---

## 1. Contrato del WO

WO-E8 corre AL FINAL. Su input son los reportes WO-E1..E7 escritos por los pares en
`audits/e2e-demo-20260917/`. Su output es la respuesta canónica del operador con 5
secciones obligatorias:

1. QUEDA DEMOSTRADO — solo afirmaciones con evidencia `file:line` + ids reales.
2. GAPS abiertos — cada uno con dueño tentativo y si bloquea o no el claim "100% real".
3. Respuestas cruzadas — adjudica las 7 preguntas de ataque entre pares; NO responde
   por el originador si se requiere evidencia nueva.
4. Veredicto DEMO E2E — PASS / FAIL / GAPS en términos R8 ("100% real" = datos reales
   extremo a extremo, NO = rentable; rechazadas honestas con 0 accepts son parte de la
   honestidad, no su contradicción).
5. Prerrequisitos HFT-1000 que quedan en pie (FEE-TIER-AWARE-QUOTING en vuelo: sin
   accepts no hay labels).

## 2. Método (diseño de composición)

```
INVENTARIO    ls -la audits/e2e-demo-20260917/ → mapa WO → archivo
              (los reportes ausentes se declaran AUSENTES, jamás se sustituyen)
     │
TRIAJE        por cada reporte: extraer (a) claims DEMOSTRADO con cita file:line,
              (b) claims GAP con dueño/bloqueo, (c) preguntas de ataque lanzadas,
              (d) preguntas recibidas y si fueron refutadas
     │
ADJUDICACIÓN  matriz de contradicciones: si el claim de A choca con el claim de B,
              citar AMBOS archivos + línea; la síntesis adjudica SOLO con evidencia
              ya presente en los reportes; si faltara evidencia nueva → el hallazgo
              queda abierto a cargo del originador (no lo inventa la síntesis)
     │
VEREDICTO     regla de decisión R8 (sección 3) → PASS / FAIL / GAPS / NO-ADJUDICABLE
     │
SALIDA        00-SYNTHESIS.md + glosario OMEGA
```

## 3. Regla de decisión del veredicto (invariante lógico)

El veredicto DEMO E2E NO es una opinión: se deriva mecánicamente de la matriz.

| Condición (todas con cita) | Veredicto |
|---|---|
| Datos reales extremo a extremo verificados (detección→emisión→persistencia→UI, fuentes vivas) en TODOS los eslabones que el demo declara cubrir | **PASS** |
| Algún eslabón del demo sin evidencia real PERO declarado honestamente (fail-honest R8, 0 accepts con razones exactas visibles) | **GAPS** (el demo es real donde dice ser real; los vacíos se nombran) |
| Algún eslabón que presenta datos fabricados/maquillados como reales, o un vacío silenciado | **FAIL** |
| Los reportes de los pares NO EXISTEN en disco al momento de componer | **NO-ADJUDICABLE** (fail-honest: se declara inputs-missing; PROHIBIDO sintetizar desde suposición — RULE 00) |

Corolario R8: "rechazadas honestas con 0 accepts" NO convierte un PASS en FAIL: la
ausencia de Topological Yield positivo es un resultado, no un defecto de honestidad.

## 4. Invariante (INV-E8)

> **INV-E8:** Toda afirmación de `00-SYNTHESIS.md` cita un reporte fuente existente en
> `audits/e2e-demo-20260917/` (`file:line`) o un artefacto canónico citable (reports
> adyacentes fechados, SQL verificado, commits con SHA). Ninguna afirmación se compone
> desde evidencia no observada por la mesa. Si un input E1..E7 falta, la sección que lo
> requería se marca `NOT COMPUTED — INPUT MISSING <WO-id>` y el veredicto degrada a
> GAPS o NO-ADJUDICABLE según cuántos falten. El archivo nunca rellena huecos con
> adjacencia presentada como si fuera evidencia del E-series.

## 5. Gate (verificación del entregable)

- **G-E8.1 (inputs):** al componer, `ls audits/e2e-demo-20260917/` debe mostrar los
  reportes E1..E7; el inventario del §0 del SYNTHESIS debe conciliar 1:1 con ese ls.
- **G-E8.2 (citas):** cada fila de "QUEDA DEMOSTRADO" y cada GAP lleva ≥1 cita
  `archivo:linea`; cada contradicción nombrada cita a AMBOS pares.
- **G-E8.3 (veredicto):** el veredicto publicado concuerda mecánicamente con la tabla
  §3 de este diseño dada la matriz publicada en el propio SYNTHESIS.
- **G-E8.4 (anti-fabricación):** cero afirmaciones sobre el estado del pipeline VPS o
  del demo que no provengan de un reporte fechado; los 2 HTTP de cortesía sin gastar
  se declaran en el §0.
- **G-E8.5 (glosario):** el SYNTHESIS cierra con el glosario OMEGA (TLS, Holonomic
  Loop Resolution, Topological Yield, Decoherencia de Estado, Variedad de Liquidez).

## 6. Estado de ejecución real (fail-honest)

A la hora de componer (2026-09-17, hora local ~12:0x), `audits/e2e-demo-20260917/` NO
EXISTÍA: ni un solo reporte WO-E1..E7 estaba en disco (dos ventanas de poll de 10 y 15
minutos, evidencia en §0 del SYNTHESIS; `hermes_status` reportó `active_agents=0`,
`active_api_runs=0`). Conforme a INV-E8 y a la fila 4 de la tabla §3, el veredicto
publicado en `00-SYNTHESIS.md` es **NO-ADJUDICABLE (INPUTS MISSING)** con el estado
componible del registro adyacente claramente separado (sección §2 del SYNTHESIS,
marcada como ADYACENTE, no como evidencia del E-series). El SYNTHESIS queda estructurado
para que una re-ejecución de E8 (cuando E1..E7 aterricen) llene las secciones 1, 3 y 4
sin reescribir el método.

## 7. Archivos bajo claim

- `audits/e2e-demo-20260917/WO-E8-DESIGN.md` (este archivo)
- `audits/e2e-demo-20260917/00-SYNTHESIS.md` (entregable principal)
