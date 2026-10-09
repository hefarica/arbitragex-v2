# N115-VERIFY — Comando que reconstruye el denominador de aceptación (N = 115)

- **Rol:** Specification · tarea **t140 / W00b-SSOT-VERSIONADO-01**
- **Qué es esto:** el comando ejecutable que reconstruye `N = 115` desde un clon del repo, **sin depender de ningún adjunto externo**.
- **Qué NO es esto:** acreditación. Este verificador **solo cuenta** el denominador. **No mueve un solo criterio.** Ver §4.

---

## 1. El comando

Desde la raíz del repo, con la matriz versionada presente:

```powershell
$m = 'docs/specification/ACCEPTANCE-MATRIX-v1.md'
if (-not (Test-Path -LiteralPath $m)) { Write-Host "FALTA $m"; exit 1 }
$t = Get-Content -LiteralPath $m -Raw
$u = ([regex]::Matches($t,'AC-USR-\d{3}') | ForEach-Object Value | Sort-Object -Unique).Count
$f = ([regex]::Matches($t,'AC-FND-\d{2}')  | ForEach-Object Value | Sort-Object -Unique).Count
$r = ([regex]::Matches($t,'AC-FRT-\d')     | ForEach-Object Value | Sort-Object -Unique).Count
Write-Host "AC-USR=$u AC-FND=$f AC-FRT=$r N=$($u+$f+$r)"
if (($u + $f + $r) -ne 115) { Write-Host "N != 115 -> DENOMINADOR NO REPRODUCIDO"; exit 1 }
```

**Salida esperada y medida en esta sesión (proceso fresco, `powershell -NoProfile`):**

```
AC-USR=70 AC-FND=40 AC-FRT=5 N=115
```

**Por qué cuenta así y no de otra forma:**

| Detalle | Razón |
|---|---|
| `Sort-Object -Unique` | Cada `criterion_id` aparece **más de una vez** en el documento (una en su sección de matriz, otra en índices y en el crosswalk de §1.3 de `GATE-UNIVERSE-v1.md`). Sin deduplicar, el conteo infla. |
| Se cuentan **sólo** los 3 espacios de ID del denominador | `AC-FRM-*` es el **sub-censo de fórmulas (M = 3)**, explícitamente **no aditivo** a `N` por la regla R5 del propio artefacto. Incluirlo daría 118. |
| El script **falla ruidosamente** si la suma ≠ 115 | Un verificador que imprime un número equivocado sin señalizar es peor que no tenerlo. |

### 1.1 Variante POSIX — NO USAR

Una variante con `grep -oE … | Sort-Object -Unique` **miscuenta** en este entorno (devolvió `AC-USR=1 AC-FND=4 AC-FRT=5`, N=10) por efecto de pipeline sobre la salida, no por el patrón. **La variante PowerShell es la canónica.** Se deja escrito para que nadie ruede sobre esa piedra.

---

## 2. ★ TRAMPA 1 — Las 40 filas `AC-FND` NO TIENEN COLUMNA DE ESTADO

> **CONVENCIÓN DE ÍNDICE — declarada, no asumida.** Todos los `celda[n]` de este documento son
> **0-based**: `celda[0]` es la **primera** celda tras el delimitador, es decir el `criterion_id`
> (`AC-USR-001`, `AC-FND-01`, …). Por tanto **`celda[15]` y «celda[16] 1-based» son la MISMA
> posición**. Cuando un índice viaja fuera de este archivo, va **con su base declarada** — o,
> preferible, **con la cabecera citada en lugar del número**. Un índice sin base es un dato que se
> corrompe al propagarse: es el mismo modo de fallo que citar una ruta o un umbral desde prosa.

**Medido, no inferido.** Las tres tablas de criterio del documento **no tienen la misma forma**:

| Familia | Filas | Celdas por fila | ¿Tiene columna de estado? | Índice de esa columna (**0-based**) |
|---|---|---|---|---|
| `AC-USR-*` (70) | §2.1–§2.13 | **16** | **SÍ** | **`celda[15]`** (la última) |
| `AC-FND-*` (40) | §3 | **8** | **NO** | — no existe |
| `AC-FRT-*` (5) | §4 | **8** | **SÍ** | **`celda[7]`** (la última) |

**Cabeceras reales, citadas del artefacto:**

- `AC-USR` — `ACCEPTANCE-MATRIX-v1.md:107`: `| ID | Req | Fuente exacta | Capacidad / flujo | Actor | Modo | Eje | Precondiciones | Acción | Resultado observable | Invariantes | Ambiente | Umbral (origen) | Dueño | Deps | Estado |` → **16 columnas, `Estado` al final**.
- `AC-FND` — `ACCEPTANCE-MATRIX-v1.md:250`: `| ID | F | Título (SSOT) | Prioridad propuesta | Dueño | Etapa | Eje | Evidencia de cierre requerida |` → **8 columnas. La última es `Evidencia de cierre requerida`, NO un estado.**
- `AC-FRT` — `ACCEPTANCE-MATRIX-v1.md:301`: `| ID | F | Fuente AUSENTE | Ruta buscada | Comando que prueba la ausencia | Eje | Qué parte del censo bloquea | Estado inicial |` → **8 columnas, `Estado inicial` al final**.

**Consecuencia obligatoria:** el `NO_VERIFICADO` de las 40 `AC-FND` **es un mapeo por rúbrica declarado, no un dato de la matriz**. El documento lo dice en su propio encabezado de §3: *"Los 40 comparten `estado_inicial = NO_ACREDITADO` y `status` SSOT `PROPUESTO_NO_IMPLEMENTADO_EN_ESTA_TAREA`; la matriz no repite esa columna para no fatigar la lectura."*

