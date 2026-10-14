# ADM Registry — Track 𝒯 edge/SSE (t4) · familia G+C §12

> Productor: t4/edge-dev, attempt f4e6b4d2-7d98-4a9d-b823-b27223912dca (2026-10-13).
> Protocolo: π(t) → μ(t;θ)=J(θ⊕π(t))−J(θ) vs x_ref congelada → λ(t) → admisión 𝒜(θ).
> Fail-honest: toda cifra cita su artefacto; lo no medido se declara, jamás se rellena.

## 0. Insumos congelados (no re-evaluados aquí)

| Insumo | Valor | Fuente |
| --- | --- | --- |
| x_ref | línea base ℳ del sistema vivo (ventana 2026-10-13 08:21–08:29 UTC) | `docs/integration-v4/releases/R-20261013T085048Z/x_ref.json` (sha256 `a6d521ea…c26539`, ℛ.tsv:7) |
| δ_abs | 0.005 — **NO aplica a x₂/x₃** ("x2/x3 are LATENCY distributions (unit ms), not proportions: delta_abs 0.005 does not gate them") | x_ref.json `delta_abs.derivation` |
| 𝒞(θ) | canónica = 1 (la lectura literal=0 fue errata de transcripción, escalado [A] al capitán) | `releases/R-20261013T085048Z/C_THETA_verdict.json:67` |
| x₂ baseline | p50=611.6 ms · **mean=641.6 ms** · p99=997.6 ms · min=476.0 (N=40, RTT completo DNS+TLS+respuesta, conexion nueva por probe) | x_ref.json `x2_latencia_http_borde` |
| handshake wire | dns=0.111 s + tls=0.279 s = h=390 ms | x_ref.json `x2_latencia_http_borde.handshake_referencia` |
| x₃ baseline | p50=1843→2713 ms entre ventanas (métrica productor `pipeline_latency_ms` de opportunities) | x_ref.json `x3_pipeline_latency_ms` |

## 1. Definiciones DECLARADAS antes de medir (anti-hindsight)

- **δ (candidatos con coordenadas en ms)**: δ = max(0.02·|J(θ)|, δ_abs) con δ_abs no-gating para ms ⇒ **δ = 0.02·|J(θ)|**.
- **J(θ)|π para G-1** := costo por actualización de precio entregada al cliente edge (ms/update). Baseline REST (único mecanismo de entrega de precios por REST/edge hoy): cada refresh = 1 RTT completo ⇒ **J(θ) = 641.6 ms/update** (x_ref x₂ mean, N=40). Declarado ANTES de construir.
- **J(θ⊕π(t))|π para G-1** := m1 + w, donde
  - **m1** = latencia publish→cliente a través de la cadena real (ruta SSE api-server + edge dev-local `proxyStream`), medida en staging local nowire con artefacto;
  - **w** = componente de red pública del push SSE sobre conexión persistente, **NO medible localmente** (ssh arbx exit=255, x_ref.json instruments) ⇒ declarado por COTA: w ∈ [0, (x₂max − h)/2] = [0, (997.6−390)/2] = [0, 303.8] ms (una vía de transferencia de respuesta como máximo; sin DNS/TLS por conexión persistente).
- **Prueba de admisión G-1 (peor caso)**: μ_worst = 641.6 − (m1 + 303.8) ≥ δ = 0.02×641.6 = 12.832 ms ⇒ ADMITIDO. Si μ_worst < δ ⇒ NO admitido y revertir implementación.
- **m2** (cross-check de sanidad, NO parte de la fórmula): RTT REST por la MISMA cadena local (debe ser del mismo orden que m1 + overhead de request; un m2 gigante indicaría cadena local patológica y degradaría la confianza en m1).

## 2. Candidatos — evaluación π/λ declarada

| id | candidato | π(t) coordenadas x(θ) | λ(t) | 𝒞(θ⊕π) |
| --- | --- | --- | --- | --- |
| G-1 | SSE fallback `GET /api/prices/stream` (api-server upstream + edge worker + dev-local, paridad ARBX-V-014) | **x₂** (costo/entrega por update). x₃ DESCARTADA: x_ref x₃ = `pipeline_latency_ms` del productor de opportunities — interna al pipeline, el transporte no la altera | P (referencia v4 propia del operador, ZIPs ATT; SSE/EventSource = estándar web, sin código de terceros) | =𝒞(θ)=1 (cambio de transporte, SSOT intacto) |
| G-2 | `/health` con `release_sha` (sirve a Λ) | **∅** — ninguna coordenada x₁..x₆ de x_ref se mueve (deploy-verificación, no data plane) ⇒ coordenadas descartadas | P | n/a (sin π) |
| G-3 | 404 honesto con cuerpo JSON | **∅** — error rate baseline 0.0% (n=20); un cuerpo JSON en 404 no mueve x₁..x₆ | P | n/a (sin π) |
| G-4 | batching eth_call/Multicall3 (↓ round-trips por slot) | **{x₃}** — PERO π(t) ∈ θ YA: `amm_math.rs::v3_quote_exact_in_multicall` + `v3_quote_provider.rs` (B1 batching, chunk planner) + `state_projector.rs` (⌈N/batch⌉) + `triangular_adapter.rs` (1 multicall backfill) + `liquidation_worker.rs` — grep 250 matches (evidence: `ADM_G4_multicall_grep.txt`) | MIT (mds1/Multicall3, ya portado y citado en amm_math.rs:8-9) | n/a (μ incremental de lo ya presente) |

