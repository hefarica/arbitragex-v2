# E1 — Forense de datos PG (stall 01:16:25 UTC)
> Agente: data-forensics · Ventana consultada: 2026-09-16 00:30–02:10 UTC · Solo SELECT (psql) · Salida completa en transcript de sesión (task a56b7bf298dfa1f3d)

## Timeline por minuto (00:40–02:10)
- 00:40–01:04: 480/min EXACTOS (rate-capped, cadencia 8/s)
- 01:05: 551 (bump = única reacción visible al UPDATE); 01:06: 409 (dip); 01:07–01:15: 480/min (tasa plena)
- 01:16: 288 (parcial; última fila 01:16:25.399403)
- 01:17–02:10: CERO ABSOLUTO (54 min, ni un minuto > 0). **Patrón: SWITCH duro, no decaimiento.**
- max(detected_at)=01:16:25.399403+00; count últimos 10 min (02:04Z) = 0. No reanudó.
- Último bloque procesado: 25986688 (01:16:25); jamás llegó fila del 25986689. Ritmo previo ~12 s/bloque.

## Composición pre-stop (00:30–01:16:26; total 22,259 — 100% status=rejected)
| rejection_reason | n | % |
|---|---|---|
| TokenNotAllowed:0x06450dee…6fb8 (XEN) | 13,048 | 58.6% |
| v3_quote_unavailable | 8,288 | 37.2% |
| non_positive_profit | 919 | 4.1% |
| otros (single_pool/spot_product) | 4 | ~0% |

## Dependencia estructural (EL DATO CLAVE)
- Cruce token_in/out=XEN × reason: v3_quote_unavailable 8,262 XEN / **26 no-XEN**; non_positive_profit 918/1; otros 0/4.
- **El "42% no-XEN" era 99.66% XEN** (rutas XEN/WETH, pair 06450d…/c02aaa…). Flujo genuinamente independiente: **31 filas en 46.4 min (0.14%)** — y en CERO desde 00:38 (38 min ANTES del UPDATE).
- route_metadata: 22,228/22,228 filas XEN referencian el token XEN y ≥1 de los 5 pools desactivados (direcciones confirmadas en muestra). Post-disable (01:10–01:16:26): 3,028/3,028 (100%) seguían ruteando los 5 pools → el disable NO estaba aplicado en detección hasta ~01:16.
- Fuentes: 28 cartridges mev_01/02 × 466 filas c/u (= 13,048 TokenNotAllowed) + dex_arb 9,209 + triangular 2. ~29 fuentes, todas convergían en XEN.

## Simulations en paralelo
- 440/min constantes (misma forma); 01:16 parcial 264; CERO después. max(simulated_at)=01:16:31.322 (drain ~6 s tras última detección).
- Join sim→opp: 20,392/20,416 (99.9%) simulaban opportunities con token_in=XEN, todas passed=false. **Consumidor sano sin input.**

## Veredicto E1
1. **H2 FUERTE**: el flujo no-XEN no era independiente; XEN era motor del 99.86% del stream. Quitar XEN vacía el candidate-set a ~0 POR DISEÑO.
2. Caveat: stop=switch duro 11 min tras el disable → consistente con propagación tardía (ciclo de reload del universo); la alternativa (crash infra) no distinguible desde PG.
3. Sim-ctl: freeze simultáneo con drain limpio — no fallo paralelo.

Caveats técnicos: matching de pools por prefijos ILIKE validado contra direcciones completas en muestra; 31 filas no-XEN con 0 matches de prefijo. Queries acotadas por detected_at/simulated_at (índices presentes).
