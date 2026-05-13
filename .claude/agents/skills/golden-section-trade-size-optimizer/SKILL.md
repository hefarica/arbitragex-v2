# Golden Section Trade Size Optimizer

## Propósito
Maximizar el beneficio neto encontrando el volumen de entrada exacto (Optimal Trade Size) usando búsqueda de sección dorada en una función de beneficio convexa univariada.

## Algoritmo
Dado que el profit como función del volumen de entrada `f(x)` es cóncavo (crece hasta un pico y luego cae por el slippage), se evalúan puntos iterativamente usando el radio áureo (`phi = 1.618...`) reduciendo el espacio de búsqueda.
