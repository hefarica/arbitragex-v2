# T13 — Tabla de admisión 𝒯 · familias §12-H/I → ADM-n (para A7/t14)

- **Productor:** data-ingest/t13 (α-datos) · disciplina 𝒯: μ≥δ=max(0.02·|J|, δ_abs=0.005 t9) ∧ 𝒞 ∧ λ∈{P,R}
- **Sustratos congelados (con hash):** A = `qa-baseline/edge_opps_live.json` (26 items, sha `1FE9EFF0DEE0…` citado en x_ref.json) · B = A8 TESTS_ATOMICOS (sha `eb5b803c…`, tally en T8_output_r2/T13_sheets_dump) · C = hojas F01-F11 (T13_sheets_dump.json).
- **Ejes:** μ_x6 en USD (δ=358.3088 = 0.02·17915.4383) · μ_x5 en fracción (δ=0.02).
- **ℳ:** `python docs/integration-v4/t13-evidence/t13_admission_eval.py` → salida íntegra en `t13-evidence/T13_admission_eval_output.json`.

| ADM | Familia §12 | λ | μ MEDIDO | δ | ¿μ≥δ? | 𝒞 | VEREDICTO |
|---|---|---|---|---|---|---|---|
| **ADM-1** | H: sizing AMM forma cerrada (x·y=k) + cota Kelly (A14/A15) | P | **17915.4383 USD** (x6: 17/17 items, calibración exacta net(a₀)=net₀ ±1e-6) | 358.3088 | SÍ | intacto (SSOT 264/1436/hop_dist no tocado) | **ADMITIDO → implementado** (`adm/econ_sizing.py`) |
| ADM-2 | H: break-even gas↔profit | P | 0.0 (0 flips/26 con contabilidad honesta) | 0.02 | NO | — | **veredicto 0** (coherente con viables=0 de x_ref; ¬re-evaluar) |
| **ADM-3** | I: staleness oráculo (gate cuarentena) | P | **0.0769** (2/26 con T=2405ms=p95 medido) | 0.02 | SÍ | intacto | **ADMITIDO → implementado** (`adm/staleness_gate.py`) |
| **ADM-4** | I: ventanas no atómicas (gate de clase) | P | **0.113636** (30000/264000 NONATOMIC_BRIDGE_REQUIRED en A8) | 0.02 | SÍ | intacto | **ADMITIDO → implementado** (`adm/nonatomic_gate.py`) |
| ADM-5 | I: reorg (confirmaciones) | P | NO COMPUTABLE — semántica ILEGIBLE del campo (valores 1..2530 tipo block-height) | 0.02 | NO | — | **veredicto 0 por falta de señal medible** (no por ausencia de riesgo) |
| ADM-6 | H: funding/basis (A18), paridad opciones (A19), spreads LST (F07) | R | NO COMPUTABLE — sin datos de funding/opciones/LST en sustratos A/B/C | — | NO | — | **veredicto 0 DECLARADO** (λ=R admisible pero sin μ no hay admisión) |
| ADM-7 | H: optimización convexa multileg | P | NO COMPUTABLE — requiere liquidez por tick (ausente); parte V2 ya cubierta por ADM-1 (I2: un término) | — | NO | — | **veredicto 0 DECLARADO** |
| ADM-8 | I: leakage / selección adversa / cascadas liquidación | P | NO COMPUTABLE — sin mempool/positions lending en sustratos | — | NO | — | **veredicto 0 DECLARADO** |

## Modelo de μ de ADM-1 (declarado, no suavizado)

`net(a) = a·r − a²·s_c − fixed`, calibrado POR ITEM con el punto simulado medido: `s_c = slippage₀/a₀²` (V2: cuadrático EXACTO; V3: aproximado — 18/26 items tienen piernas V3, declarado), `fixed = costs_total − slippage₀ − lp₀`, `r = (net₀+slippage₀+fixed)/a₀` (tasa marginal efectiva; fees lp lineales absorbidos — exacto). Verificación: net(a₀)==net₀ en 17/17 items ±1e-6 (V4). Política: operar solo si net(a*)>0, a_final=min(a*, kelly_cap). En esta ventana: 0 trades tomados (todo edge ≤ piso) → μ = preservación de capital contrafactual sobre el x6 medido (−17915.44 → 0). **Lectura honesta:** el x6 de x_ref son candidatos RECHAZADOS simulados, no ledger ejecutado; μ es el contrafactual del sizing sobre ESA semántica, modelo-calibrado.

## V1..V4 del release (re-ejecutados, salidas pegadas)

- **V1** hashes: `V1_hashes.txt` (todo el árbol del release).
- **V2** estático: `V2_static.txt` — py_compile exit=0 (node CUELGA en host: declarado; port TS especificado en docstrings, no compilado).
- **V3** staging: `V3_tests.txt` — `python -m unittest discover -s adm -p 'test_*.py'` → **Ran 10 tests, OK, exit=0** (incluye replay del sustrato congelado).
- **V4** dry-run: `V4_dryrun.txt` — shadow replay SOLO-LECTURA sobre qa-baseline (sin escrituras a VPS/tree, sin commit/push — P1): calibración exacta 17/17, adm1 Δx6=+17915.4383, adm3 2/26 cuarentena. **DEGRADADO como en t9**: rsync ausente; NO se desplegó nada (V5+ exige decisión del orquestador).

## Cumplimiento I1/I2/I5/P1/P5

- **I1** (no tocar hot path fuera de t9): los 3 módulos son control-plane puro, NUEVOS, en `docs/integration-v4/releases/R-20261013T13HI-t13/adm/` — 0 bytes tocados de searcher-rs/hot path. ✓
- **I2** (un término por incremento): ADM-1=sizing+kelly (un término económico coherente), ADM-3=staleness, ADM-4=no-atomic — tres archivos disjuntos. ✓
- **I5** (¬𝒞 ⇒ revert): 𝒞 canónico intacto (condición 7 sigue escalada [A] del t9, sin cambio); no hubo que revertir. ✓
- **P1**: sin commit/push/pull; árbol compartido sin toques (solo docs/integration-v4/ propio). ✓
- **P5**: PAPER ON intacto (nada flippeado); MEV detection/searcher P&L intactos; secretos fuera de alcance. ✓

## Hallazgo adicional (resuelve la divergencia deriv_pos de t8)

Hoja **F04** del xlsx: familia D_ (derivados, fix DFAC D_=0.80) → **BASE=1982, IMP=3560, DELTA=1578**. Por lo tanto: `deriv_pos=3560` del MANIFEST = **IMP post-fix de la familia D_**, y el `1982` de INVARIANTES = **BASE pre-fix de D_** (etiquetado "OBSERVADO" — etiqueta confusa, no contradicción de datos). Mi 12000 de t8 = conteo de filas DELTA>0 en TESTS_ATOMICOS (métrica distinta). La "contradicción interna" declarada en t8 queda RESUELTA como diferencia de definición: 3560 (imp D_) vs 1982 (base D_) vs 12000 (filas delta>0) — tres métricas distintas, ninguna es error de datos. g1-g6 pueden consumir 3560 como "imp_pos familia D_ post-fix" con esta definición.
