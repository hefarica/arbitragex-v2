import { describe, it, expect } from "vitest";
import { OpportunityListItemSchema, SimulatedTargetSchema, TokenInfoSchema } from "../api-contracts.js";

describe("TokenInfoSchema", () => {
  it("accepts fully resolved token", () => {
    expect(() => TokenInfoSchema.parse({
      symbol: "WETH", decimals: 18,
      logo_url: "https://raw.githubusercontent.com/.../logo.png",
      resolved_via: "onchain_full",
    })).not.toThrow();
  });

  it("accepts all-null TokenInfo (resolved_via=failed)", () => {
    expect(() => TokenInfoSchema.parse({
      symbol: null, decimals: null, logo_url: null, resolved_via: "failed",
    })).not.toThrow();
  });

  it("rejects invalid resolved_via", () => {
    expect(() => TokenInfoSchema.parse({
      symbol: "X", decimals: 18, logo_url: null, resolved_via: "guessed",
    })).toThrow();
  });
});

describe("OpportunityListItemSchema", () => {
  const base = {
    id: "11111111-1111-1111-1111-111111111111",
    chain_id: 1, strategy_kind: "dex_arb", dex_a: "uniswap-v2", dex_b: null,
    pair_symbol: "x/y",
    token_in: "0x" + "a".repeat(40),  token_in_info: null,
    token_out: "0x" + "b".repeat(40), token_out_info: null,
    amount_in_wei: "1000",
    expected_profit_usd: null, roi_pct: null, risk_score: null,
    block_number: null, rejection_reason: null, status: "detected" as const,
    detected_at: "2026-05-06T00:00:00Z", trace_id: "22222222-2222-2222-2222-222222222222",
    chain_id_out: null, bridge: null, bridge_fee_usd: null,
  };

  it("accepts a fail-honest item with all NULL profit", () => {
    expect(() => OpportunityListItemSchema.parse(base)).not.toThrow();
  });

  it("accepts a simulated item with profit=0 (real value, not pending)", () => {
    expect(() => OpportunityListItemSchema.parse({
      ...base, status: "simulated", expected_profit_usd: 0,
    })).not.toThrow();
  });

  it("accepts cross-chain item with chain_id_out + bridge filled", () => {
    expect(() => OpportunityListItemSchema.parse({
      ...base, chain_id_out: 42161, bridge: "across", bridge_fee_usd: 0.50,
    })).not.toThrow();
  });

  it("rejects invalid status", () => {
    expect(() => OpportunityListItemSchema.parse({ ...base, status: "magic" })).toThrow();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ECON-SENTINEL-CONTRACT-01 — el contrato debe declarar lo que el cable manda.
//
// MEDIDO EN VIVO (2026-09-27, GET /api/opportunities/live por el tunel al VPS,
// motor local APAGADO): `required_amount_in_usd` es la CADENA "Infinity" en
// 38-42 de 42 filas — NUNCA un numero finito en esas filas. Este schema
// declaraba `z.number()` y por tanto RECHAZABA la mayoria de las filas reales.
// Nadie validaba el cable en runtime todavia (el schema solo se usa en tests),
// asi que la mentira era latente; estos casos la fijan.
//
// Los valores de abajo son los observados en produccion (binding_floor,
// estimation_basis, target_source tal como salen en el cable).
// ─────────────────────────────────────────────────────────────────────────────
describe("ECON-SENTINEL-CONTRACT-01 — SimulatedTargetSchema vs el cable real", () => {
  const target = (over: Record<string, unknown>) => ({
    target_net_usd: 50,
    target_roi_pct: 0.3,
    target_source: "simulation_tab" as const,
    binding_floor: "net-per-usd-nonpositive" as const,
    estimation_basis: "observed-gross" as const,
    required_amount_in_usd: 1175.42,
    required_is_infinite: false,
    cap_amount_in_usd: 1000,
    suggested_amount_in_usd: 0,
    suggested_net_usd: -1.196609408273031,
    suggested_roi_pct: 0,
    meets_target_at_cap: false,
    notes: ["roi-based-estimate"],
    ...over,
  });

  it("acepta el centinela \"Infinity\" con su flag (la mayoria de las filas vivas)", () => {
    // REGRESION: con `required_amount_in_usd: z.number()` esto lanzaba.
    expect(() =>
      SimulatedTargetSchema.parse(
        target({ required_amount_in_usd: "Infinity", required_is_infinite: true }),
      ),
    ).not.toThrow();
  });

  it("acepta un requerimiento finito con flag false", () => {
    expect(() =>
      SimulatedTargetSchema.parse(
        target({ required_amount_in_usd: 1175.42, required_is_infinite: false }),
      ),
    ).not.toThrow();
  });

  it("rechaza el centinela sin su flag (el par es un solo dato)", () => {
    expect(() =>
      SimulatedTargetSchema.parse(
        target({ required_amount_in_usd: "Infinity", required_is_infinite: false }),
      ),
    ).toThrow(/required_is_infinite/);
  });

  it("rechaza un flag true sobre un numero finito", () => {
    expect(() =>
      SimulatedTargetSchema.parse(
        target({ required_amount_in_usd: 10, required_is_infinite: true }),
      ),
    ).toThrow(/required_is_infinite/);
  });

  it("rechaza cualquier OTRA cadena: solo el centinela documentado pasa", () => {
    // "NaN" y "" son la clase de valor que ya se coló en otros campos; el
    // contrato no debe admitirlos aqui.
    expect(() => SimulatedTargetSchema.parse(target({ required_amount_in_usd: "NaN" }))).toThrow();
    expect(() => SimulatedTargetSchema.parse(target({ required_amount_in_usd: "" }))).toThrow();
  });
});
