# Especificación canónica 264 — volcado de los Excel fuente

**Qué es esto.** Volcado a CSV, celda por celda, de los **3 Excel canónicos** del proyecto (58 hojas,
14.066 filas, 5,8 MB). Son la fuente de la que se generaron los 264 cartuchos: su contenido es la
**especificación**, no una implementación.

**Qué NO es.** No es código, no es estado de runtime y **no es evidencia de que algo esté implementado**.
Un valor aquí es un valor de libro. El censo implementado se verifica en
`../ARBX-264-TRACEABILITY.md`.

**Procedencia (anclada criptográficamente).** Extraído de
`dsh-session-session-39f2d591-b6c7-4ea7-b21c-f7d7997b25f6 (1).zip`. Los SHA256 calculados coinciden
**exactamente** con los directorios de contenido del propio zip, así que el origen no es una
suposición:

| Workbook | SHA256 | Tamaño | Hojas |
|---|---|---|---|
| `ArbitrageX_Integrated_2_to_7_Hops_Flash_Atomic_Model.xlsx` | `456b0329fe596ae93d5dfccafd4cfe5c57a4775a3664f7cd41ed213d815cb864` | 227 KB | 21 |
| `ArbitrageX_SOP_264_Math_Software_GO_NOGO.xlsx` | `e69ea9d552e66305bd2e802371506264ef49285f0f9d8607ef1ff096cc458121` | 511 KB | 16 |
| `ArbitrageX_SOP_264_Implementacion_Rentabilidad.xlsx` | `866aaece1e752573992a1d69a9b5af1ee615b72c42677b9319b8c07145ff9e0a` | 834 KB | 21 |

**Cómo se leyó.** Solo **valores** (`data_only`): las fórmulas se volcaron como su resultado calculado.
No se ejecutaron macros. openpyxl avisó de extensiones no soportadas (condicional formatting y una
extensión desconocida) — afectan a formato, no a datos.

**Convención de layout (importante para parsear).** Cada hoja empieza con una fila de título en español
y, en las del SOP de implementación, una segunda de nota. **La fila de cabecera de datos está en la
fila 3** de las hojas `NN_*` y en la **fila 2** del resto. Las celdas vacías se emiten como cadena
vacía; las filas totalmente vacías se descartaron.

```python
import csv
rows = list(csv.reader(open("...__09_RUNTIME_264.csv", encoding="utf-8")))
hdr  = rows[0]        # titulo
cols = rows[1] if rows[1][0].strip() else rows[2]   # cabecera real
# -> la cabecera real de esta hoja esta en rows[1]
```

---

## Índice por valor de uso

### A. La matriz y sus dos ejes (la base del censo)

| Hoja | Filas × cols | Qué contiene |
|---|---|---|
| `…Integrated…__STRAT_OP_MATRIX` | 269 × 41 | **264 × 31 vínculos** `MEV_ID | Strategy | Detector_ID | op_01…op_31`. La autoridad de las **1.716** relaciones |
| `…Integrated…__STRATEGIES_264` | 266 × 45 | Catálogo canónico: grupo, familia, `Required_Surface`, `Backend_Module`, toggle de frontend, `Min_Legs`/`Max_Legs`, `Legs_Model`, clase determinista, atomicidad |
| `…Integrated…__DETECTORS` | 62 × 12 | Las **60 familias** de detector con su ecuación/criterio y sus operadores primarios y secundarios |
| `…Integrated…__OPERATORS` | 34 × 10 | Los **31 operadores**: rol canónico, `Enabled`, `Engine_Present`, `Calibration_State` |

### B. La especificación operativa por identidad ← **el material más denso**

| Hoja | Filas × cols | Qué contiene |
|---|---|---|
| `…Math_Software…__09_RUNTIME_264` | **266 × 79** | El **RUNTIME GATE**: por estrategia, atomicidad, dependencias de oráculo/bridge, e inputs marcados azul (editables) / link. Es el mapa más detallado que existe por identidad |
| `…Math_Software…__10_SOP_264` | 266 × 38 | Master SOP end-to-end por estrategia |
| `…Math_Software…__06_SOFTWARE_SPEC` | 266 × 22 | Contrato implementable por identidad: módulo de backend, detector, superficie |
| `…SOP_Implementacion…__11_SOFTWARE_264` | 267 × 22 | La misma capa desde el otro libro (contrato de software) |
| `…SOP_Implementacion…__02_IMPLEMENTACION_264` | 267 × 27 | Requisitos → desarrollo → beneficio por estrategia |

### C. El plan de trabajo — **ya está escrito**

| Hoja | Filas × cols | Qué contiene |
|---|---|---|
| `…SOP_Implementacion…__03_TAREAS_264` | **3.171 × 14** | **3.168 tareas, 12 por estrategia**: `Task_ID`, orden, tipo, entregable |
| `…SOP_Implementacion…__12_TESTS_264` | **1.851 × 14** | **1.848 especificaciones de test** vinculadas por `MEV_ID` y clase |
| `…SOP_Implementacion…__13_INVARIANTS_264` | **1.587 × 13** | **1.584 obligaciones verificables** con predicado por estrategia |
| `…Math_Software…__07_TESTS_264` | 1.850 × 12 | Catálogo de tests obligatorios: **7 por estrategia** |
| `…Math_Software…__08_INVARIANTS_264` | 1.586 × 11 | Invariantes: **6 por estrategia** |

