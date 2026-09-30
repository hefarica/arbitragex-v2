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

// ─────────────────────────────────────────────────────────────────────────────
// CARDS-NOTIONAL-01 (2026-09-26) — SSR gate on the operator's live
// contradiction. Live DOM evidence (production build `89d68ec8`,
// https://arbx.ape-tv.net/opportunities):
//
//   4b79551c PEPE | IN $0.00   | GROSS $1.47M    | NET -$0.00 | BPS ~1513332276940974
//                 | Repay $0.0000 | Total cost $73.4k
//   ed368994 DAI  | IN $1.00   | GROSS $822215.98 | BPS ~3700454443
//                 | Repay $1.00 | Total cost $41.1k
//   one frame:    | Flash loan in (TLS) · WETH $0.0000 | Gross out (AMM spread)
//                 | $1.45M | Relay fee $72.4k | Total cost $72.4k | Net yield -$0.0000
//
// Three doctrine breaks on ONE ladder: a cost ≫ its principal; `net != gross −
// total_cost` by six orders of magnitude; a seven-figure gross standing on a
// sub-dollar principal. The fixture below is that row: the recorded notional is
// the DETECTION PROBE (`amount_in_wei = 1e6` ⇒ 1 USDC ⇒ $1.00, stamped at the
// emit boundary), the gross is the DEX engine's fast-filter figure at the
// SEARCHER's own probe, and the net is the sizing KERNEL's at the kernel's own
// clamped size. Three producers, up to three notionals, one card.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Parse a rendered static-markup card into its ladder cells (`LedgerRow` →
 * label/value pairs), so the invariants are asserted on the NUMBERS the operator
 * actually reads rather than on substring luck.
 */
function ledgerCells(html: string): Array<{ label: string; value: string }> {
  const out: Array<{ label: string; value: string }> = [];
  const re =
    /class="min-w-0 flex-1 truncate">([^<]*)<\/span>(?:<span class="min-w-0 max-w-\[45%\][^>]*>\([^<]*\)<\/span>)?<\/span><span[^>]*class="shrink-0 whitespace-nowrap tabular-nums[^"]*"[^>]*>([^<]*)<\/span>/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(html)) !== null) {
    out.push({ label: m[1]!, value: m[2]! });
  }
  return out;
}

/** `$12.5k` / `-$0.0000` / `—` → USD, or null for the honest dash. */
function cellUsd(value: string | null): number | null {
  if (value == null) return null;
  const t = value.trim();
  const mm = /^(-?)\$([\d.]+)([kMBT])?$/.exec(t);
  if (!mm) return null;
  const body = Number(mm[2]);
  if (!Number.isFinite(body)) return null;
  const suffix = mm[3] ?? "";
  const mult =
    suffix === "k" ? 1e3 : suffix === "M" ? 1e6 : suffix === "B" ? 1e9 : suffix === "T" ? 1e12 : 1;
  return (mm[1] === "-" ? -1 : 1) * body * mult;
}

const cell = (cells: Array<{ label: string; value: string }>, label: string) =>
  cells.find((c) => c.label === label)?.value ?? null;

/** The operator's row: a $1.00 probe notional beside the searcher's gross. */
const probeVsKernelWire = (over: Record<string, unknown> = {}) =>
  wire({
    token_in: A,
    token_out: B,
    token_in_info: { symbol: "USDC", decimals: 6, logo_url: null, resolved_via: "onchain_full" },
    amount_in_wei: "1000000", // 1 USDC = $1.00
    expected_profit_usd: 822215.98, // searcher fast-filter gross (its own probe)
    net_expected_profit_usd: -0.000011, // sizing kernel net (the kernel's size)
    roi_pct: 37004544.43,
    simulated_amount_in_usd: 1.0,
    simulated_gross_usd: 822215.98,
    simulated_costs_total_usd: 411107.985,
    simulated_net_profit_usd: 411107.995,
    simulated_roi_pct: 41110799.5,
    simulated_cost_breakdown: {
      gas_usd: 0.673172225,
      lp_fees_usd: 0.003,
      slippage_usd: 0.5,
      failure_buffer_usd: 0.0004,
      copied_buffer_usd: 369996.493527375,
      capital_cost_usd: 0,
      ops_overhead_usd: 0.01,
      flashloan_fee_usd: 0.0009,
      relay_fee_usd: 41110.799,
    },
    ...over,
  });

