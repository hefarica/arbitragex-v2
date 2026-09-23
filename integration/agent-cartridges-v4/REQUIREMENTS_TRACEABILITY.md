# Trazabilidad de requisitos: fuente → artefacto → evidencia

## Fuentes Excel

| Hoja fuente | Uso en la generación | Estado / límite |
|---|---|---|
| `01_MEV_MATRIX_1_11` de ambos libros | Identidad, misión, familia, clasificación, atomicidad, inventario, dependencias, riesgos, toxicidad, módulos, toggles, gate y estado original | Celdas completas retenidas en `source_catalog_row` y exportaciones originales. No equivale a certificar implementación o ejecución. |
| `00_CARTRIDGE_ARCH` de ambos libros | Separación detector/operador, firmas del contrato, reglas de datos, capas nativas y modos | Requisitos aplicados al diseño; integraciones externas declaradas pendientes. Ambos textos preservados por separado. |
| `02_CARTRIDGE_MATH_MAP` | Ecuación, ID del detector, nota particular, clases, legs, fuentes matemáticas y config propuesta | 264 manifiestos y scripts; fórmulas textuales completas, no reemplazadas por spread genérico. |
| `03_DETECTOR_FAMILIES` | 60 contratos de requisitos y quote/settlement específicos | Contratos generados; no se afirma implementar 60 nuevos solvers de protocolo. |
| `04_FRONTEND_CONTRACT` de ambos libros | DTO íntegro, separación monto/beneficio/riesgo/simulación y campos fuente | Tipos y validador TS incluidos; UI/WS/DB reales no cableados en esta entrega. |
| `05_OPERATOR_CATALOG` | Identidad original 1…31 y referencias | No se renumeran operadores ni se asigna arbitrariamente OP32. |
| `06_B_STATIC_DIAG` | Diagnóstico de superficies, estados y discrepancias | Conservado; no presentado como prueba runtime actual. |
| `07_MASTER_264x31` | 8.184 filas; rol auditado, fase, requisitos, conflictos, calibración y modos | Todos los campos retenidos; roles en manifiestos; pesos no calibrados permanecen vacíos. |
| `08_CONFLICTS` | Contradicciones y decisiones originales | Celdas retenidas incluso cuando hay desalineación de filas; sin inventar correcciones. |
| `09_EXECUTION_MODES` | Economía sin degradación por modo y separación de ON/OFF/KILL | Misma contabilidad; red/capital y autorización siguen siendo del host. |
| `10_MODE_MIGRATION` | Obligaciones de migración y trazabilidad | Criterios y límites de integración; no se marca como migración terminada. |

## Arquitectura A/B/C/D solicitada

| Requisito | Implementación entregada | Prueba / estado |
|---|---|---|
| Una identidad por archivo | `init_strategy` y `agent_manifest` materializados por ID | 264 comprobaciones estructurales + paridad de IDs. |
| Misión y protocolo especializados | Ecuación, clase, nota y requisitos del detector en cada archivo | Paridad exacta con `source_math_row`, contratos 60 familias. |
| Búsqueda adaptativa 2…7 | `agent_graph::enumerate_cycles`, SnapshotServices y handoffs por máscara | Vectores independientes 2,3,4,5,6,7; compilación nativa pendiente. |
| No forzar siempre siete hops | Selección por neto entre rutas/tamaños examinados | Tests de selección y preservación de pérdidas. |
| No falsear un triángulo de cinco hops | Intersección con límites particulares y máscara existente | Tests de MEV-01-016=3 y MEV-01-017=4. |
| Mejor resultado, no falsa prueba de óptimo global | Estado de truncamiento, presupuesto y alcance de selección explícitos | Tests de presupuesto y límites; rendimiento real no medido. |
| Cotización secuencial de hops | Quote exact-in U256/U512 y clave de caché por importe/dirección/bloque | Contraejemplo de comparación paralela; continuidad por hop. |
| Comisión/impacto sin doble descuento | Quote con efectos embebidos; ledger racional de fee/impacto CPMM; costes externos separados | Tests de no doble coste, financiación ya repagada y gas. |
| Precisión monetaria | Enteros raw y BigDecimal; modelo independiente Decimal/int | Tests >2^53, uint256, decimales 0/6/8/18/36/255. |
| Fuente de precios única | Exportación PriceBus identificada por chain/dirección/revisión y expiración | Contrato nativo incluido; conexión con bus real pendiente. |
| No pesos/confianza inventados | Pesos `null/UNCALIBRATED`, confianza global no sintetizada | Paridad de las 8.184 filas; no supuesto de $1 en producción. |
| Switches opcionales reales | Adaptador llama registro existente y callback de deshabilitados | Código de integración incluido; propagación UI real pendiente. |
| Campos aplicables que fallan | Diagnóstico explícito y conservación de quote/plan/ledger parcial | Tests de campos/costes/evidencia ausentes; no se aceptan como completos. |
| N/A solo explícito | Costes/operadores no aplicables con razón y procedencia | Tests distinguen N/A, apagado, ausente y cero. |
| Mismo plan de cálculo y ejecución | Hash de plan/importe/snapshot/precios/política; lookup canónico | Tests de revisión y DTO; simulación EVM pendiente. |
| Cotización no equivale a Sim PASS | `CANDIDATE`, `NOT_RUN_BY_CARTRIDGE`, payload canónico aparte | Tests de no autoautorización. |
| No borrar pérdida porque fue rechazada | Mejores diagnósticos con Option/ausencia, no max=0 | Tests de neto negativo y exactamente cero. |
| No confundir proyección con dinero realizado | `expected_carry`, `non_atomic_expected`, `execution_improvement` | Tests de clase económica y no promoción de mejora a ganancia. |
| Trazabilidad fuente completa | XLSX originales + JSON de valores/fórmulas + manifiestos/CSV | 17 hojas, 276.225 celdas y hashes. Cobertura runtime separada. |
| No tocar configuración | Stager aditivo fuera del cargador activo, idempotente | 7 tests de staging, conflictos, symlinks y preservación. |
| Entrega doble vía y cero pérdida de mensajes | Requisitos precisos para commit/ACK/replay/control revisión | **NO IMPLEMENTADO NI PROBADO EN VIVO** en esta entrega. |
| Todas las estrategias realmente operativas | Generación de los 264 contratos y superficies de integración | **NO CERTIFICADO**; adapters/solvers, calibración, compilación y replay pendientes. |

## Qué no significa este paquete

Una fila conservada no es un algoritmo implementado. Un algoritmo implementado no es una fuente de datos conectada. Una prueba con datos sintéticos no es una transacción simulada contra un bloque real. Una simulación aprobada no garantiza rentabilidad futura. Un hash es integridad de contenido, no autenticidad del RPC. Estos niveles permanecen separados en los resultados.
