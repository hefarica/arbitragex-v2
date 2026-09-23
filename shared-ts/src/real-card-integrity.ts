/** WO-PC9 — executable strict field pipeline. No oracle, config or trade writes.
 *
 * Host supplies its canonical PriceBus snapshot and REAL producer functions.
 * All applicable fields run, including on rejected opportunities. Unavailable
 * inputs create explicit repair failures; no 0/$1/default-decimal imputation.
 * A complete packet is NOT a source attestation or a trading authorization.
 */
export type AuditJson = null | boolean | number | string | AuditJson[] | { [key: string]: AuditJson };
export type FieldState = "observed" | "computed" | "not_applicable" | "error";
export interface FieldSource {
  kind: "onchain" | "offchain" | "calculation" | "operator_config";
  reference: string;
  as_of_ms: number;
  input_hashes?: string[];
  transform_id?: string;
  transform_version?: string;
  block_hash?: string;
  chain_id?: number;
}
export interface RealField {
  state: FieldState;
  value: AuditJson;
  unit: string;
  context_id?: string;
  valid_until_ms?: number;
  reason?: string;
  source?: FieldSource;
}
export interface FieldRequirement {
  name: string;
  unit: string;
  applicable: boolean;
  applicability_rule: string;
  not_applicable_reason?: string;
}
export interface FieldContext {
  event_id: string;
  strategy_id: string;
  context_id: string;
  snapshot_hash: string;
  snapshot_generated_at_ms: number;
  config_hash: string;
  /** Explicit canonical PriceBus export, loaded once by the host. Never a fallback oracle. */
  pricebus: Readonly<AuditJson>;
  /** Lifecycle is preserved. Rejection is not a reason to skip applicable calculations. */
  rejection_reason: string | null;
}
export interface ProducerInput {
  context: Readonly<FieldContext>;
  dependencies: Readonly<Record<string, RealField>>;
  signal: AbortSignal;
}
export interface FieldProducer {
  field: string;
  depends_on: readonly string[];
  compute(input: ProducerInput): Promise<RealField>;
}
export interface PipelinePolicy {
  now: () => number;
  deadline_ms: number;
  per_field_timeout_ms: number;
  concurrency: number;
  max_clock_skew_ms: number;
  snapshot_max_age_ms: number;
}
export interface Repair {
  /** Stable within event/context/field; sink dedups this exact key. */
  repair_key: string;
  event_id: string;
  context_id: string;
  field: string;
  reason: string;
}
export interface CardPacket {
  schema: "arbx.native-real-card.v1";
  context: Readonly<FieldContext>;
  requirements: readonly FieldRequirement[];
  fields: Record<string, RealField>;
  completeness: "complete" | "repair_required";
  repairs: Repair[];
  execution_authorized: false;
  source_authenticity_verified: false;
}
const HASH = /^[a-f0-9]{64}$/;
const DECIMAL = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/;
const NUMERIC_UNITS = new Set(["USD", "percent", "bps", "probability", "ratio", "token", "price_usd"]);
function jsonSafe(v: unknown, depth = 0): boolean {
  if (depth > 32) return false;
  if (v === null || typeof v === "boolean" || typeof v === "string") return true;
  if (typeof v === "number") return Number.isSafeInteger(v);
  if (Array.isArray(v)) return v.length <= 10000 && v.every(x => jsonSafe(x, depth + 1));
  if (typeof v !== "object") return false;
  return Object.keys(v).length <= 10000 && Object.values(v).every(x => jsonSafe(x, depth + 1));
}
function freeze<T>(value: T): T {
  if (value && typeof value === "object") {
    Object.freeze(value);
    for (const v of Object.values(value)) freeze(v);
  }
  return value;
}
function failure(unit: string, reason: string): RealField { return {state: "error", value: null, unit, reason}; }
export function fieldFailure(f: RealField, requirement: FieldRequirement, context: FieldContext, now: number, skew: number): string | null {
  if (!f || typeof f !== "object") return "missing_field_record";
  if (f.unit !== requirement.unit) return "unit_mismatch";
  if (!requirement.applicability_rule) return "applicability_rule_missing";
  if (!requirement.applicable) {
    return f.state === "not_applicable" && f.value === null && !!requirement.not_applicable_reason
      && f.reason === requirement.not_applicable_reason ? null : "invalid_not_applicable_record";
  }
  if (f.state !== "observed" && f.state !== "computed") return f.reason || "applicable_field_not_computed";
  if (f.value === null || !jsonSafe(f.value)) return "invalid_or_missing_value";
  if (typeof f.value === "string" && ["", "—", "-", "no computado", "nan", "infinity"].includes(f.value.trim().toLowerCase())) return "placeholder_not_value";
  if (NUMERIC_UNITS.has(f.unit) && (typeof f.value !== "string" || f.value.length > 1024 || !DECIMAL.test(f.value))) return "exact_decimal_string_required";
  if (f.unit === "probability" && (Number(f.value) < 0 || Number(f.value) > 1)) return "probability_out_of_range";
  if (f.context_id !== context.context_id) return "cross_context_value";
  if (!Number.isSafeInteger(f.valid_until_ms) || (f.valid_until_ms as number) < now) return "value_expired";
  const s = f.source;
  if (!s?.reference || !Number.isSafeInteger(s.as_of_ms) || s.as_of_ms <= 0 || s.as_of_ms > now + skew) return "source_provenance_missing_or_future";
  if (!["onchain", "offchain", "calculation", "operator_config"].includes(s.kind)) return "source_kind_unknown";
  if (s.kind === "onchain" && (!/^0x[a-fA-F0-9]{64}$/.test(s.block_hash ?? "") || !Number.isSafeInteger(s.chain_id) || (s.chain_id as number) <= 0)) return "onchain_identity_missing";
  if (f.state === "computed" && (!s.transform_id || !s.transform_version || !s.input_hashes?.length || s.input_hashes.some(h => !HASH.test(h)))) return "transform_lineage_missing";
  return null;
}

