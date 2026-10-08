//! Persistence for sim-ctl: writes a row per simulation attempt and updates
//! the opportunity state. Transactional so both land or neither does.

use anyhow::{Context, Result};
use shared_rs::contracts::{SimulationResult, SimulatorKind};
use sqlx::postgres::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;

/// Insert one simulation attempt. Returns `true` when a NEW row landed, or
/// `false` when the `(opportunity_id, simulator='revm')` row already existed
/// (SIMWIRE-02c redelivery idempotency: XAUTOCLAIM redelivers an entry whose
/// final XACK failed after persist+XADD succeeded — the caller must skip the
/// downstream XADD so the opportunity is published exactly once).
pub async fn insert_simulation(pool: &PgPool, r: &SimulationResult) -> Result<bool> {
    let sim_str = simulator_str(&r.simulator);
    let gas_est: Option<BigDecimal> = r
        .gas_estimate_wei
        .as_deref()
        .and_then(|s| BigDecimal::from_str(s).ok());
    let gas_price: Option<BigDecimal> = r
        .gas_price_wei
        .as_deref()
        .and_then(|s| BigDecimal::from_str(s).ok());

    let mut tx = pool.begin().await.context("begin tx")?;

    let inserted = sqlx::query(
        r#"
        INSERT INTO simulations (
            opportunity_id, simulator, gas_estimate_wei, gas_price_wei,
            slippage_pct, revert_risk_pct, simulated_profit_usd,
            passed, fail_reason, trace_id, simulated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        ON CONFLICT (opportunity_id) WHERE simulator = 'revm' DO NOTHING
        "#,
    )
    .bind(r.opportunity_id)
    .bind(sim_str)
    .bind(gas_est)
    .bind(gas_price)
    .bind(r.slippage_pct)
    .bind(r.revert_risk_pct)
    .bind(r.simulated_profit_usd)
    .bind(r.passed)
    .bind(r.fail_reason.as_deref())
    .bind(r.trace_id)
    .bind(r.simulated_at)
    .execute(&mut *tx)
    .await
    .context("insert simulation")?;

    if inserted.rows_affected() == 0 {
        // Prior delivery already persisted this revm verdict (and published
        // it — only the final XACK failed). Skip the status update too: the
        // row already carries the state transition.
        tx.commit()
            .await
            .context("commit tx (duplicate redelivery)")?;
        return Ok(false);
    }

    // Advance opportunity state. 'simulated' if passed, else 'rejected' —
    // UNLESS the failure is a sim-capability gap (unsupported strategy/chain,
    // no fork configured). A capability gap means our sim engine can't verify
    // this strategy yet; it is NOT an opportunity-quality rejection. Per
    // EXECUTION_MODES_DOCTRINE §34, the live terminus (relays-client) gates
    // execution independently (default-deny mainnet), so paper/shadow may
    // surface detection without requiring the sim terminus. Keeping the
    // opportunity at its pre-sim status leaves it viable (rejection_reason
    // stays NULL) so the dashboard shows real detections; the simulation row
    // inserted above still records the SIM_SKIP outcome honestly.
    let sim_capability_gap = r
        .fail_reason
        .as_deref()
        .map(is_sim_capability_gap)
        .unwrap_or(false);

    if !r.passed && sim_capability_gap {
        // Commit the simulation row (already inserted) but do NOT flip the
        // opportunity to 'rejected'. The opp stays 'detected'/'validated'.
        tx.commit()
            .await
            .context("commit tx (sim capability gap)")?;
        return Ok(true);
    }

    let next_status = if r.passed { "simulated" } else { "rejected" };
    let reject_reason = if r.passed {
        None
    } else {
        r.fail_reason.clone()
    };
    sqlx::query(
        r#"
        UPDATE opportunities
           SET status = $2,
               rejection_reason = COALESCE($3, rejection_reason),
               updated_at = NOW()
         WHERE id = $1
           AND status IN ('validated','scored','detected')
        "#,
    )
    .bind(r.opportunity_id)
    .bind(next_status)
    .bind(reject_reason)
    .execute(&mut *tx)
    .await
    .context("update opportunity status")?;

    tx.commit().await.context("commit tx")?;
    Ok(true)
}

fn simulator_str(k: &SimulatorKind) -> &'static str {
    match k {
        SimulatorKind::Anvil => "anvil",
        SimulatorKind::Tenderly => "tenderly",
        SimulatorKind::Hardhat => "hardhat",
        SimulatorKind::Revm => "revm",
        SimulatorKind::NotImplemented => "not_implemented",
    }
}

