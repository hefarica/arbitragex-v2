# SOURCE-MANIFEST — Manifiesto de fuentes, censo N y reconciliación §2

- **Rol:** Specification (SSOT documental, fuentes, trazabilidad y contratos de especificación)
- **Tarea:** t18 — SPEC-MATRIZ-01 · Equipo `arbx-publicacion-desbloqueo-02`
- **Artefacto par:** `docs/specification/ACCEPTANCE-MATRIX-v1.md` (misma unidad de versión)
- **Base medida (git):** rama `fix/perhop-reserves-01` · HEAD `858b943fd80c8b5e1606d220aa87d63b86f4ba15` · tree `622d32c0bcf9343be79f12513db28a18cbc5dee8` · `origin/main` `274f04fd8d89692fd4b0169130ede74264e31167`
- **Alcance de escritura de esta tarea:** `docs/specification/` únicamente. Este manifiesto **no** modifica backend, frontend, contratos, base de datos, `docs/review/` ni `docs/release/`.
- **Estado del método:** todo lo afirmado aquí se midió en esta sesión con `pwsh`/`git`. Donde un comando falló, se dice y no se rellena.

---

## 0. Regla de autoridad y de no-invención

1. **La autoridad de conteos es el Anexo Técnico** (`Anexo_Tecnico_ArbitrageX_2026-10-04.zip`), no el White Paper. El propio anexo declara su alcance: *"Validación estructural y de integridad del anexo documental, no de implementación ni de rendimiento o rentabilidad"* (`validacion_anexo.json`).
2. **Ninguna fuente ausente se simula.** Un nombre que no aparece se declara **AUSENTE** con la ruta buscada y el comando que prueba la ausencia.
3. **Ausencia ≠ cero.** Un archivo AUSENTE no aporta `0` a ningún conteo: aporta **nada**, y su bloqueo se declara.
4. **Un nombre de archivo no es una fuente.** Lo que acredita es el contenido; por eso este manifiesto distingue *nombre literal* de *contenido incorporado* (§3).

---

## 1. Manifiesto de fuentes — adjuntos externos (fuera del workspace, PRESENTES)

Ruta base: `C:\Users\HFRC\Downloads`. Comando:
`Get-Item -LiteralPath <p> ; Get-FileHash -LiteralPath <p> -Algorithm SHA256`

