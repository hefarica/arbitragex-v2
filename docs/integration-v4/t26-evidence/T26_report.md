# T26 — H1+H3+H4 re-slice por costo + veredicto de diseño + KNW-001

Attempt 1 ecf236a1 · fuente: searcher_full.log (123.568 líneas traccionadas del VIEJO dfd71012e1dc,
vida completa). Parseo reproducible; pares enter→summary n=549.

## H1 — Σtiempo por causa: NO-IDENTIFICABLE (con banda y estimación declarada)

- OLS ecológico tiempo_tx ~ reasons(tx) sobre el bulk: **colinealidad perfecta** — la composición
  de reasons es casi constante entre tx (pertinent=269 fijo, reasons ~isomorfas) ⇒ X'X singular,
  coeficientes absurdos (β 1e16, Σtiempo 311% del bulk — matemáticamente inválido, evidencia
  conservada en h1_h3_results.json). El log NO trae timing por cartucho/causa (inventario de
  campos ms: solo rpc/pool_sync/route_discovery; instrumento ausente).
- **Banda honesta**: Σtiempo(DATA_GAP) ∈ [0%, 31.7%] del tiempo total (31.7% = TODO el bulk).
- **Estimación bajo supuesto uniforme** (cada eval de cartucho ~10.7ms = 2891/269; DATA_GAP-family
  = 65-66% de pertinentes): **≈21% del TOTAL** — SUPUESTO declarado, no medición.
- **Umbral del operador (≥30%): NO CERTIFICADO.**

## H3 — DOS poblaciones confirmadas (el hallazgo central del re-slice)

| población | n tx | % tx | Σtiempo | % tiempo | forma |
|---|---|---|---|---|---|
| bulk (≤10s) | 530 | 96.5% | 1350.9s | **31.7%** | p50 2891 / p90 3207 ms (apretado) |
| outliers (>10s) | 19 | 3.46% | 2904.1s | **68.3%** | 15.7→290.9s; p99 global 242.7s |

Correlatos de los outliers: **bursts de instante** (clústeres 23:20:39 ×6, 23:23:28 ×3, gaps 0.0s),
**re-entrada masiva** (hasta 124 enters del MISMO tx; media 3.8), y **misma mezcla de reasons que
el bulk** (pertinent=269 en todos) ⇒ la colada NO es semántica: es bloqueo/retry/burst.
**El p99 del pipeline lo maneja esta población — optimizar el bulk deja el p99 intacto.**

## H4 — reconciliación del delta

**GOBIERNA 2.94×** (t24: p99 3680→10819ms, medición simétrica n-grande, 13min/lado, n=214/247,
dedup, misma fuente pipeline_latency_ms). **2.38× desacreditada** (t21 w180: ventanas n=25-49
anecdóticas; t24 capa 1 lo declaró ruido: baseline atípicamente limpio cuyo rango histórico ya
incluía p99=5965 con 44%>3000ms esa mañana). Baseline para cualquier μ futuro de mitigación:
**PRE 3680 / POST 10819 (t24)**.

## PASO 4 — VEREDICTO DE DISEÑO

Σtiempo(DATA_GAP) ≥ 30% **NO certificado** (no-identificable; mejor estimación ~21% ⇒ por la letra
del orden firmado: **el prefilter marginal alcanza y KNW-001 SE REABRE para rediseñar ANTES de
implementar**). Y H3 obliga al rediseño a incluir **control de outliers** — cap de re-entrada por
tx + timeout por eval — porque concentra el 68.3% del tiempo y es el driver del p99: sin eso se
optimiza el 32% y se declara victoria sobre el 68% (advertencia literal del operador).

## PASO 5 — KNW-001 (campos EXACTOS del operador; registrado en A1 con re-hash en cadena)

- estado producción ACTUAL: binario previo dfd71012e1dc (VIEJO post-rollback, estable 7/7, PAPER ON).
- métrica: eval enter→summary (cartridge.active_eval_enter → cartridge.active_eval_summary).
- p90=3207ms, max=290.9s (291s — actualizado con n=549 del log completo; el 111.7s de t25 era
  sub-muestra), n=549 **DECLARADO CHICO**; bulk p50 2891ms.
- causa dominante: DATA_GAP 65-66% por INCIDENCIA; % TIEMPO: NO-IDENTIFICABLE (banda 0-31.7%,
  estimación uniforme ~21%) — instrumento por-cartucho ausente.
- driver real del p99: población outlier 3.46% tx / 68.3% tiempo (bloqueo+re-entrada ≤124 enters).
- cierre: mitigación A01 (prefilter marginal, fail-open) + fail-fast-classify SÓLO si el
  instrumento nuevo certifica Σtiempo(DATA_GAP)≥30% + control de outliers (cap re-entrada +
  timeout) en R-<ts2>.
- **reapertura: si Σtiempo(DATA_GAP) < 30% ⇒ REDISEÑAR antes de implementar** (campo protectivo
  del operador; hoy NO está certificado ⇒ KNW-001 NACIÓ en estado de reapertura/rediseño).

## Evidencia

searcher_full.log · t26_results.json (análisis final) · h1_h3_results.json (OLS inválido conservado
como evidencia del no-identificable). Comandos: tracción binaria cmd /c + parseo Python reproducible.