/// Classify a sim `fail_reason` as a *capability gap* (the sim engine cannot
/// run this strategy/chain/fork) rather than a genuine opportunity-quality
/// failure (revert, gas exceeded, etc.). Capability gaps must NOT reject the
/// opportunity — see `insert_simulation`. The strings mirror the
/// `not_implemented` outcomes produced in `sim_engine.rs`.
fn is_sim_capability_gap(fail_reason: &str) -> bool {
    fail_reason.starts_with("strategy_not_simulatable")
        // BR-00 (2026-09-07): cyclic-route structural gap (single-hop S4
        // probe cannot represent a closed route) -- same non-rejecting
        // semantics as the kind gap above.
        || fail_reason.starts_with("strategy_cyclic_route_not_simulatable")
        // SIM4-CYCLIC-03 (F-01 de la verificación independiente t91): la familia
        // del gap cíclico fue RENOMBRADA. SIM4-CYCLIC-01 la movió de
        // `strategy_cyclic_route_not_simulatable_*` (que afirmaba una
        // imposibilidad FALSA) a `cyclic_route_missing_route_metadata:<kind>`
        // (que nombra el DATO que falta: la ruta). El clasificador — que NO
        // tiene catch-all — se quedó con el nombre viejo, así que la familia
        // nueva caía fuera y un gap de CAPACIDAD se habría clasificado como
        // fallo de CALIDAD, flipeando la oportunidad a `rejected`.
        //
        // Ese es el modo de fallo que SIMWIRE-02 prohíbe explícitamente aquí
        // arriba, y es PEOR que la negativa anterior: la ruta deja de ser "no
        // simulable" y pasa a ser "rechazada en silencio", sin rastro de la
        // causa. Las dos familias se mantienen reconocidas: la vieja por las
        // filas históricas, la nueva por las que produce el arreglo.
        || fail_reason.starts_with("cyclic_route_missing_route_metadata")
        // SIM4-CYCLIC-04 (F9 de t91): un path PRESENTE pero incoherente con
        // `token_in`/`token_out` no se simula con una pata inventada — se declara
        // con su propio nombre. Esa familia entra acá por el MISMO motivo que la
        // anterior: una familia de razones que no esté en este clasificador
        // convierte un gap de capacidad en un rechazo silencioso (F-01). Si se
        // renombra o se añade otra, hay que añadirla TAMBIÉN acá.
        || fail_reason.starts_with("route_path_not_representable")
        || fail_reason.starts_with("anvil_fork_not_configured")
        || fail_reason.contains("_not_supported_in_s4")
        // SIMWIRE-02 (P1 safety net): typed B2c/stream gaps. Absence of
        // capability must NEVER become an opportunity-quality rejection —
        // otherwise a flipped SIM_BACKEND=revm structurally drains the
        // validated stream into permanent `rejected` rows. Families:
        //   route_encoding_not_available  — legacy RevmBackend empty calldata
        //   route_metadata_not_available  — row lacks route topology
        //   real_sim_unavailable / real_sim_env_missing — B2c env incomplete
        //   candidate_incomplete:*       — S4-02 STRUCTURAL: the row lacks
        //                                   what the encoder needs; retry
        //                                   cannot change the row
        //   b2c_encode_failed:*          — router not in the encoder catalog
        || fail_reason.starts_with("route_encoding_not_available")
        || fail_reason.starts_with("route_metadata_not_available")
        || fail_reason.starts_with("real_sim_unavailable")
        || fail_reason.starts_with("real_sim_env_missing")
        || fail_reason.starts_with("candidate_incomplete")
        || fail_reason.starts_with("b2c_encode_failed")
        // SIMWIRE-02c P1: harness/config failures are NOT market verdicts.
        // These are DETERMINISTIC given the pinned block+calldata (retrying
        // cannot change them), so they persist as typed non-rejecting gap
        // rows instead of either rejecting the opportunity or spinning the
        // PEL forever. The NON-deterministic infra failures (RPC state
        // fetch, runtime join) stay transient in the consumer (PEL).
        //   multistep_flashloan_executor_unresolved — env config gap (A3 path)
        //   multistep_call_infra:*        — REVM harness: transact_infra /
        //                                   evm_db_missing / storage_override
        //   multistep_apply_storage_failed / multistep_empty_calldata
        //   *_halted / *_decode_failed / amounts_out_empty_array — view-call
        //                                   frame/decode faults (harness-level)
        || fail_reason.starts_with("multistep_flashloan_executor_unresolved")
        || fail_reason.starts_with("multistep_call_infra")
        || fail_reason.starts_with("multistep_apply_storage_failed")
        || fail_reason.starts_with("multistep_empty_calldata")
        || fail_reason.contains("balance_read_halted")
        || fail_reason.contains("amounts_out_halted")
        || fail_reason.contains("balance_decode_failed")
        || fail_reason.contains("amounts_out_decode_failed")
        || fail_reason.contains("amounts_out_empty_array")
        // BR-00 (2026-09-07): the anvil harness could not decode its OWN
        // probe output -- absence of measurement, never a market verdict (R8).
        || fail_reason == "output_undecodable"
        // SIM-FUND-02b (2026-10-08): la familia de FONDEO del fork.
        //
        // #850 instrumento `signer_funding.rs` para separar las dos causas
        // fisicas de `slot_unresolved` y añadio tres etiquetas nuevas
        // (`write_rejected`, `verify_mismatch`, `balance_unreadable`). Lo que
        // hay que decir sin adorno es QUE SON: etiquetas de la metrica
        // `arbx_sim_funding_total{outcome=...}`. NUNCA se devuelven como
        // `Err(...)`; el unico `fail_reason` del agotamiento del loop sigue
        // siendo `sim_signer_funding_slot_unresolved` (:204). La particion
        // ocurrio en la capa de METRICA, no en la de `fail_reason`.
        //
        // Se reconocen igual, y el motivo es prospectivo y verificable: son
        // fallos de INFRAESTRUCTURA. Si algun dia se promueven a `fail_reason`
        // (el paso natural para hacer la metrica accionable), caerian fuera de
        // este clasificador — que NO tiene catch-all — y un gap de capacidad se
        // convertiria en un rechazo silencioso. Eso es F-01 con nombres nuevos,
        // y es exactamente el modo de fallo que SIMWIRE-02 prohibe arriba.
        //
        // Los `fail_reason` que `ensure_funded` SI devuelve hoy tampoco estaban
        // reconocidos, y son el MISMO hueco: `funding_balanceof_timeout/rpc`,
        // `funding_verify_timeout/rpc`, `funding_setstorage_rpc`,
        // `sim_signer_funding_slot_unresolved` y `anvil_setStorageAt[_timeout]`.
        // Medir el balance, verificar el centinela o escribir en el fork es el
        // ARNES, no el mercado: un `eth_call` que no responde no dice nada del
        // spread. (Medido: las 5.458 simulaciones con `fail_reason` de fondeo
        // unen a oportunidades YA `rejected` por el DETECTOR, asi que el
        // `UPDATE` — gateado `WHERE status IN ('validated','scored','detected')`
        // — no toco ninguna fila. El hueco es LATENTE, no vivo. No se afirma un
        // daño que la evidencia no muestra; se cierra antes de que lo haya.)
        //
        // El prefijo va NAMESPACED A PROPOSITO. `funding_` a secas tragaria
        // veredictos REALES de los cartuchos de searcher-rs
        // (`funding_edge_negative`, `funding_differential_within_band`,
        // `funding_direction_unfavorable`): eso seria el fallo OPUESTO —
        // convertir un rechazo legitimo en silencio. El CONTROL del test lo fija.
        || fail_reason.contains("write_rejected")
        || fail_reason.contains("verify_mismatch")
        || fail_reason.contains("balance_unreadable")
        || fail_reason.starts_with("sim_signer_funding")
        || fail_reason.starts_with("funding_balanceof_")
        || fail_reason.starts_with("funding_verify_")
        || fail_reason.starts_with("funding_setstorage_")
        || fail_reason.starts_with("anvil_setStorageAt")
}

