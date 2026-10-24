//! Motor de Riesgo y Scoring para DeFi

/// Parámetros de la política de riesgo en vivo
#[derive(Debug, Clone)]
pub struct RiskPolicy {
    pub min_net_profit_usd: f64,
    pub min_net_roi_pct: f64,
    pub max_gas_cost_ratio: f64, // gas_cost / gross_profit
    pub max_slippage_pct: f64,
    pub max_price_impact_pct: f64,
    pub min_liquidity_usd: f64,
    pub max_trade_size_usd: f64,
}

/// Detalles de la Oportunidad para evaluación de riesgo
#[derive(Debug, Clone)]
pub struct OpportunityRiskProfile {
    pub gross_profit_usd: f64,
    pub net_profit_usd: f64,
    pub net_roi_pct: f64,
    pub gas_cost_usd: f64,
    pub slippage_expected_pct: f64,
    pub price_impact_pct: f64,
    pub liquidity_available_usd: f64,
    pub trade_size_usd: f64,
    pub simulation_passed: bool,
    pub contracts_verified: bool,
}

pub enum RiskRejectionReason {
    NegativeNetProfit,
    NetRoiTooLow,
    GasCostTooHigh,
    SlippageTooHigh,
    PriceImpactTooHigh,
    InsufficientLiquidity,
    TradeSizeExceeded,
    SimulationFailed,
    UnverifiedContracts,
}

/// Valida una oportunidad contra la política de riesgo estricta.
pub fn validate_opportunity_risk(
    profile: &OpportunityRiskProfile,
    policy: &RiskPolicy,
) -> Result<(), RiskRejectionReason> {
    if profile.net_profit_usd <= policy.min_net_profit_usd {
        return Err(RiskRejectionReason::NegativeNetProfit);
    }
    if profile.net_roi_pct < policy.min_net_roi_pct {
        return Err(RiskRejectionReason::NetRoiTooLow);
    }

    let gas_ratio = if profile.gross_profit_usd > 0.0 {
        profile.gas_cost_usd / profile.gross_profit_usd
    } else {
        1.0
    };
    if gas_ratio > policy.max_gas_cost_ratio {
        return Err(RiskRejectionReason::GasCostTooHigh);
    }

    if profile.slippage_expected_pct > policy.max_slippage_pct {
        return Err(RiskRejectionReason::SlippageTooHigh);
    }
    if profile.price_impact_pct > policy.max_price_impact_pct {
        return Err(RiskRejectionReason::PriceImpactTooHigh);
    }
    if profile.liquidity_available_usd < policy.min_liquidity_usd {
        return Err(RiskRejectionReason::InsufficientLiquidity);
    }
    if profile.trade_size_usd > policy.max_trade_size_usd {
        return Err(RiskRejectionReason::TradeSizeExceeded);
    }
    if !profile.simulation_passed {
        return Err(RiskRejectionReason::SimulationFailed);
    }
    if !profile.contracts_verified {
        return Err(RiskRejectionReason::UnverifiedContracts);
    }

    Ok(())
}

