ID:            REA-08-PROTECTIONS
Fuente:        freqtrade/freqtrade  freqtrade/plugins/protections/stoploss_guard.py:44-111
               freqtrade/plugins/protections/max_drawdown_protection.py:46-133
               freqtrade/plugins/protections/iprotection.py:124-143 (calculate_lock_end)
               freqtrade/data/metrics.py (calculate_max_drawdown, drawdown_abs)
               @c051e9065c5b677f03817dd1f83cb980af1ed684
Licencia:      GPL-3.0 (LICENSE del sha: "GNU GENERAL PUBLIC LICENSE Version 3") → PORTADO, cero verbatim
Tipo:          PORTADO
Invariante:    (1) StoplossGuard bloquea sii el número de trades cerrados en la ventana con exit ∈
               {stop_loss, trailing_stop_loss, stoploss_on_exchange, liquidation} y close_profit < profit_limit
               es >= trade_limit; el lock termina en max(close_date de esos trades) + stop_duration.
               (2) MaxDrawdown (modo ratios) bloquea sii el drawdown del cumsum de close_profit en la ventana
               (peak−trough, peak desde 0) supera max_allowed_drawdown; lock hasta max(close)+stop_duration.
Matematica:    stoploss_guard(trades: &[ClosedTrade], now_ts_s: i64, pair_filter: Option<&str>,
                              p: &StoplossGuardParams) -> Option<ProtectionLock>
               max_drawdown_ratio(values_in_close_order: &[f64]) -> f64   // = drawdown_abs del upstream
               max_drawdown_protection(trades, now_ts_s, p: &MaxDrawdownParams) -> Option<ProtectionLock>
               (i64 epoch-seconds; f64 ratios; Option en vez de excepciones; sin panic/unwrap en producción)
Test origen:   · tests/plugins/test_protections.py:259-327 (test_stoploss_guard): lookback 60m, stop 40m,
                 trade_limit 3 → con 2 stoplosses in-window NO bloquea; con el 3º SÍ; until = max_close+40m.
               · tests/plugins/test_protections.py:329-439 (per-pair): trade_limit 2, lookback 60m → XRP con
                 2 stoplosses dispara; ETH con 1 no; trades fuera de ventana y con profit ≥ 0 no cuentan.
               · tests/data/test_metrics.py:524-591: 28 ratios → drawdown_abs = 0.091755; 5 primeros negados
                 → 0.055545; 5 primeros positivos → 0.0.
               · tests/plugins/test_protections.py:740-855 (test_MaxDrawdown_ratio_mode): lookback 1000m,
                 trade_limit 3, max_dd 0.15 → la serie final dispara; with threshold 0.95 no dispara.
Test propio:   backend/math-engine/src/risk_engine.rs::rea_08_protection_tests (5 tests:
               stoploss_guard_threshold_and_lock_end, stoploss_guard_pair_filter_and_profit_gate,
               drawdown_upstream_vectors, max_drawdown_protection_upstream_sequence, no_lock_without_enough_data)
Gate:          cargo test --manifest-path backend/Cargo.toml -p math-engine --lib rea_08 -- --nocapture
Control neg.:  (a) mismo trade set con max_allowed_drawdown 0.95 → NO bloquea (el lock no es incondicional);
               (b) 2 stoplosses en ventana → NO bloquea, el 3º → SÍ (frontera exacta del upstream);
               (c) 4 stoplosses con profit positivo → NO cuentan (profit gate);
               (d) ventana vacía / sin pérdidas → None (R8, nunca lock inventado).
               La versión vieja del destino (sin protecciones) deja operar en todos esos casos negativos: el
               control negativo es la ausencia del gate, no una variante de la fórmula.
Evidencia:     cargo test ... --lib rea_08 -> running 5 tests; 5 passed; 0 failed; 171 filtered out; exit 0
               (build increméntal reusando target: 2m49s). VERDE.
Riesgo:        Módulo puro sin consumidores aún: NO hay wiring al pipeline (no cambia el comportamiento del
               searcher). Los tipos son nuevos y autocontenidos; cero unwrap/panic en las funciones públicas.
FOLLOWUP:      REA-08-WIRE — consumir las protecciones antes de emitir/ejecutar: (1) fuente de trades cerrados
               (¿persistence? ¿executions?) mapeada a ClosedTrade; (2) gate en el emisor/sizing que consulte
               stoploss_guard/max_drawdown_protection con los knobs de config; (3) modo equity de MaxDrawdown
               (calculation_mode="equity", requiere balance inicial + profit_abs) NO portado.
               CooldownPeriod y LowProfitPairs del mismo upstream quedan fuera de esta unidad.
Estado:        IN_PROGRESS (gate verde; PR abierto, merge tras GATE1)