**Quien asuma que las 115 filas se leen igual, cuenta mal.** Un lector que aplique el índice `celda[15]` a las filas `AC-FND` **no lee un estado: no hay celda 15**, y si toma la última celda disponible (la 7) leerá **la evidencia de cierre** como si fuera un estado.

---

## 3. ★ TRAMPA 2 — EL MÉTODO DE CONTEO CAMBIA EL NÚMERO

**Contar tokens de estado sobre la FILA ENTERA** (cualquier celda, no la columna de estado) produce:

```
FILA ENTERA  =>  PARCIAL=2  ACEPTADO=1  RECHAZADO=1   (4 filas afectadas)
```

Reproducido exactamente en esta sesión, con las filas responsables:

| Token | Fila | Celda | ¿Qué es en realidad? |
|---|---|---|---|
| `ACEPTADO` | `AC-USR-005` | celda[10] | **PROSA** — el texto *"`aceptados/N`"* describe la fórmula del roadmap |
| `RECHAZADO` | `AC-USR-022` | celda[10] | **PROSA** — *"ciclos rechazados"* |
| `PARCIAL` | `AC-USR-007` | celda[15] | **ESTADO REAL** — precedido de `**PARCIAL**:` |
| `PARCIAL` | `AC-FRT-5` | celda[7] | **PROSA** — *"acreditado parcial"* dentro del texto |

Si esos tokens se leen como estados, el resultado es `EN_CURSO=2, PASS=1, FAIL=1` — **y los tres son falsos**. En particular: **`PASS=1` es un artefacto de prosa**, y en un tablero de aceptación un `PASS` falso es exactamente el fraude que esta campaña viene evitando.

### 3.1 La columna que SÍ es fuente

**Solo la columna de estado de cada tabla** reproduce el reparto real:

| Familia | Columna fuente | Reparto |
|---|---|---|
| `AC-USR` | **celda[15]** | `NO_ACREDITADO` = **69** · `**PARCIAL**: …` = **1** |
| `AC-FND` | **no existe columna** | **40** por mapeo de rúbrica → `NO_ACREDITADO` |
| `AC-FRT` | **celda[7]** | `BLOQUEADO_NO_CENSABLE` = **4** · `BLOQUEADO_POR_FUENTE` = **1** |

**Cierre aritmético — el reparto declarado por el verificador independiente (t139) se reproduce EXACTAMENTE:**

```
PASS (ACEPTADO)            = 0
EN_CURSO                   = 0
NO_ACREDITADO              = 69 (AC-USR) + 40 (AC-FND por rúbrica) = 109
BLOQUEADO                  = 4 (BLOQUEADO_NO_CENSABLE) + 1 (BLOQUEADO_POR_FUENTE) = 5
PARCIAL                    = 1 (AC-USR-007)
                             ---------------------------------------------
                             0 + 0 + 109 + 5 + 1 = 115  ✓
```

**Nota de honestidad sobre el índice:** la celda de estado **no está en el mismo índice en las tres tablas** (**15 / inexistente / 7**, 0-based). El artefacto original de t139 cita `celda[16]` como columna de estado; medido sobre el archivo versionado, `AC-USR` tiene **16 celdas en total por fila**, es decir el estado es **`celda[15]` 0-based — la MISMA posición que `celda[16]` 1-based**. **No es una discrepancia de medición: es una convención no declarada**, y el error de origen fue propagar el índice sin su base. Por eso este documento declara la base al frente (§ arriba) y da los índices por tabla, para que un tercero no tenga que adivinarla.

---

## 4. Lo que este denominador NO es (y qué NO se movió al versionarlo)

- **`N = 115` NO es "el 100 % del proyecto".** Es el **censo documental enumerado**: 70 requisitos + 40 hallazgos + 5 fronteras. La tasa `aceptados/N` mide **cobertura de aceptación documental**, no madurez, no implementación, no rentabilidad.
- **El sub-censo de fórmulas NO entra en N.** `AC-FRM-1/2/3` (M = 3) es categórico y no aditivo (regla R5). Sumarlo daría 118 y sería un error de método.
- **Versionar el denominador NO es acreditarlo.** El censo sigue:

  > **P/N = 0/115 = 0,0 %** · **PASS = 0** · ningún criterio nuevo marcado `ACEPTADO`.

- **Un `PASS` falso por prosa ya estaba identificado y sigue bloqueado** (§3): `ACEPTADO` aparece una vez en el documento como **texto**, nunca como estado.
- **Cero umbrales bajados, cero criterios reclasificados, cero `scope_version` cambiado.** `scope_version` sigue en **`arbx-scope-1.0.0`**.

---

## 5. Identidad del objeto versionado

La identidad se fija por **bytes**, no por prosa. Verificación **con `git hash-object`, nunca con `Out-File`** (re-codifica y mete CRLF — defecto de método ya cazado dos veces en esta campaña).

| Artefacto | Bytes | SHA-256 | git blob |
|---|---|---|---|
| `ACCEPTANCE-MATRIX-v1.md` | **60 080** | `7E53F22AAB3D071C4E621E9F8C1771A0F30F04FC1511DEC11A1BB57C1C3F32E0` | `73d2ff331e8855243d55a0455f6c8cb8613145d4` |
| `SOURCE-MANIFEST.md` | **28 422** | `E104DBA387B62CC88864AF3CF5C848249A14ACF5DB582467E29A94CAE993575A` | `d3c215cfe46abc1c3f4513526cd3de715cf2cd36` |

**Los tres valores coinciden con lo declarado.** Los archivos se commitean **byte-idénticos**: no se reformateó nada para que entrara.
