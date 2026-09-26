// frontend/components/__tests__/OpportunityTradeCard.test.tsx
//
// HOPS-CARD-03 — the step-ladder renders the REAL N-leg topology from
// deriveLegs (route_metadata when present, §29-marked synthetic fallback
// otherwise), with per-leg symbols (pair-info → leg_symbols → shortAddr).
// The old hardcoded "Buy A / Buy B" 2-row ladder hid every N-leg route.
// Fixtures go through the real mapper. R1: static render only — the card's
// time-dependent cells are gated by isMounted and stay deterministic here.
import React from "react";
import { describe, it, expect, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

// ui/skeleton's type-only React import breaks vitest's classic-JSX SSR path
// ("React is not defined" — cf. ui/tabs.tsx which carries the value-import
// convention). Passthrough mock: the ladder assertions never depend on the
// skeleton visual. (Latent issue in ui/skeleton.tsx, noted in the PR.)
vi.mock("@/components/ui/skeleton", () => ({
  Skeleton: (props: React.ComponentProps<"div">) =>
    React.createElement("div", { "data-slot": "skeleton", ...props }),
}));

import { OpportunityTradeCard } from "../OpportunityTradeCard";
import { mapToOmniOpportunity } from "@/lib/store/types";

const wire = (over: Record<string, unknown>) => ({
  id: "opp-1",
  chain_id: 1,
  strategy_kind: "triangular_atomic",
  detected_at: "2026-08-11T00:00:00Z",
  status: "detected",
  trace_id: "trace-1",
  dex_a: "uniswap-v2",
  dex_b: "sushiswap",
  token_in: "0xa",
  token_out: "0xb",
  block_number: 123,
  ...over,
});

function card(opp: ReturnType<typeof mapToOmniOpportunity>): string {
  return renderToStaticMarkup(
    React.createElement(OpportunityTradeCard, {
      opp,
      now: 0,
      isMounted: false, // R1: SSR path — no time-dependent text
      simLoading: false,
      onExecute: () => {},
      onInspect: () => {},
    }),
  );
}

const A = "0x" + "a".repeat(40);
const B = "0x" + "b".repeat(40);
const C = "0x" + "c".repeat(40);

// CARDS-PRICES-01 gate (adversarial-review finding #2, card-level half): a live
// PriceBus price must reach the RENDERED chip through the real mapper, and an
// absent price must render NOTHING (R8: never a guess). Without this gate the
// mapper silent-drop (finding #1) shipped as dead code with every unit test
// green.
describe("OpportunityTradeCard — CARDS-PRICES-01 live price chip", () => {
  it("renders the live USD price under the token chip; absent price renders nothing", () => {
    const info = { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full" };
    const withPrice = mapToOmniOpportunity(
      wire({
        token_in: A,
        token_out: B,
        token_in_info: info,
        token_prices_usd: { WETH: 2685.78 },
      }),
    );
    expect(card(withPrice)).toContain("$2,685.78");

    const withoutPrice = mapToOmniOpportunity(
      wire({ token_in: A, token_out: B, token_in_info: info }),
    );
    expect(card(withoutPrice)).not.toContain("$2,685.78");
  });
});

describe("OpportunityTradeCard — HOPS-CARD-03 step ladder", () => {
  it("renders every leg of a 3-hop route with symbols — not the hardcoded 2-row ladder", () => {
    const info = { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full" };
    // leg_symbols is injected at the VIEWMODEL level (post-mapper): the mapper
    // passthrough lands in PR #535 (HOPS-SYM-02) — this PR stays
    // merge-order-independent. The card resolves whatever the store holds.
    const opp = {
      ...mapToOmniOpportunity(
        wire({
          token_in: A,
          token_out: A,
          token_in_info: info,
          token_out_info: info,
          route_metadata: {
            dex_adapters: ["uniswap_v2_router", "sushiswap", "uniswap_v2_router"],
            token_addresses: [A, B, C, A],
            pool_addresses: ["0xpool1", "0xpool2", "0xpool3"],
          },
        }),
      ),
      leg_symbols: { [B.toLowerCase()]: "USDC", [C.toLowerCase()]: "PEPE" },
    };
    const html = card(opp);
    // all three hops present, in order, with the hop counter
    expect(html).toContain("Hop 1/3");
    expect(html).toContain("Hop 2/3");
    expect(html).toContain("Hop 3/3");
    // symbol resolution: pair-info endpoints + leg_symbols intermediates
    expect(html).toContain("WETH→USDC");
    expect(html).toContain("USDC→PEPE");
    expect(html).toContain("PEPE→WETH");
    // dex per leg in the hint surface
    expect(html).toContain("uniswap_v2_router");
    expect(html).toContain("sushiswap");
    // the hardcoded ladder is gone
    expect(html).not.toContain("Buy A");
    expect(html).not.toContain("Buy B");
    // real topology ⇒ NO §29 marker
    expect(html).not.toContain("SYNTHETIC LEGACY VIEW");
  });

  it("2-hop dex route renders its 2 real legs (dex_a/dex_b no longer drive the ladder)", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          dex_adapters: ["uniswap-v2", "sushiswap"],
          token_addresses: ["0xa", "0xb", "0xa"],
          pool_addresses: ["0xpool1", "0xpool2"],
        },
      }),
    );
    const html = card(opp);
    expect(html).toContain("Hop 1/2");
    expect(html).toContain("Hop 2/2");
    // shortAddr fallback for short fixture addresses (R8 — no fabrication)
    expect(html).toContain("0xa→0xb");
    expect(html).not.toContain("SYNTHETIC LEGACY VIEW");
  });

  it("no route_metadata ⇒ §29 synthetic fallback ladder, MARKED — never ROUTE VERIFIED", () => {
    const opp = mapToOmniOpportunity(wire({ route_metadata: null }));
    const html = card(opp);
    // deriveLegs synthetic 2-leg cycle from dex_a/dex_b
    expect(html).toContain("Hop 1/2");
    expect(html).toContain("Hop 2/2");
    expect(html).toContain("SYNTHETIC LEGACY VIEW");
    expect(html).toContain("no ROUTE VERIFIED");
    expect(html).toContain("syn"); // per-leg marker in the hint
  });

  it("no topology at all (no route_metadata, no dex pair) ⇒ honest gap row, no fabricated hops", () => {
    const opp = mapToOmniOpportunity(
      wire({ route_metadata: null, dex_a: "", dex_b: null }),
    );
    const html = card(opp);
    expect(html).toContain("sin topología persistida (§38)");
    expect(html).not.toContain("Hop 1/");
    expect(html).not.toContain("SYNTHETIC LEGACY VIEW");
  });

  it("per-leg amounts stay honest nulls — the ladder shows topology only until the wire carries amounts", () => {
    // HOPS-LEDGER-04 (wire amounts) is a sibling PR; until it lands every
    // hop row renders the dash for its value, never a fabricated running total.
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          dex_adapters: ["uniswap-v2", "sushiswap"],
          token_addresses: ["0xa", "0xb", "0xa"],
          pool_addresses: ["0xpool1", "0xpool2"],
        },
      }),
    );
    const html = card(opp);
    expect(html).toContain("Capital path (USD)");
    // the hop rows exist and the ledger block still renders its honest ends
    expect(html).toContain("Gross out (AMM spread)");
    expect(html).toContain("Net yield");
  });

  it("R1: pure render is byte-identical across invocations", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          dex_adapters: ["uniswap-v2", "sushiswap"],
          token_addresses: ["0xa", "0xb", "0xa"],
          pool_addresses: ["0xpool1", "0xpool2"],
        },
      }),
    );
    expect(card(opp)).toBe(card(opp));
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// CARDS-TOKENPATH-01 — full token path in the chip row (operator order
// 2026-09-20): N hops ⇒ N chips (cycle closure popped), each chip = logo +
// contract shortAddr + symbol (intermediates via leg_symbols fallback).
// ─────────────────────────────────────────────────────────────────────────────
const ARROW_RE = /<span class="text-muted-foreground\/60 shrink-0" aria-hidden="true">→<\/span>/g;
const countArrows = (html: string): number => (html.match(ARROW_RE) ?? []).length;

