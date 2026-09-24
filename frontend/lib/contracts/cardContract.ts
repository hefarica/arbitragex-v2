/** Presentation contract only. Does not recompute or simulate any economy.
 * Install on the v4 path; missing money is never `?? 0` and quote != Sim PASS.
 * Store/PG/API must preserve this same payload and revision without coercion.
 */
export interface AgentLeg {
  index?: number;
  token_in?: string;
  token_out?: string;
  amount_in_raw?: string;
  amount_out_raw?: string;
  snapshot_id?: string;
  quote_id?: string;
  [key: string]: unknown;
}
export interface AgentCard {
  contract_version: "arbx.cartridge.agent/4";
  mev_id: string;
  detector_id: string;
  context_id: string;
  status: string;
  candidate_eligible: boolean;
  approved_for_execution: false;
  plan_hash: string | null;
  snapshot_id: string | null;
  price_revision: string | null;
  policy_revision: string | null;
  amount_in_raw: string | null;
  gross_profit_usd: string | null;
  net_profit_usd: string | null;
  economic_kind: string | null;
  legs: AgentLeg[];
  observations: unknown[];
  simulation: { status: string; passed: boolean | null; [key: string]: unknown };
  reason: string;
  wire: Record<string, unknown>;
}
function object(x: unknown): Record<string, unknown> {
  if (!x || typeof x !== "object" || Array.isArray(x)) throw new Error("expected_object");
  return x as Record<string, unknown>;
}
function string(x: unknown, key: string): string {
  if (typeof x !== "string" || !x.length) throw new Error(`missing_${key}`);
  return x;
}
function nullable(x: unknown, key: string): string | null {
  return x === null || x === undefined ? null : string(x, key);
}
function money(x: unknown, key: string): string | null {
  const s = nullable(x, key);
  if (s !== null && (s.length > 512 || !/^-?\d+(?:\.\d+)?$/.test(s))) throw new Error(`invalid_${key}`);
  return s;
}
/** Does not change precision by converting USD strings through Number(). */
export function readAgentCard(input: unknown): AgentCard {
  const w = object(input);
  if (w.contract_version !== "arbx.cartridge.agent/4") throw new Error("unsupported_contract");
  if (w.approved_for_execution !== false) throw new Error("cartridge_self_authorization");
  const simulation = w.simulation == null
    ? { status: "NOT_RUN_BY_CARTRIDGE", passed: null }
    : object(w.simulation);
  if (simulation.passed !== null && typeof simulation.passed !== "boolean") throw new Error("invalid_simulation_status");
  if (!Array.isArray(w.observations)) throw new Error("missing_observations");
  const legs = w.legs == null ? [] : w.legs;
  if (!Array.isArray(legs)) throw new Error("invalid_legs");
  const result: AgentCard = {
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
    simulation: simulation as AgentCard["simulation"], reason: string(w.reason, "reason"), wire: w,
  };
  if (result.candidate_eligible && (!result.plan_hash || !result.snapshot_id || !result.amount_in_raw || !result.net_profit_usd || !result.legs.length)) throw new Error("candidate_missing_binding_or_ledger");
  if (result.amount_in_raw !== null && !/^(0|[1-9]\d*)$/.test(result.amount_in_raw)) throw new Error("invalid_raw_amount");
  return result;
}
/** Immutable plan/version identity; never merge old simulation into a new size. */
export function agentCardRevision(card: AgentCard): string {
  return JSON.stringify([card.mev_id, card.context_id, card.plan_hash, card.snapshot_id, card.price_revision, card.policy_revision, card.amount_in_raw]);
}
/** Missing is diagnostic, not N/A and not zero. A caller may show N/A only from
 * explicit field-level applicability data supplied by the producer. */
export function moneyLabel(value: string | null, reason: string): string {
  return value === null ? `Dato pendiente: ${reason}` : `${value} USD`;
}
