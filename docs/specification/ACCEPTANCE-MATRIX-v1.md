# ACCEPTANCE-MATRIX-v1 — Matriz de aceptación única y versionada

- **Rol:** Specification (SSOT documental, fuentes, trazabilidad y contratos de especificación)
- **Tarea:** t18 — SPEC-MATRIZ-01 · Equipo `arbx-publicacion-desbloqueo-02`
- **Artefacto par:** `docs/specification/SOURCE-MANIFEST.md` (misma unidad de versión)
- **Alcance de escritura:** `docs/specification/` únicamente.

---

## 0. Cabecera de versión y congelación de N

| Campo | Valor |
|-------|-------|
| `scope_version` | **`arbx-scope-1.0.0`** |
| `frozen_at` | **2026-10-04** (fecha de la base documental SSOT; ver nota de congelación C-1) |
| `measured_at` | Sesión de t18 · rama `fix/perhop-reserves-01` · HEAD `858b943fd80c8b5e1606d220aa87d63b86f4ba15` · tree `622d32c0bcf9343be79f12513db28a18cbc5dee8` · `origin/main` `274f04fd8d89692fd4b0169130ede74264e31167` |
| **N (denominador)** | **`N = 115`** = 70 requisitos + 40 hallazgos + 5 fronteras de fuente |
| `N — unidades atómicas` | **115** filas, **una capacidad/flujo por fila**, sin criterios-paraguas (regla R2) |
| Denominador de fórmulas (sub-censo) | `M = 3` ejes de fórmula (propuesta / homologación / ejecución) — §6 |
| Autoridad de la numeración | `Anexo_Tecnico_ArbitrageX_2026-10-04.zip` — `trazabilidad/requisitos_vigentes.csv` (70), `hallazgos/catalogo_F01_F40.json` (40), y §4 de `SOURCE-MANIFEST.md` (5) |
| Integridad del anexo | `SHA256SUMS.txt` → **OK=45 · MISMATCH=0 · MISSING=0** (SOURCE-MANIFEST §1.1) |
| Autoridad del mandato | `PROMPT MAESTRO ARBITRAGEX DE IMPLEMENTACIÓN Y ACEPTACIÓN EN LOOP.pdf` · SHA-256 `7583d9a80a0ad438ff1bcc65385d5037437442e0d72f2afd525a0c643b0b34c2` |

### C-1 — Por qué N se congela ANTES de cualquier porcentaje

**N se fija aquí, y ningún porcentaje de este documento existe hasta que N está fijo.** El motivo es aritmético: sin denominador, `aceptados / N` no está definido y **todo porcentaje del proyecto es `NO_CALCULABLE`**.

Qué **es** y qué **no es** `N = 115`:

