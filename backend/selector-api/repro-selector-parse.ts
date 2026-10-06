// SELECTOR-PARSE-01 — repro contra el parser REAL, con el payload REAL.
//
// NO es una reimplementacion: importa el MISMO `OpportunitySchema` de
// `shared-ts/src/contracts/index.ts` que consume `backend/selector-api/src/consumer.ts:329`.
//
// Payload capturado en vivo:
//   ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli XREVRANGE arbx:opps:detected + - COUNT 1"
//   id de stream: 1791290286529-0
//
// Uso:  npx tsx repro-selector-parse.ts

import { OpportunitySchema } from "../../shared-ts/src/contracts/index.js";

// ---- payload REAL, verbatim de XREVRANGE (sin editar) --------------------
const PAYLOAD = {
  id: "0dcd67a1-2982-43de-a64a-5f6e2f9adf80",
  chain_id: 1,
  strategy_kind: "triangular",
  dex_a: "uniswap-v2",
  dex_b: "uniswap-v3",
  pair_symbol: "698250(4-hop cycle)",
  token_in: "0x6982508145454ce325ddbe47a25d4ec3d2311933",
  token_out: "0x6982508145454ce325ddbe47a25d4ec3d2311933",
  amount_in_wei: "0",
  expected_profit_usd: null,
  net_expected_profit_usd: null,
  roi_pct: null,
  risk_score: null,
  block_number: 26133382,
  rejection_reason: "v3_quote_unavailable",
  cartridge_id: null,
  detector_id: "hop_cycle_bridge",
  pipeline_latency_ms: 945,
  economics: {
    computation_status: "error",
    error_reason: "v3_quote_unavailable",
    amount_in_wei: null,
    amount_out_wei: null,
    amount_in_usd: null,
    amount_out_usd: null,
    gross_profit_usd: null,
    gas_usd: null,
    dex_fees_usd: null,
    flash_fee_usd: null,
    bribe_usd: null,
    slippage_usd: null,
    other_costs_usd: null,
    total_cost_usd: null,
    net_profit_usd: null,
    roi_pct: null,
    target_net_usd: null,
    target_delta_usd: null,
    meets_target: null,
    quote_block: 26133382,
    simulation_block: null,
    legs: [],
    not_computed_reasons: {},
  },
  detected_at: "2026-10-06T12:38:05.578591982Z",
  trace_id: "30a11ea8-8a09-42e4-883f-5d57ba8b39c1",
};

// Claves de nivel superior que el payload trae y el schema DECLARA.
const declared = new Set(Object.keys(OpportunitySchema.shape));
const actual = Object.keys(PAYLOAD);
const undeclared = actual.filter((k) => !declared.has(k));

console.log("=== claves del payload NO declaradas en el schema ===");
console.log(undeclared.length ? undeclared.join(", ") : "(ninguna)");

console.log("\n=== parse con el schema REAL (OpportunitySchema, .strict()) ===");
const strict = OpportunitySchema.safeParse(PAYLOAD);
console.log("success =", strict.success);
if (!strict.success) {
  for (const i of strict.error.issues) {
    console.log(`  code=${i.code} keys=${JSON.stringify((i as any).keys ?? [])} msg=${i.message}`);
  }
}

// ---- la MISMA tolerancia que aplica el fix ------------------------------
console.log("\n=== parse con el schema TOLERANTE del fix (.extend({economics})) ===");
import { z } from "zod";
const Fixed = OpportunitySchema.extend({
  economics: z.object({}).passthrough().nullish(),
});
const fixed = Fixed.safeParse(PAYLOAD);
console.log("success =", fixed.success);
if (!fixed.success) {
  for (const i of fixed.error.issues) {
    console.log(`  code=${i.code} keys=${JSON.stringify((i as any).keys ?? [])} msg=${i.message}`);
  }
}

console.log("\n=== VEREDICTO ===");
console.log("strict  :", strict.success ? "PASA" : "RECHAZA -> consumer.invalid_message + XACK (mensaje perdido)");
console.log("fixed   :", fixed.success ? "PASA -> el pipeline SIGUE (no se relaja ningun gate)" : "RECHAZA");
