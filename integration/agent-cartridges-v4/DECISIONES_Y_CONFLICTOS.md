# Decisiones explícitas — sin reconciliaciones ocultas

## 1. Censo
Los dos libros describen 264 IDs numerados. El repositorio inspeccionado contiene además siete scripts raíz. Se generan 264 reemplazos propuestos, no un ID ficticio ni una eliminación para forzar 270. Los siete raíz permanecen intactos.

## 2. Límites: swaps, legs y acciones no son sinónimos
El rango solicitado 2…7 es el presupuesto de búsqueda de swaps. Los libros declaran límites de hasta 16 legs y también acciones con mínimo 1. El repo tiene una máscara por ID. Para ciclos se utiliza la intersección entre esos tres conjuntos, preservando los originales en los manifiestos. Solo 36 estrategias tienen su intervalo fuente completamente cubierto por esa intersección; las demás tienen una limitación de alcance declarada, no una validación total falsa.

`MEV-01-016` sigue requiriendo exactamente tres hops: ambos libros y la política consultada lo especializan como triangular. Los ciclos más largos descubiertos quedan como handoffs, no como ejecuciones mal etiquetadas de un triángulo. `MEV-01-017` conserva cuatro. El handoff se entrega como dato; conectarlo al router de familias existentes es parte de integración, no se ha ejecutado aquí.

## 3. Tipo de estrategia
No se convierte liquidación, carry, orderbook, NFT, opciones o cross-chain en un ciclo CPMM. Se han derivado 14 contratos de dominio para las 60 familias originales. Sus ecuaciones y notas permanecen textuales. Los planes de esos dominios necesitan los solvers, posiciones, libros, límites y mecanismos de liquidación propios. Un nombre de función o recibo no implementa ese solver.

## 4. Conflicto no resuelto
`MEV-01-022` se llama non-cyclic inventory mientras su nota específica exige un ciclo cerrado. Se conserva ambas afirmaciones; no se aprueba una propuesta bajo una identidad contradictoria. `reports/CONFLICTS.json` contiene esta discrepancia y las demás con referencia a la fuente.

## 5. Operadores
La fuente maestra contiene 31 columnas/IDs y 8.184 relaciones. El registro del repo consultado tiene 32 operadores. El 32 no se elimina, renumera ni asigna arbitrariamente a todas las estrategias. Tampoco se adopta la numeración contradictoria de 35 operadores del texto previo.

Los pesos del maestro están sin calibrar. Sus antiguos valores JSON, roles en conflicto, estado de extracción Rhai y resoluciones se mantienen como evidencia de origen. La resolución `Audited_Role` gobierna la generación porque es el rol explícito resuelto en el maestro, no porque se haya supuesto que el JSON antiguo ejecuta esa decisión actualmente.

Un operador secundario apagado se declara `DISABLED`, nunca cero. Para una obligación primaria, apagar un nombre no elimina la obligación económica: se necesita la implementación nativa equivalente documentada o se entrega una falla de dependencia. No se ejecuta una fórmula sustituta inventada para hacer verde la matriz.

## 6. Pseudocódigo aportado
Se conserva la organización A/B/C/D, pero se sustituyen sus errores técnicos explícitamente: mapas Rhai `#{}`, ausencia `()`, identidad local a función por el Scope vacío del runner, helper con nombre definido, resultado tipado y plan inmutable. El quote nativo ya descuenta fees/impacto; no se vuelven a restar de un activo distinto. El neto USD sí descuenta costes externos. Sumar impactos o riesgos no demuestra respetar un límite: lo valida la política nativa con recibo ligado al plan.

`risk_weight=0.10/0.15` y `volatility>0.5` eran ejemplos del pseudocódigo, no parámetros calibrados contenidos en ambos libros. No se inyectan en la configuración de producción. Se conserva el `Gate LIVE original` (incluida su escala de riesgo) como contrato para el evaluador canónico; no se transforma una escala 0…100 en una probabilidad 0…1 sin modelo.

## 7. Modos y realidad
LIVE_MAINNET, TESTNET y PAPER_SHADOW tienen la misma contabilidad y obligatoriedad de entradas. Red, activos, costes y estado reales de cada entorno pueden producir números distintos; la igualdad aplica a la lógica con entradas idénticas. Controles ON/OFF/KILL son independientes. Ninguna simulación paper con storage overrides certifica autorización o ejecución LIVE.

## 8. Estado declarado por las fuentes
Las fechas de pruebas, `READY_FOR_CARTRIDGE_MIGRATION`, `VERIFIED_STATIC` y `Implementado=NO` se preservan; no se convierten en resultados ejecutados en esta sesión. Las filas desalineadas de `08_CONFLICTS` se conservan en sus posiciones originales. Los JSON y XLSX contienen el material sin perder filas.