## 3. Veredictos

| ADM-n | t | λ | π(t) | μ | comando/artefacto | veredicto | fecha |
| --- | --- | --- | --- | --- | --- | --- | --- |
| ADM-1 | G-1 SSE prices/stream | P | x₂ | **μ_worst = 337.543 ms** (= 641.6 − (m1 0.257 + w_max 303.8)) ≥ δ=12.832 ⇒ margen 26.3× | `npx tsx ../../docs/integration-v4/t4-evidence/measure_mu.ts` (desde backend/api-server) → `measure_mu_output.json` (sha C2280DB3…80B876) | **1 (ADMITIDO — implementado)** | 2026-10-13 |
| ADM-2 | G-2 /health release_sha | P | ∅ | μ=0 (π vacía ⇒ sin coordenadas que mover) | π(t)=∅ por inspección de x_ref.json coordenadas | **0 (no implementar)** — sirve a Λ (t6 puede admitirlo por vía propia, no por μ) | 2026-10-13 |
| ADM-3 | G-3 404 honesto JSON | P | ∅ | μ=0 (π vacía) | π(t)=∅ por inspección; patrón honest-502/404 ya pervasivo en ambos edges | **0 (no implementar)** | 2026-10-13 |
| ADM-4 | G-4 Multicall3 batching | MIT | {x₃} | μ incremental = 0 (π(t)∈θ ya implementado) | `grep aggregate3|Multicall3|multicall backend/searcher-rs/src` → 250 matches; `ADM_G4_multicall_grep.txt` | **0 (ya presente — no re-implementar)** | 2026-10-13 |

## 4. Medición G-1 (m1/m2) — COMPLETADA 2026-10-13T14:51:31Z

**Comando**: `npx tsx ../../docs/integration-v4/t4-evidence/measure_mu.ts` ejecutado desde `backend/api-server` (node v24.12.0, staging local nowire). Artefacto: `measure_mu_output.json` (sha256 `C2280DB364FF881E537C5F13BB64B9261F759055C578EA5038A77E713280B876`).

| medición | n | mean | p50 | max | min | unidad |
| --- | --- | --- | --- | --- | --- | --- |
| **m1** SSE delivery (broadcastPriceUpdate → ruta SSE api-server → proxyStream dev-local → reader cliente) | 20 | **0.257** | 0.227 | 0.668 | 0.107 | ms |
| m2 REST refresh (loopback sanity, mismo proceso api-server) | 20 | 1.188 | 0.656 | 8.138 | 0.498 | ms |

Cálculo (fórmulas de §1, declaradas ANTES de medir):
- J(θ) = 641.6 ms/update (x_ref x₂ mean, congelada t9).
- J(θ⊕π) = m1 + w = 0.257 + w, w ∈ [0, 303.8] ms (cota declarada §1).
- **μ_worst = 641.6 − (0.257 + 303.8) = 337.543 ms**; δ = 0.02 × 641.6 = 12.832 ms.
- μ_worst / δ = 26.3× ⇒ **ADMITIDO bajo la cota PESIMISTA de w** (ni siquiera el peor caso rompe la admisión).
- Exclusión declarada: hop Redis PUBLISH→PSUBSCRIBE (sin Redis local vivo, RULE 01; O(1) sub-ms en la red docker del VPS) — excluido de m1, no estimado dentro del número.
- Sanity: m2 (1.188 ms loopback REST) >> m1 (0.257 ms) — el push SSE es ~4.6× más barato que un refresh REST AUN EN LOOPBACK (sin DNS/TLS); coherente con la dirección del efecto en el wire público.

## 5. Cadena de custodia del seam prices-stream.ts (ARBX-V-014 §α11)

| estado | sha256 | productor |
| --- | --- | --- |
| main (base 5f8ea3d) | `DA8564B6DA9A7A9A6A30016DAE8215159DB9D0EF3D89D73582C6A6379E5532CE` | (pre-t4, medido por t4 al iniciar) |
| + guard t5 (+6/−0 @ :104-110) | `3C26994AEDBC80C9E28DFEC2E6F40916872D8118AAE96659D2A01683E408A8B5` | t5/frontend-dev (declarado por capitán, verificado por t4 antes de editar) |
| + 2 keywords `export` (nextSeq/buildSnapshot) | `D019391608D8AC7DC363A8CE69454190D54EC674B33ED20E2C0690E026A665F1` | **t4-edge (cambio aditivo export, verificador→t17)** |

Fila ℛ (addendum t4, ver `R_addendum.tsv`): productor=t4-edge, sha=D0193916…, verificador=t17/t7.
Delta conductual: CERO — `npx vitest run src/prices-stream.test.ts` (backend/api-server) = 4/4 PASSED con el archivo editado (evidence: `ADM_tests_prices_stream.txt`).