describe("OpportunityTradeCard — CARDS-NOTIONAL-01 SSR gate (one ladder, one notional)", () => {
  it("no cost on a sub-dollar principal, no gross on a zero principal, net == gross − total cost", () => {
    const html = card(mapToOmniOpportunity(probeVsKernelWire()));
    const cells = ledgerCells(html);

    const principal = cellUsd(cell(cells, "Flash loan in (TLS)"));
    const gross = cellUsd(cell(cells, "Gross out (AMM spread)"));
    const totalCost = cellUsd(cell(cells, "Total cost"));
    const net = cellUsd(cells.find((c) => c.label.startsWith("Net yield"))?.value ?? null);
    const repay = cell(cells, "Repay (principal + TLS fee)");

    // (1) NO PRINCIPAL is painted for a basis the wire publishes no notional
    //     for — `IN $0.00` / `IN $1.00` beside a $822k gross IS the defect.
    //     And `Repay` cannot borrow another basis' number to fill the gap.
    expect(principal).toBeNull();
    expect(repay).toBe("—");

    // (2) The ladder is arithmetically CLOSED: net == gross − total cost.
    //     This is doctrine break #2 — `Total cost $72.4k` beside
    //     `Net yield -$0.0000`, off by $72.4k on the live row.
    expect(gross).not.toBeNull();
    expect(net).not.toBeNull();
    expect(totalCost).not.toBeNull();
    expect(Math.abs(net! - (gross! - totalCost!))).toBeLessThanOrEqual(0.01);

    // (3) Cost is non-negative and payable; a painted principal must be able to
    //     carry the gross beside it (the searcher's own 5× gate). Asserted even
    //     though the principal is absent here, so the pair cannot come back
    //     silently in a future change.
    expect(totalCost!).toBeGreaterThanOrEqual(0);
    if (principal != null) {
      expect(totalCost!).toBeLessThanOrEqual(principal + gross!);
      expect(gross!).toBeLessThanOrEqual(principal * 5);
    }

    // The row's real numbers are still shown — quiet is not blindness (R8).
    expect(html).toContain("$822.2k"); // gross, and the derived total cost
    // CARDS-FALSEZERO-01: the kernel's net is -0.000011, and the cell now prints
    // THOSE digits. The previous assertion (`-$0.0000`) passed on the collapse —
    // a nonzero net displayed as exactly zero — and only survived as a substring
    // prefix of the correct value, so it could not detect a regression.
    expect(html).toContain("-$0.000011"); // the kernel's net, verbatim
    expect(html).not.toContain("-$0.0000</span>"); // never a false zero
    // …and the machine reason travels with every suppressed cell.
    expect(html).toContain("CARDS-NOTIONAL-01");
    // OPERATOR ORDER 2026-09-27 (verbatim): "QUITA EL MALDITO RENDER QUE ESCONDE
    // LOS NUMEROS." This row no longer goes quiet, so the "las celdas van en
    // guion a proposito" footer is intentionally GONE — the caveat now travels in
    // each cell's `title` (covered by the CARDS-NOTIONAL-01 assertion above)
    // instead of silencing the capital path. The footer is asserted ABSENT so a
    // regression that brings the blanking back is caught here.
    expect(html).not.toContain('data-testid="ledger-basis-note"');
  });

  it("la escalera canónica pinta SUS filas y no finge las del basis simulado", () => {
    // REAL-LIVE-CARDS-SSOT-01: en la base canónica `Capital cost`,
    // `Failure buffer` y `Copied buffer` pertenecen al basis SIM, así que no se
    // renderizan como filas vacías (ni cero falso ni notional mezclado). Las
    // filas que la base canónica SÍ posee están todas presentes.
    const html = card(mapToOmniOpportunity(probeVsKernelWire()));
    for (const label of [
      "Gas",
      "LP fees",
      "Decoherence (slippage)",
      "TLS fee (flash)",
      "Relay fee",
      "Ops overhead",
    ]) {
      expect(html).toContain(`>${label}</span>`);
    }
    for (const simulatedOnly of ["Capital cost", "Failure buffer", "Copied buffer"]) {
      expect(html).not.toContain(`>${simulatedOnly}</span>`);
    }
  });

  it("a CLOSED ladder at the row's own notional renders the full capital path", () => {
    // The B1 shape: `amount_in_wei` IS the size the economics were computed at
    // (the orchestrator's Sized arm writes the kernel's `optimal_amount_in`
    // there), so principal, gross and every cost share ONE basis.
    const html = card(
      mapToOmniOpportunity(
        probeVsKernelWire({
          amount_in_wei: "1000000000000000000000",
          expected_profit_usd: 25.22783426,
          net_expected_profit_usd: 1.369353192,
          simulated_amount_in_usd: 1000,
          simulated_gross_usd: 25.22783426,
          simulated_costs_total_usd: 23.858481068,
          simulated_net_profit_usd: 1.369353192,
          simulated_cost_breakdown: {
            gas_usd: 0.673172225,
            lp_fees_usd: 3,
            slippage_usd: 5,
            failure_buffer_usd: 0.4,
            copied_buffer_usd: 12.61391713,
            capital_cost_usd: 0,
            ops_overhead_usd: 0.01,
            flashloan_fee_usd: 0.9,
            relay_fee_usd: 1.261391713,
          },
        }),
      ),
    );
    const cells = ledgerCells(html);
    const principal = cellUsd(cell(cells, "Flash loan in (TLS)"));
    const gross = cellUsd(cell(cells, "Gross out (AMM spread)"));
    const totalCost = cellUsd(cell(cells, "Total cost"));
    const net = cellUsd(cells.find((c) => c.label.startsWith("Net yield"))?.value ?? null);

    expect(principal).toBe(1000);
    expect(gross).toBeCloseTo(25.23, 1);
    expect(net).toBeCloseTo(1.37, 1);
    // Closed inside one notional, and payable out of principal + gross.
    expect(Math.abs(net! - (gross! - totalCost!))).toBeLessThanOrEqual(0.01);
    expect(totalCost!).toBeLessThanOrEqual(principal! + gross!);
    expect(gross!).toBeLessThanOrEqual(principal! * 5);
    // The ladder says WHICH producer it is, and the largest component is visible.
    expect(html).toContain("(SIM)");
    expect(html).toContain("$12.6");
  });

  it("R8: a row with NO economics paints dashes with the reason — never a 0", () => {
    const html = card(mapToOmniOpportunity(wire({ token_in: A, token_out: B })));
    const cells = ledgerCells(html);
    expect(cellUsd(cell(cells, "Flash loan in (TLS)"))).toBeNull();
    expect(cellUsd(cell(cells, "Total cost"))).toBeNull();
    expect(cells.find((c) => c.label.startsWith("Net yield"))?.value).toBe("—");
    expect(html).toContain("CARDS-NOTIONAL-01");
  });
});