/// Calcula el Valor Esperado (Expected Value - EV) de la operación
pub fn calc_expected_value(
    net_profit_usd: f64,
    failure_cost_usd: f64, // Ej: gas perdido si revierte
    probability_success: f64,
) -> f64 {
    let probability_failure = 1.0 - probability_success;
    (probability_success * net_profit_usd) - (probability_failure * failure_cost_usd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_validation_success() {
        let policy = RiskPolicy {
            min_net_profit_usd: 1.0,
            min_net_roi_pct: 0.1,
            max_gas_cost_ratio: 0.5,
            max_slippage_pct: 0.01,
            max_price_impact_pct: 0.05,
            min_liquidity_usd: 10_000.0,
            max_trade_size_usd: 5_000.0,
        };

        let profile = OpportunityRiskProfile {
            gross_profit_usd: 50.0,
            net_profit_usd: 20.0,
            net_roi_pct: 0.5,
            gas_cost_usd: 10.0, // Ratio: 10/50 = 0.2 < 0.5
            slippage_expected_pct: 0.005,
            price_impact_pct: 0.01,
            liquidity_available_usd: 50_000.0,
            trade_size_usd: 4_000.0,
            simulation_passed: true,
            contracts_verified: true,
        };

        assert!(validate_opportunity_risk(&profile, &policy).is_ok());
    }

    #[test]
    fn test_risk_validation_gas_too_high() {
        let policy = RiskPolicy {
            min_net_profit_usd: 1.0,
            min_net_roi_pct: 0.1,
            max_gas_cost_ratio: 0.5,
            max_slippage_pct: 0.01,
            max_price_impact_pct: 0.05,
            min_liquidity_usd: 10_000.0,
            max_trade_size_usd: 5_000.0,
        };

        let profile = OpportunityRiskProfile {
            gross_profit_usd: 50.0,
            net_profit_usd: 2.0,
            net_roi_pct: 0.5,
            gas_cost_usd: 40.0, // Ratio: 40/50 = 0.8 > 0.5 -> FAIL
            slippage_expected_pct: 0.005,
            price_impact_pct: 0.01,
            liquidity_available_usd: 50_000.0,
            trade_size_usd: 4_000.0,
            simulation_passed: true,
            contracts_verified: true,
        };

        assert!(matches!(
            validate_opportunity_risk(&profile, &policy),
            Err(RiskRejectionReason::GasCostTooHigh)
        ));
    }
}

// ============================================================================
// REA-08-PROTECTIONS — protecciones de trading portadas de freqtrade
// ============================================================================
//
// REA: freqtrade/freqtrade freqtrade/plugins/protections/stoploss_guard.py:44-111 @c051e90
// REA: freqtrade/freqtrade freqtrade/plugins/protections/max_drawdown_protection.py:46-133 @c051e90
// REA: freqtrade/freqtrade freqtrade/plugins/protections/iprotection.py:124-143 @c051e90 (calculate_lock_end)
// Licencia origen: GPL-3.0 (LICENSE: "GNU GENERAL PUBLIC LICENSE Version 3").
// PORTADO: matemática/semántica extraída; CERO líneas verbatim (Python -> Rust,
// reescrito en las convenciones del destino: i64 epoch-seconds, f64 ratios,
// Option en vez de excepciones). Ver docs/rea/units/REA-08-PROTECTIONS.md.
//
// Invariantes (falsables):
//   · StoplossGuard bloquea sii #trades en ventana con exit ∈ {SL, trailing SL,
//     SL-on-exchange, liquidation} y close_profit < profit_limit es >= trade_limit;
//     el lock termina en max(close_date de esos trades) + stop_duration.
//   · MaxDrawdown (modo ratios, el default del upstream) bloquea sii el drawdown
//     del cumsum de close_profit sobre la ventana supera max_allowed_drawdown.
//   · R8: sin trades suficientes, ventana vacía o drawdown no positivo -> None
//     (nunca se inventa un lock).

/// Exit reasons que el upstream considera stoploss (stoploss_guard.py:57-66).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    StopLoss,
    TrailingStopLoss,
    StoplossOnExchange,
    Liquidation,
    Other,
}

/// Un trade cerrado (solo los campos que la matemática de protecciones usa).
#[derive(Debug, Clone)]
pub struct ClosedTrade {
    pub pair: String,
    /// Epoch seconds del close_date (UTC).
    pub close_ts_s: i64,
    /// close_profit como ratio (p. ej. -0.05 = -5 %).
    pub profit_ratio: f64,
    pub exit_reason: ExitReason,
}

/// Resultado de una protección que dispara: hasta cuándo se bloquea.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtectionLock {
    pub until_ts_s: i64,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct StoplossGuardParams {
    pub lookback_s: i64,
    pub trade_limit: usize,
    /// Los trades cuentan si close_profit < profit_limit (upstream default 0.0).
    pub profit_limit: f64,
    pub stop_duration_s: i64,
}