| ID | Nombre | Ruta | Bytes | Fecha | SHA-256 (completo) | Alcance | Estado |
|----|--------|------|-------|-------|--------------------|---------|--------|
| S-01 | `White_Paper_ArbitrageX_0_a_100_2026-10-04.pdf` | `…\Downloads\` | 2 484 213 | 2026-10-04 09:03 | `90f9bfd44ff456a8f94db70f4ca3ca63bdd989cbebc10e5b5214423b6dd3c05d` | Documento raíz (269 páginas físicas, 256 secciones) | **PRESENTE** |
| S-02 | `White_Paper_ArbitrageX_0_a_100_2026-10-04.docx` | `…\Downloads\` | 327 733 | 2026-10-04 08:38 | `e4487d0c73c61a1feeac0c85f7902d3c4b75a626a34705a56cfc9ac4343fb337` | Misma autoridad, formato alterno | **PRESENTE** (hash derivado por copia; ver nota N-1) |
| S-03 | `Anexo_Tecnico_ArbitrageX_2026-10-04.zip` | `…\Downloads\` | 3 827 435 | 2026-10-04 08:39 | `26435dd685a7862b314d54b460247561822bfa6a3424be1f87103cedfb63db91` | **SSOT documental de conteos y trazabilidad** (46 entradas) | **PRESENTE** |
| S-04 | `ArbitrageX_MEV_Universe_Estrategias.zip` | `…\Downloads\` | 227 011 062 | 2026-10-04 08:37 | `bab66a2833fee27ec6fbec291df548081ae60e9f0d9de3dd753d809d5dc38e52` | Universo de estrategias; 5 libros Excel + matrices 264×31 | **PRESENTE** |
| S-05 | `Informe_Auditoria_ArbitrageX_2026-10-04.pdf` | `…\Downloads\` | 488 538 | 2026-10-04 06:53 | `a42e3600fd74117a57422179a298cb70a99204a52775fdb0f4287f7e416ba312` | Catálogo de hallazgos F01–F40 | **PRESENTE** |
| S-06 | `PROMPT MAESTRO ARBITRAGEX DE IMPLEMENTACIÓN Y ACEPTACIÓN EN LOOP.pdf` | `…\Downloads\` | 103 254 | 2026-10-04 10:35 | `7583d9a80a0ad438ff1bcc65385d5037437442e0d72f2afd525a0c643b0b34c2` | **Mandato vigente de implementación y aceptación (§2, §4)** | **PRESENTE** |

**Nota N-1 (método, no excusa).** El primer `Get-FileHash` sobre S-02 falló:
`The process cannot access the file '…White_Paper_ArbitrageX_0_a_100_2026-10-04.docx' because it is being used by another process.`
El hash se obtuvo copiando el archivo a un temporal y hasheando la copia (`Copy-Item` + `Get-FileHash`), que devolvió `e4487d0c…`. El prefijo `e4487d0c` coincide con el declarado en `docs/audits/SPEC-FUENTES-TRAZABILIDAD-2026-10-04.md` §1, lo que hace la reproducción verificable por dos caminos. **El archivo origen sigue bloqueado por otro proceso**: no se editó ni se movió.

### 1.1 Integridad interna del Anexo (S-03)

Comando: `Get-Content SHA256SUMS.txt` + `Get-FileHash` por entrada.

- Entradas declaradas en `SHA256SUMS.txt`: **45**
- Verificadas byte a byte contra el archivo extraído: **OK = 45 · MISMATCH = 0 · MISSING = 0**
- Resultado: `VERIFY RESULT => OK=45 MISMATCH=0 MISSING=0 (of 45)`

El Anexo es internamente íntegro. Esta es la única autoridad de conteos que este manifiesto usa.

---

## 2. Manifiesto de fuentes — workspace (medido)

Base: `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)`.

### 2.1 Identidad del repositorio

| Fuente | Comando | Resultado medido |
|--------|---------|------------------|
| Rama | `git rev-parse --abbrev-ref HEAD` | `fix/perhop-reserves-01` |
| HEAD | `git rev-parse HEAD` | `858b943fd80c8b5e1606d220aa87d63b86f4ba15` |
| Tree | `git rev-parse HEAD^{tree}` | `622d32c0bcf9343be79f12513db28a18cbc5dee8` |
| `origin/main` | `git rev-parse origin/main` | `274f04fd8d89692fd4b0169130ede74264e31167` |
| Modificados (tracked) | `git status --porcelain` filtrado | **50** |
| No rastreados (`??`) | `git status --porcelain` filtrado | **311** |
| Total líneas porcelain | `git status --porcelain` | 361 |

**Hallazgo Q-1 — el checkout NO es quiescente, y la deriva es medible.** El acta de la célula fija la base como *"34 tracked-mods sin commitear y 295 untracked en un checkout compartido no quiescente"*. La medición de esta sesión da **50 / 311**. HEAD, tree y `origin/main` **no** cambiaron. Conclusión: el árbol de trabajo mutó entre la redacción del acta y esta medición, **sin** mover el commit base. Todo conteo de árbol de este manifiesto queda anclado al instante de medición y **no** es estable entre corridas. No se corrige el acta: se registra la deriva.

### 2.2 Archivos fuente con nombre literal exigido por el §2 — **AUSENTES**

Comando (contrato, con traversal podado — ver nota N-2):

```powershell
Get-ChildItem -Recurse -File -Force -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -notmatch '(?i)\\node_modules\\|\\\.git\\|\\target\\|\\\.worktrees\\|\\\.claude\\worktrees\\' -and
                 $_.Name -in @('user_requirements.json','ui_route_inventory.json','audit_catalog_F01_F40.json','formula_registry.json','formula_bindings_index.json') }
