"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.readAgentCard = readAgentCard;
exports.agentCardRevision = agentCardRevision;
exports.moneyLabel = moneyLabel;
function object(x) {
    if (!x || typeof x !== "object" || Array.isArray(x))
        throw new Error("expected_object");
    return x;
}
function string(x, key) {
    if (typeof x !== "string" || !x.length)
        throw new Error(`missing_${key}`);
    return x;
}
function nullable(x, key) {
    return x === null || x === undefined ? null : string(x, key);
}
function money(x, key) {
    const s = nullable(x, key);
    if (s !== null && (s.length > 512 || !/^-?\d+(?:\.\d+)?$/.test(s)))
        throw new Error(`invalid_${key}`);
    return s;
}
/** Does not change precision by converting USD strings through Number(). */
function readAgentCard(input) {
    const w = object(input);
    if (w.contract_version !== "arbx.cartridge.agent/4")
        throw new Error("unsupported_contract");
    if (w.approved_for_execution !== false)
        throw new Error("cartridge_self_authorization");
    const simulation = w.simulation == null
        ? { status: "NOT_RUN_BY_CARTRIDGE", passed: null }
        : object(w.simulation);
    if (simulation.passed !== null && typeof simulation.passed !== "boolean")
        throw new Error("invalid_simulation_status");
    if (!Array.isArray(w.observations))
        throw new Error("missing_observations");
    const legs = w.legs == null ? [] : w.legs;
    if (!Array.isArray(legs))
        throw new Error("invalid_legs");
    const result = {
        contract_version: "arbx.cartridge.agent/4",
        mev_id: string(w.mev_id, "mev_id"), detector_id: string(w.detector_id, "detector_id"),
        context_id: string(w.context_id, "context_id"), status: string(w.status, "status"),
        candidate_eligible: w.candidate_eligible === true, approved_for_execution: false,
        plan_hash: nullable(w.plan_hash, "plan_hash"), snapshot_id: nullable(w.snapshot_id, "snapshot_id"),
        price_revision: nullable(w.price_revision, "price_revision"), policy_revision: nullable(w.policy_revision, "policy_revision"),
        amount_in_raw: nullable(w.amount_in_raw, "amount_in_raw"),
        gross_profit_usd: money(w.gross_profit_usd, "gross_profit_usd"), net_profit_usd: money(w.net_profit_usd, "net_profit_usd"),
        economic_kind: nullable(w.economic_kind, "economic_kind"),
        legs: legs.map(x => object(x)), observations: w.observations,
        simulation: simulation, reason: string(w.reason, "reason"), wire: w,
    };
    if (result.candidate_eligible && (!result.plan_hash || !result.snapshot_id || !result.amount_in_raw || !result.net_profit_usd || !result.legs.length))
        throw new Error("candidate_missing_binding_or_ledger");
    if (result.amount_in_raw !== null && !/^(0|[1-9]\d*)$/.test(result.amount_in_raw))
        throw new Error("invalid_raw_amount");
    return result;
}
/** Immutable plan/version identity; never merge old simulation into a new size. */
function agentCardRevision(card) {
    return JSON.stringify([card.mev_id, card.context_id, card.plan_hash, card.snapshot_id, card.price_revision, card.policy_revision, card.amount_in_raw]);
}
/** Missing is diagnostic, not N/A and not zero. A caller may show N/A only from
 * explicit field-level applicability data supplied by the producer. */
function moneyLabel(value, reason) {
    return value === null ? `Dato pendiente: ${reason}` : `${value} USD`;
}