#[derive(Debug, Clone)]
pub struct MaxDrawdownParams {
    pub lookback_s: i64,
    pub trade_limit: usize,
    pub max_allowed_drawdown: f64,
    pub stop_duration_s: i64,
}

/// StoplossGuard (stoploss_guard.py:44-111) — puerto fiel.
///
/// `pair_filter` = Some(pair) emula `stop_per_pair`; None emula `global_stop`.
pub fn stoploss_guard(
    trades: &[ClosedTrade],
    now_ts_s: i64,
    pair_filter: Option<&str>,
    params: &StoplossGuardParams,
) -> Option<ProtectionLock> {
    let since = now_ts_s - params.lookback_s;
    let qualifying: Vec<&ClosedTrade> = trades
        .iter()
        .filter(|t| {
            t.close_ts_s >= since
                && matches!(
                    t.exit_reason,
                    ExitReason::StopLoss
                        | ExitReason::TrailingStopLoss
                        | ExitReason::StoplossOnExchange
                        | ExitReason::Liquidation
                )
                && t.profit_ratio < params.profit_limit
                && pair_filter.map_or(true, |p| t.pair == p)
        })
        .collect();
    if qualifying.len() < params.trade_limit {
        return None;
    }
    // calculate_lock_end (iprotection.py:124-143): max close_date + stop_duration.
    let max_close = qualifying.iter().map(|t| t.close_ts_s).max()?;
    Some(ProtectionLock {
        until_ts_s: max_close + params.stop_duration_s,
        reason: format!("{} stoplosses within lookback", params.trade_limit),
    })
}

/// Drawdown absoluto del cumsum de una serie de ratios (en orden temporal).
///
/// Equivale a `drawdown_abs` de `calculate_max_drawdown(value_col="profit")`
/// (freqtrade/data/metrics.py): máximo de (peak - cumulative) sobre la serie,
/// con peak arrancando en 0.0 (capital inicial). Vectores byte-exactos en tests.
pub fn max_drawdown_ratio(values_in_close_order: &[f64]) -> f64 {
    let mut cumulative = 0.0f64;
    let mut peak = 0.0f64;
    let mut max_dd = 0.0f64;
    for value in values_in_close_order {
        cumulative += *value;
        if cumulative > peak {
            peak = cumulative;
        }
        let dd = peak - cumulative;
        if dd > max_dd {
            max_dd = dd;
        }
    }
    max_dd
}

/// MaxDrawdown (max_drawdown_protection.py:46-133), modo `ratios` (el default).
///
/// Solo global_stop: el upstream declara `has_local_stop = False` — el caller
/// no debe tratarlo por par (se refleja devolviendo siempre el lock global).
pub fn max_drawdown_protection(
    trades: &[ClosedTrade],
    now_ts_s: i64,
    params: &MaxDrawdownParams,
) -> Option<ProtectionLock> {
    let since = now_ts_s - params.lookback_s;
    let mut window: Vec<&ClosedTrade> = trades.iter().filter(|t| t.close_ts_s >= since).collect();
    if window.len() < params.trade_limit {
        return None;
    }
    // Cumsum en orden de close_date (el upstream ordena por date_col).
    window.sort_by_key(|t| t.close_ts_s);
    let values: Vec<f64> = window.iter().map(|t| t.profit_ratio).collect();
    let drawdown = max_drawdown_ratio(&values);
    if drawdown <= params.max_allowed_drawdown {
        return None;
    }
    let max_close = window.iter().map(|t| t.close_ts_s).max()?;
    Some(ProtectionLock {
        until_ts_s: max_close + params.stop_duration_s,
        reason: format!("drawdown {drawdown} passed {}", params.max_allowed_drawdown),
    })
}

#[cfg(test)]
mod rea_08_protection_tests {
    use super::*;

    fn trade(pair: &str, close_ts_s: i64, profit_ratio: f64, reason: ExitReason) -> ClosedTrade {
        ClosedTrade {
            pair: pair.to_string(),
            close_ts_s,
            profit_ratio,
            exit_reason: reason,
        }
    }

