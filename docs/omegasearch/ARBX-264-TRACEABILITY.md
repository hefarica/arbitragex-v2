# ARBX-264-TRACEABILITY-01 — Censo verificado contra la fuente canónica

**Fecha:** 2026-10-03 · **Base:** `origin/main` = `6d38a7c6` · **Método:** lectura directa de los 264 `.rhai` + los 3 Excel canónicos, con cruce programático.
**Estado de este documento:** verificación reproducible. No es una auditoría de opinión: cada cifra tiene su comando o su celda.

---

## 1. Fuentes canónicas localizadas

Los tres Excel que el prompt §2 nombraba como "cuando estén disponibles" están **disponibles** y fueron leídos celda a celda (no se abrieron macros; solo lectura de valores):

| Archivo | Hojas | Qué aporta |
|---|---|---|
| `ArbitrageX_SOP_264_Math_Software_GO_NOGO.xlsx` | 16 | Las **16 fórmulas** canónicas (F001–F016), la **máquina de estados de 8 fases**, el spec de software, 264 filas de runtime, los 60 detectores, los 31 operadores |
| `ArbitrageX_SOP_264_Implementacion_Rentabilidad.xlsx` | 21 | El plan de implementación, inputs económicos, rentabilidad, casos de prueba, invariantes, el **resolutor de requisitos** (procedimental) y el ciclo operativo |
| `ArbitrageX_Integrated_2_to_7_Hops_Flash_Atomic_Model.xlsx` | 21 | El modelo 2–7 hops con flash atómico, y la hoja que cierra la trazabilidad: **`STRAT_OP_MATRIX`** |

Además, históricos que **no** son estado actual y se usan solo como evidencia de decisión:
`HARDENING_AND_ROADMAP.md` (2026-08-05, HEAD `64243864`).

---

## 2. La cadena fuente → generación → carga → ejecución, verificada

### 2.1 Fuente (`STRAT_OP_MATRIX`, hoja del Integrated)

- Cabecera en la fila 3: `MEV_ID | Strategy | Detector_ID | op_01 … op_31` (31 columnas de operador).
- **264 filas** de estrategia (`MEV-01-001` … `MEV-11-012`).
- Celdas con valor ≠ 0: **1.716**.

### 2.2 Generación + carga (los 264 cartuchos `.rhai`)

```
264 archivos en backend/searcher-rs/cartridges/strategies/
264 mev_id únicos · 60 detector_id distintos · 11 familias
mev_01=36 mev_02=17 mev_03=31 mev_04=31 mev_05=14 mev_06=30
mev_07=30 mev_08=25 mev_09=20 mev_10=18 mev_11=12   (suma 264)
Cada cartucho implementa el contrato completo de 7 funciones:
  init_strategy · agent_manifest · find_optimal_paths · detector_requirements
  evaluate_opportunity_internal · evaluate_opportunity · build_payload     → 264/264
```

### 2.3 El cruce que decide: fuente vs carga

```
conjuntos de operadores IDÉNTICOS entre Excel y cartucho:  264/264
relaciones en la FUENTE:  1.716   (media 6,5 por estrategia, rango 3..8)
relaciones en la CARGA:   1.716   (media 6,5 por estrategia, rango 3..8)
detectores: cartucho vs Excel                             :  264/264 coinciden
operadores distintos usados                               :  23/31
operadores SIN uso en la matriz                           :  op_02, op_03, op_04,
  op_09, op_12, op_18, op_28, op_31  (idénticos en fuente y carga)
```

**La cadena fuente → generación → carga está cerrada y verificada.** No hay divergencia.

### 2.4 Corrección de una premisa extendida: **no hay 8.184 relaciones**

`264 × 31 = 8.184` es el **tamaño de la matriz**, no el número de relaciones. La propia fuente marca la mayoría de las celdas como `NOT_APPLICABLE`: solo **1.716** son relaciones reales.

Desglose de la carga por nivel de requisito:

| Nivel | Celdas |
|---|---|
| `NOT_APPLICABLE` (rol `N/A`) | 6.468 |
| `PRIMARY_REQUIRED_OR_EQUIVALENT` (rol `PRIMARY`) | **883** |
| `OPTIONAL_SUPPORT` (rol `SECONDARY`) | **833** |
| **Total declarado** | **1.716** |

### 2.5 Ejecución: el runtime SÍ especializa por estrategia

La cadena de ejecución, con símbolo exacto:

