//! REA-16-MEV-CLASS — clasificación de MEV por tipo sobre acciones de una tx.
//!
//! REA: flashbots/mev-inspect-rs src/reducers/arbitrage.rs:20-74 @b6e2f0a
//! REA: flashbots/mev-inspect-rs src/inspectors/mod.rs:39-71 (find_matching, check_all=true)
//! Licencia origen: MIT (LICENSE: "MIT License, Copyright (c) 2020 Flashbots").
//! PORTADO: semántica extraída; cero líneas verbatim (Rust reescrito en las
//! convenciones del destino: alloy-primitives, sin Interior Mutability).
//!
//! Invariante (falsable): un par de trades constituye un ARBITRAJE sii el segundo
//! trade cierra el token del primero (`t2.t2.token == t1.t1.token`) y devuelve MÁS
//! de lo que el primero gastó (`t2.t2.amount > t1.t1.amount`); el profit es
//! `t2.t2.amount - t1.t1.amount` en el token de cierre, y todo el rango entre
//! ambos trades se poda (deja de contarse como acciones independientes).
//!
//! R8: sin match de token o sin profit positivo, no hay clasificación — nunca se
//! fabrica un arbitraje con profit 0 o negativo.

use alloy_primitives::{Address, U256};

/// Transfer simple (token, from, to, amount) — el grano de acción del reducer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    pub from: Address,
    pub to: Address,
    pub amount: U256,
    pub token: Address,
}

/// Un swap de dos transfers: `t1` = lo que entra al pool, `t2` = lo que sale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    pub t1: Transfer,
    pub t2: Transfer,
}

/// Acción clasificada de la inspección. Solo distinguimos Trade vs resto:
/// el reducer del upstream únicamente mira Trades y poda el resto del rango.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MevAction {
    Trade(Trade),
    Other,
}

/// Resultado por posición: arbitraje detectado, acción conservada o podada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArbClassification {
    Arbitrage {
        /// `t2.t2.amount - t1.t1.amount` (token de cierre).
        profit: U256,
        token: Address,
        to: Address,
    },
    Kept,
    Pruned,
}

/// Port fiel de `ArbitrageReducer::reduce` (mev-inspect-rs arbitrage.rs:20-74).
///
/// Para cada Trade en orden: busca el PRIMER Trade posterior cuyo `t2.token`
/// iguale el `t1.token` del actual (equivalente a `find_matching(..., check_all=true)`);
/// si existe y `t2.amount > t1.amount`, clasifica el actual como Arbitrage y poda
/// el rango `(i+1)..=j`. Todo lo demás queda `Kept`.
pub fn reduce_arbitrages(actions: &[MevAction]) -> Vec<ArbClassification> {
    let mut out: Vec<ArbClassification> = vec![ArbClassification::Kept; actions.len()];
    for (i, action) in actions.iter().enumerate() {
        let MevAction::Trade(trade) = action else {
            continue;
        };
        // find_matching con check_all=true: recorre TODAS las acciones posteriores
        // y devuelve la primera que castee a Trade y cumpla el token de cierre.
        let mut found: Option<(usize, &Trade)> = None;
        for (j, candidate) in actions.iter().enumerate().skip(i + 1) {
            if let MevAction::Trade(cand_trade) = candidate {
                if cand_trade.t2.token == trade.t1.token {
                    found = Some((j, cand_trade));
                    break;
                }
            }
        }
        let Some((j, closing)) = found else {
            continue;
        };
        if closing.t2.amount > trade.t1.amount {
            out[i] = ArbClassification::Arbitrage {
                profit: closing.t2.amount.saturating_sub(trade.t1.amount),
                token: closing.t2.token,
                to: closing.t2.to,
            };
            for slot in out.iter_mut().take(j + 1).skip(i + 1) {
                *slot = ArbClassification::Pruned;
            }
        }
    }
    out
}

#[cfg(test)]
mod rea_16_tests {
    use super::*;

    fn addr(seed: u8) -> Address {
        Address::from([seed; 20])
    }

    fn transfer(from: Address, to: Address, amount: u64, token: Address) -> Transfer {
        Transfer {
            from,
            to,
            amount: U256::from(amount),
            token,
        }
    }