    // ---- StoplossGuard: vectores de tests/plugins/test_protections.py:259-327

    #[test]
    fn rea_08_stoploss_guard_threshold_and_lock_end() {
        // Upstream test_stoploss_guard: lookback 60 min, stop_duration 40 min, trade_limit 3.
        let p = StoplossGuardParams {
            lookback_s: 60 * 60,
            trade_limit: 3,
            profit_limit: 0.0,
            stop_duration_s: 40 * 60,
        };
        let now = 1_000_000i64;
        // XRP + ETH cerrados hace 30 min; BCH hace 100 min (FUERA de la ventana).
        let mut trades = vec![
            trade("XRP/BTC", now - 1800, -0.05, ExitReason::StopLoss),
            trade("BCH/BTC", now - 6000, -0.05, ExitReason::StopLoss),
            trade("ETH/BTC", now - 1800, -0.05, ExitReason::StopLoss),
        ];
        // 2 dentro de ventana < 3 -> no bloquea (el BCH no cuenta).
        assert!(stoploss_guard(&trades, now, None, &p).is_none());
        // Tercer stoploss dentro de ventana -> bloquea hasta max(close)+40 min.
        trades.push(trade("LTC/BTC", now - 1800, -0.05, ExitReason::StopLoss));
        let lock = stoploss_guard(&trades, now, None, &p).expect("debe bloquear con 3");
        assert_eq!(lock.until_ts_s, now - 1800 + 40 * 60);
    }

    #[test]
    fn rea_08_stoploss_guard_pair_filter_and_profit_gate() {
        // Upstream test_stoploss_guard_perpair: trade_limit 2, lookback 60 min.
        let p = StoplossGuardParams {
            lookback_s: 60 * 60,
            trade_limit: 2,
            profit_limit: 0.0,
            stop_duration_s: 60 * 60,
        };
        let now = 1_000_000i64;
        let trades = vec![
            trade("XRP/BTC", now - 1800, -0.05, ExitReason::StopLoss),
            trade("ETH/BTC", now - 1800, -0.05, ExitReason::StopLoss),
            trade("XRP/BTC", now - 1500, -0.05, ExitReason::StopLoss),
        ];
        // Por par: XRP tiene 2 -> dispara; ETH tiene 1 -> no.
        assert!(stoploss_guard(&trades, now, Some("XRP/BTC"), &p).is_some());
        assert!(stoploss_guard(&trades, now, Some("ETH/BTC"), &p).is_none());
        // Gate de profit: 4 stoplosses con profit POSITIVO no cuentan.
        let winners = vec![
            trade("XRP/BTC", now - 600, 0.9, ExitReason::StopLoss),
            trade("ETH/BTC", now - 600, 0.9, ExitReason::StopLoss),
            trade("LTC/BTC", now - 600, 0.9, ExitReason::StopLoss),
            trade("NEO/BTC", now - 600, 0.9, ExitReason::StopLoss),
        ];
        assert!(stoploss_guard(&winners, now, None, &p).is_none());
    }

    // ---- Drawdown: vectores de tests/data/test_metrics.py:524-591 ----------

    #[test]
    fn rea_08_drawdown_upstream_vectors() {
        // test_calculate_max_drawdown2 (28 valores) -> drawdown_abs = 0.091755.
        let values: [f64; 28] = [
            0.011580, 0.010048, 0.011340, 0.012161, 0.010416, 0.010009, 0.020024, -0.024662,
            -0.022350, 0.020496, -0.029859, -0.030511, 0.010041, 0.010872, -0.025782, 0.010400,
            0.012374, 0.012467, 0.114741, 0.010303, 0.010088, -0.033961, 0.010680, 0.010886,
            -0.029274, 0.011178, 0.010693, 0.010711,
        ];
        assert!((max_drawdown_ratio(&values) - 0.091755).abs() < 1e-12);
        // Los 5 primeros NEGADOS -> drawdown_abs = 0.055545 (upstream :588).
        let negated: Vec<f64> = values[..5].iter().map(|v| -v).collect();
        assert!((max_drawdown_ratio(&negated) - 0.055545).abs() < 1e-12);
        // Los 5 primeros positivos -> 0.0 (upstream :579).
        assert_eq!(max_drawdown_ratio(&values[..5]), 0.0);
    }

