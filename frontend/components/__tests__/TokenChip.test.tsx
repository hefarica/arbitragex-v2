/**
 * TokenChip tests using react-dom/server renderToStaticMarkup.
 *
 * Why not @testing-library/react:
 *   - Not installed; requires jsdom + operator approval (toolkit-alignment memory).
 *   - TokenChip is a pure component with no hooks, no events tested here.
 *   - renderToStaticMarkup validates all 4 branching cases.
 *
 * Defect #2 trade-off: onError img handler cannot be tested in node env.
 *   - The handler is kept in the implementation (correct browser behavior).
 *   - Deferred to Playwright e2e (Task 13).
 *
 * 4 cases under test:
 *   A. logo_url present          → <img src=... alt=SYMBOL>
 *   B. logo_url null, symbol set → DeterministicAvatar (SVG) + symbol label
 *   C. resolved_via="failed"     → DeterministicAvatar (SVG) + shortAddr
 *   D. info=null                 → DeterministicAvatar (SVG) + shortAddr
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { TokenChip } from "../TokenChip";

const ADDR = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";

describe("TokenChip", () => {
  it("renders <img> with logo when info.logo_url present", () => {
    const html = renderToStaticMarkup(
      <TokenChip
        token_address={ADDR}
        chain_id={1}
        info={{
          symbol: "WETH",
          decimals: 18,
          logo_url: "https://x/logo.png",
          resolved_via: "onchain_full",
        }}
      />
    );
    expect(html).toMatch(/<img[^>]+src="https:\/\/x\/logo\.png"/);
    expect(html).toMatch(/alt="WETH"/);
    expect(html).toContain("WETH");
  });

  it("falls back to DeterministicAvatar when info=null (enricher pending)", () => {
    const html = renderToStaticMarkup(
      <TokenChip token_address={ADDR} chain_id={1} info={null} />
    );
    expect(html).toMatch(/<svg/);
    // shortAddr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2") = "0xc02a…6cc2"
    expect(html).toContain("0xc02a");
  });

  it("falls back to DeterministicAvatar when logo_url=null but symbol present", () => {
    const html = renderToStaticMarkup(
      <TokenChip
        token_address={ADDR}
        chain_id={1}
        info={{
          symbol: "WETH",
          decimals: 18,
          logo_url: null,
          resolved_via: "onchain_full",
        }}
      />
    );
    expect(html).toMatch(/<svg/);
    expect(html).toContain("WETH");
  });

  it("renders shortAddr when resolved_via=failed (all metadata null)", () => {
    const html = renderToStaticMarkup(
      <TokenChip
        token_address={ADDR}
        chain_id={1}
        info={{
          symbol: null,
          decimals: null,
          logo_url: null,
          resolved_via: "failed",
        }}
      />
    );
    // shortAddr produces "0xc02a…6cc2"
    expect(html).toContain("0xc02a");
  });
});
