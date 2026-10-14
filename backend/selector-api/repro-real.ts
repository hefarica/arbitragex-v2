// Corre el schema REAL exportado por consumer.ts contra el payload REAL.
import { SelectorOpportunitySchema } from "./src/consumer.js";
import { OpportunitySchema } from "@arbx/shared";

const PAYLOAD = {"id":"0dcd67a1-2982-43de-a64a-5f6e2f9adf80","chain_id":1,"strategy_kind":"triangular","dex_a":"uniswap-v2","dex_b":"uniswap-v3","pair_symbol":"698250(4-hop cycle)","token_in":"0x6982508145454ce325ddbe47a25d4ec3d2311933","token_out":"0x6982508145454ce325ddbe47a25d4ec3d2311933","amount_in_wei":"0","expected_profit_usd":null,"net_expected_profit_usd":null,"roi_pct":null,"risk_score":null,"block_number":26133382,"rejection_reason":"v3_quote_unavailable","cartridge_id":null,"detector_id":"hop_cycle_bridge","pipeline_latency_ms":945,"economics":{"computation_status":"error","error_reason":"v3_quote_unavailable","legs":[],"not_computed_reasons":{}},"detected_at":"2026-10-06T12:38:05.578591982Z","trace_id":"30a11ea8-8a09-42e4-883f-5d57ba8b39c1"};

const before = OpportunitySchema.safeParse(PAYLOAD);
console.log("ANTES (OpportunitySchema, .strict()):", before.success ? "PASA" : "RECHAZA");
if (!before.success) for (const i of before.error.issues) console.log("   ", i.code, JSON.stringify((i as any).keys ?? []));

const after = SelectorOpportunitySchema.safeParse(PAYLOAD);
console.log("DESPUES (SelectorOpportunitySchema, el EXPORTADO REAL):", after.success ? "PASA" : "RECHAZA");
if (!after.success) for (const i of after.error.issues) console.log("   ", i.code, JSON.stringify((i as any).keys ?? []));

// control negativo: estricto sigue siendo estricto ante una clave de verdad desconocida
const bogus = SelectorOpportunitySchema.safeParse({ ...PAYLOAD, totally_unknown_key: 1 });
console.log("CONTROL NEGATIVO (clave basura):", bogus.success ? "PASA (mal: ya no es estricto)" : "RECHAZA (bien: sigue estricto)");
