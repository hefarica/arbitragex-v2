# Flashbots Bundle Construction

## Propósito
Empaquetar transacciones de arbitraje con firmas de búsqueda (searcher signatures) para enviarlas a los builders/relays (Flashbots, Titan, beaverbuild) mediante RPC privado.

## Conocimiento esencial
Los bundles garantizan ejecución atómica (todo o nada) sin riesgo de reversiones en cadena que cuesten gas (a menos que el revert suceda por un uncle block, lo cual es mitigado por relés privados).