```

**Resultado medido: `ROWS = 0`.**

| ID | Nombre literal buscado | Estado | Ruta buscada (raíz) | Cobertura de la búsqueda |
|----|------------------------|--------|---------------------|--------------------------|
| S-10 | `user_requirements.json` | **AUSENTE** | `arbitragex-v2-main (17)\**` | árbol principal + `.claude\worktrees\*` (22 worktrees) |
| S-11 | `ui_route_inventory.json` | **AUSENTE** | ídem | ídem |
| S-12 | `audit_catalog_F01_F40.json` | **AUSENTE** | ídem | ídem |
| S-13 | `formula_registry.json` | **AUSENTE** | ídem | ídem |
| S-14 | `formula_bindings_index.json` | **AUSENTE** | ídem | ídem |

Cobertura adicional medida: barrido específico sobre `…\.claude\worktrees` y `…\.worktrees` con el mismo conjunto de nombres →
**`worktree hits = 0`**. La ausencia no es un artefacto de exclusión: se buscó también dentro de los worktrees.

**Nota N-2 (por qué el comando literal del contrato se poda).** La forma literal
`Get-ChildItem -Recurse -File -Include '…'` sin poda **agota el timeout** (medido: abortada a 120 s y a 300 s). La causa es mecánica: el árbol contiene **724 587 archivos** y la poda ocurre después del recorrido, doncrecorre `node_modules/`, `.git/` y `target/` completos. La variante usada mueve el filtro al `Where-Object` para que el traversal no baje a esos directorios. Las exclusiones son **no-fuente** (`node_modules`, `.git`, `target`, worktrees) y por eso no pueden contener una fuente del §2; la corrida sobre worktrees se hizo por separado (§ arriba) para cerrar el hueco. **Ausencia declarada, no asumida.**

### 2.3 Documentos y hojas de cálculo en el workspace

Comando (contrato, podado):

```powershell
Get-ChildItem -Recurse -File -Force -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -notmatch '(?i)\\node_modules\\|\\\.git\\|\\target\\|\\\.worktrees\\|\\\.claude\\worktrees\\' -and
                 $_.Extension -match '(?i)^\.(pdf|docx|xlsx|xlsm)$' }
