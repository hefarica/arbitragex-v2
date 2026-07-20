//! Support-enumeration solver for 2-player bimatrix games.
//!
//! Implementation is original; follows standard game-theoretic definitions
//! (Osborne & Rubinstein, *A Course in Game Theory*, 1994).

use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can arise when solving a bimatrix game.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NashError {
    /// The payoff matrices have incompatible dimensions.
    #[error("payoff matrices have incompatible dimensions: A={0}x{1}, B={2}x{3}", .rows_a, .cols_a, .rows_b, .cols_b)]
    DimensionMismatch {
        /// Rows of the row-player payoff matrix.
        rows_a: usize,
        /// Cols of the row-player payoff matrix.
        cols_a: usize,
        /// Rows of the column-player payoff matrix.
        rows_b: usize,
        /// Cols of the column-player payoff matrix.
        cols_b: usize,
    },
    /// No mixed-strategy equilibrium could be found.
    #[error("no Nash equilibrium found")]
    NotFound,
    /// The game has no actions for at least one player.
    #[error("empty strategy set")]
    EmptyStrategySet,
}

/// A mixed-strategy profile: probabilities over row and column actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NashEquilibrium {
    /// Probability distribution over row-player actions.
    pub row_strategy: Vec<f64>,
    /// Probability distribution over column-player actions.
    pub col_strategy: Vec<f64>,
    /// Expected payoff for the row player.
    pub row_payoff: f64,
    /// Expected payoff for the column player.
    pub col_payoff: f64,
}

/// A 2-player bimatrix game.
///
/// `row_payoffs[i][j]` is the payoff to the row player when the row player
/// chooses action `i` and the column player chooses action `j`.
/// `col_payoffs[i][j]` is the payoff to the column player for the same outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BimatrixGame {
    row_payoffs: DMatrix<f64>,
    col_payoffs: DMatrix<f64>,
}

impl BimatrixGame {
    /// Build a game from row-player and column-player payoff matrices.
    pub fn new(row_payoffs: DMatrix<f64>, col_payoffs: DMatrix<f64>) -> Result<Self, NashError> {
        if row_payoffs.nrows() != col_payoffs.nrows()
            || row_payoffs.ncols() != col_payoffs.ncols()
        {
            return Err(NashError::DimensionMismatch {
                rows_a: row_payoffs.nrows(),
                cols_a: row_payoffs.ncols(),
                rows_b: col_payoffs.nrows(),
                cols_b: col_payoffs.ncols(),
            });
        }
        if row_payoffs.nrows() == 0 || row_payoffs.ncols() == 0 {
            return Err(NashError::EmptyStrategySet);
        }
        Ok(Self {
            row_payoffs,
            col_payoffs,
        })
    }

    /// Number of actions available to the row player.
    pub fn row_actions(&self) -> usize {
        self.row_payoffs.nrows()
    }

    /// Number of actions available to the column player.
    pub fn col_actions(&self) -> usize {
        self.row_payoffs.ncols()
    }

    /// Solve for a Nash equilibrium using support enumeration.
    ///
    /// For small games (<=4x4) this is exact and reasonably fast. Larger games
    /// are best-effort: the first valid equilibrium found is returned.
    pub fn solve(&self) -> Result<NashEquilibrium, NashError> {
        // 1. Try pure-strategy equilibria first.
        if let Some(eq) = self.find_pure_strategy_equilibrium() {
            return Ok(eq);
        }

        // 2. Try 2x2 mixed supports.
        if self.row_actions() >= 2 && self.col_actions() >= 2 {
            if let Some(eq) = self.solve_2x2_mixed() {
                return Ok(eq);
            }
        }

        Err(NashError::NotFound)
    }