#[cfg(test)]
mod simwire02_classifier_tests {
    use super::is_sim_capability_gap;

    #[test]
    fn legacy_gap_families_stay_gaps() {
        for reason in [
            "strategy_not_simulatable:mev_backrun",
            "anvil_fork_not_configured",
            "strategy_not_supported_in_s4",
        ] {
            assert!(is_sim_capability_gap(reason), "{reason} must stay a gap");
        }
    }

    /// BR-00 (2026-09-07): the two new structural-gap families plus the
    /// undecodable-output harness gap must classify as gaps (opportunity NOT
    /// rejected) -- the simulator shape limits are not market verdicts.
    #[test]
    fn br00_structural_gap_families_are_gaps() {
        for reason in [
            "strategy_not_simulatable_in_s4:liquidation",
            "strategy_not_simulatable_in_s4:liquidation_snipe",
            "strategy_cyclic_route_not_simulatable_in_s4:triangular",
            "strategy_cyclic_route_not_simulatable_in_s4:flashloan_arb",
            "strategy_cyclic_route_not_simulatable_in_s4:mev_01_016_triangular_arbitrage",
            "output_undecodable",
        ] {
            assert!(is_sim_capability_gap(reason), "{reason} must be a gap");
        }
    }

    /// SIM4-CYCLIC-03 (F-01 de t91): the RENAMED cyclic-gap family must be a gap.
    ///
    /// Before this fix the measured result was the opposite — the OLD pattern
    /// returned true and the NEW one returned FALSE, and the classifier has no
    /// catch-all. With `sim_engine.rs` now emitting
    /// `cyclic_route_missing_route_metadata:<kind>`, that gap would have been
    /// classified as an opportunity-QUALITY failure and the opportunity flipped
    /// to `rejected` — exactly what SIMWIRE-02 forbids, and worse than the old
    /// by-name refusal because it leaves no trace of the cause.
    ///
    /// Both families stay recognised: the OLD one for historical rows, the NEW
    /// one for everything the fix now produces. The last two entries are the
    /// CONTROL: a genuine market verdict must NOT be swallowed as a gap.
    #[test]
    fn sim4_cyclic_renamed_gap_family_is_still_a_gap() {
        for reason in [
            "cyclic_route_missing_route_metadata:triangular",
            "cyclic_route_missing_route_metadata:dex_arb",
            "cyclic_route_missing_route_metadata:flashloan_arb",
            "cyclic_route_missing_route_metadata:mev_01_016_triangular_arbitrage",
            // Historical rows keep their family recognised.
            "strategy_cyclic_route_not_simulatable_in_s4:triangular",
            // SIM4-CYCLIC-04 (F9): the OTHER new family must be recognised too,
            // for the same reason — a family missing from this classifier turns
            // a capability gap into a silent rejection (F-01).
            "route_path_not_representable:dex_arb",
            "route_path_not_representable:triangular",
        ] {
            assert!(
                is_sim_capability_gap(reason),
                "{reason} must classify as capability gap, NOT as a quality failure"
            );
        }

        // CONTROL: these are genuine MARKET/orchestration verdicts and must stay
        // OUT of the gap family — otherwise the classifier would stop
        // distinguishing anything and every rejection would become a silence.
        for reason in [
            "v3_quote_unavailable",
            "single_pool_no_spread",
            "non_positive_profit",
            "safety_below_threshold",
            "simulation_failed",
            "score_below_min",
        ] {
            assert!(
                !is_sim_capability_gap(reason),
                "{reason} must NOT be a capability gap — it is a real verdict"
            );
        }
    }

