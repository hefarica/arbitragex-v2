---

# PLAN DE ACCIÓN — Cierre de cumplimiento hasta 95 %

**Punto de partida:** 256 / 1.625 = **15,8 %** (último ciclo: −0,4 pp **por crecimiento del denominador**, no por pérdida de numerador).
**Objetivo:** ≥ 95 % (1.544 / 1.625) **con el denominador congelado y auditado**.
**Regla suprema:** MANTIS manda. Leer es libre, mutar pide aprobación. Sin artefacto no hay reporte. Todo mensaje termina en PEDIDO.

> **La tesis de este plan:** no se llega al 95 % trabajando más. Se llega **clasificando el denominador** y cerrando lo que no es trabajo. Hoy la métrica baja cuando aprendes, y eso es un defecto del instrumento, no del esfuerzo.

---

## §0. Primero el instrumento. El trabajo después.

Un porcentaje cuyo denominador crece más rápido que el numerador **castiga enumerar**. Si no se arregla antes de trabajar, cada hallazgo honesto te aleja del 95 %.

### 0.1 Congelar el denominador
- El denominador se cierra al inicio de cada ciclo. Los ítems nuevos entran al **ciclo siguiente**.
- Publicar siempre dos cifras: **numerador crudo** y **cobertura normalizada** sobre el denominador congelado. La primera es la verdad; la segunda es la que se persigue.

### 0.2 Columnas obligatorias de cada ítem
`id · familia · dueño · artefacto_esperado · criterio_de_aceptación · estado · evidencia`

Sin `dueño` o sin `criterio_de_aceptación`, el ítem **no cuenta** para el numerador aunque esté hecho.

### 0.3 Clasificar los 1.625 — **el movimiento que desbloquea el plan**
Todo ítem cae en una de siete cubetas. Solo las tres primeras se cierran **trabajando**; las otras cuatro se cierran **clasificando**:

| Cubeta | Se cierra con | ¿Sube el numerador? |
|---|---|---|
| Código | PR + test + deploy | Sí, trabajando |
| Config / operación | ventana + recibo + restart | Sí, trabajando |
| Humano | dueño + fecha + firma | Sí, con agenda |
| **Sin productor** | declarar VACUO con razón | **Sí, gratis** |
| **Duplicado** | fusionar en el canónico | **Sí, gratis** |
| **Bloqueado externo** | escalar con comando + dueño | No hasta desbloquear |
| **Rojo permanente** | declararlo y retirarlo del cómputo | **Sí, gratis** |

**Primer entregable obligatorio de la Fase A: el censo de las siete cubetas.** Es plausible que un 10–20 % del denominador sea VACUO o duplicado; cerrarlo con razón mueve el numerador cientos de ítems sin escribir una línea.

### 0.4 Reglas de conteo
1. Un ítem **VACUO** se publica VACUO, nunca verde.
2. Un hueco es **NOT COMPUTED**, nunca 0.
3. Un check en rojo **no se promedia**: se publica con su fila de ejemplo.
4. Ningún cierre por proximidad (deploy, CI, panel). Solo por medición.

---

## §1. Regla de oro del numerador

1. **Por cada ítem nuevo que entra al denominador, se cierra al menos uno.** Si no, el % baja por definición.
2. **Cierre por clasificación primero**: antes de trabajar un ítem, preguntar si tiene productor. Si no lo tiene, se cierra hoy.
3. **Ningún ítem queda en «pendiente» más de un ciclo.** O tiene dueño y fecha, o se declara rojo permanente.

---

## §2. Fases

### FASE A — Desbloqueo y censo (objetivo: +8–15 pp)
*Sin esto, ninguna fase posterior tiene base verde.*

| # | Acción | Criterio de salida | Dueño |
|---|---|---|---|
| A1 | **Arreglar el gate compartido** (advisories npm/cargo). Allowlist con razón y caducidad + PR programado de bump. **Un PR desbloquea 4 ramas.** | `main` verde; 4 ramas dejan de estar rojas por advisory | Dependencias |
| A2 | **Censo de las 7 cubetas** sobre los 1.625 | Tabla publicada con conteo por cubeta | Medición |
| A3 | **Triaje de ramas huérfanas**: PR o borrar. Prioridad a la de traza (0 commits ahead ⇒ no lleva nada) | 0 ramas con trabajo y sin PR | Cada autor |
| A4 | Borrar ramas fusionadas | Lista de ramas = solo vivas | Owner |
| A5 | Declarar los **rojos permanentes** y retirarlos del cómputo | Lista firmada de rojos permanentes | MANTIS |

**Salida de fase:** `main` verde · censo publicado · 0 ramas huérfanas · rojos permanentes declarados.

### FASE B — Cerrar la brecha de medición (objetivo: +15–25 pp)
*Cada familia de datos con su check ejecutable. Es lo que hace que el resto sea verificable.*

- B1 Checks del contrato de telemetría (§7) **con PR** — hoy viven en una rama sin vehículo.
- B2 Checks de selección (S01–S10) al repo, con la condición «población 0 ⇒ NOT MEASURED».
- B3 Instrumento de auditoría del feed con sus controles positivo/negativo en CI.
- B4 Índice de evidencia con `objeto` + invariante `ts_observado ≤ ts_mensaje`.
- B5 **Un check por ítem del numerador**: si un ítem se cierra, tiene que haber un check que lo pueda tumbar.

**Salida de fase:** toda familia de datos con check en CI; control negativo por instrumento.