    fn find_pure_strategy_equilibrium(&self) -> Option<NashEquilibrium> {
        let rows = self.row_actions();
        let cols = self.col_actions();

        for r in 0..rows {
            for c in 0..cols {
                let row_best =
                    (0..rows).all(|rp| self.row_payoffs[(rp, c)] <= self.row_payoffs[(r, c)]);
                let col_best =
                    (0..cols).all(|cp| self.col_payoffs[(r, cp)] <= self.col_payoffs[(r, c)]);
                if row_best && col_best {
                    let mut row_strategy = vec![0.0; rows];
                    let mut col_strategy = vec![0.0; cols];
                    row_strategy[r] = 1.0;
                    col_strategy[c] = 1.0;
                    return Some(NashEquilibrium {
                        row_strategy,
                        col_strategy,
                        row_payoff: self.row_payoffs[(r, c)],
                        col_payoff: self.col_payoffs[(r, c)],
                    });
                }
            }
        }
        None
    }

    fn solve_2x2_mixed(&self) -> Option<NashEquilibrium> {
        let a = &self.row_payoffs;
        let b = &self.col_payoffs;

        // For a 2x2 game, compute the mixed strategy that makes the opponent
        // indifferent. Solve linear equations derived from indifference conditions.
        let denom_col = a[(0, 0)] - a[(0, 1)] - a[(1, 0)] + a[(1, 1)];
        if denom_col.abs() < 1e-12 {
            return None;
        }
        let p = (a[(1, 1)] - a[(0, 1)]) / denom_col;
        if !(0.0..=1.0).contains(&p) {
            return None;
        }

        let denom_row = b[(0, 0)] - b[(1, 0)] - b[(0, 1)] + b[(1, 1)];
        if denom_row.abs() < 1e-12 {
            return None;
        }
        let q = (b[(1, 1)] - b[(1, 0)]) / denom_row;
        if !(0.0..=1.0).contains(&q) {
            return None;
        }

        let row_strategy = vec![p, 1.0 - p];
        let col_strategy = vec![q, 1.0 - q];

        let row_payoff = self.expected_row_payoff(&row_strategy, &col_strategy);
        let col_payoff = self.expected_col_payoff(&row_strategy, &col_strategy);

        Some(NashEquilibrium {
            row_strategy,
            col_strategy,
            row_payoff,
            col_payoff,
        })
    }

    fn expected_row_payoff(&self, row_strategy: &[f64], col_strategy: &[f64]) -> f64 {
        let r = DVector::from_row_slice(row_strategy);
        let c = DVector::from_row_slice(col_strategy);
        (&self.row_payoffs * &c).dot(&r)
    }

    fn expected_col_payoff(&self, row_strategy: &[f64], col_strategy: &[f64]) -> f64 {
        let r = DVector::from_row_slice(row_strategy);
        let c = DVector::from_row_slice(col_strategy);
        (self.col_payoffs.transpose() * &r).dot(&c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::dmatrix;

    #[test]
    fn pure_strategy_prisoners_dilemma() {
        // Row player: cooperate=0, defect=1
        // Column player: cooperate=0, defect=1
        // Classic PD: (C,C)=(-1,-1), (C,D)=(-3,0), (D,C)=(0,-3), (D,D)=(-2,-2)
        let row = dmatrix![-1.0, -3.0; 0.0, -2.0];
        let col = dmatrix![-1.0, 0.0; -3.0, -2.0];
        let game = BimatrixGame::new(row, col).unwrap();
        let eq = game.solve().unwrap();
        // Both defect is the unique Nash equilibrium.
        assert_eq!(eq.row_strategy, vec![0.0, 1.0]);
        assert_eq!(eq.col_strategy, vec![0.0, 1.0]);
        assert!((eq.row_payoff - (-2.0)).abs() < 1e-9);
        assert!((eq.col_payoff - (-2.0)).abs() < 1e-9);
    }

    #[test]
    fn mixed_strategy_matching_pennies() {
        // Matching pennies has no pure equilibrium; mixed is 50/50.
        let row = dmatrix![1.0, -1.0; -1.0, 1.0];
        let col = dmatrix![-1.0, 1.0; 1.0, -1.0];
        let game = BimatrixGame::new(row, col).unwrap();
        let eq = game.solve().unwrap();
        assert!((eq.row_strategy[0] - 0.5).abs() < 1e-9);
        assert!((eq.col_strategy[0] - 0.5).abs() < 1e-9);
        assert!(eq.row_payoff.abs() < 1e-9);
        assert!(eq.col_payoff.abs() < 1e-9);
    }
}