    /// SIMWIRE-02 P1: the structural-drain guard. Every typed B2c/stream
    /// gap must keep the opportunity NON-rejected (status stays
    /// detected/validated, rejection_reason stays NULL).
    #[test]
    fn simwire02_typed_b2c_gaps_are_not_rejections() {
        for reason in [
            "route_encoding_not_available",
            "route_metadata_not_available",
            "real_sim_unavailable: SIM_BACKEND!=revm",
            "real_sim_env_missing: ARBITRAGE_EXECUTOR env var required",
            "candidate_incomplete:token_addresses_empty",
            "candidate_incomplete:missing_decimals_[\"0xabc\"]",
            "candidate_incomplete:amount_in_wei_unparseable",
            "b2c_encode_failed:router_not_in_catalog",
        ] {
            assert!(
                is_sim_capability_gap(reason),
                "{reason} must classify as capability gap, not rejection"
            );
        }
    }

    /// The flip side: genuine opportunity-quality / market verdicts must
    /// still reject — the gap set must not swallow economic truth.
    #[test]
    fn economic_and_market_verdicts_stay_rejections() {
        for reason in [
            "execution_reverted",
            "multistep_call_halt:Revert",
            "multistep_gross_spread_non_positive",
            "stf",
            "gas_floor_breach",
            "net_zero_after_gas",
        ] {
            assert!(
                !is_sim_capability_gap(reason),
                "{reason} must stay a rejection, not a gap"
            );
        }
    }

