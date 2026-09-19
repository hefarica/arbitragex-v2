# WO-LEGS-TRIANGULAR-01 — Ledger por-leg para triangulares (investigación 2026-09-17/18)
> Orden operador: "resuelve esto y dame opciones para que aparezcan computados"
> Skill: /arbitragex-omniscience §0-§12 (LEARNINGS leído; BR-11 = waterfall por hop)

## Diagnóstico (verificado en código con líneas exactas)
El ledger por-leg MUERE dentro del kernel:
- `cycle_profit` (triangular_worker.rs:249-263): el loop `current = v2_amount_out(current, r_in, r_out, fee_bps)` **YA computa la cadena exacta por hop** con reserves cacheadas (0 RPC) — pero sólo retorna el monto final.
- `size_triangular_with_reason` (size_optimizer.rs:695-824): llama evaluate_cycle; comment explícito "triangular kernel exposes only the final cycle amount — per-leg wei honestly absent (R8)".
- El resto del pipeline YA está listo: kernel 2-leg emite su ledger (:965-969), `attach_leg_ledger` all-or-nothing (shared-rs:203), orchestrator:1129 lo adjunta, FE renderiza (HOPS-LEDGER-04, deriveLegs ladder).
- Kelly-rebound ya niega el ledger honestamente cuando rescalea el monto (:645, :661).
- Cartridge layer ya puede adjuntar (cartridge_boot:1589-1592) si el Rhai lo provee.

## Volumen vivo (PG, 2h): 44 filas con ledger (kernel 2-leg); 12.675 triangulares, 0 llegan a Sized (ventana quieta — mueren en spot_product_le_one ANTES del sizing; no es defecto del ledger).

## Opciones
- **A (recomendada, ~25 líneas + tests)**: `cycle_profit_with_ledger` — MISMO loop, colectar los intermedios; EvalResult gana `leg_outputs`; size_triangular arma `leg_amounts_in=[x,out0,out1]` / `leg_amounts_out=[out0,out1,out2]`. Garantía estructural: el ledger encadena EXACTO al final que el profit reportó (misma función, jamás divergente).
- **B**: recomputar la cadena en size_triangular — duplica la fórmula, riesgo de divergencia silenciosa.
- **C**: cartridges triangulares (MEV-09:20/10:18) emiten el ledger vía host_bindings — superficie mayor.
- **NO opción**: recomputar en el FE (§79 prohíbe).

## Pendiente
- Sancho (Hermes run durable) validando A + vector independiente (doctrina §12.1).
- Nota de alcance: fee uniforme 30bps = el que el kernel YA pricea (consistencia interna garantizada); fees reales por pool = WO aparte (exige lectura on-chain, cambia economía).