    #[test]
    fn rea_08_max_drawdown_protection_upstream_sequence() {
        // test_MaxDrawdown_ratio_mode: lookback 1000 min, trade_limit 3,
        // max_allowed_drawdown 0.15, stop_duration 60 min. close_profit = rate - 1
        // (convención de generate_mock_trade): 1.1->+0.1, 0.9->-0.1, 0.5->-0.5,
        // ROI 1.5->+0.5, ROI 0.2->-0.8. En ventana (1100 min queda fuera):
        // +0.1 x3 (900m), +0.5 (410m), -0.1 (400m), -0.8 (10m)
        // cumsum [0.1,0.2,0.3,0.8,0.7,-0.1] -> dd = 0.9 > 0.15 -> bloquea.
        let p = MaxDrawdownParams {
            lookback_s: 1000 * 60,
            trade_limit: 3,
            max_allowed_drawdown: 0.15,
            stop_duration_s: 60 * 60,
        };
        let now = 1_000_000i64;
        let trades = vec![
            trade("XRP/BTC", now - 900 * 60, 0.1, ExitReason::StopLoss),
            trade("ETH/BTC", now - 900 * 60, 0.1, ExitReason::StopLoss),
            trade("NEO/BTC", now - 900 * 60, 0.1, ExitReason::StopLoss),
            trade("XRP/BTC", now - 1100 * 60, -0.5, ExitReason::StopLoss), // FUERA
            trade("XRP/BTC", now - 400 * 60, -0.1, ExitReason::StopLoss),
            trade("XRP/BTC", now - 410 * 60, 0.5, ExitReason::Other),
            trade("XRP/BTC", now - 10 * 60, -0.8, ExitReason::Other),
        ];
        let lock = max_drawdown_protection(&trades, now, &p).expect("dd 0.9 > 0.15");
        assert_eq!(lock.until_ts_s, now - 10 * 60 + 60 * 60);
        // Control negativo: mismo input con umbral 0.95 -> NO bloquea (el lock
        // no es incondicional). Y con < trade_limit en ventana -> NO bloquea.
        let loose = MaxDrawdownParams {
            max_allowed_drawdown: 0.95,
            ..p.clone()
        };
        assert!(max_drawdown_protection(&trades, now, &loose).is_none());
        let strict_count = MaxDrawdownParams {
            trade_limit: 10,
            ..p.clone()
        };
        assert!(max_drawdown_protection(&trades, now, &strict_count).is_none());
    }

    #[test]
    fn rea_08_no_lock_without_enough_data() {
        // R8: ventana vacía -> None; sin losses -> None (upstream :787-788).
        let p = MaxDrawdownParams {
            lookback_s: 1000 * 60,
            trade_limit: 3,
            max_allowed_drawdown: 0.15,
            stop_duration_s: 60 * 60,
        };
        let now = 1_000_000i64;
        assert!(max_drawdown_protection(&[], now, &p).is_none());
        let winners = vec![
            trade("A/B", now - 600, 0.11, ExitReason::Other),
            trade("C/B", now - 600, 0.11, ExitReason::Other),
            trade("D/B", now - 600, 0.11, ExitReason::Other),
        ];
        assert!(max_drawdown_protection(&winners, now, &p).is_none());
    }
}

// ============================================================================
// REA-08 falsificadores (skill arbx-rea-brutal F7). Variante ROTA a proposito:
// la MISMA asercion que valida el port debe matarla. Si el roto pasara, el test
// no probaria nada. Estos tests se ejecutan (cargo test) y "caen" por diseno.
// ============================================================================
#[cfg(test)]
mod rea_08_falsifiers {
    use super::*;

    fn trade(pair: &str, close_ts_s: i64, profit_ratio: f64, reason: ExitReason) -> ClosedTrade {
        ClosedTrade {
            pair: pair.to_string(),
            close_ts_s,
            profit_ratio,
            exit_reason: reason,
        }
    }