    /// SIMWIRE-02c P1: deterministic harness/config failures are NOT market
    /// verdicts — they must persist as typed non-rejecting gaps (visible in
    /// the simulations row) instead of rejecting the opportunity.
    #[test]
    fn simwire02c_harness_failures_are_gaps_not_rejections() {
        for reason in [
            "multistep_flashloan_executor_unresolved:Missing { chain_id: 1 }",
            "multistep_call_infra:transact_infra:db commit failed",
            "multistep_call_infra:evm_db_missing",
            "multistep_apply_storage_failed:storage_override_failed",
            "multistep_empty_calldata",
            "multistep_read_balance_failed:balance_of:balance_read_halted",
            "multistep_forward_quote_failed:amounts_out_halted",
            "multistep_read_balance_failed:balance_of:balance_decode_failed",
            "multistep_forward_quote_failed:amounts_out_decode_failed",
            "multistep_forward_quote_failed:amounts_out_empty_array",
        ] {
            assert!(
                is_sim_capability_gap(reason),
                "{reason} must classify as harness gap, not market rejection"
            );
        }
    }

    /// SIMWIRE-02c P1: chain-state verdicts at the pinned block are market
    /// truth — halts of COMMITTED calls and view reverts on the forked state
    /// must keep rejecting (retry at the same block gives the same answer).
    #[test]
    fn simwire02c_chain_state_verdicts_stay_rejections() {
        for reason in [
            "multistep_call_revert:TransferHelper: INSUFFICIENT_OUTPUT",
            "multistep_forward_quote_failed:amounts_out_reverted",
            "multistep_forward_quote_failed:zero_intermediate",
            "multistep_read_balance_failed:balance_of:balance_read_reverted",
        ] {
            assert!(
                !is_sim_capability_gap(reason),
                "{reason} is a chain-state verdict — must stay a rejection"
            );
        }
    }