### D. Economía, contabilidad y reconciliación

| Hoja | Filas × cols | Qué contiene |
|---|---|---|
| `…SOP_Implementacion…__04_INPUTS_ECON_264` | 267 × 28 | **264 fixtures contables**: naturaleza del dato, token de referencia |
| `…SOP_Implementacion…__05_RENTABILIDAD_264` | 267 × 31 | Beneficio, objetivo y **brecha** por estrategia |
| `…SOP_Implementacion…__17_RECONCILIACION` | 267 × 19 | Ganancias y pérdidas **acreditadas**: evidencia de settlement, finalidad, numeraire, precio |
| `…SOP_Implementacion…__18_ALPHA_264` | 267 × 17 | Decaimiento del alpha: `Alpha0_bps_SIM`, hazard previo, lambdas. **Diagnóstico acumulativo, no veto a implementar** |
| `…SOP_Implementacion…__07_CONTRATO_EJECUCION` | 33 × 8 | Mínimo de beneficio verificable, campo por campo, con su test o invariante |
| `…Integrated…__ROUTE_PNL` | 14 × 22 | De la señal matemática a la ganancia neta |
| `…Integrated…__SENSITIVITY` | 10 × 10 | Viabilidad por modo de financiación y por número de hops |

### E. El modelo 2→7 hops con flash atómico

`ASSUMPTIONS` (21×6) · `TOKENS` (12×11) · `POOLS` (28×18) · `EDGES` (54×15) · `FLASH_LIQUIDITY` (10×12) ·
`ROUTES` (14×28) · `LEGS` (86×32) · `DISCOVERY` (20×11) · `HOPS_2_7` (21×12) ·
`ATOMIC_PRIMITIVES` (14×13) · `MATH_NOTES` (17×8) · `DASHBOARD` (18×12)

### F. Gobernanza, fórmulas y trazabilidad

| Hoja | Filas × cols | Qué contiene |
|---|---|---|
| `…Math_Software…__04_FORMULAS` | 18 × 8 | **F001–F016**, las ecuaciones canónicas |
| `…Math_Software…__11_STATE_MACHINE` | 10 × 9 | Ciclo de vida: `DRAFT → SHADOW → PAPER → CANARY → LIVE`, con `QUARANTINE` y `SUNSET` |
| `…Math_Software…__02_GROUP_GATES` | 13 × 26 | Umbrales por grupo: `TargetNetROI`, `MinEV_USD`, `MinRiskScore`, `MaxRevertProb`, `MaxDataAge_ms`… |
| `…Math_Software…__01_CONTROL` | 19 × 6 | Control global: solo inputs del SOP, **no** datos de mercado |
| `…Math_Software…__05_DATA_SOURCES` | 14 × 10 | Contrato de fuentes: campos requeridos, consumidor, **regla de frescura** |
| `…Math_Software…__03_VARIABLES` | 43 × 10 | Diccionario de variables con unidades, fuente y test de validación |
| `…SOP_Implementacion…__19_RESOLVER_REQUISITOS` | 19 × 8 | Procedimiento ante cada brecha: situación → procedimiento → módulo → evidencia → **error a evitar** |
| `…SOP_Implementacion…__20_CAMBIOS_FUENTES` | 24 × 8 | Qué proviene de la fuente y qué se revisó |
| `…Integrated…__SOURCE_AUDIT` | 24 × 8 | Integración de los 8 archivos fuente, con SHA256 |
| `…Integrated…__REFERENCES` | 24 × 6 | Fuentes matemáticas, de routing y de flash liquidity |
| `…Math_Software…__15_CHANGE_CONTROL` | 2 × 12 | **Ninguna estrategia LIVE sin trazabilidad** |

---

## Reglas de uso

1. **Prioridad de fuente.** Ante discrepancia entre estos CSV y el código, gana el código para
   *lo implementado*; gana esta especificación para *lo exigido*. La divergencia es un hallazgo, no
   un empate.
2. **Nada de aquí es un valor por defecto de producción.** Los precios, probabilidades, notionales y
   ROI de estos libros son **fixtures SIM** (el propio libro se declara "snapshot simulado").
   Copiarlos al runtime viola RULE 00.
3. **`Alpha_264` no es un veto.** La hoja lo dice: *"diagnóstico acumulativo, no veto a implementar"*.
4. **`weight: ()` está vacío en los 1.716 requisitos** y `weights_calibrated: false` en los 264
   manifiestos. La fuente define `PRIMARY`/`SECONDARY` pero **ningún peso numérico**: es una brecha de
   la especificación, no del código.
5. **Trazabilidad celda a celda.** `_MANIFEST.json` da, por hoja, el workbook, su SHA256, filas,
   columnas y el nombre del CSV.
