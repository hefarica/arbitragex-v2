# WO-02 — FIX (gang Omniscience, ronda 1): drift doc §5.3 cerrado (CROSS G3)

- **Work-order:** WO-02 · **Tipo:** FIX local (docs-only) — adjudicación CROSS G3 (`WO-02-CROSS.md` §3: "agent-fixable, diff listo en WO-02-DESIGN §5.3; docs-only")
- **Charter del fix:** `docs/redis-schema/hot-path-v2.md:27-36` seguía documentando un contrato wire que NUNCA existió — 4 claims falsos vs el XADD vivo de `emit_simulated`. Aplicar el diff del diseño §5.3 verbatim con marcador WO-02 y actualizar la referencia en el board.
- **Reglas respetadas:** 0 requests HTTP al dominio público (0/5), 0 SSH/VPS, 0 git local (cero commit/push — protocolo operador 2026-08-23), 0 toques a código/VPS/Redis/PG. Edición local docs + verificación de contenido.
- **Fecha:** 2026-09-07 (ronda 1 del gang).

---

## VEREDICTO: **FIXED_VERIFIED** — los 4 claims falsos eliminados; la sección ahora documenta el contrato wire vivo de `arbx:hot:simulated`, verificado campo a campo contra el XADD en código.

## 1. El gap (por qué era falso, en 4 conteos)

Doc pre-fix (`docs/redis-schema/hot-path-v2.md:29-35`, HEAD `79caae9c`):

