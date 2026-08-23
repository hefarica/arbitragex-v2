# A.5 Paper-Shadow Audit — 2026-08-23

> Evidence dossier for the `a5_paper_shadow_not_executed` blocker decision (critical, blocks A.5+LIVE).
> Required action under doctrine: "run paper-shadow continuously and **audit the daily ledger** for
> revert rate, latency, sim error rate." This is that audit. Read-only queries against VPS Postgres/Redis.

## Anchors

| Anchor | Value | Source |
|---|---|---|
| A.4 PASS | 2026-08-20T01:34:57Z | `gate_c_validation` row `a4_fork_validation` / `a4_fork_validation_20260820T013304Z.log` |
| Ledger span | 2026-07-05 → ahora (~49 días) | `paper_trade_runs` MIN/MAX(created_at), 591,753 rows |
| Paper-mode authority | `inferred` (env) — sin clave Redis explícita | `arbx:papermode*` scan = vacío |

## 1. Continuidad — ✅ PASS

- Máximo gap entre corridas consecutivas post-A.4: **1.81h** (mercado bursty, no parálisis).
- Volumen diario post-A.4: 1,482 / 655 / 4,458 / 11,526 (08-20→08-23). Sin días vacíos.
- Detections pipeline vivo: heartbeat searcher con deltas 6-57/min; `opportunities` 81 filas/3min.

## 2. Sim error rate — ✅ PASS (despreciable)

| día | detections | sim_err |
|---|---|---|
| 08-20 | 66,832 | 11 |
| 08-21 | 50,904 | 7 |
| 08-22 | 29,243 | 13 |
| 08-23 | 20,562 | 0 |

≤0.04% diario. El breaker de G-SIM-1 no ha vuelto a disparar.

## 3. Revert rate — ⚠ DOMINADA POR BUG DE GATE (fix en camino)

`TokenNotAllowed:0x…` explotó desde 2026-08-22T02:00Z y hoy es **97% del ledger** (11,256/11,526):

| token | filas | veredicto |
|---|---|---|
| `0x3235…a20` (AGLD) | 14,784 | meta Redis EXISTE (`{"symbol":"AGLD"}`); cartridge path nunca lo consultaba → dirección cruda vs allowlist de símbolos → **rechazo estructural** (CARTRIDGE-GATE-ADDR-01, PR #449) |
| `0x1f98…f984` (UNI) | 280 | mismo bug, misses transitorios del meta cache |
| `0x5149…86ca` (LINK) | 28 | ídem |
| `0xa0b8…eb48` (USDC) | 2 | ídem |

Distribución: **100% strategies `mev_01_*`** (cartuchos, 545 uniformes por cartucho) — el runtime
cartridge rechazaba TODO candidate en el gate de tokens. `flashloan_arb` (905/2d) pasa — su path ya
entrega símbolos. Los 4 tokens están `is_active=true` en `tokens` (tier-a del checklist ✓) y score 95
(tier-b ✓) — el checklist NO era el emisor; era el config-gate del spine.

Post-fix esperado: `TokenNotAllowed:AGLD` (AGLD NO está en la allowlist de 22 símbolos del operador —
rechazo limpio con señal accionable) y evaluación spine real de rutas WETH/USDC. **Decisión de
operador pendiente**: ¿agregar AGLD a `allowed_token_symbols` o dejarlo fuera del scope de mercado?

## 4. Latencia — ❌ NO MEDIDA (gap estructural)

`execution_time_ms`: **0 de 591,753 filas**. Ningún writer lo puebla (relays-client `insert_paper_trade_run`
ni el TS Shadow Archiver). La doctrina exige auditar latencia y hoy es incalculable desde el ledger.
→ Follow-up: **LATLED-01** (poblar execution_time_ms en los writers con el elapsed de sim).

## 5. Cadena evidencia — ⚠ PARCIAL

`actual_profit_usd` resuelto (drift_tracker Stage 2b): **0 filas**. La cadena
detection→sim→evidence no cierra la pata de resolución post-hoc. La evidencia de sim vive en
gate_c_metrics/evidenced (programa G-SIM-1), no en el ledger.

## Reloj A.5 — dos lecturas honestas

- **Post-A.4 estricta**: "run paper-shadow continuously **after A.4 PASSES**" → earliest **2026-08-27T01:34Z** (4 días más).
- **Acumulación total** (semántica G-PAP-1: MIN(detected_at)): ≫7d ya cumplido (49 días).

## ⚖️ DECISIÓN DEL OPERADOR — 2026-08-23

> "Hay suficientes días de evaluación, recorta los días, no podemos esperar más y resolver todo."

**Lectura adoptada: ACUMULACIÓN TOTAL.** Los 49 días continuos de ledger (591,753 corridas, gap máx
1.81h, sim-error ≤0.04%) satisfacen el umbral doctrinal ≥7d — que es exactamente cómo el verificador
canónico G-PAP-1 lo computa (desde MIN(detected_at), no desde A.4). El flip del blocker estático
`a5_paper_shadow_not_executed` se ejecuta el mismo día con este expediente como evidencia
(precedente: A.4, resuelto 2026-08-20 con gate_c_validation).

**No es un blind-flip**: la máquina de estados dinámica `a5_state` (GET /api/v1/scoring/status)
sigue rastreando la calibración de priors por separado (hoy `PAPER_SHADOW_WARMING`;
`scored_opportunities`=0, `bayesian_priors`=0 — avanza por datos reales, no por este flip). Los
follow-ups LATLED-01 y CARTRIDGE-GATE-ADDR-01 quedan registrados como deuda de evidencia, no como
blockers de fase.

## Bloqueos derivados descubiertos en esta auditoría

| ID | Hallazgo | Estado |
|---|---|---|
| PAPKEY-01 | G-PAP-1 insatisfacible: reader leía `arbx:papermode:chain:<id>`, writer escribe `arbx:papermode:<id>` | **PR #448 MERGED** — falta deploy + flip explícito del operador (dirección segura: paper ON) |
| CARTRIDGE-GATE-ADDR-01 | Cartridge runtime 100% rechazado en gate de tokens (addr vs símbolo) | **PR #449 en CI** |
| LATLED-01 | `execution_time_ms` nunca poblado — latencia inauditable | Pendiente de PR |
| AGLD-DECISION | ¿AGLD dentro del scope de mercado del operador? | Pendiente del operador |