// ── ALWAYS-COMPUTE (operator mandate 2026-09-27): the FAIL card shows its
// arithmetic, not dashes. "Si el resultado da -$50, la card debe decir -$50,
// no NO COMPUTADO." Everything below is DISPLAY of wire-owned figures — the
// card computes nothing.
describe("OpportunityTradeCard — ALWAYS-COMPUTE FAIL arithmetic", () => {
  const econ = {
    computation_status: "computed",
    error_reason: null,
    amount_in_wei: "1000000000000000000",
    amount_out_wei: "990000000000000000",
    amount_in_usd: 2350,
    amount_out_usd: 2362.5,
    gross_profit_usd: 12.5,
    gas_usd: 0.18,
    dex_fees_usd: null,
    flash_fee_usd: 2.12,
    bribe_usd: 0,
    slippage_usd: null,
    other_costs_usd: 0.01,
    total_cost_usd: 2.31,
    net_profit_usd: -50,
    roi_pct: -2.13,
    target_net_usd: 25,
    target_delta_usd: -75,
    meets_target: false,
    quote_block: 123,
    simulation_block: null,
    legs: [],
    not_computed_reasons: {},
  };

  it("rejected row with computed economics renders ECON chip + target/logrado/delta", () => {
    const opp = mapToOmniOpportunity(
      wire({
        status: "rejected",
        rejection_reason: "non_positive_profit",
        expected_profit_usd: 12.5,
        net_expected_profit_usd: -50,
        economics: econ,
      }),
    );
    const html = card(opp);
    // Honest status chip — COMPUTED ≠ PROFITABLE.
    expect(html).toContain("ECON");
    expect(html).toContain("computed");
    // The FAIL arithmetic from the economics object (target vs achieved vs delta).
    expect(html).toContain("target");
    expect(html).toContain("logrado");
    expect(html).toContain("delta");
    expect(html).toContain("25.00");
    expect(html).toContain("50.00");
  });

  it("error-status economics renders the honest reason, never fabricated numbers", () => {
    const opp = mapToOmniOpportunity(
      wire({
        status: "rejected",
        rejection_reason: "missing_reserves_pool_b",
        expected_profit_usd: null,
        net_expected_profit_usd: null,
        economics: {
          ...econ,
          computation_status: "error",
          error_reason: "missing_reserves_pool_b",
          gross_profit_usd: null,
          net_profit_usd: null,
          target_net_usd: null,
          target_delta_usd: null,
        },
      }),
    );
    const html = card(opp);
    expect(html).toContain("sin quote");
    expect(html).toContain("missing_reserves_pool_b");
  });

  it("canonical-basis ladder fills cost cells from the economics decomposition", () => {
    const opp = mapToOmniOpportunity(
      wire({
        status: "rejected",
        rejection_reason: "non_positive_profit",
        expected_profit_usd: 12.5,
        net_expected_profit_usd: -50,
        economics: econ,
      }),
    );
    const html = card(opp);
    expect(html).toContain("Gas");
    expect(html).toContain("0.18");
    expect(html).toContain("TLS fee");
    expect(html).toContain("2.12");
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// REAL-LIVE-CARDS-SSOT-01 (fixset del operador 2026-09-27) — la celda ROI sigue
// la BASE renderizada, y las bases NUNCA se mezclan:
//   canonical → economics.roi_pct del searcher, luego el campo canónico del wire;
//   simulated → el ratio del forward-sim SOLAMENTE;
//   none      → guion honesto (esa fila es diagnóstico, no card numérica).
// ─────────────────────────────────────────────────────────────────────────────
describe("REAL-LIVE-CARDS-SSOT-01 — la celda ROI sigue la base renderizada", () => {
  const econBase = {
    computation_status: "computed",
    error_reason: null,
    amount_in_wei: "1000000000000000000",
    amount_out_wei: "990000000000000000",
    amount_in_usd: 2350,
    amount_out_usd: 2362.5,
    gross_profit_usd: 12.5,
    gas_usd: 0.18,
    dex_fees_usd: null,
    flash_fee_usd: 2.12,
    bribe_usd: 0,
    slippage_usd: null,
    other_costs_usd: 0.01,
    total_cost_usd: 2.31,
    net_profit_usd: -50,
    roi_pct: -2.13,
    target_net_usd: 25,
    target_delta_usd: -75,
    meets_target: false,
    quote_block: 123,
    simulation_block: null,
    legs: [],
    not_computed_reasons: {},
  };

  // `expected/net` cerrados ⇒ buildLedger elige la base CANÓNICA (la que rige).
  const roiCard = (over: Record<string, unknown>) =>
    card(
      mapToOmniOpportunity(
        wire({ expected_profit_usd: 5, net_expected_profit_usd: 4, ...over }),
      ),
    );

  // La MISMA celda del header: el div cuyo `title` empieza con el rótulo.
  const roiCell = (html: string): string =>
    html.match(/title="Net Convergence Ratio \(ROI %\)[^"]*">[\s\S]*?<\/div>/)?.[0] ?? "";
  const roiText = (html: string): string =>
    roiCell(html).replace(/^[^>]*>/, "").replace(/<\/div>$/, "").replace(/<[^>]*>/g, "").trim();

  it("base canónica: manda economics.roi_pct del searcher", () => {
    const html = roiCard({ economics: { ...econBase, roi_pct: -3.5 }, roi_pct: 1.25 });
    expect(roiText(html)).toBe("-3.50%");
  });

  it("base canónica sin economics.roi_pct: usa el campo canónico del wire", () => {
    const html = roiCard({ roi_pct: 1.25, simulated_roi_pct: 0.75 });
    expect(roiText(html)).toBe("1.25%");
    // Las bases no se mezclan: el ratio del forward-sim no entra en una canónica.
    expect(roiText(html)).not.toContain("0.75");
  });

  it("base canónica con SOLO ratio simulado: guion — no se mezclan bases", () => {
    const html = roiCard({ simulated_roi_pct: 0.75 });
    expect(roiText(html)).toBe("—");
  });

  it("sin economics y sin ratios: guion honesto (R8)", () => {
    const html = roiCard({ roi_pct: null, simulated_roi_pct: null });
    expect(roiText(html)).toBe("—");
  });

  it("NaN no se pinta como ratio (formatPctOrDash degrada a guion)", () => {
    const html = roiCard({ roi_pct: Number.NaN });
    expect(roiText(html)).toBe("—");
    expect(roiCell(html)).not.toContain("NaN");
  });
});
