# WO-02c-FIX-UNPINNED (2026-09-17, fixer gang ronda 1)

> Charter: GAP-3 del cross-exam del verify (`WO-02c-verify-CROSS-EXAM.md:54-60`,
> agent-fixable). El §10.2 del adendum sobrestrema la falta de trazabilidad del
> unpinned replay: dice "SIN log ni observación que lo declare", pero el
> doc-comment de `simulator_for_candidate` documenta explícitamente el caso
> None → simulador compartido sin pin. Lo ausente es observabilidad RUNTIME
> (log/métrica), no documentación.

## 1. Evidencia re-verificada (RULE 00 — no heredada del charter)

- `backend/sim-ctl/src/consumer.rs:768-775` — doc-comment de
  `simulator_for_candidate` (firma :776, cuerpo :776-784):

  > "The consumer's shared simulator carries whatever pin boot gave it
  > (typically none — `latest`); [...] the shared simulator is reused
  > untouched when no pin applies."

  DOCUMENTA el caso None → shared sin pin. Precisión de cita: el
  cross-examiner citó "consumer.rs:776-783" (la firma); el comentario vive
  en :768-775 (verificado con Read directo, 2026-09-17).
- Consumo del pin: `consumer.rs:608` — `block_number:
  inputs.block_number.filter(|b| *b >= 0)` → fila PG con `block_number
  NULL` produce candidate sin pin → replay contra `latest`. El fenómeno de
  degradación silenciosa de determinismo ES REAL y SOBREVIVE acotado.
- En el wiring no existe log/métrica runtime del evento unpinned (el
  `None` entra silencioso al match de :780-783). El candidato
  `sim_consumer.unpinned_replay` debug-log sigue **gated** (corrección de
  código = operador; NO aplicada — protocolo NO-GIT).

## 2. Cambios aplicados (todos con marcador `// WO-02c-FIX-UNPINNED (2026-09-17)`)

1. `02c-SIM-STACK-SELECTOR.md` §10.2 (era :291) — "**sin log ni observación
   que lo declare**" → "**documentado en el doc-comment de la función
   (consumer.rs:768-775: \"the shared simulator is reused untouched when no
   pin applies\") pero sin trazabilidad runtime (log/métrica) que lo
   declare**" + nota de acote citando el GAP-3.
2. `02c-SIM-STACK-SELECTOR.md` §12.2 hallazgo 2 (texto del fixer G2 en el
   mismo entregable, era :473-477) — "SIN log" → acote equivalente con cita
   del doc-comment. El contenido fáctico del par G2 queda legible; solo se
   acota la frase sobrestimada (mismo archivo = mismo WO-02c; el entregable
   es la superficie viva que la mesa consume, precedente G5/G3).
3. `WO-02c-verify-VERIFY.md` §2 hallazgo 1 (era :42-47) — "SIN log" →
   "sin trazabilidad runtime (log/métrica) — el caso None está documentado
   en el doc-comment (consumer.rs:768-775)". Precedente in-situ con
   marcador en ese archivo: FIX-G4 y FIX-TALLY.
4. Erratas append-only: §16 en `02c-SIM-STACK-SELECTOR.md` + §11 en
   `WO-02c-verify-VERIFY.md`.
5. Board: entrada `// WO-02c-FIX-UNPINNED APLICADA` en GOAL-WORKORDERS.md.

## 3. Contaminación residual (documentada, NO tocada)

- `WO-02c-FIX-G2-20260917.md:64` repite "sin log" — reporte histórico del
  fixer par G2, preservado inmutable (disciplina append-only entre pares;
  no es superficie viva de claims). Lectores: cruzar con errata §16.
- `WO-02c-verify-CROSS-EXAM.md:55` cita la frase original — es la PROPIA
  cita del gap, debe quedar.
- Matriz §8 del entregable (fila "degradación silenciosa de pin") NO
  contiene la sobrestimación — sin cambio.

## 4. Verificación del fix

- Post-edit grep en `audits/first-understand-20260917/`: "SIN log ni
  observación que lo declare" = 0 ocurrencias vivas (solo citas históricas
  dentro de marcadores/erratas propios, del cross-exam :55, y del reporte
  G2 :64 declarado en §3).
- Cita consumer.rs:768-775 verificada byte-exact con Read (2026-09-17).
- Hallazgo de fondo INTACTO: sigue siendo el único Option-vacío del wiring
  que degrada calidad sin trazabilidad runtime; clasificación "observación,
  no defecto R8" inalterada; candidato debug-log gated.

## 5. Restricciones cumplidas

- Cero git/cargo/npm/build/VPS/HTTP (0/5 requests del presupuesto dominio).
- Cero .rs tocados (doc-only; el fix de código `sim_consumer.unpinned_replay`
  queda gated operador).
- §32/§33/§34.3 intactos: nada de executor/wallets/capital/broadcast; no se
  tocó `live_exec_policy` ni default-deny.
- Archivos tocados: `02c-SIM-STACK-SELECTOR.md`,
  `WO-02c-verify-VERIFY.md`, este reporte, `GOAL-WORKORDERS.md` — todos
  reportes/board del gang (mismo WO-02c o superficie común del board), ningún
  archivo de otro WO pisado.