```

**Resultado medido: `ROWS = 62` (pdf = 32, xlsx = 30).**

De esas 62 filas, **54 son vendorizadas** (`contracts\lib\openzeppelin-contracts*\audits\*.pdf`, `repo-vps-audits\**`, `test-phase1\**`) y **no** son fuentes del proyecto. Las **8 filas no vendorizadas**:

| ID | Ruta (relativa al workspace) | Bytes | SHA-256 [:16] | Alcance | Estado |
|----|------------------------------|-------|---------------|---------|--------|
| S-20 | `~$ArbitrageX_Quant_1000Pools.xlsx` | 165 | — | **Archivo-lock de Excel**, no un libro | PRESENTE (residuo) |
| S-21 | `ArbitrageX_Quant_1000Pools.xlsx` | 940 354 | `73a779d660e6fb33` | Libro de 13 hojas | PRESENTE |
| S-22 | `.omega_extracted\arbitragex-v2_OMEGA_report.pdf` | 440 346 | — | Reporte OMEGA | PRESENTE |
| S-23 | `.omega_extracted\arbitragex-v2_OMEGA_v5.0.xlsx` | 351 177 | `c9d26d4b9e17895e` | Libro de 15 hojas | PRESENTE |
| S-24 | `integration\agent-cartridges-v4\sources\ArbitrageX_264_Cartridge_Math_Architecture(1).xlsx` | 113 198 | `93e0807f5baa9dc0` | WB01 | PRESENTE |
| S-25 | `integration\agent-cartridges-v4\sources\ArbitrageX_Master_264x31_LIVE_First(2).xlsx` | 1 451 281 | `54fcdca909886884` | WB03 | PRESENTE |
| S-26 | `audits\workspace-extreme-audit-2026-09-24\agent_pkg\ARBX_CARTUCHOS_AGENTE\sources\ArbitrageX_264_Cartridge_Math_Architecture(1).xlsx` | 113 198 | `93e0807f5baa9dc0` | WB01 (duplicado idéntico de S-24) | PRESENTE |
| S-27 | `audits\workspace-extreme-audit-2026-09-24\agent_pkg\ARBX_CARTUCHOS_AGENTE\sources\ArbitrageX_Master_264x31_LIVE_First(2).xlsx` | 1 451 281 | `54fcdca909886884` | WB03 (duplicado idéntico de S-25) | PRESENTE |

**Hallazgo Q-2 — el White Paper, el Anexo y los 5 libros NO están en el workspace.** Ninguna de las 62 filas es el White Paper (PDF ni DOCX), el Anexo Técnico ni el ZIP del universo MEV. Viven **fuera** del repo (§1). El workspace contiene **2 de los 5** libros por hash (§2.4).

### 2.4 Los 5 libros del SSOT: presencia por hash y conteo de hojas

Hojas contadas leyendo `xl/worksheets/sheetN.xml` dentro de cada `.xlsx` (OpenXML), no leídas de metadatos declarados.

| ID | Libro | bytes | hojas | SHA-256 [:16] | Estado vs SSOT |
|----|-------|-------|-------|---------------|----------------|
| B1 | `ArbitrageX_264_Cartridge_Math_Architecture.xlsx` | 113 198 | 6 | `93e0807f5baa9dc0` | **PRESENTE** — hash coincide con `libros.json` WB01 |
| B2 | `ArbitrageX_Integrated_2_to_7_Hops_Flash_Atomic_Model.xlsx` | — | (21 declaradas) | — | **AUSENTE** en workspace |
| B3 | `ArbitrageX_Master_264x31_LIVE_First.xlsx` | 1 451 281 | 11 | `54fcdca909886884` | **PRESENTE** — hash coincide con `libros.json` WB03 |
| B4 | `ArbitrageX_SOP_264_Implementacion_Rentabilidad.xlsx` | 853 675 | 21 | `866aaece1e752573` | **AUSENTE** en workspace |
| B5 | `ArbitrageX_SOP_264_Math_Software_GO_NOGO.xlsx` | 523 242 | 16 | `e69ea9d552e66305` | **AUSENTE** en workspace |

**Hallazgo Q-3 — divergencia de hash en el libro ULTRA.** El SSOT declara un libro del universo MEV que corresponde a `ArbitrageX_Route_Strategy_Optimizer_264_ULTRA.xlsx`. Medición:

| Fuente | bytes | hojas | SHA-256 [:16] |
|--------|-------|-------|---------------|
| `…\Downloads\ArbitrageX_Route_Strategy_Optimizer_264_ULTRA.xlsx` | 313 971 | 21 | `362ba8762edea602` |
| `docs/excel_ingestion_manifest.json` → clave `ULTRA` | (declara 21 hojas) | 21 | — (no declara hash) |

El SSOT **sí** trae un hash para su quinto libro por hash; el conteo de hojas coincide (21) y el tamaño es consistente, pero **este manifiesto no puede acreditar igualdad byte a byte** porque el `libros.json` del anexo nombra 5 libros distintos (WB01–WB05, §2.5) y ninguno de ellos se llama `…_ULTRA`. Se declara como **divergencia abierta de identidad**, no como coincidencia. Ver frontera F-4.

### 2.5 Reconciliación exacta del conteo de hojas (§2: «75 hojas»)

`libros.json` (SSOT) declara, por libro: `sheets` = 6, 21, 11, 21, 16.

| Vía | Composición | Total |
|-----|-------------|-------|
| **SSOT** (`libros.json` → `sheets`) | 6 + 21 + 11 + 21 + 16 | **75** |
| **Manifiesto de ingesta del workspace** (`docs/excel_ingestion_manifest.json` → `total_sheets`) | CART_MATH 6 + MASTER_LIVE 11 + ULTRA 21 + MASTER_BA 9 | **47** |

Los dos números **no se contradicen**: son **dos magnitudes distintas que comparten la palabra «hojas»**.

- **75** = hojas físicas de los **5 libros del SSOT** (composición exacta reproducida arriba).
- **47** = hojas de los **4 libros efectivamente ingeridos** en el workspace; el manifiesto del workspace cubre 4 de los 5.

**Hallazgo Q-4 — el manifiesto de ingesta del workspace resuelve 47 de las 75 hojas y no cubre 3 de los 5 libros del SSOT por hash.** El déficit es **28 hojas** (75 − 47) y proviene de los libros AUSENTES B2, B4 y B5, que no tienen entrada de ingesta. **Este déficit es una frontera, no un cero:** no significa que esas 28 hojas no existan — existen en el SSOT y en `…\Downloads` — significa que **no están censadas en el workspace**.

### 2.6 Scripts, migraciones y CI

| Fuente | Comando | Resultado medido | Estado |
|--------|---------|------------------|--------|
| Scripts | `(Get-ChildItem scripts -Recurse -File).Count` | **119** | PRESENTE |
| Automatización | `(Get-ChildItem automation -Recurse -File).Count` | **33** | PRESENTE |
| Migraciones (principal) | `Get-ChildItem -Recurse -Directory -Filter migrations` (podado) | `database\migrations` → **116 archivos (115 `.sql`)**; `backend\tests\integration\migrations` → 1 archivo | PRESENTE |
| Workflows CI | `(Get-ChildItem .github\workflows -File).Count` | **56** | PRESENTE |

Nombres CI con relevancia directa de aceptación (56 en total, muestra citada por su rol): `ci.yml`, `rust.yml`, `typescript.yml`, `e2e.yml`, `unit-tests.yml`, `integration-tests.yml`, `foundry.yml`, `codeql.yml`, `security.yml`, `no-hardcode.yml`, `spec-drift-gate.yml`, `pipeline-integrity.yml`, `opportunities-fidelity-gate.yml`, `sim-fork-evidence.yml`, `sim-staging-callbundle.yml`, `ops-paper-mode.yml`, `ops-live-testnet.yml`, `m5-sepolia-validation.yml`, `auto-deploy-vps.yml`, `deploy-validation.yml`, `ethics-guard.yml`, `wallet-security.yml`, `v3-fee-units.yml`, `v3-fee-data-integrity.yml`.

---

## 3. Reconciliación §2 — cifras históricas contra fuentes PRESENTES

Anexo extraído a `…\AppData\Local\Temp\arbx-spec-t18\anexo` (extracción desde S-03; hash de origen acreditado en §1.1). **Todos los conteos de esta tabla son recuentos propios ejecutados en esta sesión**, no cifras heredadas de prosa.

| # | Cifra histórica §2 | Valor declarado (SSOT) | **Recuento propio** | Comando / artefacto leído | ¿Reproduce? |
|---|--------------------|------------------------|---------------------|---------------------------|-------------|
| 1 | Requisitos vigentes | 70 | **70** | `Import-Csv trazabilidad\requisitos_vigentes.csv` → `.Count` = 70 | **SÍ** |
| 2 | Hallazgos F01–F40 | 40 | **40** | `Get-Content hallazgos\catalogo_F01_F40.json \| ConvertFrom-Json` → `.Count` = 40 | **SÍ** |
| 3 | Rutas actuales | 59 | **59** | `Import-Csv pantallas\rutas_actuales.csv` → `.Count` = 59 | **SÍ** |
| 4 | Libros por hash | 5 | **5** | `excel\libros.json` → 5 objetos WB01–WB05, cada uno con `sha256` | **SÍ** |
| 5 | Hojas | 75 | **75** | §2.5: 6+21+11+21+16 (`excel\libros.json`) y `excel\hojas.json` agrupado por `workbook_id` → 6/21/11/21/16 | **SÍ** |
| 6 | Ubicaciones de fórmula | 60 881 | **60 881** | `Get-Content excel\ocurrencias_formula_y_fixtures.csv` → 60 882 líneas − 1 header = 60 881 | **SÍ** |
| 7 | Contratos de estrategia | 264 | **264** | `Get-Content especificaciones\contratos_264_estrategias.json \| ConvertFrom-Json` → `.Count` = 264 | **SÍ** |
| 8 | Relaciones estrategia–operador | 8 184 | **8 184** | `Get-Content especificaciones\matriz_264x31.csv` → 8 185 líneas − 1 header = 8 184 | **SÍ** |

**Las 8 cifras del §2 reproducen al 100 % por recuento propio e independiente.**

### 3.1 Cifras que el SSOT separa y que NO deben fusionarse

Estas se midieron también, y su existencia es lo que impide leer las 8 cifras de arriba como progreso:

| Magnitud | Declarado | **Recuento propio** | Comando | Significado real |
|----------|-----------|---------------------|---------|------------------|
| `proposed_binding_contracts` | 253 | **253** | `excel\bindings_propuestos.jsonl` → 253 líneas | Bindings **propuestos** (≠ 264 contratos de estrategia) |
| `unique_literal_formulas` | 34 696 | **34 696** | `excel\literales_formula.jsonl` → 34 696 líneas | Fórmulas **únicas** (60 881 son *ocurrencias*) |
| `formula_families` | 1 509 | **1 509** | `excel\familias_formula.jsonl` → 1 509 líneas | Familias de patrón (≠ función semántica aprobada) |
| `documentary_sheets` | 54 | **54** | `especificaciones\hojas_documentales.json` → `.Count` = 54 | Hojas **documentales** (≠ 75 worksheets) |
| `executed_arithmetic_records` | 6 | **6** | `pruebas\resultados_aritmeticos_ejecutados.json` → 6 registros | Registros aritméticos **realmente ejecutados** |
| `reported_arithmetic_checks` | 1 045 | **1 045** | `pruebas\reporte_aritmetico_ejecutado.json` → `.checks` = 1 045 | Checks **reportados**, no re-ejecutados |
| Requisitos antecedentes | 45 | **45** | `trazabilidad\requisitos_antecedentes.csv` → 45 filas | Antecedentes históricos, ≠ 70 vigentes |

Cita literal del propio SSOT sobre el método de esos 6: *"Independent integer/Decimal arithmetic and saved-cache comparison; **not native Excel recalculation or backend execution**"*.

### 3.2 Declaración obligatoria: qué NO acreditan estas cifras

> **Las cifras del §2 y del §3.1 no acreditan implementación, ni cobertura funcional, ni pesos de operador, ni ganancias.**
>
> Concretamente:
> - **No acreditan implementación.** Los 70 requisitos vigentes están **todos** en estado `REQUISITO_OBJETIVO_NO_IMPLEMENTADO_POR_ESTA_TAREA` (70/70) y **todos** en trazabilidad `MAPEO_DOCUMENTAL_EXPLICITO` (70/70). Cita del propio SSOT: *"MAPEO_DOCUMENTAL_EXPLICITO — hay sección que trata el requisito; **no** es cumplimiento"*. Los 40 hallazgos están los 40 en `PROPUESTO_NO_IMPLEMENTADO_EN_ESTA_TAREA`.
> - **No acreditan cobertura.** Contar enlaces documentales no multiplica pruebas ejecutadas: hay **1 045** checks *reportados* contra **6** registros *ejecutados*, y **56 843** valores son **caché guardada** (con 4 038 fórmulas sin caché: ausencia real, no cero).
> - **No acreditan pesos ni calibración.** 1 509 familias de fórmula son **familias de patrón**, no funciones semánticas aprobadas; el hallazgo **F40** del propio catálogo dice literalmente *"Operador computado no implica señal válida ni calibrada"*.
> - **No acreditan ganancias.** El alcance declarado del Anexo excluye expresamente *"rendimiento o rentabilidad"*. Ninguna cifra de este manifiesto es un resultado económico.
>
> La distancia entre **documentado** y **ejecutado** está cuantificada en el propio SSOT; este manifiesto la reproduce, no la cierra.

---

## 4. Frontera: fuentes AUSENTES y qué parte del censo bloquean

Una fuente AUSENTE no aporta `0`: **bloquea** la parte del censo que dependía de ella. Tabla de bloqueo:

| ID | Fuente AUSENTE | Dónde se buscó | Qué parte del censo bloquea | Estado |
|----|----------------|----------------|------------------------------|--------|
| F-1 | `user_requirements.json` | workspace completo + 22 worktrees | Censo nominal del §2: el nombre literal **no es resoluble por ruta**. Contenido equivalente localizado (ver F-1′ abajo) | **AUSENTE / contenido ubicado** |
| F-2 | `ui_route_inventory.json` | ídem | ídem | **AUSENTE / contenido ubicado** |
| F-3 | `audit_catalog_F01_F40.json` | ídem | ídem | **AUSENTE / contenido ubicado** |
| F-4 | `formula_registry.json` | ídem | ídem | **AUSENTE / contenido ubicado (parcial)** |
| F-5 | `formula_bindings_index.json` | ídem | ídem | **AUSENTE / contenido ubicado** |
| F-6 | White Paper PDF / DOCX dentro del workspace | 62 filas de §2.3 | El repo **no** contiene la fuente raíz del documento: su validación de páginas/secciones solo es acreditable contra `…\Downloads` | **AUSENTE del workspace** |
| F-7 | Anexo Técnico dentro del workspace | 62 filas de §2.3 | Toda la autoridad de conteos vive **fuera** del repo. Un lector que solo tenga el repo **no puede** reproducir el §3 | **AUSENTE del workspace** |
| F-8 | Libros B2, B4, B5 en el workspace | §2.4 | **28 hojas** (75 − 47) sin censo en el workspace | **AUSENTE** |
| F-9 | Identidad byte a byte del libro `…_ULTRA.xlsx` vs el 5.º libro del SSOT | §2.4 / Q-3 | Impide cerrar la igualdad por hash del 5.º libro | **DIVERGENCIA ABIERTA** |

### 4.1 F-1′ — ubicación del contenido de los 5 nombres literales (sin inventar equivalencias)

El SSOT trae su propio manifiesto de curaduría: `manifiestos\fuentes_curadas.json`, **32 entradas**, cada una con `source_name`, `sha256` y `role` (*"Evidencia curada o capítulo de referencia, incorporado total o parcialmente en las tablas del anexo"*).

Consulta literal sobre ese archivo:

| Nombre literal del §2 | ¿Declarado en `fuentes_curadas.json`? | SHA-256 declarado del nombre literal |
|-----------------------|----------------------------------------|---------------------------------------|
| `user_requirements.json` | **SÍ** | `a603009324928cb08ffe6e2ad77dd3e117988feabe52908128867a2f4d6b40fa` |
| `ui_route_inventory.json` | **SÍ** | `7a76d4c2b0b960ae9413e7ba2b1a02da548001b29a3a780d549e5a24c204a2e4` |
| `audit_catalog_F01_F40.json` | **SÍ** | `0f579a723e91dc6a9566e348d8a8c615857b44afde6a628de861a25bd123f040` |
| `formula_registry.json` | **NO** | — (el SSOT nombra `formula_registry_compact.jsonl`, `8f8f30c1c3735d142a0e4f5b5d5fa50322c0a2e03505599e1ba2533fbf11424f`) |
| `formula_bindings_index.json` | **SÍ** | `658d98495ae6aaa47354947da0fc291d509e71e68ad6d62086a0b10b9dcc0b26` |

**Correspondencia de contenido y su grado de acreditación** (§4.2). Regla: una correspondencia se declara **ACREDITADA** solo si el conteo derivado del artefacto del anexo coincide con el conteo propio del nombre literal y con las palabras clave del §2.

### 4.2 Correspondencias nombre-literal → artefacto del anexo

| Nombre literal §2 | Artefacto del anexo | Conteo propio del anexo | Conteo acreditado para el nombre literal | Veredicto |
|-------------------|---------------------|--------------------------|-------------------------------------------|-----------|
| `user_requirements.json` | `trazabilidad\requisitos_vigentes.csv` | 70 | 70 (§3 #1) | **ACREDITADA** |
| `ui_route_inventory.json` | `pantallas\rutas_actuales.csv` | 59 | 59 (§3 #3) | **ACREDITADA** |
| `audit_catalog_F01_F40.json` | `hallazgos\catalogo_F01_F40.json` | 40 | 40 (§3 #2) | **ACREDITADA** |
| `formula_bindings_index.json` | `excel\bindings_propuestos.jsonl` + `excel\contratos_hojas.json` | 253 BND | 253 (§3.1) | **ACREDITADA** (parcial: binding *propuesto*, no contrato de campo implementado) |
| `formula_registry.json` | `excel\literales_formula.jsonl` (vía `formula_registry_compact.jsonl`) | 34 696 | 34 696 únicos / 60 881 ocurrencias (§3 #6, §3.1) | **PARCIAL** — el nombre literal **no** está en la curaduría; la equivalencia con `…_compact.jsonl` es inferencia de nombre, **no** acreditación por hash. Se declara **NO ACREDITADA como equivalencia de identidad** |

**Consecuencia para el censo.** Los 5 nombres literales del §2 son **AUSENTES como archivos**, pero **4 de 5 tienen contenido equivalente acreditado** por conteo reproducible. El 5.º (`formula_registry.json`) tiene contenido presente (34 696 literales) y **equivalencia de identidad no acreditada**. Ningún conteo del §3 depende de que los nombres literales existan: dependen de los artefactos del anexo, que sí existen y verifican (45/45 en §1.1).

**Prohibición respetada.** No se creó ningún archivo con esos 5 nombres. No se rellenó contenido ausente. No se excluyó ningún requisito por ser difícil, fallar o no existir todavía: los 70 requisitos entran completos en la matriz par.

---

## 5. Comandos de prueba (reproducibilidad)

Bloque A — ausencia de los 5 nombres (contrato, podado):

```powershell
Get-ChildItem -Recurse -File -Force -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -notmatch '(?i)\\node_modules\\|\\\.git\\|\\target\\|\\\.worktrees\\|\\\.claude\\worktrees\\' -and
                 $_.Name -in @('user_requirements.json','ui_route_inventory.json','audit_catalog_F01_F40.json','formula_registry.json','formula_bindings_index.json') }
