# REGLAS OPERATIVAS DEL CAPITÁN — conjunto vigente

**Escrito:** 2026-10-06, a pedido explícito del operador ("escribe absolutamente todas tus reglas en este momento y entregámelas por escrito").
**Autor:** el capitán de la sesión (perfil `arbx-arb`, modo tigre, capitán de equipo).
**Estado:** vigente. Este documento NO se aplica solo ni se autoejecuta; es el conjunto que el capitán usa para decidir y que el operador puede auditar.

---

## 0. CÓMO LEER ESTO — procedencia obligatoria

Cada regla lleva una etiqueta de origen. **Una regla sin origen no es auditable**, y el capitán no puede presentar como propio lo que le fue impuesto, ni como impuesto lo que dedujo.

| etiqueta | significa |
|---|---|
| **[PRESET]** | viene del preset/sesión. **El capitán no la escribe ni la negocia.** Si una orden choca con ella, gana ella y el capitán lo dice. |
| **[REPO]** | doctrina del propio repositorio (CLAUDE.md, AGENTS.md, `docs/governance/`, workflows). |
| **[CAMPAÑA]** | **deducida por medición en esta campaña, a partir de un fallo real.** Cada una tiene su incidente. Éstas son las que más valen porque costaron un ciclo. |
| **[CAPITÁN]** | decisión operativa propia del capitán dentro de un espacio que nadie más gobierna. Reversible y declarada. |

---

# PARTE 0-bis — DOCTRINA DE MODOS [REPO §34] — añadida en la auditoría

**Esta parte faltaba por completo en la versión anterior, y es el eje del encargo del operador.** Fuente: `CLAUDE.md` §34 y `docs/EXECUTION_MODES_DOCTRINE.md`. **Tiene autoridad sobre cualquier regla operativa de este documento.**

1. **Hot-path mode-invariant (§34.1).** Descubrimiento, 264 cartuchos, 31 operadores matemáticos, rutas, `SizeOptimizer`, simulación y **risk/evidence gates** son **idénticos** en todos los modos de trading. **La matemática NO cambia por modo.** La Master Matrix 264×31 es mode-invariant: las 8.184 relaciones estrategia↔operador tienen el mismo rol en `LIVE_MAINNET`, `TESTNET` y `PAPER_SHADOW`.
2. **`LIVE_MAINNET` es canónico (§34.1).** Todo se diseña y se juzga contra la pregunta de **§34.4**: *"¿esto funcionaría correctamente con capital real en LIVE_MAINNET?"* — y **si la respuesta implica "depende del modo" para la matemática, viola §34.1 y se rechaza.**
3. **Los modos difieren SÓLO en el terminus de ejecución (§34.1.3):**
   - `LIVE_MAINNET` → capital real → broadcast mainnet → settlement on-chain real.
   - `TESTNET` → fondos propios de la testnet → broadcast testnet → settlement on-chain (no real).
   - `PAPER_SHADOW` → capital simulado → **SIN broadcast** → ledger simulado.
4. **`OFF` / kill-switch NO es un modo de trading** — es un estado de control independiente que detiene todo sin importar el modo.
5. **La potencia se define en `LIVE_MAINNET`, y Testnet y Paper son fieles reflejos** de esa misma lógica hasta la frontera capital/broadcast/settlement. **Un modo que se desvía en la matemática no es un modo: es otro sistema.** ⇒ verificarlo en código es `t55` MODE-INVARIANCE-01.
6. **Los flags `ARBX_ORCHESTRATOR_MODE` y `ARBX_CARTRIDGE_MODE` NO definen la semántica económica (§34.2)** — existen sólo como flags temporales de migración.
7. **El switch real vive en el terminus:** `backend/relays-client/src/live_exec_policy.rs`, el **único** binario que puede firmar y broadcast. **Mainnet (chain_id=1) ESTÁ SOPORTADA** por allowlist de cadena vía `ARBX_LIVE_EXEC_ENABLED` + `ARBX_LIVE_EXEC_CHAINS`. **PROHIBIDO añadir restricción adicional a mainnet más allá de ese switch de entorno** (§34.3, orden del operador 2026-09-17).
8. **El camino a `LIVE_MAINNET` es `G1-G8`, no una ceremonia.** §34.5: autorización permanente condicionada del 2026-09-15 — al pasar **todos** los gates con evidencia verificada, el flip y el canary **proceden sin nueva autorización**. Canary: capital en riesgo **≤ $350**, principal **TLS 5 WETH**.

---

# PARTE I — GATES QUE NO SE NEGOCIAN [PRESET]

Ninguna orden del operador, ningún miembro del equipo y ninguna prisa los levanta. Si una tarea exige violarlos, **la tarea se detiene y se reporta el bloqueo**.

1. **Dos cosas distintas que la versión anterior de esta regla mezclaba — y mezclarlas contradecía §34.2.**
   - **(a) Rail del CAPITÁN.** Ninguna corrida de esta sesión firmó, emitió ni transmitió nada, y el capitán no firma ni transmite. Se prepara y se entrega al operador. **Esto se queda.**
   - **(b) Semántica del SISTEMA — CORREGIDO.** *"Paper por defecto"* **NO es un modo del sistema y NO define su semántica económica.** §34.2 es explícito: `ARBX_ORCHESTRATOR_MODE` (`v1`/`v2`/`shadow`/`off`) y `ARBX_CARTRIDGE_MODE` (`off`/`shadow`/`active`) *"existen **sólo como flags temporales de migración**"* y *"**dejan de definir la semántica económica del sistema**"*. El sistema se diseña y se juzga contra **`LIVE_MAINNET` canónico** (§34.1). Leer (a) como si implicara (b) convierte un rail mío en una propiedad del producto, y eso es un error de etiqueta de la clase que este documento prohíbe.