    /// Variante ROTA: ignora `pair_filter` (el upstream filtra por par con stop_per_pair).
    fn broken_guard_ignores_pair_filter(
        trades: &[ClosedTrade],
        now_ts_s: i64,
        _pair_filter: Option<&str>,
        params: &StoplossGuardParams,
    ) -> Option<ProtectionLock> {
        let since = now_ts_s - params.lookback_s;
        let qualifying: Vec<&ClosedTrade> = trades
            .iter()
            .filter(|t| {
                t.close_ts_s >= since
                    && matches!(
                        t.exit_reason,
                        ExitReason::StopLoss
                            | ExitReason::TrailingStopLoss
                            | ExitReason::StoplossOnExchange
                            | ExitReason::Liquidation
                    )
                    && t.profit_ratio < params.profit_limit
            })
            .collect();
        if qualifying.len() < params.trade_limit {
            return None;
        }
        let max_close = qualifying.iter().map(|t| t.close_ts_s).max()?;
        Some(ProtectionLock {
            until_ts_s: max_close + params.stop_duration_s,
            reason: "broken".to_string(),
        })
    }

    /// Variante ROTA: ignora la ventana (lookback_s).
    fn broken_guard_ignores_window(
        trades: &[ClosedTrade],
        _now_ts_s: i64,
        pair_filter: Option<&str>,
        params: &StoplossGuardParams,
    ) -> Option<ProtectionLock> {
        let qualifying: Vec<&ClosedTrade> = trades
            .iter()
            .filter(|t| {
                matches!(
                    t.exit_reason,
                    ExitReason::StopLoss
                        | ExitReason::TrailingStopLoss
                        | ExitReason::StoplossOnExchange
                        | ExitReason::Liquidation
                ) && t.profit_ratio < params.profit_limit
                    && pair_filter.map_or(true, |p| t.pair == p)
            })
            .collect();
        if qualifying.len() < params.trade_limit {
            return None;
        }
        let max_close = qualifying.iter().map(|t| t.close_ts_s).max()?;
        Some(ProtectionLock {
            until_ts_s: max_close + params.stop_duration_s,
            reason: "broken".to_string(),
        })
    }

    /// Variante ROTA: el lock termina en MIN(close) en vez de MAX(close).
    fn broken_guard_min_close(
        trades: &[ClosedTrade],
        now_ts_s: i64,
        pair_filter: Option<&str>,
        params: &StoplossGuardParams,
    ) -> Option<ProtectionLock> {
        let since = now_ts_s - params.lookback_s;
        let qualifying: Vec<&ClosedTrade> = trades
            .iter()
            .filter(|t| {
                t.close_ts_s >= since
                    && matches!(
                        t.exit_reason,
                        ExitReason::StopLoss
                            | ExitReason::TrailingStopLoss
                            | ExitReason::StoplossOnExchange
                            | ExitReason::Liquidation
                    )
                    && t.profit_ratio < params.profit_limit
                    && pair_filter.map_or(true, |p| t.pair == p)
            })
            .collect();
        if qualifying.len() < params.trade_limit {
            return None;
        }
        let min_close = qualifying.iter().map(|t| t.close_ts_s).min()?;
        Some(ProtectionLock {
            until_ts_s: min_close + params.stop_duration_s,
            reason: "broken".to_string(),
        })
    }

    /// Variante ROTA: drawdown RELATIVO (divide por el peak) en vez de absoluto.
    fn broken_max_drawdown_ratio(values_in_close_order: &[f64]) -> f64 {
        let mut cumulative = 0.0f64;
        let mut peak = 0.0f64;
        let mut max_dd = 0.0f64;
        for value in values_in_close_order {
            cumulative += *value;
            if cumulative > peak {
                peak = cumulative;
            }
            let dd = (peak - cumulative) / peak.max(f64::EPSILON);
            if dd > max_dd {
                max_dd = dd;
            }
        }
        max_dd
    }