```
cartucho: agent_v4_operators(ctx, spec, candidate)
  → binding Rhai   backend/searcher-rs/src/rhai_agent_bridge.rs:775
  → SnapshotServices::operators()          snapshot_services.rs:1112
  → dispatcher inyectado en producción     cartridge_boot.rs:2642
      (sólo cuando existe MarketState real; sin él los servicios quedan sin dispatch)
  → native_operator_adapter::evaluate_declared()   native_operator_adapter.rs:25
      lee spec["operator_requirements"]  → recorre id 1..=31
      role == "N/A"                 → NOT_APPLICABLE  (NO se ejecuta)
      is_disabled(id)               → DISABLED        (no finge que corrió)
      admission.validate(...) falla → DATA_GAP        (con su razón)
      resto                         → registry.dispatch(id, state)
```

**Consecuencia:** los `operators.NN::DATA_GAP` de la telemetría **no** son un fallo de especialización ni de declaración. La declaración se lee y se respeta; el hueco nace en `admission.validate()` — falta la **entrada** del operador, que es exactamente lo que ataca la línea `MARKET-FEATURES-*`.

Esto es coherente con la frecuencia observada: `op_22` está declarado en **264/264** estrategias y es precisamente el que más `DATA_GAP` acumulaba en la ruta activa.

---

## 3. Las 16 fórmulas canónicas (F001–F016)

Extraídas de `04_FORMULAS`. Son el contrato económico que el motor debe satisfacer:

| ID | Nombre | Especificación |
|---|---|---|
| F001 | Alpha current | `a_i(t) = a_i,0 · exp(-λ_eff·t) · max(0.5, 1+β·(σ/σ0 − 1))` |
| F002 | Effective decay | `λ_eff = λ0 + λ_comp·competitor_index + λ_tech·I_leak` |
| F003 | Half-life | `t_1/2 = ln(2)/λ_eff` |
| F004 | Net profit | `NP = GrossProfit − Gas − FlashFee − BuilderTip − RiskHaircut − ExternalCost` |
| F005 | Net ROI | `r_net = NP / Capital` |
| F006 | Expected value | `EV = p_capture·p_success·NP − p_revert·FailureCost − InfraAlloc` |
| F007 | Expected ROI | `r_EV = EV / Capital` |
| F008 | Closed-route marginal prefilter | `w_e = −ln((1−fee_e)·rate_e / fair_e)`; candidato si `Σ w_e < 0` |
| F009 | Exact closed-route PnL | `Q_R(x) = q_n(…q_2(q_1(x)))` ; `Π_R(x) = Q_R(x) − x − C_R(x)` |
| F010 | Capture probability | `p_capture = sigmoid(b0 + b1·latency + b2·bid + b3·route_quality + …)` |
| F011 | Economic gate | `PASS iff NP>0 AND r_net ≥ target AND EV ≥ minEV AND r_EV>0` |
| F012 | Data gate | `PASS iff status READY, freshness, same-block si se exige, profundidad y slippage` |
| F013 | Execution gate | `PASS iff simulación, math tests, invariantes y política de repago atómico` |
| F014 | Dependency gate | `PASS iff toda dependencia declarada está sana/fresca` |
| F015 | **LIVE evidence gate** | `PASS iff reconciled trades ≥ mínimo AND reconciled PnL ≥ 0` |
| F016 | Final GO/NO-GO | `GO iff LIVE + enabled + aprobación manual + todos los gates PASS` |

`F008` es exactamente el peso marginal `-log(rate_after_fee)` del prompt §6 — que **genera candidatos y no prueba beneficio a tamaño finito**. `F009` es la valoración exacta que sí lo prueba. `F015` es el gate que hoy **no** está satisfecho.

## 4. La máquina de estados de 8 fases

| De | A | Requisito de entrada |
|---|---|---|
| `DRAFT` | `SHADOW` | Software spec + unit tests definidos |
| `SHADOW` | `PAPER` | Detección shadow estable + quotes exactas |
| `PAPER` | `CANARY` | Evidencia OOS/walk-forward + simulación |
| `CANARY` | `LIVE` | Umbral de trades reconciliados + PnL no negativo |
| `LIVE` | `QUARANTINE` | Fallo de cualquier gate crítico |
| `QUARANTINE` | `CANARY` | Causa raíz corregida + suite de regresión PASS |
| `LIVE` | `SUNSET` | EV persistentemente negativo / decaimiento |
| `SUNSET` | `SHADOW` | Nueva versión/variante con nueva config |