### FASE C — La palanca de dinero (objetivo: +10–20 pp)
*Nada de A ni B mueve el resultado económico. Esto sí.*

- C1 **#13 — la segunda pata** (`dex_b` no nulo). Hoy 0/200. Es la única que bloquea todo.
- C2 **Nocional fuera de polvo**: el sizing no puede caer a `dustUsd`; fallback a `minBoundUsd`, y banco dimensionado para que la gas sea <0,5 % del nocional.
- C3 **Allowlist por censo**, no a mano; con versión/hash publicado.
- C4 **Clasificar el `Panic(0x11)`** por traza, no por lectura.
- C5 **Ventana única** (modo → ejecutor → flag → identidad): un recibo, un restart.

**Criterio de hito — no de fase:** **una fila `status=executed`, aunque el neto sea negativo.** Es el peldaño anterior a `recon > 0`.

### FASE D — Consolidación (objetivo: cerrar hasta 95 %)
- D1 Checklist a 7/7.
- D2 `second_signoff` con nombre y fecha (o rojo permanente).
- D3 Retirar del cómputo todo lo declarado VACUO y duplicado.
- D4 Auditoría de cierre: recorrer el numerador y tumbar 10 ítems al azar con su evidencia.

**Salida:** ≥ 95 % sobre denominador congelado, con el 5 % restante **nombrado** (rojos permanentes + bloqueos externos).

---

## §3. Cadencia y tablero

- **Un ciclo = una ventana de merge.** Al abrir: congelar denominador, fijar la lista de ítems del ciclo. Al cerrar: publicar numerador, cobertura normalizada y delta explicado (trabajo vs clasificación).
- **Congelación de merges** mientras haya un recibo pendiente. Un merge, un deploy, verificar.
- **Protocolo de disponibilidad:** sin trabajo ⇒ `SOLICITO TRABAJO` con menú de 2–4 opciones, coste, artefacto y recomendación.
- **Tablero con 4 columnas:** Hecho (con artefacto) · Abierto (con dueño y fecha) · Bloqueado (con comando y dueño externo) · Rojo permanente (firmado).

---

## §4. Prohibiciones

1. Cerrar por proximidad (deploy, CI, panel).
2. Reportar sin artefacto.
3. Armonizar dos instrumentos que discrepan.
4. Dejar un «pendiente» sin dueño ni fecha.
5. Añadir al denominador sin cerrar nada.
6. Publicar un ítem VACUO como verde.
7. Salpicar mutaciones de config: una ventana, un recibo, un restart.
8. Dos turnos sobre el mismo checkout sin worktree.

---

## §5. Riesgos y contramedidas

| Riesgo | Contramedida |
|---|---|
| El % baja al aprender | Denumerador congelado + cobertura normalizada (§0.1) |
| El gate de advisories congela el repo cada pocas semanas | Allowlist con caducidad + bump programado (A1) |
| Trabajo en ramas huérfanas | Regla: rama sin PR no cuenta como avance (A3) |
| Verdes sobre bases viejas | Rebasar antes de fusionar y **re-verificar** |
| Optimizar el panel y no el dinero | Hito `status=executed` como puerta dura |
| Cerrar ítems para inflar el número | Auditoría de cierre tumbando 10 al azar (D4) |

---

## §6. Definición de hecho

El plan está cumplido cuando, simultáneamente:

1. El numerador es **≥ 95 %** del denominador **congelado**.
2. El 5 % restante está **nombrado**: rojos permanentes firmados y bloqueos externos con dueño.
3. Existe **al menos una fila `status=executed`**.
4. Ningún ítem del numerador carece de evidencia reproducible.
5. Los 1.625 están **clasificados en las siete cubetas**, sin ítems «sin clasificar».

**Criterio de muerte del plan:** si tras **3 ciclos** el numerador no ha subido ≥ 15 pp, el plan está mal — y se reescribe, no se estira.

---

## Anexo A — Plantilla de ítem

```
id:            C-XX
familia:       código | config | humano | vacuo | duplicado | bloqueo | rojo-permanente
dueño:
artefacto:     sha / ruta / línea / comando
aceptación:    <condición verificable, con población y presentes>
estado:        hecho | abierto | bloqueado | rojo-permanente
evidencia:     <salida literal>
```

## Anexo B — Plantilla de reporte de ciclo

```
CICLO <n> — <fecha>
NUMERADOR: <h> / <D congelado> = <%>   · COBERTURA NORMALIZADA: <%>
DELTA: +<x> pp  = <a> por trabajo + <b> por clasificación
CERRADOS (artefacto): …
ABIERTOS (dueño + fecha): …
BLOQUEADOS (comando + dueño externo): …
ROJOS PERMANENTES: …
PEDIDO: …
```

## Anexo C — Cola priorizada de arranque

1. Gate de advisories (A1) — desbloquea 4 ramas.
2. Censo de las 7 cubetas (A2) — mueve el numerador sin código.
3. Traza del `Panic(0x11)` — decide el dueño del arreglo.
4. #13 segunda pata — la única que mueve el dinero.
5. Nocional fuera de polvo — sin esto, el gas domina por 6 órdenes.
6. Checks §7 y S01–S10 con PR — hoy sin vehículo.
7. `second_signoff` — dueño y fecha o rojo permanente.

---

**Nota final, para que no se pierda de vista:** el % de cumplimiento es un instrumento. El dinero es `recon > 0`. Un 95 % verde con `recon = 0` es un sistema perfectamente medido y perfectamente improductivo — y este plan, tal como está escrito, puede entregarte exactamente eso si la Fase C se pospone.
