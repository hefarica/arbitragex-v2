ID:            REA-03-V3-MATH
Fuente:        Uniswap/v3-core  contracts/libraries/TickMath.sol:23-54  y  contracts/libraries/SqrtPriceMath.sol:153-194  @aab5aa0
Licencia:      BUSL-1.1 con Change Date 2023-04-01 (VENCIDA) → el fichero declara SPDX GPL-2.0-or-later; PORTADO (matemática extraída, cero líneas verbatim)
Tipo:          PORTADO
Invariante:    getSqrtRatioAtTick(tick) devuelve exactamente √(1.0001^tick)·2^96 para |tick| ≤ 887272 y None fuera de rango (nunca un ratio fabricado); los deltas entre dos precios redondean hacia el pool: ceil con roundUp, floor con roundDown, difiriendo en exactamente 1 wei en los vectores del upstream.
Matematica:    v3_get_sqrt_ratio_at_tick(tick: i32) -> Option<U256>
               v3_mul_div_rounding_up(a, b, denom: U256) -> Option<U256>
               v3_div_rounding_up(a, b: U256) -> Option<U256>
               v3_get_amount0_delta(sqrtA, sqrtB, liquidity: U256, round_up: bool) -> Option<U256>
               v3_get_amount1_delta(sqrtA, sqrtB, liquidity: U256, round_up: bool) -> Option<U256>
Test origen:   test/TickMath.spec.ts:31-53 → min tick 4295128739 · min+1 4295343490 · max 1461446703485210103287273052203988822378723970342 · max−1 1461373636630004318706518188784493106690254656249
               test/SqrtPriceMath.spec.ts:240-338 → getAmount0Delta(√1,√1.21,L=1e18,true)=90909090909090910 (roundDown = −1) · getAmount1Delta=100000000000000000 (roundDown = −1) · L=0→0 · precios iguales→0
Test propio:   backend/searcher-rs/src/amm_math.rs::rea_v3_math_tests (6 tests: tickmath_upstream_vectors_byte_exact, tickmath_out_of_range_is_none_never_panic, tickmath_strictly_monotone, amount_deltas_upstream_vectors_byte_exact, amount_deltas_degenerate_cases_are_zero_or_none, negative_control_rounding_is_not_floor_only)
Gate:          cargo test --manifest-path backend/Cargo.toml -p searcher-rs --lib rea_03 -- --nocapture
Control neg.:  rea_03_negative_control_rounding_is_not_floor_only → roundUp == roundDown+1 (una implementación floor-only devuelve 90909090909090909 donde el vector exige 90909090909090910 → el test FALLA); y |tick|>887272 → None en vez de ratio (R8). El vector byte-exacto hace el test no tautológico: romper la fórmula o las constantes lo tumba.
Evidencia:     cargo test ... --lib rea_03 -> running 6 tests; 6 passed; 0 failed; 1742 filtered out; exit 0 (build 17m02s). VERDE.
Riesgo:        Funciones nuevas sin consumidores todavía (no hay wiring a reledger_at) → cero riesgo de runtime; añade 19 constantes hex Q128.128 (port fiel) y dos helpers con Option (sin panic/unwrap en producción).
FOLLOWUP:      REA-03-WIRE — el gate del backlog (ledger V3 CONTIGUO en reledger_at en vez de (None,None)) NO es alcanzable con los datos actuales: RouteLeg (prioritization-spine/src/route_plan.rs:27) no trae sqrt_price_x96/liquidity; ReservesCache (engines/triangular_engine.rs:84) es V2-only (U256,U256); reledger_at (src/size_optimizer.rs:1398) no tiene Redis ni chain_id. Requiere slot0 cache en StateProjector + campos V3 en RouteLeg + emisores. Con esa plomería, esta math es la que produce out_i == in_{i+1}.
               REA-18-V3-SIM consumirá las mismas funciones en simulator-v2.
Estado:        IN_PROGRESS (matemática aterrizada con gate verde; cableado declarado en REA-03-WIRE)
