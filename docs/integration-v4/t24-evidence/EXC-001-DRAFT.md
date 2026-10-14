# EXC-001 — DRAFT (NO FIRMADO — la evidencia EXCEDE la delegación delineada)

> Productor: qa/t24 (α9 VIGIL) · 2026-10-13T20:2xZ · Este documento PREPARA la excepción con los
> campos EXACTOS del operador PERO con los valores MEDIDOS n-grande, que superan los límites de la
> delegación ('umbral_excepcionado 2.4×'; reapertura 'p50>1.2×'). **La firma exigida (capitán α11 por
> delegación del operador, voto (b) acotado) NO PROCEDE sin nueva decisión del operador**: los números
> delineados (observado 6416, ratio 2.382 ≤ 2.4) fueron superados por la medición simétrica.

| campo | valor delineado (orden operador) | valor MEDIDO n-grande (t24) |
|---|---|---|
| artefacto | R-20261013T1545Z digest a9b3ef50… | R-20261013T1545Z digest a9b3ef50… (igual) |
| métrica | p99 pipeline_latency_ms | p99 pipeline_latency_ms (misma unidad/fuente ✓) |
| observado | 6416 (n=40, ventana W2) | **10819 (n=247, 13min)** |
| baseline | 2693 (n=33, ventana V0) | **3680 (n=214, 13min, binario previo)** |
| ratio | 2.382 | **2.94** (p99) · **1.95 (p50: 2015 vs 1035)** · %>2000ms 22.9→50.6% |
| umbral_vigente | 1.2× | 1.2× (recalibrado sobre baseline n-grande: 4416) |
| umbral_excepcionado | 2.4× | **2.4× SUPERADO (2.94)** |
| alcance | PAPER ON ¬capital | PAPER ON ¬capital (igual — sin capital en riesgo) |
| evidencia | w180 2 ventanas | t24-evidence: pre_latencias.json (n=214) + post_latencias.json (n=247) + swap/v8 salidas |
| riesgo residual | cobertura subestimada en cola | cuerpo ENTERO degradado (p50 1.95×) — no solo cola |
| reapertura | flip-live ∨ p99>3× ∨ p50>1.2× | **p50>1.2× YA ACTIVO en la medición (1.95×)** |
| firma | capitán α11 por delegación | **PENDIENTE — excede la delegación ⇒ decisión directa del operador** |

## Veredicto forense final (t24)

1. El "2.382" de t21 era ruido-mezclado (ventanas anecdóticas n=25-49; la baseline 2693 era una
   ventana atípicamente limpia del binario previo, cuyo rango histórico ya incluía p99=5965/44%>3s).
2. La medición simétrica n-grande (13min/lado, dedup, misma fuente) CONFIRMA regresión **REAL y
   mayor**: p50 1.95×, p95 1.43×, p99 2.94×, %>2000ms duplicado. El binario v4 degrada la
   distribución COMPLETA del pipeline_latency_ms, no solo la cola.
3. Consecuencia disciplinar: guard recalibrado (umbral 4416/12.6%) ⇒ **DISPARÓ** ⇒ V8 rollback
   ejecutado (segundo rollback del release; producción estable en dfd71012e1dc).
4. El swap de re-publicación del contrato (C) se ejecutó (2.61s) y fue revertido por el guard con
   evidencia n-grande: **P9 permanece 0** (la pasada no fue limpia).
5. Qué hace falta para publicar: (i) mitigación de la degradación (profilng de la eval v4
   por-candidato; batching; o poda de eval en el hot path), O (ii) decisión EXPLÍCITA del operador
   con ESTA evidencia (p50 1.95× en PAPER) firmando una excepción nueva con límites propios.
   La delegación delineada no cubre 2.94×/p50-1.95×.