    // Vector del upstream: mev-inspect-rs src/reducers/arbitrage.rs:90-148
    // (test `simple_arb`: 100 token1 -> 200 token2, luego 200 token2 -> 110 token1;
    //  el par se condensa en un Arbitrage{profit: 10, token: token1, to: usr}).
    #[test]
    fn rea_16_arb_reducer_upstream_vector_profit_10() {
        let token1 = addr(0x01);
        let token2 = addr(0x02);
        let usr = addr(0x04);
        let uni1 = addr(0x05);
        let uni2 = addr(0x06);

        let t1 = Trade {
            t1: transfer(usr, uni1, 100, token1),
            t2: transfer(uni1, usr, 200, token2),
        };
        let t2 = Trade {
            t1: transfer(usr, uni2, 200, token2),
            t2: transfer(uni2, usr, 110, token1),
        };
        let actions = vec![MevAction::Trade(t1), MevAction::Trade(t2)];

        let out = reduce_arbitrages(&actions);
        assert_eq!(
            out,
            vec![
                ArbClassification::Arbitrage {
                    profit: U256::from(10u64),
                    token: token1,
                    to: usr,
                },
                ArbClassification::Pruned,
            ]
        );
    }

    // Control negativo (REA_MASTER §5.PASO 5): la condición del upstream exige
    // ESTRICTAMENTE `t2.amount > t1.amount`. Con retorno igual no hay arb.
    #[test]
    fn rea_16_no_arb_without_positive_profit() {
        let token1 = addr(0x11);
        let token2 = addr(0x12);
        let usr = addr(0x13);
        let pool1 = addr(0x14);
        let pool2 = addr(0x15);

        let t1 = Trade {
            t1: transfer(usr, pool1, 100, token1),
            t2: transfer(pool1, usr, 100, token2),
        };
        let t2 = Trade {
            t1: transfer(usr, pool2, 100, token2),
            t2: transfer(pool2, usr, 100, token1), // igual, no mayor
        };
        let out = reduce_arbitrages(&[MevAction::Trade(t1), MevAction::Trade(t2)]);
        assert_eq!(out, vec![ArbClassification::Kept, ArbClassification::Kept]);
    }

    // Sin ciclo de token (el segundo trade no cierra el token del primero) no hay match.
    #[test]
    fn rea_16_no_arb_without_token_cycle() {
        let token1 = addr(0x21);
        let token2 = addr(0x22);
        let token3 = addr(0x23);
        let usr = addr(0x24);
        let pool1 = addr(0x25);
        let pool2 = addr(0x26);

        let t1 = Trade {
            t1: transfer(usr, pool1, 100, token1),
            t2: transfer(pool1, usr, 200, token2),
        };
        let t2 = Trade {
            t1: transfer(usr, pool2, 200, token2),
            t2: transfer(pool2, usr, 300, token3), // cierra en token3, no en token1
        };
        let out = reduce_arbitrages(&[MevAction::Trade(t1), MevAction::Trade(t2)]);
        assert_eq!(out, vec![ArbClassification::Kept, ArbClassification::Kept]);
    }

    // El rango entre ambos trades se poda completo (upstream :52-73), incluidas
    // acciones intermedias y el propio closing trade.
    #[test]
    fn rea_16_prunes_entire_range_between_trades() {
        let token1 = addr(0x31);
        let token2 = addr(0x32);
        let usr = addr(0x33);
        let pool1 = addr(0x34);
        let pool2 = addr(0x35);

        let t1 = Trade {
            t1: transfer(usr, pool1, 100, token1),
            t2: transfer(pool1, usr, 200, token2),
        };
        let mid = MevAction::Other;
        let t2 = Trade {
            t1: transfer(usr, pool2, 200, token2),
            t2: transfer(pool2, usr, 110, token1),
        };
        let out = reduce_arbitrages(&[MevAction::Trade(t1), mid, MevAction::Trade(t2)]);
        assert_eq!(
            out[0],
            ArbClassification::Arbitrage {
                profit: U256::from(10u64),
                token: token1,
                to: usr,
            }
        );
        assert_eq!(out[1], ArbClassification::Pruned);
        assert_eq!(out[2], ArbClassification::Pruned);
    }
}
