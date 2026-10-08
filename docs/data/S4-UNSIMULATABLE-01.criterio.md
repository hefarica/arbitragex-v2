CRITERIO DEL DISCRIMINANTE — t158 S4-UNSIMULATABLE-01
Declarado ANTES de contar. Intento e59912d9-e8b1-4c54-892e-6275a545691f.

=== LA PREGUNTA ===
`strategy_cyclic_route_not_simulatable_in_s4` = 617.570 de 699.889 filas = 88,2 % de
`simulations`. ¿Es un VEREDICTO DE MERCADO o un HUECO DE CAPACIDAD?

- VEREDICTO DE MERCADO: la fila fue EVALUADA y el motor decidio que no se puede
  simular. Hay rastro de la evaluacion.
- HUECO DE CAPACIDAD: la fila murio ANTES de evaluar. No hay rastro.

=== EL DISCRIMINANTE, DECLARADO ANTES DE CONTAR ===
Columna `simulations` con sus columnas reales (verificadas por information_schema):
  id, opportunity_id, simulator, gas_estimate_wei, gas_price_wei, slippage_pct,
  revert_risk_pct, simulated_profit_usd, passed, fail_reason, raw_trace, trace_id,
  simulated_at

CRITERIO — una fila de la familia fue EVALUADA si y solo si tiene AL MENOS UNO de
estos rastros de evaluacion NO NULO:
    raw_trace            (la traza de la simulacion)
    gas_estimate_wei     (gas estimado = el simulador corrio)
    gas_price_wei
    slippage_pct
    revert_risk_pct
    simulated_profit_usd

CRITERIO — murio ANTES DE EVALUAR si TODOS esos seis son NULOS y `fail_reason`
esta puesto.

Es falsable: basta contar. Y es binario por fila, sin umbral que ajustar.

=== LO QUE PREDIGO, DICHO ANTES (y me obligo a reportarlo si falla) ===
Dado el nombre literal del string —"not_simulatable_IN_S4"— predigo que la MAYORIA
de esas filas murio ANTES de evaluar, o sea que el discriminante las clasifica como
HUECO DE CAPACIDAD y NO como veredicto de mercado. Si sale al reves, se reporta al
reves.

=== EL TECHO DENTRO DEL TECHO (criterio 7 del contrato), declarado antes ===
Se mide por `opportunity_id`: de las filas de la familia,
  - cuantas tienen OTRA fila en `simulations` para el MISMO opportunity_id que SI
    tiene rastro de evaluacion (=> llego a S4 ALGUNA VEZ, aunque fuera en otro intento)
  - cuantas NO tienen ninguna (=> NUNCA llego a S4 en ningun intento)
Si el 100 % es terminal, el 88,2 % no es un embudo: es un corte.

=== LO QUE NO VOY A HACER ===
1. NO voy a sumar el prefijo y presentarlo como si supiera cuantas variantes existen.
   Doy las variantes EXACTAS, una por una, con su string literal, y declaro el
   RESIDUO (total del prefijo menos la suma de las variantes listadas) como residuo
   contado por su propia consulta.
2. NO voy a afirmar que t130 mueve las 16.920 filas de la variante `triangular`:
   t130 apunta a `triangular_arb` y NO es el mismo string. Es HIPOTESIS. Entrego los
   literales exactos para que el acople se decida con el byte.
3. NO voy a mezclar el 7 (muestra de 25 del benchmark) con el 29.010 (universo PG).
   Son poblaciones distintas y ninguna corrige a la otra.
4. Si el productor del string NO existe en el arbol, ESO es el hallazgo: una etiqueta
   sin productor es un fantasma.