- **115 es la cardinalidad de las unidades enumeradas por el SSOT y el manifiesto par.** Nada de `N` se inventó: 70 y 40 se recuentan por comando (SOURCE-MANIFEST §3, filas #1 y #2) y las 5 fronteras se miden en SOURCE-MANIFEST §4 (F-1…F-5).
- **115 no es el «100 % del proyecto».** Es el **censo documental enumerado**. Una métrica `aceptados/115` mide **cobertura de aceptación documental**, no madurez, no implementación, no rentabilidad.
- **N es congelado, no inmutable.** Si entra una fuente nueva (p. ej. se materializa el Anexo dentro del workspace y aparecen requisitos no enumerados), **N se re-congela con bump de `scope_version`** (`arbx-scope-1.1.0`), y la comparación entre versiones se hace **siempre** contra el `N` de cada versión. Nunca se recalcula un porcentaje histórico con un `N` nuevo: eso es lo que produce el autoengaño que esta matriz existe para impedir.
- **Las 5 fronteras cuentan en N, no fuera de N.** Una fuente AUSENTE bloquea su parte del censo (SOURCE-MANIFEST §4); sacarla del denominador convertiría un bloqueo en un silencio. Cuenta como unidad, con estado inicial `BLOQUEADO_NO_CENSABLE`.

### C-2 — Estado inicial global (declarado, medido)

- **70/70** requisitos vigentes en estado SSOT `REQUISITO_OBJETIVO_NO_IMPLEMENTADO_POR_ESTA_TAREA`.
- **70/70** en trazabilidad `MAPEO_DOCUMENTAL_EXPLICITO`, cuyo significado en el propio SSOT es *"hay sección que trata el requisito; **no** es cumplimiento"*.
- **40/40** hallazgos en `PROPUESTO_NO_IMPLEMENTADO_EN_ESTA_TAREA`.
- Por tanto el estado inicial de la columna `estado_inicial` es **`NO_ACREDITADO`** para las 115 filas. **Ninguna fila arranca en `ACEPTADO`**, y ninguna fila puede pasar a `ACEPTADO` por existir en un documento: se exige evidencia de ejecución (§7, R4).

### C-3 — Aritmética de aceptación (definida, y hoy NO_CALCULABLE)

```
aceptados(scope_version)     = |{ fila ∈ N(scope_version) : estado = ACEPTADO }|
no_aceptados(scope_version)  = N − aceptados
bloqueados(scope_version)    = |{ fila ∈ N : estado = BLOQUEADO_* }|
tasa_documental              = aceptados / N          # NO es tasa de implementación
```

**Hoy: `aceptados = 0`, `bloqueados ≥ 5` (fronteras F-1…F-5), `tasa_documental = 0 / 115 = 0,0 %` — y ese `0,0 %` significa «cero unidades con evidencia de ejecución», NO «cero implementación real».** La implementación real no se mide con esta matriz: se mide con los artefactos de ejecución que cada fila nombra en su columna `evidencia_requerida`. R8: `NO COMPUTADO` ≠ cero.

---

## 1. Convención de identificadores y ejes

### 1.1 `criterion_id`

| Prefijo | Rango | Origen de la fila |
|---------|-------|-------------------|
| `AC-USR-xxx` | `AC-USR-001` … `AC-USR-070` | `trazabilidad/requisitos_vigentes.csv` del Anexo (70 filas, `id` = `USR-xxx`) |
| `AC-FND-xx` | `AC-FND-01` … `AC-FND-40` | `hallazgos/catalogo_F01_F40.json` del Anexo (40 filas, `id` = `Fxx`) |
| `AC-FRT-x` | `AC-FRT-1` … `AC-FRT-5` | `SOURCE-MANIFEST.md` §4, fronteras F-1…F-5 |

`N = 70 + 40 + 5 = 115`. **Cada `criterion_id` es único y aparece exactamente una vez.**

### 1.2 Modo económico (`eje_modo`)

Vocabulario obligatorio, tomado de `EXECUTION_MODES_DOCTRINE` y de la directiva de la célula (paper por defecto, sin firma, sin broadcast, sin deploy). **Ninguna fila de esta matriz autoriza mainnet.**

- `PAPER_SHADOW` — capital simulado, sin broadcast, ledger simulado. **Modo por defecto de esta matriz.**
- `TESTNET` — fondos de testnet, broadcast testnet, settlement no real.
- `LIVE_MAINNET` — capital real, broadcast mainnet. **Requiere autorización explícita del operador; esta matriz lo registra como requisito, NUNCA como permiso.**
- `—` — la fila es documental o de gobernanza y es mode-invariant.

### 1.3 Eje de distinción obligatorio (§4 del PROMPT MAESTRO)

Toda fila declara **uno y solo uno** de estos ejes. **Es la distinción que impide leer un catálogo como una función:**

| `eje` | Significado | Nunca implica |
|-------|-------------|---------------|
| `CATALOGO_HOMOLOGACION` | está catalogado / homologado / registrado | que esté preparado, autorizado o ejecutado |
| `PREPARACION` | está configurado / calculado / simulado / listo | que esté autorizado o ejecutado |
| `AUTORIZACION` | admisión, riesgo, gate, firma | que hubo broadcast o settlement |
| `EJECUCION` | broadcast, inclusión, finality, ledger | que hubo ganancia |
| `GOBERNANZA_DOCUMENTAL` | documento, SSOT, trazabilidad, censo | absolutamente nada de runtime |

### 1.4 Estados canónicos

`NO_ACREDITADO` (inicial) · `EN_PREPARACION` · `BLOQUEADO_NO_CENSABLE` · `BLOQUEADO_POR_FUENTE` · `ACEPTADO` · `RECHAZADO`.
`ACEPTADO` exige artefacto de ejecución reproducible. `RECHAZADO` exige hallazgo con causa.

### 1.5 Regla anti-paraguas (R2)

Un `criterion_id` que agregue más de una capacidad/flujo es **inválido por construcción** y debe dividirse. Por eso cada fila tiene **una** `accion` y **un** `resultado_observable`. Donde el requisito fuente es amplio, la fila declara **la capacidad mínima acreditable** y la observación queda escalonada en la columna `precondiciones` — explícitamente, en vez de esconder el problema en un criterio-paraguas.

**Nota de honestidad estructural (N-1).** Los 70 requisitos del SSOT **no** vienen descompuestos en unidades atómicas: vienen en 13 grupos (`DOC`, `MOD`, `FIN`, `GRF`, `NET`, `WAL`, `STR`, `API`, `RUL`, `MSG`, `LED`, `OFF`, `OPS`). Esta matriz **no fabrica** una descomposición que la fuente no trae —eso sería inventar requisitos— ni **colapsa** varios requisitos en uno —eso sería un paraguas—. Lo que hace es: **una fila atómica por requisito enumerado**, con la distinción catálogo/ preparación/ autorización/ ejecución declarada en el eje, y los ejes de fórmula separados en §6. Si se quiere un `N` mayor, la vía es **pedir la descomposición al SSOT**, no que Specification la invente.

---

## 2. Matriz — 70 requisitos vigentes (`AC-USR-*`)

Leyenda de columnas: **ID** · **Req** (id SSOT) · **Fuente exacta** (`sección` = `section_id` del anexo) · **Capacidad/flujo** · **Actor** · **Modo** · **Eje** · **Precondiciones** · **Acción** · **Resultado observable** · **Invariantes** · **Ambiente** · **Umbral (origen)** · **Dueño** · **Deps** · **Estado**.

### 2.1 DOC — Documentación y trazabilidad (8)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-001 | USR-001 | `secciones.csv` §A01 | Render del white paper final | Operador | — | GOBERNANZA_DOCUMENTAL | Borrador completo | Renderizar el documento | Conteo de páginas del render final | Sin padding | Local | **>200 páginas útiles** (SSOT `requisitos_vigentes.csv` col. `acceptance`) | DOC | — | NO_ACREDITADO |
| AC-USR-002 | USR-002 | `secciones.csv` §C001–§C065 (65 secciones) | Manual de plataforma página por página | Operador | — | GOBERNANZA_DOCUMENTAL | Inventario de rutas cerrado | Recorrer cada ruta y documentarla | Manual con 1 apartado por cada una de las 65 secciones C | Cada paso con control y error documentado | Local | **65 secciones** (SSOT, `documented_sections` de USR-002) | DOC + Frontend | AC-USR-021 | NO_ACREDITADO |
| AC-USR-003 | USR-003 | `secciones.csv` §A15–§A35 (21 secciones) | SOP operativo y de incidentes | Operador | — | GOBERNANZA_DOCUMENTAL | Runbooks redactados | Ejecutar el SOP de arranque/jornada/cierre | 21 apartados SOP con responsable y evidencia | Rollback declarado en cada uno | Local | **21 secciones** (SSOT A15–A35) | DOC + Ops | — | NO_ACREDITADO |
| AC-USR-004 | USR-004 | `secciones.csv` §A04–§A13 | SSOT por dominio | Arquitecto | — | GOBERNANZA_DOCUMENTAL | Dominios enumerados | Definir autoridad y lineage por dominio | Tabla dominio→autoridad→revisión→retención | Ningún dominio sin autoridad | Local | Sin umbral numérico en el SSOT | DOC | — | NO_ACREDITADO |
| AC-USR-005 | USR-005 | `secciones.csv` §E23–§E35 | Roadmap con progreso derivado de evidencia | Planner | — | GOBERNANZA_DOCUMENTAL | N congelado (esta matriz) | Publicar etapas y dependencias | Progreso calculado como `aceptados/N`, nunca supuesto | Prohibido porcentaje sin artefacto | Local | **N=115** (esta matriz) | DOC + Planner | AC-USR-001 | NO_ACREDITADO |
| AC-USR-006 | USR-006 | `secciones.csv` §D01, §D02, §D38 | Extensión incremental sobre arquitectura existente | Arquitecto | — | CATALOGO_HOMOLOGACION | Arquitectura actual inventariada | Asociar cada brecha a un módulo existente | Brecha→módulo con compatibilidad declarada | Ninguna reescritura no justificada | Local | Sin umbral numérico | DOC + Backend | — | NO_ACREDITADO |
| AC-USR-007 | USR-007 | `secciones.csv` §A02, §A03, §A04 | Integración de adjuntos | Specification | — | GOBERNANZA_DOCUMENTAL | Adjuntos hasheados | Inventariar los adjuntos y sus discrepancias | Manifiesto con hash por adjunto | Fuente ausente = AUSENTE declarado | Local | **8 adjuntos + 3 Excel** (SSOT, `acceptance` de USR-007) | Specification | — | **PARCIAL**: `SOURCE-MANIFEST.md` §1 acredita 6 adjuntos externos + 2 Excel; ver límite abajo |
| AC-USR-008 | USR-008 | `secciones.csv` §B001–§B006, §B038–§B040, §B044, §B045 | Fórmulas al backend vía especificación | Quant | — | CATALOGO_HOMOLOGACION | Fórmulas censadas | Registrar cada celda y ligarla a un módulo | Toda celda con tipo, unidad y golden test | Binding propuesto ≠ campo implementado | Local | **60 881 ocurrencias / 34 696 únicos / 253 BND** (SSOT, SOURCE-MANIFEST §3) | Quant + Backend | AC-USR-040 | NO_ACREDITADO |

**Límite declarado de AC-USR-007.** El SSOT pide *8 adjuntos y 3 Excel únicos*. `SOURCE-MANIFEST.md` §1 acredita **6** adjuntos externos con hash y **2** libros por coincidencia de hash (§2.4 B1, B3). Los restantes **no se inventan**: quedan fuera del recuento y son exactamente el objeto de la frontera F-6/F-8. El estado `PARCIAL` refleja recuento propio, no una excepción.

### 2.2 MOD — Modos de operación y autonomía (7)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-009 | USR-009 | §A09, §A12, §A15, §A18, §D05, §D07, §D12, §D21 | Cálculo y simulación en PAPER | Sistema | `PAPER_SHADOW` | **PREPARACION** | Pipeline de simulación vivo | Ejecutar una simulación trazable | Resultado marcado como simulado, nunca como activo real | Sin broadcast; capital simulado | Local/VPS | Sin umbral numérico | Backend + QA | — | NO_ACREDITADO |
| AC-USR-010 | USR-010 | §A09, §A18, §A28, §D05, §D06, §D10, §D12 | Misma lógica en TESTNET con receipt verificable | Sistema | `TESTNET` | **EJECUCION** | Ac USR-009 acreditado | Broadcast a red de prueba | Receipt verificable de testnet | Identidades de prueba distintas de producción | Testnet | Sin umbral numérico | Backend | AC-USR-009 | NO_ACREDITADO |
| AC-USR-011 | USR-011 | §A09, §A18, §A19, §A28, §D13, §D14, §D18–§D21, §D33 | Admisión→simulación→firma→inclusión→finality→conciliación real | Operador | `LIVE_MAINNET` | **EJECUCION** | Autorización explícita del operador | Recorrer el ciclo completo con capital real | Asset real conciliado en ledger | Gate de autorización no eludible | Mainnet | **Requiere autorización explícita del operador (SSOT USR-011 + doctrina de la célula)** | Backend + Release + Operador | AC-USR-010, AC-FND-* | NO_ACREDITADO |
| AC-USR-012 | USR-012 | §E21, §E22 | Operación manual | Operador | `PAPER_SHADOW` | **PREPARACION** | UI operativa | Proponer una acción y revisar consecuencias | Cada intento con su evidencia visible | Nada se ejecuta sin acción del usuario | Local | Sin umbral numérico | Frontend | — | NO_ACREDITADO |
| AC-USR-013 | USR-013 | §E06, §E10, §E21, §E22 | Operación semiautomática | Sistema + Operador | `PAPER_SHADOW` | **AUTORIZACION** | Sistema propone | Aprobar una propuesta vinculada a plan vigente | Aprobación ligada a plan y vigencia | Aprobación vencida no ejecuta | Local | Sin umbral numérico | Backend + Frontend | AC-USR-012 | NO_ACREDITADO |
| AC-USR-014 | USR-014 | §E02, §E05, §E07, §E21, §E22 | Operación autónoma con mandato acotado | Sistema | `PAPER_SHADOW` | **AUTORIZACION** | Mandato y límites definidos | Operar dentro del mandato, con interrupción disponible | Interrupción detiene la operación | **LLM sin poder ilimitado**; política determinista | Local | Límites del operador (no definidos en el SSOT → sin umbral numérico) | Backend + Risk | AC-USR-013 | NO_ACREDITADO |
| AC-USR-015 | USR-015 | §A09, §D05, §D06, §D22, §E15, §E21 | Separar entorno / autonomía / financiación | Arquitecto | — | **CATALOGO_HOMOLOGACION** | — | Publicar la matriz de estados ortogonales | Tres ejes distinguibles en la UI y en la API | **`PAPER` no es sinónimo de `MANUAL`; `SHADOW` no es etiqueta ambigua** | Local | Sin umbral numérico | Backend + Frontend | AC-USR-009 | NO_ACREDITADO |

### 2.3 FIN — Economía y capital (5)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-016 | USR-016 | §B027–§B036 | Neto por operación como objetivo | Quant | `PAPER_SHADOW` | **PREPARACION** | Costos censados | Computar el neto de una operación | Neto con todos los costos y signo conservado | **Objetivo ≠ garantía de oportunidad ni de beneficio** | Local | Sin umbral numérico (el objetivo no es una promesa) | Quant + Ledger | — | NO_ACREDITADO |
| AC-USR-017 | USR-017 | §E14, §E15, §E16 | Rango objetivo 0,5–5/6 % | Operator | `PAPER_SHADOW` | **PREPARACION** | — | Configurar umbrales por principal | Configuración con `percent` y `bps` inequívocos | Distinguir percent de basis points | Local | **0,5 % a 5–6 %** (SSOT `acceptance` de USR-017; el propio SSOT lo da como rango declarado, no como umbral validado) | Quant + Frontend | — | NO_ACREDITADO |
| AC-USR-018 | USR-018 | §E12, §E15, §E16 | Objetivos absolutos USD y porcentuales combinables | Operador | `PAPER_SHADOW` | **PREPARACION** | DSL de objetivos | Componer neto USD + ROI + ventana + límites | DSL evalúa con tipos verificados | Tipos verificados antes de cotizar | Local | Sin umbral numérico | Quant + Backend | AC-USR-048 | NO_ACREDITADO |
| AC-USR-019 | USR-019 | §B025, §B026, §B034 | Capital propio, prestado y flash loan | Ledger | `PAPER_SHADOW` | **PREPARACION** | Fuentes de capital etiquetadas | Contabilizar notional/equity/deuda/premium/repago | Cada fuente contabilizada por separado | **No mezclar fuentes de capital** | Local | Sin umbral numérico | Ledger | AC-USR-058 | NO_ACREDITADO |
| AC-USR-020 | USR-020 | §F15, §F17, §F21, §F22, §F27, §F30 | Beneficios utilizables en el mundo real | Operador + Tesorería | `LIVE_MAINNET` | **EJECUCION** | Ruta a tesorería definida | Ejecutar un retiro a tesorería | Valor distinguido: USD vs token vs fiat liquidado | **Requiere autorización del operador; no autorizado por esta matriz** | Mainnet | Requiere autorización explícita del operador | Tesorería + Ledger | AC-USR-011, AC-FND-* | NO_ACREDITADO |

### 2.4 GRF — Grafo, rutas y decisión (8)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-021 | USR-021 | §B007, §B014–§B016, §B026, §B036, §B038 | Grafo dinámico multichain/multivenue | Backend | `PAPER_SHADOW` | **PREPARACION** | Snapshots coherentes | Construir aristas con chain, pool, fee, cantidad, vigencia | Cada arista con procedencia y vigencia | Arista sin vigencia no es usable | Local | Frescura de snapshot (SSOT §A08; sin valor numérico en el SSOT) | Backend + Network | AC-USR-033 | NO_ACREDITADO |
| AC-USR-022 | USR-022 | §B014, §B015, §B018–§B020, §B031, §B032, §B042 | Rutas de 2 a 7 hops | Backend | `PAPER_SHADOW` | **PREPARACION** | Grafo construido | Enumerar y validar ciclos por longitud | Rutas con ciclos rechazados | Compatibilidad de encoder por longitud | Local | **2 ≤ hops ≤ 7** (SSOT `acceptance` de USR-022) | Quant + Backend | AC-USR-021 | NO_ACREDITADO |
| AC-USR-023 | USR-023 | §B009, §B021–§B024, §B029 | Seleccionar mejor ruta y tamaño | Quant | `PAPER_SHADOW` | **PREPARACION** | Rutas enumeradas | Optimizar tamaño por ruta | Ruta y tamaño elegidos | **Exactitud o brecha óptima documentada bajo presupuesto** | Local | Brecha óptima dentro del presupuesto (sin valor numérico en el SSOT) | Quant | AC-USR-022 | NO_ACREDITADO |
| AC-USR-024 | USR-024 | §A06, §A13, §A29, §B028, §B030–§B032, §B035, §B036 | Contabilidad por leg y por ciclo | Ledger | `PAPER_SHADOW` | **PREPARACION** | Ciclo completo | Contabilizar cashflows intermedios | Fee y principal contados exactamente una vez | **Cashflows intermedios no se suman como ganancias independientes** | Local | Sin umbral numérico | Ledger + Quant | AC-USR-019 | NO_ACREDITADO |
| AC-USR-025 | USR-025 | §D08, §D36, §E03, §E07, §E34 | Detección y decisión en milisegundos | Backend | `PAPER_SHADOW` | **PREPARACION** | Instrumentación de latencia | Medir latencia del pipeline | p50, p95, p99 locales separados de RPC e inclusión | **Separar cómputo de RPC/inclusión/finality** | Local | **p50/p95/p99** (SSOT `acceptance` de USR-025); sin valor absoluto exigido → no se inventa | SRE + Backend | AC-USR-068 | NO_ACREDITADO |
| AC-USR-026 | USR-026 | §A08, §A18, §D07, §D11–§D14 | Evaluación y validación previas al trade | Backend | `PAPER_SHADOW` | **AUTORIZACION** | Plan vigente | Validar quote, sizing, simulación, gates y firma | Todo ligado al MISMO plan y estado vigente | Plan vencido no admite validación | Local | Sin umbral numérico | Backend + Risk | AC-USR-023 | NO_ACREDITADO |
| AC-USR-027 | USR-027 | §D23, §D24 | Atomicidad verificable | Backend | `TESTNET` | **EJECUCION** | Alcance de atomicidad declarado | Declarar atomicidad solo en el alcance soportado | Declaración con su alcance explícito | **Liquidación no atómica se modela aparte** | Local/Testnet | Sin umbral numérico | Backend | AC-USR-026 | NO_ACREDITADO |
| AC-USR-028 | USR-028 | §E17, §F02, §F04, §F06 | Priorizar cadenas de mayor TVL | Backend | `PAPER_SHADOW` | **CATALOGO_HOMOLOGACION** | Fuente de TVL con fecha | Rankear cadenas por TVL | Ranking con fuente, fecha y tie-break | **TVL no sustituye liquidez ejecutable** | Local | Ranking dinámico (sin valor numérico en el SSOT) | Network | — | NO_ACREDITADO |

### 2.5 NET — Cadenas, venues y descubrimiento (8)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-029 | USR-029 | §E17, §F02, §F04, §F05 | Top 5 cadenas configurable | Operador | `PAPER_SHADOW` | **CATALOGO_HOMOLOGACION** | Ranking de AC-USR-028 | Seleccionar subconjunto elegible | Ranking global y subconjunto visibles por separado | **Sin sustitución silenciosa** | Local | **Top 5 configurable** (SSOT `acceptance` de USR-029) | Network + Frontend | AC-USR-028 | NO_ACREDITADO |
| AC-USR-030 | USR-030 | §D10, §D26, §D38 | Extensibilidad a otras blockchains | Backend | `PAPER_SHADOW` | **CATALOGO_HOMOLOGACION** | Contrato de adapter | Registrar un adapter EVM o no-EVM | Rechazo explícito de `unsupported` | Adapter sin probe no se homologa | Local | Sin umbral numérico | Backend + Network | — | NO_ACREDITADO |
| AC-USR-031 | USR-031 | §C013, §C018, §C054, §C055 | Agregar DEX y CEX desde la plataforma | Operador | `PAPER_SHADOW` | **CATALOGO_HOMOLOGACION** | Registro versionado | Dar de alta un venue | Venue con capacidades, límites y homologación | **Homologación previa a operación** | Local | Sin umbral numérico | Network + Frontend | AC-USR-030 | NO_ACREDITADO |
| AC-USR-032 | USR-032 | §E17, §F03, §F04, §F06 | Top 10 DEX por volumen 24 h | Backend | `PAPER_SHADOW` | **CATALOGO_HOMOLOGACION** | Fuente de volumen | Rankear DEX | Ranking con métrica, ventana y dedup declaradas | **Volumen no prueba mejor ejecución** | Local | **Top 10 / ventana 24 h** (SSOT `acceptance` de USR-032) | Network | AC-USR-028 | NO_ACREDITADO |
| AC-USR-033 | USR-033 | §D08, §D10 | Descubrimiento de pools | Backend | `PAPER_SHADOW` | **PREPARACION** | Acceso RPC indexado y streaming | Indexar y descubrir pools | Pool con validación de factory y dedup | Reorg tratada explícitamente | Local | Sin umbral numérico | Backend + Network | — | NO_ACREDITADO |
| AC-USR-034 | USR-034 | §A06, §D09, §D10, §D28 | Descubrimiento y gestión de tokens | Backend | `PAPER_SHADOW` | **PREPARACION** | Identidad chain+address | Alta de token | Decimales verificados y compatibilidad económica | Identidad por chain+address; sin sustitución | Local | Sin umbral numérico | Backend + Network | **AC-USR-* vinculado a hallazgo F17/F20** | NO_ACREDITADO |
| AC-USR-035 | USR-035 | §E17, §E18 | Condiciones y listas de pools y tokens | Operador | `PAPER_SHADOW` | **PREPARACION** | — | Filtrar por calidad y liquidez ejecutable | Cada exclusión con su razón registrada | **Razón de exclusión obligatoria (R8)** | Local | Frescura y liquidez ejecutable (sin valor numérico en el SSOT) | Network + Quant | AC-USR-033 | NO_ACREDITADO |
| AC-USR-036 | USR-036 | §D17, §D26–§D29 | Agregar y administrar wallets | Operador | `PAPER_SHADOW` | **CATALOGO_HOMOLOGACION** | — | Alta de wallet watch-only | Wallet con red, permisos, balance, nonces | **Watch-only y firmante separados** | Local | Sin umbral numérico | Frontend + Custodia | — | NO_ACREDITADO |

### 2.6 WAL — Wallets y custodia (2)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-037 | USR-037 | §A19, §D03, §D18, §D25, §D32 | Conectar activos sin exponer claves a LLM | Custodia | `PAPER_SHADOW` | **AUTORIZACION** | Signer aislado | Otorgar referencia de identidad | Referencia, nunca clave; mínimo privilegio | **Ninguna clave expuesta a LLM** | Local | Sin umbral numérico | Custodia + Security | **AC-FND-30, AC-FND-31** | NO_ACREDITADO |
| AC-USR-038 | USR-038 | §C016, §C017, §C020, §C057, §C060 | Agregar estrategias desde DApp | Operador | `PAPER_SHADOW` | **CATALOGO_HOMOLOGACION** | DSL o plugin controlado | Dar de alta una estrategia | Estrategia con esquema y versión | **Sin código libre por LLM** | Local | Sin umbral numérico | Frontend + Backend | AC-USR-039 | NO_ACREDITADO |
| AC-USR-039 | USR-039 | §B040–§B043, §B045 | Validar estrategias antes de operar | QA | `PAPER_SHADOW` | **PREPARACION** | Estrategia registrada | Correr unit, golden, fork y estrés | Evidencia por nivel de prueba | **Exposición gradual con rollback** | Local | Sin umbral numérico | QA + Quant | AC-USR-038 | NO_ACREDITADO |

### 2.7 STR — Catálogo de estrategias (3)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-040 | USR-040 | §A11, §A35, §B014, §B038, §B039, §B045, §E23, §E27, §E32 | Completar catálogo y capacidades iniciadas | Quant | — | **CATALOGO_HOMOLOGACION** | — | Completar la matriz de IDs del catálogo | Matriz con familias, operadores, datos, encoder, readiness | **Registro ≠ ejecución; readiness ≠ capacidad probada** | Local | **264 IDs** (SSOT §3 #7; `contratos_264_estrategias.json` = 264) | Quant + Backend | — | NO_ACREDITADO |
| AC-USR-041 | USR-041 | §B022, §B024, §B033, §B037–§B039 | Elegir estrategia por mercado | Quant | `PAPER_SHADOW` | **PREPARACION** | Features medidos | Estimar y seleccionar | Selección respaldada por resultados fuera de muestra | **Drift declarado; sin extrapolación** | Local | Sin umbral numérico | Quant | **AC-FND-40** | NO_ACREDITADO |
| AC-USR-042 | USR-042 | §E02, §E03, §E04 | Operación mediante API de cualquier LLM elegible | Operador | `PAPER_SHADOW` | **PREPARACION** | API versionada | Conectar un proveedor LLM | Adapter con roles y tool contracts | Compatibilidad comprobada, no supuesta | Local | Sin umbral numérico | Backend | — | NO_ACREDITADO |

### 2.8 API — Interfaz de agentes (5)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-043 | USR-043 | §E01, §E04, §E05, §E06 | Agentes operan como usuarios sin tocar código | Agente | `PAPER_SHADOW` | **AUTORIZACION** | Herramientas acotadas | Ejecutar una acción de producto | Acción equivalente a la del usuario | **Sin shell; sin modificar repositorio** | Local | Sin umbral numérico | Backend | AC-USR-042 | NO_ACREDITADO |
| AC-USR-044 | USR-044 | §E05–§E09 | Equipo de agentes especializados | Planner | `PAPER_SHADOW` | **GOBERNANZA_DOCUMENTAL** | Roles definidos | Coordinar agentes | Trazabilidad por agente y rol | **Separación propuesta/riesgo/ejecución** | Local | Sin umbral numérico | Planner | — | NO_ACREDITADO |
| AC-USR-045 | USR-045 | §E10, §E20, §E32 | Observación y adaptación de parámetros | Backend | `PAPER_SHADOW` | **AUTORIZACION** | Métricas disponibles | Proponer un cambio de parámetro | Diff de política con autorización | **Cambio versionado; sin autoaplicación** | Local | Sin umbral numérico | Backend + Risk | AC-USR-052 | NO_ACREDITADO |
| AC-USR-046 | USR-046 | §C058, §C059, §C064, §E10 | Chat dentro de la plataforma | Operador | `PAPER_SHADOW` | **PREPARACION** | Chat disponible | Convertir intención en propuesta tipada | Propuesta tipada con confirmación | Contexto protegido | Local | Sin umbral numérico | Frontend + Backend | AC-USR-043 | NO_ACREDITADO |

### 2.9 RUL — Reglas y gates (6)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-047 | USR-047 | §A16, §A17, §E14, §E16 | Horario 08–18 configurable | Operador | `PAPER_SHADOW` | **PREPARACION** | Timezone y calendario | Configurar ventana horaria | Corte de admisión aplicado | DST tratado; operaciones en vuelo definidas | Local | **08:00–18:00 configurable** (SSOT `acceptance` de USR-047) | Backend + Frontend | — | NO_ACREDITADO |
| AC-USR-048 | USR-048 | §E12, §E13 | Lógica AND/OR anidada | Operador | `PAPER_SHADOW` | **PREPARACION** | AST tipado | Evaluar una expresión anidada | Resultado con explicación por cláusula | **Evaluación tri-valuada** | Local | Sin umbral numérico | Backend | — | NO_ACREDITADO |
| AC-USR-049 | USR-049 | §E19 | Cumplir todas o primera condición | Operador | `PAPER_SHADOW` | **PREPARACION** | Reglas definidas | Evaluar admisión vs stop goals | Distinguir «todas» de «primera alcanzada» | Regla de empate y estado durable | Local | Sin umbral numérico | Backend | AC-USR-048 | NO_ACREDITADO |
| AC-USR-050 | USR-050 | §E16, §E18 | Ejemplo ROI15 net2000 capital100 | Specification | — | **GOBERNANZA_DOCUMENTAL** | — | Publicar el ejemplo canónico | El ejemplo muestra que requiere ROI > 2000 % | **No es garantía ni contradicción formal** | Local | **net > 2000 y principal ≤ 100 ⇒ ROI > 2000 %** (SSOT `acceptance` de USR-050) | Specification + Quant | AC-USR-018 | NO_ACREDITADO |
| AC-USR-051 | USR-051 | §E13, §E22 | Seguridad no anulable mediante OR | Risk | — | **AUTORIZACION** | Gates de seguridad separados | Evaluar gates antes de firma | Gate de seguridad no eludible por la expresión comercial | **OR comercial no puede anular seguridad** | Local | Sin umbral numérico | Risk + Security | **AC-FND-32, AC-FND-33** | NO_ACREDITADO |
| AC-USR-052 | USR-052 | §A10, §E10, §E12, §E20 | Cambios de configuración por chat | Operador | `PAPER_SHADOW` | **AUTORIZACION** | Config versionada | Aplicar un cambio vía chat | Comparación de impacto + autorización | **Revisión monotónica; planes viejos invalidados** | Local | Sin umbral numérico | Backend | AC-USR-045 | NO_ACREDITADO |

### 2.10 MSG — Mensajería (2)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-053 | USR-053 | §E11 | Mensajería WhatsApp / Telegram / email | Sistema | — | **PREPARACION** | Conectores oficiales | Enviar una notificación | Destino verificado y consentimiento | **Dedup y trazabilidad; datos sensibles minimizados** | Local | Sin umbral numérico | Backend | — | NO_ACREDITADO |
| AC-USR-054 | USR-054 | §A17, §E11, §E19, §E29 | Aviso de objetivos y cierre de jornada | Sistema | — | **PREPARACION** | Fuente contable | Emitir evento de cierre | Evento con fuente contable y estado | Datos sensibles minimizados | Local | Sin umbral numérico | Backend + Ledger | AC-USR-053, AC-USR-059 | NO_ACREDITADO |

### 2.11 LED — Ledger y contabilidad (5)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-055 | USR-055 | §F16, §F18–§F20 | Extractos y estados de cuenta | Ledger | — | **PREPARACION** | Movimientos registrados | Emitir un extracto | Saldo inicial→movimientos→fees→PnL→saldo final | Cada cifra con referencia comprobable | Local | Sin umbral numérico | Ledger | **AC-FND-19** | NO_ACREDITADO |
| AC-USR-056 | USR-056 | §F12, §F18–§F20 | Balance de wallets y CEX | Ledger | — | **PREPARACION** | Fuentes on-chain y exchange | Conciliar saldos | Disponible / retenido / pendiente diferenciados | Sin colapsar estados | Local | Sin umbral numérico | Ledger | AC-USR-055 | NO_ACREDITADO |
| AC-USR-057 | USR-057 | §F15, §F17, §F20 | Valorización fiat USD/COP/EUR y activos digitales | Ledger | — | **PREPARACION** | Fuentes de precio con timestamp | Valorizar | Precisión y método FX declarados | **Precio ausente = estado explícito, no cero** | Local | Sin umbral numérico | Ledger + Quant | **AC-FND-20** | NO_ACREDITADO |
| AC-USR-058 | USR-058 | §A13, §A29, §F16, §F17, §F19, §F20 | Doble partida personal/institucional | Ledger | — | **PREPARACION** | Eventos contables | Asentar por evento | Asiento idempotente con reverso | **Sin alterar historia** | Local | Sin umbral numérico | Ledger | AC-USR-019 | NO_ACREDITADO |
| AC-USR-059 | USR-059 | §A16, §A17, §F11, §F17, §F19, §F20, §F30 | Resultados de gestión de jornada | Ledger | — | **PREPARACION** | Ventana y timezone | Cerrar jornada | Resultado con intentos perdedores y costos completos | **Pérdidas y días sin actividad se conservan** | Local | Sin umbral numérico | Ledger + QA | AC-USR-058 | NO_ACREDITADO |

### 2.12 OFF — Off-ramps y monetización (3)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-060 | USR-060 | §E32, §F07–§F11, §F22, §F23, §F28 | Binance, Bybit y canales P2P | Operador | `LIVE_MAINNET` | **CATALOGO_HOMOLOGACION** | Fuente oficial de disponibilidad | Verificar capacidad regional | Capacidad verificada con fuente oficial | **Riesgo de contraparte y costo neto declarados** | Mainnet | Requiere autorización del operador para fondear | Network + Operador | — | NO_ACREDITADO |
| AC-USR-061 | USR-061 | §E32, §F22, §F24–§F26, §F28, §F29 | Nequi, Wenia, PayPal y Airtm | Operador | `LIVE_MAINNET` | **CATALOGO_HOMOLOGACION** | — | Clasificar cada canal | Wallet fiduciaria vs cripto distinguida | **Ausencia de API se declara, no se asume** | Mainnet | Requiere autorización del operador | Network + Operador | — | NO_ACREDITADO |
| AC-USR-062 | USR-062 | §F21, §F27, §F28, §F30 | Sugerencia de monetización | Sistema | `LIVE_MAINNET` | **PREPARACION** | Alternativas cotizadas | Proponer una ruta de monetización | Alternativa con tiempo, riesgo y límites | **Aprobación del propietario antes de retiro** | Mainnet | Requiere autorización del operador | Ledger + Operador | AC-USR-060 | NO_ACREDITADO |

### 2.13 OPS — Operación, seguridad y cierre (8)

| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |
|----|-----|---------------|-------------------|-------|------|-----|----------------|--------|----------------------|-------------|-----------|-----------------|-------|------|--------|
| AC-USR-063 | USR-063 | §A19, §A30, §D25, §D27, §D30–§D34, §E07, §E08, §E22, §E30 | Seguridad institucional de uso personal | Security | — | **AUTORIZACION** | — | Aplicar aislamiento de claves, auth y límites | Controles activos y auditados | **No presentarse como banco licenciado** | Local | Sin umbral numérico | Security | **AC-FND-30, AC-FND-31, AC-FND-34** | NO_ACREDITADO |
| AC-USR-064 | USR-064 | §A15, §A17, §A18, §A20–§A33, §A35, §D14–§D16, §D20, §D37, §E07, §E28, §E34 | Continuidad operativa | SRE | — | **PREPARACION** | Infraestructura instrumentada | Ejercer replay idempotente y restauración | Outbox/journal/PEL/DLQ operativos | **Restauración probada, no declarada** | Local/VPS | Sin umbral numérico | SRE + Backend | **AC-FND-07, AC-FND-08, AC-FND-10** | NO_ACREDITADO |
| AC-USR-065 | USR-065 | §A12, §A14, §A34, §E09, §E23, §E34 | Observabilidad y SLO | SRE | — | **PREPARACION** | Indicadores definidos | Publicar salud y SLO | Fuente de verdad por indicador | **`NO COMPUTADO` ≠ cero** | Local/VPS | Sin umbral numérico | SRE | **AC-FND-12** | NO_ACREDITADO |
| AC-USR-066 | USR-066 | §A04, §A10, §A31, §A32, §D37–§D40, §E24, §E25, §E33–§E35 | CI/CD y despliegue reproducible | DevOps | — | **AUTORIZACION** | Pipeline de CI | Desplegar con SHA exacto | Artefacto e imagen attestados | **Rollback y gates sin autoengaño** | Local/VPS | **SHA exacto** (SSOT `acceptance` de USR-066) | DevOps | **AC-FND-01, AC-FND-02, AC-FND-04, AC-FND-35** | NO_ACREDITADO |
| AC-USR-067 | USR-067 | §A35, §D39, §D40, §E23, §E25–§E31, §E35 | Cierre de los 40 hallazgos de auditoría | Planner | — | **GOBERNANZA_DOCUMENTAL** | Catálogo F01–F40 | Cerrar cada hallazgo | Cada hallazgo con dueño, etapa y evidencia | **Propuesta de cierre ≠ reparación técnica** | Local | **40/40 hallazgos** (SSOT §3 #2; hoy 40/40 en `PROPUESTO_NO_IMPLEMENTADO`) | Planner + por dominio | AC-FND-01…AC-FND-40 | NO_ACREDITADO |
| AC-USR-068 | USR-068 | §A21, §A22, §A25, §A34, §D08, §D16, §D36, §E07, §E18, §E34 | Rendimiento y escalamiento medidos | SRE | — | **PREPARACION** | Carga reproducible | Correr benchmarks por carga | Benchmark reproducible y backpressure | **Sin prometer ausencia de latencia** | Local/VPS | Sin umbral numérico (no se inventa un SLA) | SRE + Backend | AC-USR-025 | NO_ACREDITADO |
| AC-USR-069 | USR-069 | §A18, §A19, §A30, §D13, §D17, §D22, §D28, §D33, §D35, §E07, §E15, §E18, §E20, §E22, §E30 | Control de riesgo y capital | Risk | `PAPER_SHADOW` | **AUTORIZACION** | Límites del operador | Evaluar exposición, drawdown y pérdida diaria | Bloqueo con estado durable | **Stop y límites con autorización; estado durable** | Local | Límites del operador (no constan en el SSOT → sin umbral numérico) | Risk | **AC-FND-32, AC-FND-33** | NO_ACREDITADO |
| AC-USR-070 | USR-070 | §A13, §A28, §A29, §D20, §D21, §D29, §E09, §E15, §E28, §E31, §E35, §F11, §F13, §F17, §F19–§F21, §F30 | Ganancia real final conciliada | Ledger + Operador | `LIVE_MAINNET` | **EJECUCION** | Receipt y finality | Conciliar el ciclo real | Receipt + finality + saldo + costos en ledger | **`mock` y estimación no se confunden con ganancia** | Mainnet | Requiere autorización explícita del operador | Ledger + Operador | AC-USR-011, AC-FND-* | NO_ACREDITADO |

---

## 3. Matriz — 40 hallazgos de auditoría (`AC-FND-*`)

Fuente: `hallazgos/catalogo_F01_F40.json` (40) + `hallazgos/plan_cierre_F01_F40.json` (`priority_proposed`, `owner_role`, `stage`, `acceptance_evidence`, `dependencies`). **Los 40 comparten `estado_inicial = NO_ACREDITADO` y `status` SSOT `PROPUESTO_NO_IMPLEMENTADO_EN_ESTA_TAREA`**; la matriz no repite esa columna para no fatigar la lectura. Nota del propio SSOT: *"Prioridad y responsables son propuesta de planificación… Esta matriz no declara implementadas las correcciones."*

**Distinción de eje aplicada:** todo hallazgo cuyo cierre exige broadcast/firma/settlement va a `EJECUCION`; los de seguridad a `AUTORIZACION`; los de datos/cálculo a `PREPARACION`; los de proceso/repo/CI a `GOBERNANZA_DOCUMENTAL`.

| ID | F | Título (SSOT) | Prioridad propuesta (origen: SSOT `priority_proposed`) | Dueño (origen: SSOT `owner_role`) | Etapa (origen: SSOT `stage`) | Eje | Evidencia de cierre requerida (origen: SSOT `acceptance_evidence`) |
|----|---|---------------|--------------------------------------------------------|-----------------------------------|-------------------------------|-----|------------------------------------------------------------------|
| AC-FND-01 | F01 | Integración Rust impide promover main | P1 | Backend/Quant | 0–10: procedencia y baseline | GOBERNANZA_DOCUMENTAL | Suite Rust del SHA combinado sin ocultar `DATA_GAP` |
| AC-FND-02 | F02 | Pipeline Integrity no verifica el recorrido | P1 | SRE | 0–10 | GOBERNANZA_DOCUMENTAL | Trazas por hop y prueba de servicio detenido detectada |
| AC-FND-03 | F03 | Simulación obligatoria sin benchmark válido | P1 | Quant/QA | 0–10 | PREPARACION | Artifacts exact-SHA, población y descartes completos |
| AC-FND-04 | F04 | Main sin protección efectiva | P1 | Repo admin | 0–10 | GOBERNANZA_DOCUMENTAL | Configuración efectiva verificada y bloqueo ensayado |
| AC-FND-05 | F05 | El modo mostrado no acredita el modo efectivo | P1 | API/frontend | 55–70: paper y DApp | PREPARACION | Seleccionado, solicitado y aplicado son distinguibles |
| AC-FND-06 | F06 | Testnet no completa la vía canónica | P1 | Backend/adapters | 70–82: testnet y seguridad | EJECUCION | Mismo plan llega a receipt y ledger de esa red |
| AC-FND-07 | F07 | Persistir no garantiza publicar el siguiente evento | P1 | Backend | 40–55: plan y continuidad | PREPARACION | Crash COMMIT/XADD/XACK no pierde ni duplica efecto |
| AC-FND-08 | F08 | Los pendientes no se recuperan en dos consumidores | P1 | SRE/backend | 40–55 | PREPARACION | Reinicio con consumidor huérfano recupera trabajo |
| AC-FND-09 | F09 | La entrada HTTP omite la persistencia común | P1 | Executor | 40–55 | PREPARACION | Ambas crean execution intent, attempt y ledger durable |
| AC-FND-10 | F10 | El tracker puede esperar indefinidamente | P1 | Executor | 40–55 | PREPARACION | RPC caído termina espera acotada sin abandonar seguimiento |
| AC-FND-11 | F11 | Paper no evalúa todos los gates live | P1 | Backend/QA | 55–70 | PREPARACION | Mismos inputs ejercitan gates comunes; diferencias documentadas |
| AC-FND-12 | F12 | Selector declara salud tras fallar el grupo Redis | P1 | Selector | 40–55 | PREPARACION | Fallo de `ensureGroup` no declara readiness falsa |
| AC-FND-13 | F13 | La sesión HTTP no autentica los ACK por WebSocket | P1 | Identidad/frontend | 55–70 | AUTORIZACION | Sesión legítima recibe ACK propio; tercero no puede falsificarlo |
| AC-FND-14 | F14 | Snapshots y errores pueden conservar cards obsoletas | P1 | Frontend | 55–70 | PREPARACION | Snapshot vacío retira cards; error no aparece como feed vivo |
| AC-FND-15 | F15 | La card no refleja el resultado real de simulación | P1 | Frontend/API | 55–70 | PREPARACION | Valor y estado provienen del resultado identificado |
| AC-FND-16 | F16 | Los E2E y el validador permiten éxitos incompletos | P1 | QA | 55–70 | GOBERNANZA_DOCUMENTAL | Ausencia de build, datos o aserción requerida falla |
| AC-FND-17 | F17 | El sizing sustituye tokens desconocidos | **P0** | Quant | 10–25: datos y economía | PREPARACION | Token desconocido produce `DATA_GAP`, no WETH supuesto |
| AC-FND-18 | F18 | Kelly modifica el importe y deja economics anteriores | **P0** | Quant | 10–25 | PREPARACION | `amount`, economics, principal y payload coinciden |
| AC-FND-19 | F19 | Los modelos económicos no comparten un ledger | **P0** | Contabilidad/backend | 10–25 | PREPARACION | Fees embebidos no se descuentan otra vez; pérdidas conservadas |
| AC-FND-20 | F20 | Gas ausente se valora como mínimo o cero | **P0** | Datos/Quant | 10–25 | PREPARACION | Dato ausente bloquea; cero acreditado sigue válido |
| AC-FND-21 | F21 | History y métricas cambian el significado del PnL | P1 | Contabilidad/UI | 55–70 | PREPARACION | Pérdidas, días sin actividad y gastos contados una vez |
| AC-FND-22 | F22 | Recon legacy no acredita conciliación USD | P1 | Contabilidad | 40–55 | PREPARACION | Receipt, finality y PnL evidenciado antes de `reconciled` |
| AC-FND-23 | F23 | Workers opcionales escriben resultados no realizados | P1 | Backend | 55–70 | PREPARACION | No copian predicho a actual ni fabrican valoración |
| AC-FND-24 | F24 | La cotización V3 y su cota necesitan trazabilidad | P1 | Routing | 10–25 | PREPARACION | Pruebas multitick y quote diferencial al mismo estado |
| AC-FND-25 | F25 | El callback Balancer no se vincula a una solicitud | **P0** | Contratos | 70–82 | AUTORIZACION | Callbacks no solicitados y payload alterado revierten |
| AC-FND-26 | F26 | El repago puede consumir beneficios previos | **P0** | Contratos | 70–82 | EJECUCION | El préstamo no consume reservas previas del wrapper |
| AC-FND-27 | F27 | Los beneficios no tienen salida operativa del wrapper | **P0** | Contratos/tesorería | 82–90: canary y tesorería | EJECUCION | Destino correcto, permisos y eventos; sin retirar principal ajeno |
| AC-FND-28 | F28 | La pausa de emergencia queda detrás del timelock | **P0** | Seguridad | 70–82 | AUTORIZACION | Pausa rápida; no despausa, upgrade ni retiro no autorizado |
| AC-FND-29 | F29 | Adapter Aave aún no homologado | P1 | Contratos/QA | 25–40: búsqueda y catálogo | CATALOGO_HOMOLOGACION | Fork autorizado reproduce préstamo y repayment exacto |
| AC-FND-30 | F30 | Claves descifradas se proyectan al Redis compartido | **P0** | Custodia | 70–82 | AUTORIZACION | Servicio de bajo privilegio no obtiene clave |
| AC-FND-31 | F31 | SSR de credenciales presta autoridad del servidor | **P0** | Frontend/seguridad | 70–82 | AUTORIZACION | Anónimo no recibe inventario ni metadata sensible |
| AC-FND-32 | F32 | Los breakers no forman un control integral de ejecución | **P0** | Risk | 70–82 | AUTORIZACION | Pérdida, gas, drawdown y evaluador stale bloquean |
| AC-FND-33 | F33 | Kill y paper pueden cambiar después de la validación | **P0** | Executor | 70–82 | AUTORIZACION | Kill antes de broadcast produce cero envíos |
| AC-FND-34 | F34 | Sesión revocable e identidad de operador incompletas | P1 | Identidad | 70–82 | AUTORIZACION | Logout/expiración y scopes rechazan replay |
| AC-FND-35 | F35 | Dependencias con alertas y aplicabilidad pendiente | P1 | Seguridad/SRE | 82–90 | GOBERNANZA_DOCUMENTAL | Sin critical/high aplicable o excepción específica vigente |
| AC-FND-36 | F36 | Falta acreditar cierre del incidente de clave WS | P1 | Seguridad/operador | 0–10 | AUTORIZACION | Constancia de revocación y reconexión sin revelar secreto |
| AC-FND-37 | F37 | El canon v4 no atraviesa sus dos fronteras principales | **P0** | Backend/Quant | 25–40 | PREPARACION | Contexto productivo atraviesa `economic_check`; falta individual rechaza |
| AC-FND-38 | F38 | Ready no acredita solvers ni verificadores de dominio | P1 | Equipos de dominio | 25–40 | CATALOGO_HOMOLOGACION | Cada restricción acredita propiedad real, no solo forma |
| AC-FND-39 | F39 | Los nombres de protocolos exceden el soporte exacto | P1 | Adapters | 25–40 | CATALOGO_HOMOLOGACION | Diferencial contra deployment y tamaño/dirección reales |
| AC-FND-40 | F40 | Operador computado no implica señal válida ni calibrada | P1 | Math | 10–25 | PREPARACION | Mezcla de pares rechazada; calibración fuera de muestra |

**Reparto P0/P1 (recuento propio sobre el SSOT).** `plan_cierre_F01_F40.json.rows` agrupado por `priority_proposed` da **P0 = 13** y **P1 = 27** (13 + 27 = 40 ✓). Los 13 P0 son: F17, F18, F19, F20, F25, F26, F27, F28, F30, F31, F32, F33, F37. Reparto por etapa: 0–10 → 5 · 10–25 → 6 · 25–40 → 4 · 40–55 → 6 · 55–70 → 8 · 70–82 → 9 · 82–90 → 2 (5+6+4+6+8+9+2 = 40 ✓). **La severidad NO viene de `catalogo_F01_F40.json`** (ese archivo trae `id/title/paragraphs/sources`, sin campo de severidad: recuento propio → agrupación vacía para los 40). La prioridad se toma de `plan_cierre_F01_F40.json.priority_proposed`, que el propio SSOT marca como **propuesta de planificación**. No se inventa una severidad que el SSOT no declare.

---

## 4. Matriz — fronteras de fuente (`AC-FRT-*`), 5 filas

Fuente: `SOURCE-MANIFEST.md` §4 (medido). Estas filas **cuentan en N**: una fuente ausente que se saca del denominador es un bloqueo disfrazado de silencio.

| ID | F | Fuente AUSENTE | Ruta buscada | Comando que prueba la ausencia | Eje | Qué parte del censo bloquea | Estado inicial |
|----|---|----------------|--------------|-------------------------------|-----|------------------------------|----------------|
| AC-FRT-1 | F-1 | `user_requirements.json` | `arbitragex-v2-main (17)\**` + `.claude\worktrees\*` | Bloque A §5 SOURCE-MANIFEST → `ROWS = 0` | GOBERNANZA_DOCUMENTAL | Nombre literal no resoluble por ruta. Contenido equivalente **acreditado**: 70 filas (SOURCE-MANIFEST §4.2) | `BLOQUEADO_NO_CENSABLE` |
| AC-FRT-2 | F-2 | `ui_route_inventory.json` | ídem | Bloque A → `ROWS = 0` | GOBERNANZA_DOCUMENTAL | Ídem. Contenido equivalente **acreditado**: 59 rutas (§4.2) | `BLOQUEADO_NO_CENSABLE` |
| AC-FRT-3 | F-3 | `audit_catalog_F01_F40.json` | ídem | Bloque A → `ROWS = 0` | GOBERNANZA_DOCUMENTAL | Ídem. Contenido equivalente **acreditado**: 40 hallazgos (§4.2) | `BLOQUEADO_NO_CENSABLE` |
| AC-FRT-4 | F-4 | `formula_registry.json` | ídem | Bloque A → `ROWS = 0`; `fuentes_curadas.json` → **NO-DECLARADO** | GOBERNANZA_DOCUMENTAL | **Equivalencia de identidad NO ACREDITADA.** Contenido presente (34 696 literales) pero el nombre literal no consta en la curaduría del anexo | `BLOQUEADO_POR_FUENTE` |
| AC-FRT-5 | F-5 | `formula_bindings_index.json` | ídem | Bloque A → `ROWS = 0`; `fuentes_curadas.json` → **PRESENTE**, sha256 `658d9849…` | GOBERNANZA_DOCUMENTAL | Ídem. Contenido equivalente **acreditado parcial**: 253 bindings propuestos (+ `contratos_hojas.json`) | `BLOQUEADO_NO_CENSABLE` |

---

## 5. Frontera de mayor severidad (F-6/F-7/F-8): el repo no contiene su propia autoridad

Esta matriz y su manifiesto par se apoyan en el **Anexo Técnico**, que vive en `C:\Users\HFRC\Downloads` — **fuera** del repositorio (SOURCE-MANIFEST §2.3, F-6/F-7/F-8). Consecuencia medida y declarada:

> **Un lector que solo tenga el repositorio NO puede reproducir el censo §3 de SOURCE-MANIFEST.** El White Paper (PDF/DOCX), el Anexo Técnico y 3 de los 5 libros (B2, B4, B5 → **28 de 75 hojas**) están ausentes del workspace: `ROWS = 62` en el barrido de documentos, **0** de ellos es fuente raíz. Y el checkout **no es quiescente** (SOURCE-MANIFEST Q-1: 50 mods / 311 untracked medidos, contra 34/295 en el acta), de modo que cualquier conteo de árbol caduca entre corridas.

Esto no es un defecto de esta entrega: es el hallazgo. `N = 115` se congela **sobre lo enumerado y acreditado**, y la parte del censo que depende de las fuentes ausentes queda marcada `BLOQUEADO_*` en vez de estimada.

---

## 6. Eje de fórmula — propuesta vs homologación vs ejecución (sub-censo `M = 3`)

El §4 del PROMPT MAESTRO exige distinguir **fórmula propuesta** de **función ejecutada**. Este sub-censo es **categórico, no aditivo**: va aparte de `N = 115` para no inflar el denominador con unidades de naturaleza distinta (**R5**).

| ID | Eje de fórmula | Artefacto que lo acredita | Recuento propio (comando) | Qué es | Qué NO es |
|----|----------------|---------------------------|---------------------------|--------|-----------|
| AC-FRM-1 | **PROPUESTA** | `excel/bindings_propuestos.jsonl` | **253** líneas | Binding de campo/tipo/unidad/numeraire **propuesto** | No es contrato de campo implementado; su estado SSOT es `PROPUESTO_NO_IMPLEMENTADO` |
| AC-FRM-2 | **HOMOLOGACIÓN** | `excel/literales_formula.jsonl` + `excel/familias_formula.jsonl` | **34 696** literales únicos · **1 509** familias | Fórmula registrada y agrupada por patrón | **Familia de patrón ≠ función semántica aprobada**; homologar no es implementar |
| AC-FRM-3 | **EJECUCIÓN** | `pruebas/resultados_aritmeticos_ejecutados.json` vs `pruebas/reporte_aritmetico_ejecutado.json` | **6** ejecutados vs **1 045** reportados | Registro aritmético realmente ejecutado | Un check **reportado** no es un check **re-ejecutado**; una **caché** no es un recálculo nativo |

**Cuña cuantificada (la brecha que este eje existe para exponer):** de **34 696** literales únicos censados, **6** tienen registro de ejecución aritmética. Además, **56 843** valores son **caché guardada** y **4 038** fórmulas **no** tienen valor guardado (ausencia real, no cero). Cita del propio SSOT sobre los 6: *"Independent integer/Decimal arithmetic and saved-cache comparison; **not native Excel recalculation or backend execution**"*. **El gap entre 34 696 y 6 es de cuatro órdenes de magnitud y está materializado en el SSOT, no inferido por mí.**

### 6.1 Anclaje al motor real (sin inventar fórmulas)

La distinción «fórmula propuesta vs función ejecutada» tiene un caso ya documentado en este mismo directorio: `FORMULA_VENUE_BINDING_DISCREPANCIES.md` §2 (B-01 vs B-04/B-05) muestra dos módulos emitiendo contratos **contradictorios** sobre el pricing de `Curve`/`Balancer`, con `cex_dex_engine.rs:74` conteniendo `theoretical_profit = (diff * 5.0) - 5.0`, **literal sin autoridad documentada**. Ese literal es exactamente el objeto del eje `AC-FRM-1`: **propuesto sin fuente ⇒ `NO_HOMOLOGADO`**, y así debe quedar hasta que exista fórmula con autoridad. Esta matriz **no resuelve** ese caso (propietario: Quant + MarketMicrostructure); sólo lo ancla al eje.

---

## 7. Reglas de evaluación (vinculantes)

- **R1 — Congelación previa.** Ningún porcentaje se publica antes de que `scope_version` y `N` estén congelados. `N` cambia sólo con bump de `scope_version`, y las series históricas se comparan siempre contra el `N` de su propia versión.
- **R2 — Anti-paraguas.** Un `criterion_id` = una capacidad/flujo. Un criterio que agregue varios es inválido: se divide.
- **R3 — Modo explícito.** Toda fila declara `PAPER_SHADOW`, `TESTNET`, `LIVE_MAINNET` o `—`. `LIVE_MAINNET` **sólo se registra**; esta matriz no autoriza firma, broadcast ni deploy.
- **R4 — `ACEPTADO` exige artefacto.** `ACEPTADO` requiere evidencia de ejecución reproducible (salida de test, tally, SHA de artefacto, receipt). **Un PASS citado de un documento o de una skill NO es un artefacto reproducible.**
- **R5 — No mezclar naturalezas.** El sub-censo de fórmulas (§6) no entra en `N`. Sumarlo inflaría el denominador con unidades categóricas.
- **R6 — Frontera no oculta.** Una fuente ausente cuenta en `N` con estado `BLOQUEADO_*`. Nunca se resta del denominador ni se rellena con una estimación.
- **R7 — Estados sin sinónimos.** `NO_ACREDITADO ≠ RECHAZADO`; `EN_PREPARACION ≠ ACEPTADO`; `compila ≠ funciona`; `exit 0 ≠ el evento ocurrió`; `no computado ≠ cero`; `sin productor ≠ sin hallazgos`; `declarado ≠ probado`; `propuesto ≠ implementado`.
- **R8 — Leyenda de ausencia.** Un campo no computado se reporta **`NO COMPUTADO`** con su razón. Nunca `0`, nunca vacío.
- **R9 — Sin exclusión por dificultad.** Ningún requisito, hallazgo o frontera se excluye por ser difícil, por fallar o por no existir todavía. Los 70 + 40 + 5 están **todos** en la matriz. Los 3 requisitos con modo `LIVE_MAINNET` (AC-USR-011, AC-USR-020, AC-USR-070) y los 2 de off-ramp (AC-USR-060, AC-USR-061) son los más incómodos y **están**.
- **R10 — Trazabilidad obligatoria.** Toda fila cita fuente exacta (`section_id` del anexo, `id` del hallazgo o frontera del manifiesto). Una fila sin cita de fuente es inválida.
- **R11 — Umbral con origen.** Todo umbral numérico cita su origen (columna `acceptance` del SSOT o `priority_proposed` del plan de cierre). **Donde el SSOT no declara umbral, se escribe «sin umbral numérico» y NO se inventa un valor** (afecta a 47 de las 70 filas).

---

## 8. Índice de verificación

| Afirmación de esta matriz | Artefacto que la prueba |
|---------------------------|-------------------------|
| `N = 115` | 70 (`requisitos_vigentes.csv`) + 40 (`catalogo_F01_F40.json`) + 5 (`SOURCE-MANIFEST.md` §4) |
| 70/70 en `NO_IMPLEMENTADO` y `MAPEO_DOCUMENTAL_EXPLICITO` | `Group-Object` sobre `requisitos_vigentes.csv` → 70 y 70 (§C-2) |
| Las 8 cifras del §2 reproducen | `SOURCE-MANIFEST.md` §3, tabla de 8 filas con comando por fila |
| 253 / 34 696 / 1 509 / 54 / 6 / 1 045 | `SOURCE-MANIFEST.md` §3.1 |
| Integridad del anexo | `SHA256SUMS.txt` → `OK=45 MISMATCH=0 MISSING=0` |
| Los 5 nombres literales son AUSENTES | Bloque A → `ROWS = 0`; worktrees → `0` |
| 5 fronteras y su bloqueo | `SOURCE-MANIFEST.md` §4, F-1…F-9 |
| 13 hallazgos P0 y 27 P1 | `plan_cierre_F01_F40.json.priority_proposed` agrupado |
| Deriva del checkout (50/311) | `git status --porcelain` filtrado (SOURCE-MANIFEST Q-1) |

---

## 9. Cierre

- **N queda congelado en `115`** para `scope_version = arbx-scope-1.0.0`. Con esto, la aceptación global **deja de ser `NO_CALCULABLE`** en su denominador: existe `N`, existe la fórmula (§C-3) y existe el estado inicial declarado (`aceptados = 0`).
- **Lo que sigue `NO_CALCULABLE` es otra cosa, y se dice:** la tasa de **implementación** y cualquier cifra de **rentabilidad**. Esta matriz mide **cobertura de aceptación documental** contra un censo enumerado. Confundirla con progreso real es exactamente el error que R4/R7/R8 prohíben.
- **Ninguna fila se marcó `ACEPTADO`.** Las 115 arrancan en `NO_ACREDITADO`, `BLOQUEADO_NO_CENSABLE` o `BLOQUEADO_POR_FUENTE`, porque no existe todavía evidencia de ejecución para ninguna. Un `N` con `aceptados = 0` es un resultado válido y reportable; un `N` con porcentajes de adorno no lo es.
- **Siguiente paso natural (fuera de esta tarea):** revisión independiente de esta matriz por un revisor distinto del autor (la firma no la emite Specification), y decisión sobre F-4 (equivalencia de identidad de `formula_registry.json`) y F-9 (identidad del libro ULTRA).

---

*Matriz única y versionada. `scope_version = arbx-scope-1.0.0` · `N = 115` congelado antes de cualquier porcentaje.*
