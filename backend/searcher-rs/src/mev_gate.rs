//! MEV Ethics Gate — canonical decision-tree validator
//!
//! Implements the decision tree from arbx-mev-ethics-gate SKILL.md.
//! This is the SINGLE SOURCE OF TRUTH for MEV permit/prohibit logic.
//! Tests import this module, not the other way around.

use std::fmt;

/// Gate verdict: permit or prohibit a strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    Permitted { reason: String },
    Prohibited { reason: String },
}

impl fmt::Display for GateVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateVerdict::Permitted { reason } => write!(f, "✅ PERMITTED: {}", reason),
            GateVerdict::Prohibited { reason } => write!(f, "❌ PROHIBITED: {}", reason),
        }
    }
}

/// Decision tree: permit or prohibit a strategy based on 3 binary questions
///
/// Reference: arbx-mev-ethics-gate SKILL.md — Decision Matriz de Zona Gris
///
/// Decision 1: Does the strategy depend on a SPECIFIC pending user transaction?
///   - YES → PROHIBITED (sandwich/frontrun/JIT/predatory)
///   - NO → Continue to Decision 2
///
/// Decision 2: Is the strategy's profit WORSE for the victim user than doing nothing?
///   - YES → PROHIBITED (extracts user value)
///   - NO → Continue to Decision 3
///
/// Decision 3: Does the protocol explicitly permit this mechanism in its docs/DAO?
///   - YES → PERMITTED (liquidations, MEV-Share, public relays)
///   - NO → PROHIBITED (unauthorized value extraction)
pub fn apply_mev_gate(
    depends_on_specific_pending_tx: bool,
    worsens_user_outcome: bool,
    protocol_explicitly_permits: bool,
) -> GateVerdict {
    // Decision 1: Mempool-specific dependency
    if depends_on_specific_pending_tx {
        return GateVerdict::Prohibited {
            reason: "Strategy depends on specific pending user transaction (sandwich/frontrun/JIT)".to_string(),
        };
    }

    // Decision 2: User outcome check
    if worsens_user_outcome {
        return GateVerdict::Prohibited {
            reason: "Strategy extracts value from user outcome".to_string(),
        };
    }

    // Decision 3: Protocol consent
    if protocol_explicitly_permits {
        return GateVerdict::Permitted {
            reason: "Protocol explicitly permits this mechanism (liquidation bonus / MEV-Share)".to_string(),
        };
    }

    GateVerdict::Prohibited {
        reason: "Strategy not authorized by protocol".to_string(),
    }
}

/// Example strategies for reference
#[derive(Debug, Clone)]
pub struct JitV3 {
    pub depends_on_specific_tx: bool,
}

#[derive(Debug, Clone)]
pub struct CrossDexArbitrage {
    pub depends_on_specific_tx: bool,
}

#[derive(Debug, Clone)]
pub struct LiquidationBackrun {
    pub protocol_permits: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_v3_prohibited() {
        let verdict = apply_mev_gate(true, false, false); // depends=true → PROHIBITED
        assert_eq!(
            verdict,
            GateVerdict::Prohibited {
                reason: "Strategy depends on specific pending user transaction (sandwich/frontrun/JIT)".to_string()
            }
        );
    }

    #[test]
    fn test_cross_dex_arbitrage_permitted() {
        let verdict = apply_mev_gate(false, false, false); // depends=false, not user-extract, no protocol → Prohibited
        assert_eq!(
            verdict,
            GateVerdict::Prohibited {
                reason: "Strategy not authorized by protocol".to_string()
            }
        );
        // Note: Cross-DEX arb is TECHNICALLY in gray zone (Mempool independence ✓, no user worse-off ✓)
        // but absent explicit protocol consent, defaults to prohibited. If DEXes consent, flip protocol_permits.
    }

    #[test]
    fn test_liquidation_permitted() {
        let verdict = apply_mev_gate(false, false, true); // depends=false, user ok, protocol permits → PERMITTED
        assert_eq!(
            verdict,
            GateVerdict::Permitted {
                reason: "Protocol explicitly permits this mechanism (liquidation bonus / MEV-Share)".to_string()
            }
        );
    }
}
