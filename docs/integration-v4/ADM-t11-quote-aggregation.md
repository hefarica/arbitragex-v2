# ADM-t11 — Track 𝒯-C (FAMILIA §12-C): cotización, agregación, multicall, sim en fork

> Task t11 · backend-dev · attempt `161254ac-f4a9-47d6-9fe2-e9e5226b54f8` · rama `feat/exp-20261013` · 2026-10-13.
> Disciplina: π(t), μ(t;θ) vs x_ref, λ(t), δ = max(0.02·|J|, δ_abs). **δ_abs = 0.005** (derivada por t9 del x_ref t16: x_ref.json `delta_abs`).
> Eje declarado de la familia: **↓round-trips/slot ∧ ↑rutas x₁**. Fail-honest: todo número cita su comando+salida literal (t11-evidence/).
> I1 respetado: CERO ediciones al hot path (searcher-rs intocado en t11 — verificado en V2 §6). P5: paper ON, sin flip, sin P&L, secretos fuera.

## 1. Registro ADM-n por candidato

| t | π(t) | μ incremental (vs θ) | μ_wire (vs x_ref t16) | λ(t) | \|J\| | δ | veredicto |
|---|---|---|---|---|---|---|---|
| **C-1** Multicall3 aggregate3 | Batch de staticcalls en 1 eth_call | **0 — π ∈ θ YA**: 152 grep matches propios (`C1_multicall_grep.txt`) + t4 ADM-4 (250 matches, `t4-evidence/ADM_G4_multicall_grep.txt`) + `v3_quote_provider.rs:63` DEFAULT_BATCH_SIZE=100 + `state_projector.rs:612` "⌈N/batch_size⌉" + tests de equivalencia B1 `amm_math.rs:1557/1628` | n/a (mecanismo ya en el wire del productor) | P | n/a | n/a | **0 — YA PRESENTE, no re-implementar** |
| **C-2** batch eth_call (JSON-RPC array / eth_callMany) | N eth_calls en 1 request HTTP | **NO COMPUTADO**: x_ref SIN observable RT/slot (instrumentos t16: solo HTTP edge x₂ y pipeline x₃ — `x_ref.json` instruments); θ ya colapsa N→⌈N/100⌉ vía C-1, el caso residual es <1 chunk por slot típico; sin productor TS (api-server NO cotiza on-chain) y I1 prohíbe cablear hot path ⇒ aún implementado sería μ_wire=0 `no wire producer` | **0 — sin seam productor** (§3) | P (eth_callMany soportado en publicnode: `C2_jsonrpc_batch_grep.txt` sonda -32602 de tipos, método existe) | — | — | **0 — μ no computado + sin productor; no implementar** (§D no aplica: no es falla, es medición cerrada) |
| **C-3** trace_callMany | N calls secuenciales-con-estado en 1 trace | **NO COMPUTADO — instrumento inaccesible**: `trace_callMany '[]'` → HTTP 403 "Archive requests require a personal token" (`C3_trace_callMany_probe.txt`) — el stack soberano gratuito lo REHUSA y §38 PROHÍBE pagar | n/a | R (requiere nodo tracing) | — | — | **0 — incompatible con stack soberano (403 + §38) + sin nodo propio (ssh exit 255, x_ref instruments)** |
| **C-4** state override (eth_call stateDiff) | what-if sin mutar estado | **0 en el eje declarado**: no reduce round-trips (mismo 1 RT por call); es HABILITADOR de escenarios sim, no un reductor | n/a | R | — | — | **0 — μ=0 en eje ↓RT (habilitador, no reductor)** |
| **C-5** split de routers | paralelizar cotización cross-router | **0 standalone**: paralelismo cambia latencia, NO el CONTEO de round-trips; eje ↑x₁ NO COMPUTABLE (x₁ base = 0 viables, `x_ref.json` x1_cobertura.viables=0 — mejora proporcional desde 0 indefinida); combinado con C-1 ya está realizado (los chunks cruzan routers) | n/a | R | — | — | **0 — μ=0 standalone; vía C-1 al combinarse** |
| **C-6** quoters staticcall | cotizar vía view-only | **0 — YA ES LÍNEA BASE**: `amm_math.rs:6-7` (QuoterV2 mainnet + docs) + `quoteExactInputSingle` :538 + doctrina §38 expresa (`C6_staticcall_grep.txt`) | n/a | P | n/a | n/a | **0 — YA ES LÍNEA BASE** |
| **C-7** sim en fork (anvil externo) | sim pre-trade sobre fork | **≈0 — equivalente YA en θ**: REVM in-process (`C7a_revm_ya_presente.txt`: Cargo.toml:39 G-SIM-1 multi-step, :75-76 `execute_multistep_revm`, candidate_simulation.rs, sim_orchestrator.rs, sim_encoder_pg.rs) — un fork externo es un SEGUNDO mecanismo de la misma función; además instrumento local BLOQUEADO: anvil.exe rechazado por AppControl ×2 intentos (`C7b_anvil_appcontrol.txt`), cast/forge sí corren | n/a | R | — | — | **0 — YA PRESENTE (revm) + instrumento anvil bloqueado por política** |

