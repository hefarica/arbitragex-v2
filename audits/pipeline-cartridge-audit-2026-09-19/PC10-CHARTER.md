# CHARTER PC-10 — Certificación matemática 264 estrategias × 32 operadores (296 grafos)

## /Goal (operador, 2026-09-19, mid-turn #5/#6/#8)
Para CADA una de las 264 estrategias (cartuchos Rhai en `backend/searcher-rs/cartridges/strategies/`)
y CADA uno de los 32 operadores (`backend/math-engine/src/operators/`, `OPERATOR_COUNT=32`),
producir un GRAFO por estrategia (≥264) + un grafo por operador (32) que certifique:

1. **Aplicabilidad**: si el operador APLICA o NO APLICA a esa estrategia (fuente: 
   `backend/searcher-rs/cartridges/strategy_mapping.json` SSOT "Mapeo individual 264x31" — 
   OJO: dice 31, el operador dice 32, math-engine dice 32: resolver el delta con evidencia, 
   identificar al operador 32 y si está/excluido del mapping).
2. **Matemática exacta**: cada fórmula del operador (Rust) vs la hoja canónica del Excel
   (`audits/pipeline-cartridge-audit-2026-09-19/xlsx_extract/`, QuoteBase Manual 24 hojas +
   ULTRA 21 hojas). Vector independiente: reproducir cada fórmula con aritmética propia
   (Python decimal/entero, NUNCA re-ejecutar el código Rust bajo test como "prueba").
3. **Potenciamiento real en detección**: trazar la arista código-verificada
   detector → cartucho → operadores math-engine (o su ausencia) → sizing → gates → cards.
   Si un operador aplicable NO participa en el flujo de detección de su estrategia = 
   GAP (reportar, no remediar sin gate).
4. **Flujo E2E**: detección → activación del cartucho con TODA su matemática → cards con
   rentabilidad ordenada mayor→menor.

## Doctrina del operador (verbatim, mid-turn #7/#8)
- Estrategia 1 puede estar potenciada por 32; estrategia 2 por 10 — GARANTIZAR que los
  aplicables (10/10, 32/32) efectivamente se aplican y ejecutan = máximo potencial.
- Matemática pura, lenguaje topológico: entender la matemática ANTES de certificar.
- Ventaja sobre el 95% de la competencia: más rápido, más preciso, casi matemáticamente perfectos.
- El subconjunto de operadores habilitado por toggle es el universo que se aplica.

## Mandato ampliado (operador, mid-turn #10, 2026-09-19)
Primero determinar que TODAS las estrategias están implementadas, muy bien escritas, en el
lenguaje correcto, y que pasan TODAS las pruebas que rigen en los cánones para apoyar el
arbitraje matemático puro. Igual con los 32 operadores: bien escritos, implementados. Si algo
FALTA en cualquiera de ellos (estrategia u operador) → DESARROLLARLO en base al Excel, aplicando
todas las técnicas disponibles. Los desarrolladores del gang NO se detienen en reportar: el gap
se cierra con código + tests (cero commit/push sin gate del operador, resto de reglas intactas).

## Reglas del gang
- RULE 00 ZERO MOCKS · R8 fail-honest ("no verificado" ≠ "no existe").
- Read-only sobre repo/VPS; CERO commits/push/deploy; capital/flips = operador-only.
- LEER `audits/pipeline-cartridge-audit-2026-09-19/GOAL-WORKORDERS.md` (BOARD) antes de
  trabajar y actualizar el propio WO al cerrar.
- LEER `C:\Users\HFRC\.claude\skills\arbitragex-omniscience\LEARNINGS.md` antes de componer.
- Veredicto por grafo: PASS / FAIL / GAP con evidencia (file:line + vector numérico).
- Clasificación anti-hallucination: CANONICAL_WORKBOOK / CANONICAL_REPO / PRIMARY_SOURCE /
  INFERRED / HYPOTHESIS / UNKNOWN.

## Entregables
- `graphs/strategy/MEV-XX-XXX.md` (≥264): nodos/aristas + operadores aplicables + veredicto.
- `graphs/operator/op_XX.md` (32): fórmula Rust vs Excel + vector independiente + estrategias
  que potencia + si participa en detección (arista verificada).
- `graphs/SUMMARY.md`: tally PASS/FAIL/GAP, delta 31-vs-32 resuelto, matriz aplicabilidad
  final, leaderboard de estrategia por nº de operadores efectivos.
