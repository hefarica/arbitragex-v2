# Antipatrones Prohibidos

## Antipatrón 1: React Context "God Mode"
Crear un único `AppProvider` gigantesco que contiene `user`, `theme`, `wsConnection`, `tableData`, `isModalOpen`. Cuando `isModalOpen` cambie, todo el DOM que dependa del contexto re-renderizará forzosamente.

## Antipatrón 2: Redundant API caching in Zustand
Recibir un objeto de DB gigante de un fetch y meterlo entero a un store de Zustand.
