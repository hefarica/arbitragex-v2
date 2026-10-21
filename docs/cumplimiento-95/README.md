# Censo de cumplimiento 95 % — A2

Artefacto de la Fase A del plan `docs/plan-95.md` (§0.3 A2, Anexo D v2). Construye el
**manifiesto** de la población: una fila por ítem enumerado, con dueño, criterio de
aceptación, estado y fuente. No estima el porcentaje.

## Qué publica

Tres cifras, siempre juntas, desde `scripts/checks/censo95.py`:

| cifra | significado |
|---|---|
| `poblacion_declarada` | 1.625 — **declarada por el owner**, no auditada: no existe como fichero en el repo |
| `con_fila` | ítems que hoy tienen una fila trazable en este JSON |
| `sin_fila` | `poblacion_declarada − con_fila`; mientras sea > 0, **C95 = NOT COMPUTED** (nunca 0, nunca «provisional») |

C95 es el único número que se publica como «95 %» (D.2.1). `GATE-VIVO`
(`arbi-95-watchdog`: `readiness.red==0 ∧ recon.total≠0`, 168 h) y `HITO-ECONÓMICO`
(`recon.total>0`, binario) se publican aparte, con su nombre y su ventana (D.2.2, D.2.3).
Un `GATE-VIVO` verde no suma cobertura; uno rojo impide declarar el ciclo en verde.

## Reglas duras

- Una fila por `id`, único. Sin `dueño` o `criterio_aceptacion` la fila no cuenta (§0.2).
- `familia` ∈ `codigo · config · humano · vacuo · duplicado · bloqueo · rojo-permanente`.
- `estado` ∈ `hecho · abierto · bloqueado · rojo-permanente`.
- `sin_fila` **no** es `vacuo`: lo no enumerado no se clasifica por defecto. `vacuo` exige
  razón escrita y fuente (D.3.7).
- Un verde de panel no cierra una fila (§4.4). Los gates de readiness se registran con
  `estado_gate_vivo` y quedan `abierto` salvo medición independiente.
- Una rama sin PR no cuenta como avance (A3). Por eso los checks §7 y S01–S10 están
  `abierto` aunque el artefacto exista.

## Cómo se verifica

```bash
python3 scripts/checks/censo95.py                       # valida + recalcula + sha256
python3 scripts/checks/censo95.py --json | jq .         # misma salida, para máquina
```

Sale con código ≠ 0 si una fila pierde un campo obligatorio, si hay ids duplicados, si el
`resumen` guardado no coincide con el recalculado, o si `sin_fila>0` sin declarar
NOT COMPUTED. Controles negativos ejecutados el 2026-10-21: fila sin `dueño` ⇒ exit 1;
`con_fila` alterado en el resumen ⇒ exit 1.

## Alcance declarado (lo que este censo NO dice)

- No dice que las 52 filas sean ítems de los 1.625: son familias que el plan nombra y que
  hoy se pueden enumerar con artefacto. El mapeo es **inferido, no verificado**, hasta que
  exista la lista de población. Por eso `con_fila` no se presenta como subconjunto
  demostrado ni el % se calcula.
- No dice que los 256 «cerrados» del owner estén mal: dice que **no son verificables** sin
  manifiesto. No se armonizan las dos cifras.
- `cerrados_por_medicion` cuenta sólo lo que se midió aquí, no lo que declara otro
  instrumento.

Corte: `2026-10-21T14:06:12Z`. Base: `54b1180489ec53a5fe377d945dc7d41342f402f5`.
