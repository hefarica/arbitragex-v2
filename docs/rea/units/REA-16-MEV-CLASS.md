ID:            REA-16-MEV-CLASS
Fuente:        flashbots/mev-inspect-rs @b6e2f0a4784d063375734e2773fa7196cf43174d
               src/reducers/arbitrage.rs:20-74 (ArbitrageReducer::reduce)
               src/inspectors/mod.rs:39-71 (find_matching, check_all=true)
Licencia:      **MIT** (LICENSE del sha: "MIT License, Copyright (c) 2020 Flashbots").
               CORRECCIÓN DE REGISTRO: la ficha REA-trace (t5) anotó esta fuente como "sin licencia" — era FALSO;
               el sha b6e2f0a la licencia es MIT. Los números y la semántica son portables con normalidad.
Tipo:          PORTADO (cero verbatim; Rust reescrito con alloy-primitives)
Invariante:    un par de trades es un ARBITRAJE sii el segundo cierra el token del primero
               (`t2.t2.token == t1.t1.token`) y devuelve MÁS (`t2.t2.amount > t1.t1.amount`); profit =
               `t2.t2.amount - t1.t1.amount` en el token de cierre; el rango (i+1)..=j se poda.
Matematica:    reduce_arbitrages(actions: &[MevAction]) -> Vec<ArbClassification>
               (Trade{Transfer{from,to,amount,token}}; U256; matching = primer Trade posterior con
               cierre de token; saturating_sub; prune del rango completo — find_matching check_all=true).
Test origen:   mev-inspect-rs src/reducers/arbitrage.rs:90-148 (`simple_arb`): trades 100 token1→200 token2
               y 200 token2→110 token1 ⇒ expected Arbitrage{profit: 10, token: token1, to: usr} + Prune.
Test propio:   backend/math-engine/src/mev_reducer.rs::rea_16_tests (4 tests:
               arb_reducer_upstream_vector_profit_10, no_arb_without_positive_profit,
               no_arb_without_token_cycle, prunes_entire_range_between_trades)
Gate:          cargo test --manifest-path backend/Cargo.toml -p math-engine --lib rea_16 -- --nocapture
Control neg.:  (a) retorno IGUAL (no mayor) → sin arb (la condición del upstream es estricta); (b) segundo trade
               que cierra en OTRO token → sin match; (c) el rango entre trades se poda completo (incluye
               acciones intermedias y el closing trade) — sin esos casos el reducer pasaría en falso.
Evidencia:     cargo test ... --lib rea_16 -> running 4 tests; 4 passed; 0 failed; 171 filtered out; exit 0
               (build 3m04s con target compartido). VERDE.
Riesgo:        Módulo nuevo SIN consumidores: no cambia el runtime (analytics puro). Dependencia añadida:
               alloy-primitives (ya en el workspace) — cero dependencias nuevas externas.
FOLLOWUP:      (a) LiquidationReducer (liquidation.rs:20-95, profit = received − paid) no portado: su vector
               (profit: 2) exige el matching completo trade/liquidation/payout; declarado.
               (b) Ingesta de trazas reales (revm/parity traces → MevAction) y wiring en recon = otro proyecto.
               (c) Sandwich/JIT classification de mev-inspect no revisado (fuera del alcance de esta unidad).
Estado:        IN_PROGRESS (gate verde; PR abierto, merge tras GATE1/orden del operador)