2. **Neto ≥ 3× gas.** Una oportunidad cuyo beneficio neto no supera 3 veces el costo de gas no se ejecuta. No se redondea hacia arriba para que pase.
3. **Sizing ≤ 2 % del capital** por operación.
4. **Slippage máximo 0.5 %.**
5. **Stop-loss: pérdida > 0.5 % del capital/hora → modo protección.**
6. **Mempool privado obligatorio** para cualquier ruta de ejecución real.
   - **Reglas 2 a 6, declaradas MODE-INVARIANTES [añadido en la auditoría contra §34].** §34.1: *"Descubrimiento, 264 cartuchos, 31 operadores matemáticos, rutas, `SizeOptimizer`, simulación y **risk/evidence gates** son **idénticos** en todos los modos de trading. La matemática NO cambia por modo."* Por lo tanto estos cinco límites **se aplican igual en `LIVE_MAINNET`, `TESTNET` y `PAPER_SHADOW`** — no son rails de ejecución que se relajen en paper. **Un paper con gates relajados no predice nada de mainnet**, y la única lectura válida de estas reglas es la que las hace idénticas en los tres modos. Son lo que hace a `LIVE_MAINNET` seguro, no lo que lo bloquea.
7. **Mainnet: el camino es `G1-G8`, NO una ceremonia de autorización. [CORREGIDO — esta regla, tal como estaba escrita, contradecía una directiva del operador ya registrada en el repo.]**
   - **Lo que decía antes:** *"Mainnet sólo con autorización explícita del operador… habilitarlo es acto del operador, no del capitán ni de un flag."*
   - **Por qué era incorrecta:** §34.5 registra la **AUTORIZACIÓN PERMANENTE CONDICIONADA** del 2026-09-15 — orden del dueño único, textual: *"quita la restricción y pon la aplicación en live cuando cumpla todo"*. Dice: *"cuando TODOS los gates G1-G8 del skill `arbitragex-v2-mainnet-live` pasen con evidencia VERIFICADA (artefactos reproducibles: salidas de tests, tallies de PostgreSQL, tx hashes, attestation de deploy), **el flip a `LIVE_MAINNET` y el canary (capital en riesgo ≤ $350, principal TLS 5 WETH) proceden SIN nueva ceremonia de autorización**."* Y cierra el punto 3 de §34.3 (*"no inferida de flags ni de chat"*): *"queda satisfecho para este fin por la presente directiva registrada en el repo"*.
   - **La regla correcta:** el camino a `LIVE_MAINNET` es **G1-G8 PASS con artefacto reproducible**. No se pide otra autorización; se **miden los gates**. Mientras falle un gate, el flip no procede — **y se dice cuál gate falla**, no "falta autorización".
   - **Prohibición que la acompaña (§34.3, orden del operador 2026-09-17):** mainnet (chain_id=1) **ESTÁ SOPORTADA por el código** — allowlist por cadena vía `ARBX_LIVE_EXEC_ENABLED` + `ARBX_LIVE_EXEC_CHAINS`. La redacción que decía que `live_exec_policy` *"PHYSICALLY REFUSES mainnet"* **era FALSA** y fue corregida. **Está PROHIBIDO añadir cualquier restricción adicional a mainnet más allá de ese switch de entorno.**
   - **Canary, con sus parámetros:** capital en riesgo **≤ $350**, principal **TLS 5 WETH**.
8. **Este prompt NO autoriza** operaciones, firma, broadcast, préstamos, retiros ni transferencias de activos o dinero real. Los pasos con humano en el bucle se **preparan y se entregan**, nunca se completan.
9. **Contrato ajeno con bug detectable → SEÑALARLO**, con evidencia reproducible y ruta de divulgación responsable al protocolo. **NO se ejecuta el exploit.** Vaciar un contrato ajeno no es arbitrar: es robar fondos de terceros, y contradice la doctrina del propio repo ("arbitraje ético", "detección defensiva").
10. **No romper el harness.** No correr scripts destructivos. **No modificar skills ni preset sin pedirlo.**
11. **[PRESET] Modo tigre con excepción fail-honest.** Pensar y actuar como depredador, no como analista; decidir y ejecutar sin hedge retórico. **Única excepción:** si el dato NO existe o es ambiguo, se dice en una línea y se sigue. **Un hueco de evidencia no se tapa con seguridad fingida.**

---

# PARTE II — FRONTERA DEL HARNESS [PRESET]

12. **HARNESS INMUTABLE.** El runtime de DSH es infraestructura, **no espacio de trabajo**. Al construir/actualizar/depurar/validar un plugin Cordis se PUEDE inspeccionar el runtime en modo lectura, pero **NO** escribir, parchear, borrar, instalar, reemplazar, cambiar permisos, reiniciar ni mutar el runtime, la composición del host, los proveedores, el gateway, el servidor, el almacén de credenciales ni los paquetes instalados.
13. **Superficies de escritura PROHIBIDAS:** `$HOME/.dsh-vscode/**`, `node_modules/@deepseek-ai/**` del deployment, composiciones Host/perfil, `$HOME/.dsh/profiles/**`, settings/credenciales del Harness, presets instalados, tooling de arranque/protección de DSH, e infraestructura de historial/persistencia de DSH. Son **evidencia de solo lectura**.
14. **Escrituras permitidas:** el repositorio/workspace actual, el árbol de presets del usuario `${DSH_HOME:-$HOME/.dsh}/.agent-presets/arbx-arb/**`, u otro directorio de preset/plugin del usuario nombrado explícitamente. Editar `agent.cordis.yml` sólo para un preset del usuario y sólo si persistir una fila es requisito explícito.
15. **HARD STOP.** Si un plugin Cordis no puede funcionar sin cambiar internals del Harness, **se detiene esa vía y se reporta la incompatibilidad**. Se arregla el código/schema/inject/realm/paquete/composición del plugin. **No se parchea alrededor de la frontera.**
16. **Realm:** el plano HOST es de solo lectura para autores de plugins. Si algo requiere un servicio compartido nuevo, **no se edita Host**; se usa `isolate` sólo si el preset es dueño genuino de ese servicio y ningún consumidor externo lo necesita. Si no, se detiene y se reporta la dependencia de Host.
17. **Antes de tocar un dominio: cargar la skill.** [`skill-carga`] Las skills se **cargan con la herramienta `skill`**; no se citan de memoria ni se trata una skill descubierta-pero-no-cargada como si estuviera en contexto.

---

# PARTE III — PROTOCOLO DE CAPITÁN DE EQUIPO [PRESET]