describe("OpportunityTradeCard — CARDS-TOKENPATH-01 token path chips", () => {
  it("3-hop route renders exactly 3 token chips (cycle close popped) with endpoint + leg_symbols symbols", () => {
    const info = { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full" };
    const opp = {
      ...mapToOmniOpportunity(
        wire({
          token_in: A,
          token_out: A,
          token_in_info: info,
          token_out_info: info,
          route_metadata: {
            dex_adapters: ["uniswap_v2_router", "sushiswap", "uniswap_v2_router"],
            token_addresses: [A, B, C, A],
            pool_addresses: ["0xpool1", "0xpool2", "0xpool3"],
          },
        }),
      ),
      leg_symbols: { [B.toLowerCase()]: "USDC", [C.toLowerCase()]: "PEPE" },
    };
    const html = card(opp);
    // 3 tokens ⇒ 2 arrows in the chip row (ladder arrows are separate glyphs)
    expect(countArrows(html)).toBe(2);
    // every participating token shows symbol + contract shortAddr
    expect(html).toContain("WETH");
    expect(html).toContain("USDC");
    expect(html).toContain("PEPE");
    expect(html.toLowerCase()).toContain(A.toLowerCase());
    expect(html.toLowerCase()).toContain(B.toLowerCase());
    expect(html.toLowerCase()).toContain(C.toLowerCase());
  });

  it("7-hop route renders 7 chips — the full path up to the operator's max", () => {
    const addrs = Array.from({ length: 7 }, (_, i) => "0x" + String.fromCharCode(97 + i).repeat(40));
    const closed = [...addrs, addrs[0]!]; // 7 hops, cycle closes on token 0
    const leg_symbols: Record<string, string> = {};
    for (const a of addrs.slice(1)) leg_symbols[a.toLowerCase()] = "TKN" + a.slice(2, 4);
    const opp = {
      ...mapToOmniOpportunity(
        wire({
          token_in: addrs[0]!,
          token_out: addrs[0]!,
          route_metadata: {
            dex_adapters: Array.from({ length: 7 }, () => "uniswap-v2"),
            token_addresses: closed,
            pool_addresses: Array.from({ length: 7 }, (_, i) => "0xpool" + i),
          },
        }),
      ),
      leg_symbols,
    };
    const html = card(opp);
    // 7 tokens ⇒ 6 arrows; each intermediate symbol surfaced via fallback
    expect(countArrows(html)).toBe(6);
    for (const a of addrs.slice(1)) {
      expect(html).toContain("TKN" + a.slice(2, 4));
    }
  });

  it("no route_metadata ⇒ honest 2-chip in→out fallback row", () => {
    const opp = mapToOmniOpportunity(
      wire({ route_metadata: null, token_in: "0xa", token_out: "0xb" }),
    );
    const html = card(opp);
    expect(countArrows(html)).toBe(1);
    expect(html).toContain("0xa");
    expect(html).toContain("0xb");
  });

  it("intermediate without leg_symbols entry still renders contract shortAddr (R8 — no fabrication)", () => {
    const opp = {
      ...mapToOmniOpportunity(
        wire({
          token_in: A,
          token_out: A,
          route_metadata: {
            dex_adapters: ["uniswap_v2_router", "sushiswap", "uniswap_v2_router"],
            token_addresses: [A, B, C, A],
            pool_addresses: ["0xpool1", "0xpool2", "0xpool3"],
          },
        }),
      ),
      // leg_symbols EMPTY — B and C fall back to their contract address chip
      leg_symbols: {},
    };
    const html = card(opp);
    expect(countArrows(html)).toBe(2);
    // shortAddr of B rendered as the symbol line ("0xbb…bbbb" style truncation
    // is shortAddr's business — assert the raw chip contract line exists)
    expect(html.toLowerCase()).toContain(B.toLowerCase());
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// AUDIT-CARDS-MINOR (§3) — the §29 fallback token path (no route_metadata ⇒
// [token_in, token_out]) used to render EXACTLY like a topology-backed chip
// row. The ladder already marks its synthetic legs ("SYNTHETIC LEGACY VIEW");
// now the chip row carries the same compact mark: a "·syn" suffix + tooltip.
// ─────────────────────────────────────────────────────────────────────────────
describe("OpportunityTradeCard — AUDIT-CARDS-MINOR (§3) fallback chip-row mark", () => {
  it("no route_metadata (synthetic legs) ⇒ chip row carries the compact §29 mark + tooltip", () => {
    const opp = mapToOmniOpportunity(wire({ route_metadata: null }));
    const html = card(opp);
    expect(html).toContain("·syn");
    expect(html).toContain('title="fallback §29 — sin topología persistida"');
    // ladder discipline unchanged — the band still marks the synthetic legs
    expect(html).toContain("SYNTHETIC LEGACY VIEW");
  });

  it("no topology at all (no dex pair either) ⇒ the in→out pair is ALSO a fallback and is marked", () => {
    const opp = mapToOmniOpportunity(
      wire({ route_metadata: null, dex_a: "", dex_b: null }),
    );
    const html = card(opp);
    expect(html).toContain("·syn");
    expect(html).toContain('title="fallback §29 — sin topología persistida"');
  });

  it("persisted topology ⇒ NO fallback mark (the chip path is real, not §29)", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          dex_adapters: ["uniswap-v2", "sushiswap"],
          token_addresses: ["0xa", "0xb", "0xa"],
          pool_addresses: ["0xpool1", "0xpool2"],
        },
      }),
    );
    const html = card(opp);
    expect(html).not.toContain("·syn");
    expect(html).not.toContain("sin topología persistida");
  });

  it("the mark adds no extra chip-row arrows — a compact suffix, never a fake hop", () => {
    const opp = mapToOmniOpportunity(wire({ route_metadata: null }));
    const html = card(opp);
    expect(countArrows(html)).toBe(1); // exactly the in→out pair, as before the mark
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// HOPS-LEDGER-04 (PER-HOP, card half) — the operator's per-hop mandate: every
// hop of a 2..7-leg cycle shows ITS numbers (exact wei in→out) on the row that
// already owns that hop, plus the price-marked leg Δ and the closed-cycle delta
// on the closing leg. Fail-honest gates: no ledger ⇒ no figures; unknown
// decimals ⇒ raw wei (never a guessed unit); no live price ⇒ no Δ (never $0).
// ─────────────────────────────────────────────────────────────────────────────
describe("OpportunityTradeCard — HOPS-LEDGER-04 per-hop amounts", () => {
  const WETH = A;
  const USDC = B;
  const out18 = (n: string) => `${n}000000000000000000`; // n → n·1e18 wei
  const usdc6 = (n: string) => `${n}000000`; // n → n·1e6 wei (6-dec token)
  // 1.002 WETH in exact wei — a ledger entry is an integer wei string, never a decimal.
  const WETH_1_002 = "1002000000000000000";

  // 2-hop closed cycle WETH→USDC→WETH with the kernel's exact wei ledger:
  // 1 WETH in → 2,700 USDC out → 1.002 WETH back.
  const sized = mapToOmniOpportunity(
    wire({
      token_in: WETH,
      token_out: WETH,
      token_in_info: { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full" },
      token_out_info: { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full" },
      token_prices_usd: { WETH: 2700, USDC: 1 },
      route_metadata: {
        dex_adapters: ["uniswap_v2_router", "sushiswap"],
        token_addresses: [WETH, USDC, WETH],
        pool_addresses: ["0xpool1", "0xpool2"],
        decimals: { [WETH.toLowerCase()]: 18, [USDC.toLowerCase()]: 6 },
        leg_amounts_in: [out18("1"), usdc6("2700")],
        leg_amounts_out: [usdc6("2700"), WETH_1_002],
        leg_zero_for_one: [true, false],
      },
    }),
  );

  it("renders the exact per-hop wei of a sized cycle, scaled by each token's decimals", () => {
    const html = card({ ...sized, leg_symbols: { [USDC.toLowerCase()]: "USDC" } });
    // hop 1: 1 WETH → 2,700 USDC (6 decimals, thousands separator)
    expect(html).toContain("Hop 1/2");
    expect(html).toContain("1 WETH");
    expect(html).toContain("2,700 USDC");
    // hop 2: 2,700 USDC → 1.002 WETH
    expect(html).toContain("Hop 2/2");
    expect(html).toContain("1.002 WETH");
  });

  it("marks the leg Δ at live PriceBus prices and the closed-cycle delta on the closing leg only", () => {
    const html = card({ ...sized, leg_symbols: { [USDC.toLowerCase()]: "USDC" } });
    // leg 1 preserves value (1 WETH @2700 → 2,700 USDC @1): Δ $0, not a gain
    expect(html).toContain("Δ $0.0000");
    // leg 2 gains 0.002 WETH @2700 = $5.40 — the operator's per-hop number
    expect(html).toContain("Δ $5.40");
    // cycle delta is EXACT wei arithmetic (1.002e18 − 1e18 = 2e15), closing leg only
    const cycleHits = (html.match(/ciclo /g) ?? []).length;
    expect(cycleHits).toBe(1);
    expect(html).toContain("ciclo 0.002 WETH");
  });

  it("a partial ledger renders NO per-hop figures (all-or-nothing, mirrors attach_leg_ledger)", () => {
    const partial = mapToOmniOpportunity(
      wire({
        token_in: WETH,
        token_out: WETH,
        route_metadata: {
          dex_adapters: ["uniswap_v2_router", "sushiswap"],
          token_addresses: [WETH, USDC, WETH],
          pool_addresses: ["0xpool1", "0xpool2"],
          decimals: { [WETH.toLowerCase()]: 18, [USDC.toLowerCase()]: 6 },
          // leg_zero_for_one missing ⇒ deriveLegLedger must return null
          leg_amounts_in: [out18("1"), usdc6("2700")],
          leg_amounts_out: [usdc6("2700"), WETH_1_002],
        },
      }),
    );
    const html = card(partial);
    expect(html).toContain("Hop 1/2");
    expect(html).not.toContain("ciclo ");
    expect(html).not.toContain("Δ $");
  });

  it("unknown decimals ⇒ the raw wei verbatim (`·wei`), never a guessed 18-decimals unit", () => {
    const noDecimals = mapToOmniOpportunity(
      wire({
        token_in: WETH,
        token_out: WETH,
        route_metadata: {
          dex_adapters: ["uniswap_v2_router", "sushiswap"],
          token_addresses: [WETH, USDC, WETH],
          pool_addresses: ["0xpool1", "0xpool2"],
          // decimals map EMPTY: the wire omitted the deployment fact (R8)
          leg_amounts_in: [out18("1"), usdc6("2700")],
          leg_amounts_out: [usdc6("2700"), WETH_1_002],
          leg_zero_for_one: [true, false],
        },
      }),
    );
    const html = card(noDecimals);
    expect(html).toContain(`${out18("1")}·wei`);
    expect(html).toContain(`${usdc6("2700")}·wei`);
    // no unit claim anywhere on those legs; the cycle delta falls back to raw wei
    expect(html).not.toContain("Δ $");
    expect(html).toContain("ciclo 2000000000000000 wei");
  });

  it("no live price for a leg's symbol ⇒ that leg shows amounts but NO Δ (never a $0)", () => {
    const priced = {
      ...sized,
      token_prices_usd: { WETH: 2700 }, // USDC price absent
      leg_symbols: { [USDC.toLowerCase()]: "USDC" },
    };
    const html = card(priced);
    expect(html).toContain("2,700 USDC");
    expect(html).not.toContain("Δ $");
  });

  it("R8: a 7-hop cycle renders 7 hop rows, each with its own ledger cell when sized", () => {
    // T1..T7 distinct intermediates + closing leg back to T1 (operator max: 7).
    const T = [1, 2, 3, 4, 5, 6, 7].map((i) => `0x${String(i).repeat(40)}`);
    const tokens = [...T, T[0]!];
    const decimals: Record<string, number> = {};
    const legSymbols: Record<string, string> = {};
    tokens.forEach((t, i) => {
      decimals[t.toLowerCase()] = 18;
      if (i < 7) legSymbols[t.toLowerCase()] = `T${i + 1}`;
    });
    const opp = {
      ...mapToOmniOpportunity(
        wire({
          token_in: T[0],
          token_out: T[6],
          route_metadata: {
            dex_adapters: Array.from({ length: 7 }, () => "uniswap_v2_router"),
            token_addresses: tokens,
            pool_addresses: Array.from({ length: 7 }, (_, i) => `0xpool${i}`),
            decimals,
            leg_amounts_in: Array.from({ length: 7 }, (_, i) => out18(String(i + 1))),
            leg_amounts_out: Array.from({ length: 7 }, (_, i) => out18(String(i + 2))),
            leg_zero_for_one: Array.from({ length: 7 }, (_, i) => i % 2 === 0),
          },
        }),
      ),
      leg_symbols: legSymbols,
    };
    const html = card(opp);
    for (let i = 1; i <= 7; i++) expect(html).toContain(`Hop ${i}/7`);
    // hop 1 cell: 1 → 2 T-units; closing leg carries the cycle delta (8 − 1 = 7)
    expect(html).toContain("1 T1");
    expect(html).toContain("ciclo 7 T1");
  });
});