    /// SIM-FUND-02b: la instrumentacion de #850 y la familia de fondeo.
    ///
    /// Dos grupos, y la distincion importa:
    ///
    /// 1. Las tres etiquetas que #850 añadio en `signer_funding.rs`
    ///    (`write_rejected`, `verify_mismatch`, `balance_unreadable`) son HOY
    ///    valores del label `outcome` de `arbx_sim_funding_total`, NO
    ///    `fail_reason`: `ensure_funded` nunca las devuelve. Se afirman igual
    ///    para que, si se promueven a `fail_reason`, NO entren como fallo de
    ///    calidad. Se incluyen las formas SUFIJADAS porque ese es el modo
    ///    natural de promocionarlas (`<reason>:<causa>`), que es como este
    ///    codebase ya compone los `fail_reason` de `multistep_*` y
    ///    `cyclic_route_missing_route_metadata:<kind>`.
    ///
    /// 2. Los `fail_reason` que `ensure_funded` SI devuelve hoy, leidos del
    ///    codigo (`signer_funding.rs:117,121,136,169,173,183,204,252,256`).
    ///    Ninguno estaba reconocido. Son el mismo modo de fallo: el arnes no
    ///    pudo medir, y ausencia de medicion no es veredicto (R8).
    #[test]
    fn simfund02b_funding_harness_failures_are_gaps_not_rejections() {
        for reason in [
            // --- Grupo 1: etiquetas nuevas de #850 (hoy metricas). ---
            "write_rejected",
            "verify_mismatch",
            "balance_unreadable",
            // Forma sufijada: promocion natural a `fail_reason`.
            "sim_signer_funding_slot_unresolved:write_rejected",
            "sim_signer_funding_slot_unresolved:verify_mismatch",
            "sim_signer_funding_slot_unresolved:balance_unreadable",
            // --- Grupo 2: los `fail_reason` que SI llegan hoy. ---
            "sim_signer_funding_slot_unresolved",
            "funding_balanceof_timeout",
            "funding_balanceof_rpc: error sending request for url (http://anvil:8545/)",
            "funding_verify_timeout",
            "funding_verify_rpc: error decoding response body",
            "funding_setstorage_rpc: error sending request",
            "anvil_setStorageAt_timeout",
            "anvil_setStorageAt: nonce too low",
        ] {
            assert!(
                is_sim_capability_gap(reason),
                "{reason} must classify as harness/funding gap, not market rejection"
            );
        }
    }

    /// CONTROL de no-gaps para SIM-FUND-02b — sin esto, el test de arriba
    /// pasaria igual si el clasificador devolviera `true` para todo.
    ///
    /// El primer bloque es el que justifica que el prefijo vaya NAMESPACED.
    /// `funding_edge_negative`, `funding_differential_within_band` y
    /// `funding_direction_unfavorable` son VEREDICTOS de mercado que EXISTEN en
    /// los cartuchos de `searcher-rs` (verificado por grep). Un
    /// `starts_with("funding_")` a secas los tragaria y convertiria un rechazo
    /// legitimo en un silencio: el fallo OPUESTO, igual de grave.
    ///
    /// El segundo bloque son las razones de mercado mas frecuentes medidas en la
    /// tabla `opportunities` (8.063.209 filas, todas `rejected` con razon del
    /// DETECTOR): `spread_negative_round_trip` (3.306.709), `non_positive_profit`
    /// (1.015.839), `v3_quote_unavailable` (467.390).
    #[test]
    fn simfund02b_funding_prefix_does_not_swallow_market_verdicts() {
        for reason in [
            // Veredictos REALES de los cartuchos que empiezan con `funding_`.
            "funding_edge_negative",
            "funding_differential_within_band",
            "funding_direction_unfavorable",
            "funding_feed_unavailable",
            "funding_component_missing",
            "funding_horizon_unavailable",
            // Veredictos de mercado / chain-state.
            "spread_negative_round_trip",
            "non_positive_profit",
            "v3_quote_unavailable",
            "v3_pool_revert",
            "negative_net_profit",
            "revert",
            "gas_exceeded",
        ] {
            assert!(
                !is_sim_capability_gap(reason),
                "{reason} must stay a REJECTION — swallowing it would be the opposite failure"
            );
        }
    }
}