    #[test]
    fn rea_08_falsifier_pair_filter_bites() {
        // 3 stoplosses: 2 en ETH/USDC y 1 en BTC/USDT. Con stop_per_pair(ETH/USDC)
        // el port correcto ve 2 (< trade_limit 3) => None. El roto ignora el par => Some.
        let trades = vec![
            trade("ETH/USDC", 1_000, -0.02, ExitReason::StopLoss),
            trade("ETH/USDC", 1_100, -0.02, ExitReason::TrailingStopLoss),
            trade("BTC/USDT", 1_200, -0.02, ExitReason::StopLoss),
        ];
        let p = StoplossGuardParams {
            lookback_s: 3_600,
            trade_limit: 3,
            profit_limit: 0.0,
            stop_duration_s: 2_400,
        };
        let now = 1_300;
        assert_eq!(
            stoploss_guard(&trades, now, Some("ETH/USDC"), &p),
            None,
            "per-pair: solo 2 del par cuentan (la asercion que el roto viola)"
        );
        let broken = broken_guard_ignores_pair_filter(&trades, now, Some("ETH/USDC"), &p);
        assert!(
            broken.is_some(),
            "el roto SI dispara: la asercion de arriba lo mata => el falsificador muerde"
        );
    }

    #[test]
    fn rea_08_falsifier_lookback_window_bites() {
        // Solo 1 trade dentro de la ventana (now 10_000, lookback 3600 => since 6_400).
        // Correcto: 1 < 3 => None. El roto (sin ventana) ve 3 => Some.
        let trades = vec![
            trade("A/B", 1_000, -0.02, ExitReason::StopLoss),
            trade("A/B", 2_000, -0.02, ExitReason::StopLoss),
            trade("A/B", 9_000, -0.02, ExitReason::StopLoss),
        ];
        let p = StoplossGuardParams {
            lookback_s: 3_600,
            trade_limit: 3,
            profit_limit: 0.0,
            stop_duration_s: 2_400,
        };
        let now = 10_000;
        assert_eq!(
            stoploss_guard(&trades, now, None, &p),
            None,
            "solo 1 dentro de la ventana (la asercion que el roto viola)"
        );
        let broken = broken_guard_ignores_window(&trades, now, None, &p);
        assert!(broken.is_some(), "el roto cuenta los viejos => muerde");
    }

    #[test]
    fn rea_08_falsifier_lock_end_max_close_bites() {
        // calculate_lock_end (iprotection.py:124-143): max(close_date) + stop_duration.
        let trades = vec![
            trade("A/B", 1_000, -0.02, ExitReason::StopLoss),
            trade("A/B", 1_100, -0.02, ExitReason::StopLoss),
            trade("A/B", 1_200, -0.02, ExitReason::StopLoss),
        ];
        let p = StoplossGuardParams {
            lookback_s: 3_600,
            trade_limit: 3,
            profit_limit: 0.0,
            stop_duration_s: 2_400,
        };
        let now = 1_300;
        let lock = stoploss_guard(&trades, now, None, &p).expect("dispara");
        assert_eq!(lock.until_ts_s, 1_200 + 2_400);
        let broken = broken_guard_min_close(&trades, now, None, &p).expect("dispara");
        assert_eq!(broken.until_ts_s, 1_000 + 2_400);
        assert_ne!(
            broken.until_ts_s, lock.until_ts_s,
            "si el roto pasara, el test no probaria nada"
        );
    }

    #[test]
    fn rea_08_falsifier_drawdown_formula_bites() {
        // Serie donde absoluto != relativo: cumsum 0.5, 0.25; peak 0.5; abs 0.25; rel 0.5.
        let v = [0.5f64, -0.25f64];
        assert_eq!(max_drawdown_ratio(&v), 0.25);
        assert_eq!(broken_max_drawdown_ratio(&v), 0.5);
        assert_ne!(
            broken_max_drawdown_ratio(&v),
            max_drawdown_ratio(&v),
            "si el roto pasara, el test no probaria nada"
        );
    }
}
