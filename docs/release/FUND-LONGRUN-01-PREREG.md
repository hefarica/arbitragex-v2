# FUND-LONGRUN-01 — PRE-REGISTRO DEL MÍNIMO EXIGIDO

> **Publicado ANTES de mirar los datos.** Este archivo se commitea y se empuja antes de
> extraer un solo agregado de `sim.funding_probe`. Su razón de existir es que el mínimo
> no se pueda acomodar después de ver el resultado. El commit que lo introduce y su
> fecha son el artefacto de precedencia; el primer timestamp de medición se declara en
> el informe `FUND-LONGRUN-01.md`.
>
> tarea=t122 · run_id=arbx-entrega-20261008 · SHA_BASE=`8414e51211d0a26d664b7e669af2eacbee8d1dd4`
> dueño=Release-B · permisos=SOLO LECTURA sobre el VPS · path escribible=docs/release/

## 1. Por qué hay un mínimo

La medición de t114 (`LAND-861-01`) fue: **25 intentos × 4 slots = 100 sondeos, en ~3 min
de un contenedor recién arrancado, con UN SOLO token y UN SOLO signer.** Su propio autor
declaró el límite textualmente:

> *"25 intentos / ~3 min de un contenedor recién arrancado, un solo token y un solo signer.
> Re-muestrear en marcha larga ANTES de diseñar el arreglo del slot."*

Dos defectos de inferencia se siguen de ahí, y son los que este re-muestreo ataca:

1. **N no es 100.** Los 4 slots de un mismo intento **no son 4 muestras independientes**:
   son 4 sondas del MISMO layout del MISMO token. La unidad de independencia es el
   **intento**, no el slot. Con 25 intentos, 100 sondeos son 25 unidades.
2. **Un solo token no distingue "falla WETH" de "falla todo".** Si el índice `[0,2,3,9]`
   no cubre el layout de un token, eso puede ser una propiedad **de ese token** o **del
   índice**. Con n_tokens = 1 las dos hipótesis son indistinguibles **por construcción**.

## 2. MÍNIMO EXIGIDO (declarado antes de mirar)

| id | exigencia | valor | por qué ese valor |
|---|---|---|---|
| **MIN-1** | intentos de `sim.funding_probe` contados | **≥ 200** | 8× los 25 de t114; baja el error estándar de una fracción cerca de 0,24 en ~2,8× |
| **MIN-2** | marcha continua desde `State.StartedAt`, sin reinicio dentro de la ventana | **≥ 30 min** | 10× los ~3 min de t114; cubre el transitorio de arranque (el contenedor medido en t114 llevaba 85 s en pie) |
| **MIN-3** | cobertura de la ventana de logs retenida (R9) | la primera línea retenida debe estar a **≤ 5 s** del arranque del contenedor | sin esto el conteo puede estar truncado por rotación y el "cero" sería de instrumento |
| **MIN-4** | ventana del caudal: dos muestras separadas | **≥ 600 s** | la anterior fue de 60 s — 10× más corta; con churn ~0/s una ventana corta no distingue "no hay churn" de "no lo vi" |
| **MIN-5** | si al mirar `intentos < MIN-1` o `edad < MIN-2` | **ESPERAR** hasta alcanzarlos, con tope de **40 min** de espera acumulada | esperar es barato; concluir sobre una muestra insuficiente no |
| **MIN-6** | tope de espera agotado sin alcanzar el mínimo | **declarar muestra insuficiente** y NO concluir sobre lo que el mínimo declarado no alcanza | un mínimo que se relaja tras mirar no es un mínimo |

### 2.1 Enmiendas

Ninguna. Si este archivo se enmienda después de la primera medición, la enmienda se
registra abajo **con su razón y su instante**, y el informe declara explícitamente que el
criterio se movió después de mirar.

## 3. Criterios de falsación (declarados antes de mirar)

- **F1 — tokens distintos.** Si el conjunto de tokens tiene **≥ 2 elementos**, la medición
  pasa a poder distinguir por token y el informe reporta el desglose **por token**. Si
  tiene **1 elemento**, el informe declara que el **instrumento** no distingue, y la
  conclusión pasa a ser sobre el alcance del instrumento — **no** sobre el índice.
- **F2 — el dato que desbloquea.** Si aparece **≥ 1** sondeo con `balance_of` == centinela,
  se declara con su **token** y su **slot**: eso probaría que el índice SÍ cubre al menos
  un layout y que el fallo de WETH es específico del token.
- **F3 — "unreadable".** La fracción de `balance_of = "unreadable"` se compara contra
  **24/100 = 24,0 %** (t114, misma vía: log) y contra **18,68 %** (t107, vía Prometheus
  `balance_unreadable`). Se declara si se mantiene, sube o baja; **nunca** se agrupa con
  `balance_of = "0"`.
- **F4 — margen de retención.** `retención = 10 000 / tasa_de_llegada` segundos (MAXLEN del
  stream); `margen = retención − oldest_pending_ms`. **Signo positivo** ⇒ la ventana de
  retención cubre el pendiente. **Umbral de signo = 10 000 / oldest_pending_ms** entradas/s.
  El signo se reporta **con** su tasa y su umbral: no es una propiedad estructural
  (+68,62 s a 21,1/s · −205,85 s a 70,75/s; umbral ~28,8/s en t110).
- **F5 — `passed=true`.** Se reporta el conteo. Cero se declara **con** la prueba de que el
  productor existe (`{passed="false",...}` y `{simulator="revm"}` emitidos); sin esa prueba
  el cero es de instrumento.

## 4. Reglas de instrumento que esta campaña ya pagó (se aplican sin excepción)

1. **Control de canal PRIMERO**: `SELECT 1` debe devolver fila antes de reportar cualquier cero.
2. Toda cantidad que dependa de una tasa se reporta **con su tasa y con su umbral de signo**.
3. **Un cero de instrumento no es un cero de fenómeno**: `bound_report` es un EVENTO DE LOG
   con `BOUND_LOG_INTERVAL = 600 s`; un grep vacío antes de la cadencia es cero de instrumento.
4. NUNCA inferir éxito de un `grep -c = 0`; un grep de archivo matchea PROSA.
5. R9: declarar la ventana de logs retenida (líneas y rango de timestamps contra el arranque)
   **antes** de concluir cualquier ausencia.
6. **SOLO LECTURA** sobre el VPS. Prohibido reiniciar, reparar, mergear, desplegar.