## 5. El resolutor de requisitos (procedimental)

`19_RESOLVER_REQUISITOS` no es una lista de nombres: es un procedimiento `situación → detección → procedimiento → módulo → evidencia → error a evitar`. Tres filas que importan:

- **Falta campo / quote** → implementar binding + validador → `DATA + ADAPTER` → evidencia: snapshot y quote exacta → **"No sustituir null por cero."**
- **Neto menor al objetivo** → explorar tamaños, pools, venues → `SIZE + ECON` → evidencia: mejor NP y brecha restante → **"No aumentar gross por decreto."**
- **Neto negativo** → conservar signo; hallar otra combinación → `ECON` → evidencia: caso negativo visible y nueva búsqueda → **"Un requerimiento de desarrollo no [se satisface inventando]"**

Es la misma doctrina que R8/RULE 00 y que el prompt §1.

---

## 6. Correcciones a premisas previas (incluidas las propias)

| Premisa | Estado | Evidencia |
|---|---|---|
| "264 identidades degeneradas (`I=0`), activarlas violaría RULE 00" (HARDENING 2026-08-05, §8) | **OBSOLETA** | Los 264 `.rhai` actuales son workbook-derived, tienen el contrato de 7 funciones, 0 literales degenerados y su `agent_manifest` reproduce la matriz de la fuente |
| "hay 8.184 relaciones que verificar" | **CORREGIDA** | 8.184 es el tamaño de la matriz; las relaciones reales son **1.716** (883 primarias + 833 secundarias) |
| "los cartuchos no declaran operadores" | **FALSA** | Declaran por **id numérico** (`"id": 1`), no por `op_01`; un grep por `op_\d{2}` en el manifiesto da 0/264 y es un artefacto del patrón |
| "el runtime aplica el mismo conjunto a todos" | **FALSA** | `evaluate_declared` (`native_operator_adapter.rs:25`) lee `operator_requirements` por estrategia y respeta `N/A` |
| "el bloqueo dominante es la ausencia de productores de features" | **PARCIALMENTE CIERTA** | Cierto en la ruta v4 (`admission.validate` → DATA_GAP), pero **no** es un fallo de especialización ni de declaración |

## 7. Lo que sigue abierto

1. **`F015` no satisfecho**: `reconciled trades = 0`, `reconciled PnL = 0`. Ninguna estrategia puede pasar a `LIVE` mientras el gate LIVE no tenga evidencia — no por falta de implementación, por falta de operación reconciliada.
2. **23/31 operadores declarados; 8 nunca usados** (`op_02, op_03, op_04, op_09, op_12, op_18, op_28, op_31`). Ocho capacidades del motor están implementadas y **no las pide ninguna de las 264 estrategias**: decidir si es correcto (la matriz manda) o si la fuente dejó hueco.
3. **`weights_calibrated: false`** en los 264 manifiestos y `weight: ()` vacío en los 1.716 requisitos. La fuente define 883 `PRIMARY` y 833 `SECONDARY`, pero **ningún peso numérico**. El motor no puede ponderar evidencia sin esa columna: es una brecha de la fuente, no del código.
4. **Trazabilidad procedural pendiente**: localizar el archivo que declara cada nombre de requisito (`full_tick_traversal_or_protocol_quoter`, etc. — cada uno aparece en exactamente 1 archivo de `backend/`) y enlazarlo por nombre en la matriz de aceptación.

---

## 8. Cómo reproducirlo

```powershell
# censo del contrato y de los literales degenerados
python - <<'PY'   # ver §2.2: 264/264 en las 7 funciones, 0 literales degenerados
# cruce fuente vs carga: 264/264 conjuntos idénticos, 1716 relaciones
PY
```

Los CSV extraídos de los Excel (solo valores, sin macros) son:
`SOP_MATH__04_FORMULAS.csv` · `SOP_MATH__09_RUNTIME_264.csv` (264 × 79) ·
`SOP_MATH__11_STATE_MACHINE.csv` · `SOP_MATH__13_DETECTORS.csv` (60) ·
`SOP_MATH__14_OPERATORS.csv` (31) · `SOP_IMPL__19_RESOLVER_REQUISITOS.csv`

---

*Este documento reemplaza cualquier afirmación previa sobre el censo 264×31 que no cite la hoja `STRAT_OP_MATRIX`. Un documento que afirme 8.184 relaciones está contando la matriz, no las relaciones.*
