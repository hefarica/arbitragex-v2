//! MEV Ethics Gate Test Suite
//!
//! Imports the canonical gate from src/mev_gate.rs (not mocks).
//! Validates that prohibited MEV patterns (sandwich, frontrun, JIT displacement)
//! are correctly REJECTED by the gate decision-tree.

#[cfg(test)]
mod mev_ethics_gate_tests {
    use searcher_rs::mev_gate::{apply_mev_gate, GateVerdict};

    /// Test 1: JIT V3 strategy is PROHIBITED (depends on specific pending tx)
    #[test]
    fn test_jit_v3_prohibited_by_gate() {
        // JIT V3: watches mempool for large swap, mints tight LP before it,
        // captures disproportionate fees, burns after. DEPENDS ON SPECIFIC TX.
        let verdict = apply_mev_gate(
            true,   // depends_on_specific_pending_tx = true
            false,  // worsens_user_outcome = false (LP gets market rate)
            false,  // protocol_explicitly_permits = false (Uniswap V3 doesn't consent to this)
        );

        assert_eq!(
            verdict,
            GateVerdict::Prohibited {
                reason: "Strategy depends on specific pending user transaction (sandwich/frontrun/JIT)".to_string()
            },
            "JIT V3 should be PROHIBITED: reads mempool targeting specific user tx"
        );
    }

    /// Test 2: Cross-DEX arbitrage is PROHIBITED (no protocol consent)
    #[test]
    fn test_cross_dex_arbitrage_gray_zone() {
        // Cross-DEX arb: exploits price divergence between Uniswap V2 and V3.
        // Does NOT depend on specific pending tx, does NOT worsen user outcome,
        // but absent explicit protocol consent, defaults to PROHIBITED.
        let verdict = apply_mev_gate(
            false,  // depends_on_specific_pending_tx = false
            false,  // worsens_user_outcome = false
            false,  // protocol_explicitly_permits = false (DEXes don't consent)
        );

        assert_eq!(
            verdict,
            GateVerdict::Prohibited {
                reason: "Strategy not authorized by protocol".to_string()
            },
            "Cross-DEX arb in gray zone: no mempool dep, no user harm, but no consent"
        );
    }