**Parada de admisión 𝒜(θ)=∅ para §12-C**: 7/7 candidatos con veredicto **0** — TRES por "ya presente en θ" (C-1 multicall3, C-6 staticcall, C-7 revm-sim: la familia ya fue construida por el repo en el hot path), DOS por instrumento inaccesible sin violar §38/I1 (C-3 tracing 403, C-7-anvil AppControl), DOS por eje/productor (C-2 sin seam TS + μ no computado, C-4/C-5 μ=0 en el eje declarado). **Cero implementaciones nuevas en t11 — y eso ES el veredicto**: re-implementar lo ya presente violaría I1/I2 y añadiría código sin productor (R10). ¬re-evaluar.

## 2. Evidencia por candidato (comando + salida literal)

| candidato | artefacto | contenido |
|---|---|---|
| instrumentos | `t11-evidence/RPC_soberano_probe.txt` | llamarpc 525 / **publicnode 26184346 @526ms** / drpc 26184346 @1265ms |
| C-1 | `t11-evidence/C1_multicall_grep.txt` | 152 matches + constantes batch=100 + ⌈N/b⌉ + tests B1 |
| C-2 | `t11-evidence/C2_jsonrpc_batch_grep.txt` | 0 matches batch JSON-RPC (todo es multicall EVM-level) + sonda eth_callMany existe |
| C-3 | `t11-evidence/C3_trace_callMany_probe.txt` | HTTP 403 token-personal literal |
| C-4/C-5 | (razonamiento de eje citando `x_ref.json` x1/x2/instruments — sin comando: π=∅/μ=0 por inspección del eje, patrón t4 ADM-2/ADM-3) | eje ↓RT: override no reduce conteo; split no reduce conteo; x₁=0 base |
| C-6 | `t11-evidence/C6_staticcall_grep.txt` | QuoterV2 mainnet + quoteExactInputSingle |
| C-7 | `t11-evidence/C7a_revm_ya_presente.txt` + `C7b_anvil_appcontrol.txt` | G-SIM-1/revm + 2 denegaciones AppControl literales |

## 3. μ_wire = 0 — sin seam productor (estado terminal legítimo, patrón t5 §3 ratificado)

La familia C cotiza on-chain SOLO desde el hot path (searcher-rs), que I1 excluye de t11.
El control-plane TS no cotiza on-chain (verificado: api-server/edge no tienen eth_call de
cotización — greps C-2). Por tanto CANDIDATIVO implementado hoy sería código sin productor
(R10: wire declarado-never-created). El seam correcto para una futura admisión de C-2
(quedará documentado para t14/t17): `backend/api-server/src/integration-v4/` con transport
batch → consumidor potencial: lecturas on-chain de control-plane que HOY no existen.

## 4. V1..V4 (release staging `releases/R-20261013T15-TC/`)

- **V1** `V1_hashes.txt` — sha256 recursivo del release (generado al cierre).
- **V2** `V2_static.txt` — verificación estática: re-grep de presencia (C-1/C-2/C-6), ints de las sondas, hot-path intocado (git diff paths), 𝒞(θ)=1 vigente (C_THETA_verdict canónica).
- **V3** `V3_build.txt` — build/test del árbol de t11 (que NO tocó código): tsc api-server + vitest integration-v4 (los artefactos de t3 siguen verdes — certifica que t11 fue evaluación pura sin rotura).
- **V4** `V4_dryrun.md` — dry-run declarado: NADA que promover (0 admitidos ⇒ 0 archivos de código); staging local como fuente de verdad (P1); sin commit/push/pull; sin deploy (F5/F6 fuera del plan, RG-01).

## 5. Cadena de custodia ℛ (filas t11, verificador → t17/t7)

Ver `releases/R-20261013T15-TC/ℛ-t11.tsv`: 8 filas (7 evidencias + 1 este registro ADM),
productor=t11-backend, verificador=t17-FALSATOR/t7-reviewer, estado=Pendiente.

> Regla de hash §11-bis: el sha256 de ESTE archivo debe quedar registrado dentro de A1 al regenerarlo (t6).