# → ROWS = 0
```

Bloque B — documentos y hojas de cálculo (contrato, podado):

```powershell
Get-ChildItem -Recurse -File -Force -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -notmatch '(?i)\\node_modules\\|\\\.git\\|\\target\\|\\\.worktrees\\|\\\.claude\\worktrees\\' -and
                 $_.Extension -match '(?i)^\.(pdf|docx|xlsx|xlsm)$' }
# → ROWS = 62 (pdf 32, xlsx 30)
```

Bloque C — integridad del Anexo:

```powershell
# extraer S-03 a un temporal y luego:
Get-Content SHA256SUMS.txt | ForEach-Object { … Get-FileHash … }
# → OK=45 MISMATCH=0 MISSING=0
```

Bloque D — los 8 conteos del §2 (uno por comando):

```powershell
$a = '<temp>\anexo'
(Import-Csv "$a\trazabilidad\requisitos_vigentes.csv").Count                       # 70
(Get-Content "$a\hallazgos\catalogo_F01_F40.json" -Raw | ConvertFrom-Json).Count   # 40
(Import-Csv "$a\pantallas\rutas_actuales.csv").Count                               # 59
(Get-Content "$a\excel\libros.json" -Raw | ConvertFrom-Json).Count                 # 5
(Get-Content "$a\excel\hojas.json" -Raw | ConvertFrom-Json).Count                  # 75
(Get-Content "$a\excel\ocurrencias_formula_y_fixtures.csv").Count - 1              # 60881
(Get-Content "$a\especificaciones\contratos_264_estrategias.json" -Raw | ConvertFrom-Json).Count  # 264
(Get-Content "$a\especificaciones\matriz_264x31.csv").Count - 1                    # 8184
```

---

## 6. Fronteras que este manifiesto NO cruza (declaradas, no tapadas)

- **No** ejecuté `cargo test`, `vitest`, `pytest`, MCP ni ningún backend. Ninguna cifra de este documento es un PASS de runtime.
- **No** re-ejecuté los 1 045 checks aritméticos; solo leí que **6** tienen registro de ejecución.
- **No** validé que los hashes de los 3 libros AUSENTES (B2, B4, B5) correspondan a bytes que yo haya tenido: los tomé del SSOT, que es la autoridad documental, **no** una re-derivación de bytes.
- **No** abrí el White Paper para validar sus 269 páginas / 256 secciones: esas cifras se citan como **declaradas** por el SSOT (`manifiestos\conteos.json`), no como recontadas por mí.
- **No** resolví F-9 (divergencia de identidad del libro ULTRA).
- **No** edité las 3 stores de skills, ni backend, ni frontend, ni contratos, ni base de datos.

## 7. Insumos consumidos de gobernanza previa (no re-derivados)

Estos artefactos ya existían y se usaron como contexto, no como autoridad de conteo:

| Artefacto | Qué aporta | Relación con este manifiesto |
|-----------|-----------|------------------------------|
| `docs/audits/SPEC-FUENTES-TRAZABILIDAD-2026-10-04.md` | Reconciliación previa del §2 y hallazgo D-4 (nombres literales vs contenido) | **Confirmado y extendido**: aquí se reproduce por recuento propio y se añade §4.2 con el grado de acreditación |
| `docs/specification/ssot-contract-and-discrepancies-v1.md` | Inventario medido de las 3 stores de skills, contrato de binding propuesto | Fuente de vocabulario (`VERIFICADO_*`, `PROPUESTO_NO_IMPLEMENTADO`, no-pérdida de fuentes) |
| `docs/specification/FORMULA_VENUE_BINDING_DISCREPANCIES.md` | Contradicción CPMM en venues no-CPMM (B-01 vs B-04) | Insumo para el eje «fórmula propuesta vs función ejecutada» de la matriz par |

---

*Fin del manifiesto. La matriz de aceptación con `scope_version` y N congelados está en `docs/specification/ACCEPTANCE-MATRIX-v1.md`.*
