# Mixed Integer Routing with Fixed Costs

## Propósito
Resolver el enrutamiento considerando los costos fijos (Gas de interacción de contrato por cada DEX cruzado) usando optimización lineal entera mixta (MILP).

## Conocimiento esencial
Un enrutador continuo puede sugerir dividir un trade en 5 DEXes para ganar 1 centavo en price impact, pero quemar $50 en gas por los saltos adicionales. Este modelo penaliza cada salto con un costo escalón (Step Cost).