| # | Claim falso del doc | Realidad del código |
|---|---|---|
| 1 | Producer "only for passed results" | `emit_simulated` emite para TODA sim que corrió REVM: `status` = `passed \| failed` verbatim (`hot_path_emitter.rs:149` `if result.passed {"passed"} else {"failed"}`); el passed-only solo aplica al HSET `arbx:hot:sim:{id}` (`:183-199`) |
| 2 | Campo `sim_result` (JSON-encoded) | NO existe en el XADD — jamás existió en ninguna versión del emitter. Los campos reales: `hot_path_emitter.rs:159-178` |
| 3 | Campo `trace_hash` | NO existe en el XADD (el `trace_hash_sentinel` vive en el status de la oportunidad PG, no en el stream hot) |
| 4 | `net_profit_wei` = "Net Topological Yield in wei (after gas estimation)" | El campo transporta el delta GROSS `simulated_profit_token_in` (U256 → decimal string); el net-of-gas es decisión downstream (`hot_path_emitter.rs:33-35`, paper-executor net gate). "after gas estimation" era falso |
| (+) | Faltaban 5 campos vivos | `opportunity_id`, `chain_id`, `strategy_kind`, `token_pair`, `gas_price_wei` (`hot_path_emitter.rs:167-176`) — sin ellos el PaperExecutor haría `skip_incomplete` de toda entrada (defecto latente #2 del diseño, cerrado por WO-02) |

## 2. El fix aplicado

`docs/redis-schema/hot-path-v2.md` — diff del diseño §5.3 (`WO-02-DESIGN.md` §5.3) aplicado **verbatim**, más una línea de marcador de comentarios markdown (regla "diffs propios marcados con el ID del WO"):

- `git diff --stat`: **1 file changed, 16 insertions(+), 6 deletions(-)** — solo la sección `arbx:hot:simulated`.
- Marcador: `<!-- WO-02 (2026-09-06): section rewritten per WO-02-DESIGN §5.3 ... verified against the live XADD ... -->` (línea inmediatamente antes del Producer).
- Producer nuevo: post-REVM, DESPUÉS del publish canónico `arbx:opps:detected`, passed\|failed verbatim, nunca para candidatos que fallaron pre-REVM.
- Fields: los 10 campos del XADD vivo (`id`, `opportunity_id`, `status`, `net_profit_wei` gross-decimal-string, `gas_used`, `gas_price_wei`, `chain_id`/`strategy_kind`/`token_pair`, `timestamp_ms`).
- Consumers (nuevo): `ws-emitter-g0` y `paper-executor-g0` con nota de dormancy.

Desviación única vs el diff del diseño: la línea-marcador HTML extra (ordenada por la regla dura de marcado del orquestador). Todo lo demás byte-a-byte el diff §5.3.

## 3. Verificación (RULE 00 — cada claim del doc nuevo traza a código)

1. **Contraste campo a campo** doc↔XADD (`hot_path_emitter.rs:153-180`): los 10 fields del XADD (`id:159`, `status:161`, `net_profit_wei:163`, `gas_used:165`, `gas_price_wei:167`, `opportunity_id:169`, `chain_id:171`, `strategy_kind:173`, `token_pair:175`, `timestamp_ms:177`) están todos documentados; el doc no lista ningún campo que no exista en el wire. **Nota honesta (R8): el brief del fix hablaba de un "11-field XADD"; el XADD vivo tiene 10 campos** — conteo verificado por lectura directa. No se fabricó un 11º campo para cuadrar el brief.
2. **Claims de Producer** verificadas contra el wiring WO-02 (ya APPROVE por VERIFY + reproducido por CROSS): post-publish `scanner.rs:2648-2669`, pre-REVM→None jamás emitido como failed (VERIFY §1.3), veredicto verbatim (VERIFY §3.6).
3. **Claims de Consumers** verificadas por grep propio esta ronda: grupo `ws-emitter-g0` (`websocket.ts:801` `HOT_OPPORTUNITIES_GROUP`), evento `opportunity:validated` (`websocket.ts:907`, `:976`); grupo `paper-executor-g0` (`paper/executor.ts:8`, `:22`), dormante por default `ARBX_PAPER_EXECUTOR_MODE` off (`index.ts:1904-1915`).
4. **Sin blast radius**: `git diff docs/redis-schema/hot-path-v2.md` = exactamente la sección objetivo; ninguna otra sección del doc tocada; ningún otro archivo tocado por este fix salvo el board (fila WO-02, ver §5).
5. **Gates**: docs-only markdown — no aplica tsc/vitest/cargo (ningún archivo de código tocado). La verificación es la de contenido §3.1-§3.4, declarada así fail-honest.

## 4. Re-clasificación de ownership (INFO-5 + APPLY §5.3)

- El APPLY pospuso el doc con owner "Operador / WO de docs" (`WO-02-APPLY.md:89`).
- El VERIFY repitió la clasificación: INFO-5 "dueño = operador/WO de docs" (`WO-02-VERIFY.md:81`).
- El CROSS lo corrigió: G3 = **agent-fixable** (docs-only, diff listo). Este fix confirma la corrección: es una edición local pura, sin gate de operador (no requiere deploy/flip/VPS — el doc describe código ya deployado en `a0bcf29d`). El INFO-5 queda re-clasificado.

## 5. Board

`GOAL-WORKORDERS.md` fila WO-02: añadido "**G3 CLOSED (gang fix ronda 1 — WO-02-FIX.md, 2026-09-07)**" con referencia a este reporte y la re-clasificación de ownership. Sin conflicto de claims: `docs/redis-schema/hot-path-v2.md` es file-claim exclusivo de WO-02 (G3); el board se actualiza solo en la fila WO-02.

## 6. Residuales declarados (observados, NO tocados — quirúrgico)

- **R-1**: la línea `**Purpose**` de la sección ("for opportunities that passed validation") quedó intacta — es línea de contexto del diff §5.3 (verbatim mandate). Ligeramente stale (el stream también lleva failed), pero el Producer nuevo la desmiente en la línea siguiente; ajustarla excede el mandato verbatim.
- **R-2**: la sección `arbx:hot:sim:{id}` (Hash) del mismo doc sigue describiendo "Full REVM trace summary / State diffs / Error logs" — el contenido real del HSET es el `SimulationResult` serializado (`passed/net_profit_wei/gas_used/gas_price_wei`, `hot_path_emitter.rs:185-190`). Fuera del scope del diff §5.3 (que solo cubría L27-36); queda como drift menor para un futuro WO-docs.
- **R-3**: G1/G2/G4/G5 del CROSS siguen abiertos (G1 operador-gated: modo v2 / flips R-0001-S4; G2 ya volcado al board por el propio CROSS; G4 operador-gated; G5 test-nit de 1 línea).

## 7. Presupuesto

0 requests HTTP al dominio público (0/5). 0 SSH. 0 git local (solo `git diff` de lectura). Escrituras: `docs/redis-schema/hot-path-v2.md`, `GOAL-WORKORDERS.md` (fila WO-02) y este reporte.

---

*WO-02 FIX — 2026-09-07, gang Omniscience ronda 1. Fail-honest: conteo real de campos = 10 (el brief decía 11); residuales R-1/R-2 declarados sin tocar; sin gates de código porque el cambio es markdown puro.*
