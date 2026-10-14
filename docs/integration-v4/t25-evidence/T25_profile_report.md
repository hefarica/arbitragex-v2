# T25 — (a) Perfil cola p99 v4 · TIME-BOX cerrado · VEREDICTO: eval de cartuchos ES la cola (hipótesis confirmada); mitigación diseñada; release queda para protocolo completo

Attempt 1 16966dd1 · box declarado 90 min en timebox.txt (cierre obligatorio antes de cualquier flip —
EXC-001 sigue ABIERTO SIN FIRMA). Gate de entrada: t24=REGRESIÓN REAL (cita dependency) ⇒ se procedió.

## 1. PERFIL (todo comando+salida en t25-evidence/)

**Hipótesis del contrato: pertinent=269 × 31 operadores es CANDIDATO. VEREDICTO: CONFIRMADO como
cola dominante del pipeline — con tres matices medidos:**

- **Eval enter→summary: p50=2674ms, p90=3219ms, max=111.7s** (n=202 tx con par enter+summary,
  logs completos del contenedor VIEJO 20:12:02→20:15:30Z, 8360 eventos traccionados —
  viejo_eval_events.log, parseo reproducible). ~913 tx distintos evaluados/min (3195 tx en 3.5min;
  1474-2166 eval_enter/min).
- **La eval produce 0 positivos**: pertinent=269, negative=269, positive=0 en TODAS las muestras;
  causas dominantes `dispatch_needs_route_data`=174 (65%) y bloque capital/costs MISSING ×58,
  `operators.21/22::DATA_GAP` ×54/58 (cartridge.active_eval_summary, profile_probe3.txt).
  ⇒ Hoy la eval es FRICCIÓN PURA (269×31 operador-evals por tx para 0 output).
- **La cola es ESTRUCTURAL, no exclusiva del v4**: el VIEJO (dfd71012e1dc, post-rollback, arranque
  20:11:49Z) la padece idéntico (es quien produjo los números arriba) Y su feed AHORA muestra
  p50 1528→1877 con colas p90 7916→20128ms en dos samples (n=50 y n=41 — DECLARADOS anecdóticos:
  no se concluye ratio de feed con n<200; t24 estableció esa regla). No se observó decay de
  cold-start en 13 min.
- **Causa del DELTA v4-vs-viejo de t24 (2.94×)**: NO aislabl ya — el contenedor v4 fue removido
  por el rollback (docker ps -a: solo el searcher actual; declarado) y sus logs murieron con él.
  El binario v4 sólo añade una tabla static NO consumida (+134 líneas, coexistencia) ⇒ sin
  mecanismo conocido para +1s. La mezcla binario-vs-recreación de t24 queda SIN AISLAR (fail-honest).

## 2. MITIGACIÓN DISEÑADA (contrato (2): A01 prefilter + short-circuit) — punto de inserción exacto

- **Dónde**: loop de eval por-tx en `backend/searcher-rs/src/cartridge_boot.rs` (eventos
  `cartridge.active_eval_enter` :1503 → `cartridge.active_eval_summary` :2474; pool_data Map se
  construye :406) invocando `cartridge/runner.rs:354 call_fn("evaluate_opportunity")`.
- **(i) A01 MARGINAL-PREFILTER** (gate del HOST, antes de invocar el cartucho): computar
  Σ−ln((1−fee)·rate) del ciclo del pool_data; si ≥0 (sin margen) ⇒ SKIP del cartucho con reason
  nueva `prefilter_marginal_negativo` registrada en el summary (R8: observable, no silencioso).
  **Fail-open**: sin rates/fees computables NO se salta (se evalúa como hoy) — no mata los 174
  DATA_GAP por las malas. Impacto esperado: proporcional a ciclos con margen≤0 (hoy 100% de los
  evaluados son negative; el xlsx MEJORAS_MS estima −10..−30ms/tick — medir μ real al implementar).
- **(ii) SHORT-CIRCUIT** al primer fallo: DOS variantes con trade-off semántico distinto —
  (a) por-cartucho dentro del rhai (tocaría los 264 .rhai ⇒ NO recomendado); (b) por-batch del
  host: al primer `dispatch_needs_route_data` de un tx, los restantes pertinentes sin route data
  comparten la causa ⇒ cortar y registrar summary con reason agrupada (menos 269→~95 evals/tx,
  PERO cambia la granularidad del summary ⇒ REQUIERE visto bueno del operador sobre el contrato
  de telemetría). Diseño listo; elección de variante = decisión de implementación con el operador.
- **Disciplina (3) del contrato**: esto es cambio a la eval ⇒ release NUEVO + drill con imagen
  real + swap + V7 COMPLETO (sin excepciones). NO se implementó bajo este box (vencido a los 90
  min con el perfil consumiendo el tiempo): implementar apurado un cambio al hot path con EXC-001
  abierto habría sido el atajo que la propia tarea prohíbe.

## 3. HOJA DE RUTA para el siguiente box (lista para ejecutar)

1. Operador resuelve EXC-001 (publicar con p50 alto vs mitigar) — el perfil de HOY da el insumo:
   la cola es la eval (0 positivos, DATA_GAP dominante), no un misterio.
2. Implementar (i)+(ii-b) con tests unitarios (gate fail-open + summary reason nueva) en estación.
3. Release nuevo R-<ts2> → build VPS RULE 03 (patrón t20: flock+nohup+poll+df-watchdog) → digest
   a A1 + re-hash → drill imagen REAL → swap único → V7 (protocolo completo, sin excepciones).
4. μ de la mitigación medido contra el baseline n-grande de t24 (simétrico, dedup, misma fuente).

## Evidencia (t25-evidence/)

timebox.txt · profile_probe1/2/3.txt · viejo_eval_events.log (8360 eventos) · feed_now1.json (n=50)
· feed_now2.json (n=41) · este reporte. Cero bytes escritos en el VPS salvo /tmp/t25-eval.log
(limpiable por qa; declarado).
