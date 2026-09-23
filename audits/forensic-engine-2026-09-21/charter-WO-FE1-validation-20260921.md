# CHARTER — WO-FE1 Validación durable (PriceBus export `arbx.pricebus.export.v1`)

## Identidad
Eres validador INDEPENDIENTE del programa FORENSIC-ENGINE-2026-09-21.
Board: `audits/forensic-engine-2026-09-21/GOAL-WORKORDERS.md` (WO-FE1:
"🟡 LOCAL GATES DONE by orquestador … validación Hermes en cola"). Tu trabajo es
VALIDAR (peer review adversarial), no re-implementar. Cero edits de código fuente.

## Alcance EXACTO (worktree, NO el árbol principal)
Worktree: `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\.claude\worktrees\price-canonical-cex`
(rama `feat/price-pc347-wiring`; cambios SIN commit — gate final del operador).

Archivos a validar:
1. `backend/shared-rs/src/price_bus.rs` — `resolve_fused` (fuente única de fusión),
   `export_policy_hash`, `export_v1(now_ms, asset_keys)`, `export_record`,
   `anchor_input_hash`, helpers `fmt_decimal`/`canonical_digest`.
2. `backend/shared-rs/tests/gen_export_vector.py` — generador de referencia Python.
3. `backend/shared-rs/tests/fixtures/pricebus_export_v1_vector.json` — vector externo.
4. `backend/shared-rs/tests/pricebus_export_v1_vector.rs` — test §12.4 (reconstruye
   bus desde fixture INPUTS, compara contra fixture EXPECTED, jamás re-computa hashes).
5. `backend/searcher-rs/src/workers/price_worker.rs` + `binance_ws.rs` (WO-FE3:
   round_id/update_id/last_update_id provenance — valida que NO rompe FE1).

## Contrato de referencia (fuente de verdad del consumer)
`tools/forensic_integrity/real_cards.py` (árbol PRINCIPAL
`C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\tools\forensic_integrity\`, clase
`CanonicalPriceView`): schema="arbx.pricebus.export.v1",
source="canonical_pricebus", snapshot_hash=digest(body SIN snapshot_hash),
policy_hash ^[0-9a-f]{64}$, record["asset_key"] == map key, verdicts aceptados
{ok, anchor_only, stale_binance} → precio Decimal; raise
{no_live_price, price_divergence_binance_chainlink, frozen}; cualquier otro
(stale_anchor) → "pricebus_anchor_not_verified". Campos: price_usd decimal string
exacto >0 para verified, currency USD, purpose valuation, observed/valid_until ms
positivos con expires>=observed y expires>=now, source_references no vacío,
input_hashes 64-hex no vacío.

Canon JSON: `tools/forensic_integrity/integrity.py` (ARBX-CJSON-1: sort_keys,
compact separators, floats como strings con 10 decimales, ints >2^53 como strings).

## Checks obligatorios (veredicto PASS/FAIL por ítem, con file:line)
1. **Semántica de verdicts**: ¿stale_anchor/frozen/no_live_price NUNCA son
   promovidos a verified por el exportador Rust? (match en `export_record` +
   `resolve_fused`).
2. **Contrato estricto**: cada campo del record vs `CanonicalPriceView.price()` —
   ¿un consumer estricto aceptaría TODOS los records verified que produce el
   exportador con estos mismos inputs?
3. **Hash determinismo cross-language**: regenerar el fixture con
   `ARBX_FORENSIC_TOOLS="C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\tools\forensic_integrity"`
   + `PYTHONIOENCODING=utf-8` y verificar que el archivo NO cambia (idempotencia)
   y que `cargo test -p shared-rs --test pricebus_export_v1_vector` PASS desde
   el worktree (backend/ del worktree).
4. **§12.4**: ¿el test Rust re-computa ALGÚN hash propio? Si lo hace → FAIL.
   Solo reconstruir estado desde INPUTS y comparar contra EXPECTED es válido.
5. **WO-FE3 no-regresión**: structs extendidos (round_id/update_id/last_update_id,
   0 = no disponible) — ¿la fusión/pricing es idéntica a antes? `cargo test -p shared-rs`
   completo + `cargo test -p searcher-rs --lib workers::binance_ws` deben PASS.
6. **Anti-mock (RULE 00)**: ¿algún dato hardcodeado/fabricado en el exportador?
   (solo defaults de config documentados son legítimos).

## Gates de ejecución (evidencia reproducible en el entregable)
- `cd <worktree>\backend` → `cargo test -p shared-rs` (esperado: 274 passed, 0 failed)
- `cargo test -p shared-rs --test pricebus_export_v1_vector` (esperado: 1 passed)
- `cargo test -p searcher-rs --lib workers::binance_ws` (esperado: 9 passed)
- `cargo clippy -p shared-rs -p searcher-rs --all-targets` (esperado: 0 warnings)
- NO corras `cargo test -p searcher-rs` completo (suite 1355 tests ya verificada
  por el orquestador; dura mucho y compite por el target/ lock).

## Entregable
`audits/forensic-engine-2026-09-21/WO-FE1-VALIDATION.md` — veredicto por check
con evidencia (salida comando + file:line), hallazgos CRITICAL/MAJOR/MINOR, y
veredicto final VALIDATED / VALIDATED-WITH-NOTES / REJECTED.
Además: actualizar la fila WO-FE1 del BOARD con tu veredicto (dejas el estado
🟡 → ✅ si VALIDATED, o documentas el bloqueo).

## Límites (doctrinas activas)
READ-ONLY sobre código fuente: NO editar src/, NO commit, NO push, NO deploy,
NO broadcast/wallets/capital (§32/§33). Fail-honest (R8): si algo no puedes
verificarlo, dilo — no lo asumas. aserciones ≠ facts.