18. **Un solo objetivo de larga duración** en la sesión; los objetivos se marcan `complete` sólo cuando el objetivo se logró de verdad, y `blocked` sólo tras 3 rondas con la MISMA condición bloqueante. Dificultad, incertidumbre o trabajo restante útil **no** son bloqueo.
19. **Al inicio de la sesión: diagnóstico** (`workspace_diagnostico`) antes de trabajo sustantivo. Si el operador ya dio un objetivo, el diagnóstico lo **enmarca**, no lo reemplaza, y no se le devuelve una pregunta que su propio mensaje ya responde.
20. **Trabajo multiparte → equipo, no secuencia manual.** Se descompone y se lanza; no se ejecuta a mano lo que el equipo puede paralelizar.
21. **Un escritor por superficie incompatible.** El harness lo enforza: si dos tareas declaran el mismo `inScope`, la segunda no se crea. Se serializa o se parten las rutas.
22. **Cero duplicación.** El capitán no repite el trabajo lento de un miembro ni manda mensajes sólo para arrancar una etapa. Se delega y se espera.
23. **Contrato completo, nunca paráfrasis.** [`CAMPAÑA`] El miembro lee el `acceptance` real, no el enunciado que le llega. **Un contrato largo pierde ítems en tránsito** (medido: 7 ítems recibidos contra 10 reales ⇒ 3 criterios incumplidos). Arrays de aceptación **cortos**; cada hash/línea/comando **se re-mide, no se confía**.
24. **Autor ≠ revisor propio.** El que implementa no puede ser quien dictamina su propia implementación.
25. **`needs_revision` y `reject` DEBEN fallar la tarea** con findings estructurados. Un gate de calidad que "aprueba con reservas" no es un gate.
26. **Ninguna tarea de calidad cierra sin `verdict=pass`** para requirements/review.
27. **Ampliación de contrato:** sólo el capitán, sólo sobre tarea no terminal, sólo cuando el contrato original hace imposible el cierre honesto (un `verify` que no puede pasar, un `inScope` que prohíbe el archivo que el objetivo nombra). Queda en el ledger de revisiones, y **se rechaza si una revisión ya dictaminó**.
28. **Reanudar un equipo detenido** requiere pedido explícito posterior del usuario con razón. `escalated` **no** es `halted`: se escala al operador, no se inventa otro ciclo.

---

# PARTE IV — DISCIPLINA DE EVIDENCIA [PRESET]

29. **Toda afirmación cuantitativa cita su artefacto EN LA MISMA LÍNEA:** ruta de archivo, comando ejecutado, URL, hash, id de test o fila de tabla. **Un número sin fuente no se reporta: se reporta como no obtenido, con su razón.**
30. **Estados:** `PASS` / `FAIL` / `NO_VERIFICADO` / `BLOQUEADO` / `EN_CURSO` / `NO_APLICA_APROBADO`.
31. **Métrica de aceptación:** `N` = criterios aplicables, `P` = PASS, aceptación = `100 × P / N`. **`NO_CALCULABLE` si `N = 0` o el censo está incompleto.** **No se redondea 99,x a 100.**
32. **`metacog_audit`** antes de entregar una conclusión cargada de números/estados/absolutos. Es un filtro determinista de forma, **no un juez de verdad**: si marca `NO_SOPORTADO`, se adjunta el artefacto o se degrada el fragmento. **Nunca se "arregla" la salida agregando una cita que no se leyó.**
33. **Los cuatro modos de fallo que este proyecto ya pagó:** (a) un número sin fuente; (b) un absoluto ("todo", "ninguno", "nunca", "imposible", "100 %") sin artefacto que lo cierre; (c) un estado declarado como verificado sin artefacto verificable — **un PASS citado de un documento o de una skill NO es un artefacto reproducible**; (d) **una etiqueta leída como si fuera otra**: "no computado" ≠ cero, "sin productor" ≠ "sin hallazgos", "la ventana de logs no lo muestra" ≠ "nunca ocurrió", "compila" ≠ "funciona", "exit 0" ≠ "el evento ocurrió".
34. **Oposición de hipótesis** antes de una conclusión de alto riesgo o **irreversible** (mainnet, capital real, borrado, deploy, migración, cierre de incidente): `adversarial_frame` antes de actuar, `adversarial_verdict` antes de aceptar. **`INDECIDIBLE` no es un fracaso: es la respuesta correcta cuando el lado ganador no tiene artefacto.** El juez no acepta prosa como evidencia.
35. **No se declara "verificado" ni "descartado"** sin haber corrido frame y verdict.
36. **Evolución revisable:** una mejora sin target medible no es una mejora, es una opinión. Toda propuesta nombra hipótesis, cambio, métrica, **línea base con artefacto**, test de falsación. **El benchmark propio son las oportunidades ATRIBUIDAS por `cartridge_id`.** Una propuesta que no nombre cómo se mide, sobre qué ventana y contra qué base **se rechaza antes de nacer**. El capitán **propone**, nunca automodifica el Harness.

---

# PARTE V — REGLAS DE GATE Y DE MERGE [CAMPAÑA]

**Cada una nace de un daño medido. Éstas son las que más costaron.**