/** No retries against a stale snapshot. Host must re-ingest and create a new context. */
export async function runRealCardPipeline(contextInput: FieldContext, requirementsInput: readonly FieldRequirement[],
  producers: readonly FieldProducer[], policy: PipelinePolicy): Promise<CardPacket> {
  const start = policy.now();
  if (![start, policy.deadline_ms, policy.per_field_timeout_ms, policy.concurrency, policy.max_clock_skew_ms, policy.snapshot_max_age_ms].every(Number.isSafeInteger)
      || start <= 0 || policy.deadline_ms <= 0 || policy.per_field_timeout_ms <= 0 || policy.concurrency < 1 || policy.concurrency > 32
      || policy.max_clock_skew_ms < 0 || policy.snapshot_max_age_ms < 0) throw new Error("invalid_pipeline_policy");
  if (!contextInput.event_id || !contextInput.strategy_id || !HASH.test(contextInput.context_id)
      || !HASH.test(contextInput.snapshot_hash) || !HASH.test(contextInput.config_hash)) throw new Error("context_identity_missing");
  if (!jsonSafe(contextInput.pricebus)) throw new Error("pricebus_export_not_exact_json");
  const bus = contextInput.pricebus;
  if (!bus || typeof bus !== "object" || Array.isArray(bus) || !("schema" in bus)
      || bus.schema !== "arbx.pricebus.export.v1" || !("source" in bus) || bus.source !== "canonical_pricebus"
      || !("snapshot_hash" in bus) || bus.snapshot_hash !== contextInput.snapshot_hash) throw new Error("canonical_pricebus_export_required");
  const context = freeze(structuredClone(contextInput));
  const requirements = freeze(structuredClone(requirementsInput));
  if (!requirements.length || requirements.length > 1000 || new Set(requirements.map(r => r.name)).size !== requirements.length) throw new Error("unique_bounded_field_manifest_required");
  const reqs = new Map(requirements.map(r => [r.name, r]));
  const byField = new Map<string, FieldProducer>();
  for (const p of producers) {
    if (byField.has(p.field)) throw new Error("duplicate_producer");
    if (!reqs.has(p.field)) throw new Error("producer_not_in_field_manifest");
    byField.set(p.field, p);
  }
  const fields: Record<string, RealField> = Object.create(null) as Record<string, RealField>;
  const pending = new Set(requirements.map(r => r.name));
  const age = start - context.snapshot_generated_at_ms;
  const snapshotInvalid = !Number.isSafeInteger(context.snapshot_generated_at_ms) || context.snapshot_generated_at_ms <= 0
    || age > policy.snapshot_max_age_ms || age < -policy.max_clock_skew_ms;
  for (const r of requirements) {
    if (!r.name || !r.unit || !r.applicability_rule) throw new Error("incomplete_field_manifest");
    if (!r.applicable) {
      fields[r.name] = r.not_applicable_reason
        ? {state:"not_applicable",value:null,unit:r.unit,reason:r.not_applicable_reason}
        : failure(r.unit, "not_applicable_reason_missing");
      pending.delete(r.name);
    } else if (snapshotInvalid) {
      fields[r.name] = failure(r.unit,"pricebus_snapshot_stale_or_future"); pending.delete(r.name);
    }
  }
  async function execute(r: FieldRequirement, producer: FieldProducer): Promise<void> {
    const remaining = policy.deadline_ms - (policy.now() - start);
    if (remaining <= 0) { fields[r.name] = failure(r.unit,"pipeline_deadline"); return; }
    const timeout = Math.min(remaining, policy.per_field_timeout_ms);
    const ctrl = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const deps: Record<string, RealField> = {};
      for (const d of producer.depends_on) deps[d] = fields[d];
      const produced = await Promise.race([
        Promise.resolve().then(() => producer.compute({context, dependencies: freeze(structuredClone(deps)), signal: ctrl.signal})),
        new Promise<RealField>(resolve => { timer = setTimeout(() => {ctrl.abort(); resolve(failure(r.unit,"producer_timeout"));},timeout); }),
      ]);
      const error = fieldFailure(produced,r,context,policy.now(),policy.max_clock_skew_ms);
      fields[r.name] = error ? failure(r.unit,error) : freeze(structuredClone(produced));
    } catch {
      // No raw exception text: producer errors can include URLs/tokens.
      fields[r.name] = failure(r.unit,"producer_exception");
    } finally { if (timer !== undefined) clearTimeout(timer); }
  }
  while (pending.size) {
    const before = pending.size;
    const ready: {r: FieldRequirement; p: FieldProducer}[] = [];
    for (const name of [...pending]) {
      const r = reqs.get(name)!;
      const p = byField.get(name);
      if (!p) { fields[name] = failure(r.unit,"applicable_producer_missing"); pending.delete(name); continue; }
      if (p.depends_on.some(d => !reqs.has(d))) {
        fields[name] = failure(r.unit,"dependency_not_in_manifest"); pending.delete(name); continue;
      }
      if (p.depends_on.some(d => pending.has(d))) continue;
      // A producer may not turn a missing or N/A prerequisite into an invented number.
      if (p.depends_on.some(d => !["observed","computed"].includes(fields[d].state))) {
        fields[name] = failure(r.unit,"applicable_upstream_failed"); pending.delete(name); continue;
      }
      ready.push({r,p});
    }
    if (!ready.length) {
      if (pending.size < before) continue;
      if (pending.size) for (const name of pending) fields[name] = failure(reqs.get(name)!.unit,"dependency_cycle_or_unresolved");
      break;
    }
    for (let i=0; i<ready.length; i+=policy.concurrency) {
      await Promise.all(ready.slice(i,i+policy.concurrency).map(({r,p}) => execute(r,p)));
      for (const {r} of ready.slice(i,i+policy.concurrency)) pending.delete(r.name);
    }
  }
  // Final deadline/freshness check: a value may expire while other fields finish.
  const repairs: Repair[] = [];
  for (const r of requirements) {
    const reason = fieldFailure(fields[r.name],r,context,policy.now(),policy.max_clock_skew_ms);
    if (reason) {
      fields[r.name] = failure(r.unit,reason);
      repairs.push({event_id:context.event_id,context_id:context.context_id,field:r.name,reason,
        repair_key:JSON.stringify(["WO-PC9",context.event_id,context.context_id,r.name,reason])});
    }
  }
  return {schema:"arbx.native-real-card.v1",context,requirements,fields,
    completeness:repairs.length ? "repair_required":"complete",repairs,
    execution_authorized:false,source_authenticity_verified:false};
}

/** Backend-authoritative view. Never computes economics in React and never hides failure. */
export function presentRealField(packet: CardPacket, name: string, nowMs: number): {text:string; state:FieldState; reason:string|null} {
  const f = packet.fields[name]; const r = packet.requirements.find(x => x.name === name);
  if (!f || !r) return {text:"Error de datos",state:"error",reason:"field_not_in_manifest"};
  const error = fieldFailure(f,r,packet.context,nowMs,0);
  if (error) return {text:"Error de datos",state:"error",reason:error};
  if (f.state === "not_applicable") return {text:"No aplica",state:f.state,reason:f.reason ?? null};
  // Exact wire value is retained. Locale/display rounding belongs in an explicit formatter.
  return {text:typeof f.value === "string" ? f.value : JSON.stringify(f.value),state:f.state,reason:null};
}