    /// Test 3: Liquidation backrun is PERMITTED (protocol explicitly permits)
    #[test]
    fn test_liquidation_backrun_permitted() {
        // Aave liquidation bonus: protocol explicitly incentivizes liquidators.
        // Does NOT depend on specific user tx (works on any underwater position),
        // improves user outcome (prevents bad debt), protocol consents (publishes bonus).
        let verdict = apply_mev_gate(
            false,  // depends_on_specific_pending_tx = false
            false,  // worsens_user_outcome = false (actually improves)
            true,   // protocol_explicitly_permits = true (Aave pays 5% bonus)
        assert_eq!(
            verdict,
            GateVerdict::Permitted {
                reason: "Liquidation in permissionless-by-design protocol (Aave)".to_string()
            }
        );
    }

    /// Test 5: Sandwich attack is PROHIBITED (no exceptions)
    #[test]
    fn test_sandwich_attack_prohibited() {
        let strategy = SandwichAttack {
            name: "sandwich_pool_swap".to_string(),
            pattern: "buy_front_sell_back".to_string(),
            targets_specific_user_tx: true,
            gives_user_worse_outcome: true,
        };

        let verdict = apply_mev_gate(&strategy);
        // Gate decision tree: Decision #1 (depends on specific pending tx) fails first
        assert_eq!(
            verdict,
            GateVerdict::Prohibited {
                reason: "Strategy depends on specific pending user transaction".to_string()
            }
        );
    }

    /// Test 6: Frontrunning of user tx is PROHIBITED
    #[test]
    fn test_frontrunning_prohibited() {
        let strategy = FrontrunStrategy {
            name: "frontrun_arbitrage".to_string(),
            reads_pending_user_tx: true,
            orders_before_user_tx: true,
            gives_user_worse_fill: true,
        };

        let verdict = apply_mev_gate(&strategy);
        // Gate decision tree: Decision #1 (depends on specific pending tx) fails first
        assert_eq!(
            verdict,
            GateVerdict::Prohibited {
                reason: "Strategy depends on specific pending user transaction".to_string()
            }
        );
    }

        assert_eq!(
            verdict,
            GateVerdict::Permitted {
                reason: "Protocol explicitly permits this mechanism (liquidation bonus / MEV-Share)".to_string()
            },
            "Liquidation backrun should be PERMITTED: protocol explicitly consents via bonus"
        );
    }
}

    fn apply_mev_gate(strategy: &dyn MevStrategy) -> GateVerdict {
        // Gate decision tree from SKILL.md §Protocolo de zona gris

        // Decision 1: ¿La estrategia sería no-rentable sin la tx pendiente específica?
        if strategy.profit_depends_on_specific_user_tx() {
            return GateVerdict::Prohibited {
                reason: "Strategy depends on specific pending user transaction".to_string(),
            };
        }

        // Decision 2: ¿Da a algún usuario un peor outcome?
        if strategy.gives_user_worse_outcome() {
            return GateVerdict::Prohibited {
                reason: "Strategy gives specific user worse outcome".to_string(),
            };
        }

        // Decision 3: ¿Paga a builder/relay para ser ordenada relativamente a tx de usuario?
        if strategy.pays_builder_for_relative_ordering() {
            return GateVerdict::Prohibited {
                reason: "Strategy pays builder/relay for relative ordering to user tx".to_string(),
            };
        }

        // Check PERMITTED list
        if strategy.is_permitted() {
            GateVerdict::Permitted {
                reason: strategy.permitted_reason().to_string(),
            }
        } else {
            // Gray zone default: PROHIBITED
            GateVerdict::Prohibited {
                reason: "Strategy not in PERMITTED list; gray zone defaults to PROHIBITED".to_string(),
            }
        }
    }

    trait MevStrategy {
        fn profit_depends_on_specific_user_tx(&self) -> bool;
        fn gives_user_worse_outcome(&self) -> bool;
        fn pays_builder_for_relative_ordering(&self) -> bool;
        fn is_permitted(&self) -> bool;
        fn permitted_reason(&self) -> &str;
    }

    impl MevStrategy for JitV3Strategy {
        fn profit_depends_on_specific_user_tx(&self) -> bool {
            self.profit_depends_on_specific_user_tx
        }
        fn gives_user_worse_outcome(&self) -> bool {
            self.displaces_third_party_lp_fees
        }
        fn pays_builder_for_relative_ordering(&self) -> bool {
            false
        }
        fn is_permitted(&self) -> bool {
            false
        }
        fn permitted_reason(&self) -> &str {
            ""
        }
    }

    impl MevStrategy for CrossDexArbitrage {
        fn profit_depends_on_specific_user_tx(&self) -> bool {
            self.depends_on_specific_user_tx
        }
        fn gives_user_worse_outcome(&self) -> bool {
            self.extracts_from_user
        }
        fn pays_builder_for_relative_ordering(&self) -> bool {
            false
        }
        fn is_permitted(&self) -> bool {
            !self.reads_mempool && !self.depends_on_specific_user_tx
        }
        fn permitted_reason(&self) -> &str {
            "Cross-pool arbitrage on public on-chain data"
        }
    }

    impl MevStrategy for LiquidationBackrun {
        fn profit_depends_on_specific_user_tx(&self) -> bool {
            false  // Liquidations are protocol-permitted, not user-specific
        }
        fn gives_user_worse_outcome(&self) -> bool {
            false  // User is already insolvent; liquidation is incentivized
        }
        fn pays_builder_for_relative_ordering(&self) -> bool {
            false
        }
        fn is_permitted(&self) -> bool {
            self.published_bonus.is_some()
        }
        fn permitted_reason(&self) -> &str {
            "Liquidation in permissionless-by-design protocol (Aave)"
        }
    }

    impl MevStrategy for SandwichAttack {
        fn profit_depends_on_specific_user_tx(&self) -> bool {
            self.targets_specific_user_tx
        }
        fn gives_user_worse_outcome(&self) -> bool {
            self.gives_user_worse_outcome
        }
        fn pays_builder_for_relative_ordering(&self) -> bool {
            true  // Sandwiches require specific ordering
        }
        fn is_permitted(&self) -> bool {
            false
        }
        fn permitted_reason(&self) -> &str {
            ""
        }
    }

    impl MevStrategy for FrontrunStrategy {
        fn profit_depends_on_specific_user_tx(&self) -> bool {
            self.reads_pending_user_tx
        }
        fn gives_user_worse_outcome(&self) -> bool {
            self.gives_user_worse_fill
        }
        fn pays_builder_for_relative_ordering(&self) -> bool {
            true
        }
        fn is_permitted(&self) -> bool {
            false
        }
        fn permitted_reason(&self) -> &str {
            ""
        }
    }
}