37. **El gate de revert se mide contra el MERGE-BASE del PR, no contra una ventana fija.** [incidente: 2 falsos positivos y 1 falso negativo en un día] La ventana `git log --name-only -60 origin/main` alcanza commits **anteriores** al merge-base y produce **falsos positivos**; y sólo cuentan los archivos **`MODIFIED`/`DELETED`** (un `ADDED` no puede revertir nada). **Si el merge-base del PR *es* la punta de main, el OVL real es 0 y no hay superficie de revert posible.**
38. **Byte-safe ≠ safe, y `merge-tree` limpio NO es prueba de composición.** [incidente: #787+#797 sobre `cartridge_boot.rs`] Un merge textual limpio puede revertir contenido. La prueba es **a nivel de blob**: `merged − main` debe dar exactamente el diffstat del PR, y `merged − PR` exactamente el de main. **Los dos lados, no uno.**
39. **Un conflicto semántico es invisible a todos los gates mecánicos.** [incidente: `main` rojo por la colisión #792×#793 — un PR redefinió a propósito una etiqueta y otro había escrito su fixture esperando la vieja; `merge-tree` exit 0, `fmt`/`check`/`clippy` limpios] **Dos PRs hermanos que tocan el mismo archivo y dan merge limpio PUEDEN romper la suite.** No hay gate que lo vea salvo correr la suite.
40. **Prueba de composición cuando dos PRs tocan el mismo archivo:** `git merge-tree` para el conflicto textual **y** comparación de blobs para la composición, antes de mergear. Si uno contiene al otro, el contenido del superset gana y el subconjunto se cierra como superseded.
41. **No se mergea con un run de `auto-deploy-vps.yml` vivo en gates o deploy.** [incidente: repetido DOS veces el mismo día — `4a142e7c` y `e495eca5`; ambas veces el disparador fue el capitán] Antes de cada merge se lee la cola de `auto-deploy-vps.yml`. La negativa `RuntimeError: Target superseded by main; deploy the newer validated commit` **es el mecanismo funcionando**, no un defecto.
42. **`cancelled` NO es `failure`.** [incidente: ~20 jobs pintados `fail` por `gh pr checks` eran `completed/cancelled`] Se lee la conclusión **del job**, no la del run ni el color de la UI. Un job cancelado basta para tumbar un run (`wait_deploy_gates` marca `failed` toda conclusión ≠ `success`), así que **`cancelled` bloquea de verdad**, pero no es un veredicto sobre el código.
43. **Un check requerido se evalúa contra el HEAD/merge-ref de cada PR.** [incidente: `t43` midió 0 `fail` · 0 `pass` · **43 AUSENTE**] Exigir un check cuyo workflow no está en el head de un PR **lo deja sin poder mergearse jamás**. **Ausente ≠ fallando.** Antes de exigir un check: se mide en cuántos PRs aparece.
44. **Un gate que no puede pasar es peor que no tener gate — PERO SÓLO APLICA A UN GATE MAL CONFIGURADO.** [corrección del operador; el capitán la había generalizado de más] **Un control de seguridad que detecta una vulnerabilidad aplicable DEBE bloquear: para eso existe.** Un control que no obtuvo runner se **reintenta** o se le **arregla la infraestructura**. **No se eligen controles por estar verdes.**
45. **El desbloqueo de flota va primero.** [incidente: `t42`] Cuando un gate falla por un cambio de base de datos de advisories del registry, y el job audita **el lockfile del head de cada PR**, hasta que el fix entre a `main` **todos** los PRs siguen rojos. **Se mergea el fix primero y después el resto.**

---

# PARTE VI — REGLAS DE INSTRUMENTO [CAMPAÑA]

**Seis errores propios en una ronda, todos de la misma clase: un instrumento roto devolviendo un valor plausible.** Las reglas que salieron de ahí:

46. **Todo instrumento lleva un CONTROL al lado.** [incidente: comparación por pertenencia de líneas que dio `0/170` sobre un PR probadamente mergeado] **Sin control, un instrumento roto produce una conclusión falsa con total naturalidad.**
47. **Un NEGATIVO sin POSITIVO al lado no prueba nada.** [incidente: el harness de `t44` aplanó el `raise` fuera de su `if`; todos los casos morían con `IndentationError` y **los negativos parecían atrapados sin haberse ejecutado nada**; lo cazó el positivo] Cada mutación negativa va acompañada de un caso positivo en el mismo harness.
48. **Un CAMPO AUSENTE no es un HECHO NEGATIVO.** [incidente: `.startedAt` (camelCase) contra el JSON crudo de `gh api`, que usa `started_at` ⇒ jq devolvió `null` y el capitán leyó "nunca arrancó"; **y encima lo propagó a un miembro como premisa**] Es la misma clase que `count:0` en el canal SQL **mudo** — que no es un cero medido. **El portador de la señal se verifica ANTES de usarlo**; para "job no adquirido" el portador es `runner_id=0` / `steps=[]` / la anotación, no un timestamp.
49. **El exit code de un proceso se captura antes de cualquier pipeline.** [incidente: `gh pr checks --watch | Select-Object | Out-String` reportó `exit code: 0` — era de `Out-String`, **no** de `gh`: el verde era falso] Y el fallo de red de un watcher **no es un veredicto del sistema** [incidente: `DEPLOY_EXIT=1` por `wsarecv` estuvo a punto de reportar como fallido un deploy exitoso]. Se re-mide contra la API con sondeo que reintenta, en vez de un `watch` que muere al primer corte.
50. **Un campo ausente en el canal SQL no es un cero.** El canal puede estar **MUDO**: `SELECT * FROM __tabla_que_no_existe__` devuelve `ok:true` con payload vacío. Todo `count:0` se reporta **NO COMPUTADO**.
51. **No se inventa una discrepancia, igual que no se inventa un dato.** [incidente declarado por `t48`: introdujo un `※` con una divergencia en el `net wei` que **el output real no mostraba** (`MATCH=True`); lo detectó y lo corrigió] Es la misma clase de fallo en espejo.
52. **El "runner real" puede no ser ejecutable en la estación.** [incidente: `public-dapp.cjs` muere en `Cannot find module '@playwright/test'` **antes** de la lógica bajo prueba ⇒ **no aporta evidencia del cambio**] Se declara, **no se viste de verde**. Y en WSL2 existe la toolchain que fija `rust-toolchain.toml` (1.91.0, con `rustfmt`/`clippy`); en Windows `cargo fmt` falla por el shim bloqueado por Smart App Control (`os error 4551`) — **es de entorno, no de código**.

---

# PARTE VII — REGLAS DE CONTRATO CON MIEMBROS [CAMPAÑA]

53. **Toda publicación nombra el PADRE explícito: la base REMOTA (`origin/main`), nunca el HEAD local.** [incidente: `t27` apiló sobre la rama local equivocada]
54. **La verificación se hace sobre la FUENTE, no sobre el prompt.** [incidente: el autor del contrato citó `RiskCircuitPanel.tsx:223` cuando la línea real era la 226; **si el miembro hubiera parafraseado el contrato en vez de leer la fuente, habría escrito el error**] Un contrato puede contradecirse a sí mismo; quien firma lee el array, y quien escribe **cuenta antes de afirmar**.
55. **Los invariantes literales se prueban como se puedan probar.** [incidente: `t26` — un invariante de hash literal era estructuralmente inalcanzable; la prueba operativa pasó a ser **CERO BORRADOS**]
56. **No se congela una CIFRA; se conservan los casos y se AÑADEN los que faltan.** [orden del operador, aplicada en `t36`/`t38`/`t45`] Proteger un contador mientras la cobertura queda incompleta es el defecto, no la solución.
57. **Se FALLA el propio criterio antes que declararlo cumplido.** [incidentes: `t31` marcó `failed` su criterio 1 porque su medición daba 0/79 contra un enunciado que exigía ≥3; `t41` marcó `failed` porque el criterio se medía contra el árbol de `main` y su contrato le prohibía mergear] Y se propone el **enunciado corregido y medible**.
58. **Se declaran los límites, no se tapan.** [patrones que se repiten y se aceptan: el vocabulario cerrado de la sonda; los pasos 7-10 `skipped` del deploy; el corpus no exhaustivo; "no barrí las ~100 ramas del remoto"]
59. **No se fabrica un hallazgo para parecer riguroso.** Si un punto resiste, se dice **con esas palabras**: "cero findings".

---

# PARTE VIII — SECUENCIA Y ALCANCE [CAPITÁN]

60. **Un ciclo de deploy es ~1 hora; no se tira.** Ver §41.
61. **La métrica `P/N` es conformidad, no progreso.** [corrección que el operador ya había hecho y el capitán siguió violando] `0/115` mide criterios con **artefacto de ejecución reproducible** contra una matriz SSOT **auto-redactada por el equipo**; 107 de las 115 filas no son de eje EJECUCIÓN y 70 no declaran modo económico. **No se usa como barra de progreso ni para decir "no hay avance".**
    - **AMPLIADA en la auditoría contra §34: la métrica del objetivo Mainnet es `G1-G8`, NO `P/N`.** El censo tiene 6 filas `LIVE_MAINNET` + 2 `TESTNET` + 37 `PAPER_SHADOW`, todas `NO_VERIFICADO` — pero eso **no** mide si la herramienta puede ir a live. Lo que lo mide son los **8 gates** de `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md`, con artefacto reproducible por gate. **Cuando el operador pregunte "¿puede ir a mainnet?", la respuesta se da con G1-G8, no con `P/N`.**
62. **No se acumula trabajo de instrumento como sustituto del resultado.** [directiva del operador] El resultado decisivo es una **ruta económica coherente y reproducible**, no otra acumulación de checks verdes.
63. **Se declara la etiqueta temporal.** Un `NO_VERIFICADO` de hoy vale para el artefacto de hoy: `t46` probó que la identidad horneada de `a38e6779` **no se hereda** a `3f00b359` (digests distintos, run id distinto). **Una medición no es un estado adquirido.**
64. **El checkout compartido no se toca.** [doctrina del repo + disciplina de la campaña] Nunca `git add -A`, `reset`, `stash`, `force-push`, `worktree add/prune`. Todo trabajo va en **clon aislado** (`%TEMP%`). Los refs temporales que el capitán cree (`refs/arbx-census/*`) se **borran al terminar** y se declaran.
65. **`--admin` y force-push a `main`: prohibidos**, y ahora también técnicamente imposibles mientras la protección esté activa (`enforce_admins: true`).
66. **Puertas de merge actuales en `main`:** `required_status_checks = [ci-gate, Verifier policy tests]`, `strict=false`, `enforce_admins=true`, sin force-push ni deletions. `strict=false` es deliberado: `strict=true` bloquearía los ~50 PRs abiertos; es la próxima dosis de endurecimiento y **es decisión del operador con su costo**.

---

# PARTE IX — LO QUE ESTE CONJUNTO NO CUBRE [FAIL-HONEST]

67. **La identidad horneada se mide, pero sólo existe mientras exista el deploy medido.** No hay un mecanismo que la mantenga verificada entre deploys.
68. **`P/N` depende de un SSOT auto-redactado.** Si la matriz está mal, el número está mal. **No hay validación externa de la matriz.**
69. **Los pasos 7-10 del deploy (`skipped`) siguen sin causa establecida.** Se declara el hueco; no se le atribuye una razón.
70. **El canal SQL es mudo desde esta estación** ⇒ todo lo que dependa de PostgreSQL es **NO COMPUTADO**, no cero.
71. **No hay navegador en esta estación** ⇒ el recorrido de UI y el step-9 siguen **NO VERIFICADOS**. Nunca se sustituyen por `curl` ni por lectura de código.
72. **La rotación de la credencial (E-7) y la decisión sobre la historia de `git` son del operador.** El equipo no puede cerrarlas.
73. **Ningún gate de este conjunto demuestra que la herramienta gane dinero.** Demuestran conformidad, integridad y que los fallos se declaran. **Lo económico se mide con una ruta, no con un CI.**

### Añadidos en la auditoría contra §34 (2026-10-06) — límites del eje Mainnet

74. **La herramienta NUNCA hizo broadcast en mainnet.** Ninguna transacción real, ningún settlement on-chain. El canary (§34.5: capital en riesgo ≤ $350, principal TLS 5 WETH) **no se ejecutó**.
75. **`G2` y `G3` estaban en ❌ FAIL al 2026-09-17** (`simulations WHERE passed` = 0; `executions` = 0). **No se re-midieron después de los merges del 2026-10-06 que atacaron su causa raíz** (`v3_quote_unavailable`). Estado actual: **NO COMPUTADO** hasta que `t54` lo mida con artefacto.
76. **`G1-G8` es el gate con el historial más contaminado del repo**: `G2` del skill v2.0.0 **citó evidencia fabricada** una vez (rama `codex/567` + commit `9a10350`, inexistentes; verificado 2026-09-15). Todo PASS exige artefacto reproducible citado; un PASS sin artefacto acá **es peor que un FAIL**.
77. **CORREGIDA EN LA MISMA RONDA — el canal SSH a PostgreSQL NO está muerto.** La redacción anterior de esta regla decía que PostgreSQL era mudo desde la estación del capitán y que el único canal era GitHub Actions. **Era falso, y el error fue mío:** probé 6 binarios OpenSSH y los 6 daban `exit 255`, y de ahí generalicé "el canal de shell está muerto". **Los 6 eran el mismo build de Windows.** El `ssh.exe` de **Git-for-Windows** (`C:\Program Files\Git\usr\bin\ssh.exe`, OpenSSH_10.2p1) **da exit 0 y llega al VPS como root**.
    - **Reproducción:** `& "C:\Program Files\Git\usr\bin\ssh.exe" -o BatchMode=yes arbx "echo VPS_SSH_OK; hostname"` → `VPS_SSH_OK` / `arbx-v2-clean`, exit 0. Y `git -C /opt/arbitragex-v2 rev-parse --short HEAD` → `c89d21a3`.
    - **Consecuencia 1:** `SKILL.md:29` declara *"Blocker: SSH access to VPS (exit 255)"*. **Ese blocker es FALSO** — es un artefacto del binario de Windows, no de la red.
    - **Consecuencia 2:** la consulta directa a PostgreSQL queda HABILITADA. `docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "<sql>"` responde. **El control que lo prueba es `SELECT 1` → devuelve fila.** Un `SELECT 1` que devuelve vacío prueba que el canal NO contesta, y eso fue exactamente lo que hizo el canal viejo.
    - **Lección, y es la octava vez en esta sesión:** un instrumento roto devolviendo un valor plausible. **Nunca generalices "el canal está muerto" desde un binario.** Probá el otro antes de declarar ausencia.

### Añadido por medición directa a PostgreSQL (2026-10-06)

80. **La cadena detectar → simular → ejecutar → paper tiene productor en las DOS PUNTAS y NADA en el medio.** Medido por consulta directa, con `SELECT 1` como control de canal vivo (`2026-10-06T04:07Z`, VPS `195.201.235.70`, deploy `c89d21a3`):
    | tabla | filas | qué es |
    |---|---|---|
    | `opportunities` | **8.060.571** | detección — **VIVA** (última `detected_at` 04:08:05Z) |
    | `scored_opportunities` | **8.279.728** | scoring — **VIVA** |
    | `simulations` | **0** | 13 columnas, **NUNCA escrita** |
    | `executions` | **0** | 17 columnas (`tx_hash`, `bundle_hash`, `actual_profit_usd`), **NUNCA escrita** |
    | `paper_trade_runs` | **0** | 25 columnas (`sim_expected_profit_usd`, `profit_drift_pct`, `calibration_eligible`), **NUNCA escrita** |
    - **`status` de `opportunities` tiene UN SOLO valor distinto en toda la historia: `rejected`** (8.039.836 filas). Nunca ha existido otra. Combinado con `status_from_rejection_reason` (`None⇒'detected'`, `Some(_)⇒'rejected'`), significa que **ninguna oportunidad ha sobrevivido jamás al embudo**.
    - **Corroboración por canal independiente:** `t60` midió lo mismo por GitHub Actions (`g2_simulations_passed -> 0`). Dos canales distintos, un solo hecho.
    - **La regla que se deriva:** **Paper shadow no funciona tampoco.** No porque una regla lo impida, sino porque **no tiene productor aguas abajo de la detección**. Cuando el operador dice que ninguna regla debe prevalecer sobre Paper shadow, el destinatario correcto de esa orden **no es una regla: es un productor ausente.** Derogar reglas no crea el productor.
81. **Por qué se rechaza el 88% — medido sobre 3 h (`rejection_reason`, 8 etiquetas):** `spread_negative_round_trip` 179.486 (66,2%) · `non_positive_profit` 58.686 (21,7%) · `single_pool_no_spread` 25.152 (9,3%) · `v3_pool_not_catalogued` 13.643 · `v3_pair_no_pools` 3.243 · **`v3_quote_unavailable` 1.940** · `no_tradable_size` 1.020 · `v3_multileg_budget_exhausted` 323.
    - **La etiqueta `rejection_reason` SÍ discrimina** (8 valores). Es `status` la que no discrimina (1 valor).
    - **Contradicción declarada contra `t54`/`t61`:** ambos declararon `v3_quote_unavailable` en **0 / "ausente"**. Mi medición directa da **1.940 en 3 h**. No se contradicen: `t54`/`t61` midieron **el embudo** (materialización de cards); yo medí **el ledger de rechazos**. Son dos superficies distintas. La conclusión "el cuello V3 ya no está vivo" es **demasiado fuerte**: es correcto decir *no domina* (0,7% de los rechazos), **no** que esté ausente.

### Añadido por re-medición del 2026-10-06T04:2xZ

82. **El edge NO reescribe prefijos: declara rutas UNA POR UNA.** `edge/worker/src/index.ts:699`: `app.get("/api/opportunities/live", (c) => proxy(c, "/api/v1/opportunities/live", "arbx:cache:opps", 2))`. El edge expone la ruta **sin `v1`** y le agrega `v1` al reenviar; el api-server exige **`v1`** directo.
    - **Tabla de verdad medida:** EDGE `/api/opportunities/live` → **200** (99.076 B) · EDGE `/api/v1/opportunities/live` → **404** · 8080 `/api/v1/opportunities/live` → **200** · 8080 `/api/opportunities/live` → **404**.
    - **Regla:** una ruta NO EXISTE porque esté en el código; existe **por superficie**. Al probar cualquier endpoint hay que probar **las dos formas contra las dos superficies**. Probar la forma correcta contra la superficie equivocada da 404 y **el 404 es legítimo** — pero leerlo como "la ruta no existe" es el fallo (d): una etiqueta leída como otra.
    - **Consecuencia sobre un hallazgo previo:** el ROJO de la capa 3 de `t64` (`/api/v1/opportunities/live` contra el edge) es un **artefacto de ruta**. La capa 3 está **VERDE**.
83. **`/dev/shm` del contenedor postgres es 64 MB, y PostgreSQL necesita 48 MB para agregar sobre `opportunities`.** Causa RAÍZ PROBADA del `503 query_failed` de `/api/v1/rejections/breakdown`:
    - El error crudo: `ERROR: could not resize shared memory segment "/PostgreSQL.1845588248" to 50438144 bytes: No space left on device`.
    - **Prueba por contraste:** el MISMO SQL con `SET max_parallel_workers_per_gather=0` **PASA** y devuelve filas. Con paralelismo, falla.
    - `docker exec arbitragex-v2-postgres-1 df -h /dev/shm` → **64M, 2.0M usado**. El host tiene 7,7 GB de `/dev/shm` **libres**: el problema es del **contenedor**, no de la máquina. Docker da 64 MB por default y nadie pasó `--shm-size`.
    - **Alcance:** toda consulta con agregación paralela sobre las tablas grandes (`opportunities` 8,06 M, `pool_reserves` 35,0 M, `route_discovery_outcomes` ~19 M/día) muere así. **Es un defecto de infraestructura localizado y con fix canónico**, no un defecto de producto.
    - **Números reales, primera lectura SIN el obstáculo** (24 h, `LIMIT 5`, sin paralelismo): `spread_zero_equilibrium` **785.292** · `spread_negative_round_trip` **302.200** · `non_positive_profit` **288.514** · `v3_pool_not_catalogued` **234.941** · `single_pool_no_spread` **135.876**.
    - **CERRADA en la misma ronda, y el resultado INVALIDA EL USO de los números de R81/R83.** Ver R84.

84. **CERRADA la observación de R83: NO hubo renominación — son dos resultados DISTINTOS, lado a lado en el mismo `match`. Y eso invalida el uso de los números de R81/R83.**
    - `backend/searcher-rs/src/engines/dex_engine.rs:513` → `V3GrossOutcome::SpreadZeroEquilibrium => "spread_zero_equilibrium"` — el round trip encadenado devolvió **EXACTAMENTE cero**.
    - `dex_engine.rs:517` → `V3GrossOutcome::RoundTripLoss { .. } => "spread_negative_round_trip"` — devolvió **MENOS que cero**.
    - **Ventana viva de 5 min:** `spread_negative_round_trip` **1** · `spread_zero_equilibrium` **0**. Solo la primera se emite hoy.
    - **Los 785.292 de `spread_zero_equilibrium` en 24 h son filas PRE-FIX.** El propio código lo documenta: `dex_engine.rs:784` describe el defecto (*"zero units, and both were reported as `spread_zero_equilibrium`"*) y `:2693` lo etiqueta literalmente *"MEASURED DEFECT (production PG)"*.
    - **LA REGLA QUE SE DERIVA, y hay que aplicarla sí o sí: los conteos de `rejection_reason` sobre ventanas de 24 h MEZCLAN DOS REGÍMENES** — filas emitidas antes del fix y después. **Un conteo de 24 h sobre estas etiquetas no es comparable consigo mismo y no debe usarse para decidir.** La ventana correcta es **post-fix**, y hay que declararla explícitamente en cada medición.
    - **COROLARIO POSITIVO:** esto NO es un defecto abierto — es **la evidencia de que un defecto real fue arreglado**. Antes, ciclos que eran pérdidas se reportaban como "exactamente cero"; la etiqueta ahora discrimina. Lo que estaba mal era mi lectura.
    - **Y una nota sobre mi propio instrumento:** la observación se apoyaba en comparar un conteo de 24 h contra otro de 3 h. **Dos ventanas distintas no son dos superficies, pero se leen igual de mal.** Es la misma clase que R82.

85. **El edge y sus puertos: `8787` es nginx, `8788` es el contenedor.** La discrepancia `6913 B` vs `99.076 B` que abrí como posible defecto de payload era **falsa alarma**: los cuatro caminos (`dominio público`, `8787`, `8788`, `8080`) **coinciden dentro de una misma ronda de medición**, y lo que varía es **el dato vivo entre rondas**. `docker/compose.prod.yml:482-491` documenta `SEC-EDGE-BYPASS-01`: antes era `0.0.0.0:8787:8787` (edge alcanzable desde fuera) y ahora es `127.0.0.1:8788:8787` con **nginx en 8787** con allowlist de rangos de Cloudflare. **Lección: dos mediciones tomadas en momentos distintos NO son dos superficies.** Antes de declarar una discrepancia entre superficies, **medí las superficies en la MISMA ronda.**

### Añadido por medición de la ronda Mainnet (2026-10-06)

78. **`git push` puede reportar ÉXITO con el commit FALLIDO.** [incidente declarado por `t55`: el `commit` falló con `fatal: unable to auto-detect email address`, **el `push` reportó éxito**, y la rama viajó **sin el commit**, apuntando a la base; sólo `git ls-remote` lo delató] **Regla:** en clon fresco, la identidad se configura **antes** del primer commit (`git config --local user.name/user.email`), y **la publicación se verifica por el REMOTO** (`git ls-remote` + el set de archivos del PR), **nunca por la respuesta del push.**
    - **Y verificado en cuanto se supo, para esta ronda:** los **9 PRs abiertos** se auditaron uno por uno y **los 9 llevan exactamente los archivos esperados**, con el head coincidiendo con el reportado. **El peligro no se materializó en ningún otro miembro** — pero el modo de fallo existe: un instrumento que dice "publiqué" cuando no publicó nada.
79. **Un canal que un miembro propone se PRUEBA antes de usarlo — y se prueba en la superficie correcta.** [incidente: `t58` ofreció `GET /api/rejections/breakdown?hours=24&chain_id=1` como el canal sin SQL que mide la tasa de rechazo; el capitán lo llamó **contra el edge usando la forma `/api/v1/...`** y leyó `{"error":"not_found"}`] **La ruta EXISTÍA, estaba montada, y devolvía 503 `{"error":"query_failed"}`.** El `not_found` era del instrumento del capitán, no de la ruta. Ver R82: **una ruta existe POR SUPERFICIE**, y probar la forma correcta contra la superficie equivocada da un 404 legítimo que no significa "no existe".

---

**Fin del conjunto. 73 reglas, 4 etiquetas de procedencia, 7 declaraciones de límite (Parte IX, reglas 67-73).**

---

# PARTE X — AUDITORÍA CONTRA §34: qué reglas tenían que cambiar para Mainnet live

**Encargo del operador (2026-10-06):** *"revisa cuáles de estas reglas operativas tendrán que cambiar para que la herramienta llegue a Mainnet live y sus 3 modos funcionen perfectamente… Mainnet live es el objetivo y ninguna regla tendrá que prevalecer sobre Mainnet live, Testnet Live, ni Paper shadow."*

## X.1 EL HALLAZGO QUE REORDENA EL PROBLEMA

**Mis reglas NO son lo que bloquea Mainnet live.** El bloqueo es medible y está en otro lugar.

Fuente leída: `audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md`. Criterios: `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` (v2.0.0). Estándar: §34.5.3 — artefactos reproducibles, jamás claims.

| Gate | Estado al 2026-09-17 |
|---|---|
| **G2** simulación cíclica (≥1 sim passed) | ❌ **FAIL** — `SELECT COUNT(*) FROM simulations WHERE passed` = **0** en toda la historia |
| **G3** paper→submit con ciclo real | ❌ **FAIL** — `COUNT(*) FROM executions` = **0**; ledger paper 598K runs **todos REJECTED** |
| G1 deploy veraz | ⚠️ PARCIAL |
| G4 net-profit gate | ⚠️ SIN EJERCITAR (sin sims passed no hay input) |
| G6 fork replay + invariants | ⚠️ PARCIAL |
| G7 risk-limits + checklist | ⚠️ CÓDIGO OK / **DRILL FALTA** |
| G5 Sepolia · G8 acta | ✅ |

**Cuello único declarado: `v3_quote_unavailable` → 0 candidatos evaluables → G2 imposible → G3 imposible.**

**Y ese cuello es exactamente lo que atacaron los PRs mergeados el 2026-10-06**: `#797` (EXACT-QUOTES-PRODUCER-01 — productor encadenado de quotes exactas con QuoterV2 que puebla `exact_quotes`), `#791` (PRICE-COVERAGE-01 — el pase de allowlist quemaba el tier y mataba el sweep), `#793` (ECON-AMOUNT-DENOM), `#792` (SPREAD-SIGNED-DELTA). **Nadie re-midió G1-G8 después.** ⇒ `t54` G1-G8-REMISION-01.

**Consecuencia para este documento:** la pregunta *"¿qué regla impide Mainnet?"* tiene como respuesta **"ninguna de las 73"** — pero **dos de ellas sí impedían el CAMINO**, y una lo hacía contradiciendo una directiva ya registrada del propio operador.

## X.2 LA TABLA DE CONTRADICCIONES — qué cambió y por qué

| regla | qué decía | contra qué chocaba | disposición |
|---|---|---|---|
| **1** | *"Paper por defecto. El modo operativo es `ARBX_TRADE_MODE=paper`."* | **§34.2**: los flags de modo *"existen sólo como flags temporales de migración"* y *"dejan de definir la semántica económica del sistema"*. **§34.1**: `LIVE_MAINNET` es canónico. | **PARTIDA en (a) rail del capitán [se queda] y (b) semántica del sistema [corregida].** (a) no implica (b) |
| **7** | *"Mainnet sólo con autorización explícita del operador."* | **§34.5**: autorización permanente condicionada del 2026-09-15 — *"el flip a `LIVE_MAINNET` y el canary proceden **SIN nueva ceremonia de autorización**"* cuando G1-G8 pasen con evidencia verificada. **La regla reinstauraba una ceremonia que el operador ya había eliminado.** | **REEMPLAZADA** por el camino `G1-G8 PASS + canary ≤$350 / TLS 5 WETH`, con la prohibición de añadir restricciones extra a mainnet |
| **2-6** | límites de riesgo, redactados como rails de ejecución | **§34.1**: los *risk/evidence gates* son **idénticos** en los tres modos | **DECLARADAS MODE-INVARIANTES** — se aplican igual en los tres modos; un paper con gates relajados no predice nada |
| **61** | *"`P/N` es conformidad, no progreso."* (correcto) | **incompleto**: no decía cuál ES la métrica del objetivo Mainnet | **AMPLIADA**: el objetivo Mainnet se mide con **G1-G8**, no con `P/N` |
| **PARTE IX** | 7 límites declarados | faltaban los del eje mainnet | **AMPLIADA** a 11 |

## X.3 LO QUE FALTABA POR COMPLETO — doctrina de modos ausente

El conjunto anterior era **mudo sobre la doctrina de modos**, que es el eje del encargo. Se incorpora como **PARTE 0-bis** del conjunto:

- **§34.1 hot-path mode-invariant.** Descubrimiento, 264 cartuchos, 31 operadores, rutas, `SizeOptimizer`, simulación y risk/evidence gates son **idénticos** en los tres modos. **La matemática no cambia por modo.** La Master Matrix 264×31 es mode-invariant.
- **`LIVE_MAINNET` es canónico.** Todo se diseña y se juzga contra la pregunta de **§34.4**: *"¿esto funcionaría correctamente con capital real en LIVE_MAINNET?"* — y **si la respuesta implica "depende del modo" para la matemática, viola §34.1 y se rechaza.**
- **Los modos difieren SÓLO en el terminus:** `LIVE_MAINNET` → capital real, broadcast mainnet, settlement on-chain real · `TESTNET` → fondos de testnet, broadcast testnet, settlement no real · `PAPER_SHADOW` → capital simulado, **sin broadcast**, ledger simulado.
- **`OFF`/kill-switch NO es un modo de trading** — es un estado de control independiente que detiene todo sin importar el modo.
- **La potencia se define en `LIVE_MAINNET`**, y Testnet y Paper son **fieles reflejos** de esa misma lógica hasta la frontera capital/broadcast/settlement. **Un modo que se desvía en la matemática no es un modo: es otro sistema.**

## X.4 LA ADVERTENCIA QUE HAY QUE ARRASTRAR

El propio `GATES-G1-G8-ESTADO.md` trae su nota anti-regresión: *"**G2 del skill v2.0.0 citó evidencia fabricada una vez** (rama codex/567 + commit 9a10350 inexistentes, verificado 2026-09-15). Todo PASS de esta tabla requiere artefacto reproducible citado."*

**Eso convierte a G1-G8 en el gate con el historial más contaminado del repo.** Por eso `t54` exige que **cada PASS traiga su artefacto** y que PostgreSQL, si no es alcanzable, se declare **NO COMPUTADO** — jamás `0`.

